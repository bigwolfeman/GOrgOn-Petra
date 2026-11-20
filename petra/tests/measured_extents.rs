//! A virtualized list whose rows do not measure their estimate.
//!
//! Rows alternate 8 and 56 units tall under a 20-unit estimate: every row is
//! wrong, and the list is 32 000 units long where the estimate says 20 000.
//! The ledger of measured extents (`gorgon_petra::layout::extents`) is what
//! makes such a list behave: the end is reachable, and a scroll moves every
//! visible row by exactly the scroll. Each frame here is followed by what a
//! host does after a frame, adding the ledger's anchor shifts to the offset.

use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::RowSource;
use gorgon_petra::testing::{GeneratedRows, Harness, MonoContent, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{NodeKind, Props, ViewNode};

const SOURCE: &str = "rows";
const ESTIMATE: f32 = 20.0;
const VIEW: f32 = 300.0;
const SCROLL: &str = "/list";

fn height(index: usize) -> f32 {
    if index.is_multiple_of(2) { 8.0 } else { 56.0 }
}

struct Mixed {
    total: usize,
}

impl RowSource for Mixed {
    fn rows(&mut self, _source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        range
            .filter(|i| *i < self.total)
            .map(|i| Arc::new(GeneratedRows::row_of_height(i, height(i))))
            .collect()
    }
}

fn list(total: usize) -> ViewNode {
    let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(total),
        source: Some(SOURCE.into()),
        estimated_extent: Some(ESTIMATE),
        ..Props::default()
    });
    ViewNode::new(NodeKind::Scroll, "list")
        .with_props(Props {
            overscan: Some(40.0),
            ..Props::default()
        })
        .child(rows)
}

struct Rig {
    harness: Harness<MonoContent, Mixed>,
    tree: ViewNode,
    seq: u64,
}

impl Rig {
    fn new(total: usize) -> Self {
        Self {
            harness: Harness::with(MonoContent::new(), Mixed { total }),
            tree: list(total),
            seq: 0,
        }
    }

    fn offset(&self) -> f32 {
        self.harness.state.scroll_offset(SCROLL)
    }

    /// One frame at the stored offset, then the host's half.
    fn frame(&mut self) -> PetrifiedFrame {
        self.seq += 1;
        let frame = petrify(
            self.seq,
            validated(&self.tree),
            &mut self.harness.ctx(),
            Viewport::new(Size::new(200.0, VIEW), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        self.harness.apply_shifts();
        frame
    }

    fn scroll_to(&mut self, offset: f32) -> PetrifiedFrame {
        self.harness.set_scroll(SCROLL, offset);
        self.frame()
    }

    /// Frames at the stored offset until the ledger stops changing.
    fn settle(&mut self) -> PetrifiedFrame {
        for _ in 0..16 {
            let revision = self.harness.extents.revision();
            let frame = self.frame();
            if self.harness.extents.revision() == revision {
                return frame;
            }
        }
        panic!("the ledger never settled");
    }
}

/// Every placed row that is wholly or partly in view, by id, with its top in
/// the viewport's frame (a scroll places content at `-offset`, so a row's
/// `y` is already relative to the viewport's top, 0).
fn visible(frame: &PetrifiedFrame) -> Vec<(String, f32)> {
    frame
        .placements
        .iter()
        .filter(|p| p.id.starts_with("/list/rows/row-") && p.is_visible())
        .map(|p| (p.id.clone(), p.rect.y))
        .collect()
}

/// Asking for the end of the list, again and again as a held End key does,
/// reaches the last row with its bottom on the viewport's bottom.
///
/// Before the ledger the content extent was 1000 × 20 = 20 000 against a
/// real 32 000, so the furthest offset showed row 624 or so and the last
/// 375 rows could not be reached at all.
#[test]
fn mixed_heights_reach_the_last_row() {
    let mut rig = Rig::new(1000);
    rig.frame();
    let mut frame = rig.scroll_to(f32::MAX);
    for _ in 0..16 {
        frame = rig.scroll_to(f32::MAX);
        if frame.placement("/list/rows/row-999").is_some() {
            frame = rig.settle();
            break;
        }
    }
    let last = frame
        .placement("/list/rows/row-999")
        .expect("the last row is placed at the end of the list")
        .rect;
    assert_eq!(last.h, height(999));
    assert!(
        (last.y + last.h - VIEW).abs() < 0.5,
        "the last row ends at {} in a {VIEW}-tall viewport",
        last.y + last.h
    );
}

/// A wheel that moves the offset 7 units a frame moves every row that stays
/// visible by exactly 7 units, down from the top and up from the middle.
///
/// Down from the top, rows enter at the bottom already measured. Up from the
/// middle, every row entering at the top is measured for the first time,
/// and it is the anchor shift that keeps the rows below it still. Before the
/// ledger a row jumped by its measure less the estimate, 12 or 36 units,
/// each time the window's first row changed.
#[test]
fn a_wheel_scroll_moves_every_visible_row_by_the_scroll() {
    for (start, step) in [(0.0_f32, 7.0_f32), (16_000.0, -7.0)] {
        let mut rig = Rig::new(1000);
        rig.scroll_to(start);
        let mut before = visible(&rig.settle());
        let mut jumps = Vec::new();
        for n in 0..240 {
            let at = rig.offset();
            let after = visible(&rig.scroll_to(at + step));
            for (id, y) in &after {
                if let Some((_, was)) = before.iter().find(|(other, _)| other == id) {
                    let moved = y - was;
                    if (moved + step).abs() > 1e-3 {
                        jumps.push(format!("frame {n}: {id} moved {moved}"));
                    }
                }
            }
            before = after;
        }
        assert!(
            jumps.is_empty(),
            "a {step}-unit wheel step from {start} moved {} rows by something else:\n{}",
            jumps.len(),
            jumps[..jumps.len().min(12)].join("\n")
        );
    }
}

/// What the ledger costs on a 100 000-row list: a frame that measures new
/// rows and moves the anchor, at offsets spread across the whole list.
///
/// A measurement, not a gate: SC-003's 17 ms budget is asserted by
/// `virtualization_scale.rs` on its own list. This prints the median so the
/// number is on record beside the change that made it.
#[test]
fn the_ledger_on_a_hundred_thousand_rows() {
    const TOTAL: usize = 100_000;
    let mut rig = Rig::new(TOTAL);
    rig.frame();
    let mut samples = Vec::new();
    for i in 0..400_u32 {
        // A stride that is not a multiple of anything in the fixture, so
        // every frame lands on rows the ledger has not seen.
        let at = (f64::from(i) * 7_919.3) % 3_000_000.0;
        rig.harness.set_scroll(SCROLL, at as f32);
        let clock = Instant::now();
        let frame = rig.frame();
        samples.push(clock.elapsed());
        assert!(frame.placements.len() > 2);
    }
    samples.sort_unstable();
    let median = samples[samples.len() / 2];
    let measured = rig
        .harness
        .extents
        .list("/list/rows")
        .map_or(0, gorgon_petra::layout::extents::ListExtents::measured);
    println!(
        "EXTENTS median_ms={:.4} p99_ms={:.4} measured_rows={measured}",
        median.as_secs_f64() * 1000.0,
        samples[samples.len() * 99 / 100].as_secs_f64() * 1000.0
    );
    assert!(measured <= gorgon_petra::layout::extents::ROW_CAP);
}
