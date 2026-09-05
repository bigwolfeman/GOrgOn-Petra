//! Transition definitions the component library names.
//!
//! A `toggle` knob always names [`TOGGLE_KNOB`] and every `button` names
//! [`BUTTON_PRESS`]. The host installs this registry by default so a tree
//! that uses the library is accepted without every application re-declaring
//! them.

use super::curve::{CubicBezier, Curve};
use super::registry::{Timing, TransitionDef, TransitionRegistry};
use super::value::PropertyKind;

/// The name [`crate::component::toggle`] puts on its knob.
pub const TOGGLE_KNOB: &str = "toggle-knob";

/// The name every `crate::component::button` constructor puts on its
/// container and on the children it packs, so the whole control crunches
/// together on a press.
pub const BUTTON_PRESS: &str = "button-press";

/// Carbon `$duration-fast-02` is 110 ms. We add 30 ms so a 24-unit travel
/// reads as a slide, not a snap.
const TOGGLE_MS: f64 = 0.14;

/// MEASURED `@carbon/motion/scss/generated/_tokens.scss:32`:
/// `$duration-fast-01: 70ms`, whose own doc comment reads *"Micro-
/// interactions such as **button** and toggle. Instant response to user
/// action."* Carbon spends it on a button's `background`, `box-shadow`,
/// `border-color` and `outline` (MEASURED
/// `@carbon/styles/scss/components/button/_mixins.scss:72-76`) and on
/// nothing geometric. The duration is Carbon's; what it drives here is not.
const PRESS_MS: f64 = 0.07;

/// How far a pressed button pulls in on every side, in logical units.
///
/// **Carbon does not do this at all** — see [`registry`]. 2 units is 10% of
/// a 40-unit button's height across the pair of edges and 4% of a 98-unit
/// `Delete` button's width, which is a dip a reader sees without it
/// reading as the control jumping. At the capture scale the gallery
/// photographs at it is 4 device pixels on each edge, which is why
/// `shots::tests::a_pressed_button_crunches_down_and_comes_back` can
/// measure it off a picture rather than only off a rect.
const PRESS_INSET: f32 = 2.0;

/// Carbon `motion(exit, productive)`: `cubic-bezier(0.2, 0, 1, 0.9)`.
fn productive_exit() -> CubicBezier {
    CubicBezier::new(0.2, 0.0, 1.0, 0.9)
}

/// Carbon `motion(entrance, productive)`: `cubic-bezier(0, 0, 0.38, 0.9)`
/// (MEASURED `@carbon/motion/scss/generated/_tokens.scss:19`). The curve
/// Carbon's own button transition names.
fn productive_entrance() -> CubicBezier {
    CubicBezier::new(0.0, 0.0, 0.38, 0.9)
}

/// The definitions every host that paints the component library must install.
///
/// # [`BUTTON_PRESS`] is a departure from Carbon, on the operator's
/// instruction
///
/// Carbon's button transitions `background`, `box-shadow`, `border-color`
/// and `outline` and never its geometry (MEASURED
/// `@carbon/styles/scss/components/button/_mixins.scss:72-76`); there is
/// no `transform`, no `scale` and no press-down anywhere in
/// `_button.scss`. Round 4, row 04: *"Buttons: we need to give these an on
/// click animation so it is like it is crunching down on the click"*.
///
/// So the *depth* of the dip is ours ([`PRESS_INSET`]) and the timing is
/// Carbon's ([`PRESS_MS`], [`productive_entrance`]). The mechanism is the
/// one this module already exists to be:
/// [`crate::anim::TransitionDef::press_inset`] moves the target and the
/// engine interpolates the trip, so nothing in `component` owns a clock and
/// nothing about the crunch can push a button's neighbours around.
#[must_use]
pub fn registry() -> TransitionRegistry {
    let mut registry = TransitionRegistry::new();
    registry.register(
        BUTTON_PRESS,
        TransitionDef::builder()
            .drive(
                PropertyKind::Position,
                Timing::Curve(
                    Curve::new(PRESS_MS, productive_entrance())
                        .expect("button-press duration is 70 ms"),
                ),
            )
            .drive(
                PropertyKind::Size,
                Timing::Curve(
                    Curve::new(PRESS_MS, productive_entrance())
                        .expect("button-press duration is 70 ms"),
                ),
            )
            .press(PRESS_INSET)
            .build()
            .expect("button-press drives position and size with a press inset"),
    );
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
    use super::{BUTTON_PRESS, PRESS_INSET, TOGGLE_KNOB, registry};
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

    /// The press response drives **both** halves of a centre scale and
    /// carries a non-zero inset.
    ///
    /// Falsify by dropping either `.drive` call, which `build` refuses, or
    /// by dropping `.press(PRESS_INSET)`, which leaves a definition that
    /// animates a button's rect toward the rect it already has — a
    /// transition that is registered, resolved, and does nothing.
    #[test]
    fn button_press_scales_about_the_centre_by_a_real_distance() {
        let registry = registry();
        let def = registry
            .get(BUTTON_PRESS)
            .expect("the shipped registry names button-press");
        assert!(def.timing(PropertyKind::Position).is_some());
        assert!(def.timing(PropertyKind::Size).is_some());
        assert!(!def.is_ambient());
        assert_eq!(def.press_inset(), PRESS_INSET);
        assert!(
            def.press_inset() > 0.0,
            "a zero inset is a transition that animates a rect to itself"
        );
    }

    /// A press inset on a definition that drives only one half of the scale
    /// is refused, with both halves named.
    #[test]
    fn a_press_response_that_drives_only_one_axis_of_the_scale_is_refused() {
        use crate::anim::registry::{Timing, TransitionDef};
        use crate::anim::spring::Spring;

        let err = TransitionDef::builder()
            .drive(
                PropertyKind::Position,
                Timing::Spring(Spring::response(0.1, 1.0).unwrap()),
            )
            .press(2.0)
            .build()
            .expect_err("a half-scale press response is refused");
        assert!(
            err.contains("Size"),
            "the refusal has to name the property that is missing: {err}"
        );
    }
}
