//! Resolved token values, one shape per [`TokenKind`].
//!
//! Colour uses a linear-light, straight-alpha `f32` representation
//! ([`ColorValue`]) rather than gamma-encoded sRGB. `contracts/animation.md`
//! requires "colour in a defined interpolation space", and linear light is
//! that space: interpolating two gamma-encoded sRGB colours in equal steps
//! produces a visibly duller, muddier midpoint than interpolating the same
//! colours in linear light (the classic "red-to-green averages to brown"
//! artefact — gamma encoding is a perceptual compression, and lerping
//! through it lerps the compressed numbers, not the light). Linear light is
//! the space in which a straight per-channel lerp is, physically, a
//! straight line. Authors still write colours as sRGB
//! ([`ColorValue::from_srgb8`]); conversion happens once, at construction,
//! so storage and interpolation never have to think about gamma again.

use serde::{Deserialize, Serialize};

/// Which value shape a token resolves to. Every [`crate::token::vocabulary::DesignToken`]
/// declares one; every [`TokenValue`] carries one back via [`TokenValue::kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TokenKind {
    /// A colour, stored linear-light with straight alpha.
    Color,
    /// A logical-unit extent (`geom::Size`'s units, before device scale).
    Spacing,
    /// A text style: size, line height, weight.
    Typography,
    /// A transition timing: duration and easing curve.
    Motion,
    /// A geometric parameter of a shape, e.g. a corner radius.
    Shape,
}

/// A colour in linear light, straight (non-premultiplied) alpha, each
/// channel `f32` in `[0.0, 1.0]`. See the module doc for why linear light.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorValue {
    /// Linear-light red.
    pub r: f32,
    /// Linear-light green.
    pub g: f32,
    /// Linear-light blue.
    pub b: f32,
    /// Straight alpha (not premultiplied into r/g/b).
    pub a: f32,
}

impl ColorValue {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Build a colour from 8-bit sRGB-encoded channels plus straight alpha,
    /// converting r/g/b to linear light. Alpha is not gamma-encoded in
    /// sRGB, so it passes straight through.
    #[must_use]
    pub fn from_srgb8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: srgb_to_linear(f32::from(r) / 255.0),
            g: srgb_to_linear(f32::from(g) / 255.0),
            b: srgb_to_linear(f32::from(b) / 255.0),
            a: f32::from(a) / 255.0,
        }
    }

    /// Linearly interpolate toward `other` by `t` (unclamped: callers doing
    /// spring overshoot want values outside `[0, 1]`).
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: lerp(self.r, other.r, t),
            g: lerp(self.g, other.g, t),
            b: lerp(self.b, other.b, t),
            a: lerp(self.a, other.a, t),
        }
    }
}

/// The IEC 61966-2-1 sRGB electro-optical transfer function: encoded
/// `[0, 1]` in, linear-light `[0, 1]` out.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A text style.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyValue {
    /// Font size in logical units.
    pub size: f32,
    /// Line height in logical units.
    pub line_height: f32,
    /// Font weight class.
    pub weight: TypographyWeight,
}

/// Font weight, as the small closed set the engine distinguishes rather
/// than a raw 100-900 number: a token names a role ("this heading is
/// `bold`"), not a font-family-specific numeric weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypographyWeight {
    /// Body-text weight.
    Regular,
    /// Emphasis weight between regular and bold.
    Medium,
    /// Heading/emphasis weight.
    Bold,
}

/// A transition timing: how long, and along which curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MotionValue {
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Easing curve.
    pub easing: MotionEasing,
}

/// A named easing curve. `contracts/animation.md`'s spring/keyframe system
/// is the general-purpose animator; this is the small fixed vocabulary a
/// design token can name without authoring a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MotionEasing {
    /// Constant rate.
    Linear,
    /// Slow start, fast finish.
    EaseIn,
    /// Fast start, slow finish.
    EaseOut,
    /// Slow start and finish, fast middle.
    EaseInOut,
}

/// A geometric parameter of a shape. Extended with more fields (border
/// width, etc.) as the vocabulary grows; a single corner radius is what the
/// shipped vocabulary needs today.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeValue {
    /// Corner radius in logical units.
    pub corner_radius: f32,
}

/// A resolved token value. Exactly one variant per [`TokenKind`]; the two
/// enums are kept in lock-step by [`TokenValue::kind`], which
/// [`crate::token::theme::Theme::build`] uses to refuse a value assigned at
/// the wrong kind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum TokenValue {
    /// A colour.
    Color(ColorValue),
    /// A spacing extent, in logical units.
    Spacing(f32),
    /// A text style.
    Typography(TypographyValue),
    /// A transition timing.
    Motion(MotionValue),
    /// A shape parameter.
    Shape(ShapeValue),
}

impl TokenValue {
    /// Which [`TokenKind`] this value resolves.
    #[must_use]
    pub fn kind(&self) -> TokenKind {
        match self {
            Self::Color(_) => TokenKind::Color,
            Self::Spacing(_) => TokenKind::Spacing,
            Self::Typography(_) => TokenKind::Typography,
            Self::Motion(_) => TokenKind::Motion,
            Self::Shape(_) => TokenKind::Shape,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ColorValue, MotionEasing, MotionValue, ShapeValue, TokenKind, TokenValue, TypographyValue,
        TypographyWeight,
    };

    #[test]
    fn each_variant_reports_its_own_kind() {
        assert_eq!(
            TokenValue::Color(ColorValue::TRANSPARENT).kind(),
            TokenKind::Color
        );
        assert_eq!(TokenValue::Spacing(8.0).kind(), TokenKind::Spacing);
        assert_eq!(
            TokenValue::Typography(TypographyValue {
                size: 14.0,
                line_height: 20.0,
                weight: TypographyWeight::Regular
            })
            .kind(),
            TokenKind::Typography
        );
        assert_eq!(
            TokenValue::Motion(MotionValue {
                duration_ms: 150.0,
                easing: MotionEasing::EaseOut
            })
            .kind(),
            TokenKind::Motion
        );
        assert_eq!(
            TokenValue::Shape(ShapeValue { corner_radius: 4.0 }).kind(),
            TokenKind::Shape
        );
    }

    #[test]
    fn linear_light_lerp_is_brighter_than_a_naive_srgb_byte_average() {
        // Averaging the *encoded* bytes of 0 and 255 gives 0.5 in encoded
        // space too (127.5/255 == 0.5), so the naive approach and the
        // correct one agree on what number to compute — they disagree on
        // which space that number lives in. Decoded as if it were an sRGB
        // code value (the naive, wrong reading), 0.5 is only ~0.214 of the
        // actual light; decoded as the linear-light value it actually is
        // (our `lerp`, correct), it is 0.5 of the light. That gap is the
        // "muddy brown midpoint" artefact this module exists to avoid.
        let red = ColorValue::from_srgb8(255, 0, 0, 255);
        let green = ColorValue::from_srgb8(0, 255, 0, 255);
        let correct = red.lerp(green, 0.5);
        assert!((correct.r - 0.5).abs() < 1e-6, "r={}", correct.r);
        assert!((correct.g - 0.5).abs() < 1e-6, "g={}", correct.g);
        assert_eq!(correct.b, 0.0);

        let naive_reading_of_the_same_number = super::srgb_to_linear(0.5);
        assert!(
            naive_reading_of_the_same_number < 0.25,
            "the naive (wrong) decode should read as much dimmer than the correct linear-space value: {naive_reading_of_the_same_number}"
        );
    }

    #[test]
    fn srgb_round_trip_is_stable_at_the_extremes() {
        let black = ColorValue::from_srgb8(0, 0, 0, 0);
        assert_eq!(black, ColorValue::TRANSPARENT);
        let white = ColorValue::from_srgb8(255, 255, 255, 255);
        assert!((white.r - 1.0).abs() < 1e-6);
        assert!((white.g - 1.0).abs() < 1e-6);
        assert!((white.b - 1.0).abs() < 1e-6);
        assert!((white.a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn json_round_trips_a_color_value() {
        let value = TokenValue::Color(ColorValue::from_srgb8(10, 20, 30, 255));
        let json = serde_json::to_string(&value).unwrap();
        let back: TokenValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, value);
    }
}
