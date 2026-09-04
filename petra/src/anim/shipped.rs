//! Transition definitions the component library names.
//!
//! A `toggle` knob always names [`TOGGLE_KNOB`]. The host installs this
//! registry by default so a tree that uses the library is accepted without
//! every application re-declaring the slide.

use super::curve::{CubicBezier, Curve};
use super::registry::{Timing, TransitionDef, TransitionRegistry};
use super::value::PropertyKind;

/// The name [`crate::component::toggle`] puts on its knob.
pub const TOGGLE_KNOB: &str = "toggle-knob";

/// Carbon `$duration-fast-02` is 110 ms. We add 30 ms so a 24-unit travel
/// reads as a slide, not a snap.
const TOGGLE_MS: f64 = 0.14;

/// Carbon `motion(exit, productive)`: `cubic-bezier(0.2, 0, 1, 0.9)`.
fn productive_exit() -> CubicBezier {
    CubicBezier::new(0.2, 0.0, 1.0, 0.9)
}

/// The definitions every host that paints the component library must install.
#[must_use]
pub fn registry() -> TransitionRegistry {
    let mut registry = TransitionRegistry::new();
    registry.register(
        TOGGLE_KNOB,
        TransitionDef::builder()
            .drive(
                PropertyKind::Position,
                Timing::Curve(
                    Curve::new(TOGGLE_MS, productive_exit())
                        .expect("toggle-knob duration is 140 ms"),
                ),
            )
            .build()
            .expect("toggle-knob is a position curve"),
    );
    registry
}

#[cfg(test)]
mod tests {
    use super::{TOGGLE_KNOB, registry};
    use crate::anim::value::PropertyKind;

    #[test]
    fn toggle_knob_is_a_position_curve() {
        let registry = registry();
        let def = registry
            .get(TOGGLE_KNOB)
            .expect("the shipped registry names toggle-knob");
        assert!(def.timing(PropertyKind::Position).is_some());
        assert!(!def.is_ambient());
    }
}
