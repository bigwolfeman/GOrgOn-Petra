//! `Props::text_scale` is an environment value: declared once on a
//! container, every run of text under it is measured and painted at that
//! scale, rows a collection materializes later included, nested
//! declarations multiply, and an incremental frame never carries a subtree
//! placed under one scale into another.
//!
//! `MonoContent` multiplies its advance and line height by the request's
//! scale, as the egui shaper multiplies the style's size, so a width read off
//! a placement is the scale read off the picture.

use std::sync::Arc;

use gorgon_petra::frame::{
    PetrifiedFrame, TransitionActivity, Viewport, petrify, petrify_with_memo,
};
use gorgon_petra::geom::{Axis, Scale, Size};
use gorgon_petra::layout::ChangeSet;
use gorgon_petra::layout::reuse::FrameMemo;
use gorgon_petra::testing::{GeneratedRows, Harness, MonoContent, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Key, NodeKind, Props, Registry, ViewNode, Violation, validate};

const VIEWPORT: Size = Size { w: 800.0, h: 600.0 };

fn viewport() -> Viewport {
    Viewport::new(VIEWPORT, ThemeMode::Dark)
}

fn label(key: &str, text: &str) -> Arc<ViewNode> {
    Arc::new(
        ViewNode::new(NodeKind::Text, Key::new(key)).with_props(Props {
            text: Some(text.to_owned()),
            ..Props::default()
        }),
    )
}

/// A stack that declares `scale` (or nothing) around `children`.
fn zone(key: &str, scale: Option<f32>, children: &[Arc<ViewNode>]) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Stack, Key::new(key)).with_props(Props {
        axis: Some(Axis::Vertical),
        text_scale: scale,
        ..Props::default()
    });
    for child in children {
        node = node.child_shared(Arc::clone(child));
    }
    node
}

fn full(tree: &ViewNode) -> PetrifiedFrame {
    let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("rows", 1000));
    petrify(
        1,
        validated(tree),
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

/// The width of the text placement whose id ends with `suffix`, and the
/// scale its paint payload carries.
fn text_at(frame: &PetrifiedFrame, suffix: &str) -> (f32, Scale) {
    let index = frame
        .placements
        .iter()
        .position(|p| p.id.ends_with(suffix))
        .unwrap_or_else(|| panic!("{suffix} is placed"));
    let paint = frame.content[index]
        .text
        .as_ref()
        .unwrap_or_else(|| panic!("{suffix} paints text"));
    (frame.placements[index].rect.h, paint.scale)
}

/// One declaration scales the text under it and nothing beside it; a
/// nested one multiplies; the painter is told the same scale the measure
/// used, so the run is shaped at the size it was laid out at.
///
/// Falsify by dropping the `slot.text_scaled` call in `place_node`: the
/// payload keeps scale 1 while the measure used 2.
#[test]
fn a_declared_scale_reaches_every_run_under_it_and_nested_scales_multiply() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .child_shared(label("plain", "abc"))
        .child(zone("doc", Some(2.0), &[label("big", "abc")]).child(zone(
            "inner",
            Some(1.5),
            &[label("bigger", "abc")],
        )));
    let frame = full(&tree);
    let (plain, plain_scale) = text_at(&frame, "/plain");
    let (big, big_scale) = text_at(&frame, "/doc/big");
    let (bigger, bigger_scale) = text_at(&frame, "/inner/bigger");
    assert_eq!(plain_scale, Scale::ONE);
    assert_eq!(big_scale, Scale::new(2.0).unwrap());
    assert_eq!(bigger_scale, Scale::new(3.0).unwrap());
    assert_eq!(big, plain * 2.0, "a line is twice as tall at scale 2");
    assert_eq!(bigger, plain * 3.0, "2 x 1.5 is 3");
}

/// A collection's rows are not in the tree; the engine fetches them during
/// placement. They still take the scale of the container around the list.
#[test]
fn rows_a_collection_materializes_take_the_scale_around_it() {
    let list = |scale: Option<f32>| {
        ViewNode::new(NodeKind::Scroll, "doc")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                text_scale: scale,
                ..Props::default()
            })
            .child(
                ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                    total_count: Some(1000),
                    source: Some("rows".into()),
                    estimated_extent: Some(16.0),
                    ..Props::default()
                }),
            )
    };
    let plain = full(&list(None));
    let zoomed = full(&list(Some(2.0)));
    let (h1, s1) = text_at(&plain, "/rows/row-0");
    let (h2, s2) = text_at(&zoomed, "/rows/row-0");
    assert_eq!(s1, Scale::ONE);
    assert_eq!(s2, Scale::new(2.0).unwrap());
    assert_eq!(h2, h1 * 2.0);
}

/// The ancestor changes its scale while the subtree under it is the same
/// allocation and is declared clean. Pointer identity alone says "reuse",
/// and reuse would paint the old size under a digest that says the frame is
/// current. The scale rides the slot, so the slot differs and the subtree is
/// placed again: the memo path equals a full negotiation.
///
/// Falsify by comparing slots without their `text_scale` in
/// `ReuseState::reusable`: `/doc/panel/body` is carried over at height 16
/// and the placements differ (checked 2026-11-23).
#[test]
fn a_changed_scale_is_never_carried_over_on_the_memo_path() {
    // A fixed box, so the panel's own slot rect is the same under both
    // scales and only the scale tells the two slots apart.
    let mut boxed = zone("panel", None, &[label("body", "some words")]);
    boxed.constraints.horizontal.min = Some(300.0);
    boxed.constraints.horizontal.max = Some(300.0);
    boxed.constraints.vertical.min = Some(100.0);
    boxed.constraints.vertical.max = Some(100.0);
    let panel = Arc::new(boxed);
    let aside = label("aside", "untouched");
    let tree = |scale: Option<f32>| {
        Arc::new(
            ViewNode::new(NodeKind::Stack, "root")
                .with_props(Props {
                    axis: Some(Axis::Vertical),
                    ..Props::default()
                })
                .child_shared(Arc::clone(&aside))
                .child(zone("doc", scale, &[]).child_shared(Arc::clone(&panel))),
        )
    };
    let before = tree(None);
    let after = tree(Some(2.0));

    let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("rows", 0));
    let first = petrify(
        1,
        validated(&before),
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let memo = FrameMemo::adopt(
        Arc::clone(&before),
        first,
        h.state.clone(),
        h.theme_rev,
        h.scale,
    );
    // What a host reports: the node whose props changed, and nothing under it.
    let changes = ChangeSet::Nodes(["/root/doc".to_owned()].into_iter().collect());
    h.cache.apply(&changes);
    let dirty = memo
        .dirty_ids(&changes, &h.state)
        .expect("a node set has a dirty set");
    let (memoed, stats) = petrify_with_memo(
        2,
        validated(&after),
        &mut h.ctx(),
        &memo,
        &dirty,
        viewport(),
        TransitionActivity::default(),
    );
    let fresh = full(&after);
    assert!(
        stats.reused_nodes > 0,
        "nothing was reused, so nothing was tested: {stats:?}"
    );
    assert_eq!(memoed.placements, fresh.placements, "placements differ");
    assert_eq!(memoed.content, fresh.content, "paint payloads differ");
    assert_eq!(memoed.digest, fresh.digest, "digests differ");
    let (height, scale) = text_at(&memoed, "/doc/panel/body");
    assert_eq!(scale, Scale::new(2.0).unwrap());
    assert_eq!(height, text_at(&full(&before), "/doc/panel/body").0 * 2.0);
}

/// A scale that is not finite and positive is refused at acceptance, where
/// every other out-of-range prop is.
#[test]
fn a_scale_that_is_not_finite_and_positive_is_refused() {
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let tree = zone("doc", Some(bad), &[label("x", "x")]);
        let errors = validate(&tree, &Registry::new()).expect_err("refused");
        assert!(
            errors.as_slice().iter().any(|e| matches!(
                e.violation,
                Violation::ValueOutOfRange {
                    prop: "text_scale",
                    ..
                }
            )),
            "{bad}: {errors}"
        );
    }
}
