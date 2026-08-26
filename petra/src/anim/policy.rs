//! Reduced motion, ambient semantics, and the refusal that keeps idle
//! reachable.
//!
//! Two policies live here because they are the two answers to the same
//! question — *is this motion allowed to run, and does it count against
//! settle?* — and separating them would put the settle contract in two files.
//!
//! # Reduced motion (FR-031)
//!
//! `contracts/animation.md`: *"With reduced motion on: movement
//! (position/size) transitions complete instantly; opacity/dissolve
//! transitions MAY still run (the accepted substitute) ... Toggling reduced
//! motion mid-flight completes running transitions instantly to their
//! targets."*
//!
//! MDN's own canonical `prefers-reduced-motion` example is the same rule:
//! keep the opacity dissolve, drop the scale pulse, same duration
//! (`research-layout-animation.md` §8). Neither MDN, WCAG 2.2 SC 2.3.3, nor
//! Apple's HIG publishes a numeric threshold; the research note records that
//! absence as confirmed rather than as a gap, so this module has no numbers
//! in it beyond the property classification itself.
//!
//! # Where the reduced-motion flag comes from
//!
//! It is set on the [`MotionPolicy`] the scheduler owns, and the host sets it
//! — `gorgon-petra-egui`'s `Host::set_reduced_motion`. It is deliberately
//! **not** a `Theme` field and **not** a `ThemeSnapshot` field. Both of those
//! carry a revision, and that revision is a digest input
//! (`Viewport::theme_rev`) and a measure-cache key. Plumbing an accessibility
//! preference through either would change the digest of every frame in the
//! application the moment the preference toggled, including frames with no
//! motion in them at all — which is exactly the opposite of FR-031's
//! requirement that reduced motion reach *identical end states*. The full
//! argument, and the two alternatives, are in
//! `.agents/notes/implemented/architecture/2026-08-23-petra-motion.md`.
//!
//! # Ambient declaration (FR-062, T084)
//!
//! FR-033's zero-idle claim and FR-039's settle definition are both written in
//! terms of *declared* ambient animation. A self-repainting surface — anything
//! [`crate::frame::PaintContent::repaints_itself`] answers `true` for — puts
//! new pixels on the screen without Petra placing a new frame. If such a
//! surface drives repaints without declaring `ambient`, idle is unreachable
//! and `wait_settle` is unbounded, and SC-002 and every driver journey go with
//! it.
//!
//! The predicate is deliberately **not**
//! [`crate::frame::PaintContent::is_hosted`], which the ledger used until this
//! change. That one is an upper bound on where the *digest* is blind, which is
//! a different question with a different answer: a geometry-only draw list is
//! fully digest-visible and still repaints itself every frame, so attributing
//! against `is_hosted` would leave exactly the surface FR-030 exists to catch
//! invisible to the lane meant to catch it (`research.md` D-05).
//!
//! The rule here is *refuse and report*, both. **Refuse**: an undeclared
//! surface's repaint never reaches [`crate::frame::TransitionActivity`], so
//! the driver's settle wait stays bounded no matter what a painter does.
//! **Report**: [`AmbientLedger`] records it, and [`AmbientLedger::audit`]
//! turns it into an error naming the surface, the source location that asked,
//! and how many times — so the 60-second zero-idle assertion fails for a named
//! reason instead of mysteriously.
//!
//! # Recording and auditing are two different jobs, on two different clocks
//!
//! Being precise about this, because the two are easy to conflate and the
//! difference decides where each one is called from.
//!
//! [`AmbientLedger::observe`] is **continuous**. It runs once per pass, in
//! the shipped host — `gorgon-petra-egui`'s `FrameMotion::advance`, reached
//! from `Host::pass` — because evidence that is not gathered on the frame it
//! happened cannot be recovered afterwards. It is O(self-repainting
//! placements) and allocates nothing when the frame is clean, which is what
//! lets it sit on the idle path at all.
//!
//! [`AmbientLedger::audit`] is a **query over a window**. Nobody can ask it
//! per frame and mean anything: on frame one the observed span is zero
//! seconds, and on every frame it would build a fresh [`IdleViolation`] with
//! a cloned [`Offender`] per entry. It belongs to whoever owns the window —
//! a journey gate closing out a run, or an operator holding a live host — and
//! `gorgon-petra` deliberately does not guess who that is.
//!
//! Between the two sits the per-pass count. [`AmbientLedger::observe`] returns
//! how many *new* offences it just recorded, `Scheduler::advance` carries it
//! out as `FrameDecision::undeclared`, and `Host::decision()` keeps the last
//! one. That is the signal a host can act on without paying for an audit, and
//! `FrameDecision` is `#[must_use]` so a pass cannot quietly drop it.

use std::collections::BTreeMap;
use std::fmt;

use super::value::PropertyKind;
use crate::frame::PetrifiedFrame;

/// How much motion is allowed to run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MotionPolicy {
    reduced_motion: bool,
}

impl MotionPolicy {
    /// The default policy: full motion.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A policy with reduced motion already on.
    #[must_use]
    pub fn reduced() -> Self {
        Self {
            reduced_motion: true,
        }
    }

    /// Whether reduced motion is on.
    #[must_use]
    pub fn reduced_motion(self) -> bool {
        self.reduced_motion
    }

    /// Turn reduced motion on or off, answering whether this changed it.
    ///
    /// The answer is what the engine acts on: *"Toggling reduced motion
    /// mid-flight completes running transitions instantly to their targets."*
    /// A set that changes nothing must not complete anything, or a host
    /// re-publishing the same preference every frame would make every
    /// transition instantaneous.
    pub fn set_reduced_motion(&mut self, on: bool) -> bool {
        let changed = self.reduced_motion != on;
        self.reduced_motion = on;
        changed
    }

    /// Whether `property` may animate under this policy.
    ///
    /// Movement may not; opacity and colour may — the accepted substitution,
    /// not an exemption. A property that may not animate still reaches the
    /// same end state, on the frame the transition would have started.
    #[must_use]
    pub fn animates(self, property: PropertyKind) -> bool {
        !self.reduced_motion || !property.is_movement()
    }
}

/// A repaint request Petra did not make.
///
/// `source` is wherever the request came from — the host passes egui's own
/// `RepaintCause` (a `file:line`), so the report names the code that asked,
/// not just the surface it is suspected of belonging to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForeignRepaint {
    /// Where the request came from, in whatever form the host can name.
    pub source: String,
}

impl ForeignRepaint {
    /// A repaint attributed to `source`.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

/// How often one (surface, source) pair asked, and when.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occurrences {
    /// How many frames it asked on.
    pub count: u64,
    /// The first time it asked, in the host's clock.
    pub first: f64,
    /// The most recent time it asked.
    pub last: f64,
}

/// One entry of an [`IdleViolation`].
#[derive(Clone, Debug, PartialEq)]
pub struct Offender {
    /// The self-repainting placement the request is attributed to, or `None`
    /// when the frame carried no self-repainting placement at all and there is
    /// nothing to attribute it to. `None` is reported, never dropped: an
    /// unattributable repaint still makes idle unreachable.
    pub surface: Option<String>,
    /// Where the request came from.
    pub source: String,
    /// How often, and when.
    pub occurrences: Occurrences,
}

/// A clean audit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IdleReport {
    /// Seconds of host clock the ledger actually observed. Smaller than the
    /// audited window means the window was never reached, which is stated
    /// rather than rounded up.
    pub observed_seconds: f64,
    /// Frames observed over that span.
    pub frames: u64,
    /// Declared-ambient placements in the most recent frame.
    pub declared_ambient: usize,
}

/// A self-repainting surface drove repaints without declaring itself ambient.
#[derive(Clone, Debug, PartialEq)]
pub struct IdleViolation {
    /// The window that was audited, in seconds.
    pub window: f64,
    /// Seconds actually observed.
    pub observed_seconds: f64,
    /// Every distinct offender, sorted.
    pub offenders: Vec<Offender>,
}

impl fmt::Display for IdleViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the {:.0}s zero-idle assertion failed: {} undeclared repaint \
             source(s) over {:.2}s observed. A surface that repaints itself \
             must declare `ambient` (ViewNode::with_ambient), or idle is \
             unreachable and wait_settle is unbounded (FR-033, FR-039, FR-062).",
            self.window,
            self.offenders.len(),
            self.observed_seconds
        )?;
        for offender in &self.offenders {
            write!(
                f,
                "\n  - surface {} asked {} time(s) from {} (first {:.3}s, last {:.3}s)",
                offender
                    .surface
                    .as_deref()
                    .unwrap_or("<none: the frame carried no self-repainting placement>"),
                offender.occurrences.count,
                offender.source,
                offender.occurrences.first,
                offender.occurrences.last
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for IdleViolation {}

/// Records repaint requests Petra did not make, and who is answerable for
/// them.
///
/// The ledger never grants anything. Nothing it records reaches
/// [`crate::frame::TransitionActivity`], which is the refusal half of FR-062;
/// [`AmbientLedger::audit`] is the reporting half.
#[derive(Clone, Debug, Default)]
pub struct AmbientLedger {
    entries: BTreeMap<(Option<String>, String), Occurrences>,
    started: Option<f64>,
    last: Option<f64>,
    frames: u64,
    declared_ambient: usize,
}

impl AmbientLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one frame and any repaint requests that came from outside
    /// Petra's own scheduler, answering how many new offences were recorded.
    ///
    /// `foreign` must already exclude Petra's own requests — the host knows
    /// which of its own call sites asked, and this crate does not. The
    /// attribution rule is stated in the returned [`Offender`]: a request is
    /// attributed to every **self-repainting** placement in the frame
    /// ([`crate::frame::PetrifiedFrame::self_repainting_placements`]) that did
    /// **not** declare `ambient`. When every such placement did declare it,
    /// the request is explained and nothing is recorded — that is what
    /// declaring ambient buys. When the frame carries no self-repainting
    /// placement at all, the request is recorded with no surface rather than
    /// dropped.
    pub fn observe(
        &mut self,
        now: f64,
        frame: &PetrifiedFrame,
        foreign: &[ForeignRepaint],
    ) -> usize {
        self.frames += 1;
        self.started.get_or_insert(now);
        self.last = Some(now);
        let mut undeclared: Vec<&str> = Vec::new();
        let mut declared = 0_usize;
        for (placement, _) in frame.self_repainting_placements() {
            if placement.semantics.ambient {
                declared += 1;
            } else {
                undeclared.push(placement.id.as_str());
            }
        }
        self.declared_ambient = declared;
        if foreign.is_empty() {
            return 0;
        }
        // Every self-repainting surface in the frame declared itself. The
        // request is accounted for, and declaring is exactly what accounts
        // for it.
        if undeclared.is_empty() && declared > 0 {
            return 0;
        }
        let mut recorded = 0;
        for request in foreign {
            if undeclared.is_empty() {
                recorded += usize::from(self.record(None, &request.source, now));
            } else {
                for id in &undeclared {
                    recorded +=
                        usize::from(self.record(Some((*id).to_owned()), &request.source, now));
                }
            }
        }
        recorded
    }

    /// Records one occurrence, answering whether it was the first of its kind.
    fn record(&mut self, surface: Option<String>, source: &str, now: f64) -> bool {
        match self.entries.entry((surface, source.to_owned())) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(Occurrences {
                    count: 1,
                    first: now,
                    last: now,
                });
                true
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                let entry = slot.get_mut();
                entry.count += 1;
                entry.last = now;
                false
            }
        }
    }

    /// Whether nothing undeclared has been seen.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many frames have been observed.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Seconds of host clock between the first and last observation.
    #[must_use]
    pub fn observed_seconds(&self) -> f64 {
        match (self.started, self.last) {
            (Some(start), Some(end)) => end - start,
            _ => 0.0,
        }
    }

    /// The audit over a `window`-second span.
    ///
    /// # Errors
    /// [`IdleViolation`], naming every offending surface and the source that
    /// asked, if anything undeclared was recorded. The window is carried into
    /// the message so the failure reads as the assertion it is, not as a
    /// stray log line.
    pub fn audit(&self, window: f64) -> Result<IdleReport, IdleViolation> {
        if self.entries.is_empty() {
            return Ok(IdleReport {
                observed_seconds: self.observed_seconds(),
                frames: self.frames,
                declared_ambient: self.declared_ambient,
            });
        }
        Err(IdleViolation {
            window,
            observed_seconds: self.observed_seconds(),
            offenders: self
                .entries
                .iter()
                .map(|((surface, source), occurrences)| Offender {
                    surface: surface.clone(),
                    source: source.clone(),
                    occurrences: *occurrences,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AmbientLedger, ForeignRepaint, MotionPolicy};
    use crate::anim::value::PropertyKind;

    #[test]
    fn reduced_motion_stops_movement_and_keeps_the_dissolve() {
        let full = MotionPolicy::new();
        for property in [
            PropertyKind::Position,
            PropertyKind::Size,
            PropertyKind::Opacity,
            PropertyKind::Color,
        ] {
            assert!(full.animates(property), "{property:?}");
        }
        let reduced = MotionPolicy::reduced();
        assert!(!reduced.animates(PropertyKind::Position));
        assert!(!reduced.animates(PropertyKind::Size));
        assert!(reduced.animates(PropertyKind::Opacity));
        assert!(reduced.animates(PropertyKind::Color));
    }

    /// A host republishing the same preference every frame must not complete
    /// every transition every frame.
    #[test]
    fn only_a_real_toggle_reports_a_change() {
        let mut policy = MotionPolicy::new();
        assert!(policy.set_reduced_motion(true));
        assert!(!policy.set_reduced_motion(true));
        assert!(policy.set_reduced_motion(false));
        assert!(!policy.set_reduced_motion(false));
    }

    #[test]
    fn an_empty_ledger_audits_clean() {
        let ledger = AmbientLedger::new();
        let report = ledger.audit(60.0).unwrap();
        assert_eq!(report.frames, 0);
        assert_eq!(report.observed_seconds, 0.0);
        assert!(ledger.is_clean());
    }

    /// The violation message has to name the surface and the source. A test
    /// that only asserted `is_err()` would pass against a message reading
    /// "something went wrong", which is the failure this whole task exists to
    /// prevent.
    #[test]
    fn the_violation_names_the_surface_and_the_source() {
        let violation = super::IdleViolation {
            window: 60.0,
            observed_seconds: 59.98,
            offenders: vec![super::Offender {
                surface: Some("/app/spark".into()),
                source: "examples/gallery.rs:214".into(),
                occurrences: super::Occurrences {
                    count: 3597,
                    first: 0.016,
                    last: 59.98,
                },
            }],
        };
        let shown = violation.to_string();
        assert!(shown.contains("/app/spark"), "{shown}");
        assert!(shown.contains("examples/gallery.rs:214"), "{shown}");
        assert!(shown.contains("3597"), "{shown}");
        assert!(shown.contains("ambient"), "{shown}");
    }

    /// Declaring `ambient` is what accounts for a hosted surface's repaints.
    /// Nothing else does, and nothing else has to.
    #[test]
    fn a_declared_hosted_surface_explains_its_own_repaints() {
        let mut ledger = AmbientLedger::new();
        let tree = crate::anim::fixtures::hosted(true);
        let frame = crate::anim::fixtures::frame(&tree, 1, 100.0, 40.0);
        assert_eq!(
            ledger.observe(0.0, &frame, &[ForeignRepaint::new("painter.rs:1")]),
            0
        );
        assert!(ledger.is_clean());
        assert_eq!(ledger.audit(60.0).unwrap().declared_ambient, 1);
    }

    #[test]
    fn an_undeclared_hosted_surface_is_attributed_to_its_id() {
        let mut ledger = AmbientLedger::new();
        let tree = crate::anim::fixtures::hosted(false);
        let frame = crate::anim::fixtures::frame(&tree, 1, 100.0, 40.0);
        assert_eq!(
            ledger.observe(0.0, &frame, &[ForeignRepaint::new("painter.rs:1")]),
            1
        );
        let err = ledger.audit(60.0).unwrap_err();
        assert_eq!(err.offenders[0].surface.as_deref(), Some("/app/spark"));
        assert_eq!(err.offenders[0].source, "painter.rs:1");
    }

    #[test]
    fn a_repaint_with_no_self_repainting_placement_is_recorded_with_no_surface() {
        let mut ledger = AmbientLedger::new();
        let frame = crate::anim::fixtures::empty_frame();
        let recorded = ledger.observe(0.0, &frame, &[ForeignRepaint::new("somewhere.rs:1")]);
        assert_eq!(recorded, 1);
        let err = ledger.audit(60.0).unwrap_err();
        assert_eq!(err.offenders.len(), 1);
        assert_eq!(err.offenders[0].surface, None);
        assert!(
            err.to_string().contains("no self-repainting placement"),
            "{err}"
        );
    }

    /// One surface declaring `ambient` does not buy its neighbour a pass.
    ///
    /// The short-circuit in `observe` returns early only when *every*
    /// self-repainting placement declared itself. A version that returned on
    /// the first declaration it found would leave the undeclared sibling —
    /// the one actually driving the repaints — unnamed, and the 60-second
    /// zero-idle assertion would pass while idle stayed unreachable.
    #[test]
    fn a_declared_surface_does_not_excuse_an_undeclared_sibling() {
        let mut ledger = AmbientLedger::new();
        let tree = crate::anim::fixtures::two_hosted(true, false);
        let frame = crate::anim::fixtures::frame(&tree, 1, 100.0, 40.0);
        assert_eq!(
            ledger.observe(0.0, &frame, &[ForeignRepaint::new("painter.rs:1")]),
            1
        );
        let err = ledger.audit(60.0).unwrap_err();
        assert_eq!(err.offenders.len(), 1);
        assert_eq!(err.offenders[0].surface.as_deref(), Some("/app/gauge"));
    }

    /// Two undeclared surfaces are two offenders, both named.
    ///
    /// A request cannot be pinned on one of them — the host reports which of
    /// *its* call sites asked, never which surface — so attribution is to the
    /// whole undeclared set. Reporting only the first would send an operator
    /// to fix one surface and watch the assertion fail again.
    #[test]
    fn every_undeclared_surface_is_named_not_just_the_first() {
        let mut ledger = AmbientLedger::new();
        let tree = crate::anim::fixtures::two_hosted(false, false);
        let frame = crate::anim::fixtures::frame(&tree, 1, 100.0, 40.0);
        assert_eq!(
            ledger.observe(0.0, &frame, &[ForeignRepaint::new("painter.rs:1")]),
            2
        );
        let err = ledger.audit(60.0).unwrap_err();
        let named: Vec<&str> = err
            .offenders
            .iter()
            .filter_map(|o| o.surface.as_deref())
            .collect();
        assert_eq!(named, ["/app/gauge", "/app/spark"]);
    }

    /// The ledger attributes against the frame's self-repainting set, and
    /// against nothing else.
    ///
    /// Computed from the frame rather than hardcoded, so the claim survives
    /// the predicate widening: when a geometry-only canvas becomes
    /// self-repainting without becoming hosted (T121), this test's expected
    /// set widens with it and a ledger left on the narrower hosted iterator
    /// starts disagreeing. It cannot separate the two predicates *today* — nothing
    /// can, until that node kind exists — and it is written this way so it
    /// will, rather than pinning the coincidence.
    #[test]
    fn attribution_covers_the_frames_self_repainting_set() {
        let tree = crate::anim::fixtures::two_hosted(false, false);
        let frame = crate::anim::fixtures::frame(&tree, 1, 100.0, 40.0);
        let expected: Vec<&str> = frame
            .self_repainting_placements()
            .filter(|(p, _)| !p.semantics.ambient)
            .map(|(p, _)| p.id.as_str())
            .collect();
        assert!(!expected.is_empty(), "the fixture must have offenders");

        let mut ledger = AmbientLedger::new();
        ledger.observe(0.0, &frame, &[ForeignRepaint::new("painter.rs:1")]);
        let err = ledger.audit(60.0).unwrap_err();
        let mut named: Vec<&str> = err
            .offenders
            .iter()
            .filter_map(|o| o.surface.as_deref())
            .collect();
        named.sort_unstable();
        let mut expected = expected;
        expected.sort_unstable();
        assert_eq!(named, expected);
    }
}
