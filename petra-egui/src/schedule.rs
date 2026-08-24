//! Repaint scheduling: the glue between Petra's motion and egui's frame pump.
//!
//! `gorgon-petra`'s `anim::scheduler` owns the *decision* — whether anything
//! is still moving. This module owns the *plumbing*: reading the host clock,
//! collecting the repaint requests Petra did not make, and turning the answer
//! into `egui::Context::request_repaint`.
//!
//! The split is the same one the crate doc draws for focus:
//! `gorgon-petra` decides what, this crate decides when.
//!
//! # The zero-idle contract on this side
//!
//! `contracts/animation.md` §"Frame scheduling" rule 1 lists four reasons a
//! frame is scheduled: input, a store or projection change, a theme/scale/
//! viewport change, and motion. egui already repaints on the first three, so
//! the only thing this host must do is *not ask* when nothing is running.
//! [`request_if_moving`] is the whole of that, and its decision is
//! `gorgon_petra::anim::wants_frame` — one rule, in one place, so this file
//! and the engine cannot disagree about what "moving" means.
//!
//! # Attributing a repaint nobody declared (FR-062, T084)
//!
//! A hosted painter is handed an `egui::Painter`, and an `egui::Painter` can
//! reach the `Context`. Nothing stops it calling `request_repaint` — and if it
//! does so every frame without its node declaring `ambient`, idle is
//! unreachable and a driver's `wait_settle` never returns.
//!
//! egui records the file and line of every `request_repaint` call
//! ([`egui::RepaintCause`]), which is enough to say *who* asked.
//! [`foreign_causes`] keeps the ones that came from neither the toolkit nor
//! this host, and hands them to the ledger, which refuses to treat them as
//! declared motion and reports them by name.
//!
//! Two honest limits, both stated rather than discovered later:
//!
//! * `Context::repaint_causes` returns the *previous* pass's causes (egui
//!   swaps the list in `begin_pass`), so attribution runs one frame behind. For
//!   a surface repainting continuously — the case that breaks idle — one frame
//!   of lag changes nothing. For a surface that asks exactly once, the request
//!   is attributed to the frame after the one it was made on.
//! * A cause is attributed to the hosted placements *in the frame*, not proven
//!   to come from one of them. The file and line in the message are the proof
//!   of who called; the surface list is the candidate set. The message says so.

use egui::Context;
use gorgon_petra::anim::{
    Declarations, ForeignRepaint, FrameDecision, Scheduler, TransitionRegistry, wants_frame,
};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::tree::ViewNode;

/// Ask for another frame if, and only if, something is still moving.
///
/// This is the decision `Host::schedule` has always made, moved here
/// unchanged: `running > 0 || ambient > 0`. Ambient counts here and does not
/// count for settle, and both are right — see
/// [`gorgon_petra::anim::wants_frame`].
pub fn request_if_moving(ctx: &Context, frame: &PetrifiedFrame) {
    if wants_frame(frame.transitions) {
        ctx.request_repaint();
    }
}

/// Which of egui's recorded repaint causes came from neither the toolkit nor
/// this host.
///
/// Everything the toolkit itself schedules — window events, IME, egui's own
/// widget animations — is the toolkit's business and is not a Petra
/// transition. Everything *this file* and `host.rs` schedule is Petra's own
/// motion and its focus-ring reconciliation, both already accounted for. What
/// is left is application or painter code, which is exactly the population
/// FR-062 is about.
#[must_use]
pub fn foreign_causes(causes: &[egui::RepaintCause]) -> Vec<ForeignRepaint> {
    causes
        .iter()
        .filter(|cause| !is_accounted_for(cause.file))
        .map(|cause| ForeignRepaint::new(format!("{}:{}", cause.file, cause.line)))
        .collect()
}

/// Whether a repaint from `file` is already accounted for.
///
/// `RepaintCause::file` is whatever `file!()` expanded to at the call site: a
/// workspace path for a path dependency, a registry path for a crates.io one.
/// Both carry the crate's own directory as a path component, so the match is
/// on components, not on substrings. That distinction is load-bearing:
/// `gorgon/petra-egui/examples/gallery.rs` contains the text `egui/` and is
/// emphatically not a toolkit file, and a substring match would silently
/// exempt every application example in this repository from the audit.
fn is_accounted_for(file: &str) -> bool {
    /// Crate names whose repaints are the toolkit's own bookkeeping —
    /// window events, IME, egui's widget animations. None of them is a Petra
    /// transition and none of them belongs to a hosted painter.
    const TOOLKIT: [&str; 5] = ["egui", "eframe", "epaint", "ecolor", "accesskit"];
    /// This crate's own two request sites: `host.rs`'s focus-ring
    /// reconciliation and `schedule.rs`'s motion request. Both are already
    /// accounted for by the thing that made them.
    const OURS: [&str; 2] = ["petra-egui/src/host.rs", "petra-egui/src/schedule.rs"];
    if OURS.iter().any(|ours| file.ends_with(ours)) {
        return true;
    }
    file.split('/').any(|part| {
        // A cargo registry directory carries the version: `egui-0.36.1`. A
        // sibling crate carries an underscore: `accesskit_winit`. Both reduce
        // to the crate name; `petra-egui` reduces to `petra`, which is the
        // whole point.
        let stem = part.split(['-', '_']).next().unwrap_or(part);
        TOOLKIT.contains(&stem)
    })
}

/// Everything the host needs to make a frame move.
///
/// Owns the `gorgon-petra` scheduler and nothing else. It is a field on
/// `Host` rather than free functions so that the trajectories survive between
/// passes — an engine rebuilt each frame would restart every transition at
/// every frame and never move.
#[derive(Debug)]
pub struct FrameMotion {
    scheduler: Scheduler,
}

impl FrameMotion {
    /// Motion over `definitions`.
    #[must_use]
    pub fn new(definitions: TransitionRegistry) -> Self {
        Self {
            scheduler: Scheduler::new(definitions),
        }
    }

    /// The scheduler, for its frame counts and its idle audit.
    #[must_use]
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Turn reduced motion on or off.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.scheduler.set_reduced_motion(on);
    }

    /// Advance every transition to this pass's clock and rewrite `frame` into
    /// what is actually on screen.
    ///
    /// The clock is `egui::InputState::time`, the same value every other
    /// timing decision in this pass reads, so a transition and a tooltip
    /// cannot disagree about when now is.
    ///
    /// `tree` is walked once per pass to read the transition declarations off
    /// it. That is one extra walk of the same tree layout already walks twice,
    /// and it is what keeps `ViewNode` plain data: the alternative is a
    /// declaration field on `Placement`, which is a frame-digest change.
    pub fn advance(
        &mut self,
        ctx: &Context,
        frame: &mut PetrifiedFrame,
        tree: &ViewNode,
    ) -> FrameDecision {
        let now = ctx.input(|input| input.time);
        let foreign = foreign_causes(&ctx.repaint_causes());
        let declarations = Declarations::collect(tree);
        self.scheduler.advance(frame, &declarations, now, &foreign)
    }
}

#[cfg(test)]
mod tests {
    use super::{foreign_causes, is_accounted_for};

    fn cause(file: &'static str, line: u32) -> egui::RepaintCause {
        egui::RepaintCause {
            file,
            line,
            reason: "".into(),
        }
    }

    /// The toolkit's own repaints and this host's own repaints are accounted
    /// for; an application's are not. Getting this backwards in either
    /// direction is the whole failure mode: too eager and every real app
    /// fails the idle audit for egui's own bookkeeping, too lax and a
    /// self-animating painter goes unreported.
    #[test]
    fn only_application_causes_are_foreign() {
        let causes = [
            cause(
                "/home/x/.cargo/registry/src/index.crates.io-1/egui-0.36.1/src/context.rs",
                90,
            ),
            cause(
                "/home/x/.cargo/registry/src/index.crates.io-1/eframe-0.36.1/src/native/run.rs",
                12,
            ),
            cause("gorgon/petra-egui/src/host.rs", 504),
            cause("gorgon/petra-egui/src/schedule.rs", 55),
            cause(
                "/home/x/.cargo/registry/src/index.crates.io-1/accesskit_winit-0.1/src/lib.rs",
                3,
            ),
            cause("gorgon/petra-egui/examples/gallery.rs", 214),
            cause("src/panels/sparkline.rs", 77),
        ];
        let foreign = foreign_causes(&causes);
        assert_eq!(foreign.len(), 2, "{foreign:?}");
        assert_eq!(
            foreign[0].source,
            "gorgon/petra-egui/examples/gallery.rs:214"
        );
        assert_eq!(foreign[1].source, "src/panels/sparkline.rs:77");
    }

    /// The trap a substring match falls into: this crate's own directory has
    /// `egui` in its name, so every file under `gorgon/petra-egui/` would be
    /// exempted and no application example could ever be reported.
    #[test]
    fn this_crates_directory_is_not_mistaken_for_the_toolkit() {
        assert!(!is_accounted_for("gorgon/petra-egui/examples/gallery.rs"));
        assert!(!is_accounted_for("gorgon/petra-egui/src/paint.rs"));
        assert!(is_accounted_for("gorgon/petra-egui/src/host.rs"));
    }

    /// The focus-ring request at `host.rs:504` must never be read as a
    /// self-animating surface. It is this host's own reconciliation and it
    /// happens at most once per focus move.
    #[test]
    fn the_focus_ring_repaint_is_not_a_foreign_cause() {
        assert!(is_accounted_for("gorgon/petra-egui/src/host.rs"));
        assert!(foreign_causes(&[cause("gorgon/petra-egui/src/host.rs", 504)]).is_empty());
    }

    #[test]
    fn no_causes_means_no_foreign_causes() {
        assert!(foreign_causes(&[]).is_empty());
    }
}
