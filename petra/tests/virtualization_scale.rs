//! SC-008 at scale: what a 100 000-row list costs per frame, and what it
//! costs to keep.
//!
//! SC-008 makes four claims about a 100 000-row virtualized list. Two of them
//! already have a test. `tests/virtualized_frame_identity.rs` builds its
//! fixture at `TOTAL_ROWS = 100_000` and
//! `the_frame_stays_small_while_the_list_stays_long` asserts the frame carries
//! 20..=30 row placements — that is *bounded materialization*, proved, and
//! nothing here repeats it. The same file's
//! `scrolling_the_list_materializes_different_rows_and_moves_the_digest`
//! proves the window tracks the offset rather than merely staying small.
//!
//! The two claims left over are the ones this file exists for:
//!
//! * **Frame-rate hold.** "Scrolls at full frame rate" is SC-003's budget: 17
//!   ms at the 99th percentile, one frame at 60 Hz (`spec.md` SC-003, and
//!   FR-034 which defers to it). A placement count says nothing about time —
//!   `measure_collection` answers `total_count * estimated_extent` without
//!   fetching a row, and a regression that started walking rows to answer it
//!   would keep the placement count at 27 while costing 100 000 measurements.
//! * **Memory flat against total row count.** Also invisible to a placement
//!   count, and in one specific way that is not hypothetical: the measurement
//!   cache is keyed on a node id, so without eviction it would hold one entry
//!   per row *ever scrolled past* while every frame stayed 27 placements wide.
//!   `MeasureCache`'s own doc names this as the growth SC-008 forbids
//!   (`layout/proposal.rs`), and `cache.rs`'s module doc quotes SC-008 by
//!   name. Neither had a test that scrolled far enough to reach the bound.
//!
//! # How memory is measured, and what the number is not
//!
//! [`Counting`] is a global allocator for this test binary that keeps a
//! per-thread net of bytes requested minus bytes released. A measurement is
//! the difference between that net before and after building something, taken
//! while the thing is still alive — a *retained* footprint, not a peak and not
//! a churn total. `Vec` capacity counts, which is the honest reading: capacity
//! is what is held.
//!
//! It does not capture the stack, static data, memory the allocator holds back
//! from the OS, per-allocation bookkeeping inside the allocator, or anything
//! allocated on another thread. The counter is per-thread precisely so that
//! the other tests in this binary, which libtest runs concurrently on their
//! own threads, cannot perturb a measurement — and it is sound here only
//! because every allocation a measured region makes is also released on the
//! same thread. That is true of everything below (nothing spawns) and would
//! stop being true if something did.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::MeasureCache;
use gorgon_petra::testing::{GeneratedRows, Harness, MonoContent, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{NodeKind, Props, ViewNode};

// ------------------------------------------------------------- instrument

thread_local! {
    /// Bytes this thread has asked the allocator for, minus bytes it has
    /// given back. `Cell<isize>` has no destructor and its own operations
    /// allocate nothing, so reading it from inside the allocator cannot
    /// recurse and cannot fail during thread teardown.
    static LIVE_BYTES: Cell<isize> = const { Cell::new(0) };
}

/// Add `delta` to this thread's net. `try_with` rather than `with` so that an
/// allocation made before or after this thread's TLS exists is dropped from
/// the count instead of aborting the process.
fn account(delta: isize) {
    let _ = LIVE_BYTES.try_with(|live| live.set(live.get() + delta));
}

/// The system allocator, counting per thread.
struct Counting;

// SAFETY: every method forwards to `System` unchanged and touches only a
// thread-local `Cell<isize>`, which allocates nothing and so cannot re-enter.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            account(layout.size() as isize);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        account(-(layout.size() as isize));
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            account(layout.size() as isize);
        }
        ptr
    }

    /// Forwarded rather than left to the default alloc-copy-free, so that
    /// growing a `Vec` costs what it costs in production: this allocator is
    /// in force for the frame-timing tests too, and an instrument that
    /// changed the thing it measures would be reporting on itself.
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // Only on success: a failed `realloc` leaves the original block
            // allocated and unchanged.
            account(new_size as isize - layout.size() as isize);
        }
        new_ptr
    }
}

/// In force for this test binary only. An integration test is its own crate,
/// so nothing outside this file pays for the counting.
#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// This thread's net live bytes.
fn live_bytes() -> isize {
    LIVE_BYTES.with(Cell::get)
}

// ---------------------------------------------------------------- fixture

/// The row counts compared. Three orders of magnitude apart, which is what
/// turns "flat" into a claim: linear growth would be a hundredfold.
const ROWS_1K: usize = 1_000;
const ROWS_100K: usize = 100_000;

const SOURCE: &str = "fibers";
const ROW_EXTENT: f32 = 24.0;
const OVERSCAN: f32 = 120.0;
const VIEWPORT: Size = Size { w: 200.0, h: 400.0 };

/// SC-003's budget, in milliseconds: one frame at 60 Hz, at the 99th
/// percentile. SC-008's "full frame rate" is this budget applied to a scroll.
const FRAME_BUDGET_MS: f64 = 17.0;

/// Frames per scroll phase. Enough that a 99th percentile is the tenth worst
/// sample of a phase pair rather than the worst — a single descheduling by
/// the OS must not be able to decide the verdict.
const SCROLL_FRAMES: usize = 480;

/// A `scroll` owning the axis and the overscan, wrapping a virtualized
/// `collection` of `total_rows` rows. The same shape
/// `virtualized_frame_identity.rs` uses, so the two files' numbers describe
/// one list.
fn list(total_rows: usize) -> ViewNode {
    let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(total_rows),
        source: Some(SOURCE.into()),
        estimated_extent: Some(ROW_EXTENT),
        ..Props::default()
    });
    ViewNode::new(NodeKind::Scroll, "list")
        .with_props(Props {
            overscan: Some(OVERSCAN),
            ..Props::default()
        })
        .child(collection)
}

/// A harness whose row source synthesizes rows on demand.
///
/// [`GeneratedRows`] holds one `(name, total)` pair and no rows, so its own
/// footprint is the same at a thousand rows and at a hundred thousand. A
/// fixture that materialized its backing store would put its own hundredfold
/// into every measurement below and call it the engine's.
fn harness(total_rows: usize) -> Harness<MonoContent, GeneratedRows> {
    Harness::with(MonoContent::new(), GeneratedRows::new(SOURCE, total_rows))
}

/// The furthest a list of `total_rows` rows can be scrolled before its last
/// row reaches the bottom of the viewport.
fn max_offset(total_rows: usize) -> f32 {
    (total_rows as f32 * ROW_EXTENT - VIEWPORT.h).max(0.0)
}

/// Petrify `tree` at `offset` through `harness`, returning the frame.
///
/// Takes the harness by reference so a caller can run many frames against one
/// cache, which is what a host does and what makes cache growth observable.
fn frame_at(
    harness: &mut Harness<MonoContent, GeneratedRows>,
    tree: &ViewNode,
    seq: u64,
    offset: f32,
) -> PetrifiedFrame {
    harness.set_scroll("/list", offset);
    petrify(
        seq,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// Run one petrify and throw it away, so that every process-wide lazy
/// initialization a measured region would otherwise pay for — the shared
/// fixture theme's `OnceLock`, the allocator's first arena — has already
/// happened.
///
/// Without this the first measurement taken in a test carries the whole
/// static theme, and the first *timed* frame carries a cold instruction
/// cache. Both would be charged to whichever row count happened to go first.
fn warm_up() {
    let mut h = harness(ROWS_1K);
    let tree = list(ROWS_1K);
    let frame = frame_at(&mut h, &tree, 1, 0.0);
    assert!(
        frame.placements.len() > 2,
        "the warm-up must actually materialize rows, or it warms nothing"
    );
}

// ------------------------------------------------------------- statistics

/// The `q`-quantile of `samples` by nearest-rank, sorting in place.
///
/// Nearest-rank rather than an interpolating definition: an interpolated p99
/// of a heavy-tailed latency sample is a number no frame took, and the claim
/// under test is about frames that happened.
fn quantile(samples: &mut [Duration], q: f64) -> Duration {
    assert!(!samples.is_empty(), "a quantile of nothing is nothing");
    samples.sort_unstable();
    let rank = (q * samples.len() as f64).ceil() as usize;
    samples[rank.clamp(1, samples.len()) - 1]
}

/// Milliseconds, as the `SCALE` lines report them.
fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}

/// Time one petrify per frame while `offset` walks `frames` steps of `step`
/// starting at `start`, against one harness and one cache — a scroll, not a
/// sequence of cold starts.
///
/// Returns one sample per frame. The frame is dropped inside the timed region
/// on the *next* iteration rather than this one, so the samples measure
/// petrify and not the deallocation of the frame before it; `previous` holds
/// it across the boundary and the final drop happens after the last sample is
/// taken.
fn scroll_samples(total_rows: usize, start: f32, step: f32, frames: usize) -> Vec<Duration> {
    let mut h = harness(total_rows);
    let tree = list(total_rows);
    let mut samples = Vec::with_capacity(frames);
    let mut previous: Option<PetrifiedFrame> = None;
    for i in 0..frames {
        let offset = start + i as f32 * step;
        h.set_scroll("/list", offset);
        let clock = Instant::now();
        let frame = petrify(
            i as u64 + 1,
            validated(&tree),
            &mut h.ctx(),
            Viewport::new(VIEWPORT, ThemeMode::Dark),
            TransitionActivity::default(),
        );
        samples.push(clock.elapsed());
        // Held, not dropped: a frame whose placements were freed before the
        // clock was read would charge the free to the frame after it.
        previous = Some(frame);
    }
    let held = previous.expect("a scroll runs at least one frame");
    assert!(
        held.placements.len() > 2,
        "the last frame of the scroll materialized no rows; the sweep left the list"
    );
    samples
}

// ------------------------------------------------------------ frame rate

/// A 100 000-row list scrolls inside SC-003's budget, and costs no more per
/// frame than a 1 000-row list does.
///
/// Two assertions, because either alone is weak. The budget alone is an
/// absolute that a fast machine passes while running an algorithm that is
/// linear in `total_count`; the ratio alone would pass a uniform slowdown
/// that blew the budget at both row counts.
///
/// The ratio is taken on medians, not on p99s. A median over 480 frames is
/// insensitive to the handful of samples the OS scheduler decides, which
/// matters here: this suite is expected to run beside compiler jobs. The
/// factor of six is chosen against what it must catch, not against what it
/// must tolerate — an `O(total_count)` regression at a hundred times the rows
/// is a hundredfold or worse, never a few percent.
#[test]
fn a_hundred_thousand_row_scroll_holds_the_frame_budget() {
    warm_up();

    // Half a row per frame from the top of the list: an ordinary drag, where
    // consecutive windows overlap and the measurement cache earns its keep.
    let step = ROW_EXTENT / 2.0;
    let near_1k = scroll_samples(ROWS_1K, 0.0, step, SCROLL_FRAMES);
    let near_100k = scroll_samples(ROWS_100K, 0.0, step, SCROLL_FRAMES);
    // The same drag 99 000 rows deep, where every node id is at its longest
    // and every offset arithmetic is at its largest. Only the long list has
    // such a place, so this half has no 1k counterpart and belongs to the
    // budget assertion rather than to the ratio.
    let deep_start = max_offset(ROWS_100K) - SCROLL_FRAMES as f32 * step;
    let deep_100k = scroll_samples(ROWS_100K, deep_start, step, SCROLL_FRAMES);

    let mut all_100k: Vec<Duration> = near_100k.iter().chain(deep_100k.iter()).copied().collect();
    let p99_100k = quantile(&mut all_100k, 0.99);
    let mut p99_1k_samples = near_1k.clone();
    let p99_1k = quantile(&mut p99_1k_samples, 0.99);

    let mut near_1k_sorted = near_1k;
    let median_1k = quantile(&mut near_1k_sorted, 0.50);
    let mut near_100k_sorted = near_100k;
    let median_100k = quantile(&mut near_100k_sorted, 0.50);

    println!("SCALE frame_p99_ms={:.4}", ms(p99_100k));
    println!("SCALE frame_p99_ms_1k={:.4}", ms(p99_1k));
    println!("SCALE frame_median_ms_100k={:.4}", ms(median_100k));
    println!("SCALE frame_median_ms_1k={:.4}", ms(median_1k));
    println!(
        "SCALE frame_median_ratio={:.3}",
        ms(median_100k) / ms(median_1k)
    );
    println!("SCALE frame_samples_100k={}", all_100k.len());

    assert!(
        ms(p99_100k) <= FRAME_BUDGET_MS,
        "a {ROWS_100K}-row scroll must stay inside SC-003's {FRAME_BUDGET_MS} ms budget; \
         p99 was {:.4} ms over {} frames",
        ms(p99_100k),
        all_100k.len()
    );

    const MAX_MEDIAN_RATIO: f64 = 6.0;
    let ratio = ms(median_100k) / ms(median_1k);
    assert!(
        ratio <= MAX_MEDIAN_RATIO,
        "petrify cost must not follow total_count: the median frame took {:.4} ms at \
         {ROWS_100K} rows against {:.4} ms at {ROWS_1K} rows, a factor of {ratio:.2} \
         against a ceiling of {MAX_MEDIAN_RATIO}",
        ms(median_100k),
        ms(median_1k)
    );
}

// ---------------------------------------------------------------- memory

/// What one settled list retains, and how wide its frame is.
struct Footprint {
    /// Net live bytes held by the harness, the tree, and the frame.
    bytes: isize,
    /// Placements in the frame.
    placements: usize,
    /// Measurement-cache entries held.
    cache_entries: usize,
}

/// Build a list of `total_rows` rows, scroll it to `offset`, and report what
/// is still allocated while all of it is held.
///
/// Nothing prints inside the measured region: libtest captures stdout into a
/// buffer that outlives the region, so a `println!` here would be charged to
/// the engine.
fn footprint(total_rows: usize, offset: f32) -> Footprint {
    let base = live_bytes();

    let mut h = harness(total_rows);
    let tree = list(total_rows);
    // The `Registry` `validated` builds is a temporary of this statement and
    // is released before the next line reads the counter.
    let frame = frame_at(&mut h, &tree, 1, offset);

    let bytes = live_bytes() - base;
    let measured = Footprint {
        bytes,
        placements: frame.placements.len(),
        cache_entries: h.cache.len(),
    };

    // Everything the number covers is released here, and nowhere earlier.
    drop(frame);
    drop(tree);
    drop(h);
    let leaked = live_bytes() - base;
    assert!(
        leaked.abs() < 4_096,
        "the instrument is not reading a retained footprint: {leaked} bytes are \
         unaccounted for after dropping everything the measurement covered"
    );

    measured
}

/// The retained footprint of a settled 100 000-row list is the retained
/// footprint of a settled 1 000-row list.
///
/// Both are scrolled to the same offset, so both hold the same rows under the
/// same ids and `total_count` is the only input that differs. That is what
/// makes the comparison a statement about `total_count` rather than about
/// which rows happened to be on screen.
///
/// `live_nodes_*` is printed for context and is not the claim being made
/// here: bounded materialization is already proved at 100 000 rows by
/// `virtualized_frame_identity.rs`. The claim is `bytes_*`, which covers
/// everything the pass retains and not only what it placed — the measurement
/// cache, the id strings inside it, and the frame's five parallel arrays.
#[test]
fn the_retained_footprint_does_not_grow_with_total_row_count() {
    warm_up();

    // Twenty rows down: the same window in both lists, and far enough in that
    // the window is a window rather than the head of the list.
    const OFFSET: f32 = 480.0;
    let small = footprint(ROWS_1K, OFFSET);
    let large = footprint(ROWS_100K, OFFSET);

    println!("SCALE bytes_1k={}", small.bytes);
    println!("SCALE bytes_100k={}", large.bytes);
    println!("SCALE live_nodes_1k={}", small.placements);
    println!("SCALE live_nodes_100k={}", large.placements);
    println!("SCALE cache_entries_1k={}", small.cache_entries);
    println!("SCALE cache_entries_100k={}", large.cache_entries);
    println!(
        "SCALE bytes_ratio={:.4}",
        large.bytes as f64 / small.bytes.max(1) as f64
    );

    assert!(
        small.bytes > 0 && large.bytes > 0,
        "a settled list retains something; the instrument read {} and {} bytes",
        small.bytes,
        large.bytes
    );

    // A hundredfold in rows may buy the footprint one page and no more. The
    // slack is absolute rather than proportional because the honest
    // expectation is *no* difference: the two frames hold the same rows under
    // the same ids, so anything beyond rounding is the engine holding
    // something per row.
    const SLACK_BYTES: isize = 4_096;
    assert!(
        large.bytes <= small.bytes + SLACK_BYTES,
        "the retained footprint follows total_count: {} bytes at {ROWS_100K} rows \
         against {} bytes at {ROWS_1K} rows, a difference of {} past a slack of \
         {SLACK_BYTES}",
        large.bytes,
        small.bytes,
        large.bytes - small.bytes - SLACK_BYTES
    );

    // The control. Both measurements would also be "flat" if the fixture had
    // stopped materializing anything at all, and a flat footprint over two
    // empty frames proves nothing.
    assert_eq!(
        small.placements, large.placements,
        "both lists must place the same window for the byte comparison to be \
         about total_count at all"
    );
    assert!(
        large.placements > 2,
        "the measured frame placed no rows: {} placements",
        large.placements
    );
}

/// Scrolling a hundred thousand rows past the viewport does not accumulate
/// memory.
///
/// The other memory test settles one list once. This one is the growth SC-008
/// actually forbids and the one the caches were bounded for: the measurement
/// cache is keyed on a node id, every row that enters the window mints a new
/// id, and an unbounded cache would therefore hold one entry per row *ever
/// scrolled past* while every individual frame stayed twenty-seven placements
/// wide. `MeasureCache::DEFAULT_CAPACITY`'s doc says so; nothing until now
/// scrolled far enough to find out.
///
/// The sweep flings rather than drags — each step is wider than the window —
/// so consecutive frames share no rows and every frame is a fresh set of
/// keys. That is the worst case for the cache and the fastest way to a
/// hundred thousand distinct ids.
///
/// The verdict is taken on the *last quarter* of the sweep. Growth up to the
/// bound is allowed and expected: a bounded cache fills before it plateaus,
/// and asserting flatness from the first frame would be asserting that the
/// cache never caches. What must be flat is what happens after it is full.
///
/// That is why the sweep runs the length of the list [`PASSES`] times and
/// wraps rather than stopping at the end. A fling frame of this window adds
/// about 28 cache entries, so a single non-overlapping pass — at most
/// `max_offset / (viewport + 2 * overscan)` frames — cannot fill the shipped
/// 65 536-entry bound until it is around two thirds done, and a plateau
/// asserted over the last quarter of *that* sweep would be asserted over a
/// cache still filling.
#[test]
fn a_long_scroll_past_every_row_does_not_accumulate_memory() {
    warm_up();

    /// Frames in the session. Enough that the shipped cache bound is reached
    /// inside the first half, so the last quarter is measuring a plateau.
    const FRAMES: usize = 6_000;
    /// How many times the session runs the length of the list.
    const PASSES: f32 = 2.0;

    let span = max_offset(ROWS_100K);
    let step = PASSES * span / FRAMES as f32;
    assert!(
        step > VIEWPORT.h + 2.0 * OVERSCAN,
        "a step of {step} would overlap consecutive windows; this sweep is meant to \
         share no rows between frames"
    );

    let base = live_bytes();
    let mut h = harness(ROWS_100K);
    let tree = list(ROWS_100K);

    let mut checkpoints = [0_isize; 4];
    let mut rows_seen = 0_usize;
    for i in 0..FRAMES {
        // Wrapped, so the session keeps flinging past fresh rows for as many
        // frames as it takes rather than for as many as one pass allows.
        let offset = (i as f32 * step) % span;
        let frame = frame_at(&mut h, &tree, i as u64 + 1, offset);
        rows_seen = rows_seen.max(frame.placements.len().saturating_sub(2));
        // Dropped before the checkpoint, so a checkpoint reads what the
        // session *keeps* and never the frame in flight.
        drop(frame);
        let quarter = (i + 1) * 4;
        if quarter.is_multiple_of(FRAMES) {
            checkpoints[quarter / FRAMES - 1] = live_bytes() - base;
        }
    }

    let entries = h.cache.len();
    let evictions = h.cache.evictions();
    let capacity = h.cache.capacity();
    let served = h.rows.served;
    let deepest = h.rows.max_index_seen;

    println!("SCALE session_bytes_q1={}", checkpoints[0]);
    println!("SCALE session_bytes_q2={}", checkpoints[1]);
    println!("SCALE session_bytes_q3={}", checkpoints[2]);
    println!("SCALE session_bytes_q4={}", checkpoints[3]);
    println!(
        "SCALE session_tail_growth_bytes={}",
        checkpoints[3] - checkpoints[2]
    );
    println!("SCALE session_rows_served={served}");
    println!("SCALE session_deepest_row={deepest}");
    println!("SCALE session_cache_entries={entries}");
    println!("SCALE session_cache_evictions={evictions}");

    // The controls come first: without them a plateau could be the plateau of
    // a sweep that never reached the bound, or never left the first screen.
    assert!(
        deepest >= ROWS_100K - 64,
        "the sweep must reach the end of the list; the deepest row materialized was \
         {deepest} of {ROWS_100K}"
    );
    assert!(
        served > ROWS_100K / 2,
        "the sweep must materialize a large fraction of the list to pressure the \
         cache; it served {served} rows"
    );
    assert!(
        evictions > 0,
        "the cache never reached its {capacity}-entry bound ({entries} held), so this \
         test would pass on an unbounded cache and proves nothing"
    );
    assert!(
        entries <= capacity,
        "the bound does not hold: {entries} entries against a capacity of {capacity}"
    );

    // The claim. Once the cache is full the session's footprint stops moving,
    // whatever else scrolls past. The slack is one page per quarter-sweep of
    // a thousand frames.
    const TAIL_SLACK_BYTES: isize = 4_096;
    let tail_growth = checkpoints[3] - checkpoints[2];
    assert!(
        tail_growth <= TAIL_SLACK_BYTES,
        "memory grows with rows scrolled past: the last quarter of a {FRAMES}-frame \
         sweep added {tail_growth} bytes on top of {} already held",
        checkpoints[2]
    );
    assert!(
        rows_seen <= 32,
        "every frame of the sweep must stay windowed; the widest placed {rows_seen} rows"
    );

    drop(tree);
    drop(h);
}

/// Eviction is what makes the sweep above plateau, and the entry bound is
/// what decides where.
///
/// This is the control for the plateau. A plateau on its own is consistent
/// with a session that simply stopped touching new rows, so the same fling is
/// run against two different bounds: if the retained footprint tracks the
/// bound, the bound is what is holding it down. A cache that had quietly
/// stopped evicting, or one whose entries no longer held anything, would
/// answer the same number twice.
///
/// The short sweep here is deliberately *below* the shipped bound's
/// saturation point — a thousand fling frames mint about 28 000 entries
/// against a 65 536-entry cache — which is the second thing this test says:
/// the shipped bound is not reached by a casual scroll, and the previous
/// test has to fling for six thousand frames to reach a plateau at all.
#[test]
fn the_cache_bound_is_what_bounds_the_session() {
    warm_up();

    /// Frames per sweep. Short on purpose: see this test's doc.
    const FRAMES: usize = 1_000;
    /// The thrashing bound. Far below one sweep's working set, so this cache
    /// is evicting for most of the run.
    const TIGHT_CAPACITY: usize = 1_024;

    /// One fling sweep against a cache of `capacity` entries, reporting
    /// `(retained bytes, entries held, evictions)`.
    fn sweep(capacity: usize) -> (isize, usize, u64) {
        let step = max_offset(ROWS_100K) / FRAMES as f32;
        let base = live_bytes();
        let mut h = harness(ROWS_100K);
        h.cache = MeasureCache::with_capacity(capacity);
        let tree = list(ROWS_100K);
        for i in 0..FRAMES {
            drop(frame_at(&mut h, &tree, i as u64 + 1, i as f32 * step));
        }
        let held = live_bytes() - base;
        let out = (held, h.cache.len(), h.cache.evictions());
        drop(tree);
        drop(h);
        out
    }

    let (tight_bytes, tight_entries, tight_evictions) = sweep(TIGHT_CAPACITY);
    let (default_bytes, default_entries, default_evictions) = sweep(MeasureCache::DEFAULT_CAPACITY);

    println!("SCALE bounded_bytes_1024={tight_bytes}");
    println!("SCALE bounded_bytes_default={default_bytes}");
    println!("SCALE bounded_entries_1024={tight_entries}");
    println!("SCALE bounded_entries_default={default_entries}");
    println!("SCALE bounded_evictions_1024={tight_evictions}");
    println!("SCALE bounded_evictions_default={default_evictions}");

    assert!(
        tight_entries <= TIGHT_CAPACITY,
        "a {TIGHT_CAPACITY}-entry bound must hold: {tight_entries} entries"
    );
    assert!(
        tight_evictions > 0,
        "the tight sweep must evict, or the two sweeps differ in nothing but luck"
    );
    assert_eq!(
        default_evictions,
        0,
        "a thousand fling frames must stay inside the shipped {}-entry bound \
         ({default_entries} held); if this sweep now evicts, the previous test's \
         six-thousand-frame sweep is no longer measuring a plateau reached early",
        MeasureCache::DEFAULT_CAPACITY
    );

    // The footprint follows the bound, and by a wide margin rather than by
    // rounding: an eviction that dropped its key's storage but kept the entry
    // would still shrink the number a little.
    assert!(
        tight_bytes * 4 < default_bytes,
        "the entry bound is what decides the session's footprint, but a \
         {TIGHT_CAPACITY}-entry cache retained {tight_bytes} bytes against \
         {default_bytes} at the shipped bound of {} — {default_entries} entries \
         against {tight_entries}",
        MeasureCache::DEFAULT_CAPACITY
    );
}
