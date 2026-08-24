//! Animatable values: fixed-length vectors, one interpolation rule per
//! property type, and the rule for the changes that are not animatable at all.
//!
//! `contracts/animation.md`: *"An animatable property interpolates as a
//! fixed-length vector (positions, sizes, opacity, color in a defined
//! interpolation space) — one interpolation rule per property type (FR-029).
//! Non-animatable changes land discretely at transition start or end by
//! declared rule."*
//!
//! Everything the spring and the curve solvers touch is an [`AnimVector`], so
//! neither of them knows what a rect or an opacity is. That is what keeps the
//! solver code single-copy: adding a property type adds a [`PropertyKind`] and
//! an [`Animatable`] impl, never a second copy of the closed forms.
//!
//! # Why the colour space is fixed here and not chosen per call
//!
//! [`crate::token::ColorValue`] is already linear light with straight alpha
//! (its module doc carries the argument). Interpolating in that space is a
//! physical average of light; interpolating gamma-encoded bytes is an average
//! of a perceptual compression, which darkens a red→green ramp visibly. Petra
//! has exactly one colour representation, so the "defined interpolation space"
//! the contract asks for is defined once, here, rather than being a per-
//! transition option nobody would set correctly.

use crate::geom::{Point, Size};
use crate::token::ColorValue;

/// The widest animatable value this engine carries, in components.
///
/// Four: a rect (`x, y, w, h`) and a colour (`r, g, b, a`) are both four, and
/// nothing on a [`crate::frame::Placement`] is wider. A fixed array rather
/// than a `Vec` because a transition is evaluated once per property per frame
/// and an allocation per evaluation would be the whole cost of the engine.
pub const MAX_COMPONENTS: usize = 4;

/// A fixed-length animatable vector.
///
/// Two vectors of different lengths never meet: every operation that takes
/// two asserts they agree, because a length mismatch means a property was
/// mixed with a different property and silently truncating one of them would
/// animate the wrong thing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimVector {
    comps: [f64; MAX_COMPONENTS],
    len: usize,
}

impl AnimVector {
    /// A vector from the first `n` components of `comps`.
    ///
    /// # Panics
    /// If `n` is zero or greater than [`MAX_COMPONENTS`]. A zero-length
    /// animatable value is not a value, and a longer one has no property that
    /// could hold it; both are caller bugs rather than states to interpolate.
    #[must_use]
    pub fn new(comps: [f64; MAX_COMPONENTS], len: usize) -> Self {
        assert!(
            len > 0 && len <= MAX_COMPONENTS,
            "an animatable vector has 1..={MAX_COMPONENTS} components, not {len}"
        );
        Self { comps, len }
    }

    /// A one-component vector.
    #[must_use]
    pub fn scalar(v: f64) -> Self {
        Self::new([v, 0.0, 0.0, 0.0], 1)
    }

    /// A vector of `len` zeros.
    #[must_use]
    pub fn zeros(len: usize) -> Self {
        Self::new([0.0; MAX_COMPONENTS], len)
    }

    /// The live components.
    #[must_use]
    pub fn as_slice(&self) -> &[f64] {
        &self.comps[..self.len]
    }

    /// How many components are live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Never true — [`AnimVector::new`] refuses a zero length. Present because
    /// clippy asks for it beside `len`, and answering honestly is cheaper than
    /// an allow.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Component `i`.
    ///
    /// # Panics
    /// If `i` is past the live length.
    #[must_use]
    pub fn get(&self, i: usize) -> f64 {
        assert!(
            i < self.len,
            "component {i} of a {}-component vector",
            self.len
        );
        self.comps[i]
    }

    /// Overwrite component `i`.
    ///
    /// # Panics
    /// If `i` is past the live length.
    pub fn set(&mut self, i: usize, v: f64) {
        assert!(
            i < self.len,
            "component {i} of a {}-component vector",
            self.len
        );
        self.comps[i] = v;
    }

    /// `self * k`.
    #[must_use]
    pub fn scale(self, k: f64) -> Self {
        let mut out = self;
        for i in 0..self.len {
            out.comps[i] = self.comps[i] * k;
        }
        out
    }

    /// The straight componentwise interpolation, unclamped.
    ///
    /// Unclamped on purpose: an underdamped spring overshoots, and clamping
    /// here would silently turn a bouncy transition into a critically damped
    /// one at exactly the frames the bounce is visible.
    ///
    /// # Panics
    /// If the lengths differ.
    #[must_use]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        self.zip_with(other, |a, b| a + (b - a) * t)
    }

    /// The largest absolute difference between any two matching components.
    ///
    /// The settle test is per-component (`contracts/animation.md`: *"|x −
    /// target| ... under declared thresholds"*), so the aggregate that decides
    /// it is the max, not the Euclidean norm: a rect one thousandth off in
    /// `x` and half a unit off in `h` is not settled, and an L2 norm scaled by
    /// four components could say it was.
    ///
    /// # Panics
    /// If the lengths differ.
    #[must_use]
    pub fn chebyshev(self, other: Self) -> f64 {
        self.assert_same_len(other);
        (0..self.len)
            .map(|i| (self.comps[i] - other.comps[i]).abs())
            .fold(0.0_f64, f64::max)
    }

    /// The largest absolute component.
    #[must_use]
    pub fn amplitude(self) -> f64 {
        self.as_slice()
            .iter()
            .copied()
            .map(f64::abs)
            .fold(0.0, f64::max)
    }

    /// Whether every component is finite.
    ///
    /// A trajectory that produced a NaN or an infinity is a solver bug, and
    /// the engine refuses to write one into a placement rather than shipping a
    /// frame whose digest is a hash of NaN.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.as_slice().iter().copied().all(f64::is_finite)
    }

    fn assert_same_len(self, other: Self) {
        assert_eq!(
            self.len, other.len,
            "two animatable vectors of different lengths met: {} vs {} — a \
             property was mixed with a different property",
            self.len, other.len
        );
    }

    fn zip_with(self, other: Self, f: impl Fn(f64, f64) -> f64) -> Self {
        self.assert_same_len(other);
        let mut out = self;
        for i in 0..self.len {
            out.comps[i] = f(self.comps[i], other.comps[i]);
        }
        out
    }
}

/// Componentwise addition.
///
/// # Panics
/// If the lengths differ — see [`AnimVector`]'s own doc for why that is a
/// refusal rather than a truncation.
impl std::ops::Add for AnimVector {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.zip_with(other, |a, b| a + b)
    }
}

/// Componentwise subtraction.
///
/// # Panics
/// If the lengths differ.
impl std::ops::Sub for AnimVector {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self.zip_with(other, |a, b| a - b)
    }
}

/// Which property a transition drives, and therefore which interpolation rule
/// and which settle thresholds it uses.
///
/// The set is closed on purpose. Every variant here names something a
/// [`crate::frame::Placement`] carries or that [`crate::token::ColorValue`]
/// resolves to, so no variant can be declared and then quietly do nothing —
/// except [`PropertyKind::Color`], whose position is stated in this module's
/// `color_has_no_placement_field_yet` test rather than left to be discovered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropertyKind {
    /// A rect's origin, in logical units. Two components.
    Position,
    /// A rect's extent, in logical units. Two components.
    Size,
    /// Cumulative opacity in `[0, 1]`. One component.
    Opacity,
    /// A colour in linear light with straight alpha. Four components.
    Color,
}

impl PropertyKind {
    /// How many components this property interpolates as.
    #[must_use]
    pub fn components(self) -> usize {
        match self {
            Self::Position | Self::Size => 2,
            Self::Opacity => 1,
            Self::Color => 4,
        }
    }

    /// Whether this property is *movement* in the reduced-motion sense.
    ///
    /// `contracts/animation.md`: *"movement (position/size) transitions
    /// complete instantly; opacity/dissolve transitions MAY still run (the
    /// accepted substitute)"*. Colour is a dissolve, not movement, so it sits
    /// with opacity — matching MDN's own reduced-motion example, which keeps
    /// the opacity animation and drops only the scale one.
    #[must_use]
    pub fn is_movement(self) -> bool {
        matches!(self, Self::Position | Self::Size)
    }

    /// The settle thresholds for this property: `(position, velocity)`.
    ///
    /// Per-property because the units differ by three orders of magnitude. A
    /// hundredth of a logical unit is invisible on any display scale this
    /// engine supports; a hundredth of an opacity step is not — 1/100 of full
    /// opacity is a visible step on a large flat fill — so opacity and colour
    /// settle four times tighter. These are engineering defaults, and the
    /// research note says so directly: no external standard publishes a
    /// number here (`research-layout-animation.md` §6, "Settling thresholds").
    #[must_use]
    pub fn thresholds(self) -> Thresholds {
        match self {
            Self::Position | Self::Size => Thresholds {
                value: 0.01,
                velocity: 0.05,
            },
            Self::Opacity | Self::Color => Thresholds {
                value: 0.0025,
                velocity: 0.0125,
            },
        }
    }
}

/// The pair of epsilons a property settles under.
///
/// Both must hold at once. A spring passing through its target at speed is at
/// `|x − target| = 0` and is emphatically not settled; testing position alone
/// would snap it mid-flight and lose the whole overshoot
/// (`research-layout-animation.md` §6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Largest per-component distance from target that still counts as there.
    pub value: f64,
    /// Largest per-component speed that still counts as at rest, per second.
    pub velocity: f64,
}

impl Thresholds {
    /// Whether `value` is at `target` and `velocity` is at rest, both at once.
    #[must_use]
    pub fn settled(self, value: AnimVector, target: AnimVector, velocity: AnimVector) -> bool {
        value.chebyshev(target) <= self.value && velocity.amplitude() <= self.velocity
    }
}

/// A value that can be driven by a transition.
///
/// The round trip must be exact for the values this engine actually writes:
/// `from_vector(to_vector(v)) == v`. That is what makes the settle rule
/// ("snap exactly to target") produce a byte-identical placement rather than a
/// placement one `f32` ulp away, which the digest would see.
pub trait Animatable: Copy {
    /// Which property this value is.
    const KIND: PropertyKind;

    /// This value as a vector.
    fn to_vector(self) -> AnimVector;

    /// The value a vector describes.
    fn from_vector(v: AnimVector) -> Self;
}

impl Animatable for Point {
    const KIND: PropertyKind = PropertyKind::Position;

    fn to_vector(self) -> AnimVector {
        AnimVector::new([f64::from(self.x), f64::from(self.y), 0.0, 0.0], 2)
    }

    fn from_vector(v: AnimVector) -> Self {
        Self {
            x: v.get(0) as f32,
            y: v.get(1) as f32,
        }
    }
}

impl Animatable for Size {
    const KIND: PropertyKind = PropertyKind::Size;

    fn to_vector(self) -> AnimVector {
        AnimVector::new([f64::from(self.w), f64::from(self.h), 0.0, 0.0], 2)
    }

    fn from_vector(v: AnimVector) -> Self {
        Self {
            w: v.get(0) as f32,
            h: v.get(1) as f32,
        }
    }
}

/// Opacity, wrapped so the trait can name a property for a bare `f32`.
///
/// A newtype rather than `impl Animatable for f32`: `f32` is also what a
/// spacing token and a corner radius are, and a blanket impl would let any of
/// them be driven with opacity's settle thresholds by accident.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opacity(pub f32);

impl Animatable for Opacity {
    const KIND: PropertyKind = PropertyKind::Opacity;

    fn to_vector(self) -> AnimVector {
        AnimVector::scalar(f64::from(self.0))
    }

    fn from_vector(v: AnimVector) -> Self {
        Self(v.get(0) as f32)
    }
}

impl Animatable for ColorValue {
    const KIND: PropertyKind = PropertyKind::Color;

    fn to_vector(self) -> AnimVector {
        AnimVector::new(
            [
                f64::from(self.r),
                f64::from(self.g),
                f64::from(self.b),
                f64::from(self.a),
            ],
            4,
        )
    }

    fn from_vector(v: AnimVector) -> Self {
        Self {
            r: v.get(0) as f32,
            g: v.get(1) as f32,
            b: v.get(2) as f32,
            a: v.get(3) as f32,
        }
    }
}

/// When a change that cannot be interpolated takes effect.
///
/// `contracts/animation.md`: *"Non-animatable changes land discretely at
/// transition start or end by declared rule."* A node's text, its truncation
/// flag and its custom-painter name are all in this class: there is no
/// halfway between two strings, so the only question is which side of the
/// motion the swap happens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiscreteRule {
    /// Swap on the frame the transition starts. The right default: a label
    /// that changes as a panel slides in should already read correctly while
    /// it slides, not flip at the end.
    #[default]
    AtStart,
    /// Swap on the frame the transition settles.
    AtEnd,
}

impl DiscreteRule {
    /// Which of `from`/`to` is in force at progress `t ∈ [0, 1]`, where `t`
    /// is `0` on the starting frame and `1` on the settled frame.
    ///
    /// `AtStart` takes `to` from `t = 0` inclusive; `AtEnd` keeps `from`
    /// until `t = 1` inclusive. The two are exact complements, so a value
    /// declared under one rule is never in an undefined state under the
    /// other.
    pub fn resolve<T>(self, from: T, to: T, t: f64) -> T {
        match self {
            Self::AtStart => to,
            Self::AtEnd if t >= 1.0 => to,
            Self::AtEnd => from,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimVector, Animatable, ColorValue, DiscreteRule, Opacity, PropertyKind};
    use crate::geom::{Point, Size};

    #[test]
    fn lerp_is_componentwise_and_unclamped() {
        let a = AnimVector::new([0.0, 10.0, 0.0, 0.0], 2);
        let b = AnimVector::new([10.0, 0.0, 0.0, 0.0], 2);
        assert_eq!(a.lerp(b, 0.5).as_slice(), &[5.0, 5.0]);
        // Overshoot survives: a bouncy spring lives past t = 1.
        assert_eq!(a.lerp(b, 1.5).as_slice(), &[15.0, -5.0]);
    }

    #[test]
    fn chebyshev_is_the_worst_component_not_an_average() {
        let a = AnimVector::new([0.0, 0.0, 0.0, 0.0], 4);
        let b = AnimVector::new([0.0, 0.0, 0.0, 0.5], 4);
        assert!((a.chebyshev(b) - 0.5).abs() < 1e-12);
    }

    #[test]
    #[should_panic(expected = "different lengths")]
    fn mixing_two_property_widths_is_refused_not_truncated() {
        let _ = AnimVector::zeros(2) + AnimVector::zeros(4);
    }

    /// The round trip has to be exact, or "snap exactly to target" would land
    /// one ulp away and the digest would see it.
    #[test]
    fn every_animatable_round_trips_exactly() {
        let p = Point { x: 12.5, y: -3.25 };
        assert_eq!(Point::from_vector(p.to_vector()), p);
        let s = Size { w: 640.0, h: 481.5 };
        assert_eq!(Size::from_vector(s.to_vector()), s);
        let o = Opacity(0.37);
        assert_eq!(Opacity::from_vector(o.to_vector()), o);
        let c = ColorValue::from_srgb8(31, 97, 200, 128);
        assert_eq!(ColorValue::from_vector(c.to_vector()), c);
    }

    #[test]
    fn component_counts_match_the_vectors_produced() {
        assert_eq!(
            Point::KIND.components(),
            Point { x: 0.0, y: 0.0 }.to_vector().len()
        );
        assert_eq!(Size::KIND.components(), Size::ZERO.to_vector().len());
        assert_eq!(Opacity::KIND.components(), Opacity(1.0).to_vector().len());
        assert_eq!(
            ColorValue::KIND.components(),
            ColorValue::TRANSPARENT.to_vector().len()
        );
    }

    /// Colour interpolates in linear light, which is a different answer from
    /// interpolating the sRGB bytes — and the visibly correct one. Halfway
    /// between sRGB `#000` and sRGB `#fff` in linear light is 0.5 of the
    /// light, which encodes back to sRGB ~188, not 128.
    #[test]
    fn color_interpolates_in_linear_light() {
        let black = ColorValue::from_srgb8(0, 0, 0, 255);
        let white = ColorValue::from_srgb8(255, 255, 255, 255);
        let mid = ColorValue::from_vector(black.to_vector().lerp(white.to_vector(), 0.5));
        assert!(
            (mid.r - 0.5).abs() < 1e-6,
            "linear-light midpoint, got {}",
            mid.r
        );
        // The naive sRGB-byte midpoint would be this much light instead.
        let naive = ColorValue::from_srgb8(128, 128, 128, 255);
        assert!(naive.r < 0.25, "sRGB 128 is {} of the light", naive.r);
    }

    #[test]
    fn movement_is_position_and_size_only() {
        assert!(PropertyKind::Position.is_movement());
        assert!(PropertyKind::Size.is_movement());
        assert!(!PropertyKind::Opacity.is_movement());
        assert!(!PropertyKind::Color.is_movement());
    }

    /// Both epsilons must hold. A spring crossing its target at speed is at
    /// distance zero and is not settled.
    #[test]
    fn settling_needs_position_and_velocity_together() {
        let t = PropertyKind::Position.thresholds();
        let at_target = AnimVector::scalar(0.0);
        assert!(!t.settled(at_target, at_target, AnimVector::scalar(40.0)));
        assert!(!t.settled(AnimVector::scalar(9.0), at_target, AnimVector::scalar(0.0)));
        assert!(t.settled(at_target, at_target, AnimVector::scalar(0.0)));
    }

    #[test]
    fn opacity_settles_tighter_than_geometry() {
        assert!(
            PropertyKind::Opacity.thresholds().value < PropertyKind::Position.thresholds().value
        );
    }

    #[test]
    fn discrete_rules_are_exact_complements() {
        for t in [0.0, 0.25, 0.5, 0.999] {
            assert_eq!(DiscreteRule::AtStart.resolve("old", "new", t), "new");
            assert_eq!(DiscreteRule::AtEnd.resolve("old", "new", t), "old");
        }
        assert_eq!(DiscreteRule::AtStart.resolve("old", "new", 1.0), "new");
        assert_eq!(DiscreteRule::AtEnd.resolve("old", "new", 1.0), "new");
    }

    /// Stated, not hidden: `PropertyKind::Color` has a full interpolation
    /// rule and full test coverage here, and nothing drives it end to end
    /// yet, because no field on `crate::frame::Placement` carries a colour —
    /// token bindings reach the painter through `PaintContent` and resolve at
    /// paint time. The engine drives `Position`, `Size` and `Opacity`, and
    /// says so in one place
    /// ([`crate::anim::engine::DRIVEN_PROPERTIES`]). This test is what starts
    /// failing the moment that stops being true, so the gap cannot quietly
    /// close or quietly widen.
    #[test]
    fn color_is_interpolable_but_not_yet_driven_by_the_engine() {
        assert_eq!(
            crate::anim::engine::DRIVEN_PROPERTIES,
            [
                PropertyKind::Position,
                PropertyKind::Size,
                PropertyKind::Opacity
            ]
        );
        assert!(!crate::anim::engine::DRIVEN_PROPERTIES.contains(&PropertyKind::Color));
    }
}
