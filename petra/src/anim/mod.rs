//! Transitions: animatable values, curves, springs, keyframes, and the
//! idle-suppressing frame scheduler.
//!
//! The binding document is `specs/003-petra-layout-engine/contracts/animation.md`.
//! Where this module and that contract disagree, the contract wins.
//!
//! # The shape of it
//!
//! * [`value`] — what an animatable value *is*: a fixed-length vector, one
//!   interpolation rule and one pair of settle thresholds per property type.
//! * [`spring`] and [`curve`] — the two timing families, both answering
//!   position *and* velocity in closed form so a retarget has something to
//!   carry.
//! * [`registry`] — named definitions, and the seam that makes
//!   `crate::tree::TransitionRef`'s "an unresolvable name is a tree-acceptance
//!   error" true by construction.
//! * [`engine`] — the key-diff, the retarget, the exits, and the exact snap
//!   that keeps residue out of the digest.
//! * [`policy`] — reduced motion, and the refusal that keeps idle reachable
//!   when a hosted surface animates itself without saying so.
//! * [`scheduler`] — one repaint decision and the zero-idle audit.
//!
//! # What a host wires up
//!
//! ```no_run
//! # use gorgon_petra::anim::{Scheduler, Declarations};
//! # use gorgon_petra::anim::registry::{TransitionDef, TransitionRegistry, Timing};
//! # use gorgon_petra::anim::spring::Spring;
//! # use gorgon_petra::anim::value::PropertyKind;
//! # use gorgon_petra::tree::Registry;
//! let mut definitions = TransitionRegistry::new();
//! definitions.register(
//!     "slide",
//!     TransitionDef::builder()
//!         .drive(PropertyKind::Position, Timing::Spring(Spring::default()))
//!         .build()
//!         .unwrap(),
//! );
//! // Tree acceptance now allows exactly the names the engine can resolve.
//! let mut tree_registry = Registry::new();
//! definitions.declare_into(&mut tree_registry);
//! let scheduler = Scheduler::new(definitions);
//! ```

pub mod curve;
pub mod engine;
pub mod policy;
pub mod registry;
pub mod scheduler;
pub mod spring;
pub mod value;

pub use engine::{Declarations, TransitionEngine};
pub use policy::{AmbientLedger, ForeignRepaint, IdleReport, IdleViolation, MotionPolicy};
pub use registry::{ExitRule, Timing, Track, TransitionDef, TransitionRegistry};
pub use scheduler::{FrameDecision, Scheduler, wants_frame};
pub use spring::{Regime, Spring, SpringError};
pub use value::{AnimVector, Animatable, Opacity, PropertyKind, Thresholds};

/// Fixtures shared by this module's own tests.
///
/// In one place rather than one copy per file: three of these test modules
/// need a petrified frame from a tree, and three slightly different
/// hand-rolled versions of that is exactly how two of them end up testing
/// something subtly different from the third.
#[cfg(test)]
pub(crate) mod fixtures {
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, extended_vocabulary, validated_with};
    use crate::token::ThemeMode;
    use crate::tree::{AxisConstraint, Constraints, NodeKind, Props, Registry, ViewNode};

    use super::registry::{ExitRule, Timing, Track, TransitionDef, TransitionRegistry};
    use super::spring::Spring;
    use super::value::PropertyKind;

    /// The definitions every fixture tree names.
    ///
    /// `slide` moves, `fade-out` fades and runs an exit track. One place, so
    /// two tests cannot disagree about what "slide" means.
    pub fn definitions() -> TransitionRegistry {
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
            "fade-out",
            TransitionDef::builder()
                .drive(
                    PropertyKind::Opacity,
                    Timing::Spring(Spring::response(0.2, 1.0).unwrap()),
                )
                .exit(ExitRule::RunExitTrack(Track::fade()))
                .build()
                .unwrap(),
        );
        registry
    }

    /// Petrify `tree` at `seq` against a `w` × `h` viewport.
    ///
    /// Every transition name the tree uses is registered for acceptance
    /// first, so a fixture cannot be refused for naming its own transition —
    /// what the *engine* resolves is a separate question, and one test
    /// deliberately leaves it unresolvable.
    pub fn frame(tree: &ViewNode, seq: u64, w: f32, h: f32) -> PetrifiedFrame {
        let mut registry = Registry::with_vocabulary(extended_vocabulary(tree));
        register_declared_names(tree, &mut registry);
        let mut harness = Harness::new();
        petrify(
            seq,
            validated_with(tree, &registry),
            &mut harness.ctx(),
            Viewport::new(Size::new(w, h), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    fn register_declared_names(node: &ViewNode, registry: &mut Registry) {
        if let Some(reference) = node.transition.as_ref() {
            registry.register_transition(reference.name());
        }
        for child in &node.children {
            register_declared_names(child, registry);
        }
    }

    /// A horizontal stack whose `/app/mover` sits `offset` logical units from
    /// the left, behind a rigid spacer, and slides when that changes.
    ///
    /// A spacer rather than a longer label: changing a label's text would
    /// change the mover's position *and* the leading node's content hash, and
    /// a test asserting "the picture moved" could then pass on the wrong
    /// half.
    pub fn two_panels(offset: f32) -> ViewNode {
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
                    .with_transition("slide"),
            )
    }

    /// A tree with one hosted (image) node, ambient or not.
    ///
    /// `NodeKind::Image` with a source is one of exactly two payloads
    /// `PaintContent::is_hosted` answers true for, so this is a real hosted
    /// surface as far as every rule in `policy` is concerned.
    pub fn hosted(ambient: bool) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "app").child(
            ViewNode::new(NodeKind::Image, "spark")
                .with_props(Props {
                    image: Some("spark.png".into()),
                    ..Props::default()
                })
                .with_ambient(ambient),
        )
    }

    /// A frame with no hosted placement at all.
    pub fn empty_frame() -> PetrifiedFrame {
        frame(&two_panels(0.0), 1, 100.0, 40.0)
    }
}
