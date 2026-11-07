//! `tree_rows` at file-tree scale (spec 016 FR-038, SC-003).
//!
//! A project can list 20,000 entries. The eager `tree_view` would build a
//! placement for every part of every row, every frame. `tree_rows` is the
//! same tree over a `collection`, so a frame carries the rows near the
//! viewport and no others.
//!
//! What this file proves, and what it leaves to others:
//!
//! * **Bounded build.** A 20,000-row tree places one screen of rows plus the
//!   scroll's overscan, counted from the frame and from the rows the source
//!   was asked for. The count does not grow with the total.
//! * **Time.** The same frame builds inside SC-003's 17 ms. Timing on a
//!   shared machine is noisy, so the assertion is on the median of many
//!   frames, and `tests/virtualization_scale.rs` holds the p99 claim for the
//!   shared `collection` path this one rides on.
//! * **Depth is said.** Every mounted row declares `level`, `pos_in_set`,
//!   and `set_size`, and the semantic tree carries the collection's total.
//! * **Focus crosses the edge.** The row after the last visible one is
//!   reachable by keyboard (it sits in the overscan), the host scrolls it
//!   into view, and focus lands on it. At every step the frame stays the
//!   same size.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use gorgon_petra::component::{TreeBranch, TreeItem, TreePlace, tree_rows};
use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::RowSource;
use gorgon_petra::semantic::project;
use gorgon_petra::testing::{Harness, MonoContent, validated_with};
use gorgon_petra::token::{ThemeMode, standard_vocabulary};
use gorgon_petra::tree::{Registry, Role};

const TOTAL: usize = 20_000;
const SOURCE: &str = "project-files";
const ROW: f32 = 32.0;
const VIEWPORT: Size = Size { w: 320.0, h: 320.0 };
/// `Props::scroll`'s default overscan.
const OVERSCAN: f32 = 64.0;

/// The flattened tree a host would hold: one row per index, deterministic.
///
/// Depth walks 1, 2, 3, 3, 2, 1 so every level from 1 to 3 appears, a third
/// of the rows are closed branches, and no row's description needs the
/// whole list.
fn place_of(index: usize) -> TreePlace {
    const DEPTHS: [u32; 6] = [1, 2, 3, 3, 2, 1];
    TreePlace {
        level: DEPTHS[index % DEPTHS.len()],
        pos_in_set: u32::try_from(index % 5).unwrap() + 1,
        set_size: 5,
    }
}

/// The source's row `index`, keyed by index, the way the shell keys by
/// path: stable under scrolling.
fn row(index: usize) -> Arc<gorgon_petra::tree::ViewNode> {
    let branch = if index.is_multiple_of(3) {
        TreeBranch::Closed
    } else {
        TreeBranch::Leaf
    };
    Arc::new(
        TreeItem::new(format!("r{index}"), format!("entry-{index}"))
            .branch(branch)
            .at(place_of(index))
            .build(vec![]),
    )
}

/// A source that builds rows on demand and counts what it was asked for.
#[derive(Default)]
struct Files {
    served: usize,
    max_index: usize,
}

impl RowSource for Files {
    fn rows(
        &mut self,
        source: &str,
        range: Range<usize>,
    ) -> Vec<Arc<gorgon_petra::tree::ViewNode>> {
        assert_eq!(
            source, SOURCE,
            "the tree asked for a source it was not given"
        );
        let end = range.end.min(TOTAL);
        let out: Vec<_> = (range.start.min(end)..end).map(row).collect();
        self.served += out.len();
        self.max_index = self.max_index.max(end.saturating_sub(1));
        out
    }
}

fn registry() -> Registry {
    Registry::with_vocabulary(standard_vocabulary())
}

fn frame_at(harness: &mut Harness<MonoContent, Files>, seq: u64, offset: f32) -> PetrifiedFrame {
    harness.set_scroll("/files", offset);
    let tree = tree_rows("files", "Files", SOURCE, TOTAL);
    petrify(
        seq,
        validated_with(&tree, &registry()),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn harness() -> Harness<MonoContent, Files> {
    Harness::with(MonoContent::new(), Files::default())
}

/// The ids of every row's head, in placement order.
fn head_ids(frame: &PetrifiedFrame) -> Vec<&str> {
    frame
        .placements
        .iter()
        .filter(|p| p.id.ends_with("/row"))
        .map(|p| p.id.as_str())
        .collect()
}

/// A 20,000-row tree builds the rows near the viewport and no others.
///
/// The bound is derived, not observed: the viewport shows `h / 32` rows, the
/// scroll materializes `overscan` past each end, and the collection rounds
/// the window out to whole rows. Counted twice, from placements and from the
/// source's own tally, so a source that was asked for the whole list cannot
/// hide behind a frame that happened to place few.
#[test]
fn a_20000_row_tree_builds_only_the_visible_window() {
    let mut h = harness();
    let frame = frame_at(&mut h, 1, 0.0);

    // At the top there is no overscan above, so the window is the viewport
    // and one overscan below, rounded out by one row at each edge.
    let max_rows = ((VIEWPORT.h + OVERSCAN) / ROW).ceil() as usize + 1;
    let heads = head_ids(&frame);
    assert!(
        heads.len() >= (VIEWPORT.h / ROW) as usize,
        "fewer rows than the viewport shows: {}",
        heads.len()
    );
    assert!(
        heads.len() <= max_rows,
        "placed {} rows, more than the {max_rows} the window allows",
        heads.len()
    );
    assert!(
        h.rows.served <= max_rows,
        "the source was asked for {} rows, more than the {max_rows} the window allows",
        h.rows.served
    );
    assert!(
        h.rows.max_index < max_rows,
        "the source was asked past the window, up to row {}",
        h.rows.max_index
    );

    // A row is a handful of parts, never a subtree that grows: the whole
    // frame is bounded by rows times the parts of one row, plus the list's
    // own placements. 20,000 rows built eagerly would be ~200,000.
    let parts_per_row = 12;
    assert!(
        frame.placements.len() <= max_rows * parts_per_row + 8,
        "{} placements for {} rows",
        frame.placements.len(),
        heads.len()
    );
    assert!(frame.placements.len() < 400, "{}", frame.placements.len());
}

/// Scrolling to the middle and to the end builds as few placements as the
/// top does, and different rows.
#[test]
fn the_window_follows_the_offset_and_stays_the_same_size() {
    let mut h = harness();
    let top = frame_at(&mut h, 1, 0.0);
    let middle = frame_at(&mut h, 2, (TOTAL as f32 / 2.0) * ROW);
    let end = frame_at(&mut h, 3, TOTAL as f32 * ROW - VIEWPORT.h);

    for (name, frame) in [("top", &top), ("middle", &middle), ("end", &end)] {
        assert!(
            frame.placements.len() < 400,
            "{name}: {} placements",
            frame.placements.len()
        );
    }
    let first_of = |frame: &PetrifiedFrame| head_ids(frame)[0].to_owned();
    assert_ne!(first_of(&top), first_of(&middle), "the window did not move");
    assert_ne!(first_of(&middle), first_of(&end));
    assert!(
        head_ids(&end)
            .last()
            .unwrap()
            .contains(&format!("r{}", TOTAL - 1)),
        "the last row of the list is not the last one placed at the end"
    );
}

/// SC-003: 20,000 entries inside 17 ms. The median of 200 frames, scrolling
/// half a row at a time, against one cache: what a scroll drag costs.
#[test]
fn a_20000_row_frame_builds_inside_one_sixty_hertz_frame() {
    let mut h = harness();
    let _warm = frame_at(&mut h, 1, 0.0);
    let mut samples = Vec::with_capacity(200);
    for i in 0..200 {
        h.set_scroll("/files", i as f32 * ROW / 2.0);
        let tree = tree_rows("files", "Files", SOURCE, TOTAL);
        let valid = validated_with(&tree, &registry());
        let clock = Instant::now();
        let frame = petrify(
            i + 2,
            valid,
            &mut h.ctx(),
            Viewport::new(VIEWPORT, ThemeMode::Dark),
            TransitionActivity::default(),
        );
        samples.push(clock.elapsed());
        assert!(!frame.placements.is_empty());
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    println!("TREE_ROWS median_ms={:.4}", median.as_secs_f64() * 1000.0);
    assert!(
        median.as_secs_f64() * 1000.0 < 17.0,
        "median frame {median:?} is over SC-003's 17 ms"
    );
}

/// Every mounted row says where it sits, and the semantic tree carries the
/// total the collection stands for.
#[test]
fn rows_declare_their_place_and_the_tree_declares_its_total() {
    let mut h = harness();
    let frame = frame_at(&mut h, 1, 0.0);
    let semantic = project(&frame).expect("a semantic tree");

    let root = semantic.root();
    assert_eq!(root.role, Some(Role::Tree), "the scroll is the tree");
    let rows = semantic.find("/files/rows").expect("the collection node");
    assert_eq!(rows.total_count, Some(TOTAL));

    let mut seen = 0;
    for placement in frame.placements.iter().filter(|p| p.id.ends_with("/row")) {
        let item_id = placement.id.trim_end_matches("/row");
        let index: usize = item_id.rsplit("/r").next().unwrap().parse().unwrap();
        let state = &semantic.find(item_id).expect("row in the tree").state;
        let want = place_of(index);
        assert_eq!(state.level, Some(want.level), "row {index}");
        assert_eq!(state.pos_in_set, Some(want.pos_in_set), "row {index}");
        assert_eq!(state.set_size, Some(want.set_size), "row {index}");
        seen += 1;
    }
    assert!(seen > 5, "too few rows to say anything: {seen}");
}

/// Keyboard focus crosses the edge of the window.
///
/// Focus the last row that is on screen. The next row in document order
/// is placed but clipped (overscan): `FocusTree::order` does not hold it, so
/// Tab alone would skip it, and `FocusTree::reachable` does name it. A host
/// answers by scrolling it into view, and then focus can land on it.
/// Repeating that 40 rows down the list moves the window each time and never
/// builds more than the same bounded frame.
#[test]
fn keyboard_focus_moves_across_the_edge_of_the_window() {
    let mut h = harness();
    let mut offset = 0.0_f32;
    let mut frame = frame_at(&mut h, 1, offset);
    let scopes = BTreeMap::new();
    let mut focus = FocusTree::from_placements(&frame.placements, &scopes);
    let mut seq = 2;

    // Start on the last fully visible row.
    let visible_rows = (VIEWPORT.h / ROW) as usize;
    let start = format!("/files/rows/r{}", visible_rows - 1);
    focus
        .focus(&start)
        .expect("the last visible row takes focus");

    for step in 0..40 {
        let current = focus.current().expect("focus is somewhere").to_owned();
        let next = focus
            .reachable(&frame.placements, &scopes, 1)
            .expect("there is a next row in document order");
        assert_ne!(next, current, "step {step}: focus did not advance");

        if !focus.order().contains(&next) {
            // The next row is past the visible edge. Scroll one row, as a
            // host does to reveal a focus target, and settle again.
            offset += ROW;
            frame = frame_at(&mut h, seq, offset);
            seq += 1;
            focus = focus.update(&frame.placements, &scopes);
        }
        focus
            .focus(&next)
            .unwrap_or_else(|e| panic!("step {step}: focus refused {next}: {e}"));
        assert_eq!(focus.current(), Some(next.as_str()));
        assert!(
            frame.placements.len() < 400,
            "step {step}: the frame grew to {}",
            frame.placements.len()
        );
    }

    let landed: usize = focus
        .current()
        .unwrap()
        .rsplit("/r")
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(landed, visible_rows - 1 + 40, "focus walked 40 rows down");
    assert!(
        offset > 0.0,
        "the window never moved, so focus never crossed its edge"
    );
}
