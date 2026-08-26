//! Hover derivation and pointer capture, driven through the real router.
//!
//! `contracts/interaction-state.md` §1 makes hover an *engine-derived* state:
//! it is `hit_test(frame, pos, Interaction::Hover)` and nothing else, so the
//! control that lights up and the control that a click reaches cannot be two
//! different nodes (FR-009). §7 puts capture on the same owner, because a
//! gesture is the one thing a frame cannot remember for itself.
//!
//! Nothing here hand-sets an interaction flag. Every test drives
//! [`PointerState`] with the events a device produces, publishes the result
//! into [`LayoutState`] the way a host does, and re-places the tree — so what
//! is under test is the whole path from an event to a painted flag, not a
//! struct literal.

use std::collections::BTreeMap;

use gorgon_petra::focus::FocusTree;
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Point, Rect, Size};
use gorgon_petra::input::{
    CancelReason, Capture, GestureOutcome, InputEvent, KeyCode, Modifiers, PointerButton,
    PointerState, Route, hit_test, required_interaction, required_interaction_during, route,
};
use gorgon_petra::layout::LayoutState;
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{
    AxisConstraint, Constraints, InputPolicy, Interaction, NodeKind, Role, ViewNode,
};

const VIEWPORT: Size = Size { w: 400.0, h: 200.0 };

fn viewport() -> Viewport {
    Viewport::new(VIEWPORT, ThemeMode::Dark)
}

/// No open surfaces: every test here is a plain page unless it says otherwise.
fn no_surfaces() -> BTreeMap<String, InputPolicy> {
    BTreeMap::new()
}

/// A fixed-size leaf declaring `actions`, with the role and label every
/// actionable node owes the audit.
fn control(key: &str, size: (f32, f32), actions: &[Interaction]) -> ViewNode {
    ViewNode::new(NodeKind::Spacer, key)
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(size.0),
                max: Some(size.0),
                priority: 10,
            },
            vertical: AxisConstraint {
                min: Some(size.1),
                max: Some(size.1),
                priority: 10,
            },
        })
        .interactive(Role::Button, key.to_owned(), actions)
}

/// Place `tree` against `state`. One helper, so a test can never accidentally
/// place two frames under two different measurement doubles.
fn place(tree: &ViewNode, state: &LayoutState) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state = state.clone();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

fn rect_of(frame: &PetrifiedFrame, id: &str) -> Rect {
    frame
        .placement(id)
        .unwrap_or_else(|| {
            panic!(
                "this frame placed no `{id}`; it placed {:?}",
                frame
                    .placements
                    .iter()
                    .map(|p| p.id.as_str())
                    .collect::<Vec<_>>()
            )
        })
        .rect
}

fn centre(rect: Rect) -> Point {
    Point::new(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0)
}

/// The host's publish step, in one line: copy the pointer snapshot into the
/// state the next negotiation reads. `Host::publish_pointer` is the shipped
/// form of exactly this.
fn publish(state: &mut LayoutState, pointer: &PointerState) {
    state.hovered = pointer.hovered().map(str::to_owned);
    state.pressed = pointer.pressed().map(str::to_owned);
    state.capture = pointer.capture().cloned();
}

fn moved(pos: Point) -> InputEvent {
    InputEvent::PointerMoved { pos }
}

fn pressed(pos: Point) -> InputEvent {
    InputEvent::PointerPressed {
        pos,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    }
}

fn released(pos: Point) -> InputEvent {
    InputEvent::PointerReleased {
        pos,
        button: PointerButton::Primary,
        modifiers: Modifiers::NONE,
    }
}

/// Ids of every placement in `frame` carrying `flag`.
fn flagged(
    frame: &PetrifiedFrame,
    flag: fn(&gorgon_petra::frame::PlacementSemantics) -> bool,
) -> Vec<&str> {
    frame
        .placements
        .iter()
        .filter(|p| flag(&p.semantics))
        .map(|p| p.id.as_str())
        .collect()
}

/// Two side-by-side controls, both hoverable and clickable.
fn two_buttons() -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app")
        .with_props(gorgon_petra::tree::Props {
            axis: Some(gorgon_petra::geom::Axis::Horizontal),
            ..gorgon_petra::tree::Props::default()
        })
        .child(control(
            "left",
            (100.0, 40.0),
            &[Interaction::Hover, Interaction::Click, Interaction::Focus],
        ))
        .child(control(
            "right",
            (100.0, 40.0),
            &[Interaction::Hover, Interaction::Click, Interaction::Focus],
        ))
}

// ---------------------------------------------------------------------------
// Hover
// ---------------------------------------------------------------------------

/// Hover is the click router's own answer, not a second opinion.
///
/// The assertion pairs the two on purpose. A toolkit that derived hover from
/// a rect test of its own would pass a "something is hovered" check and still
/// light a node the click reaches past — which is the defect FR-009 names and
/// the reason `contracts/interaction-state.md` §1 makes hover engine-derived.
#[test]
fn hover_names_the_node_a_click_at_the_same_point_would_reach() {
    let tree = two_buttons();
    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let mut pointer = PointerState::new();

    for id in ["/app/left", "/app/right"] {
        let pos = centre(rect_of(&frame, id));
        pointer.route(&frame, None, &no_surfaces(), &moved(pos));
        assert_eq!(pointer.hovered(), Some(id));
        assert_eq!(
            hit_test(&frame, pos, Interaction::Click).map(|p| p.id.as_str()),
            Some(id),
            "hover and the click router disagree at {pos:?}"
        );
    }

    publish(&mut state, &pointer);
    let lit = place(&tree, &state);
    assert_eq!(flagged(&lit, |s| s.hovered), vec!["/app/right"]);
}

/// At most one node is hovered per frame, even where two accept the point.
///
/// The fixture is two full-size overlay children, so *both* rects contain the
/// pointer and the paint order is the only tie-break there is. One flag on the
/// frame is the whole claim: a design that walked the placements and lit
/// everything containing the point would put two rows of a list under one
/// pointer.
#[test]
fn at_most_one_node_is_hovered_when_two_rects_hold_the_same_point() {
    let tree = ViewNode::new(NodeKind::Overlay, "app")
        .child(control(
            "under",
            (200.0, 80.0),
            &[Interaction::Hover, Interaction::Click],
        ))
        .child(control(
            "over",
            (200.0, 80.0),
            &[Interaction::Hover, Interaction::Click],
        ));
    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let pos = centre(rect_of(&frame, "/app/over"));
    assert!(
        rect_of(&frame, "/app/under").contains(pos),
        "the fixture must overlap or it is not testing the tie-break"
    );

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &moved(pos));
    publish(&mut state, &pointer);

    let lit = place(&tree, &state);
    assert_eq!(
        flagged(&lit, |s| s.hovered),
        vec!["/app/over"],
        "exactly one node is hovered, and it is the topmost one"
    );
}

/// Pointer-exit lands on the hovered node, never on the focused one.
///
/// `contracts/interaction-state.md` §8. `PointerLeft` carries no position, so
/// a router with no memory falls through to the focus branch and tells
/// whatever holds focus that the pointer left it — when the pointer may never
/// have been over it. The second assertion is the one that would have caught
/// that: focus is deliberately parked on the *other* button.
#[test]
fn pointer_exit_after_a_hover_goes_to_the_hovered_node() {
    let tree = two_buttons();
    let frame = place(&tree, &LayoutState::default());
    let mut pointer = PointerState::new();
    pointer.route(
        &frame,
        None,
        &no_surfaces(),
        &moved(centre(rect_of(&frame, "/app/right"))),
    );

    let routing = pointer.route(
        &frame,
        Some("/app/left"),
        &no_surfaces(),
        &InputEvent::PointerLeft,
    );
    assert_eq!(
        routing.outcome.route,
        Route::Pointer {
            node: "/app/right".into()
        }
    );
    assert_eq!(pointer.hovered(), None, "the pointer is outside the window");

    // The seam this closes, stated as the difference it makes: the same event
    // through the memoryless router is dropped rather than misdelivered.
    assert!(matches!(
        route(&frame, Some("/app/left"), &InputEvent::PointerLeft),
        Route::Unrouted { .. }
    ));
}

/// A node that stops being under the pointer stops being hovered, without
/// waiting for a move that may never come.
///
/// The pointer does not move at all here; the *tree* does. A host that only
/// re-derived hover on a pointer event would leave a control lit after the
/// content scrolled out from under it.
#[test]
fn hover_is_re_derived_against_each_newly_placed_frame() {
    let tree = two_buttons();
    let frame = place(&tree, &LayoutState::default());
    let mut pointer = PointerState::new();
    let pos = centre(rect_of(&frame, "/app/right"));
    pointer.route(&frame, None, &no_surfaces(), &moved(pos));
    assert_eq!(pointer.hovered(), Some("/app/right"));

    // The right-hand control is gone from the next frame.
    let shrunk = ViewNode::new(NodeKind::Stack, "app").child(control(
        "left",
        (100.0, 40.0),
        &[Interaction::Hover, Interaction::Click],
    ));
    let next = place(&shrunk, &LayoutState::default());
    assert!(pointer.reconcile(&next, &no_surfaces()).is_none());
    assert_eq!(pointer.hovered(), None);
}

// ---------------------------------------------------------------------------
// Disabled and read-only
// ---------------------------------------------------------------------------

/// A disabled node takes no interaction state and leaves the focus tree.
///
/// Three separate claims, because a toolkit can get any one of them right and
/// the other two wrong: the hit test refuses it, so hover and capture cannot
/// reach it; the placement carries none of the five flags; and the focus
/// order does not contain it.
#[test]
fn a_disabled_node_takes_no_interaction_state_and_leaves_the_focus_tree() {
    let mut off = control(
        "right",
        (100.0, 40.0),
        &[Interaction::Hover, Interaction::Click, Interaction::Focus],
    );
    off.semantics.disabled = true;
    let tree = ViewNode::new(NodeKind::Stack, "app")
        .with_props(gorgon_petra::tree::Props {
            axis: Some(gorgon_petra::geom::Axis::Horizontal),
            ..gorgon_petra::tree::Props::default()
        })
        .child(control(
            "left",
            (100.0, 40.0),
            &[Interaction::Hover, Interaction::Click, Interaction::Focus],
        ))
        .child(off);

    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let pos = centre(rect_of(&frame, "/app/right"));

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &moved(pos));
    assert_eq!(pointer.hovered(), None, "a disabled node is not hoverable");
    pointer.route(&frame, None, &no_surfaces(), &pressed(pos));
    assert_eq!(pointer.capture(), None, "a disabled node cannot be grabbed");

    // Even handed the flag directly — which is what a snapshot one frame old
    // does when the application disables a control between frames — the
    // projection refuses it (§4 rank 2: disabled *clears* the three).
    state.hovered = Some("/app/right".into());
    state.pressed = Some("/app/right".into());
    state.capture = Some(Capture {
        node: "/app/right".into(),
        button: PointerButton::Primary,
        origin: pos,
        last: pos,
    });
    let lit = place(&tree, &state);
    let off = lit.placement("/app/right").expect("placed");
    assert!(!off.semantics.hovered);
    assert!(!off.semantics.active);
    assert!(!off.semantics.captured);

    let order = FocusTree::from_placements(&lit.placements, &BTreeMap::new());
    assert_eq!(order.order(), &["/app/left".to_owned()]);
}

/// A read-only node stays in the focus tree and in the hit test. Read-only is
/// not a weaker disabled.
///
/// `contracts/interaction-state.md` §5. The whole rule is what read-only does
/// **not** do: it adds no branch to the focus filter and none to `hit_test`.
/// A read-only field is Tab-reachable, is a legal `Action::Focus` target, and
/// still hears the hover events a tooltip needs. What it does not get is a
/// pressed or hovered *look*, which is the token resolver's business and not
/// the router's.
#[test]
fn a_read_only_node_stays_focusable_and_hit_testable() {
    let mut field = control(
        "field",
        (100.0, 40.0),
        &[Interaction::Hover, Interaction::Focus],
    );
    field.semantics.read_only = true;
    let tree = ViewNode::new(NodeKind::Stack, "app").child(field);

    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let pos = centre(rect_of(&frame, "/app/field"));

    assert_eq!(
        hit_test(&frame, pos, Interaction::Hover).map(|p| p.id.as_str()),
        Some("/app/field"),
        "read-only must add no refusal to the hit test"
    );
    let order = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
    assert_eq!(
        order.order(),
        &["/app/field".to_owned()],
        "read-only must add no filter to focus order"
    );

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &moved(pos));
    assert_eq!(pointer.hovered(), Some("/app/field"));
    publish(&mut state, &pointer);
    let lit = place(&tree, &state);
    let node = lit.placement("/app/field").expect("placed");
    assert!(node.semantics.hovered, "a read-only node still hears hover");
    assert!(
        !node.semantics.disabled,
        "read-only must never imply disabled"
    );
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

/// A draggable node beside an ordinary button, so a press can land on either.
fn draggable_page() -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app")
        .with_props(gorgon_petra::tree::Props {
            axis: Some(gorgon_petra::geom::Axis::Horizontal),
            ..gorgon_petra::tree::Props::default()
        })
        .child(control(
            "handle",
            (60.0, 60.0),
            &[
                Interaction::Drag,
                Interaction::Hover,
                Interaction::Click,
                Interaction::Focus,
            ],
        ))
        .child(control(
            "pane",
            (200.0, 60.0),
            &[Interaction::Hover, Interaction::Click, Interaction::Focus],
        ))
}

/// A press on a `Drag` node grabs the pointer; every later event routes there
/// wherever the pointer goes.
#[test]
fn a_press_on_a_drag_node_takes_capture_and_keeps_every_later_event() {
    let tree = draggable_page();
    let frame = place(&tree, &LayoutState::default());
    let grab = centre(rect_of(&frame, "/app/handle"));
    let away = centre(rect_of(&frame, "/app/pane"));

    let mut pointer = PointerState::new();
    let routing = pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
    assert_eq!(
        routing.outcome.route,
        Route::Pointer {
            node: "/app/handle".into()
        }
    );
    assert_eq!(
        pointer.capture().map(|c| c.node.as_str()),
        Some("/app/handle")
    );

    // Over the *other* node, which accepts clicks: without capture this move
    // would land there, and the drag would stop following the pointer.
    let routing = pointer.route(&frame, None, &no_surfaces(), &moved(away));
    assert_eq!(
        routing.outcome.route,
        Route::Pointer {
            node: "/app/handle".into()
        }
    );
    assert_eq!(
        required_interaction_during(&moved(away), pointer.capture()),
        Some(Interaction::Drag),
        "a move under capture is the body of a drag, not a hover"
    );
    assert_eq!(
        required_interaction(&moved(away)),
        Some(Interaction::Hover),
        "and the same move with nothing captured is still a hover"
    );
    assert_eq!(pointer.capture().map(|c| c.last), Some(away));
    assert_eq!(pointer.capture().map(|c| c.origin), Some(grab));
}

/// Exactly one node holds capture, and a second press starts no second
/// gesture (FR-015).
///
/// The second press lands on a node that would happily take a capture of its
/// own on a clean pointer — the same handle is not reused — so a
/// `Vec<Capture>` implementation would pass a "the first one is still there"
/// check and fail this one.
#[test]
fn exactly_one_node_holds_capture_and_a_second_press_starts_no_second_gesture() {
    let tree = ViewNode::new(NodeKind::Stack, "app")
        .with_props(gorgon_petra::tree::Props {
            axis: Some(gorgon_petra::geom::Axis::Horizontal),
            ..gorgon_petra::tree::Props::default()
        })
        .child(control(
            "first",
            (60.0, 60.0),
            &[Interaction::Drag, Interaction::Hover, Interaction::Click],
        ))
        .child(control(
            "second",
            (60.0, 60.0),
            &[Interaction::Drag, Interaction::Hover, Interaction::Click],
        ));
    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let first = centre(rect_of(&frame, "/app/first"));
    let second = centre(rect_of(&frame, "/app/second"));

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &pressed(first));
    assert_eq!(
        pointer.capture().map(|c| c.node.as_str()),
        Some("/app/first")
    );

    let routing = pointer.route(&frame, None, &no_surfaces(), &pressed(second));
    assert_eq!(
        pointer.capture().map(|c| c.node.as_str()),
        Some("/app/first"),
        "the second press must not move the capture"
    );
    assert_eq!(
        routing.outcome.route,
        Route::Pointer {
            node: "/app/first".into()
        },
        "a press while captured is an ordinary press *to the holder*"
    );
    assert!(routing.ended.is_none(), "no gesture ended");

    publish(&mut state, &pointer);
    let lit = place(&tree, &state);
    assert_eq!(flagged(&lit, |s| s.captured), vec!["/app/first"]);
}

/// A button pressed and then dragged off its own rect stays captured and
/// stops being active.
#[test]
fn a_capture_dragged_off_its_rect_stops_being_active_and_keeps_the_capture() {
    let tree = draggable_page();
    let mut state = LayoutState::default();
    let frame = place(&tree, &state);
    let grab = centre(rect_of(&frame, "/app/handle"));
    let away = centre(rect_of(&frame, "/app/pane"));

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
    assert_eq!(pointer.pressed(), Some("/app/handle"));
    publish(&mut state, &pointer);
    let held = place(&tree, &state);
    let node = held.placement("/app/handle").expect("placed");
    assert!(node.semantics.active && node.semantics.captured);

    pointer.route(&frame, None, &no_surfaces(), &moved(away));
    assert_eq!(pointer.pressed(), None, "the pointer is off the rect");
    assert_eq!(
        pointer.capture().map(|c| c.node.as_str()),
        Some("/app/handle")
    );
    publish(&mut state, &pointer);
    let dragged = place(&tree, &state);
    let node = dragged.placement("/app/handle").expect("placed");
    assert!(
        !node.semantics.active,
        "a dragged-off button is not pressed"
    );
    assert!(node.semantics.captured, "but it still owns the pointer");
    assert_ne!(
        held.digest, dragged.digest,
        "two different pictures, two different digests"
    );
}

/// A release outside a pressed button does not activate it.
///
/// The engine cannot refuse an activation an application performs, so what it
/// owes is an unambiguous report: the gesture ends `Cancelled(Left)` rather
/// than `Completed`, and `Completed` is the only outcome a component may
/// treat as activation. The inside case is asserted in the same test, because
/// a rule that reported `Cancelled` for *every* release would also pass an
/// outside-only check.
#[test]
fn a_release_outside_a_pressed_button_does_not_activate_it() {
    let tree = draggable_page();
    let frame = place(&tree, &LayoutState::default());
    let grab = centre(rect_of(&frame, "/app/handle"));
    let away = centre(rect_of(&frame, "/app/pane"));

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
    pointer.route(&frame, None, &no_surfaces(), &moved(away));
    assert_eq!(pointer.pressed(), None);
    let routing = pointer.route(&frame, None, &no_surfaces(), &released(away));
    assert_eq!(
        routing.ended.as_ref().map(|end| end.outcome),
        Some(GestureOutcome::Cancelled(CancelReason::Left)),
        "a release off the rect did not complete the gesture"
    );
    assert_eq!(
        routing.ended.as_ref().map(|end| end.node.as_str()),
        Some("/app/handle")
    );
    assert_eq!(pointer.capture(), None, "and the capture is released");

    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
    let routing = pointer.route(&frame, None, &no_surfaces(), &released(grab));
    assert_eq!(
        routing.ended.map(|end| end.outcome),
        Some(GestureOutcome::Completed),
        "a release on the rect is the one outcome that activates"
    );
}

/// The three cancellations no release describes, each ending the gesture with
/// its own reason.
///
/// Four reasons and not one bit, because a component undoes different amounts
/// of work for each. A test that only checked "the capture is gone" would pass
/// on an implementation that reported every cancellation as a blur.
#[test]
fn a_capture_is_cancelled_by_a_blur_an_escape_and_a_vanished_node() {
    let tree = draggable_page();
    let frame = place(&tree, &LayoutState::default());
    let grab = centre(rect_of(&frame, "/app/handle"));
    let escape = InputEvent::Key {
        key: KeyCode::Escape,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };

    let cases: [(&str, InputEvent, CancelReason); 3] = [
        ("blur", InputEvent::WindowBlurred, CancelReason::Blurred),
        ("escape", escape, CancelReason::Escape),
        ("left", InputEvent::PointerLeft, CancelReason::Left),
    ];
    for (what, event, reason) in cases {
        let mut pointer = PointerState::new();
        pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
        let routing = pointer.route(&frame, None, &no_surfaces(), &event);
        assert_eq!(
            routing.ended.map(|end| end.outcome),
            Some(GestureOutcome::Cancelled(reason)),
            "{what}"
        );
        assert_eq!(pointer.capture(), None, "{what}");
    }

    // The fourth: the captured node is absent from a newly placed frame.
    // Unlike focus there is no successor rule, so the gesture simply ends.
    let mut pointer = PointerState::new();
    pointer.route(&frame, None, &no_surfaces(), &pressed(grab));
    let gone = place(
        &ViewNode::new(NodeKind::Stack, "app").child(control(
            "pane",
            (200.0, 60.0),
            &[Interaction::Hover, Interaction::Click],
        )),
        &LayoutState::default(),
    );
    let ended = pointer
        .reconcile(&gone, &no_surfaces())
        .expect("the captured node vanished");
    assert_eq!(ended.node, "/app/handle");
    assert_eq!(
        ended.outcome,
        GestureOutcome::Cancelled(CancelReason::Vanished)
    );
    assert_eq!(pointer.capture(), None);
}

/// A press that hits no `Drag` node grabs nothing and routes as a click.
///
/// The fall-through half of `contracts/interaction-state.md` §7's grab rule.
/// Without it every press on every button would open a gesture, and
/// `Interaction::Drag` would stop meaning anything.
#[test]
fn a_press_on_a_click_only_node_grabs_no_capture() {
    let tree = draggable_page();
    let frame = place(&tree, &LayoutState::default());
    let pos = centre(rect_of(&frame, "/app/pane"));

    let mut pointer = PointerState::new();
    let routing = pointer.route(&frame, None, &no_surfaces(), &pressed(pos));
    assert_eq!(pointer.capture(), None);
    assert_eq!(
        routing.outcome.route,
        Route::Pointer {
            node: "/app/pane".into()
        }
    );
}

// ---------------------------------------------------------------------------
// The audit rule (`contracts/interaction-state.md` §5)
// ---------------------------------------------------------------------------

/// A node declaring both `read_only` and `disabled` is a defect the audit
/// names.
///
/// Before this rule the tree said two incompatible things — reachable and
/// readable, and not reachable at all — and every gate passed. An assistive
/// technology handed both has to pick one, and which one it picks is not the
/// author's decision to leave open.
#[test]
fn the_audit_reports_a_node_declaring_both_read_only_and_disabled() {
    use gorgon_petra::semantic::{AuditRule, audit, project};

    let mut confused = control("field", (100.0, 40.0), &[Interaction::Focus]);
    confused.semantics.read_only = true;
    confused.semantics.disabled = true;
    let tree = ViewNode::new(NodeKind::Stack, "app").child(confused);
    let frame = place(&tree, &LayoutState::default());
    let projected = project(&frame).expect("the frame has placements");

    let violations = audit(&projected, &frame);
    let mine: Vec<&str> = violations
        .iter()
        .filter(|v| v.rule == AuditRule::ReadOnlyIsNotDisabled)
        .map(|v| v.node_id.as_str())
        .collect();
    assert_eq!(mine, vec!["/app/field"], "{violations:?}");
}

/// The clean case, which is what stops the rule above from being a rule that
/// fires on every read-only node.
///
/// A read-only, focusable, enabled node is a completely ordinary thing — a
/// field showing a value this author may read and not edit — and the audit
/// must say nothing about it.
#[test]
fn the_audit_passes_a_read_only_node_that_is_merely_read_only() {
    use gorgon_petra::semantic::{AuditRule, audit, project};

    let mut field = control(
        "field",
        (100.0, 40.0),
        &[Interaction::Focus, Interaction::Hover],
    );
    field.semantics.read_only = true;
    let tree = ViewNode::new(NodeKind::Stack, "app").child(field);
    let frame = place(&tree, &LayoutState::default());
    let projected = project(&frame).expect("the frame has placements");

    let violations = audit(&projected, &frame);
    assert!(
        violations
            .iter()
            .all(|v| v.rule != AuditRule::ReadOnlyIsNotDisabled),
        "a plain read-only node was reported: {violations:?}"
    );
    // And it is in focus order, which is the engine half of §5.
    let order = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
    assert_eq!(order.order(), &["/app/field".to_owned()]);
}
