//! Frame-capture acceptance for US4, and the two invariants motion is allowed
//! to break if nobody is watching.
//!
//! Every test here drives real frames through `petrify` and then through the
//! real scheduler, capturing the placement stream the way a driver would.
//! Nothing simulates the engine; the frames are the evidence.
//!
//! Spec 003 US4's five acceptance scenarios map onto the tests below:
//!
//! | scenario | test |
//! |---|---|
//! | 1 interpolate from old to new, end exactly at the new placement | `scenario_1_*` |
//! | 2 retarget from current position and velocity, no jump | `scenario_2_*` |
//! | 3 reduced motion: same end state, no intermediate movement | `scenario_3_*` |
//! | 4 ambient settles anyway and is marked ambient | `scenario_4_*` |
//! | 5 sixty idle seconds paint zero frames | `scenario_5_*` |
//!
//! Plus the two that are not scenarios but are the reason the scenarios can be
//! trusted: `settled_digest_*` (no near-target residue) and
//! `ambient_*undeclared*` (T084 — a hosted surface that repaints without
//! declaring itself fails the idle assertion **by name**).

use gorgon_petra::anim::curve::{CubicBezier, Keyframe, KeyframeTrack};
use gorgon_petra::anim::engine::Declarations;
use gorgon_petra::anim::registry::{ExitRule, Timing, Track, TransitionDef, TransitionRegistry};
use gorgon_petra::anim::spring::Spring;
use gorgon_petra::anim::value::{AnimVector, PropertyKind};
use gorgon_petra::anim::{ForeignRepaint, Scheduler};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::testing::{Harness, extended_vocabulary, validated_with};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, Registry, ViewNode};

const W: f32 = 400.0;
const H: f32 = 160.0;
/// One frame at 60 Hz.
const TICK: f64 = 1.0 / 60.0;
/// A hard cap on every settle loop in this file. `contracts/animation.md`
/// promises termination; a test that trusted the promise with a `loop` would
/// hang the suite instead of reporting the broken promise, so every loop is
/// bounded and says what it was waiting for when the bound is hit.
const MAX_TICKS: u32 = 1200;

/// The definitions every tree here names.
fn definitions() -> TransitionRegistry {
    let mut registry = TransitionRegistry::new();
    registry.register(
        "slide",
        TransitionDef::builder()
            .drive(
                PropertyKind::Position,
                Timing::Spring(Spring::response(0.3, 1.0).unwrap()),
            )
            .build()
            .unwrap(),
    );
    registry.register(
        "slide-bouncy",
        TransitionDef::builder()
            .drive(
                PropertyKind::Position,
                Timing::Spring(Spring::duration_bounce(0.5, 0.55).unwrap()),
            )
            .build()
            .unwrap(),
    );
    registry.register(
        "fade-out",
        TransitionDef::builder()
            .drive(
                PropertyKind::Opacity,
                Timing::Spring(Spring::response(0.25, 1.0).unwrap()),
            )
            .exit(ExitRule::RunExitTrack(Track::fade()))
            .build()
            .unwrap(),
    );
    registry.register("pulse", pulse());
    registry
}

/// A declared-endless keyframe loop: the only shape an ambient definition can
/// take, because everything else settles.
fn pulse() -> TransitionDef {
    let track = KeyframeTrack::new(vec![
        Keyframe {
            time: 0.0,
            value: AnimVector::scalar(1.0),
            easing_in: CubicBezier::LINEAR,
        },
        Keyframe {
            time: 0.6,
            value: AnimVector::scalar(0.4),
            easing_in: CubicBezier::EASE_IN_OUT,
        },
        Keyframe {
            time: 1.2,
            value: AnimVector::scalar(1.0),
            easing_in: CubicBezier::EASE_IN_OUT,
        },
    ])
    .unwrap();
    TransitionDef::builder()
        .drive(PropertyKind::Opacity, Timing::Keyframes(track))
        .ambient(true)
        .build()
        .unwrap()
}

fn frame_of(tree: &ViewNode, seq: u64) -> PetrifiedFrame {
    let mut registry = Registry::with_vocabulary(extended_vocabulary(tree));
    declare_names(tree, &mut registry);
    let mut harness = Harness::new();
    petrify(
        seq,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(Size::new(W, H), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn declare_names(node: &ViewNode, registry: &mut Registry) {
    if let Some(reference) = node.transition.as_ref() {
        registry.register_transition(reference.name());
    }
    for child in &node.children {
        declare_names(child, registry);
    }
}

/// A horizontal stack whose `/app/mover` sits `offset` units from the left and
/// animates under `transition` when that changes.
fn moving(offset: f32, transition: &str) -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(
            ViewNode::new(NodeKind::Spacer, "lead").with_constraints(Constraints {
                horizontal: AxisConstraint {
                    min: Some(offset),
                    max: Some(offset),
                    priority: 10,
                },
                ..Constraints::default()
            }),
        )
        .child(
            ViewNode::new(NodeKind::Text, "mover")
                .with_props(Props {
                    text: Some("mover".into()),
                    ..Props::default()
                })
                .with_transition(transition),
        )
}

/// A stack with one hosted image node, declared ambient or not.
fn hosted(ambient: bool) -> ViewNode {
    ViewNode::new(NodeKind::Stack, "app").child(
        ViewNode::new(NodeKind::Image, "spark")
            .with_props(Props {
                image: Some("spark.png".into()),
                ..Props::default()
            })
            .with_ambient(ambient),
    )
}

fn mover_x(frame: &PetrifiedFrame) -> f32 {
    frame
        .placement("/app/mover")
        .expect("the fixture always places /app/mover")
        .rect
        .x
}

/// Drive `tree` for one tick and hand back the frame as it would be painted.
fn tick(scheduler: &mut Scheduler, tree: &ViewNode, seq: u64, now: f64) -> PetrifiedFrame {
    let mut frame = frame_of(tree, seq);
    let decision = scheduler.advance(&mut frame, &Declarations::collect(tree), now, &[]);
    // Nothing foreign is offered here, so nothing may be recorded. Asserting
    // it rather than dropping the decision: `tick` is the helper most of the
    // scenarios below run on, and a ledger that recorded on an empty
    // `foreign` slice would make every one of their idle audits meaningless.
    assert_eq!(decision.undeclared, 0, "no foreign repaint was offered");
    frame
}

// ---------------------------------------------------------------------------
// Scenario 1 — interpolation, and an exact landing.
// ---------------------------------------------------------------------------

/// *"intermediate frames interpolate from old to new placement by the declared
/// curve and the sequence ends at exactly the new placement."*
///
/// Both halves are asserted: the stream is monotone from 0 toward 120 with at
/// least a dozen distinct intermediate positions (so this cannot pass on a
/// one-frame jump), and the last frame is at exactly 120.
#[test]
fn scenario_1_interpolates_from_old_to_new_and_lands_exactly() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");

    let first = tick(&mut scheduler, &start, 1, 0.0);
    assert_eq!(mover_x(&first), 0.0);
    let target = mover_x(&frame_of(&end, 99));
    assert_eq!(target, 120.0, "the fixture's target is where it says it is");

    let mut stream = Vec::new();
    let mut settled_at = None;
    for step in 1..=MAX_TICKS {
        let frame = tick(
            &mut scheduler,
            &end,
            u64::from(step) + 1,
            f64::from(step) * TICK,
        );
        stream.push(mover_x(&frame));
        if frame.transitions.is_settled() {
            settled_at = Some(step);
            break;
        }
    }
    let settled_at = settled_at.unwrap_or_else(|| {
        panic!("a finite spring never settled within {MAX_TICKS} ticks; stream = {stream:?}")
    });
    assert!(
        settled_at > 12,
        "settled in {settled_at} tick(s) — too few frames to be an interpolation"
    );
    assert_eq!(
        *stream.last().unwrap(),
        target,
        "the sequence must end at exactly the new placement"
    );
    for pair in stream.windows(2) {
        assert!(
            pair[1] >= pair[0],
            "a critically damped spring never goes backwards: {stream:?}"
        );
        assert!(pair[1] <= target, "and never overshoots: {stream:?}");
    }
    let distinct = stream.iter().filter(|x| **x > 0.0 && **x < target).count();
    assert!(distinct >= 12, "only {distinct} intermediate frame(s)");
}

/// The other half of "by the declared curve": the captured stream must match
/// the closed form the definition names, not merely go the right way. A linear
/// ramp would pass every assertion above and fail this one.
#[test]
fn scenario_1_the_stream_matches_the_declared_closed_form() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");
    tick(&mut scheduler, &start, 1, 0.0);

    let spring = Spring::response(0.3, 1.0).unwrap();
    for step in 1..=20_u32 {
        let now = f64::from(step) * TICK;
        let frame = tick(&mut scheduler, &end, u64::from(step) + 1, now);
        // The trajectory's clock starts at the *previous* frame's timestamp —
        // that is when the node was still at its old value — so at `now` its
        // local time is `now`, the first frame having been at 0.0.
        let (expected, _) = spring.evaluate_scalar(0.0, 0.0, 120.0, now);
        let observed = f64::from(mover_x(&frame));
        assert!(
            (observed - expected).abs() < 0.01,
            "tick {step}: observed {observed}, closed form {expected}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 2 — retargeting. This is SC-007.
// ---------------------------------------------------------------------------

/// *"the value retargets from its current position and velocity — no frame
/// jumps to either endpoint."*
///
/// The measurement is a per-frame velocity, differenced off the captured
/// stream, which is what an outside observer of the frames can see. Across the
/// retarget the change in that velocity must stay inside what the ODE itself
/// allows over one tick — it must not step to zero (a restart) and it must not
/// step to the far endpoint (a snap).
///
/// **This is the test T055 sabotages.** Zeroing the velocity carry in
/// `TrackState::retarget` makes the frame-to-frame velocity fall off a cliff
/// at the seam and this assertion is what catches it.
#[test]
fn scenario_2_retarget_carries_velocity_across_the_interruption() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide-bouncy");
    let far = moving(300.0, "slide-bouncy");
    let near = moving(80.0, "slide-bouncy");

    tick(&mut scheduler, &start, 1, 0.0);

    // Nine ticks toward the far target: enough to be moving fast.
    let mut stream = vec![0.0_f64];
    for step in 1..=9_u32 {
        let frame = tick(
            &mut scheduler,
            &far,
            u64::from(step) + 1,
            f64::from(step) * TICK,
        );
        stream.push(f64::from(mover_x(&frame)));
    }
    let before = stream[stream.len() - 1] - stream[stream.len() - 2];
    assert!(
        before > 2.0,
        "the interruption must land mid-flight; per-tick step was {before}"
    );

    // Retarget. The next tick is the seam.
    let frame = tick(&mut scheduler, &near, 11, 10.0 * TICK);
    let after_position = f64::from(mover_x(&frame));
    let across = after_position - stream[stream.len() - 1];

    assert!(
        after_position > stream[stream.len() - 1],
        "position is continuous and still moving the way it was: {stream:?} then {after_position}"
    );
    assert!(
        (across - before).abs() < 0.5 * before,
        "velocity jumped across the retarget: {before} per tick before, {across} after"
    );
    assert!(
        (after_position - 80.0).abs() > 10.0,
        "the frame jumped to the new endpoint instead of retargeting: {after_position}"
    );

    // And it still terminates, at exactly the new target.
    let mut last = after_position;
    let mut settled = false;
    for step in 11..MAX_TICKS {
        let frame = tick(
            &mut scheduler,
            &near,
            u64::from(step) + 1,
            f64::from(step) * TICK,
        );
        last = f64::from(mover_x(&frame));
        if frame.transitions.is_settled() {
            settled = true;
            break;
        }
    }
    assert!(settled, "the retargeted transition never settled");
    assert!((last - 80.0).abs() < f64::EPSILON, "landed at {last}");
}

// ---------------------------------------------------------------------------
// Scenario 3 — reduced motion.
// ---------------------------------------------------------------------------

/// *"the end state is reached with no intermediate frames of movement"*, and
/// the end state is the same one full motion reaches.
#[test]
fn scenario_3_reduced_motion_reaches_the_same_end_state_with_no_movement() {
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");

    let mut reduced = Scheduler::new(definitions());
    reduced.set_reduced_motion(true);
    tick(&mut reduced, &start, 1, 0.0);
    let frame = tick(&mut reduced, &end, 2, TICK);
    assert_eq!(mover_x(&frame), 120.0, "reduced motion lands at once");
    assert!(frame.transitions.is_settled());
    assert_eq!(frame.transitions.running, 0);

    // Identical end state, byte for byte, to a build straight to the target.
    let reference = frame_of(&end, 2);
    assert_eq!(frame.placements, reference.placements);
    assert_eq!(frame.digest, reference.digest);

    // And not a single further frame is asked for.
    assert_eq!(reduced.frames_requested(), 0);
}

/// *"Toggling reduced motion mid-flight completes running transitions
/// instantly to their targets."*
#[test]
fn scenario_3_toggling_mid_flight_completes_running_transitions() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");
    tick(&mut scheduler, &start, 1, 0.0);
    for step in 1..=5_u32 {
        tick(
            &mut scheduler,
            &end,
            u64::from(step) + 1,
            f64::from(step) * TICK,
        );
    }
    let mid = tick(&mut scheduler, &end, 7, 6.0 * TICK);
    assert!(!mid.transitions.is_settled(), "must still be in flight");
    assert!(mover_x(&mid) < 120.0);

    scheduler.set_reduced_motion(true);
    let after = tick(&mut scheduler, &end, 8, 7.0 * TICK);
    assert_eq!(mover_x(&after), 120.0, "completed instantly to the target");
    assert!(after.transitions.is_settled());
    assert_eq!(after.digest, frame_of(&end, 8).digest);
}

/// Opacity is the accepted substitute and keeps running under reduced motion.
/// A blanket kill switch would fail this.
#[test]
fn scenario_3_reduced_motion_keeps_the_opacity_substitute() {
    let policy = gorgon_petra::anim::MotionPolicy::reduced();
    assert!(!policy.animates(PropertyKind::Position));
    assert!(!policy.animates(PropertyKind::Size));
    assert!(policy.animates(PropertyKind::Opacity));
    assert!(policy.animates(PropertyKind::Color));
}

// ---------------------------------------------------------------------------
// Scenario 4 — ambient.
// ---------------------------------------------------------------------------

/// *"settle is reached while the spinner still runs, and the semantic tree
/// marks the node as ambient."*
#[test]
fn scenario_4_ambient_settles_and_is_marked_in_the_semantic_tree() {
    let mut scheduler = Scheduler::new(definitions());
    let tree = hosted(true);
    let frame = tick(&mut scheduler, &tree, 1, 0.0);
    let frame = {
        let _ = frame;
        tick(&mut scheduler, &tree, 2, TICK)
    };

    assert_eq!(frame.transitions.ambient, 1, "the ambient count is real");
    assert_eq!(frame.transitions.running, 0);
    assert!(
        frame.transitions.is_settled(),
        "ambient must never block settle"
    );
    // ...and yet frames keep coming, which is the other half of the rule.
    assert!(gorgon_petra::anim::wants_frame(frame.transitions));
    assert!(scheduler.frames_requested() >= 1);

    let semantic = gorgon_petra::semantic::project(&frame).expect("a placed frame projects");
    let node = semantic
        .find("/app/spark")
        .expect("the hosted node is placed");
    assert!(
        node.state.ambient,
        "the semantic tree marks the node as ambient"
    );
}

/// An ambient *transition definition* counts too, not only the tree flag, and
/// a node carrying both is one endless animation rather than two.
#[test]
fn scenario_4_an_ambient_definition_counts_once() {
    let mut scheduler = Scheduler::new(definitions());
    let tree = ViewNode::new(NodeKind::Stack, "app").child(
        ViewNode::new(NodeKind::Text, "spinner")
            .with_props(Props {
                text: Some("...".into()),
                ..Props::default()
            })
            .with_transition("pulse")
            .with_ambient(true),
    );
    tick(&mut scheduler, &tree, 1, 0.0);
    let frame = tick(&mut scheduler, &tree, 2, TICK);
    assert_eq!(frame.transitions.ambient, 1);
    assert!(frame.transitions.is_settled());
}

// ---------------------------------------------------------------------------
// Scenario 5 — zero idle. This is SC-002.
// ---------------------------------------------------------------------------

/// *"Given no input, no transitions, and no ambient animations, when 60
/// seconds pass, then zero frames are painted (U-26)."*
///
/// Sixty seconds of simulated host clock at 60 Hz. The scheduler asks for
/// nothing, and the idle audit is clean.
///
/// Simulated, and this is the honest limit of this test: it proves the
/// scheduling *rule* over 3600 passes, not that a live `eframe` window paints
/// nothing. The window-level claim needs `gorgon/petra-egui`'s host under a
/// real event loop and is not made here.
#[test]
fn scenario_5_sixty_idle_seconds_ask_for_zero_frames() {
    let mut scheduler = Scheduler::new(definitions());
    let tree = moving(40.0, "slide");
    let declarations = Declarations::collect(&tree);
    for step in 0..3600_u32 {
        let mut frame = frame_of(&tree, u64::from(step) + 1);
        let decision = scheduler.advance(&mut frame, &declarations, f64::from(step) * TICK, &[]);
        assert!(
            !decision.repaint,
            "asked for a frame at idle step {step}: {:?}",
            decision.activity
        );
        assert_eq!(decision.activity, TransitionActivity::default());
    }
    assert_eq!(scheduler.frames_requested(), 0, "SC-002");
    let report = scheduler.idle_audit(60.0).unwrap();
    assert_eq!(report.frames, 3600);
    assert_eq!(report.declared_ambient, 0);
    assert!(report.observed_seconds > 59.0, "{report:?}");
}

// ---------------------------------------------------------------------------
// The settle rule — no near-target residue in the digest.
// ---------------------------------------------------------------------------

/// A frame driven to settle by the engine is byte-identical to a frame
/// petrified straight to the same target with no engine involved.
///
/// This is the strong form of *"it then snaps exactly to target (the digest
/// must not carry near-target residue)"*. Comparing placements as well as the
/// digest, because a digest match alone could hide a field the digest does not
/// hash.
#[test]
fn settled_digest_equals_a_build_straight_to_the_target() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");
    tick(&mut scheduler, &start, 1, 0.0);

    let mut settled = None;
    for step in 1..MAX_TICKS {
        let frame = tick(&mut scheduler, &end, 7, f64::from(step) * TICK);
        if frame.transitions.is_settled() {
            settled = Some(frame);
            break;
        }
    }
    let settled = settled.expect("the transition never settled");
    let reference = frame_of(&end, 7);
    assert_eq!(
        settled.placements, reference.placements,
        "residue in placements"
    );
    assert_eq!(settled.digest, reference.digest, "residue in the digest");
    assert_eq!(settled.subtree_hashes, reference.subtree_hashes);
    assert_eq!(settled.subtree_len, reference.subtree_len);
}

/// Mid-transition, the digest is the digest of what is on screen — not of the
/// target, and not of the start. A driver's screenshot verification depends on
/// exactly this (`contracts/driver-protocol.md`, FR-040).
#[test]
fn a_mid_transition_frame_has_its_own_identity() {
    let mut scheduler = Scheduler::new(definitions());
    let start = moving(0.0, "slide");
    let end = moving(120.0, "slide");
    tick(&mut scheduler, &start, 1, 0.0);

    let at_start = frame_of(&start, 5).digest;
    let at_end = frame_of(&end, 5).digest;
    let mut seen = Vec::new();
    for step in 1..=6_u32 {
        let frame = tick(&mut scheduler, &end, 5, f64::from(step) * TICK);
        assert_eq!(
            frame.digest,
            gorgon_petra::frame::digest::digest(&frame.viewport, &frame.placements),
            "a mid-transition digest must be recomputable from its own placements"
        );
        seen.push(frame.digest);
    }
    assert!(!seen.contains(&at_end), "a frame jumped to the target");
    assert!(
        seen.iter().skip(1).all(|d| *d != at_start),
        "the picture never moved off its starting identity"
    );
    let mut distinct = 0;
    for (i, digest) in seen.iter().enumerate() {
        if !seen[..i].contains(digest) {
            distinct += 1;
        }
    }
    assert!(distinct >= 5, "only {distinct} distinct frames");
}

// ---------------------------------------------------------------------------
// Exits.
// ---------------------------------------------------------------------------

/// A node removed mid-flight under `RunExitTrack` keeps being placed, fades,
/// and then leaves — and the frame stays a well-formed frame the whole time.
#[test]
fn an_exiting_node_runs_its_track_and_then_leaves() {
    let mut scheduler = Scheduler::new(definitions());
    let with_toast = ViewNode::new(NodeKind::Stack, "app")
        .child(ViewNode::new(NodeKind::Text, "keep").with_props(Props {
            text: Some("keep".into()),
            ..Props::default()
        }))
        .child(
            ViewNode::new(NodeKind::Text, "toast")
                .with_props(Props {
                    text: Some("toast".into()),
                    ..Props::default()
                })
                .with_transition("fade-out"),
        );
    let without = ViewNode::new(NodeKind::Stack, "app").child(
        ViewNode::new(NodeKind::Text, "keep").with_props(Props {
            text: Some("keep".into()),
            ..Props::default()
        }),
    );

    tick(&mut scheduler, &with_toast, 1, 0.0);
    let mut opacities = Vec::new();
    let mut gone_at = None;
    for step in 1..MAX_TICKS {
        let frame = tick(&mut scheduler, &without, 2, f64::from(step) * TICK);
        // Whatever else happens, the frame's identity must stay recomputable
        // from its own placements: the splice must not corrupt the tree.
        assert_eq!(
            frame.digest,
            gorgon_petra::frame::digest::digest(&frame.viewport, &frame.placements),
            "the exit splice broke the frame's parent chain at step {step}"
        );
        match frame.placement("/app/toast") {
            Some(p) => opacities.push(p.opacity),
            None => {
                gone_at = Some(step);
                assert!(frame.transitions.is_settled());
                break;
            }
        }
    }
    let gone_at = gone_at.expect("the exiting node never left");
    assert!(
        gone_at > 3,
        "it left after {gone_at} tick(s), not an exit run"
    );
    assert!(opacities.len() >= 3, "{opacities:?}");
    for pair in opacities.windows(2) {
        assert!(
            pair[1] <= pair[0],
            "the fade must be monotone: {opacities:?}"
        );
    }
    assert!(
        *opacities.last().unwrap() < 0.2,
        "it should be nearly transparent by the time it goes: {opacities:?}"
    );
    // And after it leaves, the frame equals a plain build of the new tree.
    let after = tick(&mut scheduler, &without, 3, f64::from(gone_at + 1) * TICK);
    assert_eq!(after.placements, frame_of(&without, 3).placements);
}

/// The other exit rule: the node is simply gone on the frame it is removed.
#[test]
fn a_complete_instantly_exit_leaves_at_once() {
    let mut scheduler = Scheduler::new(definitions());
    let with_it = moving(0.0, "slide");
    let without = ViewNode::new(NodeKind::Stack, "app")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(ViewNode::new(NodeKind::Spacer, "lead"));
    tick(&mut scheduler, &with_it, 1, 0.0);
    let frame = tick(&mut scheduler, &without, 2, TICK);
    assert!(frame.placement("/app/mover").is_none());
    assert!(frame.transitions.is_settled());
    assert_eq!(frame.placements, frame_of(&without, 2).placements);
}

// ---------------------------------------------------------------------------
// T084 — an undeclared self-animating hosted surface.
// ---------------------------------------------------------------------------

/// A hosted surface that drives repaints without declaring `ambient` is
/// **refused**: nothing it does reaches `TransitionActivity`, so the driver's
/// settle wait stays bounded.
///
/// And it is **reported**: the sixty-second audit fails naming the surface and
/// the source that asked. Asserting the *text* on purpose — a test that only
/// checked `is_err()` would pass against a message reading "something went
/// wrong", which is the exact failure this task exists to prevent.
#[test]
fn ambient_an_undeclared_repainting_surface_fails_the_idle_assertion_by_name() {
    let mut scheduler = Scheduler::new(definitions());
    let tree = hosted(false);
    let declarations = Declarations::collect(&tree);
    let asked = [ForeignRepaint::new("examples/gallery.rs:214")];

    for step in 0..3600_u32 {
        let mut frame = frame_of(&tree, u64::from(step) + 1);
        let decision = scheduler.advance(&mut frame, &declarations, f64::from(step) * TICK, &asked);
        // Refused: the surface never buys itself an ambient slot by asking.
        assert_eq!(decision.activity.ambient, 0, "step {step}");
        assert_eq!(decision.activity.running, 0, "step {step}");
        assert!(decision.activity.is_settled(), "settle stays reachable");
        assert!(!decision.repaint, "and Petra never asks on its behalf");
    }

    let violation = scheduler
        .idle_audit(60.0)
        .expect_err("an undeclared repainting surface must fail the idle audit");
    let message = violation.to_string();
    assert!(
        message.contains("/app/spark"),
        "names the surface: {message}"
    );
    assert!(
        message.contains("examples/gallery.rs:214"),
        "names the source: {message}"
    );
    assert!(message.contains("3600"), "names the count: {message}");
    assert!(message.contains("ambient"), "names the fix: {message}");
    assert!(message.contains("60s"), "names the window: {message}");
    assert_eq!(violation.offenders.len(), 1);
    assert_eq!(
        violation.offenders[0].surface.as_deref(),
        Some("/app/spark")
    );
}

/// The control: declare it, and the same traffic audits clean. Without this
/// the test above would pass against a ledger that refused everything.
#[test]
fn ambient_a_declared_repainting_surface_audits_clean() {
    let mut scheduler = Scheduler::new(definitions());
    let tree = hosted(true);
    let declarations = Declarations::collect(&tree);
    let asked = [ForeignRepaint::new("examples/gallery.rs:214")];

    for step in 0..3600_u32 {
        let mut frame = frame_of(&tree, u64::from(step) + 1);
        let decision = scheduler.advance(&mut frame, &declarations, f64::from(step) * TICK, &asked);
        assert_eq!(decision.activity.ambient, 1, "step {step}");
        assert!(
            decision.repaint,
            "a declared ambient surface keeps painting"
        );
        assert!(
            decision.activity.is_settled(),
            "and still never blocks settle"
        );
    }
    let report = scheduler
        .idle_audit(60.0)
        .expect("declared traffic is clean");
    assert_eq!(report.declared_ambient, 1);
    assert_eq!(report.frames, 3600);
    assert_eq!(scheduler.frames_requested(), 3600);
}
