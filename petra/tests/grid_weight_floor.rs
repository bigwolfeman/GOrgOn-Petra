//! What a weighted grid track keeps beside fit-content tracks.
//!
//! A weight track is the leftover. Before 2026-11-20 a fit-content track
//! could take all of it; round 4 reserved the weight tracks' minimums first.
//! Reserving their *content* minimum was wrong: a pagination bar is
//! `[page size (fit), range (weight), page numbers (fit), page controls
//! (fit)]`. The range cell's content minimum is its padding and one
//! character, 28 units, and reserving it clipped the previous and next
//! buttons off the end of the controls, so a click on `next` hit nothing
//! (`gorgon-lua-gallery`'s pagination test). Only a
//! minimum the author declared (`constraints.min`) is reserved: the tree
//! that declares a floor keeps it, and an expendable caption still gives way.

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Rect, Size};
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{NodeKind, Props, TextWrap, TrackSize, ViewNode};

fn text(key: &str, body: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(body.to_owned()),
        wrap: Some(TextWrap::Ellipsis),
        ..Props::default()
    })
}

fn frame_of(tree: &ViewNode, width: f32) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(width, 100.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn rect(frame: &PetrifiedFrame, id: &str) -> Rect {
    frame
        .placement(id)
        .unwrap_or_else(|| panic!("{id} is placed"))
        .rect
}

/// `[range (weight), numbers (fit), next (fit)]` at 8 units a character.
/// The range cell holds a 28-unit inset and its caption, so its content
/// minimum is 28 whether or not the cell itself declares one.
fn bar(range_floor: Option<f32>) -> ViewNode {
    let mut inset = ViewNode::new(NodeKind::Stack, "inset");
    inset.constraints.horizontal.min = Some(28.0);
    let mut range = ViewNode::new(NodeKind::Stack, "range")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(inset)
        .child(text("caption", "1-10 of 50 items"));
    range.constraints.horizontal.min = range_floor;
    ViewNode::new(NodeKind::Grid, "bar")
        .with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::FitContent,
            ],
            rows: vec![TrackSize::FitContent],
            ..Props::default()
        })
        .child(range)
        .child(text("numbers", "1 2 3 4 5"))
        .child(text("next", "Next"))
}

/// A bar narrower than everything: the range gives way and the controls
/// keep their width, because the range cell declared no floor of its own.
/// Falsify by reserving the weight track's content minimum (its `Zero`
/// answer) in `grid::spend_declared`: `next` is cut to 0.
#[test]
fn a_caption_with_no_declared_floor_gives_way_to_the_controls() {
    let frame = frame_of(&bar(None), 9.0 * 8.0 + 4.0 * 8.0);
    assert_eq!(rect(&frame, "/bar/numbers").w, 9.0 * 8.0);
    let next = rect(&frame, "/bar/next");
    assert_eq!(next.w, 4.0 * 8.0, "next keeps its width");
    assert_eq!(rect(&frame, "/bar/range").w, 0.0, "the range gives way");
}

/// A weight track whose child declares a floor keeps it, and the
/// fit-content tracks share what is left.
#[test]
fn a_declared_floor_on_a_weight_track_is_reserved() {
    let frame = frame_of(&bar(Some(40.0)), 120.0);
    assert_eq!(rect(&frame, "/bar/range").w, 40.0);
    assert_eq!(rect(&frame, "/bar/numbers").w, 9.0 * 8.0);
    assert_eq!(rect(&frame, "/bar/next").w, 120.0 - 40.0 - 72.0);
}

/// The vertical case that motivated the reservation: a tree with a declared
/// floor above a long fit-content list keeps its floor.
#[test]
fn a_floored_row_above_a_long_fit_content_row_keeps_its_floor() {
    let mut tree = ViewNode::new(NodeKind::Stack, "tree");
    tree.constraints.vertical.min = Some(30.0);
    let mut list = ViewNode::new(NodeKind::Stack, "list").with_props(Props {
        axis: Some(Axis::Vertical),
        ..Props::default()
    });
    for i in 0..20 {
        list = list.child(text(&format!("r{i}"), "row"));
    }
    let grid = ViewNode::new(NodeKind::Grid, "g")
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
            ..Props::default()
        })
        .child(tree)
        .child(list);
    let frame = frame_of(&grid, 200.0);
    assert_eq!(rect(&frame, "/g/tree").h, 30.0);
    assert_eq!(rect(&frame, "/g/list").h, 100.0 - 30.0);
}
