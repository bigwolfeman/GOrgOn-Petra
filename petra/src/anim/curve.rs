//! Cubic-bezier easing, the named presets, and keyframe tracks.
//!
//! `contracts/animation.md`: *"**Curve**: duration + easing (standard
//! cubic-bezier; named presets `linear`, `ease`, `ease-in`, `ease-out`,
//! `ease-in-out` with their canonical control points)"* and *"**Keyframes**: a
//! per-property track of (time, value, easing-in) points; tracks compose with
//! the same interpolation rules"*.
//!
//! The control points are the CSS ones, which `research-layout-animation.md`
//! §7 confirms are numerically identical to SwiftUI's `UnitCurve` values,
//! control-point for control-point. Two independent standards agreeing is why
//! they are transcribed rather than tuned.
//!
//! # Velocity, and why a curve has one
//!
//! `contracts/animation.md` on retargeting: *"Curves retarget by the same rule
//! (velocity estimated from the curve derivative)."* So a curve is not just a
//! `progress → eased_progress` map; it has to answer how fast it is moving, or
//! a curve interrupted mid-flight would hand the engine a velocity of zero and
//! the motion would visibly stall at the seam. [`CubicBezier::slope`] is that
//! answer, and it is analytic — `dy/dt = (dy/du)/(dx/du)`, both from the
//! bezier's own derivative — not a finite difference over two frames.

use super::value::AnimVector;

/// A cubic-bezier easing on the unit square, from `(0,0)` to `(1,1)`, with
/// two free control points.
///
/// The `x` coordinates are clamped into `[0, 1]` at construction — the CSS
/// definition requires it, and an `x` outside that range makes `x(u)`
/// non-monotone, which would make "the progress at time t" ambiguous. `y` is
/// deliberately unclamped: an overshooting ease (`y > 1`) is a legitimate,
/// widely used shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezier {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl Default for CubicBezier {
    fn default() -> Self {
        Self::LINEAR
    }
}

impl CubicBezier {
    /// `cubic-bezier(0, 0, 1, 1)` — constant rate.
    pub const LINEAR: Self = Self {
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };
    /// `cubic-bezier(0.25, 0.1, 0.25, 1)` — the CSS `ease` default.
    pub const EASE: Self = Self {
        x1: 0.25,
        y1: 0.1,
        x2: 0.25,
        y2: 1.0,
    };
    /// `cubic-bezier(0.42, 0, 1, 1)`.
    pub const EASE_IN: Self = Self {
        x1: 0.42,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };
    /// `cubic-bezier(0, 0, 0.58, 1)`.
    pub const EASE_OUT: Self = Self {
        x1: 0.0,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };
    /// `cubic-bezier(0.42, 0, 0.58, 1)`.
    pub const EASE_IN_OUT: Self = Self {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    };

    /// A curve from two control points, with the `x` coordinates clamped into
    /// `[0, 1]`.
    #[must_use]
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x1: x1.clamp(0.0, 1.0),
            y1,
            x2: x2.clamp(0.0, 1.0),
            y2,
        }
    }

    /// The preset `name` refers to, or `None`.
    ///
    /// The five names are exactly the ones `contracts/animation.md` lists. An
    /// unknown name is `None` rather than a silent fall back to `linear`: a
    /// misspelt easing that quietly becomes linear is a bug nobody ever finds.
    #[must_use]
    pub fn preset(name: &str) -> Option<Self> {
        match name {
            "linear" => Some(Self::LINEAR),
            "ease" => Some(Self::EASE),
            "ease-in" => Some(Self::EASE_IN),
            "ease-out" => Some(Self::EASE_OUT),
            "ease-in-out" => Some(Self::EASE_IN_OUT),
            _ => None,
        }
    }

    /// Every preset name, sorted — for an error message that names the legal
    /// set instead of only rejecting.
    #[must_use]
    pub fn preset_names() -> [&'static str; 5] {
        ["ease", "ease-in", "ease-in-out", "ease-out", "linear"]
    }

    /// The control points, `(x1, y1, x2, y2)`.
    #[must_use]
    pub fn control_points(self) -> (f64, f64, f64, f64) {
        (self.x1, self.y1, self.x2, self.y2)
    }

    /// The eased progress at normalized time `t ∈ [0, 1]`.
    ///
    /// Outside the range the endpoints hold: before a curve starts it is at
    /// its start, after it ends it is exactly at `1`. "Exactly" matters —
    /// this is the value the engine writes on the settling frame, and the
    /// contract requires the settled placement to carry the exact target.
    #[must_use]
    pub fn eval(self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        self.bezier_y(self.solve_u(t))
    }

    /// `d(eased)/dt` at normalized time `t`, in progress per unit of
    /// normalized time.
    ///
    /// Multiply by `1/duration` to get progress per second, and by the
    /// travelled distance to get a velocity in the property's own units.
    ///
    /// At the two ends the parametric derivative can be `0/0` — for
    /// `ease-in`, `x'(0) = y'(0) = 0` — so a one-sided finite difference over
    /// a millisecond of normalized time is used there. That is a bounded,
    /// stated approximation at exactly two points, not a general fallback.
    #[must_use]
    pub fn slope(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        let u = self.solve_u(t);
        let dx = self.bezier_dx(u);
        if dx.abs() < 1e-9 {
            const H: f64 = 1e-3;
            let (lo, hi) = ((t - H).max(0.0), (t + H).min(1.0));
            if hi <= lo {
                return 0.0;
            }
            return (self.eval(hi) - self.eval(lo)) / (hi - lo);
        }
        self.bezier_dy(u) / dx
    }

    /// `x(u)` for the bezier's first coordinate. `P0 = 0`, `P3 = 1`.
    fn bezier_x(self, u: f64) -> f64 {
        cubic(0.0, self.x1, self.x2, 1.0, u)
    }

    fn bezier_y(self, u: f64) -> f64 {
        cubic(0.0, self.y1, self.y2, 1.0, u)
    }

    fn bezier_dx(self, u: f64) -> f64 {
        cubic_prime(0.0, self.x1, self.x2, 1.0, u)
    }

    fn bezier_dy(self, u: f64) -> f64 {
        cubic_prime(0.0, self.y1, self.y2, 1.0, u)
    }

    /// The bezier parameter `u` whose `x(u)` is `t`.
    ///
    /// Newton first, because `x(u)` is monotone on `[0, 1]` for clamped
    /// control points and Newton converges in three or four steps there.
    /// Bisection is the fallback, not the backup plan: Newton is abandoned
    /// the moment it steps outside `[0, 1]` or stalls on a flat derivative
    /// (`ease-in`'s `x'(0) = 0`), and bisection on a monotone function cannot
    /// fail to bracket. The two together are total, so this never returns a
    /// wrong `u` quietly and never loops forever.
    fn solve_u(self, t: f64) -> f64 {
        const TOLERANCE: f64 = 1e-9;
        let mut u = t;
        for _ in 0..8 {
            let err = self.bezier_x(u) - t;
            if err.abs() < TOLERANCE {
                return u;
            }
            let d = self.bezier_dx(u);
            if d.abs() < 1e-9 {
                break;
            }
            let next = u - err / d;
            if !(0.0..=1.0).contains(&next) {
                break;
            }
            u = next;
        }
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..60 {
            let mid = f64::midpoint(lo, hi);
            if self.bezier_x(mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
            if hi - lo < TOLERANCE {
                break;
            }
        }
        f64::midpoint(lo, hi)
    }
}

fn cubic(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let m = 1.0 - u;
    m * m * m * p0 + 3.0 * m * m * u * p1 + 3.0 * m * u * u * p2 + u * u * u * p3
}

fn cubic_prime(p0: f64, p1: f64, p2: f64, p3: f64, u: f64) -> f64 {
    let m = 1.0 - u;
    3.0 * m * m * (p1 - p0) + 6.0 * m * u * (p2 - p1) + 3.0 * u * u * (p3 - p2)
}

/// A timed easing: how long, and along what shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    duration: f64,
    easing: CubicBezier,
}

impl Curve {
    /// A curve running for `duration` seconds along `easing`.
    ///
    /// # Errors
    /// A non-positive or non-finite duration, as a message naming the value.
    /// Zero would make every evaluation a division by zero and every
    /// transition instantaneous — which is a real thing to want, and is
    /// spelled by not declaring a transition at all.
    pub fn new(duration: f64, easing: CubicBezier) -> Result<Self, String> {
        if !duration.is_finite() || duration <= 0.0 {
            return Err(format!(
                "a curve's duration must be finite and greater than 0 seconds, got {duration}"
            ));
        }
        Ok(Self { duration, easing })
    }

    /// Seconds this curve runs for.
    #[must_use]
    pub fn duration(self) -> f64 {
        self.duration
    }

    /// The easing shape.
    #[must_use]
    pub fn easing(self) -> CubicBezier {
        self.easing
    }

    /// Position and velocity at `t` seconds along a run from `from` to `to`.
    ///
    /// Velocity is in the property's units per second: the curve's slope in
    /// normalized time, divided by the duration, times the travelled vector.
    /// Past the end the position is exactly `to` and the velocity is exactly
    /// zero — the settled state, with no residue.
    ///
    /// # Panics
    /// If `from` and `to` have different lengths, or `t` is negative.
    #[must_use]
    pub fn evaluate(self, from: AnimVector, to: AnimVector, t: f64) -> (AnimVector, AnimVector) {
        assert!(
            t.is_finite() && t >= 0.0,
            "a curve is evaluated forward from its own start; t = {t}"
        );
        if t >= self.duration {
            return (to, AnimVector::zeros(to.len()));
        }
        let normalized = t / self.duration;
        let eased = self.easing.eval(normalized);
        let rate = self.easing.slope(normalized) / self.duration;
        let travelled = to - from;
        (from.lerp(to, eased), travelled.scale(rate))
    }
}

/// One point on a keyframe track: when, what value, and how to get there from
/// the point before it.
///
/// The easing belongs to the *incoming* segment, which is what
/// `contracts/animation.md` means by "(time, value, easing-in)": a track is
/// read as a sequence of arrivals, so the first keyframe's easing is unused
/// and stating that here is cheaper than a caller wondering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    /// Seconds from the track's start.
    pub time: f64,
    /// The value at that moment.
    pub value: AnimVector,
    /// How the segment ending at this keyframe is eased.
    pub easing_in: CubicBezier,
}

/// A per-property sequence of keyframes.
///
/// Sorted and non-empty by construction; both are checked once, at build
/// time, so [`KeyframeTrack::evaluate`] has no ordering branch and cannot be
/// handed an out-of-order track by a caller who built one by hand.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyframeTrack {
    frames: Vec<Keyframe>,
}

impl KeyframeTrack {
    /// A track from keyframes in ascending time.
    ///
    /// # Errors
    /// An empty track, a track whose keyframes are not strictly ascending in
    /// time, or a track mixing keyframes of different component widths — each
    /// as a message naming what was wrong. The width check is the one that
    /// matters most: two widths in one track means two properties in one
    /// track, and the interpolation would silently animate whichever came
    /// first.
    pub fn new(frames: Vec<Keyframe>) -> Result<Self, String> {
        let Some(first) = frames.first() else {
            return Err("a keyframe track needs at least one keyframe".into());
        };
        let width = first.value.len();
        for (i, pair) in frames.windows(2).enumerate() {
            if pair[1].time <= pair[0].time {
                return Err(format!(
                    "keyframe {} is at t={} which is not after keyframe {} at t={}",
                    i + 1,
                    pair[1].time,
                    i,
                    pair[0].time
                ));
            }
        }
        for (i, frame) in frames.iter().enumerate() {
            if !frame.time.is_finite() || frame.time < 0.0 {
                return Err(format!(
                    "keyframe {i} is at t={}, which is not a time",
                    frame.time
                ));
            }
            if frame.value.len() != width {
                return Err(format!(
                    "keyframe {i} has {} components but the track's first has {width} — \
                     one track drives one property",
                    frame.value.len()
                ));
            }
        }
        Ok(Self { frames })
    }

    /// The keyframes, in time order.
    #[must_use]
    pub fn frames(&self) -> &[Keyframe] {
        &self.frames
    }

    /// When the last keyframe lands.
    #[must_use]
    pub fn duration(&self) -> f64 {
        self.frames.last().map_or(0.0, |f| f.time)
    }

    /// How many components this track drives.
    #[must_use]
    pub fn width(&self) -> usize {
        self.frames[0].value.len()
    }

    /// Value and velocity at `t` seconds.
    ///
    /// Before the first keyframe the first value holds; after the last, the
    /// last value holds and the velocity is exactly zero.
    ///
    /// # Panics
    /// If `t` is negative or not finite.
    #[must_use]
    pub fn evaluate(&self, t: f64) -> (AnimVector, AnimVector) {
        assert!(
            t.is_finite() && t >= 0.0,
            "a keyframe track is evaluated forward from its own start; t = {t}"
        );
        let zero = AnimVector::zeros(self.width());
        let first = &self.frames[0];
        if t <= first.time {
            return (first.value, zero);
        }
        let last = &self.frames[self.frames.len() - 1];
        if t >= last.time {
            return (last.value, zero);
        }
        let idx = self
            .frames
            .partition_point(|f| f.time <= t)
            .clamp(1, self.frames.len() - 1);
        let (a, b) = (&self.frames[idx - 1], &self.frames[idx]);
        let span = b.time - a.time;
        let normalized = (t - a.time) / span;
        let eased = b.easing_in.eval(normalized);
        let rate = b.easing_in.slope(normalized) / span;
        (
            a.value.lerp(b.value, eased),
            (b.value - a.value).scale(rate),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{CubicBezier, Curve, Keyframe, KeyframeTrack};
    use crate::anim::value::AnimVector;

    /// The control points the contract names, transcribed from CSS and
    /// confirmed identical to SwiftUI's `UnitCurve`
    /// (`research-layout-animation.md` §7).
    #[test]
    fn the_named_presets_carry_their_canonical_control_points() {
        assert_eq!(
            CubicBezier::preset("linear").unwrap().control_points(),
            (0.0, 0.0, 1.0, 1.0)
        );
        assert_eq!(
            CubicBezier::preset("ease").unwrap().control_points(),
            (0.25, 0.1, 0.25, 1.0)
        );
        assert_eq!(
            CubicBezier::preset("ease-in").unwrap().control_points(),
            (0.42, 0.0, 1.0, 1.0)
        );
        assert_eq!(
            CubicBezier::preset("ease-out").unwrap().control_points(),
            (0.0, 0.0, 0.58, 1.0)
        );
        assert_eq!(
            CubicBezier::preset("ease-in-out").unwrap().control_points(),
            (0.42, 0.0, 0.58, 1.0)
        );
    }

    /// A misspelt easing must not become linear behind the author's back.
    #[test]
    fn an_unknown_preset_is_none_not_linear() {
        assert_eq!(CubicBezier::preset("ease-in-ouy"), None);
        assert_eq!(CubicBezier::preset("easeIn"), None);
        assert_eq!(CubicBezier::preset_names().len(), 5);
    }

    /// `linear` is the identity, to solver tolerance, everywhere — the
    /// cheapest check that `solve_u` inverts `bezier_x` correctly.
    #[test]
    fn linear_is_the_identity() {
        for i in 0..=100 {
            let t = f64::from(i) / 100.0;
            assert!((CubicBezier::LINEAR.eval(t) - t).abs() < 1e-8, "at {t}");
        }
    }

    #[test]
    fn every_preset_pins_its_endpoints_exactly() {
        for name in CubicBezier::preset_names() {
            let c = CubicBezier::preset(name).unwrap();
            assert_eq!(c.eval(0.0), 0.0, "{name} at 0");
            assert_eq!(c.eval(1.0), 1.0, "{name} at 1");
            // And outside the range the endpoints hold rather than
            // extrapolating off the curve.
            assert_eq!(c.eval(-0.5), 0.0, "{name} before start");
            assert_eq!(c.eval(1.5), 1.0, "{name} after end");
        }
    }

    /// `ease-in` starts slow and `ease-out` starts fast — the property that
    /// distinguishes them, checked against each other rather than against a
    /// magic number.
    #[test]
    fn ease_in_and_ease_out_are_mirror_images() {
        for i in 1..100 {
            let t = f64::from(i) / 100.0;
            let ease_in = CubicBezier::EASE_IN.eval(t);
            let ease_out = CubicBezier::EASE_OUT.eval(t);
            assert!(ease_in < t, "ease-in at {t} is {ease_in}");
            assert!(ease_out > t, "ease-out at {t} is {ease_out}");
            // Mirror: easeOut(t) == 1 - easeIn(1 - t) for these control points.
            let mirrored = 1.0 - CubicBezier::EASE_IN.eval(1.0 - t);
            assert!((ease_out - mirrored).abs() < 1e-6, "at {t}");
        }
    }

    /// The analytic slope is what a retarget carries. Checked against a
    /// central difference of `eval`, which is an independent computation
    /// (bisection-solved `u` at two nearby points) rather than a restatement.
    #[test]
    fn the_analytic_slope_matches_a_finite_difference() {
        for name in CubicBezier::preset_names() {
            let c = CubicBezier::preset(name).unwrap();
            for i in 5..95 {
                let t = f64::from(i) / 100.0;
                let h = 1e-5;
                let numeric = (c.eval(t + h) - c.eval(t - h)) / (2.0 * h);
                let analytic = c.slope(t);
                assert!(
                    (numeric - analytic).abs() < 1e-3,
                    "{name} at {t}: analytic {analytic}, numeric {numeric}"
                );
            }
        }
    }

    /// `ease-in`'s derivative is 0/0 at t=0 in parametric form. The one-sided
    /// difference must still answer a finite, non-negative number rather than
    /// a NaN that would poison a retarget.
    #[test]
    fn a_degenerate_endpoint_slope_is_finite() {
        let s = CubicBezier::EASE_IN.slope(0.0);
        assert!(s.is_finite() && s >= 0.0, "slope {s}");
        assert!(CubicBezier::EASE_OUT.slope(1.0).is_finite());
    }

    #[test]
    fn a_curve_lands_exactly_on_its_target_and_stops() {
        let curve = Curve::new(0.4, CubicBezier::EASE_IN_OUT).unwrap();
        let from = AnimVector::new([0.0, 0.0, 0.0, 0.0], 2);
        let to = AnimVector::new([100.0, -50.0, 0.0, 0.0], 2);
        let (pos, vel) = curve.evaluate(from, to, 0.4);
        assert_eq!(pos, to);
        assert_eq!(vel.as_slice(), &[0.0, 0.0]);
        let (pos_after, _) = curve.evaluate(from, to, 9.0);
        assert_eq!(pos_after, to);
    }

    /// Mid-flight a curve is moving, and its velocity points the right way
    /// on each component independently.
    #[test]
    fn a_curve_reports_a_velocity_a_retarget_can_carry() {
        let curve = Curve::new(0.5, CubicBezier::LINEAR).unwrap();
        let from = AnimVector::new([0.0, 100.0, 0.0, 0.0], 2);
        let to = AnimVector::new([50.0, 0.0, 0.0, 0.0], 2);
        let (_, vel) = curve.evaluate(from, to, 0.25);
        // Linear over 0.5 s: 50 units in 0.5 s is 100/s; -100 units is -200/s.
        assert!((vel.get(0) - 100.0).abs() < 1e-6, "{vel:?}");
        assert!((vel.get(1) + 200.0).abs() < 1e-6, "{vel:?}");
    }

    #[test]
    fn a_zero_duration_curve_is_refused_by_name() {
        let err = Curve::new(0.0, CubicBezier::LINEAR).unwrap_err();
        assert!(err.contains("greater than 0 seconds"), "{err}");
        assert!(Curve::new(f64::NAN, CubicBezier::LINEAR).is_err());
    }

    fn kf(time: f64, v: f64) -> Keyframe {
        Keyframe {
            time,
            value: AnimVector::scalar(v),
            easing_in: CubicBezier::LINEAR,
        }
    }

    #[test]
    fn a_track_interpolates_between_its_own_segments() {
        let track = KeyframeTrack::new(vec![kf(0.0, 0.0), kf(1.0, 10.0), kf(2.0, 0.0)]).unwrap();
        assert_eq!(track.evaluate(0.0).0.get(0), 0.0);
        assert!((track.evaluate(0.5).0.get(0) - 5.0).abs() < 1e-9);
        assert!((track.evaluate(1.0).0.get(0) - 10.0).abs() < 1e-9);
        assert!((track.evaluate(1.5).0.get(0) - 5.0).abs() < 1e-9);
        assert_eq!(track.evaluate(2.0).0.get(0), 0.0);
        assert_eq!(track.duration(), 2.0);
    }

    /// The two ends are at rest and the middle is not — the same
    /// position-plus-velocity shape the spring answers with, so the engine
    /// treats the two timing kinds uniformly.
    #[test]
    fn a_track_is_at_rest_outside_its_own_span() {
        let track = KeyframeTrack::new(vec![kf(0.0, 0.0), kf(1.0, 10.0)]).unwrap();
        assert_eq!(track.evaluate(0.0).1.get(0), 0.0);
        assert_eq!(track.evaluate(5.0).1.get(0), 0.0);
        assert_eq!(track.evaluate(5.0).0.get(0), 10.0);
        assert!((track.evaluate(0.5).1.get(0) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn a_segment_uses_the_easing_of_the_keyframe_it_arrives_at() {
        let track = KeyframeTrack::new(vec![
            kf(0.0, 0.0),
            Keyframe {
                time: 1.0,
                value: AnimVector::scalar(10.0),
                easing_in: CubicBezier::EASE_IN,
            },
        ])
        .unwrap();
        // ease-in is behind linear everywhere in the open interval.
        assert!(track.evaluate(0.5).0.get(0) < 5.0);
    }

    #[test]
    fn a_malformed_track_is_refused_by_name() {
        assert!(
            KeyframeTrack::new(vec![])
                .unwrap_err()
                .contains("at least one")
        );
        let out_of_order = KeyframeTrack::new(vec![kf(1.0, 0.0), kf(0.5, 1.0)]).unwrap_err();
        assert!(out_of_order.contains("not after"), "{out_of_order}");
        let mixed = KeyframeTrack::new(vec![
            kf(0.0, 0.0),
            Keyframe {
                time: 1.0,
                value: AnimVector::zeros(4),
                easing_in: CubicBezier::LINEAR,
            },
        ])
        .unwrap_err();
        assert!(mixed.contains("one track drives one property"), "{mixed}");
    }
}
