//! A column with a scrolling transcript and a fixed composer, seated in a
//! slot of any height: the transcript takes what the composer leaves, never
//! less than its declared minimum, and the composer stays reachable.
//!
//! The shape is a chat tile: `column[scroll(collection), composer]`. Seated
//! straight in a short slot, the stack concedes the transcript first, past
//! its declared minimum when it must (FR-005's second step: a scroll region
//! loses no content by shrinking). Seated under a scroll that declares
//! `fill_viewport`, the column is at least the slot tall: the transcript
//! fills a tall slot, and in a short one the column is its minimum long, the
//! transcript keeps its floor, and the outer scroll reaches the composer.

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Rect, Size};
use gorgon_petra::testing::{GeneratedRows, Harness, MonoContent, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};

/// The transcript's declared floor.
const FLOOR: f32 = 60.0;
/// The composer's fixed height.
const COMPOSER: f32 = 68.0;

fn fixed_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

fn column() -> ViewNode {
    let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
        total_count: Some(10_000),
        source: Some("generated".into()),
        estimated_extent: Some(20.0),
        ..Props::default()
    });
    let mut transcript = ViewNode::new(NodeKind::Scroll, "transcript")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(rows);
    transcript.constraints.vertical.min = Some(FLOOR);
    let mut composer = ViewNode::new(NodeKind::Stack, "composer");
    composer.constraints = fixed_height(COMPOSER);
    ViewNode::new(NodeKind::Stack, "column")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child(transcript)
        .child(composer)
}

fn seat(fill: bool) -> ViewNode {
    ViewNode::new(NodeKind::Scroll, "seat")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            fill_viewport: fill.then_some(true),
            ..Props::default()
        })
        .child(column())
}

fn frame_of(tree: &ViewNode, height: f32) -> PetrifiedFrame {
    let mut harness = Harness::with(MonoContent::new(), GeneratedRows::new("generated", 10_000));
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(300.0, height), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn rect(frame: &PetrifiedFrame, suffix: &str) -> Rect {
    frame
        .placements
        .iter()
        .find(|p| p.id.ends_with(suffix))
        .unwrap_or_else(|| panic!("{suffix} is placed"))
        .rect
}

fn rows_built(frame: &PetrifiedFrame) -> usize {
    frame
        .placements
        .iter()
        .filter(|p| p.id.contains("/rows/"))
        .count()
}

/// The control for the short slot: seated bare, the composer stays whole
/// and the transcript absorbs the overflow past its floor, which is why a
/// chat tile with no floor showed no transcript at all.
#[test]
fn a_bare_short_slot_shrinks_the_transcript_past_its_floor() {
    let frame = frame_of(&column(), 100.0);
    assert_eq!(rect(&frame, "/transcript").h, 100.0 - COMPOSER);
    assert_eq!(rect(&frame, "/composer").h, COMPOSER);
}

/// Under a filling seat, a tall slot: the transcript takes what the
/// composer leaves and the composer sits on the slot's bottom edge.
#[test]
fn a_filling_seat_gives_the_transcript_the_slot_less_the_composer() {
    let frame = frame_of(&seat(true), 600.0);
    let transcript = rect(&frame, "/transcript");
    let composer = rect(&frame, "/composer");
    assert_eq!(transcript.h, 600.0 - COMPOSER);
    assert_eq!(composer.y + composer.h, 600.0);
    assert!(
        rows_built(&frame) < 60,
        "the transcript builds the rows its own viewport shows: {}",
        rows_built(&frame)
    );
}

/// Under a filling seat, a short slot: the column is its minimum long, the
/// transcript keeps its floor, and the composer is below the fold where the
/// seat's own scroll reaches it.
#[test]
fn a_filling_seat_in_a_short_slot_scrolls_to_the_composer() {
    let frame = frame_of(&seat(true), 100.0);
    assert_eq!(rect(&frame, "/transcript").h, FLOOR);
    assert_eq!(rect(&frame, "/column").h, FLOOR + COMPOSER);
    let composer = rect(&frame, "/composer");
    assert_eq!(composer.h, COMPOSER, "the composer is whole");
    assert!(rows_built(&frame) < 60, "{}", rows_built(&frame));
}

/// The control: a seat that does not fill places the column at its
/// natural length, which for a transcript of 10,000 rows is every row's
/// estimate, and builds rows for a viewport that tall.
#[test]
fn a_seat_that_does_not_fill_hands_the_transcript_its_whole_content() {
    let frame = frame_of(&seat(false), 600.0);
    assert!(
        rect(&frame, "/transcript").h > 600.0,
        "without fill the inner scroll is as long as its content"
    );
}
