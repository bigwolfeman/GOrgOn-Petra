//! The change-set safety property: naming a change moves the frame, and
//! naming nothing leaves it stale.
//!
//! `MeasureCache::apply` is exactly the call `gorgon-petra-egui`'s
//! `Host::pass` makes every frame (`petra-egui/src/host.rs`); this isolates
//! it from the UI host so the hazard the design accepts is visible on its
//! own rather than buried inside a paint test.
//! `.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`
//! §1: "A host that reports an incomplete change set ships a stale frame.
//! That is the hazard this design accepts in exchange for the speed, and
//! `ChangeSet::All` is the escape."

use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::layout::ChangeSet;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Key, NodeKind, Props, ViewNode};

const LEAF_ID: &str = "/root/panel-0/row-0";

/// Two panels of one text row each, so the leaf sits two levels under the
/// root and an ancestor walk actually has something to climb.
fn tree(leaf_text: &str) -> ViewNode {
    let panel = |name: &str, text: &str| {
        ViewNode::new(NodeKind::Stack, Key::new(name)).child(
            ViewNode::new(NodeKind::Text, Key::new("row-0")).with_props(Props {
                text: Some(text.to_owned()),
                ..Props::default()
            }),
        )
    };
    ViewNode::new(NodeKind::Stack, "root")
        .child(panel("panel-0", leaf_text))
        .child(panel("panel-1", "unrelated"))
}

fn viewport() -> Viewport {
    Viewport::new(Size::new(1200.0, 800.0), ThemeMode::Dark)
}

/// Naming the changed leaf (and letting the crate walk its ancestors) is
/// what makes the ancestor invalidation load-bearing rather than decorative:
/// the leaf's placement must actually reflect the edit.
#[test]
fn a_named_change_moves_the_leafs_placement() {
    let mut h = Harness::new();
    let warm = petrify(
        1,
        validated(&tree("x")),
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let before = warm.placement(LEAF_ID).expect("the leaf is placed").rect.w;

    h.cache.apply(&ChangeSet::node(LEAF_ID));
    let after = petrify(
        2,
        validated(&tree("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")), // 42 chars, 8 up from 1
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let width = after.placement(LEAF_ID).expect("still placed").rect.w;

    assert!(
        width > before,
        "a correctly applied change set must move the changed leaf's \
         placement: {before} -> {width}"
    );
}

/// The documented contract, not a bug: an edit the host never reports stays
/// invisible to the cache, so the ancestor's cached measurement answers for
/// it and the frame is stale. This is what makes `ChangeSet::All` the
/// required escape for a host that cannot track ids, rather than an
/// optimization it can skip.
#[test]
fn an_unreported_change_leaves_the_frame_stale() {
    let mut h = Harness::new();
    let warm = petrify(
        1,
        validated(&tree("x")),
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let before = warm.placement(LEAF_ID).expect("the leaf is placed").rect.w;

    h.cache.apply(&ChangeSet::None);
    let after = petrify(
        2,
        validated(&tree("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx")),
        &mut h.ctx(),
        viewport(),
        TransitionActivity::default(),
    );
    let width = after.placement(LEAF_ID).expect("still placed").rect.w;

    assert_eq!(
        width, before,
        "ChangeSet::None must reuse the stale cached measurement — this is \
         the hazard the design accepts, documented rather than hidden"
    );
}
