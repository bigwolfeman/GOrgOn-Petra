//! Damped springs, in closed form, per regime.
//!
//! The ODE is the one `contracts/animation.md` fixes:
//!
//! ```text
//! x'' + 2ζω₀x' + ω₀²x = ω₀²·target
//! ```
//!
//! Substituting `y = x − target` removes the forcing term and leaves the
//! homogeneous `y'' + 2ζω₀y' + ω₀²y = 0`, which is the form every closed
//! solution below is written in. Position **and** velocity are closed forms:
//! `contracts/animation.md` requires both, and the retarget rule (FR-030,
//! SC-007) is unimplementable without velocity — it is the value carried
//! across the interruption.
//!
//! Derivation source: `research-layout-animation.md` §6 (Ryan Juckett's
//! "Damped Springs", the canonical closed-form reference, itself standard
//! control theory). The three regimes and the two parameter conversions are
//! transcribed from that section, not reinvented here.
//!
//! # Why closed form and not a per-frame integrator
//!
//! `contracts/animation.md` §"Frame scheduling" 3: *"closed-form evaluation
//! makes frame-rate independence exact (dropping to 30 Hz changes smoothness,
//! never trajectory endpoints or settle values)"*. A semi-implicit Euler step
//! is cheaper per frame and is not frame-rate independent: the same transition
//! at 30 Hz and at 120 Hz lands on different values, so two hosts would
//! produce two different digests for the same logical frame. That is the
//! whole reason the driver can compare digests at all.

use std::fmt;

use super::value::AnimVector;

/// Two π, spelled once — both parameterisations divide by a period.
const TAU: f64 = std::f64::consts::TAU;

/// Which closed form a spring's parameters select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Regime {
    /// `ζ < 1`: overshoots and rings down.
    Underdamped,
    /// `ζ = 1`: the fastest approach with no overshoot. The default.
    Critical,
    /// `ζ > 1`: two decaying exponentials, no overshoot, slower than critical.
    Overdamped,
}

/// A spring's parameters could not describe a physical spring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpringError {
    /// `ω₀ ≤ 0`, or not finite. A spring with no stiffness never reaches its
    /// target, so this is refused rather than clamped to something arbitrary.
    Frequency(f64),
    /// `ζ < 0`, or not finite. Negative damping is an amplifier: the
    /// trajectory diverges and never settles, so a driver waiting on it would
    /// wait forever.
    Damping(f64),
    /// `response ≤ 0` or `duration ≤ 0`, or not finite.
    Period(f64),
    /// `bounce` outside `[-1, 1]`. `contracts/animation.md` fixes the range;
    /// at `bounce = 1` the spring is undamped and at `bounce = -1` the
    /// conversion divides by zero, so the ends are the ends.
    Bounce(f64),
}

impl fmt::Display for SpringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frequency(v) => write!(
                f,
                "spring frequency ω₀ must be finite and greater than 0, got {v}"
            ),
            Self::Damping(v) => write!(
                f,
                "spring damping ratio ζ must be finite and at least 0, got {v} \
                 (a negative ζ amplifies instead of damping and never settles)"
            ),
            Self::Period(v) => write!(
                f,
                "a spring's response/duration must be finite and greater than 0 seconds, got {v}"
            ),
            Self::Bounce(v) => write!(f, "bounce must be within [-1, 1], got {v}"),
        }
    }
}

impl std::error::Error for SpringError {}

/// A damped harmonic oscillator, parameterised by `(ω₀, ζ)`.
///
/// Unit mass throughout: `stiffness = ω₀²` and `damping = 2ζω₀`, so mass is a
/// free scale that both convenience parameterisations already fold away
/// (`research-layout-animation.md` §6). Carrying it would be a third number
/// that no caller can set meaningfully.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    omega0: f64,
    zeta: f64,
}

impl Default for Spring {
    /// *"Default spring: critically damped"* (`contracts/animation.md`), at a
    /// 0.3 s response — a quarter-second-ish settle, the band WWDC23 calls
    /// standard UI pace and the one `response`/`dampingFraction` was scaled
    /// for.
    fn default() -> Self {
        Self {
            omega0: TAU / 0.3,
            zeta: 1.0,
        }
    }
}

impl Spring {
    /// A spring from the physical parameters directly.
    ///
    /// # Errors
    /// [`SpringError::Frequency`] for a non-positive or non-finite `omega0`,
    /// [`SpringError::Damping`] for a negative or non-finite `zeta`.
    pub fn new(omega0: f64, zeta: f64) -> Result<Self, SpringError> {
        if !omega0.is_finite() || omega0 <= 0.0 {
            return Err(SpringError::Frequency(omega0));
        }
        if !zeta.is_finite() || zeta < 0.0 {
            return Err(SpringError::Damping(zeta));
        }
        Ok(Self { omega0, zeta })
    }

    /// The `response`/`dampingFraction` parameterisation:
    /// `ω₀ = 2π/response`, `ζ = damping_fraction`
    /// (`contracts/animation.md`, `research-layout-animation.md` §6).
    ///
    /// # Errors
    /// [`SpringError::Period`] for a non-positive `response`,
    /// [`SpringError::Damping`] for a negative `damping_fraction`.
    pub fn response(response: f64, damping_fraction: f64) -> Result<Self, SpringError> {
        if !response.is_finite() || response <= 0.0 {
            return Err(SpringError::Period(response));
        }
        Self::new(TAU / response, damping_fraction)
    }

    /// The `duration`/`bounce` parameterisation: `ω₀ = 2π/duration`, and
    ///
    /// ```text
    /// ζ = 1 − bounce            for bounce ≥ 0
    /// ζ = 1 / (1 + bounce)      for bounce < 0
    /// ```
    ///
    /// `bounce = 0` is critical, positive bounce is underdamped, negative
    /// bounce is overdamped (`research-layout-animation.md` §6, the
    /// community-corrected form — the original WWDC23 slide formula is
    /// recorded there as wrong).
    ///
    /// # Errors
    /// [`SpringError::Period`] for a non-positive `duration`,
    /// [`SpringError::Bounce`] for `bounce` outside `[-1, 1]`, and
    /// [`SpringError::Damping`] at `bounce = 1` exactly, where `ζ = 0` is
    /// accepted (an undamped spring rings forever but is a legitimate
    /// ambient shape) — so the only rejection at the ends is `bounce = -1`,
    /// where the conversion is a division by zero.
    pub fn duration_bounce(duration: f64, bounce: f64) -> Result<Self, SpringError> {
        if !duration.is_finite() || duration <= 0.0 {
            return Err(SpringError::Period(duration));
        }
        if !bounce.is_finite() || !(-1.0..=1.0).contains(&bounce) {
            return Err(SpringError::Bounce(bounce));
        }
        let zeta = if bounce >= 0.0 {
            1.0 - bounce
        } else {
            let denom = 1.0 + bounce;
            if denom <= 0.0 {
                return Err(SpringError::Bounce(bounce));
            }
            1.0 / denom
        };
        Self::new(TAU / duration, zeta)
    }

    /// The undamped angular frequency, rad/s.
    #[must_use]
    pub fn omega0(self) -> f64 {
        self.omega0
    }

    /// The damping ratio.
    #[must_use]
    pub fn zeta(self) -> f64 {
        self.zeta
    }

    /// Unit-mass stiffness, `ω₀²`.
    #[must_use]
    pub fn stiffness(self) -> f64 {
        self.omega0 * self.omega0
    }

    /// Unit-mass damping coefficient, `2ζω₀`.
    #[must_use]
    pub fn damping(self) -> f64 {
        2.0 * self.zeta * self.omega0
    }

    /// Which closed form this spring evaluates through.
    ///
    /// The `ζ = 1` band is a window, not an equality: the underdamped form
    /// divides by `ωd = ω₀√(1−ζ²)` and the overdamped form divides by
    /// `2ω₀√(ζ²−1)`, and both denominators go to zero as `ζ → 1`. Inside the
    /// window the critical form — which has no such denominator — is both the
    /// numerically stable answer and, to well under a display pixel, the
    /// physically right one.
    #[must_use]
    pub fn regime(self) -> Regime {
        const NEAR_CRITICAL: f64 = 1e-6;
        if (self.zeta - 1.0).abs() <= NEAR_CRITICAL {
            Regime::Critical
        } else if self.zeta < 1.0 {
            Regime::Underdamped
        } else {
            Regime::Overdamped
        }
    }

    /// Position and velocity at `t` seconds into a trajectory that started at
    /// `x0` with velocity `v0`, heading for `target`. Scalars.
    ///
    /// # Panics
    /// If `t` is negative or not finite. A trajectory has no past; asking for
    /// one is a caller bug, and evaluating `e^{+ζω₀t}` for it would return a
    /// plausible-looking divergent number instead of saying so.
    ///
    /// Also panics if the result is not finite. `omega0` and `t` are each
    /// bounded only by [`f64::is_finite`] (`Spring::new`'s doc: "refused
    /// rather than clamped to something arbitrary" — there is no principled
    /// UI-domain ceiling to pick for a raw physical frequency). An extreme
    /// but individually finite `omega0` and `t` can still overflow an
    /// intermediate product (`omega0 * t`, or `c * t` where `c` carries an
    /// `omega0` factor) to `±inf`, and `0.0 * inf = NaN` once the decay term
    /// has already underflowed to exactly zero. That NaN would look like a
    /// plausible-but-wrong settled value to every caller downstream — the
    /// same failure mode the `t < 0` check above already refuses rather than
    /// hides, extended to the output instead of just the input.
    #[must_use]
    pub fn evaluate_scalar(self, x0: f64, v0: f64, target: f64, t: f64) -> (f64, f64) {
        assert!(
            t.is_finite() && t >= 0.0,
            "a spring is evaluated forward from its own start; t = {t}"
        );
        let y0 = x0 - target;
        let (y, v) = match self.regime() {
            Regime::Underdamped => {
                let omega_d = self.omega0 * (1.0 - self.zeta * self.zeta).sqrt();
                let decay = (-self.zeta * self.omega0 * t).exp();
                let (sin, cos) = (omega_d * t).sin_cos();
                let y = decay * (y0 * cos + ((v0 + self.zeta * self.omega0 * y0) / omega_d) * sin);
                let v = decay
                    * (v0 * cos
                        - ((self.zeta * self.omega0 * v0 + self.omega0 * self.omega0 * y0)
                            / omega_d)
                            * sin);
                (y, v)
            }
            Regime::Critical => {
                let decay = (-self.omega0 * t).exp();
                let c = v0 + self.omega0 * y0;
                (decay * (y0 + c * t), decay * (v0 - c * self.omega0 * t))
            }
            Regime::Overdamped => {
                let root = self.omega0 * (self.zeta * self.zeta - 1.0).sqrt();
                let z1 = -self.omega0 * self.zeta - root;
                let z2 = -self.omega0 * self.zeta + root;
                let c1 = (v0 - y0 * z2) / (z1 - z2);
                let c2 = y0 - c1;
                let (e1, e2) = ((z1 * t).exp(), (z2 * t).exp());
                (c1 * e1 + c2 * e2, c1 * z1 * e1 + c2 * z2 * e2)
            }
        };
        let (pos, vel) = (target + y, v);
        assert!(
            pos.is_finite() && vel.is_finite(),
            "spring closed form overflowed to a non-finite result (pos={pos}, vel={vel}) from \
             omega0={}, zeta={}, t={t} — this is an overflowing product, not a real trajectory \
             value",
            self.omega0,
            self.zeta
        );
        (pos, vel)
    }

    /// The same evaluation, componentwise over a vector.
    ///
    /// Componentwise is the whole reason an animatable value is a vector: each
    /// component is an independent one-dimensional oscillator with the same
    /// `(ω₀, ζ)`, so a rect's four numbers stay in step without the solver
    /// needing to know it is a rect.
    ///
    /// # Panics
    /// If the three vectors do not all have the same length, or if `t` is
    /// negative.
    #[must_use]
    pub fn evaluate(
        self,
        x0: AnimVector,
        v0: AnimVector,
        target: AnimVector,
        t: f64,
    ) -> (AnimVector, AnimVector) {
        // Reuse `AnimVector`'s own length check (see `Sub`'s panic doc in
        // `value.rs`) rather than duplicating it here. Checking `x0` against
        // both `v0` and `target` covers all three by transitivity, and each
        // panic already names both lengths. Without this, the loop below
        // walks only `x0.len()` and would silently leave a longer `v0` or
        // `target` partially unevaluated — the exact truncation
        // `value.rs`'s module doc forbids.
        let _ = x0 - v0;
        let _ = x0 - target;
        let mut pos = x0;
        let mut vel = v0;
        for i in 0..x0.len() {
            let (x, v) = self.evaluate_scalar(x0.get(i), v0.get(i), target.get(i), t);
            pos.set(i, x);
            vel.set(i, v);
        }
        (pos, vel)
    }
}

#[cfg(test)]
mod tests {
    use super::{Regime, Spring, SpringError, TAU};
    use crate::anim::value::AnimVector;

    /// The closed forms must satisfy the ODE the contract fixes. This is the
    /// test that would catch a transcription slip in any of the three
    /// regimes: it never looks at the formulas, only at whether the residual
    /// of `x'' + 2ζω₀x' + ω₀²(x − target)` is zero along the trajectory.
    ///
    /// `x''` comes from a central difference of the closed-form velocity, so
    /// the tolerance is the difference's own truncation error (O(h²) with
    /// h = 1e-4), not a fudge factor.
    #[test]
    fn every_regime_satisfies_the_contract_ode() {
        for (name, spring) in [
            ("underdamped", Spring::new(20.0, 0.35).unwrap()),
            ("critical", Spring::new(20.0, 1.0).unwrap()),
            ("overdamped", Spring::new(20.0, 2.5).unwrap()),
        ] {
            let (x0, v0, target) = (100.0, -40.0, 10.0);
            for step in 1..40 {
                let t = f64::from(step) * 0.01;
                let h = 1e-5;
                let (x, v) = spring.evaluate_scalar(x0, v0, target, t);
                let (_, v_plus) = spring.evaluate_scalar(x0, v0, target, t + h);
                let (_, v_minus) = spring.evaluate_scalar(x0, v0, target, t - h);
                let accel = (v_plus - v_minus) / (2.0 * h);
                let residual = accel
                    + 2.0 * spring.zeta() * spring.omega0() * v
                    + spring.omega0() * spring.omega0() * (x - target);
                // Relative to the sum of the three terms' magnitudes, not to
                // their sum. They are each O(1e4) here and largely cancel —
                // the acceleration passes through zero twice per ring — so
                // scaling by the *result* would demand more precision than a
                // central difference can give exactly where the result is
                // small. Scaling by the inputs asks the right question: is
                // any term wrong by a meaningful fraction of itself? A
                // transcription slip puts the residual at the same order as
                // the terms, which is 1e7 times this bound.
                let scale = accel.abs()
                    + 2.0 * spring.zeta() * spring.omega0() * v.abs()
                    + spring.omega0() * spring.omega0() * (x - target).abs()
                    + 1.0;
                assert!(
                    residual.abs() < 1e-7 * scale,
                    "{name} at t={t}: ODE residual {residual} against scale {scale}, x={x}, v={v}"
                );
            }
        }
    }

    #[test]
    fn a_trajectory_starts_exactly_where_it_was_told_to() {
        for spring in [
            Spring::new(12.0, 0.2).unwrap(),
            Spring::new(12.0, 1.0).unwrap(),
            Spring::new(12.0, 4.0).unwrap(),
        ] {
            let (x, v) = spring.evaluate_scalar(7.5, -2.25, 100.0, 0.0);
            assert!((x - 7.5).abs() < 1e-12, "x0 = {x}");
            assert!((v + 2.25).abs() < 1e-12, "v0 = {v}");
        }
    }

    /// Every regime converges on the target and stops. A spring that did not
    /// would make `wait_settle` unbounded.
    #[test]
    fn every_regime_converges_and_comes_to_rest() {
        for (name, spring) in [
            ("underdamped", Spring::new(20.0, 0.3).unwrap()),
            ("critical", Spring::new(20.0, 1.0).unwrap()),
            ("overdamped", Spring::new(20.0, 3.0).unwrap()),
        ] {
            let (x, v) = spring.evaluate_scalar(0.0, 0.0, 50.0, 8.0);
            assert!((x - 50.0).abs() < 1e-6, "{name}: x = {x}");
            assert!(v.abs() < 1e-6, "{name}: v = {v}");
        }
    }

    /// Only the underdamped regime crosses its target.
    #[test]
    fn overshoot_belongs_to_the_underdamped_regime_alone() {
        let sample = |spring: Spring| {
            (0..400)
                .map(|i| {
                    spring
                        .evaluate_scalar(0.0, 0.0, 1.0, f64::from(i) * 0.005)
                        .0
                })
                .fold(f64::MIN, f64::max)
        };
        assert!(sample(Spring::new(20.0, 0.25).unwrap()) > 1.0 + 1e-3);
        assert!(sample(Spring::new(20.0, 1.0).unwrap()) <= 1.0 + 1e-9);
        assert!(sample(Spring::new(20.0, 2.0).unwrap()) <= 1.0 + 1e-9);
    }

    /// `ω₀ = 2π/response` and `ζ = dampingFraction`, verbatim from the
    /// contract.
    #[test]
    fn the_response_parameterisation_is_the_contract_formula() {
        let s = Spring::response(0.5, 0.8).unwrap();
        assert!((s.omega0() - TAU / 0.5).abs() < 1e-12);
        assert!((s.zeta() - 0.8).abs() < 1e-12);
    }

    /// `ζ = 1 − bounce` above zero, `ζ = 1/(1 + bounce)` below it, and
    /// `bounce = 0` is exactly critical.
    #[test]
    fn the_bounce_parameterisation_maps_onto_the_three_regimes() {
        let critical = Spring::duration_bounce(0.4, 0.0).unwrap();
        assert_eq!(critical.regime(), Regime::Critical);
        assert!((critical.omega0() - TAU / 0.4).abs() < 1e-12);

        let bouncy = Spring::duration_bounce(0.4, 0.3).unwrap();
        assert_eq!(bouncy.regime(), Regime::Underdamped);
        assert!((bouncy.zeta() - 0.7).abs() < 1e-12);

        let sluggish = Spring::duration_bounce(0.4, -0.5).unwrap();
        assert_eq!(sluggish.regime(), Regime::Overdamped);
        assert!((sluggish.zeta() - 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_two_parameterisations_agree_where_they_overlap() {
        // duration/bounce at bounce = b is response/dampingFraction at 1 - b.
        let a = Spring::duration_bounce(0.35, 0.25).unwrap();
        let b = Spring::response(0.35, 0.75).unwrap();
        assert!((a.omega0() - b.omega0()).abs() < 1e-12);
        assert!((a.zeta() - b.zeta()).abs() < 1e-12);
    }

    #[test]
    fn impossible_parameters_are_refused_by_name() {
        assert_eq!(Spring::new(0.0, 1.0), Err(SpringError::Frequency(0.0)));
        assert_eq!(Spring::new(10.0, -0.1), Err(SpringError::Damping(-0.1)));
        assert_eq!(Spring::response(0.0, 1.0), Err(SpringError::Period(0.0)));
        assert_eq!(
            Spring::duration_bounce(0.3, 1.5),
            Err(SpringError::Bounce(1.5))
        );
        assert_eq!(
            Spring::duration_bounce(0.3, -1.0),
            Err(SpringError::Bounce(-1.0))
        );
        assert!(Spring::new(f64::NAN, 1.0).is_err());
        assert!(Spring::new(10.0, f64::INFINITY).is_err());
    }

    #[test]
    fn the_default_spring_is_critically_damped() {
        assert_eq!(Spring::default().regime(), Regime::Critical);
    }

    /// SC-007's core claim, at the level of the solver: evaluate the running
    /// trajectory at `t`, start a new one from that `(x, v)` toward a new
    /// target, and both position and velocity are continuous across the seam
    /// — not approximately, exactly, because the new trajectory's `t = 0`
    /// values are its inputs.
    #[test]
    fn retarget_carries_position_and_velocity_exactly() {
        let spring = Spring::new(18.0, 0.4).unwrap();
        let (x_at, v_at) = spring.evaluate_scalar(0.0, 0.0, 100.0, 0.11);
        assert!(v_at.abs() > 1.0, "the interruption must land mid-flight");
        let (x_after, v_after) = spring.evaluate_scalar(x_at, v_at, -50.0, 0.0);
        assert_eq!(x_at, x_after);
        assert_eq!(v_at, v_after);
    }

    /// Frame-rate independence, which is what closed form buys: the same
    /// trajectory sampled at 30 Hz and at 120 Hz agrees at every shared
    /// instant, so two hosts running at different cadences produce the same
    /// value for the same logical time.
    #[test]
    fn evaluation_is_frame_rate_independent() {
        let spring = Spring::new(15.0, 0.6).unwrap();
        let one_jump = spring.evaluate_scalar(0.0, 0.0, 1.0, 0.5);
        // 30 Hz: fifteen steps of 1/30 s, each restarted from the last state
        // exactly as the engine restarts a trajectory every frame.
        let mut slow = (0.0, 0.0);
        for _ in 0..15 {
            slow = spring.evaluate_scalar(slow.0, slow.1, 1.0, 1.0 / 30.0);
        }
        // 120 Hz: sixty steps of 1/120 s over the same half second.
        let mut fast = (0.0, 0.0);
        for _ in 0..60 {
            fast = spring.evaluate_scalar(fast.0, fast.1, 1.0, 1.0 / 120.0);
        }
        assert!(
            (one_jump.0 - slow.0).abs() < 1e-9,
            "30 Hz drifted: {slow:?}"
        );
        assert!(
            (one_jump.1 - slow.1).abs() < 1e-9,
            "30 Hz drifted: {slow:?}"
        );
        assert!(
            (one_jump.0 - fast.0).abs() < 1e-9,
            "120 Hz drifted: {fast:?}"
        );
        assert!(
            (one_jump.1 - fast.1).abs() < 1e-9,
            "120 Hz drifted: {fast:?}"
        );
    }

    #[test]
    fn vector_evaluation_is_componentwise() {
        let spring = Spring::new(14.0, 0.5).unwrap();
        let x0 = AnimVector::new([0.0, 20.0, 0.0, 0.0], 2);
        let v0 = AnimVector::new([5.0, -5.0, 0.0, 0.0], 2);
        let target = AnimVector::new([10.0, 10.0, 0.0, 0.0], 2);
        let (pos, vel) = spring.evaluate(x0, v0, target, 0.07);
        for i in 0..2 {
            let (x, v) = spring.evaluate_scalar(x0.get(i), v0.get(i), target.get(i), 0.07);
            assert_eq!(pos.get(i), x);
            assert_eq!(vel.get(i), v);
        }
    }

    #[test]
    #[should_panic(expected = "forward from its own start")]
    fn a_spring_refuses_to_be_evaluated_backwards() {
        let _ = Spring::default().evaluate_scalar(0.0, 0.0, 1.0, -0.001);
    }

    /// A `ζ` a hair off 1 must not divide by an almost-zero `ωd`. The near-
    /// critical window is what stops that, and this is the check that it is
    /// actually wide enough to matter.
    #[test]
    fn a_near_critical_zeta_stays_finite() {
        let spring = Spring::new(20.0, 1.0 - 1e-9).unwrap();
        assert_eq!(spring.regime(), Regime::Critical);
        let (x, v) = spring.evaluate_scalar(0.0, 0.0, 1.0, 0.05);
        assert!(x.is_finite() && v.is_finite(), "x={x} v={v}");
    }

    /// F12: `evaluate`'s doc promises a panic when the three vectors do not
    /// all agree in length. `x0`/`target` are 2 components, `v0` is 4 — the
    /// mismatch the doc says is refused.
    #[test]
    #[should_panic(expected = "different lengths")]
    fn evaluate_refuses_mismatched_vector_lengths() {
        let spring = Spring::new(20.0, 1.0).unwrap();
        let x0 = AnimVector::new([1.0, 2.0, 0.0, 0.0], 2);
        let v0 = AnimVector::new([1.0, 2.0, 3.0, 4.0], 4);
        let target = AnimVector::new([5.0, 6.0, 0.0, 0.0], 2);
        let _ = spring.evaluate(x0, v0, target, 0.1);
    }

    /// F13: a legal-but-extreme `omega0` combined with a legal-but-extreme
    /// `t` overflows the closed form's intermediate products in the critical
    /// regime, and used to return `(NaN, NaN)` silently. It must now refuse
    /// instead of returning a plausible-looking divergent number.
    #[test]
    #[should_panic(expected = "overflowed to a non-finite result")]
    fn evaluate_scalar_refuses_a_non_finite_result_critical() {
        let spring = Spring::new(1e10, 1.0).unwrap();
        let _ = spring.evaluate_scalar(0.0, 0.0, 1.0, 1e300);
    }

    /// Same overflow shape, underdamped regime: `omega_d * t` overflows and
    /// `sin`/`cos` of an infinite argument is `NaN` in IEEE 754.
    #[test]
    #[should_panic(expected = "overflowed to a non-finite result")]
    fn evaluate_scalar_refuses_a_non_finite_result_underdamped() {
        let spring = Spring::new(1e10, 0.5).unwrap();
        let _ = spring.evaluate_scalar(0.0, 0.0, 1.0, 1e300);
    }

    /// Same shape, overdamped regime. Both roots stay negative for `ζ > 1`,
    /// so `exp(z * t)` underflows to exactly zero rather than overflowing —
    /// this regime was never reachable to `NaN` this way, and the finiteness
    /// assert added for the other two regimes must not turn this legitimate,
    /// fully-decayed result into a spurious panic.
    #[test]
    fn evaluate_scalar_stays_finite_overdamped_at_the_same_extreme() {
        let spring = Spring::new(1e10, 2.5).unwrap();
        let (x, v) = spring.evaluate_scalar(0.0, 0.0, 1.0, 1e300);
        assert!(x.is_finite() && v.is_finite(), "x={x} v={v}");
    }
}
