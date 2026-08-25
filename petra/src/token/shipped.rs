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
    ColorValue, CoverageValue, MotionEasing, MotionValue, ShapeValue, Silhouette, SpringValue,
    TokenKind, TokenValue, TypographyValue, TypographyWeight,
};
use crate::token::vocabulary::{DesignToken, Vocabulary};

fn name(n: &str) -> TokenName {
    TokenName::new(n)
        .unwrap_or_else(|err| panic!("shipped vocabulary name {n:?} must be well-formed: {err}"))
}

/// The names of the four surface layers, ground first.
///
/// A **layer set**, which is M-Carbon's depth cue and is not an elevation
/// ramp: depth is carried by one grey sitting on another, never by a shadow.
/// See `.agents/notes/proposed/architecture/2026-08-24-m-carbon-design-language.md`.
///
/// Carbon names these `background`, `layer-01`, `layer-02`, `layer-03`, and
/// this set keeps that shape — a role word for the ground, ordinals above
/// it — but spells the ordinals as words. `surface.layer-01` is not
/// constructible: [`crate::token::name`] splits on `-` and requires every
/// segment to end in lowercase letters, so a pure-digit segment is refused.
/// That rule is what stops `surface.grey700` from existing, and it catches
/// Carbon's numbering as collateral. Words are the way through it that keeps
/// the ordinal, and the ordinal is load-bearing: M-Carbon's field rule is
/// *"a field sits one layer number ahead of the background it is on"*, which
/// cannot be stated over a set of role words.
///
/// Two rules travel with this set and are recorded here because neither has
/// a token yet:
///
/// - **`field-*` sits one layer number ahead of its background.** A field on
///   `layer-two` uses `field-three`. Borders pair with their *same* number.
///   No `field-*` token ships here: nothing in the painter draws a field
///   background yet, and a declared name nothing reads is the defect the
///   M-Carbon note measured at 12 of 18 names. The rule is written down so
///   the token arrives correct rather than arriving early.
/// - **`layer-accent-*` is not a layer.** Carbon: *"not considered a proper
///   layer but a supporting color for `$layer` inside of components."* It
///   does not belong in this array and must not be counted when stepping.
const LAYER_TOKENS: [&str; 4] = [
    "surface.base",
    "surface.layer-one",
    "surface.layer-two",
    "surface.layer-three",
];

/// `surface.raised` — the name the painter binds today for layer one.
///
/// Petra named two surfaces before it had a layer set, and `gorgon-petra-egui`
/// binds both names in ~30 places. This alias resolves to exactly
/// `LAYER_TOKENS[1]`'s colour in both themes, asserted by
/// `the_raised_alias_is_layer_one_in_both_themes`, so the painter keeps
/// working unchanged and the migration is a rename in one crate rather than
/// a flag day across two. **It is a shim, not a fifth layer.** New bindings
/// use the ordinal name; the alias goes when the painter stops naming it.
const RAISED_ALIAS: &str = "surface.raised";

/// The light layer set: an **alternation** between two greys, not a ramp.
///
/// `#ffffff → #f2f2f2 → #ffffff → #f2f2f2`. This is the structural finding
/// that is easiest to get wrong, and a light theme built as a monotonic ramp
/// is wrong even when every step in it passes contrast. On a light ground
/// depth is read from the *change* at a boundary, not from a direction:
/// keep stepping a light theme darker and by layer three it is a mid-grey
/// theme, and the text colour tuned for near-white no longer holds. Carbon
/// ships four themes and both of its light ones alternate.
///
/// The pair kept is Petra's own `#ffffff`/`#f2f2f2` rather than Carbon
/// White's `#ffffff`/`#f4f4f4`. The two differ by 2/255 on each channel,
/// which is exactly the per-channel tolerance the parity lane already
/// allows, so no capture can tell them apart and adopting Carbon's would
/// re-baseline every light capture to buy a difference no gate can measure.
const LIGHT_LAYERS: [[u8; 3]; 4] = [
    [0xff, 0xff, 0xff],
    [0xf2, 0xf2, 0xf2],
    [0xff, 0xff, 0xff],
    [0xf2, 0xf2, 0xf2],
];

/// The dark layer set: a **monotonic step**, lighter each time.
///
/// `#121212 → #222222 → #333333 → #444444`, which is Petra's two shipped
/// darks extended by the same interval that already separates them. The
/// interval is chosen in CIE L\*, not in hex: the four measure L\* 5.46,
/// 13.23, 21.25, 28.85, so the steps are 7.76, 8.02 and 7.61 — a spread of
/// 0.41 across the whole set. Carbon Gray 100's own ramp (`#161616`,
/// `#262626`, `#393939`, `#525252`) steps 7.91, 8.81, 10.91, a spread of
/// 2.99, so its layers separate progressively harder as they climb.
///
/// Even steps are the point of a depth cue. Two panels nested two deep
/// should read as the same amount of "further forward" as two nested one
/// deep, and that is a statement about perceived lightness, which is what
/// L\* measures and hex does not. Keeping Petra's own base also means no
/// painted token moves: `surface.base` and `surface.raised` hold the values
/// they already had, so this set adds two layers and re-baselines nothing.
///
/// `the_dark_layer_set_steps_evenly_in_perceived_lightness` holds both
/// claims — monotone, and even — to measurement rather than to this comment.
const DARK_LAYERS: [[u8; 3]; 4] = [
    [0x12, 0x12, 0x12],
    [0x22, 0x22, 0x22],
    [0x33, 0x33, 0x33],
    [0x44, 0x44, 0x44],
];

/// Assign a mode's layer set, plus the [`RAISED_ALIAS`] shim, into a theme's
/// value map. Both themes call this for the same reason
/// [`insert_spacing_ramp`] exists: a hand-copied set is a set that drifts,
/// and a drifted alias would make `surface.raised` and `surface.layer-one`
/// two different greys under one theme.
fn insert_layer_set(values: &mut BTreeMap<TokenName, TokenValue>, layers: &[[u8; 3]; 4]) {
    for (token, rgb) in LAYER_TOKENS.iter().zip(layers) {
        values.insert(
            name(token),
            TokenValue::Color(ColorValue::from_srgb8(rgb[0], rgb[1], rgb[2], 0xff)),
        );
    }
    let one = layers[1];
    values.insert(
        name(RAISED_ALIAS),
        TokenValue::Color(ColorValue::from_srgb8(one[0], one[1], one[2], 0xff)),
    );
}

/// The shipped spring set: M-Carbon's motion constants, `(name, ζ, stiffness)`.
///
/// From Material 3 Expressive, because Carbon has no spring at all —
/// `@carbon/motion` ships easing curves and there is no `mass`, `stiffness`
/// or `damping` anywhere in its source. Petra has the opposite problem: a
/// working damped harmonic oscillator in [`crate::anim::spring`] that no
/// theme could reach, because [`MotionValue`] can only say "120 ms,
/// ease-out". [`crate::token::value::SpringValue`] is the variant that can
/// say it, and this is the seed.
///
/// **The `spatial`/`effects` split is a rule, not a naming scheme.** A
/// spring that moves a thing through space carries a little bounce, because
/// a thing arriving at a place overshoots slightly and that is what makes it
/// read as a thing rather than a fade. A spring driving opacity or colour is
/// critically damped, ζ = 1.0, in all three `effects` rows: an opacity that
/// overshoots goes past 1.0 and clips, or past 0.0 and flickers, which is
/// not expressive, it is a bug with a curve. [`crate::anim::PropertyKind`]
/// already draws exactly this line for reduced motion — `is_movement` is
/// true for position and size and false for opacity and colour — so the
/// taxonomy has a home rather than being a prefix convention, and
/// `crate::anim::spring::theme_spring` is where the two meet.
///
/// Both systems are unit-mass, so the conversion is exact: Compose's
/// `SpringForce` computes `mNaturalFreq = Math.sqrt(stiffness)` with no mass
/// term, and `crate::anim::spring` states *"stiffness = ω₀² and damping =
/// 2ζω₀"*. `ω₀ = √stiffness`, `ζ = dampingRatio`, no correction factor.
/// `the_spring_set_converts_to_the_published_frequencies` checks every row
/// against the published ω₀ rather than against `sqrt` of its own input, so
/// a mistyped stiffness fails instead of animating slightly wrong.
///
/// Shared verbatim by both themes: a spring is physics, and physics does not
/// change when the lights go out. `the_two_shipped_themes_agree_on_every_spring`
/// holds them to it, the same way `..._agree_on_every_gap` holds the spacing
/// ramp.
const SPRING_SET: [(&str, f32, f32); 6] = [
    ("motion.spatial.fast", 0.60, 800.0),
    ("motion.spatial.default", 0.80, 380.0),
    ("motion.spatial.slow", 0.80, 200.0),
    ("motion.effects.fast", 1.00, 3800.0),
    ("motion.effects.default", 1.00, 1600.0),
    ("motion.effects.slow", 1.00, 800.0),
];

/// Assign every [`SPRING_SET`] row into a theme's value map.
fn insert_spring_set(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (token, damping_ratio, stiffness) in SPRING_SET {
        values.insert(
            name(token),
            TokenValue::Spring(SpringValue {
                damping_ratio,
                stiffness,
            }),
        );
    }
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

/// The shipped corner-radius ramp: five steps, in logical units.
///
/// Five, not two, because two steps cannot express either end a component
/// library needs: a sharp edge (a table cell, a tab underline) and a full
/// pill (a toggle knob, a status dot). `none` and `full` are those two ends;
/// `sm`/`md`/`lg` are the everyday range between them — a chip or a field
/// (`sm`), a button or a card (`md`), a panel or a modal (`lg`).
///
/// `full` is `999.0`, not some smaller "big enough" number. The painter
/// hands this to `egui::CornerRadius`, whose fields are `u8` — Petra's own
/// `From<f32>` conversion there does `radius.round() as u8`, a saturating
/// cast in Rust, so `999.0` lands on exactly `255`, the type's maximum,
/// regardless of how large the sentinel is. That is the same idiom CSS uses
/// for `border-radius: 9999px`: a value chosen to always exceed half of any
/// realistic control's shorter edge, so the rendered corner is always a full
/// stadium rather than a specific radius that might not be big enough for a
/// particular box.
///
/// Two values are carried forward unchanged from the pre-ramp vocabulary —
/// `shape.corner-sm` at 4 and `shape.corner-lg` at 12 — so nothing already
/// bound to either name moves.
const SHAPE_RAMP: [(&str, f32); 5] = [
    ("shape.corner-none", 0.0),
    ("shape.corner-sm", 4.0),
    ("shape.corner-md", 8.0),
    ("shape.corner-lg", 12.0),
    ("shape.corner-full", 999.0),
];

/// Assign every [`SHAPE_RAMP`] step into a theme's value map. Both themes
/// call this rather than spelling the ramp out twice, for the same reason
/// [`insert_spacing_ramp`] exists: a hand-copied ramp is a ramp that drifts.
fn insert_shape_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, radius) in SHAPE_RAMP {
        values.insert(
            name(step),
            TokenValue::Shape(ShapeValue {
                corner_radius: radius,
            }),
        );
    }
}

/// The shipped silhouette family: the three outline figures a paint slot
/// can name.
///
/// Not a ramp — there is no ordering between a triangle and a diamond — so
/// unlike [`SHAPE_RAMP`] this is a set, and the tests below hold it to
/// membership rather than to monotonicity.
///
/// `rect` is declared even though it is what an unbound `silhouette` slot
/// already means. A design system that can only name the exceptions makes
/// "this marker is deliberately a plain box" and "somebody forgot the
/// silhouette" the same declaration; naming the default lets
/// `component::status` bind the slot for all four of `StatusShape`'s
/// variants rather than for two of them, which is what makes its mapping
/// total and its test able to walk every variant.
const SILHOUETTE_FAMILY: [(&str, Silhouette); 3] = [
    ("shape.silhouette-rect", Silhouette::Rect),
    ("shape.silhouette-triangle", Silhouette::Triangle),
    ("shape.silhouette-diamond", Silhouette::Diamond),
];

/// Assign every [`SILHOUETTE_FAMILY`] member into a theme's value map, for
/// the same reason [`insert_shape_ramp`] exists.
fn insert_silhouette_family(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (token, figure) in SILHOUETTE_FAMILY {
        values.insert(name(token), TokenValue::Silhouette(figure));
    }
}

/// The shipped typography ramp: `body` plus three heading levels.
///
/// `typography.body` and `typography.heading` are unchanged from the
/// pre-ramp vocabulary — same names, same sizes — so nothing already bound
/// to either moves. `heading-sm` and `heading-lg` are new, added around
/// `heading` the same way the corner ramp added `none`/`md`/`full` around
/// its own unchanged `sm`/`lg`: one level cannot express a hierarchy, and a
/// page with a title, a section heading, and a field label needs three
/// distinct sizes, not one repeated three times. Every level is `Bold`
/// (only `body` is `Regular`): a heading ramp is a size scale, not a weight
/// scale, and mixing the two variables would make "is this a heading?" a
/// question about two fields instead of one.
///
/// Declared once and shared by both shipped themes, because typography is
/// type, not colour: a heading's size does not change when the operator
/// turns the lights off, the same reasoning [`SPACING_RAMP`] and
/// [`SHAPE_RAMP`] use for their own families.
const TYPOGRAPHY_RAMP: [(&str, f32, f32, TypographyWeight); 4] = [
    ("typography.body", 14.0, 20.0, TypographyWeight::Regular),
    ("typography.heading-sm", 16.0, 22.0, TypographyWeight::Bold),
    ("typography.heading", 20.0, 28.0, TypographyWeight::Bold),
    ("typography.heading-lg", 28.0, 36.0, TypographyWeight::Bold),
];

/// Assign every [`TYPOGRAPHY_RAMP`] step into a theme's value map, for the
/// same drift-proofing reason [`insert_spacing_ramp`] exists.
fn insert_typography_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, size, line_height, weight) in TYPOGRAPHY_RAMP {
        values.insert(
            name(step),
            TokenValue::Typography(TypographyValue {
                size,
                line_height,
                weight,
            }),
        );
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
        .declare(DesignToken::new(
            name("surface.layer-one"),
            TokenKind::Color,
        ))
        .declare(DesignToken::new(
            name("surface.layer-two"),
            TokenKind::Color,
        ))
        .declare(DesignToken::new(
            name("surface.layer-three"),
            TokenKind::Color,
        ))
        .declare(DesignToken::new(name(RAISED_ALIAS), TokenKind::Color))
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
            name("typography.heading-sm"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(
            name("typography.heading"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(
            name("typography.heading-lg"),
            TokenKind::Typography,
        ))
        .declare(DesignToken::new(name("motion.fast"), TokenKind::Motion))
        .declare(DesignToken::new(name("motion.slow"), TokenKind::Motion))
        .declare(DesignToken::new(
            name("motion.spatial.fast"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("motion.spatial.default"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("motion.spatial.slow"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("motion.effects.fast"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("motion.effects.default"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("motion.effects.slow"),
            TokenKind::Spring,
        ))
        .declare(DesignToken::new(
            name("shape.corner-none"),
            TokenKind::Shape,
        ))
        .declare(DesignToken::new(name("shape.corner-sm"), TokenKind::Shape))
        .declare(DesignToken::new(name("shape.corner-md"), TokenKind::Shape))
        .declare(DesignToken::new(name("shape.corner-lg"), TokenKind::Shape))
        .declare(DesignToken::new(
            name("shape.corner-full"),
            TokenKind::Shape,
        ))
        // The outline family (FR-015). A corner radius spans square to
        // circle and stops there; these are the two figures beyond it, plus
        // the name for the box itself. See `crate::token::value::Silhouette`.
        .declare(DesignToken::new(
            name("shape.silhouette-rect"),
            TokenKind::Silhouette,
        ))
        .declare(DesignToken::new(
            name("shape.silhouette-triangle"),
            TokenKind::Silhouette,
        ))
        .declare(DesignToken::new(
            name("shape.silhouette-diamond"),
            TokenKind::Silhouette,
        ))
        // The keyboard focus ring (FR-015, FR-025). Two colours, because the
        // ring is an ink band flanked by two paper halos: see
        // `crate::token::focus` for why one band cannot be enough, and
        // `the_focus_ring_is_visible_over_any_surface` below for the
        // measurement that holds the pair to it.
        .declare(DesignToken::new(name(RING_TOKEN), TokenKind::Color))
        .declare(DesignToken::new(name(HALO_TOKEN), TokenKind::Color))
        // The glyph coverage curve: how many times a glyph's rasterised
        // coverage is composited against itself, plus whether its origin
        // snaps to a whole pixel. The port of ai-macs' `GORGON_TEXT_PASSES`
        // and `GORGON_TEXT_SNAP` env knobs into a token
        // (`ignored/builds/2026-08-24-text-pipeline-port/SPEC.md` §6). See
        // `crate::token::value::CoverageValue` for both fields' full
        // rationale, including the `snap`-to-`subpixel_binning` inversion a
        // binder must apply.
        .declare(DesignToken::new(
            name("text.coverage-curve"),
            TokenKind::Coverage,
        ));

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

    insert_layer_set(&mut values, &LIGHT_LAYERS);
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0x1a, 0x1a, 0x1a, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0x44, 0x44, 0x44, 0xff)),
    );
    insert_spacing_ramp(&mut values);
    insert_typography_ramp(&mut values);
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
    insert_spring_set(&mut values);
    insert_shape_ramp(&mut values);
    insert_silhouette_family(&mut values);

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

    values.insert(
        name("text.coverage-curve"),
        TokenValue::Coverage(CoverageValue {
            passes: 2.0,
            snap: false,
        }),
    );

    Theme::build(ThemeMode::Light, &vocab, values).expect("shipped light theme must be complete")
}

/// The shipped dark theme. See [`light`] for the completeness note.
#[must_use]
pub fn dark() -> Theme {
    let vocab = standard_vocabulary();
    let mut values = BTreeMap::new();

    insert_layer_set(&mut values, &DARK_LAYERS);
    values.insert(
        name("text.primary"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0xf2, 0xf2, 0xff)),
    );
    values.insert(
        name("text.muted"),
        TokenValue::Color(ColorValue::from_srgb8(0xd4, 0xd4, 0xd4, 0xff)),
    );
    insert_spacing_ramp(&mut values);
    insert_typography_ramp(&mut values);
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
    insert_spring_set(&mut values);
    insert_shape_ramp(&mut values);
    insert_silhouette_family(&mut values);

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

    values.insert(
        name("text.coverage-curve"),
        TokenValue::Coverage(CoverageValue {
            passes: 2.0,
            snap: false,
        }),
    );

    Theme::build(ThemeMode::Dark, &vocab, values).expect("shipped dark theme must be complete")
}

#[cfg(test)]
mod tests {
    use super::{LAYER_TOKENS, RAISED_ALIAS, SPRING_SET, dark, light, standard_vocabulary};
    use crate::token::ThemeMode;
    use crate::token::focus::{HALO_TOKEN, RING_TOKEN};
    use crate::token::name::TokenName;
    use crate::token::value::{ColorValue, CoverageValue, TokenValue};

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

    /// Viénot-Brettel-Mollon 1999 reduced matrices, applied to *linear* RGB.
    /// Deuteranopia (no green cone) and protanopia (no red cone) are both
    /// simulated because "red-green colour blind" covers both and they do not
    /// collapse the same pairs.
    ///
    /// These are not transcribed from a blog post. They are the composition
    /// `LMS→RGB · reduce · RGB→LMS` of the three matrices the 1999 paper
    /// publishes (Eqs. 4-6), and
    /// [`the_simulation_matrices_are_the_ones_the_paper_derives`] recomposes
    /// them from those equations and fails on a changed digit. The matrix
    /// this file shipped before was the "colorjack ColorMatrix" deuteranope
    /// matrix (`0.625/0.375`, `0.700/0.300`, `0/0.300/0.700`), whose own
    /// author disclaims it as inaccurate, plus an untraceable `1/6, 5/6`
    /// protanope matrix. That pair did not simulate what the comment claimed:
    /// pure red and pure green stayed ΔE\*ab 88.4 apart under it, against
    /// 30.4 under the real transform, so the separation gate was scoring the
    /// palette for a reader who can still tell red from green.
    const DEUTERANOPE: [[f32; 3]; 3] = [
        [0.29275, 0.70725, 0.0],
        [0.29275, 0.70725, 0.0],
        [-0.02234, 0.02234, 1.0],
    ];
    const PROTANOPE: [[f32; 3]; 3] = [
        [0.11238, 0.88762, 0.0],
        [0.11238, 0.88762, 0.0],
        [0.00401, -0.00401, 1.0],
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

    /// The two simulation matrices, recomputed from the paper rather than
    /// trusted as constants.
    ///
    /// The pair this file used to ship was mislabelled: the comment named
    /// Viénot-Brettel-Mollon and the numbers were something else, and nothing
    /// in the suite could tell, because a 3x3 of plausible-looking decimals
    /// reads the same whether it models the eye or not. So this test does not
    /// assert *about* the matrices, it rebuilds them: it starts from the
    /// three matrices the 1999 paper publishes and composes them, and the
    /// shipped constants have to be what falls out.
    ///
    /// Method (Viénot, Brettel & Mollon 1999, "Digital video colourmaps for
    /// checking the legibility of displays by dichromats", Eqs. 4-6):
    ///
    /// 1. take linear display RGB into LMS cone responses ([`RGB_TO_LMS`]);
    /// 2. rebuild the missing cone's response from the two that survive,
    ///    which projects the colour onto the diagonal plane through the
    ///    neutral axis and the deficiency's anchor wavelength;
    /// 3. come back to linear RGB.
    ///
    /// Steps 1 and 3 are one matrix and its inverse, so only [`RGB_TO_LMS`]
    /// and the two reductions are typed in here; the inverse is solved for.
    /// A wrong digit anywhere in [`DEUTERANOPE`] or [`PROTANOPE`] fails this
    /// test instead of quietly animating the accessibility gate.
    #[test]
    fn the_simulation_matrices_are_the_ones_the_paper_derives() {
        type M3 = [[f64; 3]; 3];

        /// Eq. 4: linear RGB to LMS on the Smith-Pokorny fundamentals.
        const RGB_TO_LMS: M3 = [
            [17.8824, 43.5161, 4.11935],
            [3.45565, 27.1554, 3.86714],
            [0.029_956_6, 0.184_309, 1.46709],
        ];
        /// Eq. 5: the protanope has no L cone, so L is rebuilt from M and S.
        const PROTAN_REDUCE: M3 = [[0.0, 2.02344, -2.52581], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        /// Eq. 6: the deuteranope has no M cone, so M is rebuilt from L and S.
        const DEUTAN_REDUCE: M3 = [[1.0, 0.0, 0.0], [0.494_207, 0.0, 1.24827], [0.0, 0.0, 1.0]];

        fn mul(a: &M3, b: &M3) -> M3 {
            std::array::from_fn(|i| {
                std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum())
            })
        }

        /// Cofactor inverse. Three-by-three, so the closed form is shorter
        /// than any elimination loop and has no pivot choices to get wrong.
        fn invert(m: &M3) -> M3 {
            let c: M3 = std::array::from_fn(|i| {
                std::array::from_fn(|j| {
                    let (r, s) = ((i + 1) % 3, (i + 2) % 3);
                    let (u, v) = ((j + 1) % 3, (j + 2) % 3);
                    m[r][u] * m[s][v] - m[r][v] * m[s][u]
                })
            });
            let det: f64 = (0..3).map(|j| m[0][j] * c[0][j]).sum();
            assert!(det.abs() > 1e-9, "RGB_TO_LMS is singular");
            // Transpose of the cofactor matrix over the determinant.
            std::array::from_fn(|i| std::array::from_fn(|j| c[j][i] / det))
        }

        let lms_to_rgb = invert(&RGB_TO_LMS);
        // The inverse has to actually invert, or the composition below is
        // measuring nothing.
        let identity = mul(&lms_to_rgb, &RGB_TO_LMS);
        for (i, row) in identity.iter().enumerate() {
            for (j, got) in row.iter().enumerate() {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (got - want).abs() < 1e-9,
                    "the LMS inverse is wrong at [{i}][{j}]: {got}"
                );
            }
        }

        for (vision, reduce, shipped) in [
            ("deuteranope", &DEUTAN_REDUCE, &DEUTERANOPE),
            ("protanope", &PROTAN_REDUCE, &PROTANOPE),
        ] {
            let derived = mul(&lms_to_rgb, &mul(reduce, &RGB_TO_LMS));
            for i in 0..3 {
                for j in 0..3 {
                    assert!(
                        (derived[i][j] - f64::from(shipped[i][j])).abs() < 5e-5,
                        "{vision} row {i} column {j}: the shipped matrix says \
                         {}, composing the 1999 paper's Eqs. 4-6 gives {:.5}. \
                         The constant is not the transform it claims to be.",
                        shipped[i][j],
                        derived[i][j],
                    );
                }
            }

            // The same property, measured through [`simulate`] rather than
            // read off the constants, because [`simulate`] is what the gate
            // below actually calls. The composition check above cannot see a
            // bug that lives in the application — a transposed index, a
            // stray gamma step — and such a bug moves every ΔE in the suite
            // without moving a single digit in this file.
            //
            // The property: the reduced model projects onto the diagonal
            // plane through the neutral axis and the deficiency's anchor,
            // and on that plane the red and green outputs are equal. A
            // dichromat's surviving cones cannot pull those two apart, so an
            // unequal pair means the simulation is handing the reader a
            // discrimination they do not have.
            for step in 0..=8 {
                let t = step as f32 / 8.0;
                for probe in [
                    ColorValue {
                        r: t,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    ColorValue {
                        r: 0.0,
                        g: t,
                        b: 0.0,
                        a: 1.0,
                    },
                    ColorValue {
                        r: 0.0,
                        g: 0.0,
                        b: t,
                        a: 1.0,
                    },
                    ColorValue {
                        r: t,
                        g: 1.0 - t,
                        b: 0.5,
                        a: 1.0,
                    },
                ] {
                    let out = simulate(probe, shipped);
                    assert!(
                        (out[0] - out[1]).abs() < 1e-4,
                        "{vision}: simulating {probe:?} gives red {} and \
                         green {}, which is off the model's diagonal plane. \
                         The matrix is right, so the fault is in how it is \
                         applied.",
                        out[0],
                        out[1],
                    );
                }
            }
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

    /// CIE L\*, the perceived-lightness axis, from a linear-light colour.
    /// Shares [`to_lab`]'s D65 white and its `f` companion; separate because
    /// the layer tests want lightness alone and ΔE would drown a 0.4 spread
    /// in chroma noise.
    fn lightness(c: ColorValue) -> f32 {
        to_lab([c.r, c.g, c.b])[0]
    }

    /// The dark layer set climbs, and climbs by an even amount.
    ///
    /// Two separate claims and both matter. **Monotone** is the difference
    /// between a dark layer set and a light one: a dark theme steps one way
    /// and a light theme alternates, and building either with the other's
    /// algorithm produces something that passes every contrast check and
    /// still reads wrong.
    ///
    /// **Even** is the claim that makes the set a depth cue rather than four
    /// greys. Two panels nested two deep should read as the same amount of
    /// "further forward" as two nested one deep, and that is a statement
    /// about perceived lightness. The measurement is therefore in CIE L\* and
    /// not in hex: `#121212 → #222222 → #333333 → #444444` is even in hex by
    /// inspection, but so is any other fixed hex interval, and hex is not
    /// linear in perceived lightness. Carbon Gray 100's ramp is the
    /// counter-example — it steps 7.91, 8.81, 10.91 in L\*, a spread of 2.99,
    /// so its deepest layers separate a third harder than its shallowest.
    #[test]
    fn the_dark_layer_set_steps_evenly_in_perceived_lightness() {
        let theme = dark();
        let ls: Vec<f32> = LAYER_TOKENS
            .iter()
            .map(|t| lightness(theme_color(&theme, t)))
            .collect();

        let steps: Vec<f32> = ls.windows(2).map(|w| w[1] - w[0]).collect();
        for (i, step) in steps.iter().enumerate() {
            assert!(
                *step > 0.0,
                "dark {} -> {} falls by {step:.2} in L*; a dark layer set \
                 steps lighter every time, and a set that reverses reads as \
                 two unrelated pairs rather than one stack",
                LAYER_TOKENS[i],
                LAYER_TOKENS[i + 1],
            );
        }

        let (lo, hi) = steps
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), s| (lo.min(*s), hi.max(*s)));
        assert!(
            hi - lo < 1.0,
            "the dark layer steps are {steps:?} in L*, a spread of {:.2}. \
             Even steps are what makes the set a depth cue: one layer of \
             nesting has to look like one layer of nesting wherever it \
             happens. For scale, Carbon Gray 100 spreads 2.99 and is the \
             ramp this set deliberately does not copy.",
            hi - lo,
        );
    }

    /// The light layer set alternates between two colours and does not ramp.
    ///
    /// The whole reason this test exists as its own function: a light theme
    /// built as a monotonic ramp passes every contrast assertion in this
    /// file and is still wrong. Nothing else here can catch it, because the
    /// defect is structural rather than numeric — by layer three a ramped
    /// light theme is a mid-grey theme, and `text.primary`, chosen against
    /// near-white, has quietly stopped being the right ink.
    #[test]
    fn the_light_layer_set_alternates_rather_than_ramping() {
        let theme = light();
        let colors: Vec<ColorValue> = LAYER_TOKENS
            .iter()
            .map(|t| theme_color(&theme, t))
            .collect();

        assert_eq!(
            colors[0], colors[2],
            "light {} and {} must be the same colour: a light layer set \
             alternates between two greys, and depth is read from the change \
             at a boundary rather than from a direction",
            LAYER_TOKENS[0], LAYER_TOKENS[2],
        );
        assert_eq!(
            colors[1], colors[3],
            "light {} and {} must be the same colour",
            LAYER_TOKENS[1], LAYER_TOKENS[3],
        );
        assert!(
            (lightness(colors[0]) - lightness(colors[1])).abs() > 1.0,
            "the light set's two greys are only {:.2} apart in L*; an \
             alternation nobody can see is one surface with two names",
            (lightness(colors[0]) - lightness(colors[1])).abs(),
        );
    }

    /// `surface.raised` resolves to layer one, in both themes.
    ///
    /// The alias exists because `gorgon-petra-egui` binds the pre-layer-set
    /// name in about thirty places, and this change does not own that crate.
    /// Two names for one colour is drift waiting to happen, so the invariant
    /// that makes it safe is asserted rather than commented: when the painter
    /// migrates to the ordinal name, this test and [`RAISED_ALIAS`] are
    /// deleted together.
    #[test]
    fn the_raised_alias_is_layer_one_in_both_themes() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            assert_eq!(
                theme_color(&theme, RAISED_ALIAS),
                theme_color(&theme, LAYER_TOKENS[1]),
                "{label}: {RAISED_ALIAS} has drifted from {}; it is a \
                 migration alias for layer one, not a fifth layer",
                LAYER_TOKENS[1],
            );
        }
    }

    /// How deep a status marker may be painted, per theme, measured.
    ///
    /// A status colour was tuned against one surface, and the layer set has
    /// four. This pins the boundary in **both** directions: every layer up to
    /// and including the index below carries all three status colours at
    /// [`MIN_STATUS_SEPARATION`]'s sibling floor, and the first layer past it
    /// carries at least one that does not.
    ///
    /// Asserting the far side is the point. The near side alone would let a
    /// future palette edit silently gain or lose a layer of depth, and the
    /// painter needs the number: **in dark mode a status marker may sit on
    /// the ground and on layer one, and nowhere deeper.** `status.down`
    /// misses on `surface.layer-two` by 0.02 — 2.98:1 against a 3.0 floor —
    /// which is close enough that it will read as a rounding accident to
    /// anyone who meets it without this test to point at.
    ///
    /// This is a legibility floor, not the colour channel. FR-015's meaning
    /// travels on `StatusShape`'s silhouette and its word, both of which
    /// survive any background. What fails past the boundary is *seeing the
    /// marker at all*, which is why it is a floor rather than a preference.
    const DEEPEST_STATUS_LAYER: [(&str, usize); 2] = [("light", 3), ("dark", 1)];

    #[test]
    fn a_status_marker_is_legible_only_down_to_its_theme_s_deepest_layer() {
        const STATUSES: [&str; 3] = ["status.ok", "status.degraded", "status.down"];
        for (label, deepest) in DEEPEST_STATUS_LAYER {
            let theme = if label == "light" { light() } else { dark() };
            let colors: Vec<ColorValue> = STATUSES.iter().map(|s| theme_color(&theme, s)).collect();

            for (depth, token) in LAYER_TOKENS.iter().enumerate() {
                let layer = theme_color(&theme, token);
                let worst = STATUSES
                    .iter()
                    .zip(&colors)
                    .map(|(n, c)| (contrast(*c, layer), *n))
                    .fold((f32::MAX, ""), |acc, x| if x.0 < acc.0 { x } else { acc });

                if depth <= deepest {
                    assert!(
                        worst.0 >= MIN_SURFACE_CONTRAST,
                        "{label}/{token} is declared status-bearing (depth \
                         {depth} <= {deepest}) but {} is only {:.2}:1 on it, \
                         under the {MIN_SURFACE_CONTRAST} floor. Either the \
                         palette moved or the layer did.",
                        worst.1,
                        worst.0,
                    );
                } else {
                    assert!(
                        worst.0 < MIN_SURFACE_CONTRAST,
                        "{label}/{token} (depth {depth}) now carries every \
                         status — the worst is {} at {:.2}:1. That is an \
                         improvement, and it means DEEPEST_STATUS_LAYER is \
                         stale: raise {label} to {depth} so the painter is \
                         allowed to use the depth it just gained.",
                        worst.1,
                        worst.0,
                    );
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

    /// A spring is physics, so both themes must name the same one.
    ///
    /// The spacing sibling of `the_two_shipped_themes_agree_on_every_gap`,
    /// and for the same reason: colour is the only channel `light` and
    /// `dark` are allowed to disagree on. A panel that slides in faster
    /// because the operator switched to dark mode is a bug that no capture
    /// would ever show, because a capture is a still.
    #[test]
    fn the_two_shipped_themes_agree_on_every_spring() {
        let (l, d) = (light(), dark());
        let vocab = standard_vocabulary();
        let mut checked = 0usize;
        for name in vocab.names() {
            if vocab.kind_of(name) != Some(crate::token::value::TokenKind::Spring) {
                continue;
            }
            assert_eq!(
                l.value(name),
                d.value(name),
                "{name} differs between light and dark; a spring is physics, not colour"
            );
            checked += 1;
        }
        assert_eq!(
            checked,
            SPRING_SET.len(),
            "expected {} spring tokens, checked {checked}",
            SPRING_SET.len(),
        );
    }

    /// The corner ramp, pinned by value. `none` and `full` are the two ends
    /// two steps could not reach; `sm` and `lg` are unchanged from the
    /// pre-ramp vocabulary so nothing already bound to either name moves.
    #[test]
    fn the_shape_ramp_is_five_ordered_steps_from_sharp_to_pill() {
        let expected: [(&str, f32); 5] = [
            ("shape.corner-none", 0.0),
            ("shape.corner-sm", 4.0),
            ("shape.corner-md", 8.0),
            ("shape.corner-lg", 12.0),
            ("shape.corner-full", 999.0),
        ];
        let theme = light();
        let mut previous = -1.0f32;
        for (step, radius) in expected {
            let got = theme.value(&TokenName::new(step).unwrap());
            assert_eq!(
                got,
                Some(&TokenValue::Shape(crate::token::value::ShapeValue {
                    corner_radius: radius
                })),
                "{step} must be {radius} logical units, found {got:?}"
            );
            assert!(
                radius > previous,
                "the ramp must grow: {step} at {radius} does not exceed {previous}"
            );
            previous = radius;
        }

        // Every shape name the vocabulary declares is one of the five above,
        // so a sixth step cannot appear without this test naming it.
        let vocab = standard_vocabulary();
        let declared: Vec<&TokenName> = vocab
            .names()
            .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Shape))
            .collect();
        assert_eq!(
            declared.len(),
            expected.len(),
            "the vocabulary declares {} shape tokens but the ramp has {}: {declared:?}",
            declared.len(),
            expected.len()
        );
    }

    /// Corner radius is geometry, the same reasoning
    /// [`the_two_shipped_themes_agree_on_every_gap`] applies to spacing: a
    /// rounded corner does not change shape when the operator turns the
    /// lights off, so light and dark must agree on every step.
    /// The outline family, pinned by value in both themes.
    ///
    /// A silhouette is geometry, not colour: a "degraded" marker is a
    /// triangle with the lights on and a triangle with them off, the same
    /// reasoning [`the_two_shipped_themes_agree_on_every_corner`] applies
    /// to a corner radius. The membership assertion at the end is what
    /// stops a fourth figure appearing without this test naming it — and
    /// without `gorgon-petra-egui`'s painter, which matches on this enum
    /// exhaustively, being asked to draw it.
    #[test]
    fn the_silhouette_family_is_three_figures_pinned_in_both_themes() {
        use crate::token::value::Silhouette;
        let expected: [(&str, Silhouette); 3] = [
            ("shape.silhouette-rect", Silhouette::Rect),
            ("shape.silhouette-triangle", Silhouette::Triangle),
            ("shape.silhouette-diamond", Silhouette::Diamond),
        ];
        let (l, d) = (light(), dark());
        for (token, figure) in expected {
            let n = TokenName::new(token).unwrap();
            assert_eq!(
                l.value(&n),
                Some(&TokenValue::Silhouette(figure)),
                "{token} must be {figure:?} in the light theme, found {:?}",
                l.value(&n)
            );
            assert_eq!(
                d.value(&n),
                l.value(&n),
                "{token} differs between light and dark; a silhouette is geometry, not colour"
            );
        }

        let vocab = standard_vocabulary();
        let declared: Vec<&TokenName> = vocab
            .names()
            .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Silhouette))
            .collect();
        assert_eq!(
            declared.len(),
            expected.len(),
            "the vocabulary declares {} silhouette tokens but the family has {}: {declared:?}",
            declared.len(),
            expected.len()
        );
    }

    #[test]
    fn the_two_shipped_themes_agree_on_every_corner() {
        let (l, d) = (light(), dark());
        let vocab = standard_vocabulary();
        let mut checked = 0usize;
        for name in vocab.names() {
            if vocab.kind_of(name) != Some(crate::token::value::TokenKind::Shape) {
                continue;
            }
            assert_eq!(
                l.value(name),
                d.value(name),
                "{name} differs between light and dark; corner radius is geometry, not colour"
            );
            checked += 1;
        }
        assert_eq!(checked, 5, "expected the five-step ramp, checked {checked}");
    }

    /// The typography ramp, pinned by value. `body` and `heading` keep the
    /// sizes the pre-ramp vocabulary shipped; `heading-sm`/`heading-lg` are
    /// the new levels, added around `heading` the same way the corner ramp
    /// added `none`/`md`/`full` around its own unchanged `sm`/`lg`.
    #[test]
    fn the_typography_ramp_is_body_and_three_ordered_heading_levels() {
        use crate::token::value::TypographyWeight;
        let expected: [(&str, f32, f32, TypographyWeight); 4] = [
            ("typography.body", 14.0, 20.0, TypographyWeight::Regular),
            ("typography.heading-sm", 16.0, 22.0, TypographyWeight::Bold),
            ("typography.heading", 20.0, 28.0, TypographyWeight::Bold),
            ("typography.heading-lg", 28.0, 36.0, TypographyWeight::Bold),
        ];
        let theme = light();
        for (step, size, line_height, weight) in expected {
            let got = theme.value(&TokenName::new(step).unwrap());
            assert_eq!(
                got,
                Some(&TokenValue::Typography(
                    crate::token::value::TypographyValue {
                        size,
                        line_height,
                        weight,
                    }
                )),
                "{step} must be size {size}, found {got:?}"
            );
        }

        // The three heading levels are a size hierarchy: each strictly
        // larger than the last, so "heading" without a suffix is
        // meaningfully the middle of three rather than a name with no
        // siblings.
        let sizes = [16.0_f32, 20.0, 28.0];
        for pair in sizes.windows(2) {
            assert!(
                pair[1] > pair[0],
                "heading levels must grow: {} does not exceed {}",
                pair[1],
                pair[0]
            );
        }

        let vocab = standard_vocabulary();
        let declared: Vec<&TokenName> = vocab
            .names()
            .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Typography))
            .collect();
        assert_eq!(
            declared.len(),
            expected.len(),
            "the vocabulary declares {} typography tokens but the ramp has {}: {declared:?}",
            declared.len(),
            expected.len()
        );
    }

    /// Typography is type, not colour: a heading's size does not change
    /// when the operator turns the lights off, the same reasoning
    /// [`the_two_shipped_themes_agree_on_every_gap`] and
    /// [`the_two_shipped_themes_agree_on_every_corner`] apply to spacing
    /// and shape.
    #[test]
    fn the_two_shipped_themes_agree_on_every_typography_level() {
        let (l, d) = (light(), dark());
        let vocab = standard_vocabulary();
        let mut checked = 0usize;
        for name in vocab.names() {
            if vocab.kind_of(name) != Some(crate::token::value::TokenKind::Typography) {
                continue;
            }
            assert_eq!(
                l.value(name),
                d.value(name),
                "{name} differs between light and dark; typography is type, not colour"
            );
            checked += 1;
        }
        assert_eq!(
            checked, 4,
            "expected the four-entry ramp, checked {checked}"
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

    /// SPEC.md §6.2's "same value in both themes" argument, checked rather
    /// than trusted: `light` and `dark` are two independent value maps, and
    /// nothing but this test stops one of them drifting to a different
    /// pass count or a different snap setting the way
    /// [`the_two_shipped_themes_agree_on_every_gap`] guards the spacing
    /// ramp.
    #[test]
    fn both_shipped_themes_assign_the_coverage_curve_the_same_passes_and_snap() {
        let token = TokenName::new("text.coverage-curve").unwrap();
        for (label, theme) in [("light", light()), ("dark", dark())] {
            assert_eq!(
                theme.value(&token),
                Some(&TokenValue::Coverage(CoverageValue {
                    passes: 2.0,
                    snap: false
                })),
                "{label} theme's text.coverage-curve must be passes: 2.0, snap: false"
            );
        }
    }

    /// Every text tone must stay readable on every surface it can land on,
    /// and must stay a *visible step* away from the tone above it.
    ///
    /// # Why this test exists
    ///
    /// It was written after a bug report that read "the text is greying out
    /// from the background", and it is the check that would have caught that
    /// report before it was filed. The reported defect was chased through
    /// the glyph rasteriser first — coverage curves, sub-pixel binning,
    /// hinting targets — and none of it was the cause. The cause was a
    /// token: dark `text.muted` was `#a3a3a3`, which is **3.86:1** against
    /// `surface.layer-three` (`#444444`), below the 4.5:1 AA floor for body
    /// text. Text set in it did not "grey out" because of how it was
    /// rasterised. It was grey.
    ///
    /// The lesson the test encodes: a tone is not judged on its own, it is
    /// judged against every ground it can be painted on. `text.muted` reads
    /// fine on `surface.base` and fails four layers up, and nothing in the
    /// theme's own construction notices, because a theme assigns colours one
    /// at a time and legibility is a property of pairs.
    ///
    /// # The two floors, and why the second one is needed
    ///
    /// [`MIN_TEXT_CONTRAST`] is WCAG AA for body text. On its own it is
    /// gameable in the laziest possible way: set `text.muted` to
    /// `text.primary` and every ratio passes. So the second assertion pins
    /// the *step* — muted must be meaningfully quieter than primary, or the
    /// type ramp has one tone with two names and every "de-emphasised" label
    /// on every page silently shouts.
    ///
    /// The two themes are tuned to *different* steps on purpose — see
    /// [`DARK_STEP`] inside the test for why one number could not serve
    /// both, and what it cost to find that out.
    #[test]
    fn every_text_tone_clears_aa_on_every_surface_it_can_be_painted_on() {
        /// WCAG 2.x AA for body text. Not AAA (7:1): that floor would force
        /// `text.muted` so close to `text.primary` on the deepest layer that
        /// the step assertion below could not also hold.
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        /// The de-emphasis step each theme is allowed, as `primary:muted`
        /// contrast on one ground.
        ///
        /// **Per theme, and deliberately not the same number.** The first
        /// version of this gate held both themes to one band, because one
        /// band is tidier and because a reader switching themes should get
        /// the same hierarchy. That reasoning was right and the
        /// implementation of it was wrong: a contrast *ratio* is not a
        /// perceptual unit, and light-on-dark blooms in a way dark-on-light
        /// does not. Holding dark to light's step put dark `text.muted` at
        /// `#b7b7b7`, which cleared every arithmetic floor and was reported
        /// as hard to read on the screen it actually ships on.
        ///
        /// So the bands are tuned per theme against what each one looks
        /// like, and the *shipped* values sit inside them rather than the
        /// bands being derived from a formula. Dark needs the smaller step
        /// to read as equally quiet. Neither band is a measurement; each is
        /// a fence a few points either side of a judged value, there to
        /// catch drift rather than to define taste.
        const DARK_STEP: std::ops::RangeInclusive<f32> = 1.20..=1.60;
        /// See [`DARK_STEP`]. Light tolerates — and needs — the wider step.
        const LIGHT_STEP: std::ops::RangeInclusive<f32> = 1.55..=2.10;

        for (label, theme, step_band) in
            [("light", light(), LIGHT_STEP), ("dark", dark(), DARK_STEP)]
        {
            let primary = theme_color(&theme, "text.primary");
            let muted = theme_color(&theme, "text.muted");

            for surface in LAYER_TOKENS.iter().chain(std::iter::once(&RAISED_ALIAS)) {
                let ground = theme_color(&theme, surface);
                for (tone_name, tone) in [("text.primary", primary), ("text.muted", muted)] {
                    let ratio = contrast(tone, ground);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{label}: {tone_name} on {surface} is {ratio:.2}:1, \
                         below the {MIN_TEXT_CONTRAST}:1 AA floor for body \
                         text. A reader sees this as text that has greyed \
                         out, and no amount of glyph rasterisation fixes a \
                         tone that is genuinely too close to its ground."
                    );
                }

                let step = contrast(primary, ground) / contrast(muted, ground);
                assert!(
                    step_band.contains(&step),
                    "{label}: on {surface} the primary:muted step is \
                     {step:.2}x, outside {step_band:?}. Too small and the \
                     ramp has one tone under two names; too large and the \
                     quiet tone is the one this test exists to keep readable."
                );
            }
        }
    }
}
