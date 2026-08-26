//! An anchored placement is a function of its inputs and of nothing else.
//!
//! `contracts/anchored-placement.md` §7, as a test rather than as a promise.
//! The scheme that resolves `Anchor::Node` runs extra walks over the tree,
//! hands their results forward through a map, and shares a measurement cache
//! with the walk that reads them. Every one of those is a place a previous
//! frame, an iteration order, or a warm cache could leak into an answer, and
//! none of them would show up as a crash — they would show up as a popover
//! that lands in a different place on the second run, under a digest that
//! says the two frames are the same picture.
//!
//! So the assertions here are deliberately not "the digest is stable". Two
//! frames can share a digest and disagree about a rect the digest happens not
//! to distinguish, and a test that compared digests alone would pass for a
//! scheme that resolved nothing at all. Every case compares the whole
//! placement list, the whole payload list, and *then* the digest.

use std::sync::Arc;

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::testing::{Harness, gap, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    Align, Anchor, ClampRule, Edge, InputPolicy, Layer, NodeKind, Props, ViewNode,
};

/// One popover in a fixture: everything about it an anchored placement can
/// turn on.
///
/// A named struct rather than a tuple because it carries six things, and a
/// reader of [`four_sided`] should not have to count commas to find out which
/// `Edge` goes with which `Align`.
struct Popover {
    key: &'static str,
    edge: Edge,
    align: Align,
    offset: Option<TokenName>,
    size: Size,
    clamp: ClampRule,
}

/// A control of `size` inside a stack, inset by `pad` from the window's
/// corner, with `surfaces` popovers anchored to it.
///
/// The control is inside a `stack` so that it takes its own natural size
/// rather than the whole window: an anchor rect equal to the viewport would
/// make every edge and every alignment resolve to the same box, and the
/// comparisons below would hold for a scheme that ignored both.
fn anchored(control: Size, pad: f32, surfaces: &[Popover]) -> Arc<ViewNode> {
    let mut button = ViewNode::new(NodeKind::Spacer, "button");
    button.constraints.horizontal.min = Some(control.w);
    button.constraints.horizontal.max = Some(control.w);
    button.constraints.vertical.min = Some(control.h);
    button.constraints.vertical.max = Some(control.h);

    let mut root = ViewNode::new(NodeKind::Overlay, "root").child(
        ViewNode::new(NodeKind::Stack, "bar")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                align: Some(gorgon_petra::geom::Align::Start),
                padding: (pad > 0.0).then(|| {
                    gorgon_petra::tree::InsetRefs::all(gorgon_petra::testing::gap_token(pad))
                }),
                ..Props::default()
            })
            .child(button),
    );
    for popover in surfaces {
        let size = popover.size;
        let mut body = ViewNode::new(NodeKind::Spacer, "body");
        body.constraints.horizontal.min = Some(size.w);
        body.constraints.horizontal.max = Some(size.w);
        body.constraints.vertical.min = Some(size.h);
        body.constraints.vertical.max = Some(size.h);
        root = root.child(
            ViewNode::new(NodeKind::Surface, popover.key)
                .with_props(Props {
                    layer: Some(Layer::Popup),
                    anchor: Some(Anchor::Node {
                        id: "/root/bar/button".into(),
                        edge: popover.edge,
                        align: popover.align,
                        offset: popover.offset.clone(),
                    }),
                    clamp: Some(popover.clamp),
                    input_policy: Some(InputPolicy::DismissOutside),
                    ..Props::default()
                })
                .child(body),
        );
    }
    Arc::new(root)
}

/// Petrify `tree` into a `w` x `h` window from a brand-new harness.
fn frame(tree: &ViewNode, w: f32, h: f32, seq: u64) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        seq,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(w, h), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// Everything a consumer can observe about two frames, compared in the order
/// that makes a failure readable: the placement that differs first, then the
/// payloads, then the digest.
fn assert_same(a: &PetrifiedFrame, b: &PetrifiedFrame, what: &str) {
    assert_eq!(
        a.placements.len(),
        b.placements.len(),
        "{what}: different placement counts"
    );
    for (i, (x, y)) in a.placements.iter().zip(b.placements.iter()).enumerate() {
        assert_eq!(x, y, "{what}: placement {i} differs\n{x:#?}\n{y:#?}");
    }
    assert_eq!(a.content, b.content, "{what}: paint payloads differ");
    assert_eq!(
        a.subtree_hashes, b.subtree_hashes,
        "{what}: subtree hashes differ"
    );
    assert_eq!(a.digest, b.digest, "{what}: digests differ");
}

/// The fixture the flat cases below use: a control off the window's corner
/// with popovers on all four sides, three alignments among them, one gapped
/// and one not, and one `Scroll` clamp beside the `Flip`s.
fn four_sided() -> Arc<ViewNode> {
    anchored(
        Size::new(100.0, 40.0),
        60.0,
        &[
            Popover {
                key: "top",
                edge: Edge::Top,
                align: Align::Start,
                offset: None,
                size: Size::new(40.0, 30.0),
                clamp: ClampRule::Flip,
            },
            Popover {
                key: "bottom",
                edge: Edge::Bottom,
                align: Align::Center,
                offset: gap(8.0),
                size: Size::new(70.0, 30.0),
                clamp: ClampRule::Flip,
            },
            Popover {
                key: "left",
                edge: Edge::Left,
                align: Align::End,
                offset: None,
                size: Size::new(40.0, 30.0),
                clamp: ClampRule::Shrink,
            },
            Popover {
                key: "right",
                edge: Edge::Right,
                align: Align::Center,
                offset: gap(4.0),
                size: Size::new(50.0, 90.0),
                clamp: ClampRule::Scroll,
            },
        ],
    )
}

/// Two petrifications of one anchored tree, from two independent harnesses,
/// are the same frame — placements, payloads and digest alike.
#[test]
fn two_petrifications_of_an_anchored_tree_are_one_frame() {
    let tree = four_sided();
    let first = frame(&tree, 800.0, 600.0, 1);
    let second = frame(&tree, 800.0, 600.0, 1);
    assert_same(&first, &second, "two independent passes");

    // The fixture has to actually anchor something, or every assertion above
    // holds for a tree with no surfaces in it.
    let anchored_count = first
        .placements
        .iter()
        .filter(|p| p.kind == NodeKind::Surface)
        .count();
    assert_eq!(anchored_count, 4, "the fixture places four surfaces");
    assert!(
        first.content.iter().any(|c| c.caret.is_some()),
        "at least one of them must carry a caret, or the payload half of \
         this comparison is vacuous"
    );
}

/// SC-004's shape, on the anchored path: one hundred passes, one answer.
///
/// A hash-map anywhere in the harvest-to-place hand-off would show up here
/// and nowhere else — a `HashMap`'s iteration order is fixed within a process
/// but its *contents* would still be read in an arbitrary order, and a
/// resolution that depended on which anchor was seen first would diverge
/// across seeds.
#[test]
fn a_hundred_passes_over_many_anchors_give_one_digest() {
    let tree = four_sided();
    let first = frame(&tree, 800.0, 600.0, 1);
    for i in 0..100 {
        let again = frame(&tree, 800.0, 600.0, 1);
        assert_same(&first, &again, &format!("pass {i}"));
    }
}

/// No placement depends on frame history.
///
/// The harvest walk shares the measurement cache with the walk that reads its
/// results, which is what keeps it from being a second measurement pass — and
/// is exactly the seam through which an earlier frame could reach a later
/// one. Here one harness places the same tree in three other windows first,
/// filling the cache with entries for every one of these nodes under three
/// other proposals, and then places it at the size under test; the answer
/// must be the frame a cold harness produces.
///
/// The warming tree is the tree under test rather than a different one on
/// purpose. `MeasureCache` is keyed on `(node id, proposal, theme_rev,
/// scale)` and deliberately not on content, so warming with a *different*
/// tree that reused these ids would be a caller error (the host's
/// `ChangeSet` is what invalidates those entries) and would prove nothing
/// about anchoring. Warming with the same tree at other sizes puts real,
/// legitimately-cached entries for these exact nodes in the way.
#[test]
fn an_anchored_frame_does_not_depend_on_what_was_placed_before_it() {
    let tree = four_sided();
    let cold = frame(&tree, 800.0, 600.0, 4);

    let mut harness = Harness::new();
    for (seq, (w, h)) in [(640.0, 480.0), (1024.0, 300.0), (300.0, 900.0)]
        .into_iter()
        .enumerate()
    {
        let _ = petrify(
            seq as u64 + 1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(w, h), ThemeMode::Dark),
            TransitionActivity::default(),
        );
    }
    let warm = petrify(
        4,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(800.0, 600.0), ThemeMode::Dark),
        TransitionActivity::default(),
    );
    assert_same(&cold, &warm, "warm harness against cold");
}

/// The same tree in two windows is two different frames, and the surfaces are
/// what differ.
///
/// The negative half of the property: a resolution that ignored its inputs
/// would satisfy every equality above. This pins that the anchored placements
/// actually respond to the window they are resolved against — the fixture is
/// narrow enough that the right-hand popover cannot stay on the right.
#[test]
fn a_narrower_window_moves_the_anchored_surface_and_the_digest() {
    let tree = anchored(
        Size::new(100.0, 40.0),
        60.0,
        &[Popover {
            key: "right",
            edge: Edge::Right,
            align: Align::Center,
            offset: None,
            size: Size::new(400.0, 30.0),
            clamp: ClampRule::Flip,
        }],
    );
    let wide = frame(&tree, 900.0, 600.0, 1);
    let narrow = frame(&tree, 400.0, 600.0, 1);
    let rect = |f: &PetrifiedFrame| f.placement("/root/right").expect("placed").rect;
    assert_eq!(
        rect(&wide).x,
        160.0,
        "the declared side holds it in a wide window"
    );
    assert!(
        rect(&narrow).x < rect(&wide).x,
        "a narrow window must move it off the declared side: {:?} vs {:?}",
        rect(&narrow),
        rect(&wide)
    );
    assert_ne!(wide.digest, narrow.digest);
}

/// The caret is in the digest, proved from the frame rather than from the
/// hash function.
///
/// `frame::digest`'s own table covers `hash_paint_content` field by field.
/// This is the other end of the same claim: edit the caret on a real
/// petrified frame and the frame stops agreeing with its own digest, which is
/// what `PetrifiedFrame::paint_hashes_agree` exists to say. A caret carried
/// as `PaintContent::custom` — the shape this design refused — would leave
/// this assertion false, because `custom` is hashed as a painter's name and
/// never as what it draws.
#[test]
fn editing_a_caret_makes_a_frame_disagree_with_its_own_digest() {
    let tree = four_sided();
    let mut petrified = frame(&tree, 800.0, 600.0, 1);
    assert!(petrified.paint_hashes_agree());

    let at = petrified
        .content
        .iter()
        .position(|c| c.caret.is_some())
        .expect("the fixture carries a caret");
    let caret = petrified.content[at].caret.as_mut().expect("just found");
    caret.side = caret.side.opposite();
    assert!(
        !petrified.paint_hashes_agree(),
        "a flipped caret must be visible to the digest"
    );
}
