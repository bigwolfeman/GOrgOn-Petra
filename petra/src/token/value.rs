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
    /// A text style: size, line height, weight, tracking and face class.
    Typography,
    /// A transition timing: duration and easing curve.
    Motion,
    /// A spring's physical parameters: damping ratio and stiffness.
    Spring,
    /// A geometric parameter of a shape, e.g. a corner radius.
    Shape,
    /// An outline family: which closed figure a slot's rect is drawn as.
    Silhouette,
    /// How many times a glyph's coverage is composited against itself —
    /// the sharpness of rendered text. See [`CoverageValue`].
    Coverage,
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

    /// WCAG relative luminance.
    ///
    /// The channels are already linear-light ([`ColorValue::from_srgb8`]
    /// applies the transfer function on the way in), so this is the weighted
    /// sum and nothing else. Alpha is **not** read: a translucent colour has
    /// no luminance of its own until it is composited over something, which
    /// is what [`ColorValue::over`] is for.
    #[must_use]
    pub fn relative_luminance(self) -> f32 {
        0.2126 * self.r + 0.7152 * self.g + 0.0722 * self.b
    }

    /// The WCAG 2.1 contrast ratio between two colours, `1.0` to `21.0`.
    ///
    /// Symmetric: the brighter of the two goes on top whichever way round the
    /// call is written, so a caller cannot get a ratio below 1 by passing the
    /// pair the wrong way.
    #[must_use]
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// This colour composited over `background`, using straight source-over
    /// alpha in linear light.
    ///
    /// This is what makes a faded colour measurable. A design system that
    /// only ever compares *token values* cannot see an opacity applied
    /// downstream — `Props.opacity` fades a whole subtree at paint time, so
    /// `text.primary` at 10.7:1 against its card reaches the screen at 3.4:1
    /// and every value-level check still passes. Compositing first is the
    /// only way a gate reads what the reader reads (FR-010).
    #[must_use]
    pub fn over(self, background: Self) -> Self {
        let a = self.a.clamp(0.0, 1.0);
        let out_a = a + background.a * (1.0 - a);
        if out_a <= f32::EPSILON {
            return Self::TRANSPARENT;
        }
        let mix = |src: f32, dst: f32| (src * a + dst * background.a * (1.0 - a)) / out_a;
        Self {
            r: mix(self.r, background.r),
            g: mix(self.g, background.g),
            b: mix(self.b, background.b),
            a: out_a,
        }
    }

    /// This colour with its alpha scaled by `factor`, clamped to `0.0..=1.0`.
    ///
    /// The compositing form of `Props.opacity`: a painter that sets a group
    /// opacity of 0.45 draws every colour in that group at 45% of the alpha
    /// it declared, so this is how a gate reproduces what was drawn.
    #[must_use]
    pub fn faded(self, factor: f32) -> Self {
        Self {
            a: (self.a * factor).clamp(0.0, 1.0),
            ..self
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
///
/// Four fields, not three. `letter_spacing` and `family` were added on
/// 2026-08-25 when the Carbon inventory was ported (spec 005 FR-023,
/// `contracts/token-vocabulary.md` §7), and both were added in the *same*
/// change on purpose: this is a public type carrying `Serialize` and
/// `Deserialize`, so each field costs one compile-wide migration, and the
/// research that asked for the first
/// (`specs/005-petra-carbon-authoring/research/R-F-tokens-and-themes.md` §4)
/// deferred the second only to avoid "growing the type twice". Growing it
/// once is the cheaper reading of that same sentence.
///
/// Both new fields are `#[serde(default)]`, so a payload written before this
/// change still deserializes — as `0.0` tracking in the sans face, which is
/// exactly what those payloads meant.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypographyValue {
    /// Font size in logical units.
    pub size: f32,
    /// Line height in logical units.
    pub line_height: f32,
    /// Font weight class.
    pub weight: TypographyWeight,
    /// Extra tracking between glyphs, in logical units. `0.0` is the font's
    /// own spacing; positive opens it up.
    ///
    /// **This field exists because Carbon puts tracking on the small roles,
    /// not the big ones.** `@carbon/type` 11.65.0 measures `0.32px` on
    /// `caption-01`, `label-01` and `code-01`, `0.16px` on `body-01`,
    /// `body-compact-01`, `label-02` and `heading-compact-01`, and `0`
    /// everywhere at 16 units and above — the opposite shape from Material 3,
    /// which tracks display and headline roles. Small dense text is most of
    /// what a desktop tool paints, so a type ramp with no field for this
    /// cannot express the part of Carbon that matters here.
    ///
    /// Reaches the screen through `epaint 0.36.1`'s
    /// `TextFormat::extra_letter_spacing` (`text_layout_types.rs:484`),
    /// carried there by `gorgon_petra_egui::text::TextStyle`.
    #[serde(default)]
    pub letter_spacing: f32,
    /// Which face class draws this style.
    ///
    /// Separate from [`TypographyValue::weight`] because they are orthogonal
    /// questions with orthogonal answers: weight picks between three faces of
    /// one family, and this picks the family. A single `weight`-keyed lookup
    /// cannot express "monospace, regular", which is what `code-01` is.
    #[serde(default)]
    pub family: TypographyFamily,
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

/// Which face class a typography token draws through.
///
/// Two members, and there is no third planned: Carbon's whole published
/// stack is `IBM Plex Sans` and `IBM Plex Mono` (`@carbon/type`'s
/// `fontFamily.ts`), and every one of the twelve roles the 42 audited
/// components reference is one or the other. A serif or display class would
/// be a name with no reader, which is the defect the M-Carbon note counted at
/// 12 of 18 names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypographyFamily {
    /// The proportional face. The default, because twelve of Carbon's
    /// fourteen productive roles carry no `fontFamily` key at all and inherit
    /// sans from its `reset` mixin.
    #[default]
    Sans,
    /// The fixed-pitch face, for `code-01`/`code-02`. The only two roles that
    /// name a family explicitly.
    Mono,
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

/// A spring, named by the two numbers that define one.
///
/// Separate from [`MotionValue`] rather than a field on it, because the two
/// are different animations and not two settings of one. A [`MotionValue`]
/// is a duration and a curve: it starts, it runs for exactly that long, it
/// stops. A spring has no duration — it is a differential equation that
/// settles when it settles, it can be retargeted mid-flight from wherever it
/// currently is, and it carries velocity across that retarget. A struct
/// holding `duration_ms`, `easing`, `stiffness` and `damping_ratio` would be
/// a union wearing a struct's clothes, with two of the four fields dead in
/// either reading.
///
/// This type deliberately does **not** name
/// [`crate::anim::Spring`](crate::anim::spring::Spring). `anim` depends on
/// `token` — `anim::value` resolves [`ColorValue`] — so a token naming an
/// `anim` type would close that loop. The conversion lives on the `anim`
/// side, as `impl TryFrom<SpringValue> for Spring`, which is also where the
/// error for an unphysical pair belongs: a `TokenValue` is a declaration and
/// this type stays a plain pair of numbers, checked when something builds a
/// spring out of it.
///
/// # Units
///
/// `stiffness`, not frequency, because that is the unit the source
/// publishes. M-Carbon's motion constants come from Material 3 Expressive,
/// whose `SpringForce` is unit-mass — it computes `mNaturalFreq =
/// Math.sqrt(stiffness)` with no mass term — and
/// [`crate::anim::Spring`](crate::anim::spring::Spring) is unit-mass too
/// (`spring.rs`: *"stiffness = ω₀² and damping = 2ζω₀"*). So the conversion
/// is `ω₀ = √stiffness` and `ζ = damping_ratio`, exactly, with no correction
/// factor, and storing stiffness keeps the token readable against the table
/// it was copied from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpringValue {
    /// ζ, the damping ratio. Below 1 overshoots, 1 is critically damped
    /// (fastest approach with no overshoot), above 1 crawls in.
    pub damping_ratio: f32,
    /// The spring constant, at unit mass. `ω₀ = √stiffness`.
    pub stiffness: f32,
}

/// A geometric parameter of a shape. Extended with more fields (border
/// width, etc.) as the vocabulary grows; a single corner radius is what the
/// shipped vocabulary needs today.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeValue {
    /// Corner radius in logical units.
    pub corner_radius: f32,
}

/// The glyph text-sharpness curve: how many times rasterised coverage is
/// composited against itself, plus whether a glyph's horizontal origin
/// snaps to a whole device pixel.
///
/// The port of ai-macs' two *related but distinct* env knobs,
/// `GORGON_TEXT_PASSES` and `GORGON_TEXT_SNAP`
/// (`ai-macs/pkg/ui/gio/text_tuning.go:41-101`), into one Petra token rather
/// than process environment, so an agent or a user can tune either through a
/// theme revision instead of a recompile
/// (`ignored/builds/2026-08-24-text-pipeline-port/SPEC.md` §6, and
/// `ai-macs/Ai-notes/08-15-2026/TextRendering/01-glyph-pixel-snap.md` for
/// the root cause the pair addresses).
///
/// **Two knobs, one root cause, opposite ends of it.** The defect is
/// per-character *weight variance*: a stem that lands on a pixel boundary
/// rasterises as one dark column, and the same stem half a pixel over
/// spreads across two columns at half coverage each, which reads grey next
/// to its neighbours. `snap` attacks the cause — placement — by rounding
/// every glyph origin to a whole pixel, which drives the variance to
/// exactly zero and pays for it by quantising kerning (uneven-looking
/// spacing). `passes` attacks the symptom — how visible a given amount of
/// variance reads — by steepening the coverage curve so a half-coverage
/// column darkens toward its full-coverage neighbours rather than sitting
/// visibly between them. ai-macs' own comment on the trade-off: it "cannot
/// be settled by measurement", which is why both ship as knobs rather than
/// as a single chosen constant.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoverageValue {
    /// How many times coverage is composited against itself. `1.0` is no
    /// compositing (equivalent to epaint's `Off`); `2.0` is exactly
    /// epaint's `TwoCoverageMinusCoverageSq`; values above `2.0` have no
    /// closed-form atlas curve and are reached by repeated painting instead
    /// (SPEC.md §2.3). Continuous, not an integer count, for the same
    /// reason ai-macs' own field is a `float32`: `1.0 - (1.0 - a).powf(passes)`
    /// is defined for a fractional `passes`, and a slider dragged across
    /// the range should not snap between whole numbers (SPEC.md §1.4).
    ///
    /// # Legal range
    ///
    /// `[1.0, 4.0]`, refused outside that by
    /// [`crate::token::theme::Theme::build`] the same way a negative
    /// [`ShapeValue::corner_radius`] is refused: `1.0` is "no compositing"
    /// (the identity), and `4.0` is ai-macs' own `maxTextSharpness`
    /// (`text_tuning.go:55`) — the ceiling it measured, not an arbitrary
    /// round number.
    pub passes: f32,
    /// Whether a glyph's horizontal origin snaps to a whole device pixel
    /// before it is drawn. Named for the intent — "does placement snap to
    /// the pixel grid" — the same word ai-macs' own env var uses
    /// (`GORGON_TEXT_SNAP`), not for the epaint field a binder maps it onto.
    ///
    /// **The mapping is inverted.** A binder reads this as
    /// `TextOptions::subpixel_binning = !snap`: `snap: true` means
    /// `subpixel_binning: false`. epaint's field asks "keep sub-pixel
    /// precision", so *on* means *do not* snap; this field asks "snap to
    /// the grid", so *on* means the opposite of epaint's *on*. The doc
    /// comment carries the inversion here, at the type the binder reads,
    /// rather than leaving it to be re-derived correctly at each call site.
    ///
    /// No range to refuse: every `bool` is legal, so
    /// [`crate::token::theme::Theme::build`]'s usability check has nothing
    /// to say about this field the way it does about `passes`.
    pub snap: bool,
}

/// Which closed figure a paint slot's rect is drawn as.
///
/// A separate token kind from [`ShapeValue`] rather than a field on it,
/// because the two answer different questions and a slot binds them
/// separately: [`TokenKind::Shape`] says *how round the corners are*,
/// this says *what figure the corners belong to*. Keeping them apart is
/// what lets one `radius` ramp serve every silhouette instead of one
/// combined token per (figure, radius) pair.
///
/// Three variants, not four, and that is deliberate. A circle is
/// [`Silhouette::Rect`] at `shape.corner-full`: the rect family already
/// spans every convex rounded box from a sharp square to a full disc, and
/// adding a `Circle` variant would make the same picture reachable two
/// ways. [`Silhouette::Triangle`] and [`Silhouette::Diamond`] are the two
/// figures no corner radius can reach, which is the whole reason this kind
/// exists — see `crate::component::status`, where FR-015's shape channel
/// used to collapse to a sub-pixel difference in corner radius because a
/// rounded rect was the only figure the painter could draw.
///
/// The engine names the figure; a renderer decides the geometry. Nothing
/// here is in device units and nothing here knows about a graphics API.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Silhouette {
    /// The node's rect, with its `radius` slot's corners. A square at
    /// `shape.corner-none`, a circle at `shape.corner-full` on a square
    /// box, a stadium at `shape.corner-full` on an oblong one.
    #[default]
    Rect,
    /// A triangle inscribed in the node's rect, apex at the top edge's
    /// midpoint and base along the bottom edge.
    Triangle,
    /// A diamond inscribed in the node's rect: one vertex at the midpoint
    /// of each edge.
    Diamond,
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
    /// A spring's physical parameters.
    Spring(SpringValue),
    /// A shape parameter.
    Shape(ShapeValue),
    /// An outline family.
    Silhouette(Silhouette),
    /// A glyph coverage compositing curve.
    Coverage(CoverageValue),
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
            Self::Spring(_) => TokenKind::Spring,
            Self::Shape(_) => TokenKind::Shape,
            Self::Silhouette(_) => TokenKind::Silhouette,
            Self::Coverage(_) => TokenKind::Coverage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ColorValue, CoverageValue, MotionEasing, MotionValue, ShapeValue, Silhouette, TokenKind,
        TokenValue, TypographyFamily, TypographyValue, TypographyWeight,
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
                weight: TypographyWeight::Regular,
                letter_spacing: 0.16,
                family: TypographyFamily::Sans,
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
        assert_eq!(
            TokenValue::Silhouette(Silhouette::Triangle).kind(),
            TokenKind::Silhouette
        );
        assert_eq!(
            TokenValue::Coverage(CoverageValue {
                passes: 3.0,
                snap: false
            })
            .kind(),
            TokenKind::Coverage
        );
    }

    /// `CoverageValue`'s wire spelling, pinned the same way
    /// [`silhouette_round_trips_through_its_kebab_case_wire_form`] pins
    /// `Silhouette`'s: a theme file already on disk names this shape by its
    /// serialized form, so a renamed field would silently stop matching it.
    #[test]
    fn coverage_value_round_trips_through_its_wire_form() {
        let value = TokenValue::Coverage(CoverageValue {
            passes: 3.0,
            snap: false,
        });
        let json = serde_json::to_string(&value).expect("a struct variant serializes");
        assert_eq!(
            json,
            r#"{"kind":"coverage","value":{"passes":3.0,"snap":false}}"#
        );
        let decoded: TokenValue =
            serde_json::from_str(&json).expect("the pinned wire form decodes");
        assert_eq!(decoded, value);
    }

    /// `Silhouette`'s wire spelling is what a serialized theme carries, so
    /// it is pinned here rather than left to the derive's defaults: a
    /// renamed variant would otherwise silently stop matching a theme file
    /// already on disk.
    #[test]
    fn silhouette_round_trips_through_its_kebab_case_wire_form() {
        for (variant, wire) in [
            (Silhouette::Rect, "\"rect\""),
            (Silhouette::Triangle, "\"triangle\""),
            (Silhouette::Diamond, "\"diamond\""),
        ] {
            let encoded = serde_json::to_string(&variant).expect("a unit variant serializes");
            assert_eq!(encoded, wire, "{variant:?} changed its wire spelling");
            let decoded: Silhouette =
                serde_json::from_str(wire).expect("the pinned wire form decodes");
            assert_eq!(decoded, variant);
        }
    }

    /// A slot that binds no silhouette gets the figure every node had
    /// before this kind existed.
    #[test]
    fn the_default_silhouette_is_the_rect_every_node_used_to_draw() {
        assert_eq!(Silhouette::default(), Silhouette::Rect);
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
