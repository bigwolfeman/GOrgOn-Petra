//! What one layout negotiation costs — against tree size, and against the size
//! of the change. The second is FR-035, and FR-035 does not hold.
//!
//! Same instrument as `kernel/benches/provide_scaling.rs`: **the number is the
//! log-log exponent of cost against N**, which is dimensionless and therefore
//! stable across machines and build profiles. The microseconds beside it are
//! context and are not stable. That choice earns its keep here — every number
//! below was taken while a training job held the GPU at 90%+ utilization and
//! 26-31 GB, and while three sibling agents compiled behind a build lock. The
//! exponents reproduced across five runs under that load — the FR-035 number
//! below landed between N^0.98 and N^1.01 on every one of them — while the
//! absolute times moved by up to 10%. Every table below is a single run:
//! `cargo bench -p gorgon-petra`, release, 2026-08-22.
//!
//! # H1 — a cold negotiation costs one pass, and depth costs extra
//!
//! One `petrify`: `measure`, then `place`, then the digest, on a fresh cache.
//! A cold pass legitimately touches every node once, so N^1 is the honest
//! target and not flat.
//!
//! | shape | nodes | cost | per node | exponent |
//! |---|---|---|---|---|
//! | wide stack, depth 2 | 513 → 8 193 | 722 µs → 12.45 ms | 1 408 → 1 520 ns | N^1.03 |
//! | grid, depth 2 | 577 → 9 217 | 670 µs → 12.77 ms | 1 161 → 1 385 ns | N^1.06 |
//! | chain, depth = N | 129 → 1 025 | 2.14 ms → 103.69 ms | 16.6 → 101.2 µs | **N^1.87** |
//!
//! Reporting one exponent for "a tree" would have hidden the third row. A
//! chain is quadratic: **negotiation cost carries a term proportional to a
//! node's depth**, so summing it over a chain of depth d gives d². The cost is
//! not the recursion — hold the node count near 4 000 and trade fan-out for
//! depth and it still climbs, by 3.2x over ten levels:
//!
//! | depth | fan-out | nodes | per node |
//! |---|---|---|---|
//! | 1 | 4 094 | 4 095 | 1 375 ns |
//! | 2 | 63 | 4 033 | 1 952 ns |
//! | 5 | 5 | 3 906 | 3 288 ns |
//! | 11 | 2 | 4 095 | 4 401 ns |
//!
//! Severed, because a plausible reading of the code is not a cause. Every node
//! is looked up by `KeyPath::id()`, a `String` rebuilt from the whole path at
//! every `measure` and every `place`, then hashed and compared as the `node`
//! field of `MeasureKey`. If that is the depth term, then the *characters* in
//! the path must be what is paid for — so the chain was re-measured with only
//! the key length changed:
//!
//! | key length | cost at 1 025 nodes | exponent |
//! |---|---|---|
//! | 1 char | 72.72 ms | N^1.88 |
//! | 6 chars | 104.86 ms | N^1.88 |
//! | 24 chars | 221.20 ms | N^1.92 |
//!
//! Three times the cost for 24x the key text, at the same node count and the
//! same shape: the id string is a real and large part of the per-level cost.
//! The exponent does not move, which is the other half of the conclusion and
//! the more important half — **shortening keys scales the quadratic, it does
//! not remove it.** A path id is proportional to depth whatever the keys are
//! called (a depth-1024 chain's id is 2 048 characters with one-character
//! keys), so a fix has to stop rebuilding and re-hashing whole paths, not
//! shorten them.
//!
//! # H2 — FR-035: measured, and false
//!
//! FR-035: "Per-frame work MUST scale with what changed, not with total tree
//! size: an isolated change in one panel must not re-negotiate every panel."
//! Stated as an experiment: hold the change at one leaf, grow the tree around
//! it, and the cost must not move. Tree: 128 panels of 32 text rows, 4 225
//! nodes. The change widens one leaf's text five-fold, so a correct
//! re-negotiation moves that leaf's placement — which is what `leaf w` checks,
//! because the frame digest moves under every policy (`place` reads paint
//! content off the tree) and therefore proves nothing.
//!
//! | change | policy | invalidate | re-negotiate | hits | misses | leaf w |
//! |---|---|---|---|---|---|---|
//! | 1 leaf | nothing | – | 3.66 ms | 12 673 | 0 | 64 → 64 **stale** |
//! | 1 leaf | global `content_rev` | 60 ns | 9.36 ms | 8 576 | 24 961 | 64 → 320 |
//! | 1 leaf | invalidate leaf only | 36 µs | 3.56 ms | 12 670 | 3 | 64 → 64 **stale** |
//! | 1 leaf | invalidate leaf + ancestors | 74 µs | 3.71 ms | 13 114 | 103 | 64 → 320 |
//! | 8 leaves | invalidate leaf + ancestors | 656 µs | 3.93 ms | 13 520 | 817 | 64 → 320 |
//! | 64 leaves | invalidate leaf + ancestors | 5.49 ms | 6.88 ms | 16 768 | 6 529 | 64 → 320 |
//! | 512 leaves | invalidate leaf + ancestors | 42.40 ms | 8.88 ms | 19 328 | 14 209 | 64 → 320 |
//!
//! Against change size k, at a fixed tree: the shipping policy is k^0.00 — the
//! cost of one leaf's edit is the cost of five hundred and twelve. The best
//! policy the cache allows is k^0.14 in re-negotiation, which is not "scales
//! with the change" either; it is the Θ(nodes) floor drowning k.
//!
//! And FR-035 stated exactly — **one** leaf changes, the tree grows:
//!
//! | nodes | global `content_rev` | leaf + ancestors | misses | warm-walk floor |
//! |---|---|---|---|---|
//! | 529 | 1.13 ms | 481 µs | 103 | 444 µs |
//! | 1 057 | 2.29 ms | 938 µs | 103 | 889 µs |
//! | 2 113 | 4.59 ms | 1.83 ms | 103 | 1.80 ms |
//! | 4 225 | 9.13 ms | 3.69 ms | 103 | 3.64 ms |
//! | 8 449 | 20.07 ms | 7.29 ms | 103 | 7.16 ms |
//! | | **N^1.04** | **N^0.98** | flat | **N^1.00** |
//!
//! The miss count is the line that settles it. Under the best policy it is
//! **103 at every tree size** — the cache is doing exactly its job, serving
//! every unchanged node — and the cost still grows linearly with the tree.
//! So the growth is not measurement. Two unconditional full walks are:
//!
//! 1. `frame::petrify` calls `layout::place`, which recurses into every node,
//!    builds every node's id, and pushes one `Placement` for each. Nothing in
//!    that path consults the cache; there is no way to tell it a subtree did
//!    not move.
//! 2. `frame::digest` then hashes every placement `place` produced.
//!
//! The warm-walk floor column is those two alone, and it is 98% of the best
//! achievable re-negotiation at every size.
//!
//! Under the host that shipped when this was measured there was a third, and
//! it is **fixed as of 2026-08-22**: `petra-egui`'s host wrote one global
//! `App::content_rev` into `LayoutState`, and that number was a field of every
//! `MeasureKey` — so one leaf's edit missed every entry in the cache and
//! re-measured the whole tree. That is the 2.5x between the two cost columns.
//! `App::take_changes` and `MeasureCache::apply` replace it, and the ancestor
//! walk now lives in the crate rather than in this file. The `ChangeSet::All`
//! column below is what that old behaviour cost and is kept as the honest
//! baseline: an application that has not been taught to name what it touched
//! still pays it. `MeasureCache::invalidate_node` existed and
//! would have avoided it, and nothing outside a unit test called it. Two
//! reasons why not, and the table has both: it invalidates the leaf but not the leaf's
//! ancestors, so on its own it produces a **stale frame** — `measure` hits at
//! the root and returns before it ever reaches the change — and it is a
//! `retain` over the whole cache per call, so k of them cost k · entries,
//! which is the k^1.02 invalidation column and 42 ms at k=512, dearer than a
//! cold negotiation of the whole tree.
//!
//! **Verdict: FR-035 is not met, by a factor of the tree.** The bound below
//! enforces it and this bench therefore fails, deliberately, in the way
//! `provide_scaling.rs` failed until the kernel's provide path was fixed.
//! Meeting it needs a `place` that can reuse an unchanged subtree's
//! placements, a digest that can be updated rather than recomputed, and a
//! per-node content revision. It is not a cache tuning.
//!
//! # What this does not measure
//!
//! Text is measured by `testing::MonoContent`, one multiply per run. Real
//! shaping is one to two orders of magnitude dearer, which raises the
//! measurement half against the walk half and would make the cache look much
//! better than it does here — it does not change any exponent, because the
//! floor stays Θ(nodes) either way. FR-035's second sentence, the memory and
//! video-memory baselines, is not measured here at all. Neither is the
//! `collection` path: no shape here virtualizes, so nothing below says what a
//! 100 000-row list costs (SC-008 owns that). And every cache above is sized
//! past its working set on purpose. The shipped
//! `MeasureCache::DEFAULT_CAPACITY` of 8 192 is *below* one cold pass over
//! this 4 225-node tree — 9.04 ms with room, 15.75 ms and 45 825 evictions at
//! the default — so a host with a dense tree pays 1.7x until it calls
//! `with_capacity`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gorgon_petra::frame::petrify_with_memo;
use gorgon_petra::frame::{TransitionActivity, Viewport};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::reuse::{FrameMemo, ReuseStats};
use gorgon_petra::layout::{ChangeSet, MeasureCache};
use gorgon_petra::petrify;
use gorgon_petra::testing::{Harness, MonoContent, NoRows};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Key, KeyPath, NodeKind, Props, TrackSize, ViewNode};

/// The window every experiment negotiates against. Fixed across every ladder:
/// a viewport that grew with N would put a second variable in every exponent.
const VIEWPORT: Size = Size::new(1200.0, 800.0);

/// A ladder rung costing more than this stops that ladder; the exponent is
/// then reported from the rungs that ran.
///
/// This is also what keeps the debug run bounded. `cargo test --all-targets`
/// builds and *runs* this bench in debug, where a rung costs an order of
/// magnitude more than it does here; the same budget that never fires in
/// release truncates every ladder there. The trade is deliberate: the debug
/// run reports an exponent over a shorter ladder rather than taking a minute
/// of the test lane.
const POINT_BUDGET: Duration = Duration::from_millis(120);

/// Largest exponent a cold negotiation of a *shallow* tree may show against
/// node count.
///
/// A cold pass legitimately touches every node: it measures each one, places
/// each one, and hashes each one's placement into the digest. The honest
/// target is N^1, not flat, so this bound exists to refuse a term that is
/// worse than linear in the node count at fixed depth — a container that
/// scans its siblings, a sink that searches before it pushes. 1.25 lets an
/// eightfold N cost about twelvefold, which covers allocator and cache-line
/// drift at these sizes, and refuses N^1.5.
const MAX_COLD_EXPONENT: f64 = 1.25;

/// Largest exponent a cold negotiation of a *chain* may show against depth.
///
/// Not 1.25, and the gap is the finding rather than a concession: this path is
/// quadratic in depth today and the table above says why. This fence refuses a
/// cubic — a third depth-proportional term joining the two already there —
/// and it is not an endorsement of the second.
const MAX_DEEP_EXPONENT: f64 = 2.1;

/// Largest exponent a **one-node change** may show against total tree size.
///
/// This is FR-035 stated as a number. "Per-frame work MUST scale with what
/// changed, not with total tree size" means exactly this experiment: hold the
/// change at one leaf, grow the tree around it, and the cost must not move.
/// The target is N^0. The bound is 0.35 for the reason
/// `kernel/benches/provide_scaling.rs` gives for the same number — it permits
/// an eightfold N to cost twice as much, which covers allocator drift in a
/// bigger tree, and refuses anything carrying a per-node term.
const MAX_CHANGE_EXPONENT: f64 = 0.35;

// ---------------------------------------------------------------- tree shapes

/// One text leaf. The character count is what the fake shaper turns into a
/// width, so editing a leaf's text changes its measured size and not only
/// what it paints.
fn leaf(key: String, chars: usize) -> ViewNode {
    ViewNode::new(NodeKind::Text, Key::new(key)).with_props(Props {
        text: Some("x".repeat(chars)),
        ..Props::default()
    })
}

/// A vertical stack with `n` text children: maximum fan-out, depth 2.
fn wide(n: usize) -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Stack, "root");
    root.children = (0..n)
        .map(|i| Arc::new(leaf(format!("leaf-{i}"), 8)))
        .collect();
    root
}

/// `n` stacks nested one inside the next around one text leaf: maximum depth,
/// fan-out 1. `key_len` sets how long each level's key is, which is the knob
/// the severing experiment turns.
fn deep_with(n: usize, key_len: usize) -> ViewNode {
    // Every level has exactly one child, so sibling keys cannot collide and
    // the same key text is legal at every level. That is what lets the key
    // length be varied without varying anything else about the shape.
    let key = "d".repeat(key_len);
    let mut node = leaf("leaf".into(), 8);
    for _ in 0..n {
        let mut parent = ViewNode::new(NodeKind::Stack, Key::new(key.clone()));
        parent.children = vec![Arc::new(node)];
        node = parent;
    }
    node
}

/// The chain shape at the length real key names run to.
fn deep(n: usize) -> ViewNode {
    deep_with(n, 6)
}

/// A `side x side` grid of text cells: fan-out plus two-pass track resolution.
fn grid(side: usize) -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Grid, "root").with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }; side],
        rows: vec![TrackSize::FitContent; side],
        ..Props::default()
    });
    root.children = (0..side * side)
        .map(|i| Arc::new(leaf(format!("cell-{i}"), 4)))
        .collect();
    root
}

/// A balanced tree of `depth` levels and `fanout` children per level, with
/// text leaves at the bottom. Used to vary depth at a fixed node count.
fn balanced(depth: usize, fanout: usize) -> ViewNode {
    fn build(level: usize, depth: usize, fanout: usize) -> ViewNode {
        if level == depth {
            return leaf(format!("l{level}"), 8);
        }
        let mut node = ViewNode::new(NodeKind::Stack, Key::new(format!("n{level}")));
        node.children = (0..fanout)
            .map(|i| {
                let mut child = build(level + 1, depth, fanout);
                child.key = Key::new(format!("{}-{i}", child.key.as_str()));
                Arc::new(child)
            })
            .collect();
        node
    }
    build(0, depth, fanout)
}

/// A stack of `panels` panels, each a stack of `rows` text rows.
///
/// This is the shape FR-035 is written about — "an isolated change in one
/// panel must not re-negotiate every panel" — so it is the shape every change
/// experiment uses.
fn panelled(panels: usize, rows: usize) -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Stack, "root");
    root.children = (0..panels)
        .map(|p| {
            let mut panel = ViewNode::new(NodeKind::Stack, Key::new(format!("panel-{p}")));
            panel.children = (0..rows)
                .map(|r| Arc::new(leaf(format!("row-{r}"), 8)))
                .collect();
            Arc::new(panel)
        })
        .collect();
    root
}

/// Node count, counted the same way for every shape: every exponent below is
/// against this number, not against a per-shape parameter.
fn node_count(tree: &ViewNode) -> usize {
    1 + tree.children.iter().map(|c| node_count(c)).sum::<usize>()
}

/// The canonical id of leaf `index` in a [`panelled`] tree with `rows` rows
/// per panel.
///
/// Its ancestors are no longer derived here: `MeasureCache::apply` walks them
/// (`KeyPath::ancestor_ids`), which is the change this bench exists to
/// exercise. `Policy::LeafOnly` names only this id, on purpose, to show what
/// happens without that walk.
fn leaf_id(rows: usize, index: usize) -> String {
    format!("/root/panel-{}/row-{}", index / rows, index % rows)
}

// ----------------------------------------------------------------- instrument

/// A harness whose cache is far above the working set, so a number taken from
/// it measures negotiation rather than eviction.
///
/// Sixteen entries per node, not the default 8192 total: a cold pass on these
/// trees interns about six keys per node, and a *change* experiment holds two
/// generations of them at once. Every result below carries its eviction count,
/// and the change experiments assert it is zero, so a wrong guess here fails
/// the bench instead of quietly halving a number.
fn harness(nodes: usize) -> Harness<MonoContent, NoRows> {
    let mut h = Harness::new();
    h.cache = MeasureCache::with_capacity(16 * nodes + 1024);
    h
}

fn viewport() -> Viewport {
    Viewport::new(VIEWPORT, ThemeMode::Dark)
}

/// One full cold negotiation: fresh cache, `measure` + `place` + digest.
///
/// The harness is built outside the timer on purpose. Allocating and dropping
/// a cache sized for N is itself O(N), and folding that in would report an
/// allocator's slope as the layout engine's.
fn cold(tree: &ViewNode, nodes: usize) -> Duration {
    let mut h = harness(nodes);
    let t = Instant::now();
    let frame = petrify(
        1,
        tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let elapsed = t.elapsed();
    assert_eq!(
        frame.placements.len(),
        nodes,
        "a cold negotiation must place every node"
    );
    elapsed
}

/// How a host tells the cache that content moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Policy {
    /// `ChangeSet::All`, applied through `MeasureCache::apply`. This used to
    /// be what `gorgon-petra-egui`'s `Host::pass` actually did — write one
    /// global `App::content_rev` into every `MeasureKey` — before the change
    /// set replaced it; `apply` reproduces that cost exactly for a host that
    /// cannot yet name what moved.
    GlobalRev,
    /// `ChangeSet::Nodes` naming only the changed leaves, applied through
    /// `MeasureCache::apply`. The crate walks each named id's ancestors
    /// itself (`KeyPath::ancestor_ids`) — nothing here writes that loop by
    /// hand any more, which is the fix `LeafOnly` exists to show is
    /// necessary.
    Path,
    /// Invalidate only the changed leaf. Its ancestors stay cached, so
    /// `measure` hits at the root and returns before it ever reaches the leaf.
    LeafOnly,
    /// Tell the cache nothing. The floor: what `place` and the digest cost
    /// over a fully warm cache, which is the work no invalidation policy can
    /// avoid.
    None,
    /// Drop the cache outright. Same effect as `GlobalRev`; kept separate
    /// because it is the sabotage control. If this is not much dearer than
    /// `Path`, the cache is not doing anything and no number here means what
    /// it says.
    Clear,
}

/// One re-negotiation after a change.
#[derive(Clone, Copy, Debug)]
struct Change {
    /// Time spent telling the cache what moved.
    invalidate: Duration,
    /// Time spent re-negotiating.
    renegotiate: Duration,
    hits: u64,
    misses: u64,
    evictions: u64,
    /// Width of the changed leaf's placement after re-negotiation. A policy
    /// that leaves this at the pre-change width produced a stale frame, and
    /// its timing describes work that was never correct.
    changed_width: f32,
    /// That leaf's width before the change, for the same reason.
    width_before: f32,
}

impl Change {
    /// Invalidation and re-negotiation together: what a host pays for the
    /// change, end to end.
    fn total(self) -> Duration {
        self.invalidate + self.renegotiate
    }
}

/// Text a changed leaf is given. Five times the original run, so a correct
/// re-negotiation moves the leaf's placement by a margin no rounding hides.
const CHANGED_TEXT_CHARS: usize = 40;

/// Warm a `panelled` tree, change `k` leaves, and re-negotiate under `policy`.
fn change(panels: usize, rows: usize, k: usize, policy: Policy) -> Change {
    let leaves = panels * rows;
    assert!(k <= leaves, "cannot change more leaves than the tree has");
    let mut tree = panelled(panels, rows);
    let nodes = node_count(&tree);
    let mut h = harness(nodes);

    // Frame 1 warms the cache. Not timed: it is a cold negotiation, and H1
    // already measures those.
    let warm = petrify(
        1,
        &tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let target = "/root/panel-0/row-0";
    let width_before = warm.placement(target).expect("the leaf is placed").rect.w;

    // Spread the changed leaves across panels rather than taking a contiguous
    // run inside one: FR-035's own wording is about panels, and k changes
    // confined to one panel would leave every other panel's ancestor chain
    // untouched and flatter the result.
    let stride = (leaves / k).max(1);
    let touched: Vec<usize> = (0..k).map(|i| (i * stride) % leaves).collect();
    assert_eq!(touched.len(), k);
    assert!(
        touched.contains(&0),
        "the probed leaf must be one of the changed"
    );
    for &index in &touched {
        let panel = Arc::make_mut(&mut tree.children[index / rows]);
        Arc::make_mut(&mut panel.children[index % rows]).props.text =
            Some("x".repeat(CHANGED_TEXT_CHARS));
    }

    let (h0, m0) = h.cache.stats();
    let e0 = h.cache.evictions();

    let t = Instant::now();
    match policy {
        Policy::GlobalRev => h.cache.apply(&ChangeSet::All),
        Policy::Path => {
            let ids: BTreeSet<String> = touched.iter().map(|&index| leaf_id(rows, index)).collect();
            // The ancestor walk is not written here: `apply` owns it, which
            // is the whole point of the change set replacing the global
            // revision.
            h.cache.apply(&ChangeSet::Nodes(ids));
        }
        Policy::LeafOnly => {
            // Deliberately bypasses `apply`: this is the stale-result
            // control, and going through `apply` would walk the ancestors
            // `apply` is supposed to and stop being a control at all.
            for &index in &touched {
                h.cache.invalidate_node(&leaf_id(rows, index));
            }
        }
        Policy::None => {}
        Policy::Clear => h.cache.clear(),
    }
    let invalidate = t.elapsed();

    let t = Instant::now();
    let frame = petrify(
        2,
        &tree,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let renegotiate = t.elapsed();

    let (h1, m1) = h.cache.stats();
    // The digest moves under every policy, because `place` reads paint content
    // straight off the tree — which is exactly why the digest is not what says
    // whether the layout was re-negotiated. `changed_width` is.
    assert_ne!(warm.digest, frame.digest, "the edit must move the frame");
    Change {
        invalidate,
        renegotiate,
        hits: h1 - h0,
        misses: m1 - m0,
        evictions: h.cache.evictions() - e0,
        changed_width: frame.placement(target).expect("still placed").rect.w,
        width_before,
    }
}

/// Median of `reps` runs. Odd counts only, so the median is a value that was
/// measured rather than the mean of two that were.
///
/// Not an average: this bench shares a machine with whatever else is building,
/// and one interference spike on one rung is enough to swing an exponent taken
/// from two endpoints. A median discards the spike; a real regression moves
/// every round together and survives it.
fn median<T: Copy, F: FnMut() -> T>(reps: usize, key: fn(T) -> Duration, mut f: F) -> T {
    assert_eq!(reps % 2, 1, "an even round count has no measured median");
    let mut v: Vec<T> = (0..reps).map(|_| f()).collect();
    v.sort_unstable_by_key(|&x| key(x));
    v[v.len() / 2]
}

fn median_dur<F: FnMut() -> Duration>(reps: usize, f: F) -> Duration {
    median(reps, |d| d, f)
}

/// Rounds per rung; the median is kept.
///
/// One in debug. `cargo test --all-targets` runs this bench in debug, where no
/// bound is enforced and the numbers are informational — a median of three
/// informational numbers is worth less than the twelve seconds of test lane it
/// costs. Release, where the bounds are calibrated and enforced, keeps three.
const ROUNDS: usize = if cfg!(debug_assertions) { 1 } else { 3 };

/// Exponent `e` such that `cost ~ N^e`, from the first and last rung that ran.
///
/// Endpoints rather than a least-squares fit over every rung, for the reason
/// `provide_scaling.rs` gives: the small rungs carry a fixed setup cost that
/// biases a fit downward, and a biased-down exponent is the one that lets a
/// regression through.
/// One incremental re-negotiation of a `panelled` tree after one leaf's text
/// widens, with every untouched panel handed back as the same allocation.
///
/// This is what a host that shares its subtrees actually pays, and it is the
/// number FR-035 is a claim about. `change` above measures the same edit under
/// a full negotiation; the difference between them is what reuse buys.
///
/// The tree is rebuilt the way an incremental application rebuilds one: a new
/// root, a new panel 0 holding a new row 0, and `Arc::clone` for every other
/// panel. Cloning an `Arc` is a refcount bump, so nothing under those panels
/// is touched, allocated, or copied.
fn change_incremental(panels: usize, rows: usize) -> (Duration, ReuseStats, f32) {
    let before_panels: Vec<Arc<ViewNode>> = (0..panels)
        .map(|p| {
            let mut panel = ViewNode::new(NodeKind::Stack, Key::new(format!("panel-{p}")));
            panel.children = (0..rows)
                .map(|r| Arc::new(leaf(format!("row-{r}"), 8)))
                .collect();
            Arc::new(panel)
        })
        .collect();
    let root_of = |ps: &[Arc<ViewNode>]| {
        let mut root = ViewNode::new(NodeKind::Stack, "root");
        root.children = ps.to_vec();
        Arc::new(root)
    };
    let before = root_of(&before_panels);
    let nodes = node_count(&before);
    let mut h = harness(nodes);

    let warm = petrify(
        1,
        &before,
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let memo = FrameMemo::adopt(
        Arc::clone(&before),
        warm,
        h.state.clone(),
        h.theme_rev,
        h.scale,
    );

    // Panel 0 rebuilt with a wider row 0; every other panel handed straight
    // back.
    let mut after_panels = before_panels.clone();
    let mut panel0 = ViewNode::new(NodeKind::Stack, Key::new("panel-0"));
    panel0.children = (0..rows)
        .map(|r| {
            Arc::new(leaf(
                format!("row-{r}"),
                if r == 0 { CHANGED_TEXT_CHARS } else { 8 },
            ))
        })
        .collect();
    after_panels[0] = Arc::new(panel0);
    let after = root_of(&after_panels);

    let target = "/root/panel-0/row-0";
    let changes = ChangeSet::Nodes(BTreeSet::from([target.to_owned()]));

    let t = Instant::now();
    h.cache.apply(&changes);
    let dirty = memo
        .dirty_ids(&changes, &h.state)
        .expect("ChangeSet::Nodes has a dirty set");
    let (frame, stats) = petrify_with_memo(
        2,
        &after,
        &mut h.ctx(),
        &memo,
        &dirty,
        viewport(),
        TransitionActivity::default(),
    );
    let elapsed = t.elapsed();

    let width = frame
        .placement(target)
        .expect("the changed leaf is placed")
        .rect
        .w;
    assert_eq!(
        frame.placements.len(),
        nodes,
        "an incremental frame must still place every node"
    );
    (elapsed, stats, width)
}

fn exponent(points: &[(usize, Duration)]) -> Option<f64> {
    let (n0, t0) = *points.first()?;
    let (n1, t1) = *points.last()?;
    if n1 <= n0 || t0.is_zero() || t1.is_zero() {
        return None;
    }
    Some((t1.as_secs_f64() / t0.as_secs_f64()).ln() / ((n1 as f64) / (n0 as f64)).ln())
}

fn slope(points: &[(usize, Duration)]) -> f64 {
    exponent(points).unwrap_or(f64::NAN)
}

// ------------------------------------------------------------------ H1 and H2

/// H1: one cold negotiation against node count, for one tree shape.
fn h1(label: &str, ladder: &[usize], build: impl Fn(usize) -> ViewNode) -> Option<f64> {
    println!("\nH1 {label}: cold negotiation (measure + place + digest), fresh cache");
    let mut points: Vec<(usize, Duration)> = Vec::new();
    for &param in ladder {
        let tree = build(param);
        let nodes = node_count(&tree);
        let elapsed = median_dur(ROUNDS, || cold(&tree, nodes));
        let per_node = elapsed.as_secs_f64() * 1e9 / nodes as f64;
        println!("  nodes={nodes:6}  {elapsed:>10.2?}  {per_node:8.1} ns/node");
        points.push((nodes, elapsed));
        if elapsed > POINT_BUDGET {
            println!(
                "  ladder stopped after nodes={nodes}: the rung cost {elapsed:.2?}, over the \
                 {POINT_BUDGET:.0?} budget. The exponent uses the rungs that ran."
            );
            break;
        }
    }
    println!(
        "  cold negotiation ~ N^{:.2} over nodes={}..{}",
        slope(&points),
        points.first().map_or(0, |p| p.0),
        points.last().map_or(0, |p| p.0)
    );
    exponent(&points)
}

fn print_change(label: &str, c: &Change) {
    println!(
        "  {label:<26} invalidate {:>9.2?}  renegotiate {:>9.2?}  hits {:>6}  misses {:>6}  \
         evict {:>5}  leaf w {:.0}->{:.0}",
        c.invalidate, c.renegotiate, c.hits, c.misses, c.evictions, c.width_before, c.changed_width
    );
}

fn main() {
    // 256 MiB of stack. The chain shape nests a thousand stacks, and every
    // level costs several recursive frames in `measure` and several more in
    // `place`; a debug build's frames are large enough that the 8 MiB default
    // overflows, and a bench that aborts is not a measurement.
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(run)
        .expect("bench thread")
        .join()
        .expect("bench thread");
}

#[allow(clippy::too_many_lines)]
fn run() {
    println!(
        "petra layout negotiation: exponent e in cost ~ N^e. The exponent is the claim — it is \
         dimensionless and survives a loaded machine. The absolute times are context and are not."
    );

    let e_wide = h1("wide stack", &[512, 1024, 2048, 4096, 8192], wide);
    let e_grid = h1("grid      ", &[24, 34, 48, 68, 96], grid);
    let e_deep = h1("deep chain", &[128, 256, 512, 1024], deep);

    // H1d: the same node count at four depths. If the chain's exponent were
    // recursion overhead, holding the node count fixed would hold the cost
    // fixed too.
    println!("\nH1d depth at a held node count: about 4 000 nodes, fan-out traded for depth");
    for (depth, fanout) in [(1usize, 4094usize), (2, 63), (5, 5), (11, 2)] {
        let tree = balanced(depth, fanout);
        let nodes = node_count(&tree);
        let elapsed = median_dur(ROUNDS, || cold(&tree, nodes));
        println!(
            "  depth={depth:3}  fanout={fanout:5}  nodes={nodes:6}  {elapsed:>10.2?}  {:8.1} ns/node",
            elapsed.as_secs_f64() * 1e9 / nodes as f64
        );
    }

    // H1s: the severing. Key length is the only thing that changes, so a cost
    // that follows it is a cost paid per character of `KeyPath::id()`.
    println!("\nH1s severing the chain's depth term: the same shape with shorter keys");
    for key_len in [1usize, 6, 24] {
        let mut points: Vec<(usize, Duration)> = Vec::new();
        for &n in &[128usize, 256, 512, 1024] {
            let tree = deep_with(n, key_len);
            let nodes = node_count(&tree);
            let elapsed = median_dur(ROUNDS, || cold(&tree, nodes));
            points.push((nodes, elapsed));
            if elapsed > POINT_BUDGET {
                break;
            }
        }
        let ladder: Vec<String> = points.iter().map(|(n, t)| format!("{n}:{t:.2?}")).collect();
        println!(
            "  key length {key_len:2} chars: ~ N^{:.2}   [{}]",
            slope(&points),
            ladder.join("  ")
        );
    }
    println!(
        "  a path id at depth 1024 is {} chars with 6-char keys and {} with 1-char keys.",
        chain_id_len(1024, 6),
        chain_id_len(1024, 1)
    );

    // ---- H2: cost against change size, tree held fixed.
    const ROWS: usize = 32;
    const PANELS: usize = 128;
    let fixed_nodes = node_count(&panelled(PANELS, ROWS));
    println!(
        "\nH2a change size at a fixed tree: {PANELS} panels x {ROWS} rows = {fixed_nodes} nodes. \
         FR-035 says the cost must follow k, not the tree."
    );
    let floor = median(ROUNDS, Change::total, || {
        change(PANELS, ROWS, 1, Policy::None)
    });
    print_change("k=0 nothing (STALE)", &floor);
    let mut global: Vec<(usize, Duration)> = Vec::new();
    let mut path_re: Vec<(usize, Duration)> = Vec::new();
    let mut path_inv: Vec<(usize, Duration)> = Vec::new();
    for k in [1usize, 8, 64, 512] {
        let g = median(ROUNDS, Change::total, || {
            change(PANELS, ROWS, k, Policy::GlobalRev)
        });
        print_change(&format!("k={k:<4} global rev"), &g);
        global.push((k, g.total()));
        let p = median(ROUNDS, Change::total, || {
            change(PANELS, ROWS, k, Policy::Path)
        });
        print_change(&format!("k={k:<4} path invalidate"), &p);
        assert_eq!(p.evictions, 0, "the change cache must not thrash");
        path_re.push((k, p.renegotiate));
        path_inv.push((k, p.invalidate));
        if p.total() > POINT_BUDGET {
            println!("  ladder stopped after k={k}: over the {POINT_BUDGET:.0?} budget.");
            break;
        }
    }
    println!(
        "  vs k: global rev ~ k^{:.2}; path re-negotiation ~ k^{:.2}; \
         path invalidation ~ k^{:.2}",
        slope(&global),
        slope(&path_re),
        slope(&path_inv)
    );

    // ---- H2c: FR-035 stated exactly. One leaf changes; the tree grows.
    println!("\nH2c FR-035 exactly: ONE leaf changes, the tree grows around it. Target: flat.");
    let mut one_global: Vec<(usize, Duration)> = Vec::new();
    let mut one_path: Vec<(usize, Duration)> = Vec::new();
    let mut one_floor: Vec<(usize, Duration)> = Vec::new();
    let mut one_incremental: Vec<(usize, Duration)> = Vec::new();
    for panels in [16usize, 32, 64, 128, 256] {
        let nodes = node_count(&panelled(panels, ROWS));
        let g = median(ROUNDS, Change::total, || {
            change(panels, ROWS, 1, Policy::GlobalRev)
        });
        let p = median(ROUNDS, Change::total, || {
            change(panels, ROWS, 1, Policy::Path)
        });
        let f = median(ROUNDS, Change::total, || {
            change(panels, ROWS, 1, Policy::None)
        });
        println!(
            "  nodes={nodes:6}  global rev {:>9.2?} ({:>5} misses)  path {:>9.2?} ({:>3} misses)  \
             warm-walk floor {:>9.2?} ({} misses)",
            g.renegotiate, g.misses, p.renegotiate, p.misses, f.renegotiate, f.misses
        );
        // The same edit, but the host hands back the panels it did not touch.
        let (inc, stats, w) = change_incremental(panels, ROWS);
        assert!(
            w > 64.0,
            "the incremental pass must produce the post-change layout, not a stale one: \
             the changed leaf is {w:.0} wide and was 64 before the edit"
        );
        assert_eq!(
            stats.reused_subtrees,
            panels - 1,
            "every panel but the changed one should have been carried over"
        );
        println!(
            "  nodes={nodes:6}  incremental {inc:>9.2?}  ({} of {nodes} nodes carried over, {} rebuilt)",
            stats.reused_nodes, stats.replaced_nodes
        );
        one_global.push((nodes, g.renegotiate));
        one_path.push((nodes, p.renegotiate));
        one_floor.push((nodes, f.renegotiate));
        one_incremental.push((nodes, inc));
        if p.renegotiate > POINT_BUDGET {
            println!("  ladder stopped after nodes={nodes}: over the {POINT_BUDGET:.0?} budget.");
            break;
        }
    }
    println!(
        "  one-leaf change ~ N^{:.2} (global rev), N^{:.2} (path invalidate), \
         N^{:.2} (warm-walk floor), N^{:.2} (incremental)",
        slope(&one_global),
        slope(&one_path),
        slope(&one_floor),
        slope(&one_incremental)
    );
    let e_change = exponent(&one_incremental);

    // ---- Sabotage: sever a known cause; the number must move the way it must.
    println!("\nSabotage: sever a known cause and check the number moves.");
    let control = median(ROUNDS, Change::total, || {
        change(PANELS, ROWS, 1, Policy::Path)
    });
    let cleared = median(ROUNDS, Change::total, || {
        change(PANELS, ROWS, 1, Policy::Clear)
    });
    let leaf_only = median(ROUNDS, Change::total, || {
        change(PANELS, ROWS, 1, Policy::LeafOnly)
    });
    print_change("k=1 path (control)", &control);
    print_change("k=1 cache cleared", &cleared);
    print_change("k=1 leaf only (STALE)", &leaf_only);
    println!(
        "  cache cleared costs {:.2}x the path-invalidated re-negotiation, on {} misses \
         against {}.",
        cleared.renegotiate.as_secs_f64() / control.renegotiate.as_secs_f64(),
        cleared.misses,
        control.misses
    );
    println!(
        "  staleness: path w={:.0} (correct), leaf-only w={:.0}, no invalidation w={:.0}, \
         before the edit w={:.0}.",
        control.changed_width, leaf_only.changed_width, floor.changed_width, control.width_before
    );
    assert!(
        control.changed_width > control.width_before,
        "the control policy must produce the post-change layout, or every number here \
         describes work that was never correct"
    );
    assert_eq!(
        leaf_only.changed_width, leaf_only.width_before,
        "invalidating only the leaf must leave the frame stale: its ancestors stay cached \
         and `measure` returns at the root"
    );

    // ---- The shipping cache bound, against the same tree. Every number
    // above comes from a cache sized well past the working set, so it
    // measures negotiation; this is what the shipped default does instead.
    let tree = panelled(PANELS, ROWS);
    let unbounded_cold = median_dur(ROUNDS, || cold(&tree, fixed_nodes));
    let mut bounded = Harness::new();
    let capacity = bounded.cache.capacity();
    let t = Instant::now();
    let frame = petrify(
        1,
        &tree,
        &mut bounded.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let at_default = t.elapsed();
    assert_eq!(frame.placements.len(), fixed_nodes);
    println!(
        "\nMeasureCache::DEFAULT_CAPACITY is {capacity} entries. One cold pass on this \
         {fixed_nodes}-node tree costs {unbounded_cold:.2?} with room to spare and \
         {at_default:.2?} at the default, on {} evictions.",
        bounded.cache.evictions()
    );

    if cfg!(debug_assertions) {
        println!("\n(debug build: numbers printed, bounds enforced in release only)");
        return;
    }
    for (label, e, bound) in [
        ("wide stack", e_wide, MAX_COLD_EXPONENT),
        ("grid", e_grid, MAX_COLD_EXPONENT),
        ("deep chain", e_deep, MAX_DEEP_EXPONENT),
    ] {
        match e {
            Some(e) if e <= bound => {}
            Some(e) => panic!(
                "a cold negotiation of the {label} shape costs N^{e:.2}, over the N^{bound} \
                 bound"
            ),
            None => panic!("{label}: exponent unmeasurable; too few rungs ran to report a slope"),
        }
    }
    match e_change {
        Some(e) if e <= MAX_CHANGE_EXPONENT => println!("\nFR-035: within bound"),
        Some(e) => panic!(
            "FR-035 is not met: one leaf's edit costs N^{e:.2} in the size of the tree around \
             it, over the N^{MAX_CHANGE_EXPONENT} bound, against a target of flat. This is the \
             *incremental* column: the measurement cache is invalidated by path, unchanged \
             subtrees are carried over from the previous frame, and their subtree hashes are \
             reused instead of refolded. Only 34 of these nodes are rebuilt at any tree size. \
             What is left is not a walk that can be skipped: the container above the change \
             must re-run its arrangement, because a resized child moves its siblings, and that \
             arrangement is O(children). The bench grows panels, so that term grows with the \
             tree. Making this bound hold needs a container that can update one child's slot \
             without re-running the whole arrangement when every sibling's main-axis extent is \
             untouched -- a per-container incremental arrangement, which is a larger piece of \
             work than subtree reuse and should be measured against these numbers before \
             anyone decides it is worth building"
        ),
        None => panic!("FR-035 exponent unmeasurable; too few rungs ran to report a slope"),
    }
}

/// Length of a chain node's canonical id at `depth`, with `key_len`-character
/// keys: one `/` and `key_len` characters per level.
fn chain_id_len(depth: usize, key_len: usize) -> usize {
    let mut path = KeyPath::root();
    for _ in 0..depth {
        path.push(Key::new("d".repeat(key_len)));
    }
    path.id().len()
}
