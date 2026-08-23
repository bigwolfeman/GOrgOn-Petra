//! The vocabulary and the two themes Petra ships: `standard_vocabulary`
//! declares the names every theme must define, and `light`/`dark` assign
//! them. Both themes are built through [`Theme::build`], so an author who
//! adds a name here and forgets one theme's assignment gets a compile-time-
//! adjacent test failure (see the tests module), not a silently incomplete
//! theme shipped to a feature author.

use std::collections::BTreeMap;

use crate::token::ThemeMode;
use crate::token::focus::{HALO_TOKEN, RING_TOKEN};
use crate::token::name::TokenName;
use crate::token::status::{StatusShape, StatusToken};
use crate::token::theme::Theme;
use crate::token::value::{
    ColorValue, MotionEasing, MotionValue, ShapeValue, TokenKind, TokenValue, TypographyValue,
    TypographyWeight,
};
use crate::token::vocabulary::{DesignToken, Vocabulary};

fn name(n: &str) -> TokenName {
    TokenName::new(n)
        .unwrap_or_else(|err| panic!("shipped vocabulary name {n:?} must be well-formed: {err}"))
}

/// The shipped spacing ramp: eight steps, in logical units, on a 4-unit base.
///
/// A **ramp**, not a number line. Its job is to be the whole set of gaps a
/// page is allowed to use, so that two panels written by two authors line up
/// without either of them measuring the other. That is why it is short and
/// why the steps grow: `2xs`/`xs` separate glyph-scale things (an icon from
/// its label), `sm`/`md` separate controls inside a group, `lg`/`xl` separate
/// groups, and `2xl`/`3xl` separate regions of a page. A gap that lands
/// between two steps is a gap nobody chose.
///
/// Every value the pre-ramp vocabulary defined — 4, 8, 16 — survives here
/// under some name, so the change from three steps to eight is a rename, not
/// a revalue. Two names shift, and both shifts are downward by one step: what
/// was `spacing.sm` (4) is now `spacing.xs`, and what was `spacing.md` (8) is
/// now `spacing.sm`. `spacing.lg` (16) does not move.
///
/// Declared once and shared by both shipped themes verbatim, because spacing
/// is geometry: a gap does not change when the lights go out. Colour is the
/// only channel [`light`] and [`dark`] disagree on, and
/// `the_two_shipped_themes_agree_on_every_gap` holds them to that.
const SPACING_RAMP: [(&str, f32); 8] = [
    ("spacing.2xs", 2.0),
    ("spacing.xs", 4.0),
    ("spacing.sm", 8.0),
    ("spacing.md", 12.0),
    ("spacing.lg", 16.0),
    ("spacing.xl", 24.0),
    ("spacing.2xl", 32.0),
    ("spacing.3xl", 48.0),
];

/// Assign every [`SPACING_RAMP`] step into a theme's value map.
///
/// Both themes call this rather than spelling the ramp out twice: a hand-
/// copied ramp is a ramp that drifts, and a drifted one would make the same
/// token name mean two different gaps depending on the operator's theme.
fn insert_spacing_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, units) in SPACING_RAMP {
        values.insert(name(step), TokenValue::Spacing(units));
    }
}

/// The full set of tokens the shipped light and dark themes must define.
///
/// Deliberately small: enough surface/text/status/spacing/typography/
/// motion/shape coverage to exercise every [`TokenKind`], not an attempt at
/// a production-scale design system.
#[must_use]
pub fn standard_vocabulary() -> Vocabulary {
    let mut vocab = Vocabulary::new();

    vocab
        .declare(DesignToken::new(name("surface.base"), TokenKind::Color))
        .declare(DesignToken::new(name("surface.raised"), TokenKind::Color))
        .declare(DesignToken::new(name("text.primary"), TokenKind::Color))
        .declare(DesignToken::new(name("text.muted"), TokenKind::Color))
        .declare(DesignToken::new(name("spacing.2xs"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.xs"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.sm"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.md"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.lg"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.xl"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.2xl"), TokenKind::Spacing))
        .declare(DesignToken::new(name("spacing.3xl"), TokenKind::Spacing))
        .declare(DesignToken::new(
            name("typography.body"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(
            name("typography.heading"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(name("motion.fast"), TokenKind::Motion))
        .declare(DesignToken::new(name("motion.slow"), TokenKind::Motion))
        .declare(DesignToken::new(name("shape.corner-sm"), TokenKind::Shape))
        .declare(DesignToken::new(name("shape.corner-lg"), TokenKind::Shape))
        // The keyboard focus ring (FR-015, FR-025). Two colours, because the
        // ring is an ink band flanked by two paper halos: see
        // `crate::token::focus` for why one band cannot be enough, and
        // `the_focus_ring_is_visible_over_any_surface` below for the
        // measurement that holds the pair to it.
        .declare(DesignToken::new(name(RING_TOKEN), TokenKind::Color))
        .declare(DesignToken::new(name(HALO_TOKEN), TokenKind::Color));

    // The status subset (FR-015): colour, shape, and text together. Shape
    // and text are mode-independent — only the colour painted into the
    // shape changes between `light()` and `dark()`.
    vocab
        .declare_status(
            StatusToken::new(name("status.ok"), StatusShape::Circle, "OK")
                .expect("non-empty literal text"),
        )
        .declare_status(
            StatusToken::new(name("status.degraded"), StatusShape::Triangle, "Degraded")
                .expect("non-empty literal text"),
        )
        .declare_status(
            StatusToken::new(name("status.down"), StatusShape::Square, "Down")
                .expect("non-empty literal text"),
        );

    vocab
}

/// The shipped light theme, checked complete against [`standard_vocabulary`]
/// at call time (`Theme::build`'s `.expect` below is the completeness
/// proof: it can only panic if this function's own assignments fall out of
/// step with the vocabulary above, which the module's tests catch).
#[must_use]
pub fn light() -> Theme {
    let vocab = standard_vocabulary();
    let mut values = BTreeMap::new();

    values.insert(
        name("surface.base"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xff, 0xff, 0xff)),
    );
    values.insert(
        name("surface.raised"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0x1a, 0x1a, 0x1a, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0x5c, 0x5c, 0x5c, 0xff)),
    );
    insert_spacing_ramp(&mut values);
    values.insert(
        name("typography.body"),
        TokenValue::Typography(TypographyValue {
            size: 14.0,
            line_height: 20.0,
            weight: TypographyWeight::Regular,
        }),
    );
    values.insert(
        name("typography.heading"),
        TokenValue::Typography(TypographyValue {
            size: 20.0,
            line_height: 28.0,
            weight: TypographyWeight::Bold,
        }),
    );
    values.insert(
        name("motion.fast"),
        TokenValue::Motion(MotionValue {
            duration_ms: 120.0,
            easing: MotionEasing::EaseOut,
        }),
    );
    values.insert(
        name("motion.slow"),
        TokenValue::Motion(MotionValue {
            duration_ms: 320.0,
            easing: MotionEasing::EaseInOut,
        }),
    );
    values.insert(
        name("shape.corner-sm"),
        TokenValue::Shape(ShapeValue { corner_radius: 4.0 }),
    );
    values.insert(
        name("shape.corner-lg"),
        TokenValue::Shape(ShapeValue {
            corner_radius: 12.0,
        }),
    );

    // Status colours. The values are chosen by measurement, not by taste:
    // `status_colours_stay_apart_under_red_green_colour_blindness` simulates
    // deuteranopia and protanopia and asserts a floor on the perceptual
    // distance between every pair. Read that test before changing any of
    // these three, because the constraint is not obvious — the pair that
    // actually breaks is `degraded` vs `down`, not `ok` vs `down`, and the
    // previous palette scored ΔE*ab 8.8 on it (indistinguishable) while a
    // comment here claimed the colours were separated in lightness.
    //
    // Both are dark against a near-white surface, so all three have to fit
    // between L* 0 and roughly L* 62 to clear 3:1 against the background;
    // the separation therefore comes from spreading them across that band
    // rather than from hue, which red-green colour blindness collapses.
    // The focus ring, light mode: ink core, paper halos. Both are
    // achromatic on purpose — the indicator must not depend on hue at all,
    // and a grey pair is the one choice red-green colour blindness cannot
    // touch. The two are ~18:1 apart, and between them they cover every
    // possible background: see `the_focus_ring_is_visible_over_any_surface`,
    // which sweeps the luminance range rather than trusting this comment.
    values.insert(
        name(RING_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(0x14, 0x14, 0x14, 0xff)),
    );
    values.insert(
        name(HALO_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xff, 0xff, 0xff)),
    );

    values.insert(
        name("status.ok"),
        TokenValue::Color(ColorValue::from_srgb8(0x40, 0x96, 0x88, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xb5, 0x54, 0x1a, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0x49, 0x12, 0x15, 0xff)),
    );

    Theme::build(ThemeMode::Light, &vocab, values).expect("shipped light theme must be complete")
}

/// The shipped dark theme. See [`light`] for the completeness note.
#[must_use]
pub fn dark() -> Theme {
    let vocab = standard_vocabulary();
    let mut values = BTreeMap::new();

    values.insert(
        name("surface.base"),
        TokenValue::Color(ColorValue::from_srgb8(0x12, 0x12, 0x12, 0xff)),
    );
    values.insert(
        name("surface.raised"),
        TokenValue::Color(ColorValue::from_srgb8(0x22, 0x22, 0x22, 0xff)),
    );
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0xa3, 0xa3, 0xa3, 0xff)),
    );
    insert_spacing_ramp(&mut values);
    values.insert(
        name("typography.body"),
        TokenValue::Typography(TypographyValue {
            size: 14.0,
            line_height: 20.0,
            weight: TypographyWeight::Regular,
        }),
    );
    values.insert(
        name("typography.heading"),
        TokenValue::Typography(TypographyValue {
            size: 20.0,
            line_height: 28.0,
            weight: TypographyWeight::Bold,
        }),
    );
    values.insert(
        name("motion.fast"),
        TokenValue::Motion(MotionValue {
            duration_ms: 120.0,
            easing: MotionEasing::EaseOut,
        }),
    );
    values.insert(
        name("motion.slow"),
        TokenValue::Motion(MotionValue {
            duration_ms: 320.0,
            easing: MotionEasing::EaseInOut,
        }),
    );
    values.insert(
        name("shape.corner-sm"),
        TokenValue::Shape(ShapeValue { corner_radius: 4.0 }),
    );
    values.insert(
        name("shape.corner-lg"),
        TokenValue::Shape(ShapeValue {
            corner_radius: 12.0,
        }),
    );

    // The focus ring, dark mode: the ink/paper pair inverted, so the core
    // still reads as the drawn line and the halos as the ground around it.
    values.insert(
        name(RING_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name(HALO_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(0x05, 0x05, 0x05, 0xff)),
    );

    values.insert(
        name("status.ok"),
        TokenValue::Color(ColorValue::from_srgb8(0x29, 0x8e, 0x86, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xc4, 0x7a, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0x1c, 0x0d, 0xff)),
    );

    Theme::build(ThemeMode::Dark, &vocab, values).expect("shipped dark theme must be complete")
}

#[cfg(test)]
mod tests {
    use super::{dark, light, standard_vocabulary};
    use crate::token::ThemeMode;
    use crate::token::focus::{HALO_TOKEN, RING_TOKEN};
    use crate::token::name::TokenName;
    use crate::token::value::{ColorValue, TokenValue};

    /// The floor every pair of shipped status colours must clear, in CIE
    /// ΔE*ab, after the frame is simulated through red-green colour
    /// blindness. 30 is chosen as "two colours a reader will not confuse at a
    /// glance"; for scale, the palette this replaced scored 5.5 in dark mode
    /// and 8.8 in light, which is the same colour twice.
    const MIN_STATUS_SEPARATION: f32 = 30.0;

    /// The floor a status colour must clear against the surface it is painted
    /// on, as a WCAG contrast ratio. Without this a palette could win the
    /// separation test by being invisible in three different ways.
    const MIN_SURFACE_CONTRAST: f32 = 3.0;

    /// The floor the focus ring's two bands must clear against *each other*.
    /// Higher than [`MIN_SURFACE_CONTRAST`] on purpose: the pair is there to
    /// cover the whole luminance range between them, and two bands only 3:1
    /// apart leave a band of surface colours that hides both.
    const MIN_RING_BAND_SEPARATION: f32 = 7.0;

    /// Viénot-Brettel-Mollon reduced matrices, applied to *linear* RGB.
    /// Deuteranopia (no green cone) and protanopia (no red cone) are both
    /// simulated because "red-green colour blind" covers both and they do not
    /// collapse the same pairs.
    const DEUTERANOPE: [[f32; 3]; 3] = [
        [0.625, 0.375, 0.0],
        [0.700, 0.300, 0.0],
        [0.0, 0.300, 0.700],
    ];
    const PROTANOPE: [[f32; 3]; 3] = [
        [0.1667, 0.8333, 0.0],
        [0.1667, 0.8333, 0.0],
        [0.0, 0.1667, 0.8333],
    ];

    fn simulate(c: ColorValue, m: &[[f32; 3]; 3]) -> [f32; 3] {
        let v = [c.r, c.g, c.b];
        std::array::from_fn(|i| (m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2]).clamp(0.0, 1.0))
    }

    /// Linear sRGB to CIE L\*a\*b\* under D65, the space ΔE\*ab is defined in.
    fn to_lab(v: [f32; 3]) -> [f32; 3] {
        const M: [[f32; 3]; 3] = [
            [0.412_456_4, 0.357_576_1, 0.180_437_5],
            [0.212_672_9, 0.715_152_2, 0.072_175],
            [0.019_333_9, 0.119_192, 0.950_304_1],
        ];
        const WHITE: [f32; 3] = [0.950_47, 1.0, 1.088_83];
        let f: [f32; 3] = std::array::from_fn(|i| {
            let t = (M[i][0] * v[0] + M[i][1] * v[1] + M[i][2] * v[2]) / WHITE[i];
            if t > 0.008_856 {
                t.cbrt()
            } else {
                7.787 * t + 16.0 / 116.0
            }
        });
        [
            116.0 * f[1] - 16.0,
            500.0 * (f[0] - f[1]),
            200.0 * (f[1] - f[2]),
        ]
    }

    fn delta_e(a: [f32; 3], b: [f32; 3]) -> f32 {
        let d: [f32; 3] = std::array::from_fn(|i| a[i] - b[i]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    fn relative_luminance(c: ColorValue) -> f32 {
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }

    fn contrast(a: ColorValue, b: ColorValue) -> f32 {
        let (x, y) = (relative_luminance(a), relative_luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    fn theme_color(theme: &crate::token::Theme, token: &str) -> ColorValue {
        match theme.value(&TokenName::new(token).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{token} is not a colour: {other:?}"),
        }
    }

    /// FR-015's colour channel, measured rather than asserted.
    ///
    /// This is the test the previous palette did not have, and its absence is
    /// why a comment claiming the status colours were "distinguishable in hue
    /// *and* lightness" survived while `degraded` and `down` were ΔE 5.5 apart
    /// under deuteranopia — the same colour to the person this project is
    /// built for.
    ///
    /// The shape and text channels are what actually carry status meaning
    /// (see [`crate::token::status`]); this test does not make colour
    /// sufficient on its own and is not trying to. It keeps colour from being
    /// actively misleading, which is a lower bar and a real one.
    #[test]
    fn status_colours_stay_apart_under_red_green_colour_blindness() {
        const STATUSES: [&str; 3] = ["status.ok", "status.degraded", "status.down"];
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let surface = theme_color(&theme, "surface.raised");
            let colors: Vec<ColorValue> = STATUSES.iter().map(|s| theme_color(&theme, s)).collect();

            for (name, c) in STATUSES.iter().zip(&colors) {
                let ratio = contrast(*c, surface);
                assert!(
                    ratio >= MIN_SURFACE_CONTRAST,
                    "{label}/{name} is only {ratio:.2}:1 against surface.raised; \
                     a status nobody can see is not a channel"
                );
            }

            for (vision, matrix) in [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)] {
                let seen: Vec<[f32; 3]> = colors
                    .iter()
                    .map(|c| to_lab(simulate(*c, matrix)))
                    .collect();
                for i in 0..STATUSES.len() {
                    for j in (i + 1)..STATUSES.len() {
                        let d = delta_e(seen[i], seen[j]);
                        assert!(
                            d >= MIN_STATUS_SEPARATION,
                            "{label}: {} and {} are only ΔE*ab {d:.1} apart to a \
                             {vision} reader (floor is {MIN_STATUS_SEPARATION}); \
                             the pair that breaks first is usually degraded/down, \
                             because red-green colour blindness maps amber and red \
                             onto each other — separate them in lightness, not hue",
                            STATUSES[i],
                            STATUSES[j],
                        );
                    }
                }
            }
        }
    }

    /// FR-015 for keyboard focus: the ring must be legible over whatever it
    /// lands on, and it must not need hue to be so.
    ///
    /// `crate::token::focus` arranges the ring as halo/core/halo, so each of
    /// the two surfaces the ring can touch — the focused node's own fill
    /// inside its edge, whatever is behind the node outside it — carries one
    /// band of each colour. The claim this test measures is therefore about a
    /// *single* surface: for any colour at all, at least one of the pair
    /// clears [`MIN_SURFACE_CONTRAST`] against it.
    ///
    /// The sweep is over luminance rather than over the shipped palette,
    /// because WCAG contrast is a function of luminance alone: covering
    /// `0.0..=1.0` covers every colour that exists, including whatever a
    /// feature author's own theme binds to a node's `background`. The shipped
    /// palette is then checked as well, so a failure names a real token.
    #[test]
    fn the_focus_ring_is_visible_over_any_surface() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let core = theme_color(&theme, RING_TOKEN);
            let halo = theme_color(&theme, HALO_TOKEN);

            // Every colour that exists, by luminance. `grey` is built in
            // linear light directly, which is the space `relative_luminance`
            // reads, so stepping it steps luminance uniformly.
            for step in 0..=200 {
                let y = step as f32 / 200.0;
                let grey = ColorValue {
                    r: y,
                    g: y,
                    b: y,
                    a: 1.0,
                };
                let best = contrast(core, grey).max(contrast(halo, grey));
                assert!(
                    best >= MIN_SURFACE_CONTRAST,
                    "{label}: a surface at luminance {y:.3} hides the whole \
                     focus ring — core {:.2}:1, halo {:.2}:1, floor is \
                     {MIN_SURFACE_CONTRAST}. The pair must straddle the \
                     luminance range: one band light enough for dark ground, \
                     one dark enough for light ground.",
                    contrast(core, grey),
                    contrast(halo, grey),
                );
            }

            // Named surfaces, so a regression points at a token.
            for token in [
                "surface.base",
                "surface.raised",
                "text.primary",
                "text.muted",
                "status.ok",
                "status.degraded",
                "status.down",
            ] {
                let under = theme_color(&theme, token);
                let best = contrast(core, under).max(contrast(halo, under));
                assert!(
                    best >= MIN_SURFACE_CONTRAST,
                    "{label}: a node bound to {token} hides the focus ring \
                     ({best:.2}:1, floor is {MIN_SURFACE_CONTRAST})"
                );
            }

            // The two bands must also read against each other, or the ring is
            // one thick band and the halos buy nothing.
            assert!(
                contrast(core, halo) >= MIN_RING_BAND_SEPARATION,
                "{label}: the ring's core and halo are only {:.2}:1 apart",
                contrast(core, halo)
            );

            // No hue, so no hue to lose. This is the strongest form of "not
            // by colour alone" available: red-green colour blindness cannot
            // move an achromatic pair at all, and the assertions below prove
            // it by re-running the separation through both simulations.
            for (what, c) in [("core", core), ("halo", halo)] {
                assert!(
                    (c.r - c.g).abs() < 1e-6 && (c.g - c.b).abs() < 1e-6,
                    "{label}: the ring's {what} is not achromatic ({c:?}); a \
                     focus indicator must not spend hue it may not have"
                );
            }
            for (vision, matrix) in [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)] {
                let seen = |c: ColorValue| {
                    let v = simulate(c, matrix);
                    ColorValue {
                        r: v[0],
                        g: v[1],
                        b: v[2],
                        a: 1.0,
                    }
                };
                let (c, h) = (seen(core), seen(halo));
                assert!(
                    contrast(c, h) >= MIN_RING_BAND_SEPARATION,
                    "{label}: to a {vision} reader the ring's bands are only \
                     {:.2}:1 apart",
                    contrast(c, h)
                );
                assert!(
                    delta_e(
                        to_lab(simulate(core, matrix)),
                        to_lab(simulate(halo, matrix))
                    ) >= MIN_STATUS_SEPARATION,
                    "{label}: the ring's bands collapse together for a {vision} reader"
                );
            }
        }
    }

    /// The ramp, pinned by value.
    ///
    /// Every styling prop in every tree now resolves through one of these
    /// eight numbers, so changing one is a page-wide visual change and has to
    /// be a deliberate edit here rather than a quiet drift. The two carried
    /// forward from the pre-ramp vocabulary — `spacing.xs` at 4 and
    /// `spacing.sm` at 8 — are the ones that used to be called `spacing.sm`
    /// and `spacing.md`; that rename is the reason a converted call site may
    /// name a different token than the number it replaced suggests.
    #[test]
    fn the_spacing_ramp_is_eight_growing_steps_on_a_four_unit_base() {
        let expected: [(&str, f32); 8] = [
            ("spacing.2xs", 2.0),
            ("spacing.xs", 4.0),
            ("spacing.sm", 8.0),
            ("spacing.md", 12.0),
            ("spacing.lg", 16.0),
            ("spacing.xl", 24.0),
            ("spacing.2xl", 32.0),
            ("spacing.3xl", 48.0),
        ];
        let theme = light();
        let mut previous = 0.0f32;
        for (step, units) in expected {
            let got = theme.value(&TokenName::new(step).unwrap());
            assert_eq!(
                got,
                Some(&TokenValue::Spacing(units)),
                "{step} must be {units} logical units, found {got:?}"
            );
            assert!(
                units > previous,
                "the ramp must grow: {step} at {units} does not exceed {previous}"
            );
            previous = units;
        }

        // Every spacing name the vocabulary declares is one of the eight
        // above, so no ninth step can appear without this test naming it.
        let vocab = standard_vocabulary();
        let declared: Vec<&TokenName> = vocab
            .names()
            .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Spacing))
            .collect();
        assert_eq!(
            declared.len(),
            expected.len(),
            "the vocabulary declares {} spacing tokens but the ramp has {}: {declared:?}",
            declared.len(),
            expected.len()
        );
    }

    /// Spacing is geometry. A gap does not change when the operator turns the
    /// lights off, so the two shipped themes must assign every spacing name
    /// the same number — the only channel they are allowed to disagree on is
    /// colour.
    #[test]
    fn the_two_shipped_themes_agree_on_every_gap() {
        let (l, d) = (light(), dark());
        let vocab = standard_vocabulary();
        let mut checked = 0usize;
        for name in vocab.names() {
            if vocab.kind_of(name) != Some(crate::token::value::TokenKind::Spacing) {
                continue;
            }
            assert_eq!(
                l.value(name),
                d.value(name),
                "{name} differs between light and dark; spacing is geometry, not colour"
            );
            checked += 1;
        }
        assert_eq!(
            checked, 8,
            "expected the eight-step ramp, checked {checked}"
        );
    }

    #[test]
    fn the_shipped_light_theme_is_complete() {
        let theme = light();
        assert_eq!(theme.mode(), ThemeMode::Light);
        for name in standard_vocabulary().names() {
            assert!(theme.value(name).is_some(), "light theme is missing {name}");
        }
    }

    #[test]
    fn the_shipped_dark_theme_is_complete() {
        let theme = dark();
        assert_eq!(theme.mode(), ThemeMode::Dark);
        for name in standard_vocabulary().names() {
            assert!(theme.value(name).is_some(), "dark theme is missing {name}");
        }
    }

    #[test]
    fn every_shipped_status_token_carries_a_shape_and_non_empty_text() {
        let vocab = standard_vocabulary();
        let statuses: Vec<_> = vocab.statuses().collect();
        assert_eq!(
            statuses.len(),
            3,
            "expected status.ok, status.degraded, status.down"
        );
        for status in statuses {
            assert!(
                !status.text().trim().is_empty(),
                "{} has no text channel",
                status.name()
            );
            // `shape()` returns a `StatusShape`, not an `Option`: simply
            // calling it is proof a shape exists, since there is no other
            // way to have constructed this `StatusToken`.
            let _ = status.shape();
        }
    }
}
