//! Interaction state is never a measure input.
//!
//! `contracts/interaction-state.md` §9. `LayoutCtx` hands the whole
//! [`LayoutState`] to every container, and that handoff cannot be removed —
//! `scroll_offsets` needs it — so the rule that `hovered`, `pressed` and
//! `capture` are read only *after* geometry has run cannot be enforced by a
//! type. This file is the gate that enforces it instead.
//!
//! # Why it matters more than it looks
//!
//! A layout that got wider on hover would re-lay-out the page under the
//! pointer, which is the reflow every hover-driven design system exists to
//! avoid. Worse for Petra specifically: it would make a frame's geometry a
//! function of where a mouse happens to be resting, so an idle window would
//! never be byte-identical twice and SC-002's sixty-second zero-idle
//! assertion would be measuring the mouse.
//!
//! # What "byte-identical" means here
//!
//! Every coordinate is compared through `f32::to_bits`, not with `==`. Two
//! rects that print the same can differ in the last bit, and a container that
//! nudged a child by one ULP on hover would pass a printed comparison and
//! still put a different frame on the wire — the digest hashes the bits.

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Point, Size};
use gorgon_petra::input::{Capture, PointerButton};
use gorgon_petra::layout::{LayoutState, Slot};
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{
    AxisConstraint, Constraints, Interaction, NodeKind, Props, Role, ViewNode,
};

fn viewport() -> Viewport {
    Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark)
}

/// A page with enough shape for a measure difference to have somewhere to
/// show up: a horizontal stack with a gap, two fixed controls, and a text run
/// that wraps against whatever width is left.
fn page() -> ViewNode {
    let control = |key: &str| {
        ViewNode::new(NodeKind::Spacer, key)
            .with_constraints(Constraints {
                horizontal: AxisConstraint {
                    min: Some(80.0),
                    max: Some(120.0),
                    priority: 5,
                },
                vertical: AxisConstraint {
                    min: Some(40.0),
                    max: Some(40.0),
                    priority: 10,
                },
            })
            .interactive(
                Role::Button,
                key.to_owned(),
                &[
                    Interaction::Drag,
                    Interaction::Hover,
                    Interaction::Click,
                    Interaction::Focus,
                ],
            )
    };
    ViewNode::new(NodeKind::Stack, "app")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            spacing: Some(gorgon_petra::testing::gap_token(8.0)),
            ..Props::default()
        })
        .child(control("left"))
        .child(control("right"))
        .child(ViewNode::new(NodeKind::Text, "caption").with_props(Props {
            text: Some("Fibers waiting on the scheduler".into()),
            ..Props::default()
        }))
}

fn place(state: LayoutState) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.bind_tree_gaps(&page());
    harness.state = state;
    petrify(
        1,
        validated(&page()),
        &mut harness.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

/// Every measured number in a frame, as raw bits, in placement order.
///
/// Rects **and** offered slots. The rect is what a node took; the slot is
/// what it was offered, and a container that changed the offer while the
/// child clamped back to the same size would move the second and not the
/// first — which is a changed negotiation reported as an unchanged one.
fn geometry(frame: &PetrifiedFrame) -> Vec<(String, [u32; 8])> {
    let bits = |slot: &Slot, rect: gorgon_petra::geom::Rect| {
        [
            rect.x.to_bits(),
            rect.y.to_bits(),
            rect.w.to_bits(),
            rect.h.to_bits(),
            slot.rect.x.to_bits(),
            slot.rect.y.to_bits(),
            slot.rect.w.to_bits(),
            slot.rect.h.to_bits(),
        ]
    };
    frame
        .placements
        .iter()
        .zip(frame.slots.iter())
        .map(|(p, slot)| (p.id.clone(), bits(slot, p.rect)))
        .collect()
}

fn snapshot(hovered: Option<&str>, pressed: Option<&str>, captured: Option<&str>) -> LayoutState {
    LayoutState {
        hovered: hovered.map(str::to_owned),
        pressed: pressed.map(str::to_owned),
        capture: captured.map(|node| Capture {
            node: node.to_owned(),
            button: PointerButton::Primary,
            origin: Point::new(10.0, 10.0),
            last: Point::new(10.0, 10.0),
        }),
        ..LayoutState::default()
    }
}

/// The gate: no interaction state moves a single measured bit.
///
/// Six snapshots against one baseline, covering each of the three members on
/// its own, the two ends of a hover move, and the pressed-plus-captured pair a
/// held button is actually in.
#[test]
fn no_interaction_state_moves_a_measured_rect() {
    let baseline = place(LayoutState::default());
    let want = geometry(&baseline);
    assert_eq!(want.len(), 4, "app, two controls, one caption");

    let cases: [(&str, LayoutState); 6] = [
        ("hover left", snapshot(Some("/app/left"), None, None)),
        ("hover right", snapshot(Some("/app/right"), None, None)),
        ("hover caption", snapshot(Some("/app/caption"), None, None)),
        ("pressed", snapshot(None, Some("/app/left"), None)),
        ("captured", snapshot(None, None, Some("/app/left"))),
        (
            "held",
            snapshot(Some("/app/left"), Some("/app/left"), Some("/app/left")),
        ),
    ];
    for (what, state) in cases {
        let frame = place(state);
        assert_eq!(
            geometry(&frame),
            want,
            "`{what}` changed the geometry; interaction state reached a measure \
             (contracts/interaction-state.md §9)"
        );
    }
}

/// The other half of the same claim, and the reason the one above is not
/// vacuous: the state **did** arrive, it just did not reach a measure.
///
/// Without this a `semantics_of` that dropped the three flags on the floor
/// would pass the geometry gate perfectly.
#[test]
fn the_hover_that_moved_no_rect_still_moved_the_picture() {
    let resting = place(LayoutState::default());
    let hovered = place(snapshot(Some("/app/left"), None, None));

    let lit: Vec<&str> = hovered
        .placements
        .iter()
        .filter(|p| p.semantics.hovered)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(lit, vec!["/app/left"], "the flag has to have landed");
    assert_ne!(
        resting.digest, hovered.digest,
        "two frames differing only in hover are two different pictures \
         (contracts/interaction-state.md §3, FR-013)"
    );

    let pressed = place(snapshot(Some("/app/left"), Some("/app/left"), None));
    assert_ne!(
        hovered.digest, pressed.digest,
        "hovered and pressed are two pictures too"
    );
}

/// Both ends of an interaction-state move are dirty, the same treatment focus
/// gets.
///
/// `contracts/interaction-state.md` §9's last line. The incremental path
/// copies an unchanged subtree forward whole, so a node whose *only* change is
/// a hover flag would otherwise be carried over still carrying the flag it had
/// last frame — a row that stays lit after the pointer moves off it, on a page
/// where nothing else changed. Focus has been in this set since it existed;
/// these three were not, which was latent for exactly as long as nothing ever
/// set them.
///
/// **Both** ends, because a move has two: the node being left has to repaint
/// as much as the node being entered.
#[test]
fn a_hover_press_or_capture_move_marks_both_ends_dirty() {
    use gorgon_petra::layout::ChangeSet;
    use gorgon_petra::layout::reuse::FrameMemo;
    use std::sync::Arc;

    let before = snapshot(Some("/app/left"), None, None);
    let memo = FrameMemo::adopt(
        Arc::new(page()),
        place(before.clone()),
        before,
        1,
        gorgon_petra::geom::Scale::ONE,
    );

    let cases: [(&str, LayoutState, &[&str]); 4] = [
        (
            "hover moved",
            snapshot(Some("/app/right"), None, None),
            &["/app/left", "/app/right"],
        ),
        ("hover cleared", LayoutState::default(), &["/app/left"]),
        (
            "pressed",
            snapshot(Some("/app/left"), Some("/app/left"), None),
            &["/app/left"],
        ),
        (
            "captured",
            snapshot(Some("/app/left"), None, Some("/app/right")),
            &["/app/right"],
        ),
    ];
    for (what, now, want) in cases {
        let dirty = memo
            .dirty_ids(&ChangeSet::None, &now)
            .expect("`None` is a dirty set, not a dirty everything");
        for id in want {
            assert!(
                dirty.contains(*id),
                "`{what}` left `{id}` clean, so a reused subtree keeps painting \
                 the state it was built in: {dirty:?}"
            );
        }
    }

    // And a snapshot that did not move dirties nothing, or every frame under a
    // resting pointer would re-place the whole page.
    let unchanged = snapshot(Some("/app/left"), None, None);
    assert!(
        memo.dirty_ids(&ChangeSet::None, &unchanged)
            .expect("a dirty set")
            .is_empty(),
        "a pointer that did not move dirtied something"
    );
}
