//! Frame identity across the virtualized path.
//!
//! Two changes landed together and never met each other in a test. The scroll
//! context now reaches a `collection` through the placement walk rather than
//! through an id-prefix guess, so `place_collection` builds its rows on a new
//! code path. Separately, the digest grew a paint-payload hash written when the
//! dispatcher attaches a payload, guarded by
//! [`PetrifiedFrame::paint_hashes_agree`].
//!
//! Every existing `petrify` test builds a stack of text and spacers. None of
//! them materializes a row. So the guard had never run against the path that
//! creates rows, and the digest had never been asked whether it covers a node
//! that exists only because a scroll offset put it on screen. These tests ask.
//!
//! The rows here are deliberately identical in geometry and differ only in a
//! token binding: a digest that noticed the difference by seeing a moved rect
//! would prove nothing about the payload.

use std::ops::Range;
use std::sync::Arc;

use gorgon_petra::frame::{FrameDigest, PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::RowSource;
use gorgon_petra::testing::{Harness, MonoContent};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Key, NodeKind, Props, ViewNode};

const TOTAL_ROWS: usize = 100_000;
const ROW_EXTENT: f32 = 24.0;
const VIEWPORT: Size = Size { w: 200.0, h: 400.0 };

/// A row source whose rows carry a token binding, so a frame can differ in
/// what it paints while every rect stays where it was.
struct TokenRows {
    /// The token every row binds to `background`.
    background: &'static str,
}

impl RowSource for TokenRows {
    fn rows(&mut self, _source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>> {
        let end = range.end.min(TOTAL_ROWS);
        if range.start >= end {
            return Vec::new();
        }
        (range.start..end)
            .map(|index| {
                let mut props = Props {
                    // Fixed width, so every row measures the same whatever its
                    // index: the digest must not be able to tell rows apart by
                    // their extent.
                    text: Some("row".into()),
                    ..Props::default()
                };
                props
                    .tokens
                    .insert("background".into(), self.background.into());
                Arc::new(
                    ViewNode::new(NodeKind::Text, Key::new(format!("row-{index}")))
                        .with_props(props),
                )
            })
            .collect()
    }
}

/// A `scroll` owning its overscan, wrapping a virtualized `collection`.
fn list() -> ViewNode {
    let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(TOTAL_ROWS),
        source: Some("fibers".into()),
        estimated_extent: Some(ROW_EXTENT),
        ..Props::default()
    });
    ViewNode::new(NodeKind::Scroll, "list")
        .with_props(Props {
            // Not DEFAULT_OVERSCAN: a fixture that declares the default cannot
            // tell a value that travelled from one that fell back.
            overscan: Some(120.0),
            ..Props::default()
        })
        .child(collection)
}

fn frame_at(offset: f32, background: &'static str) -> PetrifiedFrame {
    let mut harness = Harness::with(MonoContent::new(), TokenRows { background });
    harness.set_scroll("/list", offset);
    petrify(
        1,
        &list(),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn digest_at(offset: f32, background: &'static str) -> FrameDigest {
    frame_at(offset, background).digest
}

/// The payload hash carried in each placement must describe the payload that
/// placement actually got, on the path that materializes rows too.
///
/// Asserted here rather than left to `petrify`'s `debug_assert`, which a
/// release build compiles away: this is the claim the digest rests on, and a
/// claim that only holds in debug builds is not the claim the contract makes.
#[test]
fn a_virtualized_frame_agrees_with_its_own_paint_hashes() {
    for offset in [0.0, 480.0, 2_399_000.0] {
        let frame = frame_at(offset, "surface.raised");
        assert!(
            frame.paint_hashes_agree(),
            "a placement's paint hash disagrees with its payload at offset {offset}"
        );
        assert_eq!(
            frame.placements.len(),
            frame.content.len(),
            "every placement needs a payload at offset {offset}"
        );
    }
}

/// Virtualization is in force while all of this is true: the frame describes a
/// 100 000-row list with a placement count in the tens.
#[test]
fn the_frame_stays_small_while_the_list_stays_long() {
    let frame = frame_at(480.0, "surface.raised");
    // scroll + collection + rows. The window is the 400-unit viewport plus
    // 120 units of overscan at each end, at 24 units a row.
    let rows = frame.placements.len() - 2;
    assert!(
        (20..=30).contains(&rows),
        "expected a windowed row count, got {rows} placements for {TOTAL_ROWS} rows"
    );
}

/// The digest must be a function of the inputs on this path too, not of
/// anything the row source or the cache carries between passes.
#[test]
fn the_same_scroll_offset_petrifies_to_the_same_digest() {
    let once = digest_at(480.0, "surface.raised");
    let again = digest_at(480.0, "surface.raised");
    assert_eq!(once, again, "the virtualized path must be deterministic");
}

/// The id of the first materialized row in a frame.
fn first_row(frame: &PetrifiedFrame) -> Option<&str> {
    frame
        .placements
        .iter()
        .map(|p| p.id.as_str())
        .find(|id| id.starts_with("/list/rows/row-"))
}

/// Scrolling changes which rows exist and where they sit, so it must change
/// the frame's identity.
///
/// The row-index assertion is the load-bearing half. Without it this test
/// passes even when the derived offset is forced to zero, because the `scroll`
/// container still translates its child by the state offset: the rows move on
/// screen while the *wrong* rows are the ones materialized. That is the same
/// hole the SC-008 seed had, and a digest change alone cannot see it.
#[test]
fn scrolling_the_list_materializes_different_rows_and_moves_the_digest() {
    let top = frame_at(0.0, "surface.raised");
    let down = frame_at(480.0, "surface.raised");

    assert_eq!(
        first_row(&top),
        Some("/list/rows/row-0"),
        "an unscrolled list starts at its first row"
    );
    // The window opens `overscan` before the viewport: (480 - 120) / 24 = 15.
    assert_eq!(
        first_row(&down),
        Some("/list/rows/row-15"),
        "a scrolled list materializes the window its offset selects"
    );
    assert_ne!(
        top.digest, down.digest,
        "two different scroll positions are two different pictures"
    );
}

/// The cross-product this file exists for: a row that exists only because a
/// scroll offset materialized it still has its token binding covered by the
/// digest.
///
/// Both frames place the same rows at the same rects — only the token each row
/// binds differs — so a digest that passes this by noticing geometry cannot.
#[test]
fn a_row_token_binding_changes_a_virtualized_frame_digest() {
    let raised = frame_at(480.0, "surface.raised");
    let sunken = frame_at(480.0, "surface.sunken");

    let raised_rects: Vec<_> = raised
        .placements
        .iter()
        .map(|p| (p.id.as_str(), p.rect))
        .collect();
    let sunken_rects: Vec<_> = sunken
        .placements
        .iter()
        .map(|p| (p.id.as_str(), p.rect))
        .collect();
    assert_eq!(
        raised_rects, sunken_rects,
        "the two frames must be geometrically identical, or this proves nothing"
    );

    assert_ne!(
        raised.digest, sunken.digest,
        "a materialized row's token binding decides the picture and must reach the digest"
    );
}
