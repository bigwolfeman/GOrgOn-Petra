//! FR-062 / T084 end to end, through the shipped `Host`.
//!
//! This file exists because of a constraint, and the constraint is worth
//! stating before the tests: `schedule::is_accounted_for` exempts
//! `petra-egui/src/host.rs` and `petra-egui/src/schedule.rs` wholesale, and
//! `egui::Context::request_repaint` is `#[track_caller]`. So a repaint
//! requested from inside either of those files can never be foreign, and
//! `host.rs`'s own unit tests are therefore structurally unable to play the
//! part of a third-party painter. An integration test can: `file!()` here is
//! `gorgon/petra-egui/tests/idle_audit.rs`, which is neither the toolkit nor
//! this host, which is exactly what a hosted painter's file looks like.
//!
//! What is under test is the *production* path, not a simulation of it:
//! `Host::pass` → `FrameMotion::advance` → `Scheduler::advance` →
//! `AmbientLedger::observe`. Nothing here reaches into the ledger directly.

use egui::{Context, RawInput};
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::Presenter;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use std::ops::Range;
use std::sync::Arc;

/// Seconds of simulated host clock per pass. 600 passes at this rate is the
/// sixty seconds SC-002 is written about, driven in milliseconds.
const TICK: f64 = 0.1;
const PASSES: u32 = 600;

/// The custom kind the hosted surface declares, and the painter registered
/// for it.
const SPARK: &str = "spark";

/// An application with exactly one hosted surface, declared ambient or not.
struct Hosted {
    ambient: bool,
}

impl RowSource for Hosted {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Hosted {
    fn view(&mut self) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Custom, SPARK)
                .with_props(Props {
                    custom_kind: Some(SPARK.to_owned()),
                    ..Props::default()
                })
                .with_constraints(Constraints {
                    horizontal: AxisConstraint {
                        min: Some(80.0),
                        max: Some(80.0),
                        priority: 10,
                    },
                    vertical: AxisConstraint {
                        min: Some(24.0),
                        max: Some(24.0),
                        priority: 10,
                    },
                })
                .with_ambient(self.ambient),
        )
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

/// A context that has been through one pass, so `pixels_per_point` and the
/// screen rect are real rather than defaulted.
fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(RawInput::default(), |_| {})
        .drop_without_applying_deltas();
    ctx
}

/// Raw input carrying an explicit host clock.
///
/// The clock is set rather than left to egui's own guess so the ledger's
/// `observed_seconds` is the simulated minute these tests claim it is, and so
/// six hundred passes take milliseconds instead of a minute. `Host::pass`
/// reads exactly this value through `egui::InputState::time`.
fn raw_at(now: f64) -> RawInput {
    RawInput {
        time: Some(now),
        ..RawInput::default()
    }
}

fn host_with(ambient: bool, ctx: &Context) -> Host<Hosted> {
    let mut host = Host::new(
        ctx,
        Hosted { ambient },
        Presenter::new(gorgon_petra::token::dark()),
    );
    // Both halves a hosted surface needs: the name tree acceptance allows,
    // and the painter that draws it. The painter answers `true` — it drew —
    // so the paint report stays complete and `Host::pass`'s own
    // `debug_assert` is satisfied by a real drawing painter rather than by
    // an exemption.
    host.registry_mut().register_custom_kind(SPARK);
    host.painters_mut().register(SPARK, |_painter, _ctx| true);
    host
}

/// Drive one pass at simulated time `now`, with the surface asking for a
/// repaint from *this* file.
///
/// The request is made inside the pass, before `Host::pass` runs, which is
/// where a hosted painter's own `request_repaint` would land. egui hands the
/// *previous* pass's causes to `repaint_causes()`, so the offence is
/// attributed on the following pass — the one-frame lag `schedule.rs`'s
/// module doc states.
///
/// Answers the pass's `undeclared` count, which is what `Host::decision()`
/// published for it.
fn step_asking(ctx: &Context, host: &mut Host<Hosted>, now: f64) -> usize {
    let out = ctx.run_ui(raw_at(now), |_| {
        // The line this file is judged on. `request_repaint` is
        // `#[track_caller]`, so the recorded cause names this file and this
        // line — a third-party painter as far as `foreign_causes` can tell.
        ctx.request_repaint();
        host.pass(ctx);
    });
    out.drop_without_applying_deltas();
    host.decision()
        .expect("a pass always publishes its decision")
        .undeclared
}

/// The finding this file was written for: the **shipped host** — not a test
/// harness driving `Scheduler` by hand — records an undeclared repainting
/// surface, and the audit names it.
///
/// Every assertion below runs against state that only `Host::pass` could have
/// produced. If the production wiring from `Host::pass` down to
/// `AmbientLedger::observe` were removed, `is_clean()` would stay true and
/// `idle_audit` would answer `Ok`, and this test would fail.
#[test]
fn the_shipped_host_reports_an_undeclared_repainting_surface_by_name() {
    let ctx = headless();
    let mut host = host_with(false, &ctx);

    let mut new_offences = 0_usize;
    for pass in 0..PASSES {
        new_offences += step_asking(&ctx, &mut host, f64::from(pass) * TICK);
    }

    // `undeclared` is an *edge*: `observe` answers how many (surface, source)
    // pairs it had never seen before. One surface asking from one line, six
    // hundred times, is one new offence — not six hundred. Asserting the
    // exact number rather than `> 0` is what pins that meaning.
    assert_eq!(
        new_offences, 1,
        "one (surface, source) pair became known, once"
    );

    let scheduler = host.motion().scheduler();
    assert!(
        !scheduler.ledger().is_clean(),
        "the level signal: the ledger carries the offence for the whole run"
    );

    let violation = scheduler
        .idle_audit(60.0)
        .expect_err("an undeclared repainting surface must fail the idle audit");
    let message = violation.to_string();

    assert!(
        message.contains("/root/spark"),
        "names the surface: {message}"
    );
    assert!(
        message.contains("idle_audit.rs"),
        "names the source file that asked: {message}"
    );
    assert!(
        message.contains("ambient"),
        "names the fix the author must apply: {message}"
    );
    assert!(message.contains("60s"), "names the window: {message}");

    assert_eq!(violation.offenders.len(), 1, "{message}");
    let offender = &violation.offenders[0];
    assert_eq!(offender.surface.as_deref(), Some("/root/spark"));
    // 600 passes, minus the first — `repaint_causes()` answers the previous
    // pass, so pass 0 sees an empty list.
    assert_eq!(
        offender.occurrences.count,
        u64::from(PASSES) - 1,
        "counted every pass the request was visible on: {message}"
    );
}

/// The control. Declare `ambient` on the same surface making the same
/// requests, and the shipped host audits clean.
///
/// Without this the test above would pass against a host that reported
/// everything, which would be a different bug and an equally useless audit.
#[test]
fn the_shipped_host_audits_a_declared_surface_clean() {
    let ctx = headless();
    let mut host = host_with(true, &ctx);

    let mut new_offences = 0_usize;
    for pass in 0..PASSES {
        new_offences += step_asking(&ctx, &mut host, f64::from(pass) * TICK);
    }

    assert_eq!(new_offences, 0, "declaring ambient is what accounts for it");
    let scheduler = host.motion().scheduler();
    assert!(scheduler.ledger().is_clean());

    let report = scheduler
        .idle_audit(60.0)
        .expect("a declared surface's traffic is accounted for");
    assert_eq!(report.declared_ambient, 1, "the declaration was seen");
    assert_eq!(report.frames, u64::from(PASSES));
}

/// A quiet application asks for nothing and audits clean — the SC-002 case
/// itself, through the shipped host.
///
/// This is the test that would go red if `Host::pass` ever started asking for
/// frames it does not need, and it is the baseline the two tests above are
/// measured against.
#[test]
fn a_quiet_host_asks_for_no_motion_frames_and_audits_clean() {
    let ctx = headless();
    let mut host = host_with(false, &ctx);

    for pass in 0..PASSES {
        let out = ctx.run_ui(raw_at(f64::from(pass) * TICK), |_| host.pass(&ctx));
        out.drop_without_applying_deltas();
        assert_eq!(
            host.decision().expect("a decision").undeclared,
            0,
            "nothing asked, so nothing may be recorded (pass {pass})"
        );
    }

    let scheduler = host.motion().scheduler();
    assert_eq!(
        scheduler.frames_requested(),
        0,
        "SC-002: motion asks for no frames over a simulated minute"
    );
    assert_eq!(scheduler.frames_observed(), u64::from(PASSES));
    assert!(scheduler.ledger().is_clean());
    scheduler.idle_audit(60.0).expect("a quiet host is clean");
}
