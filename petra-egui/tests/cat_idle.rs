//! The cat: spec 005's draw-list acceptance scene, driven through the shipped
//! `Host`, and the `idle-audit` lane's failing case.
//!
//! `specs/005-petra-carbon-authoring/contracts/draw-list.md` T133. The scene
//! itself lives in the gallery binary — `src/bin/gallery/cat.rs`, compiled in
//! here by `#[path]` — so these tests exercise the **shipped** scene rather
//! than a fixture that resembles it. Edit the scene and these tests move with
//! it, which is the whole point: taking `Scene::AMBIENT` away must turn the
//! lane red, and it can only do that if the lane reads the same file the
//! gallery draws.
//!
//! `sibling.rs` in a `tests/` directory is its own crate, so `mod cat` here
//! compiles a second copy of the scene rather than linking the binary's. That
//! is fine and it is the only option Cargo offers: a `src/bin/` module is not
//! reachable from a library or an integration test any other way. The copy is
//! of the *source*, so the two cannot drift.

#[path = "../src/bin/gallery/cat.rs"]
mod cat;

use std::sync::Arc;

use cat::Scene;
use egui::{Context, RawInput};
use gorgon_petra::draw::Command;
use gorgon_petra::frame::{TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Rect as PetraRect, Scale, Size};
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::testing::{Harness, extended_vocabulary, validated_with};
use gorgon_petra::token::{Presenter, ThemeMode, ThemeSnapshot, dark};
use gorgon_petra::tree::{Registry, ViewNode};
use gorgon_petra_egui::draw::{NoAssets, paint_canvas};
use gorgon_petra_egui::host::{App, Host};
use std::ops::Range;

/// The window the scene is driven in. Fractional on purpose: the cat's walk
/// track is built from these numbers, so a round window would let a rounding
/// mistake inside the canvas hide behind an integer.
const WINDOW: Size = Size {
    w: 900.5,
    h: 540.25,
};

/// Seconds of simulated host clock per pass, and how many passes. Sixty
/// simulated seconds, the window `idle_audit` is asked about, at ten passes a
/// second — and ten laps of the cat's six-second walk, so the loop wraps many
/// times rather than once.
const TICK: f64 = 0.1;
const PASSES: u32 = 600;

/// The cat scene, optionally with its ambient declaration taken away.
struct CatApp {
    /// When `false`, the scene is rebuilt with `ambient` stripped off the
    /// canvas. This is the sabotage the `idle-audit` lane exists to catch,
    /// performed in-process so the failing case is a test rather than a
    /// promise about one.
    declare_ambient: bool,
}

impl RowSource for CatApp {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for CatApp {
    fn view(&mut self) -> ViewNode {
        let mut tree = cat::scene();
        if !self.declare_ambient {
            undeclare(&mut tree);
        }
        tree
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

/// Strip `ambient` from every node in `tree`.
///
/// The exact edit an author makes by deleting one `.with_ambient(true)` from
/// the scene, applied from the outside so both directions can be tested in one
/// run. `Arc::make_mut` because the tree shares its children.
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
///
/// The clock is set rather than left to egui's guess so `observed_seconds` is
/// the simulated minute these tests claim, and so six hundred passes take
/// milliseconds. The window is set so the cat's walk track — built from
/// [`WINDOW`] — matches the frame it is animating.
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

fn host_with(declare_ambient: bool, ctx: &Context) -> Host<CatApp> {
    let mut host = Host::new(
        ctx,
        CatApp { declare_ambient },
        Presenter::new(gorgon_petra::token::dark()),
    );
    host.set_transitions(
        cat::transitions(WINDOW).expect("the cat's walk is a well-formed ambient definition"),
    );
    host
}

/// One pass at simulated time `now`, with the scene asking for a repaint from
/// *this* file — which is neither the toolkit nor the host, so
/// `schedule::is_accounted_for` sees a third-party caller, exactly as it would
/// for a canvas author's own `request_repaint`.
///
/// Answers the pass's `undeclared` count, which is what `Host::decision()`
/// published for it.
fn step_asking(ctx: &Context, host: &mut Host<CatApp>, now: f64) -> usize {
    let out = ctx.run_ui(raw_at(now), |_| {
        ctx.request_repaint();
        host.pass(ctx);
    });
    out.drop_without_applying_deltas();
    host.decision()
        .expect("a pass always publishes its decision")
        .undeclared
}

/// **The green half.** The shipped scene, declaring `ambient`, audits clean
/// over a simulated minute of continuous repaints.
///
/// This is the test that goes red when someone deletes the declaration from
/// `cat.rs`. Nothing here reaches into the ledger: the assertions are on state
/// only `Host::pass` -> `FrameMotion::advance` -> `Scheduler::advance` ->
/// `AmbientLedger::observe` could have produced.
#[test]
fn the_cat_scene_declares_ambient_and_audits_clean() {
    // Compile-time, because `Scene::AMBIENT` is a `const`: setting it to
    // `false` should not get as far as a test run. The runtime half of the
    // same claim is the audit below, which goes red if the declaration is
    // deleted from the node rather than flipped on the constant.
    const _: () = assert!(Scene::AMBIENT);

    let ctx = headless();
    let mut host = host_with(true, &ctx);

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
        report.declared_ambient, 1,
        "exactly the cat, seen and accounted for"
    );
    assert_eq!(report.frames, u64::from(PASSES));
}

/// **The red half.** The same scene with its declaration taken away fails the
/// audit, and the failure names the canvas, the file that asked, the fix, and
/// the window.
///
/// Without this the test above would pass against an audit that never fails,
/// which is a green light with nothing behind it.
#[test]
fn the_cat_without_its_ambient_declaration_fails_the_idle_lane() {
    let ctx = headless();
    let mut host = host_with(false, &ctx);

    let mut offences = 0_usize;
    for pass in 0..PASSES {
        offences += step_asking(&ctx, &mut host, f64::from(pass) * TICK);
    }

    assert_eq!(
        offences, 1,
        "one (surface, source) pair became known, once — `undeclared` is an \
         edge, not a level"
    );
    let scheduler = host.motion().scheduler();
    assert!(!scheduler.ledger().is_clean());

    let violation = scheduler
        .idle_audit(60.0)
        .expect_err("an undeclared repainting canvas must fail the idle audit");
    let message = violation.to_string();
    assert!(
        message.contains(Scene::CAT_ID),
        "names the surface: {message}"
    );
    assert!(
        message.contains("cat_idle.rs"),
        "names the source file that asked: {message}"
    );
    assert!(
        message.contains("ambient"),
        "names the fix the author must apply: {message}"
    );
    assert_eq!(violation.offenders.len(), 1, "{message}");
    assert_eq!(
        violation.offenders[0].surface.as_deref(),
        Some(Scene::CAT_ID)
    );
}

/// A canvas is the surface the audit attributes to — the **wider** predicate
/// (`contracts/draw-list.md` §6) — even though it is not hosted.
///
/// This is what "written against the wider predicate" means in practice. Under
/// `is_hosted` alone the frame below carries no attributable surface at all,
/// the two tests above would both record their offence against `<none>`, and
/// the red one would stop naming the cat.
#[test]
fn the_cat_is_unhosted_and_still_the_surface_the_audit_names() {
    let frame = petrify_scene(&cat::scene());
    assert!(
        !frame.hosted(),
        "the cat is drawn from geometry only, so every coordinate of it is in \
         the digest"
    );
    assert_eq!(frame.hosted_placements().count(), 0);
    let repainting: Vec<&str> = frame
        .self_repainting_placements()
        .map(|(p, _)| p.id.as_str())
        .collect();
    assert_eq!(repainting, [Scene::CAT_ID]);

    let (_, content) = frame
        .drawn()
        .find(|(p, _)| p.id == Scene::CAT_ID)
        .expect("the scene places a canvas");
    let list = content
        .canvas
        .as_ref()
        .expect("the canvas carries its list");
    assert!(
        !list.references_assets(),
        "no Sprite: that is what keeps the cat on the unhosted side"
    );
    assert!(
        !list.commands().iter().any(|c| c.asset().is_some()),
        "and there is no asset anywhere in the list"
    );
}

/// The engine loops the walk: the cat crosses the window, wraps, and crosses
/// again, with no host re-triggering anything.
///
/// The assertion is about the **placement**, not about the list — §8's other
/// half. The list is static per frame; what moves is where the canvas was
/// placed, and a track that ran once and settled would leave the cat parked at
/// the right-hand edge for the remaining nine laps.
#[test]
fn the_engine_walks_the_cat_in_a_loop_without_the_host_retriggering() {
    let ctx = headless();
    let mut host = host_with(true, &ctx);
    let mut xs: Vec<f32> = Vec::with_capacity(PASSES as usize);
    for pass in 0..PASSES {
        let out = ctx.run_ui(raw_at(f64::from(pass) * TICK), |_| host.pass(&ctx));
        out.drop_without_applying_deltas();
        if let Some(frame) = host.frame()
            && let Some(placement) = frame.placement(Scene::CAT_ID)
        {
            xs.push(placement.rect.x);
        }
    }
    assert_eq!(xs.len(), PASSES as usize, "the cat is placed every pass");

    // A lap is 6 s and a pass is 0.1 s, so a wrap is a pass where x drops
    // sharply. Ten laps over sixty seconds means nine or ten wraps depending
    // on where the sampling lands; anything less than five is a track that
    // stopped.
    let wraps = xs.windows(2).filter(|w| w[1] < w[0] - 100.0).count();
    assert!(
        wraps >= 5,
        "the walk must loop; saw {wraps} wrap(s) across {} samples spanning \
         x in [{:?}, {:?}]",
        xs.len(),
        xs.iter().copied().fold(f32::INFINITY, f32::min),
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max)
    );
    assert!(
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max) > WINDOW.w / 2.0,
        "the cat must actually cross the window, not jitter at the left edge"
    );
    assert!(
        xs.last().copied().unwrap() < WINDOW.w,
        "sixty seconds in, the cat is still on screen rather than parked past \
         the right-hand edge"
    );
}

/// The cat actually draws: every command reaches `egui`, every token resolves
/// against the shipped dark theme, and nothing is silent.
///
/// A scene that hashed beautifully and painted nothing would satisfy every
/// other test in this file.
#[test]
fn the_cat_draws_every_command_through_the_interpreter() {
    let list = cat::drawing();
    let ctx = headless();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("cat-scene-test"),
    ));
    let theme = ThemeSnapshot::new(dark(), 1);
    let report = paint_canvas(
        &painter,
        &list,
        PetraRect::new(0.0, WINDOW.h - Scene::HEIGHT, Scene::WIDTH, Scene::HEIGHT),
        Scale::new(1.25).unwrap(),
        &theme,
        &mut NoAssets,
    );
    ctx.run_ui(raw_at(0.0), |_| {})
        .drop_without_applying_deltas();

    assert!(
        report.is_complete(),
        "the cat must draw completely against the shipped theme: {report:?}"
    );
    assert_eq!(report.missing_assets, 0);
    assert!(report.unresolved_tokens.is_empty(), "{report:?}");

    // One shape per fill and one per stroke, counted from the list rather than
    // hard-coded: a hard-coded number would drift the moment the cat gains a
    // whisker, and a count taken from the report alone would agree with an
    // interpreter that drew nothing and reported zero.
    let expected: usize = list
        .commands()
        .iter()
        .map(|command| match command {
            Command::Rect { paint, .. } | Command::Ellipse { paint, .. } => {
                usize::from(paint.fill.is_some()) + usize::from(paint.stroke.is_some())
            }
            Command::Path { paint, closed, .. } => {
                usize::from(paint.fill.is_some() && *closed) + usize::from(paint.stroke.is_some())
            }
            Command::Push { .. } | Command::Pop | Command::Sprite { .. } => 0,
        })
        .sum();
    assert!(expected > 10, "the cat is more than a couple of shapes");
    assert_eq!(report.shapes, expected, "{report:?}");
}

/// Petrify the scene at [`WINDOW`], the same size the host drives it at.
///
/// Through `validated_with` rather than `validated`, because the scene names a
/// transition: tree acceptance refuses a name no registry holds, which is the
/// seam `TransitionRef` documents, and the fixture has to hold the same name
/// the host would.
fn petrify_scene(tree: &ViewNode) -> gorgon_petra::frame::PetrifiedFrame {
    let mut registry = Registry::with_vocabulary(extended_vocabulary(tree));
    registry.register_transition(Scene::TRANSITION);
    let mut harness = Harness::new();
    petrify(
        1,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(WINDOW, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}
