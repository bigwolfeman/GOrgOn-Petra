//! The skeleton: spec 008's idle-audit case (US3, FR-017, SC-009) — a scene
//! of pulsing skeletons driven through the shipped `Host`.
//!
//! `specs/008-petra-carbon-variants/contracts/skeleton.md` binds the pulse to
//! spec 003's `with_ambient` and the `AmbientLedger`: a skeleton repaints
//! only as a declared-ambient surface, advanced by the component layer's own
//! `advance_ambient_skeletons` on the host clock. This file is that claim
//! under the `idle-audit` lane, in both directions — the shipped scene
//! declaring `ambient` audits clean, and the same scene with the declaration
//! taken away goes red naming the skeleton's placement ids — the same
//! two-directional test `cat_idle.rs` holds for the cat. "Skeletons on
//! screen" is what the lane must survive (SC-009); a lane that passed only
//! because nothing animated would be a green light with nothing behind it.

use std::sync::Arc;

use egui::{Context, RawInput};
use gorgon_petra::component::{SizeStep, SkeletonAnatomy, advance_ambient_skeletons, skeleton};
use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Axis, Size};
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::testing::{Harness, validated};
use gorgon_petra::token::{Presenter, ThemeMode, dark};
use gorgon_petra::tree::{NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use std::ops::Range;

/// The window the scene is driven in.
const WINDOW: Size = Size { w: 640.0, h: 360.0 };

/// Seconds of simulated host clock per pass, and how many passes: the same
/// simulated minute `cat_idle` audits the cat over.
const TICK: f64 = 0.1;
const PASSES: u32 = 600;

/// The three blocks on screen — a text bar, the circular knob, and a tag
/// block — and the placement ids the audit names.
const BAR: &str = "skeleton-bar";
const KNOB: &str = "skeleton-knob";
const CHIP: &str = "skeleton-chip";
const BAR_ID: &str = "/root/skeleton-bar";
const KNOB_ID: &str = "/root/skeleton-knob";
const CHIP_ID: &str = "/root/skeleton-chip";

/// The scene at host clock `now`: three blocks of the one shared shape,
/// published at phase zero and then advanced by the component layer's own
/// [`advance_ambient_skeletons`] — exactly how a contributed snapshot is kept
/// current, and the only clock that may move them. Under `reduced` the walk
/// draws Carbon's own `animation: none` rest state and the blocks stop asking
/// for frames.
#[must_use]
pub fn scene(now: f64, reduced: bool) -> ViewNode {
    let mut root = ViewNode::new(NodeKind::Stack, "root")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(skeleton(
            BAR,
            SkeletonAnatomy::Text,
            SizeStep::Md,
            160.0,
            0.0,
        ))
        .child(skeleton(
            KNOB,
            SkeletonAnatomy::Knob,
            SizeStep::Md,
            18.0,
            0.0,
        ))
        .child(skeleton(
            CHIP,
            SkeletonAnatomy::Tag,
            SizeStep::Md,
            60.0,
            0.0,
        ));
    advance_ambient_skeletons(&mut root, now, reduced);
    root
}

/// The scene, optionally with its ambient declaration taken away.
///
/// When `false`, the tree is rebuilt with `ambient` stripped off every node.
/// This is the sabotage the `idle-audit` lane exists to catch, performed
/// in-process so the failing case is a test rather than a promise about one.
struct SkeletonApp {
    now: f64,
    reduced: bool,
    declare_ambient: bool,
}

impl RowSource for SkeletonApp {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for SkeletonApp {
    fn tick(&mut self, now: f64) {
        self.now = now;
    }

    fn view(&mut self) -> ViewNode {
        let mut tree = scene(self.now, self.reduced);
        if !self.declare_ambient {
            undeclare(&mut tree);
        }
        tree
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

/// Strip `ambient` from every node in `tree`.
///
/// The exact edit an author makes by deleting one `.with_ambient(true)`,
/// applied from the outside so both directions can be tested in one run.
fn undeclare(tree: &mut ViewNode) {
    tree.ambient = false;
    for child in &mut tree.children {
        undeclare(Arc::make_mut(child));
    }
}

/// A context that has been through one pass at [`WINDOW`], so
/// `pixels_per_point` and the content rect are real rather than defaulted.
fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(raw_at(0.0), |_| {})
        .drop_without_applying_deltas();
    ctx
}

/// Raw input carrying an explicit host clock and an explicit window.
fn raw_at(now: f64) -> RawInput {
    RawInput {
        time: Some(now),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(WINDOW.w, WINDOW.h),
        )),
        ..RawInput::default()
    }
}

fn host_with(declare_ambient: bool, reduced: bool, ctx: &Context) -> Host<SkeletonApp> {
    Host::new(
        ctx,
        SkeletonApp {
            now: 0.0,
            reduced,
            declare_ambient,
        },
        Presenter::new(dark()),
    )
}

/// One pass at simulated time `now`, with the scene asking for a repaint from
/// *this* file — which is neither the toolkit nor the host, so
/// `schedule::is_accounted_for` sees a third-party caller, exactly as it
/// would for a canvas author's own `request_repaint`.
///
/// Answers the pass's `undeclared` count, which is what `Host::decision()`
/// published for it.
fn step_asking(ctx: &Context, host: &mut Host<SkeletonApp>, now: f64) -> usize {
    let out = ctx.run_ui(raw_at(now), |_| {
        ctx.request_repaint();
        host.pass(ctx);
    });
    out.drop_without_applying_deltas();
    host.decision()
        .expect("a pass always publishes its decision")
        .undeclared
}

/// **The green half.** Three pulsing skeletons, declaring `ambient`, audit
/// clean over a simulated minute of continuous repaints.
///
/// This is SC-009 as a test: `gates idle-audit` passes with skeletons on
/// screen, and passes because the pulse is declared — not because nothing
/// moved.
#[test]
fn the_skeleton_scene_declares_ambient_and_audits_clean() {
    let ctx = headless();
    let mut host = host_with(true, false, &ctx);

    let mut offences = 0_usize;
    for pass in 0..PASSES {
        offences += step_asking(&ctx, &mut host, f64::from(pass) * TICK);
    }

    assert_eq!(offences, 0, "declaring ambient is what accounts for it");
    let scheduler = host.motion().scheduler();
    assert!(scheduler.ledger().is_clean());
    let report = scheduler
        .idle_audit(60.0)
        .expect("a declared repainting canvas is accounted for");
    assert_eq!(
        report.declared_ambient, 3,
        "exactly the three blocks on screen, seen and accounted for"
    );
    assert_eq!(report.frames, u64::from(PASSES));
}

/// **The red half.** The same scene with its declaration taken away fails the
/// audit, and the failure names every block, the file that asked, and the fix.
///
/// Without this the test above would pass against an audit that never fails,
/// which is a green light with nothing behind it.
#[test]
fn the_skeleton_scene_without_its_ambient_declaration_fails_the_idle_lane() {
    let ctx = headless();
    let mut host = host_with(false, false, &ctx);

    let mut offences = 0_usize;
    for pass in 0..PASSES {
        offences += step_asking(&ctx, &mut host, f64::from(pass) * TICK);
    }

    assert_eq!(
        offences, 3,
        "one (surface, source) pair per block became known, once — \
         `undeclared` is an edge, not a level"
    );
    let scheduler = host.motion().scheduler();
    assert!(!scheduler.ledger().is_clean());

    let violation = scheduler
        .idle_audit(60.0)
        .expect_err("an undeclared repainting canvas must fail the idle audit");
    let message = violation.to_string();
    for id in [BAR_ID, KNOB_ID, CHIP_ID] {
        assert!(message.contains(id), "names the surface {id}: {message}");
    }
    assert!(
        message.contains("skeleton_idle.rs"),
        "names the source file that asked: {message}"
    );
    assert!(
        message.contains("ambient"),
        "names the fix the author must apply: {message}"
    );
    assert_eq!(violation.offenders.len(), 3, "{message}");
    let mut named: Vec<String> = violation
        .offenders
        .iter()
        .map(|offender| {
            offender
                .surface
                .clone()
                .unwrap_or_else(|| "<none>".to_owned())
        })
        .collect();
    named.sort();
    assert_eq!(named, [BAR_ID, CHIP_ID, KNOB_ID]);
}

/// The audited scene really has skeletons on screen: three placements, each
/// carrying `PlacementSemantics::skeleton`, so a driver distinguishes loading
/// from empty without pixels (FR-015) — and nothing that is not a skeleton
/// claims to be one.
#[test]
fn the_audited_scene_carries_three_driver_visible_skeleton_placements() {
    let frame = petrify_scene(&scene(0.6, false));
    let mut ids: Vec<&str> = frame
        .drawn()
        .filter(|(placement, _)| placement.semantics.skeleton)
        .map(|(placement, _)| placement.id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [BAR_ID, CHIP_ID, KNOB_ID],
        "the loading tree says skeleton; an empty tree would carry none"
    );
}

/// Petrify the scene at [`WINDOW`], the same size the host drives it at.
///
/// Plain `validated`: the scene names no transition, so there is no registry
/// seam to hold (`cat_idle`'s `validated_with` exists for the cat's walk).
fn petrify_scene(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        Viewport::new(WINDOW, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}
