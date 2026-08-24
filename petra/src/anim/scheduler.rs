//! The scheduler: one clock, one repaint decision, and the idle audit.
//!
//! `contracts/animation.md` §"Frame scheduling" is the whole specification of
//! this file, and rule 2 is the load-bearing half: *"the moment the last
//! non-ambient transition settles and no ambient exists, scheduling stops —
//! zero frames at idle is a testable invariant (SC-002), not an aspiration."*
//!
//! Rule 1 lists four reasons a frame is scheduled — input, a store or
//! projection change, a theme/scale/viewport change, and motion. The first
//! three are the toolkit's own business and the host already gets them for
//! free (`egui` repaints on input). Motion is the one nothing else knows
//! about, and it is the one this module answers. So [`wants_frame`] is
//! deliberately *only* the motion clause: a scheduler that also tried to
//! decide the other three would be a second, worse copy of the toolkit's event
//! loop.

use super::engine::{Declarations, TransitionEngine};
use super::policy::{AmbientLedger, ForeignRepaint, IdleReport, IdleViolation};
use super::registry::TransitionRegistry;
use crate::frame::{PetrifiedFrame, TransitionActivity};

/// Whether a frame carrying `activity` means another frame must be scheduled.
///
/// Ambient counts here and does **not** count for
/// [`TransitionActivity::is_settled`], and both are correct at once. Ambient
/// motion keeps painting, so frames must keep coming; ambient motion never
/// stops, so a driver that waited for it to stop would wait forever. The two
/// questions are different and the answers differ.
#[must_use]
pub fn wants_frame(activity: TransitionActivity) -> bool {
    activity.running > 0 || activity.ambient > 0
}

/// What one pass decided.
#[must_use = "a pass that ignores its decision cannot report an undeclared \
              repaint; read `undeclared`, or keep the decision where an \
              operator can reach it (FR-062)"]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameDecision {
    /// The motion the frame carries, as written onto it.
    pub activity: TransitionActivity,
    /// Whether the host should ask for another frame.
    pub repaint: bool,
    /// Undeclared repaint offences newly recorded on this pass. Non-zero
    /// means [`Scheduler::idle_audit`] will refuse.
    pub undeclared: usize,
}

/// Owns the transition engine, the repaint decision and the ambient ledger.
///
/// It holds no clock. The host passes its own timestamp into
/// [`Scheduler::advance`], so a test drives sixty simulated seconds in
/// microseconds and a window passes the frame time it already has. A
/// scheduler that read `Instant::now()` itself would make the 60-second
/// zero-idle assertion take sixty real seconds, and a gate nobody can afford
/// to run is a gate that does not run.
#[derive(Debug)]
pub struct Scheduler {
    engine: TransitionEngine,
    ledger: AmbientLedger,
    frames_observed: u64,
    frames_requested: u64,
}

impl Scheduler {
    /// A scheduler over `registry`.
    #[must_use]
    pub fn new(registry: TransitionRegistry) -> Self {
        Self {
            engine: TransitionEngine::new(registry),
            ledger: AmbientLedger::new(),
            frames_observed: 0,
            frames_requested: 0,
        }
    }

    /// The engine.
    #[must_use]
    pub fn engine(&self) -> &TransitionEngine {
        &self.engine
    }

    /// The engine, mutably — for registering nothing and resetting
    /// everything; the registry is fixed at construction.
    pub fn engine_mut(&mut self) -> &mut TransitionEngine {
        &mut self.engine
    }

    /// Turn reduced motion on or off, completing running movement instantly
    /// when it goes on.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.engine.set_reduced_motion(on);
    }

    /// Advance every transition to `now`, rewrite `frame` to what is on
    /// screen, and decide whether to ask for another frame.
    ///
    /// `foreign` is every repaint request that did **not** come from Petra's
    /// own scheduling. The host is the only thing that can tell them apart —
    /// it knows its own call sites — so it filters and passes the rest here.
    /// Nothing in `foreign` reaches [`TransitionActivity`]: an undeclared
    /// self-animating surface cannot buy itself an ambient slot by asking
    /// loudly, which is the refusal half of FR-062.
    pub fn advance(
        &mut self,
        frame: &mut PetrifiedFrame,
        declarations: &Declarations,
        now: f64,
        foreign: &[ForeignRepaint],
    ) -> FrameDecision {
        let activity = self.engine.animate(frame, declarations, now);
        let undeclared = self.ledger.observe(now, frame, foreign);
        self.frames_observed += 1;
        let repaint = wants_frame(activity);
        if repaint {
            self.frames_requested += 1;
        }
        FrameDecision {
            activity,
            repaint,
            undeclared,
        }
    }

    /// How many frames this scheduler has seen.
    #[must_use]
    pub fn frames_observed(&self) -> u64 {
        self.frames_observed
    }

    /// How many passes ended with motion wanting another frame.
    ///
    /// The *motion* contribution to SC-002, not the whole of it. A host asks
    /// for frames for other reasons too — egui repaints on input, and
    /// `petra-egui`'s focus reconciliation asks once when a ring moves — and
    /// none of those reach this counter. What it does promise is the half
    /// this module owns: over a window in which nothing is declared to move,
    /// motion asks for nothing, and this is zero.
    #[must_use]
    pub fn frames_requested(&self) -> u64 {
        self.frames_requested
    }

    /// The ambient ledger.
    #[must_use]
    pub fn ledger(&self) -> &AmbientLedger {
        &self.ledger
    }

    /// The zero-idle audit over a `window`-second span.
    ///
    /// **A query, not a per-frame duty.** The evidence is gathered every pass
    /// by [`Scheduler::advance`]; this asks the accumulated question, and only
    /// whoever owns the window can ask it. That is a journey gate at the end
    /// of a run, or an operator holding a live host. It is deliberately *not*
    /// called from `petra-egui`'s frame pump: answered on frame one it would
    /// report over an observed span of zero seconds, and answered on every
    /// frame it would allocate an [`IdleViolation`] per frame on the one path
    /// whose whole purpose is to do nothing at idle.
    ///
    /// The cheap continuous form is [`AmbientLedger::is_clean`], and the
    /// cheapest of all is [`FrameDecision::undeclared`], which every pass
    /// already returns.
    ///
    /// # Errors
    /// [`IdleViolation`] naming every hosted surface that drove repaints
    /// without declaring `ambient`, and the source that asked.
    pub fn idle_audit(&self, window: f64) -> Result<IdleReport, IdleViolation> {
        self.ledger.audit(window)
    }
}

#[cfg(test)]
mod tests {
    use super::{Scheduler, wants_frame};
    use crate::anim::engine::Declarations;
    use crate::anim::fixtures;
    use crate::frame::TransitionActivity;

    #[test]
    fn only_motion_asks_for_a_frame() {
        assert!(!wants_frame(TransitionActivity::default()));
        assert!(wants_frame(TransitionActivity {
            running: 1,
            ambient: 0
        }));
        assert!(wants_frame(TransitionActivity {
            running: 0,
            ambient: 1
        }));
    }

    /// Ambient keeps frames coming and never blocks settle. The two answers
    /// differ on purpose, and this is where that is pinned.
    #[test]
    fn ambient_schedules_frames_and_still_counts_as_settled() {
        let activity = TransitionActivity {
            running: 0,
            ambient: 2,
        };
        assert!(wants_frame(activity));
        assert!(activity.is_settled());
    }

    /// A static tree over a simulated minute asks for nothing.
    #[test]
    fn a_static_tree_asks_for_no_frames_over_a_simulated_minute() {
        let tree = fixtures::two_panels(0.0);
        let declarations = Declarations::collect(&tree);
        let mut scheduler = Scheduler::new(fixtures::definitions());
        for step in 0..3600_u32 {
            let mut frame = fixtures::frame(&tree, u64::from(step) + 1, 300.0, 120.0);
            let decision =
                scheduler.advance(&mut frame, &declarations, f64::from(step) / 60.0, &[]);
            assert!(!decision.repaint, "asked for a frame at step {step}");
        }
        assert_eq!(scheduler.frames_requested(), 0);
        assert_eq!(scheduler.frames_observed(), 3600);
        let report = scheduler.idle_audit(60.0).unwrap();
        assert_eq!(report.frames, 3600);
        assert!(
            (report.observed_seconds - 59.983_333).abs() < 1e-3,
            "{report:?}"
        );
    }
}
