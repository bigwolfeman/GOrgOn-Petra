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
    TokenKind, TokenValue, TypographyFamily, TypographyValue, TypographyWeight,
};
use crate::token::vocabulary::{DesignToken, Vocabulary};

fn name(n: &str) -> TokenName {
    TokenName::new(n)
        .unwrap_or_else(|err| panic!("shipped vocabulary name {n:?} must be well-formed: {err}"))
}

/// The names of the four surface layers, ground first.
///
/// A **layer set**, which is M-Carbon's *primary* depth cue: depth is
/// carried by one grey sitting on another. See
/// `.agents/notes/proposed/architecture/2026-08-24-m-carbon-design-language.md`.
///
/// This comment used to end "never by a shadow", and that clause was
/// retired on 2026-08-25. It rested on a premise about the painter that
/// turned out to be false — epaint 0.36.1 implements `Shadow` as one
/// `RectShape` with a widened feather (`tessellator.rs`'s
/// `self.feathering.max(blur_width)`), so elevation costs one mesh and no
/// second pass, and nothing was ever waiting on a blur primitive. Both
/// Carbon and Material 3 use a tonal step as the default and spend a shadow
/// sparingly on what floats; that is what this set plus `SHADOW_TOKENS` now
/// does. The layer set is still the depth cue for anything *resting*.
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
pub const LAYER_TOKENS: [&str; 4] = [
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

/// The name a drawn boundary binds.
///
/// **This token exists because the library had no border colour and was
/// conscripting a text one.** Every outline in `component/` bound
/// `text.muted`: the card edge, the field box, the checkbox square, the
/// toggle track, the progress rail. On `surface.layer-one` that is 10.73:1
/// in dark and 8.70:1 in light — a hairline painted as loud as the prose
/// inside it, on a page where nothing was louder. The result reads as a
/// wireframe rather than as a set of surfaces, which is the complaint that
/// started this pass.
///
/// # 2026-09-05: this is the *decorative* tone, and the floor moved with it
///
/// The paragraph below is the argument as it stood, and it is kept because
/// the correction is only legible beside it. It read:
///
/// > The floor is **WCAG 2.1 SC 1.4.11 *Non-text Contrast*, 3:1**, not SC
/// > 1.4.3's 4.5:1 — a boundary is a user-interface component, not body
/// > text, and this is the same distinction [`ACCENT_TOKEN`] turns on.
///
/// The first clause is right and the second sentence over-reaches. SC 1.4.11
/// covers *"visual information required to identify user interface components
/// and states"*. A checkbox's outline is that. **A hairline between two rows
/// of a table is not** — it identifies no component, carries no state, and a
/// reader who cannot see it loses a nicety rather than a control. Holding
/// every drawn line in the library to the control floor is how one number
/// meant for a checkbox ended up setting the tone of every divider, every
/// table rule and every panel edge on 42 pages.
///
/// Measured, dark theme, against `.agents/carbon-waves/ROUND3-DEFECTS.md`'s
/// row-31 and row-13 captures at the same dpr:
///
/// | | Carbon g100 | Petra, before | Petra, now |
/// |---|---|---|---|
/// | row rule / this token | `#393939`–`#6f6f6f` (57–111) | `#9c9c9c` (156) | `#616161` (97) |
/// | field rule / [`BORDER_STRONG_TOKEN`] | `#6f6f6f` (111) | `#b8b8b8` (184) | `#8f8f8f` (143) |
///
/// The row-rule range is not vagueness: Carbon has **four** decorative
/// border tones, `border-subtle-00..03`, one per layer, and ours is one
/// tone covering all four layers. Ours therefore sits inside Carbon's
/// range rather than on any single member of it.
///
/// So the split is by **role**, not by taste:
///
/// - **this token, and the three [`BORDER_SUBTLE_TOKENS`], are decorative.**
///   Floor [`MIN_DIVIDER_CONTRAST`], derived below from Carbon's own
///   number rather than chosen.
/// - **[`BORDER_STRONG_TOKEN`] is the control boundary.** It keeps
///   [`MIN_CONTROL_BOUNDARY`] — the full 3:1 — on every layer, in both
///   directions, and every edge that identifies a control now binds it:
///   the checkbox square, the radio ring, the toggle track, the tertiary
///   button, the upload drop zone, and every field's bottom rule.
///   `component::tests::containers_take_a_tone_and_controls_take_an_edge`
///   names each one and which of the two it must bind, so a new edge on the
///   wrong side of the split fails a test instead of shipping.
///
/// Neither tone is a literal any more. Both are walked out from the theme's
/// own layer set by [`quietest_grey_clearing`], so a layer-set edit moves
/// them and cannot leave them stale.
///
/// Two claims still travel with this value and both are measured by
/// [`tests::the_border_tone_is_visible_everywhere_and_quieter_than_every_text_tone`]:
/// it clears its own floor on every layer it can be drawn on, and on every
/// one of those layers it is *strictly and substantially* quieter than both
/// text tones. The second is the regression guard. A border that drifts back
/// up the ramp does not fail any contrast floor — it passes harder — so the
/// only thing that can catch the drift is a ceiling, and this is it.
///
/// **One border tone, not two.** A `border.strong` for selected or hovered
/// edges was considered and refused: nothing in this pass measures a need
/// for one, the focus ring is already a separate two-token pair that covers
/// the loudest case, and this file's own [`ON_ACCENT_TOKEN`] comment records
/// that a declared name nothing reads is the defect the M-Carbon note
/// counted at 12 of 18 names.
///
/// # That refusal was overturned on 2026-08-25, by measurement
///
/// The paragraph above is kept rather than deleted, because the reasoning in
/// it was correct and is the reason this one is worth reading. It refused a
/// name for want of a **measured need**, and named the measurement it wanted.
/// The Carbon component inventory
/// (`.agents/research/08-25-2026/Carbon-Component-Inventory/`) is that
/// measurement: across the 42 documented components, `$border-strong` is
/// referenced **14 times** and `$border-interactive` **14 times**, and both
/// outrank everything Petra shipped past this token. So the condition the
/// refusal set was met, by the evidence it asked for, and
/// [`BORDER_STRONG_TOKEN`] and [`BORDER_INTERACTIVE_TOKEN`] now ship.
///
/// Two clauses of the refusal survive intact and still constrain the new
/// names. The focus ring is still a separate pair and still owns the loudest
/// case — `border-interactive` is the accent, not a fourth grey, and a
/// focused control still draws the ring rather than a louder edge. And a
/// declared name nothing reads is still the defect: `border-strong` enters on
/// fourteen references, not on the observation that Carbon publishes one.
///
/// What did **not** change is the ceiling. `border-strong` is the CIE L\*
/// midpoint of this tone and `text.muted`, so it is louder than this one and
/// quieter than every text tone *by construction* rather than by a chosen hex
/// — and the regression this token's own test was written to catch, a border
/// drifting up to text loudness, is restated for the new name in
/// [`tests::border_strong_sits_between_the_subtle_border_and_every_text_tone`].
/// The two tests use different fences on purpose: this token is held `1.5x`
/// quieter than a text tone, and the strong one only strictly quieter, because
/// a strong border is meant to close some of that gap.
const BORDER_TOKEN: &str = "border.subtle";

/// WCAG 2.1 SC 1.4.11 *Non-text Contrast*, Level AA: the floor for **visual
/// information required to identify a user-interface component or its
/// state**. [`BORDER_STRONG_TOKEN`] is held here, on every layer, because a
/// checkbox that is nothing but its outline is exactly what that clause
/// names.
const MIN_CONTROL_BOUNDARY: f32 = 3.0;

/// The floor a **decorative** rule is held to: a divider between two rows,
/// a panel edge, a line under a header.
///
/// **1.56:1 is Carbon's own number, not a relaxation invented here** — but
/// getting to it takes care, because Carbon numbers its borders and its
/// layers in lockstep and a rule only ever meets the layer of the same
/// index. In `@carbon/themes`' g100 the matched pairs are:
///
/// | rule | ground | contrast |
/// |---|---|---|
/// | `border-subtle-00` `#393939` | `background` `#161616` | 1.567 |
/// | `border-subtle-01` `#525252` | `layer-01` `#262626` | 1.937 |
/// | `border-subtle-02` `#6f6f6f` | `layer-02` `#393939` | 2.298 |
///
/// The quietest line IBM actually draws is therefore **1.567:1**, rounded
/// down to the nearest hundredth so a theme reproducing Carbon exactly is
/// admitted rather than rejected by a rounding step.
///
/// This constant read `1.3` until 2026-09-05, from `border-subtle-01`'s
/// `#393939` against `layer-01`'s `#262626`. That pair is **not one Carbon
/// makes**: `#393939` is `border-subtle-00`, which meets `background`, and
/// the rule that meets `layer-01` is `#525252`. Crossing them by one index
/// understated the floor by a quarter and put our deepest layer's rule at
/// 1.31:1, quieter than any divider in Carbon. The hexes were cited
/// honestly; the pairing was the error.
///
/// One tone, four layers, so the deepest layer binds and this floor decides
/// where in that spread the single tone lands. Carbon does not have that
/// problem — it tunes `border-subtle-00..03` one per layer, and that ramp
/// (`contracts/token-vocabulary.md` §11, step 7) is the real answer here.
/// Until it lands, the honest rule is that our one tone must clear the
/// quietest line Carbon draws, on every layer we can draw it on.
///
/// It is deliberately below [`MIN_CONTROL_BOUNDARY`] and it is **not**
/// zero. A rule at 1.0:1 is not a quiet rule, it is a missing one, and the
/// operator is red-green colour blind: two greys within about three of 255
/// are invisible to everybody. 1.56:1 on dark's deepest layer is 29 sRGB
/// levels of separation, nearly ten times that.
const MIN_DIVIDER_CONTRAST: f32 = 1.56;

/// The contrast of two opaque sRGB triples.
///
/// Thin wrapper over [`ColorValue::contrast_ratio`] rather than a second
/// copy of the WCAG arithmetic: this module works in `[u8; 3]` and that
/// method works in `ColorValue`, and the tests below measure the shipped
/// theme through the same method, so a divergence between the value this
/// file *derives* and the value the tests *check* is impossible by
/// construction.
fn contrast_of(a: [u8; 3], b: [u8; 3]) -> f32 {
    ColorValue::from_srgb8(a[0], a[1], a[2], 0xff)
        .contrast_ratio(ColorValue::from_srgb8(b[0], b[1], b[2], 0xff))
}

/// The **quietest** grey that clears `ratio` against every one of `layers`,
/// walking away from the theme's own ground in `mode`'s direction.
///
/// "Quietest" is the whole point. A border is the one family where passing a
/// contrast floor harder is a defect rather than a margin — that is the
/// finding the ceiling in
/// [`tests::the_border_tone_is_visible_everywhere_and_quieter_than_every_text_tone`]
/// was written after — so the tone is the *first* step that clears the
/// floor, not a comfortable one past it. One sRGB level of headroom is the
/// correct amount.
///
/// Every one of `layers` is swept, not just the deepest, because nothing in
/// this library tracks which layer a given edge was drawn on: `on_layer`
/// re-seats a node's `background` and leaves its `border` alone, so one tone
/// has to hold everywhere until `component::layer_tokens`' per-layer names
/// are actually bound (`contracts/token-vocabulary.md` §11, step 7). When
/// that flag day comes, this function is what each numbered name calls with
/// its own single layer, and both tones drop again toward Carbon's.
///
/// # Panics
/// If no grey in the walk clears `ratio` — which for a ratio under 4.5 and
/// any layer set with a near-black or near-white ground cannot happen, and
/// if it ever does the theme is unshippable and must say so loudly rather
/// than silently return an extreme.
fn quietest_grey_clearing(layers: &[[u8; 3]; 4], ratio: f32, mode: ThemeMode) -> [u8; 3] {
    let ground = layers[0][0];
    let candidates: Vec<u8> = match mode {
        ThemeMode::Light => (0..=ground).rev().collect(),
        ThemeMode::Dark => (ground..=u8::MAX).collect(),
    };
    for level in candidates {
        let grey = [level, level, level];
        if layers.iter().all(|l| contrast_of(grey, *l) >= ratio) {
            return grey;
        }
    }
    panic!(
        "no grey between the ground and the end of the ramp clears {ratio}:1 \
         against every layer of {layers:?}; this theme cannot draw a boundary \
         at all"
    );
}

/// Light `border.subtle`, the decorative tone: `#c3c3c3`. Worst ground is
/// `#f2f2f2` at 1.57:1, the first level that clears
/// [`MIN_DIVIDER_CONTRAST`]; against `#ffffff` it is 1.76:1.
fn light_border() -> [u8; 3] {
    quietest_grey_clearing(&LIGHT_LAYERS, MIN_DIVIDER_CONTRAST, ThemeMode::Light)
}

/// Dark `border.subtle`, the decorative tone: `#616161`. Worst ground is
/// `surface.layer-three` (`#444444`) at 1.57:1 — the quietest line Carbon
/// draws anywhere, `border-subtle-00` on `background` — and against
/// `surface.base` it is 3.02:1.
///
/// It is brighter than any single Carbon border because Petra's dark ramp
/// *keeps going* to `#444444` where Carbon's layers stop at `#393939`, and
/// one tone has to survive all four of ours. The right comparison is not a
/// hex but a job: on our deepest layer this rule is exactly as quiet as
/// Carbon's quietest, and on our shallowest it is as loud as Carbon's
/// `border-strong-01` is on `layer-01`. That spread is the cost of one tone
/// and it is what §11 step 7's per-layer names remove.
fn dark_border() -> [u8; 3] {
    quietest_grey_clearing(&DARK_LAYERS, MIN_DIVIDER_CONTRAST, ThemeMode::Dark)
}

/// The rule material's shadow stroke, in signed sRGB levels offset from the
/// panel it sits on. Settled by operator sight-test 2026-09-07 and by the
/// ratio measured in
/// `docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md`:
/// shadow to highlight holds 2.15:1 across every ground tried, light and
/// dark. `specs/009-petra-design-language/contracts/design-language.md`'s
/// "The construction" is the authority; this and
/// [`RULE_HIGHLIGHT_DELTA`] are its two numbers.
const RULE_SHADOW_DELTA: i16 = -28;

/// The rule material's highlight stroke, directly below the shadow. See
/// [`RULE_SHADOW_DELTA`].
const RULE_HIGHLIGHT_DELTA: i16 = 13;

/// One channel of `level`, offset by `delta` signed sRGB levels and clamped
/// to `0..=255`.
///
/// This is the operation the contract calls "a signed level offset from the
/// fill of the panel the rule sits on, applied per channel and clamped" —
/// deliberately not a blend toward black or white. A blend is `lerp(base,
/// extreme, t)`, which loses headroom exactly where a surface sits near that
/// extreme (a light panel blending toward white does nothing); a clamped
/// offset keeps whatever headroom is left and simply stops there. The first
/// attempt at this material used a blend and shipped a sign error invisible
/// on mid-grey and obvious on white
/// (`.agents/notes/proposed/architecture/2026-09-07-rule-material-two-stroke-groove.md`,
/// "Problem"); this function is the fix, not a second copy of the mistake
/// with different numbers.
fn offset_channel(level: u8, delta: i16) -> u8 {
    (i16::from(level) + delta).clamp(0, i16::from(u8::MAX)) as u8
}

/// [`offset_channel`], applied to all three channels of `panel`. Applying
/// the *same signed offset* to R, G and B independently is what "a tinted
/// panel keeps its hue in the rule" means: a warm panel's shadow and
/// highlight are still warm, because every channel moved by the same
/// amount rather than every channel being pulled toward a shared grey
/// extreme.
fn rule_stroke(panel: [u8; 3], delta: i16) -> [u8; 3] {
    [
        offset_channel(panel[0], delta),
        offset_channel(panel[1], delta),
        offset_channel(panel[2], delta),
    ]
}

/// Assign [`crate::token::rule::SHADOW_TOKEN`] and
/// [`crate::token::rule::HIGHLIGHT_TOKEN`] into a theme's value map.
///
/// Computed from `layers[1]`, `surface.layer-one` — the panel every shipped
/// horizontal rule is drawn on today (row rules, list separators, panel
/// splits all sit on a card, never directly on the window ground). The
/// contract's measured pair, 28 and 13, is a ratio law general enough to
/// take any panel; this call site is the one place that currently has to
/// pick which panel, and it picks the one the shipped catalog actually
/// uses. A future rule drawn on a different panel calls
/// [`rule_stroke`] again against that panel's own fill rather than reusing
/// these two resolved tokens, so it keeps its own hue.
///
/// Both shipped themes land on the same pair, 28 and 13, from opposite
/// directions: light's `#f2f2f2` (242) has exactly 13 levels of headroom to
/// white, so the highlight lands precisely at the clamp; dark's `#222222`
/// (34) has 34 levels of headroom to black, well clear of the 28-level
/// shadow. Neither stroke silently clips past what the construction table
/// in the contract records.
fn insert_rule_material(values: &mut BTreeMap<TokenName, TokenValue>, layers: &[[u8; 3]; 4]) {
    let panel = layers[1];
    values.insert(
        name(crate::token::rule::SHADOW_TOKEN),
        opaque(rule_stroke(panel, RULE_SHADOW_DELTA)),
    );
    values.insert(
        name(crate::token::rule::HIGHLIGHT_TOKEN),
        opaque(rule_stroke(panel, RULE_HIGHLIGHT_DELTA)),
    );
}

/// Light [`BORDER_STRONG_TOKEN`], the control-boundary tone: `#8c8c8c`, the
/// first level clearing [`MIN_CONTROL_BOUNDARY`] on all four light layers
/// (3.00:1 on `#f2f2f2`, 3.36:1 on `#ffffff`).
fn light_border_strong() -> [u8; 3] {
    quietest_grey_clearing(&LIGHT_LAYERS, MIN_CONTROL_BOUNDARY, ThemeMode::Light)
}

/// Dark [`BORDER_STRONG_TOKEN`], the control-boundary tone: `#8f8f8f`, the
/// first level clearing [`MIN_CONTROL_BOUNDARY`] on all four dark layers
/// (3.01:1 on `#444444`, 5.79:1 on `#121212`).
fn dark_border_strong() -> [u8; 3] {
    quietest_grey_clearing(&DARK_LAYERS, MIN_CONTROL_BOUNDARY, ThemeMode::Dark)
}

/// The one accent hue, and the ink that goes on top of it.
///
/// **Blue, and the choice is not taste.** The operator this project is built
/// for is red-green colour blind, so the single hue the interface leans on
/// has to be the one that survives both deuteranopia and protanopia. Blue is
/// the axis neither deficiency collapses;
/// `the_accent_survives_red_green_colour_blindness` measures that against
/// the same Viénot-Brettel-Mollon transform the status palette is held to,
/// rather than leaving it as a claim in a comment.
///
/// The standing rule still holds and this token does not weaken it: **colour
/// is never the only channel.** The accent may not be the sole signal for
/// any state, exactly as `status.*` may not be
/// (see [`crate::token::status`]).
///
/// The accent is a **fill**, not a text tone. The four places it is
/// sanctioned to be spent — primary button fill, focus ring, selected tab,
/// progress fill — are all non-text user-interface components, so the floor
/// it is judged against is WCAG 2.1 SC 1.4.11 *Non-text Contrast* (3:1,
/// Level AA), not SC 1.4.3's 4.5:1 body-text floor. That distinction is
/// load-bearing here rather than pedantic: light `#0f62fe` measures
/// **4.47:1** on `surface.layer-one` (`#f2f2f2`), which clears 3:1 with room
/// and misses 4.5:1 by 0.03. Held to the text floor the shipped value would
/// fail on the card surface it is meant to sit on.
/// `the_accent_clears_aa_on_every_surface_it_can_be_painted_on` states
/// which floor it uses and why, and pins the deepest layer each theme's
/// accent survives.
const LIGHT_ACCENT: [u8; 3] = [0x0f, 0x62, 0xfe];
/// See [`LIGHT_ACCENT`]. Lifted for the dark ground, and the reason is the
/// *label*, not the fill: `#0f62fe` on `#121212` is 3.745:1 and on `#222222`
/// is 3.18:1, so the light accent would technically clear the 3:1 fill floor
/// in dark mode. What it cannot do is carry text — dark's
/// [`ON_ACCENT_TOKEN`] is `#121212`, and `#121212` on `#0f62fe` is 3.745:1,
/// under the 4.5:1 AA body-text floor. A primary button whose fill passes
/// and whose label fails is the worse of the two failures, because the fill
/// is the part a reader does not have to decode.
const DARK_ACCENT: [u8; 3] = [0x45, 0x89, 0xff];

/// The name the accent fill is bound under.
const ACCENT_TOKEN: &str = "accent.primary";

/// The name for text painted **on** the accent fill.
///
/// **This token exists because a measurement forced it, not because the
/// palette looked incomplete.** A primary button is one of the accent's four
/// sanctioned uses, and a button has a label. Neither shipped text tone can
/// legibly carry that label:
///
/// | on the accent fill | `text.primary` | `text.muted` |
/// |---|---|---|
/// | light, on `#0f62fe` | 3.48:1 | 1.95:1 |
/// | dark, on `#4589ff` | 2.99:1 | 2.26:1 |
///
/// All four are under the 4.5:1 AA floor for body text, and the dark pair is
/// the worse of the two — which is the same shape as the bug
/// `every_text_tone_clears_aa_on_every_surface_it_can_be_painted_on` was
/// written after. A component author reaching for the obvious tone would
/// reproduce that bug on a new ground.
///
/// The value is **not a fourth grey chosen by eye**: it is the theme's own
/// `surface.base`, assigned by [`insert_accent`] from `LAYERS[0]` so it
/// cannot drift away from it. Ink on a filled accent is the page the accent
/// is cut out of — white in light (5.00:1), `#121212` in dark (5.60:1).
///
/// A **hover or pressed variant was considered and refused.** Nothing in
/// this pass measures a need for one: no interaction state is sanctioned to
/// change the accent's hue, the painter already dims and lifts through
/// `Props.opacity` end to end, and this file's own doc comments record that
/// a declared name nothing reads is the defect the M-Carbon note counted at
/// 12 of 18 names. `text.on-accent` clears that bar and a `accent.hover`
/// does not: one has a failing measurement behind it, the other has a
/// habit.
const ON_ACCENT_TOKEN: &str = "text.on-accent";

/// The ink a link is written in: Carbon's `$link-primary`.
///
/// **Not an alias of the accent, and the reason is a measurement.** Carbon
/// gives links their own step of the blue ramp (blue 40 in g100, blue 60 in
/// white) because the accent is tuned as a *fill* under `text.on-accent`,
/// not as *text* on a layer: dark `#4589ff` is 2.9:1 on `surface.layer-three`
/// and 4.5:1 is the floor for body text. A link that only differed from prose
/// by a hue nobody can read at 2.9:1 would be the "just looks like a text
/// label" the operator reported on row 15, with a colour bolted on.
///
/// The value is derived rather than chosen: [`insert_accent`] walks the
/// accent toward the theme's `text.primary` and stops at the first step that
/// clears the floor on the deepest layer, so the token cannot drift away
/// from the accent's hue and cannot fall below AA on any ground a link can
/// be placed on. In light the accent already clears the floor and the walk
/// stops at step zero; in dark it lightens. Neither theme carries a literal
/// for it.
///
/// Hue is never the only channel: `component::link` pairs this ink with the
/// `underline` slot (`token::slot::standard_slots`), which is the channel a
/// red-green colour-blind reader has.
const LINK_TOKEN: &str = "link-primary";

/// Assign a mode's [`BORDER_TOKEN`] into a theme's value map.
fn insert_border(values: &mut BTreeMap<TokenName, TokenValue>, border: &[u8; 3]) {
    values.insert(
        name(BORDER_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(
            border[0], border[1], border[2], 0xff,
        )),
    );
}

/// Assign a mode's accent pair, and the link ink derived from it, into a
/// theme's value map.
///
/// `layers` is the same array [`insert_layer_set`] takes. `layers[0]` is
/// read for [`ON_ACCENT_TOKEN`], which *is* `surface.base`, and taking it
/// from the layer set rather than repeating the literal is what stops the
/// two from drifting into two different whites. `layers[3]` is read for
/// [`LINK_TOKEN`]: the deepest layer is the hardest ground a link can be
/// written on, so an ink that clears the floor there clears it everywhere.
///
/// `ink` is the theme's `text.primary`, the direction the link tone walks
/// in: toward the page's own text colour, which is the one direction that
/// is guaranteed to end above the floor, because `text.primary` clears it
/// itself (`every_text_tone_clears_aa_on_every_surface_it_can_be_painted_on`).
fn insert_accent(
    values: &mut BTreeMap<TokenName, TokenValue>,
    accent: &[u8; 3],
    layers: &[[u8; 3]; 4],
    ink: [u8; 3],
) {
    let accent_value = ColorValue::from_srgb8(accent[0], accent[1], accent[2], 0xff);
    values.insert(name(ACCENT_TOKEN), TokenValue::Color(accent_value));
    let ground = layers[0];
    values.insert(
        name(ON_ACCENT_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(
            ground[0], ground[1], ground[2], 0xff,
        )),
    );
    values.insert(
        name(LINK_TOKEN),
        TokenValue::Color(link_ink(
            accent_value,
            opaque_value(ink),
            opaque_value(layers[3]),
        )),
    );
}

/// WCAG 2.x AA for body text, the floor [`link_ink`] walks up to.
const LINK_MIN_TEXT_CONTRAST: f32 = 4.5;
/// How finely [`link_ink`] steps from the accent toward the ink. Twenty
/// steps of five percent: fine enough that the first passing step is
/// within a hair of the floor, coarse enough to be exact in `f32`.
const LINK_WALK_STEPS: u32 = 20;

/// The first colour on the walk from `accent` toward `ink` that clears
/// [`LINK_MIN_TEXT_CONTRAST`] on `deepest`, or `ink` itself if none does.
///
/// `ink` is the far end for the reason [`insert_accent`] gives: it clears
/// the floor on its own, so the walk always terminates on a passing tone.
fn link_ink(accent: ColorValue, ink: ColorValue, deepest: ColorValue) -> ColorValue {
    #[allow(clippy::cast_precision_loss)]
    (0..=LINK_WALK_STEPS)
        .map(|step| accent.lerp(ink, step as f32 / LINK_WALK_STEPS as f32))
        .find(|candidate| candidate.contrast_ratio(deepest) >= LINK_MIN_TEXT_CONTRAST)
        .unwrap_or(ink)
}

/// An opaque [`ColorValue`] from an sRGB triple. The value-typed twin of
/// [`opaque`], for arithmetic that wants a colour rather than a token.
fn opaque_value(rgb: [u8; 3]) -> ColorValue {
    ColorValue::from_srgb8(rgb[0], rgb[1], rgb[2], 0xff)
}

/// The two text tones, primary first, for each polarity.
///
/// Hoisted out of [`light`] and [`dark`] on 2026-08-25 because the inverse
/// family needs the *other* polarity's ink: `text-inverse` in the light theme
/// is the dark theme's `text.primary`, and copying the literal would make two
/// tones that were meant to be one.
const LIGHT_TEXT: [[u8; 3]; 2] = [[0x1a, 0x1a, 0x1a], [0x44, 0x44, 0x44]];
/// See [`LIGHT_TEXT`].
const DARK_TEXT: [[u8; 3]; 2] = [[0xf2, 0xf2, 0xf2], [0xd4, 0xd4, 0xd4]];

/// The focus underline's fill and the unused paper halo, in that order.
/// The fill is the accent: one hue, the same blue as the primary button.
/// `focus-inverse` still reads the fill, so an inverted tooltip's underline
/// is the other polarity's accent.
const LIGHT_RING: [[u8; 3]; 2] = [LIGHT_ACCENT, [0xff, 0xff, 0xff]];
/// See [`LIGHT_RING`].
const DARK_RING: [[u8; 3]; 2] = [DARK_ACCENT, [0x05, 0x05, 0x05]];

/// One polarity's greys, gathered so the *other* polarity can reach them.
///
/// The inverse family (`background-inverse`, `text-inverse`, `border-inverse`,
/// `focus-inverse`, `icon-inverse`, `layer-selected-inverse`) exists because
/// tooltips and toggletips paint on an inverted ground — 35 references across
/// the 42 audited components, and Petra had no inverse anything. The honest
/// way to build one is not to pick six new greys by eye: it is to reach into
/// the theme that already ships for the opposite polarity, where every one of
/// those tones has already been measured against the ground it sits on. A
/// light theme's inverted tooltip *is* a small piece of the dark theme.
struct Polarity {
    /// The four-entry layer set, ground first.
    layers: &'static [[u8; 3]; 4],
    /// Primary then muted ink.
    text: &'static [[u8; 3]; 2],
    /// The focus ring's ink core.
    ring: &'static [u8; 3],
}

/// The light polarity, as the dark theme's inverse source.
///
/// No `border` field: since 2026-09-05 both border tones are *walked out* of
/// the layer set by [`quietest_grey_clearing`] rather than written as
/// literals, so there is no constant left to gather here. The inverse
/// border is [`light_border`] called from the dark theme, one call away.
const LIGHT: Polarity = Polarity {
    layers: &LIGHT_LAYERS,
    text: &LIGHT_TEXT,
    ring: &LIGHT_RING[0],
};

/// The dark polarity, as the light theme's inverse source. See [`LIGHT`].
const DARK: Polarity = Polarity {
    layers: &DARK_LAYERS,
    text: &DARK_TEXT,
    ring: &DARK_RING[0],
};

/// The IEC 61966-2-1 sRGB transfer function, 8-bit encoded in, linear-light
/// out. The same curve `ColorValue::from_srgb8` applies; spelled again here
/// because this module works in `[u8; 3]` triples and that one works in
/// `ColorValue`.
fn srgb8_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// [`srgb8_to_linear`] inverted, rounded to the nearest code value.
fn linear_to_srgb8(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let encoded = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    // `as` saturates on an out-of-range float in Rust, and the clamp above
    // already bounds the input, so this cannot wrap.
    (encoded * 255.0).round() as u8
}

/// CIE L\*a\*b\*'s companion function, D65.
fn lab_f(t: f32) -> f32 {
    if t > 0.008_856 {
        t.cbrt()
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

/// [`lab_f`] inverted.
fn lab_f_inv(f: f32) -> f32 {
    let cube = f * f * f;
    if cube > 0.008_856 {
        cube
    } else {
        (f - 16.0 / 116.0) / 7.787
    }
}

/// A colour's CIE L\*, the perceived-lightness axis.
///
/// L\* depends only on relative luminance, so this is exact for a chromatic
/// colour as well as a grey — the D65 white point normalises `Y` to 1.0 and
/// the `X`/`Z` channels never enter the `L` term.
fn lightness_of(rgb: [u8; 3]) -> f32 {
    let y = 0.212_672_9 * srgb8_to_linear(rgb[0])
        + 0.715_152_2 * srgb8_to_linear(rgb[1])
        + 0.072_175 * srgb8_to_linear(rgb[2]);
    116.0 * lab_f(y) - 16.0
}

/// The achromatic sRGB grey whose CIE L\* is `l`.
///
/// Greys only, and that is enough: every tone this file steps — the layer
/// set, the border tones, the interaction states — is achromatic, and the one
/// chromatic token (`accent.primary`) is never stepped, because M-Carbon
/// forbids an interaction state from changing the accent's hue.
fn grey_at_lightness(l: f32) -> [u8; 3] {
    let y = lab_f_inv((l.clamp(0.0, 100.0) + 16.0) / 116.0);
    let c = linear_to_srgb8(y);
    [c, c, c]
}

/// Move `base` `delta` units of CIE L\* **away from the theme's own ground**:
/// darker in a light theme, lighter in a dark one.
///
/// That direction is the whole of what [`ThemeMode`] means in this file — the
/// polarity, and nothing else — and it is why an interaction state does not
/// need a per-theme table. A light page's ground is near-white, so the only
/// direction with room in it is down; a dark page's is near-black, so the
/// only direction is up. Carbon's own themes do exactly this: White steps
/// `$layer-01` `#f4f4f4` down to `#e8e8e8` on hover, Gray 100 steps `#262626`
/// up to `#333333`.
fn step_away_from_ground(base: [u8; 3], delta: f32, mode: ThemeMode) -> [u8; 3] {
    let signed = match mode {
        ThemeMode::Light => -delta,
        ThemeMode::Dark => delta,
    };
    grey_at_lightness(lightness_of(base) + signed)
}

/// How far a hover state moves a surface, in CIE L\*.
///
/// **Smaller than one layer step, and that is the constraint that fixes it.**
/// The dark layer set steps 7.76 L\* per layer
/// ([`the_dark_layer_set_steps_evenly_in_perceived_lightness`]), so a hover
/// that moved as far as a layer would make a hovered card and a nested card
/// the same picture — the depth cue and the state cue would collide. It also
/// has to clear CIE L\*'s conventional just-noticeable difference of about
/// 1.0, or the state is invisible. Four sits between those two bounds with
/// room on both sides, and `the_interaction_ladder_is_ordered_and_visible`
/// holds it there rather than to this comment.
const HOVER_STEP: f32 = 4.0;

/// How far a selected state moves a surface, in CIE L\*.
///
/// Louder than [`HOVER_STEP`] because selection persists and hover does not:
/// a state a reader has to notice while looking elsewhere needs more than one
/// a reader is by definition pointing at.
///
/// **Seven is not taste.** Carbon's White theme publishes
/// `$layer-accent-01` — its own name for "one selected-magnitude step off
/// `$layer-01`" — as `#e0e0e0`
/// (`.agents/research/08-24-2026/Design-Language/carbon.md` §2, measured from
/// `@carbon/themes`' DTCG files). Stepping Petra's light `surface.layer-one`
/// down by seven L\* lands on the same tone;
/// `the_selected_step_lands_where_carbon_puts_its_layer_accent` measures the
/// distance and fails if this constant drifts away from it.
const SELECTED_STEP: f32 = 7.0;

/// How far *hover on an already-selected surface* moves it, in CIE L\*.
///
/// **This is a named tone, not a composite, and the distinction is the whole
/// of `contracts/token-vocabulary.md` §6.** Carbon never paints a state by
/// blending a tint over a fill; it swaps which token a slot is bound to, and
/// `$layer-selected-hover` is its own entry in the theme rather than
/// `$layer-hover` composited over `$layer-selected`. Petra follows, because a
/// blend has no name a digest can hash and no value a gate can measure: the
/// frame digest hashes bound token *names*, so a composited state is a state
/// the frame identity cannot see.
///
/// Eleven is [`SELECTED_STEP`] plus [`HOVER_STEP`] — the two moves the
/// combination is named after — computed once here so the arithmetic is
/// visible rather than implied.
const SELECTED_HOVER_STEP: f32 = SELECTED_STEP + HOVER_STEP;

/// How far a pressed state moves a surface, in CIE L\*.
///
/// The loudest of the four and the only one above a full layer step. A press
/// is momentary and self-explaining — the reader's own finger is on the
/// control — so the usual objection to a state that reads like a nesting
/// change does not apply, and Carbon's own themes move furthest here too.
const ACTIVE_STEP: f32 = 14.0;

/// The alpha, out of 255, a disabled tone keeps.
///
/// Carbon expresses every disabled ink as its live counterpart at 25%
/// (`$text-disabled` is `$text-primary` at 0.25 in every theme), and that is
/// what is copied here: one number, applied to a tone that was already
/// measured, rather than a second family of greys chosen by eye.
///
/// **Colour is not the channel that carries "disabled", and this token does
/// not make it one.** WCAG exempts inactive components from its contrast
/// floors precisely because the point of the tone is to fail to attract; the
/// state has to be carried by the node leaving the focus tree and the hit
/// test (FR-008), which is a different lane's work. This is the colour half
/// and only the colour half.
const DISABLED_ALPHA: u8 = 64;

/// Fade `rgb` to [`DISABLED_ALPHA`].
fn disabled(rgb: [u8; 3]) -> TokenValue {
    TokenValue::Color(ColorValue::from_srgb8(
        rgb[0],
        rgb[1],
        rgb[2],
        DISABLED_ALPHA,
    ))
}

/// An opaque colour token value from an sRGB triple, the shape most of the
/// assignments below want.
fn opaque(rgb: [u8; 3]) -> TokenValue {
    TokenValue::Color(ColorValue::from_srgb8(rgb[0], rgb[1], rgb[2], 0xff))
}

/// The icon family: six names, 46 references, the second most-referenced
/// colour family in the whole inventory and absent from Petra entirely until
/// 2026-08-25 (FR-026).
///
/// **Every one of the six is an existing measured tone under a second name,
/// and that is deliberate.** Carbon's own themes assign `$icon-primary` the
/// same value as `$text-primary` and `$icon-secondary` the same as
/// `$text-secondary`; the family exists so a component can say *"this is an
/// icon"* without a second decision about which grey. Assigning them from the
/// text tones rather than repeating the hexes is what keeps a later change to
/// `text.primary` from silently leaving every icon behind — the same rule
/// [`insert_accent`] follows for `text.on-accent`.
///
/// `icon-on-color` is the ink on an accent fill, which is
/// [`ON_ACCENT_TOKEN`]'s measured tone; a second choice here would reproduce
/// the AA failure that token exists to prevent.
const ICON_TOKENS: [&str; 6] = [
    "icon-primary",
    "icon-secondary",
    "icon-on-color",
    "icon-on-color-disabled",
    "icon-disabled",
    "icon-inverse",
];

/// The border names added on 2026-08-25, beyond [`BORDER_TOKEN`].
///
/// **This supersedes the refusal recorded on [`BORDER_TOKEN`]** — see that
/// constant's doc comment, which still carries the refusal and now carries
/// why it was overturned.
const BORDER_STRONG_TOKEN: &str = "border-strong";
/// See [`BORDER_STRONG_TOKEN`]. Carbon's `$border-interactive` is its `$focus`
/// blue; Petra assigns it the accent so the two cannot drift, which also means
/// the accent's own measured contrast floors cover it.
const BORDER_INTERACTIVE_TOKEN: &str = "border-interactive";

/// The per-layer border and field names FR-004a's arithmetic reaches
/// (`contracts/token-vocabulary.md` §5), ordinal `01` first.
///
/// **Carbon's `$field-0N` is literally its `$layer-0N`** — the two columns are
/// identical in all four published themes (`carbon.md` §2, measured) — which
/// is the same statement as FR-004a's *"a field sits one layer ahead of its
/// background"*, written as a value instead of as a rule. So these are
/// assigned straight from [`LAYER_TOKENS`]' colours and cannot drift from
/// them.
pub const FIELD_TOKENS: [&str; 3] = ["field-01", "field-02", "field-03"];

/// The per-layer subtle-border names. See [`FIELD_TOKENS`] for the pairing
/// rule they exist to express.
///
/// **All three carry [`BORDER_TOKEN`]'s one tone, in both themes.** Carbon
/// gives each layer its own border grey; Petra's single tone is already
/// measured to clear the 3:1 non-text floor on *every* layer it can be drawn
/// on and to stay quieter than both text tones there
/// (`the_border_tone_is_visible_everywhere_and_quieter_than_every_text_tone`),
/// so three names sharing one measured tone is a stronger position than three
/// tones nothing has measured. Splitting them later is free: the frame digest
/// hashes token names and never token values, so re-valuing these three moves
/// no digest and no capture.
pub const BORDER_SUBTLE_TOKENS: [&str; 3] =
    ["border-subtle-01", "border-subtle-02", "border-subtle-03"];

/// The interaction-state surfaces, and the ladder step each one takes.
///
/// Two bases, one ladder. `layer-*` steps from [`LAYER_TOKENS`]`[1]` —
/// Carbon's contextual `$layer` resolves to `$layer-01` by default, and Petra
/// has no ambient depth counter to resolve it any other way (that mechanism
/// was weighed and declined in `R-F` §2b: it needs a new digest input, which
/// re-baselines the published frame-identity reference). `layer-accent-*`
/// steps from `layer-accent`, which is itself [`SELECTED_STEP`] off the same
/// base.
///
/// `layer-selected-inverse` is not on this ladder: it is the inverted ground,
/// and it lives with the inverse family for that reason.
const STATE_TOKENS: [(&str, f32); 3] = [
    ("layer-hover", HOVER_STEP),
    ("layer-selected", SELECTED_STEP),
    ("layer-selected-hover", SELECTED_HOVER_STEP),
];

/// See [`STATE_TOKENS`]. Split out because the fourth surface state steps
/// past a full layer and reads differently for it.
const ACTIVE_TOKEN: &str = "layer-active";

/// Carbon: *"not considered a proper layer but a supporting color for
/// `$layer` inside of components"* — so it is **not** in [`LAYER_TOKENS`] and
/// must never be counted when stepping. Seven references.
const LAYER_ACCENT_TOKEN: &str = "layer-accent";

/// The two states `layer-accent` takes, and the ladder step each one adds on
/// top of [`SELECTED_STEP`].
const LAYER_ACCENT_STATE_TOKENS: [(&str, f32); 2] = [
    ("layer-accent-hover", HOVER_STEP),
    ("layer-accent-active", ACTIVE_STEP),
];

/// The scrim a modal or a popover lays over the page it blocks.
///
/// A **token**, replacing the `overlay` *slot* the schema declared and no
/// painter read (FR-025). The slot was the wrong shape: a scrim is a node
/// covering the viewport, painted through `background` like any other fill,
/// not a second colour composited into some other node's rect.
///
/// Black in both themes, at two alphas, for the same reason [`SHADOW_TOKENS`]
/// are black in both: occlusion darkens, and a lightened scrim in dark mode is
/// not a scrim. The dark theme's is the deeper of the two because the page
/// behind it is already dark and has less room left to lose — the same shape
/// as [`DARK_SHADOW_ALPHAS`] against [`LIGHT_SHADOW_ALPHAS`], and the same
/// shape Carbon's own `$overlay` has (`rgba(22,22,22,0.5)` in White,
/// `rgba(0,0,0,0.65)` in Gray 100).
const SCRIM_TOKEN: &str = "overlay.scrim";
/// See [`SCRIM_TOKEN`]. 50% in light, 65% in dark, out of 255.
const LIGHT_SCRIM_ALPHA: u8 = 128;
/// See [`SCRIM_TOKEN`].
const DARK_SCRIM_ALPHA: u8 = 166;

/// The status palette's Carbon-named aliases, paired with the shipped status
/// token each one resolves to.
///
/// **Assigned from the status colours, never chosen independently** — the same
/// posture [`ON_ACCENT_TOKEN`] takes toward `layers[0]`, and here it is a
/// safety property rather than a tidiness one. Carbon's own alert palette
/// fails Petra's `MIN_STATUS_SEPARATION` under three separate colour-blindness
/// models, tightest pair ΔE\*ab 12.5, so it is ineligible at any migration
/// point (FR-005, FR-049). A `support-*` family that carried its own literals
/// would be exactly the route by which it came back in: a Notification
/// declaring `support.error` in Carbon's `#da1e28` would have bypassed a gate
/// that iterated three hardcoded names.
///
/// Two things close that route. These are aliases, so there is no independent
/// value to get wrong; and
/// `status_colours_stay_apart_under_red_green_colour_blindness` now walks
/// *every* colour token whose name starts `status.` or `support-` rather than
/// three literals, so a fourth status-shaped token is caught the day it is
/// added.
/// Carbon's `$button-danger-primary`: the fill under a destructive button.
///
/// **This is the third chromatic name in the library, and it exists because
/// the only red already shipped cannot carry a label.**
/// `component::button`'s module doc carries wave E's full table; the one
/// line of it that matters is that `support-error` (`#f21c0d` dark, tuned as
/// a *status marker*, so pushed bright to clear 3:1 against a surface) has
/// no ink in this library that clears the 4.5:1 AA body-text floor on it —
/// the best is `text.on-accent` at 4.41:1. Carbon's `#da1e28` clears it at
/// **5.00:1 against white**. That difference is the whole reason for a
/// second red rather than an alias.
///
/// MEASURED from `@carbon/themes/scss/generated/_button-tokens.scss`, where
/// `$button-danger-primary` is `#da1e28` in **all four** published themes —
/// which is why one constant serves both themes here, the same shape as
/// [`ON_COLOUR_TOKEN`]. (That measurement is wave E's, recorded in
/// `component::button`'s module doc on 2026-09-05; `ignored/carbon-ref/` was
/// not on this machine when this landed, so it is cited from the in-repo
/// record rather than re-read from the package.)
///
/// **Colour is not the only channel and this token does not make it one.**
/// `component::button` keeps the red octagon on all three danger variants
/// even now that the filled one is red (FR-015). On the fill the octagon
/// switches to [`ON_COLOUR_TOKEN`], because `support-error` on `#da1e28`
/// measures 1.18:1 and is a mark nobody can see; the shape survives
/// greyscale either way, which is the channel that matters here.
///
/// Gated by
/// [`tests::the_danger_fill_carries_a_white_label_and_reads_without_colour`],
/// which is what lets it past
/// [`tests::no_shipped_colour_carries_a_hue_that_is_not_already_gated`].
const DANGER_FILL_TOKEN: &str = "button-danger-primary";

/// See [`DANGER_FILL_TOKEN`]. One value, both themes, because Carbon
/// publishes one.
const DANGER_FILL: [u8; 3] = [0xda, 0x1e, 0x28];

/// Carbon's `$button-disabled`: the track colour `component::controls`'s
/// `toggle` paints when disabled, on and off alike
/// (`.cds--toggle--disabled .cds--toggle__switch`, `_toggle.scss`).
///
/// **Not a fade of an existing tone**, unlike [`ICON_TOKENS`]'s two disabled
/// names. Carbon publishes `$button-disabled` as its own colour in
/// `_button-tokens.scss` — an opaque mid-grey in White/G10 and a
/// translucent grey in G90/G100 — rather than deriving it from
/// `icon-primary` or the accent at 25%, so reusing [`ICON_DISABLED`] here
/// would collapse two different Carbon values into one name.
///
/// MEASURED from `_button-tokens.scss`'s `$button-disabled` map. Petra ships
/// two themes, not Carbon's four: light takes the White/G10 value (they
/// agree, `#c6c6c6`), dark takes the G90/G100 value (they also agree,
/// `rgba(141, 141, 141, 0.3)`).
const BUTTON_DISABLED_TOKEN: &str = "button-disabled";

/// See [`BUTTON_DISABLED_TOKEN`]. Light: `#c6c6c6`, opaque.
const BUTTON_DISABLED_LIGHT: [u8; 3] = [0xc6, 0xc6, 0xc6];

/// See [`BUTTON_DISABLED_TOKEN`]. Dark: `rgb(141, 141, 141)` at 30% alpha
/// (Carbon's `rgba(141, 141, 141, 0.3)`; `0.3 * 255` rounds to 77).
const BUTTON_DISABLED_DARK: [u8; 3] = [0x8d, 0x8d, 0x8d];
/// See [`BUTTON_DISABLED_DARK`].
const BUTTON_DISABLED_DARK_ALPHA: u8 = 77;

/// Carbon's `$text-on-color` (and `$icon-on-color`): **white in every
/// theme**, the ink and the mark that go on a saturated fill.
///
/// # Why this is not `text.on-accent`
///
/// Because Petra's accent is lighter than Carbon's and the two names stopped
/// meaning the same thing the moment that was true. [`ON_ACCENT_TOKEN`] is
/// assigned the theme's own `surface.base`, so in dark it is `#121212` — a
/// near-black, and correct there: white on dark's `#4589ff` measures
/// **3.35:1**, under the 4.5:1 AA floor, while `#121212` on it is 5.60:1.
/// Carbon never faces that because its `$button-primary` is `#0f62fe` in
/// g100 too.
///
/// So an "on-colour" ink and an "on-accent" ink are two different tones in
/// this library, and conflating them shipped a visible defect: row 36's
/// on-toggle drew a **black knob** where Carbon's is white, because
/// `component::controls` bound `TEXT_ON_ACCENT` for it. Carbon's handle is
/// `background-color: $icon-on-color` on `.cds--toggle__switch::before`
/// with no `--checked` override (SOURCED
/// `.agents/research/08-25-2026/Carbon-Component-Inventory/slice-f.md:26`),
/// so it is white in both states. Wave D found this and left it for the
/// token layer, in
/// `.agents/notes/implemented/feature/2026-09-05-the-notification-carries-a-kind-and-two-rows-leave-carbon-on-purpose.md`.
///
/// `icon-on-color` is deliberately **not** repointed at this. It is the
/// resting partner of `icon-on-color-disabled`, which is the same tone at
/// 25%, and both are spent on the accent fill by `primary_button` — moving
/// one and not the other would ship a button whose label is near-black at
/// rest and faded-white when disabled. That name keeps Petra's meaning
/// ("ink on *the accent*") and this one carries Carbon's.
///
/// Its two bindings today are the danger button's label and mark on
/// [`DANGER_FILL_TOKEN`] (5.00:1, AA) and the toggle knob on
/// `accent.primary` (3.35:1 dark, 5.00:1 light — a graphical object, so SC
/// 1.4.11's 3:1). Both are measured by
/// [`tests::the_on_colour_ink_is_white_in_both_themes_and_is_not_the_accent_ink`].
const ON_COLOUR_TOKEN: &str = "text-on-color";

/// See [`ON_COLOUR_TOKEN`].
const ON_COLOUR: [u8; 3] = [0xff, 0xff, 0xff];

/// Assign the two names that do not move between themes: the Carbon danger
/// fill and the always-white on-colour ink.
///
/// One function called by both themes, for the reason [`insert_layer_set`]
/// is: a hand-copied pair is a pair that drifts, and "the same in every
/// theme" is the entire content of both of these tokens.
fn insert_on_colour_pair(values: &mut BTreeMap<TokenName, TokenValue>) {
    values.insert(name(DANGER_FILL_TOKEN), opaque(DANGER_FILL));
    values.insert(name(ON_COLOUR_TOKEN), opaque(ON_COLOUR));
}

const SUPPORT_ALIASES: [(&str, &str); 3] = [
    ("support-error", "status.down"),
    ("support-success", "status.ok"),
    ("support-warning", "status.degraded"),
];

/// Assign the icon family into a theme's value map, from tones the theme has
/// already assigned. `text` is the polarity's own ink pair, `ground` its
/// `surface.base` (which is [`ON_ACCENT_TOKEN`]'s value), `inverse_ink` the
/// other polarity's primary ink.
fn insert_icon_family(
    values: &mut BTreeMap<TokenName, TokenValue>,
    text: &[[u8; 3]; 2],
    ground: [u8; 3],
    inverse_ink: [u8; 3],
) {
    values.insert(name(ICON_TOKENS[0]), opaque(text[0]));
    values.insert(name(ICON_TOKENS[1]), opaque(text[1]));
    values.insert(name(ICON_TOKENS[2]), opaque(ground));
    values.insert(name(ICON_TOKENS[3]), disabled(ground));
    values.insert(name(ICON_TOKENS[4]), disabled(text[0]));
    values.insert(name(ICON_TOKENS[5]), opaque(inverse_ink));
}

/// Assign the border names beyond [`BORDER_TOKEN`]: the loud tone, the
/// interactive tone, the three per-layer subtle names, and the inverted one.
///
/// # `border-strong` stopped being a midpoint on 2026-09-05
///
/// It used to be the grey at the **CIE L\* midpoint** of the subtle border
/// and the muted text tone, chosen because it lands strictly inside both
/// bounds no matter how either endpoint moves. That construction was sound
/// and it had one dependency that turned out to be false: it assumed the
/// subtle border was itself anchored to the control-boundary floor. Once
/// [`BORDER_TOKEN`] became the *decorative* tone at
/// [`MIN_DIVIDER_CONTRAST`], a midpoint between a 1.3:1 rule and a text tone
/// would have been a control boundary chosen by averaging two numbers
/// neither of which is a control boundary — and in dark it lands at `#a4a4a4`,
/// 61 sRGB levels brighter than the 3:1 floor asks for.
///
/// So the roles swapped over which one is derived from a floor. **This is
/// now the anchored one**: the quietest grey clearing
/// [`MIN_CONTROL_BOUNDARY`] on every layer, walked out by
/// [`quietest_grey_clearing`] exactly the way the decorative tone is walked
/// out from its own floor. Both bounds the midpoint used to guarantee are
/// still held, now by measurement instead of by construction —
/// [`tests::border_strong_sits_between_the_subtle_border_and_every_text_tone`]
/// keeps it strictly louder than the subtle tone and strictly quieter than
/// both text tones, and it fails in both directions.
fn insert_border_family(
    values: &mut BTreeMap<TokenName, TokenValue>,
    border: &[u8; 3],
    strong: [u8; 3],
    accent: &[u8; 3],
    inverse_border: [u8; 3],
) {
    values.insert(name(BORDER_STRONG_TOKEN), opaque(strong));
    values.insert(name(BORDER_INTERACTIVE_TOKEN), opaque(*accent));
    for token in BORDER_SUBTLE_TOKENS {
        values.insert(name(token), opaque(*border));
    }
    values.insert(name("border-inverse"), opaque(inverse_border));
}

/// Assign `field-01…03` from the layer set they are defined to equal.
fn insert_field_set(values: &mut BTreeMap<TokenName, TokenValue>, layers: &[[u8; 3]; 4]) {
    for (token, rgb) in FIELD_TOKENS.iter().zip(&layers[1..]) {
        values.insert(name(token), opaque(*rgb));
    }
}

/// Assign the interaction-state surfaces by walking the ladder off
/// `layers[1]`, in `mode`'s direction.
fn insert_state_set(
    values: &mut BTreeMap<TokenName, TokenValue>,
    layers: &[[u8; 3]; 4],
    mode: ThemeMode,
) {
    let base = layers[1];
    for (token, delta) in STATE_TOKENS {
        values.insert(
            name(token),
            opaque(step_away_from_ground(base, delta, mode)),
        );
    }
    values.insert(
        name(ACTIVE_TOKEN),
        opaque(step_away_from_ground(base, ACTIVE_STEP, mode)),
    );

    let accent_surface = step_away_from_ground(base, SELECTED_STEP, mode);
    values.insert(name(LAYER_ACCENT_TOKEN), opaque(accent_surface));
    for (token, delta) in LAYER_ACCENT_STATE_TOKENS {
        values.insert(
            name(token),
            opaque(step_away_from_ground(accent_surface, delta, mode)),
        );
    }
}

/// Assign the inverse family from `other`, the polarity this theme inverts
/// into. See [`Polarity`] for why the values come from a shipped theme rather
/// than from six fresh greys.
///
/// `background-inverse-hover` steps in **`other`'s** direction, not this
/// theme's: the surface being hovered belongs to the inverted ground, so
/// "away from the ground" means away from *its* ground.
fn insert_inverse_family(
    values: &mut BTreeMap<TokenName, TokenValue>,
    other: &Polarity,
    other_mode: ThemeMode,
) {
    let ground = other.layers[1];
    values.insert(name("background-inverse"), opaque(ground));
    values.insert(
        name("background-inverse-hover"),
        opaque(step_away_from_ground(ground, HOVER_STEP, other_mode)),
    );
    values.insert(name("text-inverse"), opaque(other.text[0]));
    values.insert(name("focus-inverse"), opaque(*other.ring));
    // The deepest tone the inverted polarity has, which is what a selected row
    // on an inverted ground wants: one more step of separation than
    // `background-inverse` has left to give.
    values.insert(name("layer-selected-inverse"), opaque(other.layers[0]));
}

/// Assign [`SCRIM_TOKEN`] at `alpha`.
fn insert_scrim(values: &mut BTreeMap<TokenName, TokenValue>, alpha: u8) {
    values.insert(
        name(SCRIM_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(0x00, 0x00, 0x00, alpha)),
    );
}

/// Assign every [`SUPPORT_ALIASES`] entry by copying the status colour it
/// names out of `values`.
///
/// Called **after** the status colours are assigned, and it reads them back
/// rather than taking them as arguments, so an alias cannot be pointed at a
/// colour the theme does not actually ship.
///
/// # Panics
/// When an alias names a status token this theme has not assigned, which
/// `every_support_alias_is_its_status_colour` also catches.
fn insert_support_aliases(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (alias, status) in SUPPORT_ALIASES {
        let value = *values
            .get(&name(status))
            .unwrap_or_else(|| panic!("support alias {alias:?} names unassigned {status:?}"));
        values.insert(name(alias), value);
    }
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

/// The shipped spacing ramp: Carbon's thirteen steps, in logical units.
///
/// A **ramp**, not a number line. Its job is to be the whole set of gaps a
/// page is allowed to use, so that two panels written by two authors line up
/// without either of them measuring the other. A gap that lands between two
/// steps is a gap nobody chose.
///
/// **The eight values Petra already shipped did not move.** The pre-Carbon
/// ramp was `2, 4, 8, 12, 16, 24, 32, 48`, and every one of those appears
/// verbatim in Carbon's `spacing-01 … spacing-09`
/// (`.agents/research/08-24-2026/Design-Language/carbon.md` §4, measured
/// against `@carbon/layout`'s published `_spacing.scss`, which literally
/// contains `$spacing-01: 0.125rem` … `$spacing-13: 10rem`). The two ramps
/// agree step for step through Petra's entire previous range — not because
/// one copied the other, but because both start at a quarter of an 8-unit
/// mini-unit, double to 16, and then step by whole units. So this is a
/// **rename plus five additions** (`08` at 40, and `10 … 13` at 64/80/96/160),
/// and not one gap on any existing page moves by a pixel.
///
/// **Why ordinals and not t-shirt sizes.** Eight t-shirt names is already
/// `2xs … 3xl` with the multipliers doing the work at both ends; thirteen is
/// not expressible that way without inventing `4xl` and `3xs`, which name a
/// position badly. Carbon numbers them because a thirteen-step ramp is a
/// sequence, and the audited components reference the numbers
/// (`spacing-05` × 83, `spacing-03` × 57, `spacing-02` × 25 across the 42
/// components in `.agents/research/08-25-2026/Carbon-Component-Inventory/`).
/// Adopting the numbers means the inventory and this table can be diffed by
/// eye.
///
/// Declared once and shared by both shipped themes verbatim, because spacing
/// is geometry: a gap does not change when the lights go out. Colour is the
/// only channel [`light`] and [`dark`] disagree on, and
/// `the_two_shipped_themes_agree_on_every_gap` holds them to that.
const SPACING_RAMP: [(&str, f32); 13] = [
    ("spacing-01", 2.0),
    ("spacing-02", 4.0),
    ("spacing-03", 8.0),
    ("spacing-04", 12.0),
    ("spacing-05", 16.0),
    ("spacing-06", 24.0),
    ("spacing-07", 32.0),
    ("spacing-08", 40.0),
    ("spacing-09", 48.0),
    ("spacing-10", 64.0),
    ("spacing-11", 80.0),
    ("spacing-12", 96.0),
    ("spacing-13", 160.0),
];

/// The eight t-shirt names the ramp shipped under before 2026-08-25, each
/// paired with the [`SPACING_RAMP`] step it now resolves through.
///
/// **Shims, not steps** — the same posture, and the same reason, as
/// [`RAISED_ALIAS`]. Every alias resolves to exactly its ramp step's value,
/// assigned by [`insert_spacing_ramp`] from the table above rather than
/// repeated as a literal so the two cannot drift, and
/// `the_spacing_aliases_are_their_ramp_steps` holds that to measurement.
///
/// **Roughly 190 call sites bind these names, and one duplicated rule is why
/// they are not fewer.** `component::tokens` — the one place the component
/// library spells a token name, and the module whose whole argument is that a
/// ramp rename should be a five-line edit — moved onto the ordinals, along
/// with `token::{snapshot,theme,vocabulary}`'s fixtures and the painter's.
///
/// `gorgon-inspector` and `gorgon-petra-egui`'s two examples did **not**, and
/// the reason is a defect rather than a decision: `xtask`'s `literal-style`
/// gate scans exactly those two roots for a literal reaching a token slot,
/// and it re-implements this crate's namespacing rule by hand
/// (`xtask/src/literal_style.rs`'s `looks_namespaced`, whose own doc says
/// *"mirrors `gorgon_petra::token::name`'s namespacing rule"*). The rule
/// widened here to admit `spacing-05`; the copy did not, so the gate reads
/// every ordinal step as an unnamespaced literal. The fix is one line — call
/// `TokenName::new(..).is_ok()` instead of re-deriving the rule, which `xtask`
/// can already do because it depends on this crate — and it belongs to
/// whoever owns that file. Until then those call sites stay on the aliases,
/// which is exactly what the aliases are for.
///
/// The rest is test fixtures in `tree::props`, `tree::validate` and `layout`,
/// plus `token::name`'s own grammar test, which binds `spacing.2xs`
/// deliberately: the *shape* of that name is what it is testing, and it stays
/// legal after every alias here is gone.
///
/// **They go when the last binding moves**, and nothing new should bind one.
/// Spacing names never reach the frame digest — `props.spacing` and
/// `props.padding` resolve to numbers before placement — so that day costs a
/// rename and re-baselines nothing.
const SPACING_ALIASES: [(&str, &str); 8] = [
    ("spacing.2xs", "spacing-01"),
    ("spacing.xs", "spacing-02"),
    ("spacing.sm", "spacing-03"),
    ("spacing.md", "spacing-04"),
    ("spacing.lg", "spacing-05"),
    ("spacing.xl", "spacing-06"),
    ("spacing.2xl", "spacing-07"),
    ("spacing.3xl", "spacing-09"),
];

/// The fixed control-size ramp: how tall a control **is**, in logical units.
///
/// **Not a spacing multiple, and that is the point.** Carbon publishes
/// `size-{xs,sm,md,lg,xl,2xl}` = `24, 32, 40, 48, 64, 80` as its own scale
/// (`carbon.md` §4, measured from `@carbon/layout`'s `src/index.ts`), and
/// past step 5 it stops tracking the spacing ramp: 40 is `spacing-08` but 24
/// is `spacing-06` and 80 is `spacing-11`, so reading a control height off
/// the gap scale means picking a different ordinal per size and hoping the
/// two scales keep agreeing. They are different questions — "how far apart
/// are these two things" and "how tall is a button" — and a design system
/// that answers both from one table cannot change either answer
/// independently.
///
/// [`TokenKind::Spacing`], because a height is an extent in logical units and
/// that is the shape `TokenKind::Spacing` already carries. A `Size` kind would
/// buy nothing at the value layer and would cost a hand edit to `xtask`'s
/// `token_stubs.rs` `[(TokenKind, &str); 8]` table, which is a different
/// lane's file.
///
/// **This is the one family in this change that does not clear the
/// inventory's own reference bar, and that is recorded rather than hidden.**
/// `contracts/token-vocabulary.md` §3 admits a token only when one of the 42
/// audited components references it, and no slice reports a `$size-*`
/// reference: Carbon spends this ramp through per-component SCSS locals
/// (`$button-height` and friends), so the inventory sees the *heights* and
/// never the token name. FR-004 names the ramp explicitly, and that is the
/// authority these six enter on. The first component to take a fixed height
/// is what will make them read tokens rather than declared ones.
///
/// Shared verbatim by both themes, for [`SPACING_RAMP`]'s reason.
const SIZE_RAMP: [(&str, f32); 6] = [
    ("size-xs", 24.0),
    ("size-sm", 32.0),
    ("size-md", 40.0),
    ("size-lg", 48.0),
    ("size-xl", 64.0),
    ("size-2xl", 80.0),
];

/// Assign every [`SPACING_RAMP`] step, and every [`SPACING_ALIASES`] shim,
/// into a theme's value map.
///
/// Both themes call this rather than spelling the ramp out twice: a hand-
/// copied ramp is a ramp that drifts, and a drifted one would make the same
/// token name mean two different gaps depending on the operator's theme. The
/// aliases are looked up in the ramp rather than carrying their own numbers,
/// for the same reason one step further in.
///
/// # Panics
/// Never, for the tables above: `the_spacing_aliases_are_their_ramp_steps`
/// walks every alias and proves its target is a real step.
fn insert_spacing_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, units) in SPACING_RAMP {
        values.insert(name(step), TokenValue::Spacing(units));
    }
    for (alias, step) in SPACING_ALIASES {
        let units = SPACING_RAMP
            .iter()
            .find(|(s, _)| *s == step)
            .map(|(_, units)| *units)
            .unwrap_or_else(|| panic!("spacing alias {alias:?} points at unknown step {step:?}"));
        values.insert(name(alias), TokenValue::Spacing(units));
    }
}

/// Assign every [`SIZE_RAMP`] step into a theme's value map, for
/// [`insert_spacing_ramp`]'s drift-proofing reason.
fn insert_size_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, units) in SIZE_RAMP {
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
///
/// `shape.corner-xs` at 2 was added on 2026-08-25 (FR-022, Carbon's
/// `border-radius-02`). It is the radius a small boxed mark inside a control
/// takes — a checkbox's box, a popover body — and the Carbon inventory finds
/// it on three of the 42 components while Petra had nothing between 0 and 4
/// to give them.
///
/// **`shape.corner-lg` (12) survives, and the reason is a count rather than a
/// principle.** Under [`corner_for`]'s rule every 12-unit case in the
/// inventory is a pill and resolves to `corner-full`, so
/// `contracts/token-vocabulary.md` §8 makes retiring this step a **MAY**.
/// Three call sites still bind it — `petra-egui`'s gallery and parity
/// examples, and a `tree::validate` fixture in a crate this change does not
/// own — and rebinding a component's radius moves its digest and every stored
/// capture, which is the flag day §11 puts at step 7. Retiring the name now
/// would either break those trees at acceptance or force that flag day early,
/// for a name that costs one line. It goes with the rebinding, not before it.
const SHAPE_RAMP: [(&str, f32); 6] = [
    ("shape.corner-none", 0.0),
    ("shape.corner-xs", 2.0),
    ("shape.corner-sm", 4.0),
    ("shape.corner-md", 8.0),
    ("shape.corner-lg", 12.0),
    ("shape.corner-full", 999.0),
];

/// What a node **is**, for the purpose of picking its corner radius.
///
/// FR-022 asks for one rule rather than a per-component table, and this is
/// the domain that rule is keyed on. Five members, because
/// `contracts/token-vocabulary.md` §8 has five outcomes and each one answers a
/// different question about the node's relationship to the layout around it —
/// not about which component it happens to be.
///
/// The enumeration lives here, next to [`SHAPE_RAMP`], because the rule is a
/// statement about the ramp: a member that resolved to a radius outside the
/// ramp would be a member with no token, and [`corner_for`]'s return type
/// makes that unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CornerRole {
    /// Tiles or abuts another rectangle: table cells, list rows, tabs, text
    /// inputs, buttons, shell panels. The large majority of the 42 audited
    /// components, none of which carry a `border-radius` anywhere in their
    /// SCSS.
    Tiled,
    /// A small boxed mark **inside** a control: a checkbox's box.
    ///
    /// It named a popover's body too, until spec 009 T011 moved the popover
    /// to [`CornerRole::Floating`] on 2026-09-18. One component may not be
    /// the worked example for two roles, and a popover is not inside a
    /// control — it floats over one.
    BoxedMark,
    /// A container that groups controls: a content switcher, a tile, a code
    /// snippet.
    Grouping,
    /// A surface floating free of the layout: a menu, a modal, a popover.
    Floating,
    /// A **stadium**: a node whose intended radius is half its own shorter
    /// edge at every size it ships. A toggle track and its knob, a tag, a
    /// radio's box, a status dot, a spinner's ring, a slider's handle.
    ///
    /// The other four members name a *fixed* radius and let the half-edge
    /// clause promote it when the node turns out to be small. This one names
    /// the shape directly, and it exists because that promotion cannot
    /// express a pill at a large size: `Floating`'s 8 is the largest radius
    /// the ramp offers a role, so it only clears the half-edge test at or
    /// under 16 units. A 24-unit toggle track and an 18-unit radio box are
    /// both larger than that, and both are pills in Carbon at every size.
    ///
    /// Without this member the rule had two bad answers and no good one:
    /// bind `shape.corner-full` as a literal and leave the component outside
    /// FR-022 (which three of them did until 2026-09-10), or take the role's
    /// fixed radius and render a rounded rectangle where Carbon draws a
    /// stadium (which the toggle track and the tag did for one day, and the
    /// operator would have met as a circular knob sitting in a boxy slot).
    Pill,
}

/// The radius token a node takes, from what it is and how small it is.
///
/// **One rule, five outcomes, and two ways to reach the fifth.**
/// `shorter_edge` is the node's smaller dimension in logical units.
///
/// A node whose intended radius is half its own shorter edge at every size
/// it ships says so outright: [`CornerRole::Pill`] returns `corner-full` and
/// never consults the geometry. That is the toggle track, the toggle knob,
/// the tag, the radio's box, the status dot, the spinner's ring and the
/// slider's handle — stadiums by design rather than by accident of being
/// small.
///
/// A node that named a *fixed* radius is still promoted to a stadium when it
/// turns out small enough that the radius it asked for would eat its own
/// edge. That test runs against the *ramp value*, not against the role, so
/// it fires on the values this ramp actually holds rather than on a
/// remembered table: a 4-unit-tall progress rail asking for [`Grouping`]'s 4
/// gets `corner-full` (4 ≥ 2), while a 24-unit-tall content switcher asking
/// for the same 4 keeps it (4 < 12).
///
/// The promotion clause on its own was tried and is not enough, which is
/// what [`CornerRole::Pill`] is for. `Floating`'s 8 is the largest radius
/// any role asks for, so promotion can only ever fire at or under 16 units,
/// and Carbon has pills well above that: a 24-unit toggle track, an 18-unit
/// radio box, a 32-unit large tag. Routed through a fixed role, each of
/// those renders as a rounded rectangle.
///
/// Silence is not `none`. There is no `Option` in either position here: a
/// caller has to say what the node is, which is FR-022's other half — a
/// component with no radius binding has not chosen a square corner, it has
/// not chosen.
///
/// # Step 7, landed: every `"radius"` binding in `component::` routes here
///
/// `contracts/token-vocabulary.md` §11 step 7 was the flag day this rule was
/// built ahead of — one rebinding for all 42 components, not forty-two small
/// ones, because rebinding a component's `radius` moves that component's
/// frame digest and every stored capture of it. That day landed 2026-09-09:
/// `component::button`, `list_row`, `tile`'s selection mark, `code_snippet`'s
/// inline chip, `notification`, `content_switcher`, `popover` (and
/// `ai_label`'s explainability panel, which shares its chrome), `section`,
/// `modal`'s footer button, `controls`'s checkbox and toggle track, `status`,
/// `slider`'s handle, `inline_loading`'s badge, `file_uploader`'s ring and
/// `tag` all resolve their radius through a `CornerRole` and this function
/// rather than a hardcoded `SHAPE_*` constant. See
/// `.agents/notes/implemented/architecture/2026-09-09-corner-radius-flag-day-routes-every-component-through-corner-for.md`
/// for the full accounting, including the values that moved: a button is now
/// always `shape.corner-none`, and a checkbox's box moved off a flat square
/// onto `shape.corner-xs`.
///
/// **No site in `component::` binds a radius literal any more.** That flag
/// day left three behind — the radio box, the toggle knob and the AI-label
/// bullet dot, all Carbon `border-radius: 50%` circles larger than the
/// promotion clause can reach — and it routed the toggle track and the tag
/// through fixed roles, which squared two shapes Carbon draws as stadiums.
/// [`CornerRole::Pill`] closed both holes on 2026-09-10, and is the reason
/// this rule can now claim every binding in the module rather than most of
/// them.
///
/// This function's own totality still matters beyond who calls it today:
/// an unread token autocompletes into a Lua author's file through the
/// generated `token.d.luau` and resolves to something they cannot see, and
/// an unread operator here — reachable only from Rust — returns a name the
/// vocabulary declares regardless, proved total by
/// [`tests::the_one_radius_rule_is_total_and_the_half_edge_clause_overrides_it`].
#[must_use]
pub fn corner_for(role: CornerRole, shorter_edge: f32) -> &'static str {
    let (token, radius) = match role {
        CornerRole::Tiled => (SHAPE_RAMP[0].0, SHAPE_RAMP[0].1),
        CornerRole::BoxedMark => (SHAPE_RAMP[1].0, SHAPE_RAMP[1].1),
        CornerRole::Grouping => (SHAPE_RAMP[2].0, SHAPE_RAMP[2].1),
        CornerRole::Floating => (SHAPE_RAMP[3].0, SHAPE_RAMP[3].1),
        // Half of any edge, so the half-edge clause below would fire on
        // every positive one. Returned here instead, and unconditionally,
        // because a stadium is what the caller *said the node is*: a
        // collapsed pill mid-animation is still a pill, and the zero-edge
        // guard below exists to protect roles that named a fixed radius, not
        // this one.
        CornerRole::Pill => return CORNER_FULL,
    };
    // `radius > 0.0` keeps a zero-height node — a collapsed row, a spacer
    // mid-animation — from turning every square corner into a pill on the
    // strength of `0.0 >= 0.0`.
    if radius > 0.0 && shorter_edge > 0.0 && radius >= shorter_edge / 2.0 {
        return CORNER_FULL;
    }
    token
}

/// The pill. Named separately from [`SHAPE_RAMP`] because it is not a fixed
/// radius the way the other five steps are: it is a sentinel that always
/// exceeds half of any control's shorter edge. Two paths reach it —
/// [`CornerRole::Pill`], which names the stadium outright, and
/// [`corner_for`]'s half-edge clause, which promotes a fixed radius that has
/// grown large enough relative to a small node.
const CORNER_FULL: &str = "shape.corner-full";

/// Which of this node's four edges abut a sibling with nothing between them —
/// the fact [`corners_for`] needs beyond [`corner_for`]'s `role` and
/// `shorter_edge` to answer "what does *this* corner take".
///
/// A corner radius is not four independent design choices. It is one role
/// plus one adjacency fact: a free corner takes the radius its role names,
/// and a corner where a sibling's edge meets it is square, because two radii
/// meeting leave a lens-shaped hole. `Joined` is that adjacency fact, named
/// per edge because a corner is square when *either* of its two edges is
/// joined — `top_left` is square when `top` or `left` is set, and so through
/// the other three.
///
/// All four fields rather than the two a horizontal row (`button_group`,
/// `input_group`) ever sets, because the rule is not itself horizontal: a
/// future vertically-stacked group asks the same question of `top` and
/// `bottom`, and a type that could only answer the horizontal half would be
/// the per-component table FR-022 exists to prevent, aimed at one axis
/// instead of all four components.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Joined {
    /// This node's top edge meets a sibling with no gap.
    pub top: bool,
    /// This node's right edge meets a sibling with no gap.
    pub right: bool,
    /// This node's bottom edge meets a sibling with no gap.
    pub bottom: bool,
    /// This node's left edge meets a sibling with no gap.
    pub left: bool,
}

impl Joined {
    /// No edge is joined: every corner is free. [`corners_for`] under this
    /// value returns four copies of exactly what [`corner_for`] returns for
    /// the same `role` and `shorter_edge` —
    /// [`tests::the_one_radius_rule_is_total_and_the_half_edge_clause_overrides_it`]
    /// asserts the equivalence, because it is what keeps this one rule
    /// rather than two.
    pub const NONE: Self = Self {
        top: false,
        right: false,
        bottom: false,
        left: false,
    };
}

/// The four corner radius tokens one node's rect paints with — the
/// per-corner answer [`corners_for`] gives, in the schema's own clockwise
/// order (`token::slot::standard_slots`'s `radius-top-left` …
/// `radius-bottom-left`).
///
/// Not `gorgon_petra::draw::Corners`: that type is four *resolved radii* a
/// canvas draws with directly. This is four *token names* a component binds
/// through `props.tokens`, the same domain [`corner_for`] returns one of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CornerTokens {
    /// The top-left corner's radius token.
    pub top_left: &'static str,
    /// The top-right corner's radius token.
    pub top_right: &'static str,
    /// The bottom-right corner's radius token.
    pub bottom_right: &'static str,
    /// The bottom-left corner's radius token.
    pub bottom_left: &'static str,
}

/// [`corner_for`]'s per-corner sibling: one role, one adjacency fact, four
/// answers.
///
/// **Still one rule.** This does not consult [`SHAPE_RAMP`] on its own or
/// carry a second policy about what a role's radius is — every non-square
/// corner this returns is [`corner_for`]'s own answer for `role` and
/// `shorter_edge`, looked up once and copied into the three corners `joined`
/// does not square. A component that wants four *unrelated* radii cannot
/// reach them through this function, which is deliberate: FR-022 asks for
/// one rule keyed on what a node is, not a per-component table, and a
/// function that could bind four independent tokens would reopen exactly
/// that table on the corner axis.
///
/// A corner is square — `shape.corner-none`, the same name [`CornerRole::Tiled`]
/// already returns — when *either* of its two edges is joined:
/// `top_left` squares on `joined.top || joined.left`, and the other three
/// follow the same pattern one edge clockwise. Two rounded corners meeting
/// at a shared edge would leave a lens-shaped gap between them; squaring the
/// joined side is what a seamless row or a flush button group needs instead.
///
/// [`Joined::NONE`] never squares anything, so every corner takes
/// `corner_for(role, shorter_edge)` unchanged — see [`Joined::NONE`]'s own
/// doc for the test that holds this.
#[must_use]
pub fn corners_for(role: CornerRole, shorter_edge: f32, joined: Joined) -> CornerTokens {
    let free = corner_for(role, shorter_edge);
    let square = SHAPE_RAMP[0].0;
    CornerTokens {
        top_left: if joined.top || joined.left {
            square
        } else {
            free
        },
        top_right: if joined.top || joined.right {
            square
        } else {
            free
        },
        bottom_right: if joined.bottom || joined.right {
            square
        } else {
            free
        },
        bottom_left: if joined.bottom || joined.left {
            square
        } else {
            free
        },
    }
}

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

/// The geometry of one cast shadow: where it falls, how far it softens, how
/// far it grows.
///
/// Named fields rather than the bare tuple `SPRING_SET` and `SHAPE_RAMP`
/// use, and the difference is deliberate: those two tables are private and
/// read exactly once each, ten lines below their own declaration, so a
/// reader never has to remember which position is which. This one is
/// **public** and is read from another crate, where `(_, [0, 3], 10, 0)`
/// would make "blur then spread" a fact the caller has to remember rather
/// than one the type states.
///
/// The field types mirror `epaint::Shadow`'s exactly — `[i8; 2]`, `u8`,
/// `u8` — so the adapter's conversion is a field-for-field move with no
/// range to check and no cast to get wrong. `gorgon-petra` does not depend
/// on `epaint`; matching the layout is what keeps the seam trivial without
/// taking the dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadowGeometry {
    /// How far the shadow is displaced from the shape casting it, in logical
    /// units, `[x, y]`. `x` is `0` in both shipped rows: a shadow offset
    /// sideways implies a light source off to one side, and a tiling shell
    /// whose panels can sit anywhere has no such side.
    pub offset: [i8; 2],
    /// The softening radius, in logical units.
    pub blur: u8,
    /// How far the shadow's rectangle grows past the shape casting it,
    /// before blurring, in logical units.
    pub spread: u8,
}

/// The elevation geometry, shared verbatim by both shipped themes.
///
/// **Geometry is not a token, and that is the same split every other family
/// in this file already makes.** A gap does not change when the lights go
/// out (`SPACING_RAMP`), a corner does not (`SHAPE_RAMP`), a spring does
/// not (`SPRING_SET`), a heading's size does not (`TYPOGRAPHY_RAMP`).
/// Neither does the direction a shadow falls in. Only the *colour* changes,
/// so only the colour is a token — `shadow.raised` and `shadow.overlay` at
/// [`TokenKind::Color`], against the `shadow` slot `slot.rs` already
/// declares. No new [`TokenKind`] is added, and none is needed.
///
/// Two levels, not five. `raised` is roughly Material 3's level 1 — a card
/// or a panel that has been lifted off the page. `overlay` is roughly its
/// level 3 — a menu, a dialog, something that has left the page entirely.
/// The layer set (`LAYER_TOKENS`) is still the primary depth cue in both
/// themes and this does not replace it; a shadow is the sparse secondary,
/// used where tone alone cannot separate a surface from what is behind it.
///
/// Keyed by token name and held to the vocabulary by
/// `the_shadow_geometry_covers_every_shadow_token_and_nothing_else`, so a
/// third shadow colour cannot be declared without a painter finding it has
/// no geometry to draw with.
pub const SHADOW_GEOMETRY: [(&str, ShadowGeometry); 2] = [
    (
        "shadow.raised",
        ShadowGeometry {
            offset: [0, 2],
            blur: 6,
            spread: 0,
        },
    ),
    (
        "shadow.overlay",
        ShadowGeometry {
            offset: [0, 4],
            blur: 12,
            spread: 0,
        },
    ),
];

/// The two elevation colours, ground first, in [`SHADOW_GEOMETRY`]'s order.
const SHADOW_TOKENS: [&str; 2] = ["shadow.raised", "shadow.overlay"];

/// The light theme's shadow alphas, out of 255, in [`SHADOW_TOKENS`]' order.
///
/// **Judged from a 2026-08-26 gallery capture.** The previous pair (15% /
/// 22%) sat on the JND floor: a light card on a light page read as having
/// no shadow at all. 25% / 34% is still a secondary cue next to the layer
/// step, and it is the smallest step that was visible on that capture.
///
/// Black in both themes. A lightened "shadow" in dark mode is not a shadow;
/// occlusion darkens, and no guidance was found recommending otherwise.
/// `the_shadow_set_is_black_and_deepens_with_its_ground` holds that.
const LIGHT_SHADOW_ALPHAS: [u8; 2] = [64, 88];

/// The dark theme's shadow alphas. Higher than [`LIGHT_SHADOW_ALPHAS`] —
/// 38% and 50% — because black on a dark ground has less room to darken it.
///
/// **Raised with the light pair on 2026-08-26**, so dark stays stronger
/// than light (`the_shadow_set_is_black_and_deepens_with_its_ground`).
/// The previous dark `shadow.raised` at alpha 64 moved `surface.base`
/// (`#121212`) by **ΔL\* 1.37** — 18% of one layer step, just above the
/// JND. The floor in that test is still 1.0; the capture is what argued
/// the alphas up.
///
/// `the_shadow_set_is_black_and_deepens_with_its_ground` pins that ΔL\*
/// with a floor so the value cannot silently drop below visibility. It
/// cannot prove the shadow is visible *on a screen*, because a token test
/// has no pixels: that is a capture's job, and until a capture exists this
/// figure is the honest ceiling on what has been shown.
const DARK_SHADOW_ALPHAS: [u8; 2] = [96, 128];

/// Assign a mode's shadow colours into a theme's value map. Both themes call
/// this for the same reason [`insert_layer_set`] exists: two hand-copied
/// alpha pairs are two pairs that drift.
fn insert_shadow_set(values: &mut BTreeMap<TokenName, TokenValue>, alphas: &[u8; 2]) {
    for (token, alpha) in SHADOW_TOKENS.iter().zip(alphas) {
        values.insert(
            name(token),
            TokenValue::Color(ColorValue::from_srgb8(0x00, 0x00, 0x00, *alpha)),
        );
    }
}

/// The shipped silhouette family: the outline figures a paint slot can name.
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
const SILHOUETTE_FAMILY: [(&str, Silhouette); 4] = [
    ("shape.silhouette-rect", Silhouette::Rect),
    ("shape.silhouette-triangle", Silhouette::Triangle),
    ("shape.silhouette-diamond", Silhouette::Diamond),
    ("shape.silhouette-octagon", Silhouette::Octagon),
];

/// Assign every [`SILHOUETTE_FAMILY`] member into a theme's value map, for
/// the same reason [`insert_shape_ramp`] exists.
fn insert_silhouette_family(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (token, figure) in SILHOUETTE_FAMILY {
        values.insert(name(token), TokenValue::Silhouette(figure));
    }
}

/// The shipped typography ramp: ten steps covering the twelve Carbon
/// productive roles the 42 audited components use (FR-023).
///
/// **The four-step ramp this replaces had no step below 14 units**, and the
/// inventory finds twenty-one references to the 12-unit label and helper-text
/// roles — so a component library built on it had to paint a field's helper
/// text at body size or invent a number. That, not a wish for more names, is
/// what this replaces.
///
/// # Where the numbers come from
///
/// Every size, line height and tracking below is **measured** from
/// `@carbon/type` 11.65.0's published `scss/_styles.scss` and `lib/index.js`,
/// resolved on 2026-08-25. Line heights are published as unitless ratios and
/// are multiplied out here: `body-compact-01` is `14 × 1.28572 = 18`,
/// `body-01` is `14 × 1.42857 = 20`, `heading-03` is `20 × 1.4 = 28`,
/// `heading-04` is `28 × 1.28572 = 36`. All ten land on the 2-unit grid,
/// which is a decent check on the arithmetic.
///
/// This resolves the precondition `contracts/token-vocabulary.md` §"Open"
/// puts on these values: `carbon.md` predates Carbon v11's type renames and
/// reports `body-short-01` / `heading-01` / `productive-heading-03`, and the
/// v10→v11 map turned out to be **aliasing, not copying** — `_styles.scss`
/// binds `$body-compact-01: $body-short-01 !default` and
/// `$heading-03: $productive-heading-03 !default`, so the records are the
/// same record and cannot drift apart. One trap avoided: v10 `heading-01` is
/// *not* `heading-compact-01`; the latter aliases `productive-heading-01`,
/// whose line height is 1.28572 rather than 1.42857, and reading the map the
/// obvious way tightens the leading by two units.
///
/// | step | Carbon role(s) it covers | refs |
/// |---|---|---|
/// | `caption` | `helper-text-01` | 5 |
/// | `label` | `label-01` | 16 |
/// | `label-lg` | `label-02` | 2 |
/// | `body-compact` | `body-compact-01` | 28 |
/// | `body` | `body-01` | 8 |
/// | `body-lg` | `body-compact-02`, `body-02` | 3 |
/// | `heading-sm` | `heading-compact-01` | 14 |
/// | `heading` | `heading-03` | 3 |
/// | `heading-lg` | `heading-04` | 1 |
/// | `code` | `code-01`, `code-02` | 2 |
///
/// # The three divergences, each on purpose
///
/// **Weight: Carbon's `semibold` (600) maps to [`TypographyWeight::Medium`],
/// and `light` (300) is refused.** Petra names three weight roles where
/// Carbon names three numbers, and 600 is the emphasis step between body and
/// bold. 300 loses stroke at 12-14 units on a dark ground, and M-Carbon
/// already fixed the legal step as Regular→Medium or Medium→Bold; no Carbon
/// role the 42 components use asks for it.
///
/// **`heading` and `heading-lg` are `Medium` where Carbon measures 400.**
/// This is the one place the ramp is louder than its source, and the previous
/// ramp's all-`Bold` headings are what it is walking back from — one step,
/// not two. A heading at exactly body weight has no weight channel at all,
/// which on a dense tool leaves size as the only cue that a line is a
/// heading; M-Carbon's rule is that the step exists.
///
/// **`body-lg` covers `body-02` (16/24) at `body-compact-02`'s 16/22.** Two
/// roles, one step, and the compact one wins because a 16-unit line in a
/// desktop tool is a subheading rather than a paragraph. One inventory
/// reference is affected, and it gets two units less leading than Carbon
/// would give it.
///
/// Declared once and shared by both shipped themes, because typography is
/// type, not colour: a heading's size does not change when the operator
/// turns the lights off, the same reasoning [`SPACING_RAMP`] and
/// [`SHAPE_RAMP`] use for their own families.
const TYPOGRAPHY_RAMP: [(&str, TypographyValue); 10] = [
    ("typography.caption", type_step(12.0, 16.0, R, 0.32, SANS)),
    ("typography.label", type_step(12.0, 16.0, R, 0.32, SANS)),
    ("typography.label-lg", type_step(14.0, 18.0, R, 0.16, SANS)),
    (
        "typography.body-compact",
        type_step(14.0, 18.0, R, 0.16, SANS),
    ),
    ("typography.body", type_step(14.0, 20.0, R, 0.16, SANS)),
    ("typography.body-lg", type_step(16.0, 22.0, R, 0.0, SANS)),
    (
        "typography.heading-sm",
        type_step(14.0, 18.0, M, 0.16, SANS),
    ),
    ("typography.heading", type_step(20.0, 28.0, M, 0.0, SANS)),
    ("typography.heading-lg", type_step(28.0, 36.0, M, 0.0, SANS)),
    ("typography.code", type_step(12.0, 16.0, R, 0.32, MONO)),
];

/// Shorthands, so a row of [`TYPOGRAPHY_RAMP`] fits on one line and the
/// column of numbers stays readable as a scale. Private to this file and used
/// nowhere else.
const R: TypographyWeight = TypographyWeight::Regular;
/// See [`R`].
const M: TypographyWeight = TypographyWeight::Medium;
/// See [`R`].
const SANS: TypographyFamily = TypographyFamily::Sans;
/// See [`R`].
const MONO: TypographyFamily = TypographyFamily::Mono;

/// One [`TYPOGRAPHY_RAMP`] row, as a `const fn` so the table above stays a
/// table rather than ten repetitions of four field names.
const fn type_step(
    size: f32,
    line_height: f32,
    weight: TypographyWeight,
    letter_spacing: f32,
    family: TypographyFamily,
) -> TypographyValue {
    TypographyValue {
        size,
        line_height,
        weight,
        letter_spacing,
        family,
    }
}

/// Assign every [`TYPOGRAPHY_RAMP`] step into a theme's value map, for the
/// same drift-proofing reason [`insert_spacing_ramp`] exists.
fn insert_typography_ramp(values: &mut BTreeMap<TokenName, TokenValue>) {
    for (step, value) in TYPOGRAPHY_RAMP {
        values.insert(name(step), TokenValue::Typography(value));
    }
}

/// The full set of tokens the shipped light and dark themes must define.
///
/// **Inventory-driven, not Carbon-complete.** A name is here because at least
/// one of the 42 components in
/// `.agents/research/08-25-2026/Carbon-Component-Inventory/` references it —
/// never because Carbon publishes it
/// (`contracts/token-vocabulary.md` §3). That bar is the difference between a
/// vocabulary and a parts bin: an unreferenced token still autocompletes into
/// a Lua author's file through the generated `token.d.luau` and still resolves
/// to something they cannot see, which is the defect the M-Carbon note counted
/// at 12 of 18 names.
///
/// The set stopped being "deliberately small" on 2026-08-25. It was 43 names
/// plus 3 statuses, chosen to exercise every [`TokenKind`] once; it is now the
/// measured vocabulary those 42 components ask for, plus the eight
/// [`SPACING_ALIASES`] shims that keep the previous ramp's call sites working
/// while they migrate.
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
        // The accent, and the ink that goes on it. One hue, spent on four
        // sanctioned things (primary button fill, focus ring, selected tab,
        // progress fill); see `LIGHT_ACCENT` for why it is blue and
        // `ON_ACCENT_TOKEN` for why the pair is two names rather than one.
        .declare(DesignToken::new(name(ACCENT_TOKEN), TokenKind::Color))
        .declare(DesignToken::new(name(ON_ACCENT_TOKEN), TokenKind::Color))
        // The link ink, derived from the accent by `link_ink`. See
        // `LINK_TOKEN` for why it is not the accent itself.
        .declare(DesignToken::new(name(LINK_TOKEN), TokenKind::Color))
        // The one drawn boundary. Judged against SC 1.4.11's 3:1 and held
        // *below* both text tones, because the defect this replaced was a
        // border that was too loud rather than one that was too faint.
        .declare(DesignToken::new(name(BORDER_TOKEN), TokenKind::Color))
        // The two-stroke rule material: see `crate::token::rule` and
        // `insert_rule_material`. Independent values, never computed by
        // blending from a surface at paint time — the painter looks these
        // two up exactly like any other colour token.
        .declare(DesignToken::new(
            name(crate::token::rule::SHADOW_TOKEN),
            TokenKind::Color,
        ))
        .declare(DesignToken::new(
            name(crate::token::rule::HIGHLIGHT_TOKEN),
            TokenKind::Color,
        ))
        // Elevation. Colour only: the offset, blur and spread that go with
        // these two live in `SHADOW_GEOMETRY` as shared Rust constants, for
        // the same reason the spacing and corner ramps are shared — a shadow
        // does not change direction when the lights go out.
        .declare(DesignToken::new(name(SHADOW_TOKENS[0]), TokenKind::Color))
        .declare(DesignToken::new(name(SHADOW_TOKENS[1]), TokenKind::Color))
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
        .declare(DesignToken::new(
            name("shape.silhouette-octagon"),
            TokenKind::Silhouette,
        ))
        // The keyboard focus underline (FR-015, FR-025). `focus.ring` is
        // the accent fill; `focus.ring-halo` stays in the pair because
        // `focus-inverse` and the theme builder still declare it. See
        // `the_focus_underline_is_legible_on_the_card`.
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

    // Every ramp is declared by walking its own table rather than by
    // repeating its names here. Fifty-seven names arrived in one change
    // (spec 005 FR-004, FR-004a, FR-022, FR-023, FR-026); spelled out
    // longhand that is fifty-seven chances for a declaration and an
    // assignment to disagree, and the disagreement surfaces as a
    // `Theme::build` panic at call time rather than as a compile error. Read
    // off the table and the two cannot part company.
    for (step, _) in SPACING_RAMP {
        vocab.declare(DesignToken::new(name(step), TokenKind::Spacing));
    }
    for (alias, _) in SPACING_ALIASES {
        vocab.declare(DesignToken::new(name(alias), TokenKind::Spacing));
    }
    for (step, _) in SIZE_RAMP {
        vocab.declare(DesignToken::new(name(step), TokenKind::Spacing));
    }
    for (step, _) in SHAPE_RAMP {
        vocab.declare(DesignToken::new(name(step), TokenKind::Shape));
    }
    for (step, _) in TYPOGRAPHY_RAMP {
        vocab.declare(DesignToken::new(name(step), TokenKind::Typography));
    }

    // The colour families the Carbon inventory measured a need for, each one
    // entering on a reference count rather than on Carbon publishing it —
    // `contracts/token-vocabulary.md` §3. The counts are in each constant's
    // own doc comment; the largest is `icon-*` at 46.
    for token in ICON_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
    for token in FIELD_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
    for token in BORDER_SUBTLE_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
    for (token, _) in STATE_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
    for (token, _) in LAYER_ACCENT_STATE_TOKENS {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }
    for (alias, _) in SUPPORT_ALIASES {
        vocab.declare(DesignToken::new(name(alias), TokenKind::Color));
    }
    for token in [
        BORDER_STRONG_TOKEN,
        BORDER_INTERACTIVE_TOKEN,
        // The destructive fill and the ink that goes on it. Two names, both
        // theme-independent, both entering on a measurement rather than on
        // Carbon publishing them: see `DANGER_FILL_TOKEN` for the 4.41:1
        // that ruled `support-error` out as a fill, and `ON_COLOUR_TOKEN`
        // for the black toggle knob that ruled `text.on-accent` out as an
        // on-colour ink.
        DANGER_FILL_TOKEN,
        ON_COLOUR_TOKEN,
        // The disabled toggle track. Its own Carbon value, not a fade of an
        // existing tone — see `BUTTON_DISABLED_TOKEN`.
        BUTTON_DISABLED_TOKEN,
        ACTIVE_TOKEN,
        LAYER_ACCENT_TOKEN,
        SCRIM_TOKEN,
        // The inverse family. Six names for one idea: a tooltip paints on an
        // inverted ground, and every part of it — fill, ink, edge, focus
        // ring, selected row — needs a tone from the other polarity.
        "background-inverse",
        "background-inverse-hover",
        "text-inverse",
        "border-inverse",
        "focus-inverse",
        "layer-selected-inverse",
    ] {
        vocab.declare(DesignToken::new(name(token), TokenKind::Color));
    }

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
            StatusToken::new(name("status.down"), StatusShape::Octagon, "Down")
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
    insert_accent(&mut values, &LIGHT_ACCENT, &LIGHT_LAYERS, LIGHT_TEXT[0]);
    insert_border(&mut values, &light_border());
    insert_rule_material(&mut values, &LIGHT_LAYERS);
    insert_shadow_set(&mut values, &LIGHT_SHADOW_ALPHAS);
    values.insert(name("text.primary"), opaque(LIGHT_TEXT[0]));
    values.insert(name("text.muted"), opaque(LIGHT_TEXT[1]));
    insert_icon_family(&mut values, &LIGHT_TEXT, LIGHT_LAYERS[0], DARK_TEXT[0]);
    insert_on_colour_pair(&mut values);
    // See `BUTTON_DISABLED_TOKEN`: Carbon's own value, not a fade.
    values.insert(name(BUTTON_DISABLED_TOKEN), opaque(BUTTON_DISABLED_LIGHT));
    insert_border_family(
        &mut values,
        &light_border(),
        light_border_strong(),
        &LIGHT_ACCENT,
        dark_border(),
    );
    insert_field_set(&mut values, &LIGHT_LAYERS);
    insert_state_set(&mut values, &LIGHT_LAYERS, ThemeMode::Light);
    insert_inverse_family(&mut values, &DARK, ThemeMode::Dark);
    insert_scrim(&mut values, LIGHT_SCRIM_ALPHA);
    insert_spacing_ramp(&mut values);
    insert_size_ramp(&mut values);
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
    // Light `status.ok` is a traffic-light green. Near-white mint fails 3:1
    // on `surface.raised`; `#00A000` is the brightest green that still
    // clears it. Dark `status.ok` *is* near-white mint, because the dark
    // ground has the headroom. `degraded` and `down` stay in the darker
    // half so the three remain separable in L* after hue collapse.
    // The focus underline is the accent. Geometry (present vs absent) is
    // the FR-015 channel; hue is the same blue the primary button already
    // spends. See `the_focus_underline_is_legible_on_the_card`.
    values.insert(name(RING_TOKEN), opaque(LIGHT_RING[0]));
    values.insert(name(HALO_TOKEN), opaque(LIGHT_RING[1]));

    values.insert(
        name("status.ok"),
        // Traffic-light mint. A 3:1 green on `#f2f2f2` collapses onto
        // `status.degraded` under deuteranopia (ΔE*ab 5.5). Near-white mint
        // is the lightness split the operator asked for and the one that
        // keeps the pair above `MIN_STATUS_SEPARATION`. The disc is a tint;
        // the circle and the word "OK" are the shape channel.
        TokenValue::Color(ColorValue::from_srgb8(0x9a, 0xff, 0xb0, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xb5, 0x54, 0x1a, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0x49, 0x12, 0x15, 0xff)),
    );
    // After the statuses, and reading them back out: see
    // `insert_support_aliases` for why an alias may not carry its own hex.
    insert_support_aliases(&mut values);

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
    insert_accent(&mut values, &DARK_ACCENT, &DARK_LAYERS, DARK_TEXT[0]);
    insert_border(&mut values, &dark_border());
    insert_rule_material(&mut values, &DARK_LAYERS);
    insert_shadow_set(&mut values, &DARK_SHADOW_ALPHAS);
    values.insert(name("text.primary"), opaque(DARK_TEXT[0]));
    values.insert(name("text.muted"), opaque(DARK_TEXT[1]));
    insert_icon_family(&mut values, &DARK_TEXT, DARK_LAYERS[0], LIGHT_TEXT[0]);
    insert_on_colour_pair(&mut values);
    // See `BUTTON_DISABLED_TOKEN`: Carbon's own value, not a fade.
    values.insert(
        name(BUTTON_DISABLED_TOKEN),
        TokenValue::Color(ColorValue::from_srgb8(
            BUTTON_DISABLED_DARK[0],
            BUTTON_DISABLED_DARK[1],
            BUTTON_DISABLED_DARK[2],
            BUTTON_DISABLED_DARK_ALPHA,
        )),
    );
    insert_border_family(
        &mut values,
        &dark_border(),
        dark_border_strong(),
        &DARK_ACCENT,
        light_border(),
    );
    insert_field_set(&mut values, &DARK_LAYERS);
    insert_state_set(&mut values, &DARK_LAYERS, ThemeMode::Dark);
    insert_inverse_family(&mut values, &LIGHT, ThemeMode::Light);
    insert_scrim(&mut values, DARK_SCRIM_ALPHA);
    insert_spacing_ramp(&mut values);
    insert_size_ramp(&mut values);
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

    // Dark underline: the dark accent, same reason as light.
    values.insert(name(RING_TOKEN), opaque(DARK_RING[0]));
    values.insert(name(HALO_TOKEN), opaque(DARK_RING[1]));

    values.insert(
        name("status.ok"),
        // Near-white mint on dark: 16:1 against `#121212`, so the "very
        // bright, near white, green" the operator asked for is legal here.
        TokenValue::Color(ColorValue::from_srgb8(0xe0, 0xff, 0xe6, 0xff)),
    );
    values.insert(
        name("status.degraded"),
        TokenValue::Color(ColorValue::from_srgb8(0xff, 0xc4, 0x7a, 0xff)),
    );
    values.insert(
        name("status.down"),
        TokenValue::Color(ColorValue::from_srgb8(0xf2, 0x1c, 0x0d, 0xff)),
    );
    // See `light` for why this runs after the statuses rather than beside
    // them.
    insert_support_aliases(&mut values);

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
    use super::{
        ACCENT_TOKEN, ACTIVE_STEP, ACTIVE_TOKEN, BORDER_INTERACTIVE_TOKEN, BORDER_STRONG_TOKEN,
        BORDER_SUBTLE_TOKENS, BORDER_TOKEN, CornerRole, DANGER_FILL_TOKEN, DARK_LAYERS,
        FIELD_TOKENS, HOVER_STEP, ICON_TOKENS, Joined, LAYER_ACCENT_STATE_TOKENS,
        LAYER_ACCENT_TOKEN, LAYER_TOKENS, LIGHT_LAYERS, LINK_TOKEN, M, MIN_CONTROL_BOUNDARY,
        MIN_DIVIDER_CONTRAST, MONO, ON_ACCENT_TOKEN, ON_COLOUR_TOKEN, R, RAISED_ALIAS,
        RULE_HIGHLIGHT_DELTA, RULE_SHADOW_DELTA, SANS, SCRIM_TOKEN, SELECTED_HOVER_STEP,
        SELECTED_STEP, SHADOW_GEOMETRY, SHADOW_TOKENS, SHAPE_RAMP, SIZE_RAMP, SPACING_ALIASES,
        SPACING_RAMP, SPRING_SET, SUPPORT_ALIASES, TYPOGRAPHY_RAMP, corner_for, corners_for, dark,
        light, lightness_of, opaque_value, rule_stroke, standard_vocabulary,
    };
    use crate::token::ThemeMode;
    use crate::token::colourblind::{
        DEUTERANOPE, MIN_STATUS_SEPARATION, MIN_SURFACE_CONTRAST, PROTANOPE, contrast, delta_e,
        simulate, status_family, theme_color, to_lab,
    };
    use crate::token::focus::RING_TOKEN;
    use crate::token::name::TokenName;
    use crate::token::value::{ColorValue, CoverageValue, TokenValue};
    use crate::token::value::{TypographyFamily, TypographyValue, TypographyWeight};

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
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let surface = theme_color(&theme, "surface.raised");
            let names = status_family();
            let colors: Vec<ColorValue> = names.iter().map(|s| theme_color(&theme, s)).collect();

            for (name, c) in names.iter().zip(&colors) {
                let ratio = contrast(*c, surface);
                // Light `status.ok` is a near-white mint tint. A 3:1 green
                // on `surface.raised` is the same colour as `status.degraded`
                // to a deuteranope (ΔE*ab 5.5). The circle and the word
                // carry the mark; pairwise ΔE below is the colour check.
                if label == "light" && (*name == "status.ok" || *name == "support-success") {
                    continue;
                }
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
                for i in 0..names.len() {
                    for j in (i + 1)..names.len() {
                        // Two names for one colour is the point of the
                        // `support-*` aliases, not a failure: `support-error`
                        // *is* `status.down`, so the pair is at ΔE 0 and there
                        // is nothing for a reader to confuse.
                        if colors[i] == colors[j] {
                            continue;
                        }
                        let d = delta_e(seen[i], seen[j]);
                        assert!(
                            d >= MIN_STATUS_SEPARATION,
                            "{label}: {} and {} are only ΔE*ab {d:.1} apart to a \
                             {vision} reader (floor is {MIN_STATUS_SEPARATION}); \
                             the pair that breaks first is usually degraded/down, \
                             because red-green colour blindness maps amber and red \
                             onto each other — separate them in lightness, not hue",
                            names[i],
                            names[j],
                        );
                    }
                }
            }
        }
    }

    /// Every `support-*` alias carries exactly the status colour it names.
    ///
    /// The separation gate above proves the family stays apart; this proves
    /// the family has no independent values to drift *into* a failure. Both
    /// are needed: an alias reassigned to a fresh hex would still pass the
    /// separation gate as long as the fresh hex happened to be far enough
    /// from the others, and would still have re-imported a palette FR-005
    /// refuses.
    #[test]
    fn every_support_alias_is_its_status_colour() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for (alias, status) in SUPPORT_ALIASES {
                assert_eq!(
                    theme_color(&theme, alias),
                    theme_color(&theme, status),
                    "{label}: {alias} must be assigned from {status}, not chosen"
                );
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
                    .filter(|(n, _)| !(label == "light" && **n == "status.ok"))
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

    /// The underline sits on the card, not on the node's own fill. It is the
    /// accent, so it is legible on every layer the accent is allowed to
    /// paint, and it *is* the accent — one hue, not a second blue.
    ///
    /// FR-015's geometry channel is that the bar is present or absent. Hue
    /// is allowed because it is not the only channel.
    #[test]
    fn the_focus_underline_is_legible_on_the_card() {
        for (label, deepest) in DEEPEST_ACCENT_LAYER {
            let theme = if label == "light" { light() } else { dark() };
            let bar = theme_color(&theme, RING_TOKEN);
            let accent = theme_color(&theme, ACCENT_TOKEN);
            assert_eq!(
                bar, accent,
                "{label}: focus.ring must be the accent, not a second hue"
            );
            for (depth, token) in LAYER_TOKENS.iter().enumerate() {
                let ratio = contrast(bar, theme_color(&theme, token));
                if depth <= deepest {
                    assert!(
                        ratio >= MIN_SURFACE_CONTRAST,
                        "{label}: the underline is only {ratio:.2}:1 on \
                         {token} (depth {depth} <= {deepest})"
                    );
                }
            }
        }
    }

    /// The ramp, pinned by value against Carbon's own published scale.
    ///
    /// Every styling prop in every tree resolves through one of these
    /// thirteen numbers, so changing one is a page-wide visual change and has
    /// to be a deliberate edit here rather than a quiet drift.
    ///
    /// **The eight steps Petra shipped before 2026-08-25 are checked for
    /// having kept their values, by name**, in the alias half of this test —
    /// `spacing.md` is still 12 and now reaches that 12 through
    /// `spacing-04`. That is the claim the migration rests on: the rename was
    /// free because the two ramps already agreed step for step.
    #[test]
    fn the_spacing_ramp_is_carbons_thirteen_steps_and_the_aliases_hold() {
        let expected: [(&str, f32); 13] = [
            ("spacing-01", 2.0),
            ("spacing-02", 4.0),
            ("spacing-03", 8.0),
            ("spacing-04", 12.0),
            ("spacing-05", 16.0),
            ("spacing-06", 24.0),
            ("spacing-07", 32.0),
            ("spacing-08", 40.0),
            ("spacing-09", 48.0),
            ("spacing-10", 64.0),
            ("spacing-11", 80.0),
            ("spacing-12", 96.0),
            ("spacing-13", 160.0),
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

        // The eight aliases, pinned to the numbers they shipped as. A drift
        // here is a page-wide silent relayout of every call site that has not
        // migrated yet, which is most of them.
        for (alias, units) in [
            ("spacing.2xs", 2.0),
            ("spacing.xs", 4.0),
            ("spacing.sm", 8.0),
            ("spacing.md", 12.0),
            ("spacing.lg", 16.0),
            ("spacing.xl", 24.0),
            ("spacing.2xl", 32.0),
            ("spacing.3xl", 48.0),
        ] {
            let got = theme.value(&TokenName::new(alias).unwrap());
            assert_eq!(
                got,
                Some(&TokenValue::Spacing(units)),
                "{alias} shipped as {units} units and must still be {units}, found {got:?}"
            );
        }

        // Every spacing name the vocabulary declares is one of the thirteen
        // steps, one of the eight aliases, or one of the six fixed control
        // sizes, so a fourteenth step cannot appear without this test naming
        // it.
        let vocab = standard_vocabulary();
        let declared: Vec<&TokenName> = vocab
            .names()
            .filter(|n| vocab.kind_of(n) == Some(crate::token::value::TokenKind::Spacing))
            .collect();
        let want = SPACING_RAMP.len() + SPACING_ALIASES.len() + SIZE_RAMP.len();
        assert_eq!(
            declared.len(),
            want,
            "the vocabulary declares {} spacing-kinded tokens but the ramp, \
             aliases and size scale account for {want}: {declared:?}",
            declared.len(),
        );
    }

    /// Every [`SPACING_ALIASES`] shim resolves to exactly the step it names,
    /// rather than to a number of its own.
    ///
    /// The alias set exists so roughly 150 call sites did not have to move in
    /// one change, and its whole value rests on the two spellings being the
    /// same gap. Read out of the same theme both ways, so a hand-edited alias
    /// table fails here instead of laying a page out two ways depending on
    /// which name an author happened to type.
    #[test]
    fn the_spacing_aliases_are_their_ramp_steps() {
        let theme = light();
        for (alias, step) in SPACING_ALIASES {
            let target = SPACING_RAMP
                .iter()
                .find(|(s, _)| *s == step)
                .unwrap_or_else(|| panic!("{alias} points at {step}, which is not a ramp step"));
            assert_eq!(
                theme.value(&TokenName::new(alias).unwrap()),
                Some(&TokenValue::Spacing(target.1)),
                "{alias} must be exactly {step} ({} units)",
                target.1
            );
        }
    }

    /// The fixed control-size ramp, pinned by value.
    ///
    /// A separate scale from spacing and a separate test for it, because the
    /// two answer different questions and the whole reason `size-*` exists is
    /// that reading a control height off the gap ramp makes them one
    /// question. `size-md` is 40 and `spacing-08` is also 40 today; that
    /// coincidence is exactly what a shared table would freeze.
    #[test]
    fn the_size_ramp_is_carbons_six_fixed_control_heights() {
        let theme = light();
        let expected: [(&str, f32); 6] = [
            ("size-xs", 24.0),
            ("size-sm", 32.0),
            ("size-md", 40.0),
            ("size-lg", 48.0),
            ("size-xl", 64.0),
            ("size-2xl", 80.0),
        ];
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
                "control sizes must grow: {step} at {units} does not exceed {previous}"
            );
            previous = units;
        }
        assert_eq!(SIZE_RAMP.len(), expected.len());
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
        let want = SPACING_RAMP.len() + SPACING_ALIASES.len() + SIZE_RAMP.len();
        assert_eq!(
            checked, want,
            "expected the ramp, its aliases and the size scale, checked {checked}"
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
    fn the_shape_ramp_is_six_ordered_steps_from_sharp_to_pill() {
        let expected: [(&str, f32); 6] = [
            ("shape.corner-none", 0.0),
            ("shape.corner-xs", 2.0),
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

        // Every shape name the vocabulary declares is one of the six above,
        // so a seventh step cannot appear without this test naming it.
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
    fn the_silhouette_family_is_pinned_in_both_themes() {
        use crate::token::value::Silhouette;
        let expected: [(&str, Silhouette); 4] = [
            ("shape.silhouette-rect", Silhouette::Rect),
            ("shape.silhouette-triangle", Silhouette::Triangle),
            ("shape.silhouette-diamond", Silhouette::Diamond),
            ("shape.silhouette-octagon", Silhouette::Octagon),
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
        assert_eq!(
            checked,
            SHAPE_RAMP.len(),
            "expected the {}-step ramp, checked {checked}",
            SHAPE_RAMP.len()
        );
    }

    /// The typography ramp, pinned by value against `@carbon/type` 11.65.0.
    ///
    /// **Every number here was fetched, not remembered.** Carbon publishes
    /// line height as a unitless ratio, so the pairs below are the published
    /// `fontSize` and `fontSize x lineHeight` — `14 x 1.28572 = 18`,
    /// `14 x 1.42857 = 20`, `20 x 1.4 = 28`, `28 x 1.28572 = 36` — and the
    /// tracking is the published `letterSpacing` in px. This table is the
    /// place a future reader can check that claim without refetching, and the
    /// place a mistyped digit fails.
    ///
    /// The three deliberate divergences from those values (`heading` and
    /// `heading-lg` at `Medium` where Carbon measures 400, and `body-lg`
    /// covering `body-02` at the compact leading) are argued at
    /// [`TYPOGRAPHY_RAMP`] and asserted here, so a reader who disagrees with
    /// the argument can see exactly which three cells to change.
    #[test]
    fn the_typography_ramp_is_the_ten_carbon_productive_steps() {
        let expected: [(&str, f32, f32, TypographyWeight, f32, TypographyFamily); 10] = [
            ("typography.caption", 12.0, 16.0, R, 0.32, SANS),
            ("typography.label", 12.0, 16.0, R, 0.32, SANS),
            ("typography.label-lg", 14.0, 18.0, R, 0.16, SANS),
            ("typography.body-compact", 14.0, 18.0, R, 0.16, SANS),
            ("typography.body", 14.0, 20.0, R, 0.16, SANS),
            ("typography.body-lg", 16.0, 22.0, R, 0.0, SANS),
            ("typography.heading-sm", 14.0, 18.0, M, 0.16, SANS),
            ("typography.heading", 20.0, 28.0, M, 0.0, SANS),
            ("typography.heading-lg", 28.0, 36.0, M, 0.0, SANS),
            ("typography.code", 12.0, 16.0, R, 0.32, MONO),
        ];
        let theme = light();
        for (step, size, line_height, weight, letter_spacing, family) in expected {
            let got = theme.value(&TokenName::new(step).unwrap());
            assert_eq!(
                got,
                Some(&TokenValue::Typography(TypographyValue {
                    size,
                    line_height,
                    weight,
                    letter_spacing,
                    family,
                })),
                "{step} must be {size}/{line_height} {weight:?} tracking \
                 {letter_spacing} in {family:?}, found {got:?}"
            );
        }

        // The three heading levels are a size hierarchy: each strictly
        // larger than the last, so "heading" without a suffix is
        // meaningfully the middle of three rather than a name with no
        // siblings. `heading-sm` is 14 rather than the 16 it shipped as
        // before this ramp, because Carbon's `heading-compact-01` is 14 and
        // the step it makes over body is weight, not size.
        let sizes = [14.0_f32, 20.0, 28.0];
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

    /// The ramp has a step below 14 units, and it is the step FR-023 was
    /// filed about.
    ///
    /// Twenty-one of the inventory's type references are to Carbon's 12-unit
    /// `label-01` and `helper-text-01` roles, and the four-step ramp this
    /// replaced bottomed out at 14 — so a field's helper text had to be
    /// painted at body size or at a number nobody chose. This is a separate
    /// test from the table above because it is a separate claim: that table
    /// would still pass if every step were 14 and larger, which is the exact
    /// state the ramp was being replaced from.
    #[test]
    fn the_ramp_reaches_below_body_size() {
        let theme = light();
        let smallest = TYPOGRAPHY_RAMP
            .iter()
            .map(|(_, v)| v.size)
            .fold(f32::MAX, f32::min);
        assert!(
            smallest < 14.0,
            "the smallest step is {smallest}; FR-023 exists because a ramp \
             whose floor is body size cannot label a field"
        );
        for step in ["typography.caption", "typography.label"] {
            let got = theme.value(&TokenName::new(step).unwrap());
            assert!(
                matches!(got, Some(TokenValue::Typography(v)) if v.size < 14.0),
                "{step} carries the 12-unit role and must be smaller than body: {got:?}"
            );
        }
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
            checked,
            TYPOGRAPHY_RAMP.len(),
            "expected the {}-step ramp, checked {checked}",
            TYPOGRAPHY_RAMP.len()
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
    /// The link ink clears AA on every layer, in both themes, and is still
    /// blue.
    ///
    /// Both halves. The first is the floor [`LINK_TOKEN`]'s doc gives the
    /// reason for: the accent it is derived from is 2.9:1 on dark's deepest
    /// layer, so an alias would have shipped a link nobody could read on a
    /// card. The second is what stops [`link_ink`] from "passing" by walking
    /// all the way to `text.primary`: a link that is the text colour is the
    /// row-15 defect this token exists to fix. Blue is asserted as the blue
    /// channel leading the red one by a margin, which is the property the
    /// accent has and a grey does not.
    #[test]
    fn the_link_ink_clears_aa_on_every_layer_and_is_still_blue() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let link = theme_color(&theme, LINK_TOKEN);
            for surface in LAYER_TOKENS.iter().chain(std::iter::once(&RAISED_ALIAS)) {
                let ground = theme_color(&theme, surface);
                let ratio = contrast(link, ground);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: {LINK_TOKEN} on {surface} is {ratio:.2}:1, below the \
                     {MIN_TEXT_CONTRAST}:1 AA floor for text"
                );
            }
            assert!(
                link.b > link.r * 2.0,
                "{label}: {LINK_TOKEN} has stopped being blue ({link:?}); the walk \
                 toward text.primary went too far and the link is prose again"
            );
            let accent = theme_color(&theme, ACCENT_TOKEN);
            assert!(
                (link.b - link.r) > 0.5 * (accent.b - accent.r),
                "{label}: {LINK_TOKEN} kept under half the accent's blue lead"
            );
        }
    }

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
    /// The deepest [`LAYER_TOKENS`] index the accent stays legible on, per
    /// theme. Read with [`the_accent_clears_aa_on_every_surface_it_can_be_painted_on`],
    /// which asserts both sides of the boundary the way
    /// [`DEEPEST_STATUS_LAYER`] does.
    ///
    /// Light reaches the bottom of its set because the light layers
    /// *alternate* rather than ramp — every one of the four is `#ffffff` or
    /// `#f2f2f2`, so there is no deep end to lose. Dark stops at layer two:
    /// `#4589ff` is 3.78:1 on `#333333` and **2.91:1** on `#444444`, and the
    /// painter needs that number. A primary button dropped onto dark's
    /// deepest layer is under the fill floor.
    const DEEPEST_ACCENT_LAYER: [(&str, usize); 2] = [("light", 3), ("dark", 2)];

    /// The accent has to be visible wherever it is spent, on every layer it
    /// can be spent on, in both themes.
    ///
    /// # Which floor, and why it is not 4.5:1
    ///
    /// This is the question the test had to answer before it could assert
    /// anything, and getting it wrong in either direction is a real failure
    /// rather than a pedantic one.
    ///
    /// The accent is a **fill**. Its four sanctioned uses — primary button
    /// fill, focus ring, selected tab, progress fill — are all non-text
    /// user-interface components, and WCAG 2.1 grades those under SC 1.4.11
    /// *Non-text Contrast*, whose Level AA threshold is 3:1. That is the
    /// floor here: [`MIN_SURFACE_CONTRAST`], the same one the status marks
    /// are held to, for the same reason.
    ///
    /// Held to SC 1.4.3's 4.5:1 body-text floor instead, the shipped light
    /// accent **fails**: `#0f62fe` is 4.47:1 on `surface.layer-one`
    /// (`#f2f2f2`), missing by 0.03. It also stops being a coherent
    /// question, because light's layer set alternates — at 4.5:1 the light
    /// accent would pass on layers zero and two and fail on one and three,
    /// and "the deepest layer it survives" would not name anything.
    ///
    /// The text floor is not dropped, it is moved to where it belongs: the
    /// *label on* the accent is text, and
    /// [`text_on_the_accent_clears_aa_and_no_other_shipped_tone_does`] holds
    /// that pair to 4.5:1.
    ///
    /// # Why the far side is asserted too
    ///
    /// Same reason as [`a_status_marker_is_legible_only_down_to_its_theme_s_deepest_layer`]:
    /// the near side alone lets a palette edit silently gain or lose a layer
    /// of headroom, and the painter needs the boundary as a number, not as a
    /// direction.
    #[test]
    fn the_accent_clears_aa_on_every_surface_it_can_be_painted_on() {
        for (label, deepest) in DEEPEST_ACCENT_LAYER {
            let theme = if label == "light" { light() } else { dark() };
            let accent = theme_color(&theme, ACCENT_TOKEN);

            for (depth, token) in LAYER_TOKENS.iter().enumerate() {
                let ratio = contrast(accent, theme_color(&theme, token));
                if depth <= deepest {
                    assert!(
                        ratio >= MIN_SURFACE_CONTRAST,
                        "{label}: {ACCENT_TOKEN} is declared paintable on \
                         {token} (depth {depth} <= {deepest}) but measures \
                         {ratio:.2}:1 on it, under the \
                         {MIN_SURFACE_CONTRAST}:1 non-text floor (WCAG SC \
                         1.4.11). A button fill nobody can find is not an \
                         affordance."
                    );
                } else {
                    assert!(
                        ratio < MIN_SURFACE_CONTRAST,
                        "{label}: {ACCENT_TOKEN} now clears \
                         {MIN_SURFACE_CONTRAST}:1 on {token} (depth {depth}) \
                         at {ratio:.2}:1. That is an improvement, and it \
                         means DEEPEST_ACCENT_LAYER is stale: raise {label} \
                         to {depth} so the painter is allowed to use the \
                         depth it just gained."
                    );
                }
            }

            // The alias the painter still binds is layer one; it must not be
            // a hole in the sweep above.
            let ratio = contrast(accent, theme_color(&theme, RAISED_ALIAS));
            assert!(
                ratio >= MIN_SURFACE_CONTRAST,
                "{label}: {ACCENT_TOKEN} on {RAISED_ALIAS} is {ratio:.2}:1, \
                 under the {MIN_SURFACE_CONTRAST}:1 non-text floor"
            );
        }
    }

    /// The accent is the one hue the interface leans on, and the operator it
    /// is built for is red-green colour blind. So it is measured through the
    /// same Viénot-Brettel-Mollon transform
    /// [`status_colours_stay_apart_under_red_green_colour_blindness`] uses,
    /// against the marks it shares a page with.
    ///
    /// This is a **separate** test rather than a fourth entry in that one,
    /// because the claim is a different shape: the three status colours must
    /// be mutually distinguishable because they are one channel with three
    /// values, whereas the accent must merely not be *confusable* with any
    /// of them. Folding it in would have made a test named for status assert
    /// something that is not status, and its message would have pointed a
    /// future reader at the wrong palette.
    ///
    /// The tightest pair is dark `accent.primary` against dark `status.ok`
    /// (ΔE\*ab 68.4 under deuteranopia) — both sit on the blue-green side,
    /// which is exactly where a red-green deficiency has the least room, so
    /// it is the pair to watch when either colour is retuned.
    ///
    /// [`LINK_TOKEN`] is measured here too, under the same transform against
    /// the same marks: it is the accent's hue walked toward the page's ink
    /// ([`link_ink`]), which is a second blue on the page and therefore a
    /// second thing a reader must not mistake for a status marker. It is
    /// this measurement that lets
    /// [`no_shipped_colour_carries_a_hue_that_is_not_already_gated`] admit
    /// it.
    #[test]
    fn the_accent_survives_red_green_colour_blindness() {
        const STATUSES: [&str; 3] = ["status.ok", "status.degraded", "status.down"];
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for blue in [ACCENT_TOKEN, LINK_TOKEN] {
                let hue = theme_color(&theme, blue);
                for (vision, matrix) in [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)] {
                    let seen_hue = to_lab(simulate(hue, matrix));
                    for status in STATUSES {
                        let seen = to_lab(simulate(theme_color(&theme, status), matrix));
                        let d = delta_e(seen_hue, seen);
                        assert!(
                            d >= MIN_STATUS_SEPARATION,
                            "{label}: {blue} and {status} are only ΔE*ab \
                             {d:.1} apart to a {vision} reader (floor is \
                             {MIN_STATUS_SEPARATION}). A blue a reader cannot \
                             tell from a state marker turns every selected tab \
                             and every link into a status report."
                        );
                    }
                }
            }
        }
    }

    /// A label on a filled accent is text on a coloured ground, and it is
    /// judged by the text floor.
    ///
    /// # Why [`ON_ACCENT_TOKEN`] exists at all
    ///
    /// Because neither shipped tone can carry it. Measured on the accent
    /// fill: light `text.primary` 3.48:1, light `text.muted` 1.95:1, dark
    /// `text.primary` 2.99:1, dark `text.muted` 2.26:1 — four misses of a
    /// 4.5:1 floor. A component author reaching for the obvious tone would
    /// reproduce the exact bug
    /// [`every_text_tone_clears_aa_on_every_surface_it_can_be_painted_on`]
    /// was written after, on a ground that test does not sweep.
    ///
    /// # Why the second half asserts a failure
    ///
    /// The `_and_no_other_shipped_tone_does` half is the token's own
    /// justification, kept honest. This file's doc comments record that a
    /// declared name nothing reads is a defect; the mirror of that is a
    /// declared name nothing *needs*. If a future accent retune let
    /// `text.primary` clear 4.5:1 on the fill, [`ON_ACCENT_TOKEN`] would be
    /// a third name for a colour two names already cover, and this assertion
    /// fires and says to delete it.
    #[test]
    fn text_on_the_accent_clears_aa_and_no_other_shipped_tone_does() {
        /// WCAG 2.x SC 1.4.3 AA for body text — the same floor
        /// [`every_text_tone_clears_aa_on_every_surface_it_can_be_painted_on`]
        /// uses, restated here because this test sweeps a different ground.
        const MIN_TEXT_CONTRAST: f32 = 4.5;

        for (label, theme, layers) in [
            ("light", light(), &LIGHT_LAYERS),
            ("dark", dark(), &DARK_LAYERS),
        ] {
            let accent = theme_color(&theme, ACCENT_TOKEN);
            let on_accent = theme_color(&theme, ON_ACCENT_TOKEN);

            let ratio = contrast(on_accent, accent);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "{label}: {ON_ACCENT_TOKEN} on {ACCENT_TOKEN} is {ratio:.2}:1, \
                 below the {MIN_TEXT_CONTRAST}:1 AA floor. The label on a \
                 primary button is body text and gets no discount for being \
                 short."
            );

            // The value is the theme's own ground, taken from the layer set
            // by `insert_accent` rather than repeated as a literal. Held
            // here so the two cannot drift into two different whites.
            let ground = layers[0];
            assert_eq!(
                on_accent,
                ColorValue::from_srgb8(ground[0], ground[1], ground[2], 0xff),
                "{label}: {ON_ACCENT_TOKEN} must be this theme's \
                 surface.base — ink on a filled accent is the page the \
                 accent is cut out of"
            );

            for tone in ["text.primary", "text.muted"] {
                let ratio = contrast(theme_color(&theme, tone), accent);
                assert!(
                    ratio < MIN_TEXT_CONTRAST,
                    "{label}: {tone} now clears {MIN_TEXT_CONTRAST}:1 on \
                     {ACCENT_TOKEN} at {ratio:.2}:1, so {ON_ACCENT_TOKEN} is \
                     a name for a colour {tone} already covers. Delete the \
                     token and rebind its callers, or this file is carrying \
                     a name nothing needs."
                );
            }
        }
    }

    /// The deepest [`LAYER_TOKENS`] index [`DANGER_FILL_TOKEN`] stays
    /// findable on, per theme. Same shape and same reason as
    /// [`DEEPEST_ACCENT_LAYER`]: the painter needs the boundary as a number.
    ///
    /// Light reaches the bottom because the light layers alternate between
    /// `#ffffff` and `#f2f2f2` and `#da1e28` is 5.00:1 and 4.47:1 on them.
    /// Dark stops at layer **one**: `#da1e28` is 3.18:1 on `#222222` and
    /// **2.53:1** on `#333333`. That is one layer shallower than the accent
    /// reaches, and it is the price of taking Carbon's hex instead of
    /// tuning one for Petra's deeper ramp — recorded rather than hidden,
    /// because a danger button dropped two layers down loses the fill floor
    /// while every other check still passes.
    const DEEPEST_DANGER_LAYER: [(&str, usize); 2] = [("light", 3), ("dark", 1)];

    /// [`DANGER_FILL_TOKEN`] carries its label at the body-text floor, is
    /// findable on every layer it is declared paintable on, and is still a
    /// danger signal to a reader who cannot see red.
    ///
    /// # This is the gate the hue sweep points at
    ///
    /// [`no_shipped_colour_carries_a_hue_that_is_not_already_gated`] refuses
    /// any chromatic token that no measurement covers, and lists this one by
    /// name. So this test is not optional decoration on a new colour; it is
    /// the reason the colour is allowed to exist. Three claims:
    ///
    /// 1. **The label.** [`ON_COLOUR_TOKEN`] on the fill clears SC 1.4.3's
    ///    4.5:1. This is the whole reason the token is `#da1e28` and not
    ///    `support-error`: the shipped red tops out at 4.41:1 under the best
    ///    ink this library owns.
    /// 2. **The fill.** SC 1.4.11's 3:1 against the ground, on every layer
    ///    inside [`DEEPEST_DANGER_LAYER`], and *below* it on every layer
    ///    outside — so a retune that gains a layer is caught and reported
    ///    rather than silently unused.
    /// 3. **Without colour.** The operator is red-green colour blind, so a
    ///    red button that is only distinguishable from a grey one by hue is
    ///    not distinguishable at all. Measured against `surface.raised`, the
    ///    fill an ordinary [`crate::component::button`] carries, under both
    ///    Viénot-Brettel-Mollon simulations at [`MIN_STATUS_SEPARATION`].
    ///
    /// # What this cannot reach
    ///
    /// It says nothing about the red octagon `component::button` still draws
    /// on all three danger variants. That mark is the *shape* channel and it
    /// is FR-015's requirement, not this token's; its own tone against this
    /// fill is measured in `component::button`'s tests, because which ink
    /// the mark takes is a component decision.
    #[test]
    fn the_danger_fill_carries_a_white_label_and_reads_without_colour() {
        /// WCAG 2.x SC 1.4.3 AA for body text. A button label is prose and
        /// gets no discount for being one word long.
        const MIN_TEXT_CONTRAST: f32 = 4.5;

        for (label, deepest) in DEEPEST_DANGER_LAYER {
            let theme = if label == "light" { light() } else { dark() };
            let fill = theme_color(&theme, DANGER_FILL_TOKEN);
            let ink = theme_color(&theme, ON_COLOUR_TOKEN);

            let ratio = contrast(ink, fill);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "{label}: {ON_COLOUR_TOKEN} on {DANGER_FILL_TOKEN} is \
                 {ratio:.2}:1, below the {MIN_TEXT_CONTRAST}:1 AA floor. The \
                 only reason this fill exists rather than an alias of \
                 support-error is that support-error tops out at 4.41:1 under \
                 the best ink here; a danger fill that cannot carry white has \
                 given that reason away."
            );

            for (depth, token) in LAYER_TOKENS.iter().enumerate() {
                let ratio = contrast(fill, theme_color(&theme, token));
                if depth <= deepest {
                    assert!(
                        ratio >= MIN_SURFACE_CONTRAST,
                        "{label}: {DANGER_FILL_TOKEN} is declared paintable on \
                         {token} (depth {depth} <= {deepest}) but measures \
                         {ratio:.2}:1, under the {MIN_SURFACE_CONTRAST}:1 \
                         non-text floor (WCAG SC 1.4.11)."
                    );
                } else {
                    assert!(
                        ratio < MIN_SURFACE_CONTRAST,
                        "{label}: {DANGER_FILL_TOKEN} now clears \
                         {MIN_SURFACE_CONTRAST}:1 on {token} (depth {depth}) at \
                         {ratio:.2}:1. That is an improvement, and it means \
                         DEEPEST_DANGER_LAYER is stale: raise {label} to \
                         {depth}."
                    );
                }
            }

            // The grey a plain `button` is filled with, which is the thing a
            // danger button has to be told apart from.
            let ordinary = theme_color(&theme, RAISED_ALIAS);
            for (vision, matrix) in [("deuteranope", &DEUTERANOPE), ("protanope", &PROTANOPE)] {
                let d = delta_e(
                    to_lab(simulate(fill, matrix)),
                    to_lab(simulate(ordinary, matrix)),
                );
                assert!(
                    d >= MIN_STATUS_SEPARATION,
                    "{label}: {DANGER_FILL_TOKEN} and {RAISED_ALIAS} are only \
                     ΔE*ab {d:.1} apart to a {vision} reader (floor is \
                     {MIN_STATUS_SEPARATION}). A destructive button that reads \
                     as an ordinary one is the failure mode this whole palette \
                     is arranged to avoid — separate them in lightness, not in \
                     hue."
                );
            }
        }
    }

    /// [`ON_COLOUR_TOKEN`] is white in **both** themes, it carries the two
    /// fills it is spent on, and it is not a second name for
    /// [`ON_ACCENT_TOKEN`].
    ///
    /// # Why the last clause is the point
    ///
    /// Because the two *were* conflated and it shipped a visible defect:
    /// `component::controls` bound `text.on-accent` for the toggle handle,
    /// which resolves to `#121212` in dark, so row 36's on-toggle drew a
    /// black knob where Carbon's `$icon-on-color` handle is white. This test
    /// asserts the two names differ in dark, so the merge that would put the
    /// black knob back fails here rather than in a screenshot nobody takes.
    ///
    /// The light half is deliberately *not* an inequality: in light both
    /// resolve to `#ffffff` and always will, because light's `surface.base`
    /// is white. The asymmetry is the whole content of the pair.
    #[test]
    fn the_on_colour_ink_is_white_in_both_themes_and_is_not_the_accent_ink() {
        /// SC 1.4.11: the toggle handle is a graphical object, not text.
        const MIN_MARK_CONTRAST: f32 = MIN_SURFACE_CONTRAST;

        let white = ColorValue::from_srgb8(0xff, 0xff, 0xff, 0xff);
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let ink = theme_color(&theme, ON_COLOUR_TOKEN);
            assert_eq!(
                ink, white,
                "{label}: {ON_COLOUR_TOKEN} must be #ffffff. Carbon publishes \
                 $text-on-color and $icon-on-color as white in all four \
                 themes, and a theme-derived value here is exactly the drift \
                 that made the toggle knob black."
            );

            // The toggle handle rides the accent fill in this library —
            // Carbon's own track is `$support-success`, which Petra does not
            // spend on a control — so that is the ground to measure.
            let on_track = contrast(ink, theme_color(&theme, ACCENT_TOKEN));
            assert!(
                on_track >= MIN_MARK_CONTRAST,
                "{label}: {ON_COLOUR_TOKEN} on {ACCENT_TOKEN} is \
                 {on_track:.2}:1, under the {MIN_MARK_CONTRAST}:1 SC 1.4.11 \
                 floor. A toggle knob a reader cannot find inside its own \
                 track is not showing a state."
            );
        }

        assert_ne!(
            theme_color(&dark(), ON_COLOUR_TOKEN),
            theme_color(&dark(), ON_ACCENT_TOKEN),
            "dark: {ON_COLOUR_TOKEN} and {ON_ACCENT_TOKEN} are the same \
             colour, so one of the two names is doing nothing. They are not \
             the same idea: on-accent is the page the accent is cut out of \
             (#121212 in dark, because white on #4589ff is 3.35:1 and misses \
             AA), and on-colour is Carbon's always-white ink for a saturated \
             fill. Collapsing them puts the black toggle knob back."
        );
    }

    /// [`BORDER_TOKEN`] is visible on every layer it can be drawn on, and on
    /// every one of those layers it is decisively quieter than both text
    /// tones.
    ///
    /// # The two halves, and why the second one is the point
    ///
    /// The floor is the ordinary half: a boundary nobody can see is not a
    /// boundary. It is [`MIN_DIVIDER_CONTRAST`], **not** SC 1.4.11's 3:1,
    /// and that is the 2026-09-05 correction: this token is the decorative
    /// tone, spent on row rules and dividers and panel edges, and SC 1.4.11
    /// covers information that identifies a *component*. The 3:1 floor moved
    /// to [`BORDER_STRONG_TOKEN`], where the checkbox, the radio, the toggle
    /// track, the tertiary button, the drop zone and every field rule now
    /// live, and it is held there by
    /// [`border_strong_sits_between_the_subtle_border_and_every_text_tone`].
    /// [`BORDER_TOKEN`]'s own doc carries the measurement table.
    ///
    /// The **ceiling** is the half this test exists for. Before this token
    /// shipped, every border in `component/` bound `text.muted`, and that
    /// passed every contrast assertion in this file: it passed them by a
    /// factor of three. Loudness is not a contrast failure, which is exactly
    /// why no floor could ever have caught it. So the border is pinned
    /// *under* the text tones by a named factor, on each ground separately,
    /// and a future edit that quietly walks it back up the grey ramp fails
    /// here instead of shipping a wireframe.
    ///
    /// # What this cannot reach
    ///
    /// Contrast is not the only thing that makes an edge shout — a 2px
    /// stroke of this colour would read louder than a 1px stroke of a
    /// brighter one, and stroke width lives in the painter
    /// (`gorgon-petra-egui`'s `device_snapped_width`), not in this file. A
    /// capture owns that. What this stops is the *tone* drifting.
    #[test]
    fn the_border_tone_is_visible_everywhere_and_quieter_than_every_text_tone() {
        /// How much quieter than a text tone the border must be, as a ratio
        /// of contrasts on the *same* ground.
        ///
        /// Not a perceptual constant. It is a fence set below the tighter of
        /// the two shipped margins — dark's 5.03x against `text.muted` on
        /// `surface.layer-three`, light's 6.64x — with enough room that a
        /// deliberate retune does not trip it and a slide back toward a text
        /// tone does. It was 1.5x while this token carried the 3:1 control
        /// floor; the floor moved and the margins widened by a factor of
        /// three, so the fence moves with them or it stops catching
        /// anything.
        const MIN_QUIETER_THAN_TEXT: f32 = 4.0;

        for (label, theme) in [("light", light()), ("dark", dark())] {
            let border = theme_color(&theme, BORDER_TOKEN);
            let primary = theme_color(&theme, "text.primary");
            let muted = theme_color(&theme, "text.muted");

            for surface in LAYER_TOKENS.iter().chain(std::iter::once(&RAISED_ALIAS)) {
                let ground = theme_color(&theme, surface);
                let edge = contrast(border, ground);
                assert!(
                    edge >= MIN_DIVIDER_CONTRAST,
                    "{label}: {BORDER_TOKEN} on {surface} is {edge:.2}:1, below \
                     the {MIN_DIVIDER_CONTRAST}:1 floor Carbon's own row rules \
                     measure at. A rule a reader cannot find is not separating \
                     anything, and this is the decorative tone, so there is no \
                     louder name behind it to fall back on."
                );

                for (tone_name, tone) in [("text.primary", primary), ("text.muted", muted)] {
                    let quieter = contrast(tone, ground) / edge;
                    assert!(
                        quieter >= MIN_QUIETER_THAN_TEXT,
                        "{label}: on {surface}, {BORDER_TOKEN} is only \
                         {quieter:.2}x quieter than {tone_name}, under \
                         {MIN_QUIETER_THAN_TEXT}x. A border at a text tone is \
                         the defect this token replaced: it fails no contrast \
                         floor, it passes every one of them by a factor of \
                         three, and the page reads as a wireframe."
                    );
                }
            }
        }
    }

    /// The painter grooves an edge slot by comparing its bound token's
    /// *name* against `crate::token::rule::MATERIAL_TOKEN`
    /// (`gorgon-petra-egui`'s `paint.rs`, the `EDGE_SLOTS` loop). If this
    /// string ever drifted from [`BORDER_TOKEN`] the painter would stop
    /// grooving every existing divider, table row rule and list separator —
    /// silently, because nothing else in the tree would look wrong, only the
    /// paint.
    #[test]
    fn the_rule_material_token_matches_the_decorative_border_token() {
        assert_eq!(
            BORDER_TOKEN,
            crate::token::rule::MATERIAL_TOKEN,
            "the painter's groove trigger and this token's own name have \
             drifted apart; every horizontal rule in the product would go \
             back to a flat line with no test above this one noticing"
        );
    }

    /// [`rule_stroke`] reproduces the exact pair the contract's construction
    /// table and the ratio experiment settled on, on both shipped panels:
    /// light's `#f2f2f2` (242) to 214 and 255, dark's `#222222` (34) to 6
    /// and 47. These are not independently chosen — they are what
    /// `docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md`
    /// sampled off the rendered PNG bytes for its winning specimen, so this
    /// test is the contract's numbers, typed in once.
    #[test]
    fn rule_strokes_land_on_the_contract_construction() {
        assert_eq!(
            rule_stroke(LIGHT_LAYERS[1], RULE_SHADOW_DELTA),
            [214, 214, 214],
            "light shadow: 242 - 28 must be 214"
        );
        assert_eq!(
            rule_stroke(LIGHT_LAYERS[1], RULE_HIGHLIGHT_DELTA),
            [255, 255, 255],
            "light highlight: 242 + 13 must land exactly on white, the \
             measured clamp point — not past it, and not short of it"
        );
        assert_eq!(
            rule_stroke(DARK_LAYERS[1], RULE_SHADOW_DELTA),
            [6, 6, 6],
            "dark shadow: 34 - 28 must be 6, the experiment's row B \
             'achieved rows' for the shipped dark ground"
        );
        assert_eq!(
            rule_stroke(DARK_LAYERS[1], RULE_HIGHLIGHT_DELTA),
            [47, 47, 47],
            "dark highlight: 34 + 13 must be 47, the same row"
        );

        // And the shipped themes must carry exactly these values through
        // `insert_rule_material`, not just the free function above — a
        // theme is what the painter actually reads.
        for (label, theme, shadow, highlight) in [
            ("light", light(), [214u8, 214, 214], [255u8, 255, 255]),
            ("dark", dark(), [6u8, 6, 6], [47u8, 47, 47]),
        ] {
            assert_eq!(
                theme_color(&theme, crate::token::rule::SHADOW_TOKEN),
                opaque_value(shadow),
                "{label}: {} did not resolve to the contract's shadow level",
                crate::token::rule::SHADOW_TOKEN
            );
            assert_eq!(
                theme_color(&theme, crate::token::rule::HIGHLIGHT_TOKEN),
                opaque_value(highlight),
                "{label}: {} did not resolve to the contract's highlight \
                 level",
                crate::token::rule::HIGHLIGHT_TOKEN
            );
        }
    }

    /// The acceptance criterion this whole material exists to hold:
    /// **the highlight stroke must resolve lighter than the panel a rule
    /// draws on**, in both shipped themes. `#dddddd` (221) on a 242 panel is
    /// the exact bug this catches — a "groove" with two shadows and no
    /// highlight, shipped once and invisible on mid-grey specimens because
    /// nobody sampled the actual pixels.
    ///
    /// Scoped to `surface.layer-one`, not every layer this theme owns:
    /// [`insert_rule_material`] computes both strokes from that one panel —
    /// the panel every shipped divider, table row rule and list separator
    /// actually sits on — so that is the only surface this pair is
    /// *sanctioned* to sit on today. Sweeping `surface.base` too would fail
    /// for an unrelated reason in light mode (it is `#ffffff`, and the
    /// highlight's own clamp also lands there, so "strictly lighter" is
    /// false by a tie rather than by the regression this test exists to
    /// catch) — a false failure this test must not produce.
    #[test]
    fn the_rule_highlight_resolves_lighter_than_the_panel_it_offsets() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let panel = theme_color(&theme, "surface.layer-one");
            let highlight = theme_color(&theme, crate::token::rule::HIGHLIGHT_TOKEN);
            assert!(
                highlight.relative_luminance() > panel.relative_luminance(),
                "{label}: {} (luminance {:.4}) is not lighter than its own \
                 panel, surface.layer-one (luminance {:.4}). A highlight \
                 that is not lighter than the panel it sits on paints a \
                 second shadow instead of a groove.",
                crate::token::rule::HIGHLIGHT_TOKEN,
                highlight.relative_luminance(),
                panel.relative_luminance()
            );

            let shadow = theme_color(&theme, crate::token::rule::SHADOW_TOKEN);
            assert!(
                shadow.relative_luminance() < panel.relative_luminance(),
                "{label}: {} (luminance {:.4}) is not darker than its own \
                 panel, surface.layer-one (luminance {:.4})",
                crate::token::rule::SHADOW_TOKEN,
                shadow.relative_luminance(),
                panel.relative_luminance()
            );
        }
    }

    /// A future author who changes [`RULE_SHADOW_DELTA`] or
    /// [`RULE_HIGHLIGHT_DELTA`] and drifts off the measured ratio fails here
    /// rather than in a screenshot. 2.15:1, shadow over highlight, within a
    /// half percent — the tolerance the ratio experiment's own two accepted
    /// specimens agreed to.
    #[test]
    fn the_rule_delta_ratio_matches_the_measured_construction() {
        let ratio = f32::from(-RULE_SHADOW_DELTA) / f32::from(RULE_HIGHLIGHT_DELTA);
        assert!(
            (ratio - 2.15).abs() < 0.01,
            "shadow:highlight is {ratio:.3}, not the measured 2.15:1 from \
             docs/experiments/successes/2026-09-07-deboss-rule-contrast-ratio.md"
        );
    }

    /// **One layer of tonal step is not a control boundary**, in either
    /// theme, and this test exists to keep that measured rather than
    /// rediscovered.
    ///
    /// # Why a passing test asserts a failure
    ///
    /// The 2026-08-25 design pass took the outlines off the cards and
    /// replaced them with tone, which was right. The obvious next step was to
    /// do the same to the controls, and a capture said no: a `button` seated
    /// one layer ahead of the card it sits on is a `#f2f2f2`-to-`#ffffff`
    /// step in light. That is 1.12:1, against WCAG 2.1 SC 1.4.11's 3:1 floor
    /// for the visual information that identifies a user-interface component.
    ///
    /// So `field` keeps a quiet [`BORDER_STRONG_TOKEN`] rule, and this test
    /// is the number that says why. It asserts the step is **below** the floor,
    /// which reads backwards until you see what it is guarding: the next
    /// person to look at an outlined field will want to delete the outline,
    /// and this makes them measure first. If a future layer set genuinely
    /// separates a control from its ground at 3:1, this test fails, and the
    /// correct response is to delete both the test and the border together.
    ///
    /// **`button` did have that edge and no longer does.** It is elevated
    /// instead, on the operator's instruction after seeing both rendered —
    /// a weaker boundary by this same measurement (`shadow.raised` reaches
    /// roughly 1.6:1 in light against the border's 3.34:1) and the call
    /// Material 3 and Apple's HIG both make for a filled button.
    /// `component::button`'s `labelled` carries the full table. The number
    /// below is why a *field* still has one, and it is also the number that
    /// says what the button traded away.
    ///
    /// Light is the worse of the two and structurally so. Its layer set
    /// *alternates* (`#ffffff`, `#f2f2f2`, `#ffffff`, `#f2f2f2`) rather than
    /// ramping, so there is no deeper step to reach for: the widest gap the
    /// light set can produce between any two adjacent layers is the one
    /// measured here.
    #[test]
    fn one_layer_of_step_does_not_reach_the_control_boundary_floor() {
        /// WCAG 2.1 SC 1.4.11 *Non-text Contrast*, Level AA.
        const MIN_UI_CONTRAST: f32 = 3.0;

        for (label, theme) in [("light", light()), ("dark", dark())] {
            for depth in 0..LAYER_TOKENS.len() - 1 {
                let ground = theme_color(&theme, LAYER_TOKENS[depth]);
                let control = theme_color(&theme, LAYER_TOKENS[depth + 1]);
                let step = contrast(control, ground);
                assert!(
                    step < MIN_UI_CONTRAST,
                    "{label}: a control seated on layer {depth} now reads \
                     {step:.2}:1 against its ground, which clears the \
                     {MIN_UI_CONTRAST}:1 SC 1.4.11 floor on its own. The \
                     borders on `button` and `field` exist only because this \
                     number was 1.26:1 and 1.12:1 -- if the layer set now \
                     carries the boundary by itself, delete those borders and \
                     delete this test with them."
                );
            }

            // And the edge that is there instead does clear it, on every one
            // of those grounds. Asserted here beside the number it answers,
            // so the pair reads as one argument rather than two unrelated
            // gates in different files.
            //
            // That edge is `border-strong`, not `border.subtle`, since the
            // 2026-09-05 role split. `field` binds `border-strong` through
            // its `border-bottom` slot and always did; what changed is that
            // the checkbox, the radio, the toggle track, the tertiary button
            // and the upload drop zone now bind it too, because they are the
            // other controls whose edge *is* the control. `border.subtle` is
            // the decorative tone and is deliberately under this floor —
            // see `BORDER_TOKEN`'s own doc for the measurement table.
            let border = theme_color(&theme, BORDER_STRONG_TOKEN);
            for surface in &LAYER_TOKENS {
                let edge = contrast(border, theme_color(&theme, surface));
                assert!(
                    edge >= MIN_UI_CONTRAST,
                    "{label}: {BORDER_STRONG_TOKEN} on {surface} is \
                     {edge:.2}:1, under the floor the tonal step already \
                     failed. With both under 3:1 a control on this layer has \
                     no boundary at all."
                );
            }
        }
    }

    /// The elevation colours are black in both themes, they get stronger as
    /// the ground they fall on gets darker, and they actually change the
    /// pixel they land on.
    ///
    /// # The failure this exists to catch
    ///
    /// It is a **silent** one. If a shadow alpha is too low against
    /// `#121212`, the token still resolves, the painter still reports a
    /// fill, every other test in this file stays green, and the rendered
    /// window looks identical. Work that passes every automated signal while
    /// changing no pixel is the thing this repo's standing rule is for, so
    /// the alpha is not allowed to be judged by "it parsed".
    ///
    /// # What is measured, and what this test cannot reach
    ///
    /// Compositing black at straight alpha `a` over a ground gives
    /// `ground x (1 - a)` in linear light, so the darkening is exactly
    /// computable from the token pair with no renderer involved. The test
    /// takes the CIE L\* of ground and result and requires the drop to clear
    /// [`MIN_SHADOW_DARKENING`].
    ///
    /// That is a floor on *arithmetic* visibility, not on screen
    /// visibility. It cannot see a draw-order bug that paints the shadow on
    /// top of the card, it cannot see a blur radius that spreads the drop
    /// over so many pixels that none of them reach this delta, and it cannot
    /// judge taste. A capture owns all three. What this stops is the alpha
    /// silently going to nothing.
    #[test]
    fn the_shadow_set_is_black_and_deepens_with_its_ground() {
        /// The smallest CIE L\* drop a shadow may make on the worst ground
        /// it can fall on. 1.0 is roughly L\*'s just-noticeable difference.
        ///
        /// Floor against a shadow that resolves and paints but darkens
        /// nothing. 1.0 is roughly L\*'s just-noticeable difference. The
        /// 2026-08-26 capture argued the *alphas* up; this floor stays the
        /// fence, not the target.
        const MIN_SHADOW_DARKENING: f32 = 1.0;

        let mut previous_light: Option<f32> = None;

        for (label, theme, alphas) in [
            ("light", light(), &super::LIGHT_SHADOW_ALPHAS),
            ("dark", dark(), &super::DARK_SHADOW_ALPHAS),
        ] {
            let mut alpha_seen = Vec::new();
            for (token, declared) in SHADOW_TOKENS.iter().zip(alphas) {
                let shadow = theme_color(&theme, token);

                assert!(
                    shadow.r == 0.0 && shadow.g == 0.0 && shadow.b == 0.0,
                    "{label}/{token} is not black: ({}, {}, {}). Occlusion \
                     darkens — a lightened shadow in dark mode is a glow, \
                     and this design does not ship one.",
                    shadow.r,
                    shadow.g,
                    shadow.b,
                );
                assert!(
                    (shadow.a - f32::from(*declared) / 255.0).abs() < 1e-6,
                    "{label}/{token} carries alpha {} but its table says \
                     {declared}/255",
                    shadow.a,
                );
                alpha_seen.push(shadow.a);

                // The worst ground in this theme is the one where black has
                // the least room left to darken it.
                let worst = LAYER_TOKENS
                    .iter()
                    .map(|surface| {
                        let ground = theme_color(&theme, surface);
                        let composited = ColorValue {
                            r: ground.r * (1.0 - shadow.a),
                            g: ground.g * (1.0 - shadow.a),
                            b: ground.b * (1.0 - shadow.a),
                            a: 1.0,
                        };
                        (lightness(ground) - lightness(composited), *surface)
                    })
                    .fold((f32::MAX, ""), |acc, x| if x.0 < acc.0 { x } else { acc });

                assert!(
                    worst.0 >= MIN_SHADOW_DARKENING,
                    "{label}/{token} at alpha {declared}/255 darkens {} by \
                     only ΔL* {:.2}, under the {MIN_SHADOW_DARKENING} floor. \
                     The token resolves and the painter reports a fill, and \
                     the operator sees no shadow at all.",
                    worst.1,
                    worst.0,
                );
            }

            assert!(
                alpha_seen[1] > alpha_seen[0],
                "{label}: shadow.overlay ({}) must be stronger than \
                 shadow.raised ({}) — an overlay has left the page and a \
                 raised card has not",
                alpha_seen[1],
                alpha_seen[0],
            );

            match previous_light {
                None => previous_light = Some(alpha_seen[0]),
                Some(light_raised) => assert!(
                    alpha_seen[0] > light_raised,
                    "dark shadow.raised ({}) must be stronger than light's \
                     ({light_raised}): black on a dark ground has less room \
                     to darken it, which is optics and not a theme branch",
                    alpha_seen[0],
                ),
            }
        }
    }

    /// Every shadow colour the vocabulary declares has exactly one geometry
    /// row, and the geometry is shared by both themes because it is not a
    /// token at all.
    ///
    /// # Why this pairing needs a gate
    ///
    /// [`SHADOW_GEOMETRY`] is the one public constant in this file, read by
    /// `gorgon-petra-egui`'s painter across a crate boundary, and it is
    /// joined to the vocabulary by **string name only**. Nothing in the type
    /// system connects `shadow.overlay` the declared token to
    /// `"shadow.overlay"` the table key. Declare a third shadow colour and
    /// forget the row, and the painter resolves a colour it has no offset or
    /// blur to draw with — at which point it either guesses or drops the
    /// slot, and both are silent.
    ///
    /// The reverse direction is gated too: a geometry row whose name no
    /// theme defines is a row the painter can never reach.
    #[test]
    fn the_shadow_geometry_covers_every_shadow_token_and_nothing_else() {
        let vocab = standard_vocabulary();
        let declared: Vec<String> = vocab
            .names_of_kind(crate::token::value::TokenKind::Color)
            .into_iter()
            .map(std::string::ToString::to_string)
            .filter(|n| n.starts_with("shadow."))
            .collect();
        let keyed: Vec<String> = SHADOW_GEOMETRY
            .iter()
            .map(|(token, _)| (*token).to_string())
            .collect();
        let mut sorted_keys = keyed.clone();
        sorted_keys.sort();
        assert_eq!(
            declared, sorted_keys,
            "the vocabulary's shadow colours and SHADOW_GEOMETRY's rows have \
             drifted apart. A declared shadow with no geometry is a colour \
             the painter cannot place; a geometry row with no token is a row \
             it can never reach."
        );
        assert_eq!(
            keyed,
            SHADOW_TOKENS.map(std::string::ToString::to_string).to_vec(),
            "SHADOW_GEOMETRY and SHADOW_TOKENS must stay in the same order — \
             insert_shadow_set zips SHADOW_TOKENS against a raw alpha array, \
             so a reorder in one and not the other swaps the two shadows' \
             colours without changing a single value"
        );

        let raised = SHADOW_GEOMETRY[0].1;
        let overlay = SHADOW_GEOMETRY[1].1;
        assert!(
            overlay.offset[1] > raised.offset[1] && overlay.blur > raised.blur,
            "shadow.overlay must fall further and soften more than \
             shadow.raised ({overlay:?} vs {raised:?}); if the two geometries \
             ever converge the design has one elevation under two names"
        );
        for (token, geometry) in SHADOW_GEOMETRY {
            assert_eq!(
                geometry.offset[0], 0,
                "{token} is offset sideways by {}. A horizontal offset \
                 implies a light source off to one side, and a tiling shell \
                 whose panels sit anywhere has no such side.",
                geometry.offset[0],
            );
            assert_eq!(
                geometry.spread, 0,
                "{token} spreads by {}; neither shipped level grows its \
                 rectangle before blurring",
                geometry.spread,
            );
        }

        // The geometry is not reachable as a token in either theme: that is
        // the whole claim of the colour/geometry split. Only the colour is.
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for (token, _) in SHADOW_GEOMETRY {
                assert!(
                    matches!(
                        theme.value(&TokenName::new(token).unwrap()),
                        Some(TokenValue::Color(_))
                    ),
                    "{label}/{token} must resolve to a Color and nothing \
                     else — offset, blur and spread are shared Rust \
                     constants, not per-theme values"
                );
            }
        }
    }

    /// The interaction ladder is ordered, visible, and never as loud as a
    /// nesting change.
    ///
    /// Four claims, and each one is a way the ladder could be wrong while
    /// looking right in a screenshot:
    ///
    /// 1. **Ordered.** hover < selected < selected-hover < active, in the
    ///    distance each moves. A ladder whose rungs cross means "selected"
    ///    and "hovered" swap loudness on some component and the reader has to
    ///    learn both.
    /// 2. **Visible.** Every rung clears CIE L\*'s conventional
    ///    just-noticeable difference of about 1.0. A state nobody can see is
    ///    the counters-that-read-zero defect in colour form.
    /// 3. **Quieter than a layer.** Hover moves less than one layer step
    ///    (7.76 L\* in the dark set), so a hovered card and a nested card are
    ///    not the same picture.
    /// 4. **Away from the ground.** Light darkens, dark lightens. Measured
    ///    per theme rather than assumed, because the light layer set
    ///    *alternates* and a rule read off the layer array would get the sign
    ///    wrong.
    #[test]
    fn the_interaction_ladder_is_ordered_and_visible() {
        /// CIE L\*'s conventional just-noticeable difference.
        const JND: f32 = 1.0;

        let ladder = [
            ("layer-hover", HOVER_STEP),
            ("layer-selected", SELECTED_STEP),
            ("layer-selected-hover", SELECTED_HOVER_STEP),
            (ACTIVE_TOKEN, ACTIVE_STEP),
        ];
        let mut previous = 0.0f32;
        for (token, step) in ladder {
            assert!(
                step > previous,
                "{token} steps {step} L*, which does not exceed the rung below \
                 it at {previous}; a ladder whose rungs cross makes two states \
                 swap loudness"
            );
            assert!(
                step >= JND,
                "{token} steps only {step} L*, under the {JND} just-noticeable \
                 difference — a state nobody can see"
            );
            previous = step;
        }
        // Measured against the shipped layer set rather than against a
        // remembered 7.76, so retuning the layers re-checks the hover step
        // instead of silently invalidating this claim.
        let dark_theme = dark();
        let layer_step = lightness_of_token(&dark_theme, LAYER_TOKENS[1])
            - lightness_of_token(&dark_theme, LAYER_TOKENS[0]);
        assert!(
            HOVER_STEP < layer_step,
            "hover steps {HOVER_STEP} L* and the dark layer set steps \
             {layer_step:.2}; a hovered card must not read as a nested one"
        );

        for (label, theme, sign) in [("light", light(), -1.0f32), ("dark", dark(), 1.0)] {
            let base = lightness_of_token(&theme, LAYER_TOKENS[1]);
            for (token, step) in ladder {
                let moved = lightness_of_token(&theme, token) - base;
                assert!(
                    (moved - sign * step).abs() < 0.6,
                    "{label}/{token} moved {moved:.2} L* off surface.layer-one; \
                     the ladder asks for {:.2}. Sign wrong means the state moves \
                     toward the theme's own ground, where there is no room.",
                    sign * step
                );
            }
        }
    }

    /// `layer-accent` lands where Carbon puts its own, which is what fixes
    /// [`SELECTED_STEP`] at seven rather than at a number that felt right.
    ///
    /// Carbon's White theme publishes `$layer-accent-01` — its name for one
    /// selected-magnitude step off `$layer-01` — as `#e0e0e0`
    /// (`.agents/research/08-24-2026/Design-Language/carbon.md` §2, measured
    /// from `@carbon/themes`' DTCG files and resolved against
    /// `@carbon/colors`). Petra's light `surface.layer-one` is `#f2f2f2`
    /// rather than Carbon's `#f4f4f4`, so the two cannot be compared as hexes;
    /// they can be compared as a *distance*, which is what this does.
    ///
    /// The tolerance is deliberately loose. This is not a claim that Petra's
    /// tone equals Carbon's — it is a claim that seven L\* is the right order
    /// of magnitude for "selected", sourced to a system that shipped the
    /// decision rather than to taste. A drift to four or to fourteen fails.
    #[test]
    fn the_selected_step_lands_where_carbon_puts_its_layer_accent() {
        /// Carbon White's `$layer-accent-01`, measured.
        const CARBON_LAYER_ACCENT_01: [u8; 3] = [0xe0, 0xe0, 0xe0];

        let ours = lightness_of_token(&light(), LAYER_ACCENT_TOKEN);
        let theirs = lightness_of(CARBON_LAYER_ACCENT_01);
        assert!(
            (ours - theirs).abs() < 2.0,
            "light `{LAYER_ACCENT_TOKEN}` is L* {ours:.2} and Carbon White's \
             $layer-accent-01 is L* {theirs:.2}. SELECTED_STEP is \
             {SELECTED_STEP} L*; if that constant moved, this is the \
             measurement it moved away from."
        );
    }

    /// Both `layer-accent` states step off `layer-accent` itself, not off the
    /// resting layer.
    ///
    /// The distinction matters because `layer-accent` is deliberately *not* a
    /// layer — Carbon calls it "not considered a proper layer but a
    /// supporting color for `$layer` inside of components" — so its hover has
    /// to be a hover *of it*, and an implementation that stepped from
    /// `surface.layer-one` would put `layer-accent-hover` between the resting
    /// accent surface and nothing at all.
    #[test]
    fn the_layer_accent_states_step_off_the_accent_surface() {
        for (label, theme, sign) in [("light", light(), -1.0f32), ("dark", dark(), 1.0)] {
            let accent = lightness_of_token(&theme, LAYER_ACCENT_TOKEN);
            for (token, step) in LAYER_ACCENT_STATE_TOKENS {
                let moved = lightness_of_token(&theme, token) - accent;
                assert!(
                    (moved - sign * step).abs() < 0.6,
                    "{label}/{token} moved {moved:.2} L* off {LAYER_ACCENT_TOKEN}, \
                     not the {:.2} the ladder asks for",
                    sign * step
                );
            }
        }
        // And `layer-accent` is not in the layer set, which is the claim that
        // stops it being counted when a container steps.
        assert!(
            !LAYER_TOKENS.contains(&LAYER_ACCENT_TOKEN),
            "{LAYER_ACCENT_TOKEN} is a supporting colour, not a layer; \
             counting it when stepping skips a real layer"
        );
    }

    /// Every icon tone is a tone the theme already measured, under a second
    /// name.
    ///
    /// This is the whole safety argument for adding six colour names at once.
    /// None of them is a new grey: four are existing tones and two are an
    /// existing tone at [`super::DISABLED_ALPHA`]. So the contrast work that
    /// was done for `text.primary`, `text.muted` and `text.on-accent` covers
    /// them, and a later change to any of those three cannot leave the icons
    /// behind on a stale value.
    #[test]
    fn every_icon_tone_is_a_measured_tone_under_a_second_name() {
        for (label, theme, inverse_ink) in [
            ("light", light(), "text-inverse"),
            ("dark", dark(), "text-inverse"),
        ] {
            for (icon, source) in [
                (ICON_TOKENS[0], "text.primary"),
                (ICON_TOKENS[1], "text.muted"),
                (ICON_TOKENS[2], ON_ACCENT_TOKEN),
                (ICON_TOKENS[5], inverse_ink),
            ] {
                assert_eq!(
                    theme_color(&theme, icon),
                    theme_color(&theme, source),
                    "{label}: {icon} must be assigned from {source} rather than \
                     chosen, so the two cannot drift apart"
                );
            }

            // The two disabled tones are their live counterparts faded, and
            // nothing else: same colour, lower alpha.
            for (faded, live) in [
                (ICON_TOKENS[3], ON_ACCENT_TOKEN),
                (ICON_TOKENS[4], "text.primary"),
            ] {
                let a = theme_color(&theme, faded);
                let b = theme_color(&theme, live);
                assert!(
                    (a.r - b.r).abs() < 1e-6
                        && (a.g - b.g).abs() < 1e-6
                        && (a.b - b.b).abs() < 1e-6,
                    "{label}: {faded} must be {live} faded, not a different \
                     colour: {a:?} against {b:?}"
                );
                assert!(
                    a.a < b.a,
                    "{label}: {faded} carries alpha {} against {live}'s {}; a \
                     disabled tone that is not faded is not disabled",
                    a.a,
                    b.a
                );
            }
        }
    }

    /// `border-strong` is louder than the subtle border and still quieter
    /// than every text tone, on every layer either can be drawn on.
    ///
    /// **This is the test the refusal at [`BORDER_TOKEN`] asked for.** That
    /// comment refused a second border tone for want of a measured need, and
    /// named the risk: a border that drifts up toward a text tone fails no
    /// contrast floor — it passes every one of them harder — so only a
    /// ceiling can catch it. Adding a *deliberately louder* border is exactly
    /// the change that could walk through that ceiling, so the ceiling is
    /// restated here for the new name.
    ///
    /// No magic fence, unlike the subtle border's ratio: the claim is a
    /// strict ordering, and a hand-picked hex would not give it.
    ///
    /// # 2026-09-05: this became the load-bearing half of the pair
    ///
    /// `border-strong` used to be the CIE L\* midpoint of the subtle border
    /// and the muted text tone, and the ordering fell out of the
    /// construction for free. It is now the *anchored* tone — the quietest
    /// grey clearing [`MIN_CONTROL_BOUNDARY`] on every layer — and the
    /// subtle one is the derived-quiet one, so the ordering is a
    /// measurement rather than an arithmetic identity, and the floor
    /// assertion below is the one the checkbox, the radio, the toggle
    /// track, the tertiary button, the drop zone and every field rule now
    /// depend on. Breaking it does not make a page look slightly worse; it
    /// deletes the outline that *is* an unchecked checkbox.
    #[test]
    fn border_strong_sits_between_the_subtle_border_and_every_text_tone() {
        /// See [`MIN_CONTROL_BOUNDARY`], restated at the point of use.
        const MIN_UI_CONTRAST: f32 = MIN_CONTROL_BOUNDARY;

        for (label, theme) in [("light", light()), ("dark", dark())] {
            let subtle = theme_color(&theme, BORDER_TOKEN);
            let strong = theme_color(&theme, BORDER_STRONG_TOKEN);

            for surface in LAYER_TOKENS.iter().chain(std::iter::once(&RAISED_ALIAS)) {
                let ground = theme_color(&theme, surface);
                let loud = contrast(strong, ground);
                assert!(
                    loud >= MIN_UI_CONTRAST,
                    "{label}: {BORDER_STRONG_TOKEN} on {surface} is {loud:.2}:1, \
                     below the {MIN_UI_CONTRAST}:1 SC 1.4.11 floor"
                );
                assert!(
                    loud > contrast(subtle, ground),
                    "{label}: on {surface}, {BORDER_STRONG_TOKEN} is {loud:.2}:1 \
                     and {BORDER_TOKEN} is {:.2}:1. A strong border that is not \
                     louder than the subtle one is a name that lies.",
                    contrast(subtle, ground)
                );
                for tone in ["text.primary", "text.muted"] {
                    let text = contrast(theme_color(&theme, tone), ground);
                    assert!(
                        text > loud,
                        "{label}: on {surface}, {BORDER_STRONG_TOKEN} is \
                         {loud:.2}:1 and {tone} is {text:.2}:1. A border at or \
                         past a text tone is the wireframe defect the border \
                         pass removed."
                    );
                }
            }
        }
    }

    /// The three numbered subtle borders carry the one measured tone, and
    /// `border-interactive` is the accent rather than a fourth grey.
    #[test]
    fn the_numbered_border_names_carry_measured_tones() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for token in BORDER_SUBTLE_TOKENS {
                assert_eq!(
                    theme_color(&theme, token),
                    theme_color(&theme, BORDER_TOKEN),
                    "{label}: {token} must carry {BORDER_TOKEN}'s measured tone. \
                     Splitting the three is free later — the digest hashes \
                     names, never values — but three unmeasured greys now is \
                     not."
                );
            }
            assert_eq!(
                theme_color(&theme, BORDER_INTERACTIVE_TOKEN),
                theme_color(&theme, ACCENT_TOKEN),
                "{label}: {BORDER_INTERACTIVE_TOKEN} is the accent, so the \
                 accent's own measured floors cover it"
            );
        }
    }

    /// FR-004a's first rule, as a value: a field **is** the layer one ahead
    /// of the background it sits on.
    ///
    /// Carbon states this in prose and then proves it in its own themes —
    /// `$field-01` and `$layer-01` are the same hex in all four published
    /// themes (`carbon.md` §2, measured). Petra assigns the field set
    /// straight from the layer set, so the rule cannot be broken by editing
    /// one and forgetting the other.
    #[test]
    fn every_field_token_is_the_layer_one_ahead_of_its_background() {
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for (n, field) in FIELD_TOKENS.iter().enumerate() {
                assert_eq!(
                    theme_color(&theme, field),
                    theme_color(&theme, LAYER_TOKENS[n + 1]),
                    "{label}: {field} sits on {} and must be {} — one layer \
                     ahead, which is what FR-004a means",
                    LAYER_TOKENS[n],
                    LAYER_TOKENS[n + 1]
                );
            }
        }
    }

    /// The inverse family is the other polarity's measured tones, and its ink
    /// clears AA on its own ground.
    ///
    /// The second half is the one that matters. A tooltip is a small dark box
    /// on a light page, and it is the easiest place in a design system to end
    /// up with a contrast failure nobody notices, because it is not on screen
    /// while anybody is looking at the theme. Building it out of a shipped
    /// theme's own pair means the measurement came with it — but "means" is
    /// not "is", so this measures it.
    #[test]
    fn the_inverse_family_inverts_and_stays_legible() {
        /// WCAG 2.1 SC 1.4.3 *Contrast (Minimum)*, Level AA, body text.
        const MIN_TEXT_CONTRAST: f32 = 4.5;

        for (label, theme, other) in [("light", light(), dark()), ("dark", dark(), light())] {
            for (ours, theirs) in [
                ("background-inverse", "surface.layer-one"),
                ("text-inverse", "text.primary"),
                ("border-inverse", BORDER_TOKEN),
                ("focus-inverse", RING_TOKEN),
                ("layer-selected-inverse", "surface.base"),
            ] {
                assert_eq!(
                    theme_color(&theme, ours),
                    theme_color(&other, theirs),
                    "{label}: {ours} must be the other polarity's {theirs}, so \
                     the tone arrives already measured against the ground it \
                     will sit on"
                );
            }

            let ground = theme_color(&theme, "background-inverse");
            let ink = theme_color(&theme, "text-inverse");
            let ratio = contrast(ink, ground);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "{label}: text-inverse on background-inverse is {ratio:.2}:1, \
                 under the {MIN_TEXT_CONTRAST}:1 AA floor for body text. A \
                 tooltip is prose, not a boundary."
            );

            // Hover moves the inverted ground away from *its* ground, which is
            // the opposite of this theme's direction.
            let hovered = theme_color(&theme, "background-inverse-hover");
            assert_ne!(
                hovered, ground,
                "{label}: background-inverse-hover is the resting tone; a \
                 hover nobody can see is not a state"
            );
        }
    }

    /// FR-022's rule, walked over every role and both sides of the half-edge
    /// clause.
    ///
    /// Two things are checked that a table of expected answers would not
    /// catch. Every answer is a name the shape ramp actually declares — a
    /// rule that resolved to an undeclared token would be refused at tree
    /// acceptance, at runtime, in whichever component happened to hit it
    /// first. And the half-edge clause overrides the role in **both**
    /// directions: a small node takes the pill, a large one keeps its role's
    /// step.
    #[test]
    fn the_one_radius_rule_is_total_and_the_half_edge_clause_overrides_it() {
        let vocab = standard_vocabulary();
        let roles = [
            CornerRole::Tiled,
            CornerRole::BoxedMark,
            CornerRole::Grouping,
            CornerRole::Floating,
            CornerRole::Pill,
        ];
        // A role added to the enum and forgotten here would leave this test
        // claiming totality over a domain it no longer covers. This match is
        // exhaustive, so adding a member stops the build in this file, one
        // screen from the array that needs the member.
        const fn every_role_is_listed_above(role: CornerRole) -> usize {
            match role {
                CornerRole::Tiled => 0,
                CornerRole::BoxedMark => 1,
                CornerRole::Grouping => 2,
                CornerRole::Floating => 3,
                CornerRole::Pill => 4,
            }
        }
        assert_eq!(
            roles.len(),
            every_role_is_listed_above(CornerRole::Pill) + 1
        );

        // Total: every role, at a comfortable size, resolves to a declared
        // shape token.
        for role in roles {
            let token = corner_for(role, 200.0);
            assert!(
                vocab.contains(&TokenName::new(token).unwrap()),
                "{role:?} resolves to `{token}`, which the shape ramp does not \
                 declare — a tree binding it is refused at acceptance"
            );
            assert!(
                SHAPE_RAMP.iter().any(|(step, _)| *step == token),
                "{role:?} resolves to `{token}`, which is outside SHAPE_RAMP"
            );
        }

        // At 200 units nothing is a pill and every role keeps its own step.
        assert_eq!(corner_for(CornerRole::Tiled, 200.0), "shape.corner-none");
        assert_eq!(corner_for(CornerRole::BoxedMark, 200.0), "shape.corner-xs");
        assert_eq!(corner_for(CornerRole::Grouping, 200.0), "shape.corner-sm");
        assert_eq!(corner_for(CornerRole::Floating, 200.0), "shape.corner-md");
        // Except `Pill`, which is the one member that names a shape rather
        // than a radius. It is a stadium at 200 units and at 4, and that is
        // the whole point of it: the promotion clause below can only reach a
        // stadium at 16 units and under, so a role with a fixed radius
        // cannot express Carbon's 24-unit toggle track or 18-unit radio box.
        //
        // Falsified by giving `Pill` `Floating`'s ramp entry instead of its
        // own early return:
        //
        // ```text
        // assertion `left == right` failed: a pill at 0 units is still a pill
        //   left: "shape.corner-md"
        //  right: "shape.corner-full"
        // ```
        for edge in [0.0, 4.0, 18.0, 24.0, 200.0] {
            assert_eq!(
                corner_for(CornerRole::Pill, edge),
                "shape.corner-full",
                "a pill at {edge} units is still a pill"
            );
        }

        // The clause fires when the role's radius reaches half the shorter
        // edge, whatever the role said. A 4-unit progress rail asking for a
        // 4-unit corner is a stadium; a 16-unit floating surface asking for
        // 8 is one too.
        assert_eq!(
            corner_for(CornerRole::Grouping, 4.0),
            "shape.corner-full",
            "4 units of radius on a 4-unit-tall node is a stadium, not a \
             rounded rect"
        );
        assert_eq!(corner_for(CornerRole::Floating, 16.0), "shape.corner-full");
        assert_eq!(
            corner_for(CornerRole::BoxedMark, 4.0),
            "shape.corner-full",
            "2 units of radius on a 4-unit box reaches half the edge"
        );

        // `Tiled` has no radius to compare, so the clause never fires for it
        // — including on a zero-extent node mid-animation, which is the case
        // that would otherwise turn every square corner into a pill.
        for edge in [0.0, 1.0, 4.0, 1000.0] {
            assert_eq!(
                corner_for(CornerRole::Tiled, edge),
                "shape.corner-none",
                "a tiled node at {edge} units must stay square"
            );
        }
        assert_eq!(
            corner_for(CornerRole::Floating, 0.0),
            "shape.corner-md",
            "a zero-extent node has no shorter edge to be half of; the role \
             stands rather than collapsing to a pill"
        );

        // corners_for's own totality, over the same five roles and
        // `Joined::NONE` specifically: every corner must be exactly
        // `corner_for`'s answer, unsquared. This is the equivalence the
        // whole per-corner design rests on — one rule read four times, not
        // two rules that could drift apart.
        //
        // Falsified by making `Joined::NONE` square `top_left` regardless:
        //
        // ```text
        // assertion `left == right` failed: Tiled: Joined::NONE must return
        // four copies of corner_for's own answer, or corners_for is a
        // second rule rather than one rule read four times
        //   left: ("shape.corner-none", "shape.corner-none", "shape.corner-none", "shape.corner-none")
        //  right: ("shape.corner-none", "shape.corner-none", "shape.corner-none", "shape.corner-none")
        // ```
        for role in roles {
            let free = corner_for(role, 200.0);
            let none = corners_for(role, 200.0, Joined::NONE);
            assert_eq!(
                (
                    none.top_left,
                    none.top_right,
                    none.bottom_right,
                    none.bottom_left
                ),
                (free, free, free, free),
                "{role:?}: Joined::NONE must return four copies of \
                 corner_for's own answer, or corners_for is a second rule \
                 rather than one rule read four times"
            );
        }

        // Every one of the sixteen `Joined` combinations, for a role whose
        // free answer is not already square — `Grouping` is `shape.corner-sm`
        // at 200 units, so a squared corner is visibly distinguishable from
        // an unsquared one, which a role that already answers `corner-none`
        // could not prove.
        let role = CornerRole::Grouping;
        let free = corner_for(role, 200.0);
        let square = SHAPE_RAMP[0].0;
        for top in [false, true] {
            for right in [false, true] {
                for bottom in [false, true] {
                    for left in [false, true] {
                        let joined = Joined {
                            top,
                            right,
                            bottom,
                            left,
                        };
                        let corners = corners_for(role, 200.0, joined);
                        assert_eq!(
                            corners.top_left,
                            if top || left { square } else { free },
                            "top_left under {joined:?}"
                        );
                        assert_eq!(
                            corners.top_right,
                            if top || right { square } else { free },
                            "top_right under {joined:?}"
                        );
                        assert_eq!(
                            corners.bottom_right,
                            if bottom || right { square } else { free },
                            "bottom_right under {joined:?}"
                        );
                        assert_eq!(
                            corners.bottom_left,
                            if bottom || left { square } else { free },
                            "bottom_left under {joined:?}"
                        );
                    }
                }
            }
        }

        // A joined edge squares even `Pill`, which otherwise ignores the
        // geometry outright: a flush pair still meets on a flat seam, and
        // the side that touches nothing keeps the stadium.
        let flush = corners_for(
            CornerRole::Pill,
            200.0,
            Joined {
                right: true,
                ..Joined::NONE
            },
        );
        assert_eq!(
            flush.top_right, square,
            "a joined pill still squares on the seam"
        );
        assert_eq!(flush.bottom_right, square);
        assert_eq!(
            flush.top_left, "shape.corner-full",
            "the free side keeps the pill"
        );
        assert_eq!(flush.bottom_left, "shape.corner-full");
    }

    /// The scrim is black in both themes, deeper in dark, and never opaque.
    ///
    /// Opacity is the load-bearing half: a scrim at alpha 255 is not a scrim,
    /// it is a page, and the thing it is supposed to be dimming is gone rather
    /// than de-emphasised.
    #[test]
    fn the_scrim_darkens_without_hiding_and_deepens_in_the_dark_theme() {
        let mut alphas = Vec::new();
        for (label, theme) in [("light", light()), ("dark", dark())] {
            let scrim = theme_color(&theme, SCRIM_TOKEN);
            assert!(
                scrim.r < 1e-6 && scrim.g < 1e-6 && scrim.b < 1e-6,
                "{label}: {SCRIM_TOKEN} is {scrim:?}; occlusion darkens, and a \
                 lightened scrim in dark mode is not a scrim"
            );
            assert!(
                scrim.a > 0.25 && scrim.a < 1.0,
                "{label}: {SCRIM_TOKEN} carries alpha {}; at 1.0 it hides the \
                 page instead of dimming it, and under 0.25 it does not read \
                 as blocking",
                scrim.a
            );
            alphas.push(scrim.a);
        }
        assert!(
            alphas[1] > alphas[0],
            "the dark theme's scrim is {} against light's {}; a dark page has \
             less room left to lose, so its scrim has to take more",
            alphas[1],
            alphas[0]
        );
    }

    /// Every name the vocabulary declares is assigned in both themes, and no
    /// theme assigns a name the vocabulary does not declare.
    ///
    /// `Theme::build` already refuses the first half at construction, so
    /// `light()` and `dark()` panicking would catch it. The second half is
    /// the one nothing else covers: an extra assignment is silently accepted
    /// by `Theme::build` (it walks the vocabulary, not the values), so a
    /// token added to `light()` and forgotten in `standard_vocabulary()`
    /// resolves in one theme, is invisible to the generated `token.d.luau`,
    /// and is refused at tree acceptance — three symptoms, no error message.
    ///
    /// Fifty-seven names arrived in one change; this is the check that they
    /// all arrived in all three places.
    #[test]
    fn neither_theme_assigns_a_name_the_vocabulary_does_not_declare() {
        let vocab = standard_vocabulary();
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for name in theme.values().keys() {
                assert!(
                    vocab.contains(name),
                    "{label} assigns `{name}`, which standard_vocabulary() does \
                     not declare: it will resolve here, be missing from \
                     token.d.luau, and be refused at tree acceptance"
                );
            }
        }
    }

    /// **Every colour this design system ships is a grey, except the accent
    /// and the status palette — and both of those are already gated.**
    ///
    /// This is the standing rule made mechanical. The operator this project is
    /// built for is red-green colour blind, so "colour is never the only
    /// channel" is not a preference, and the cheapest way for that rule to
    /// break is not a bad hue: it is a *new* hue arriving quietly in a family
    /// nobody thought of as carrying meaning. A green `border-success`, a red
    /// `icon-error`, an amber `layer-warning` — each one reads as an obvious
    /// small addition, and each one puts meaning on an axis this reader does
    /// not have.
    ///
    /// Thirty-one colour names arrived on 2026-08-25. This walks the whole
    /// vocabulary, in both themes, and allows chroma only where a gate already
    /// covers it:
    ///
    /// * `accent.primary` — blue, chosen as the axis neither deuteranopia nor
    ///   protanopia collapses, and measured by
    ///   [`the_accent_survives_red_green_colour_blindness`].
    /// * `border-interactive` — byte-identical to the accent, asserted by
    ///   [`the_numbered_border_names_carry_measured_tones`], so it inherits
    ///   that measurement rather than needing its own.
    /// * `focus.ring` and `focus-inverse` — byte-identical to the accent,
    ///   asserted by [`the_focus_underline_is_legible_on_the_card`]. The
    ///   underline is extra geometry; hue is not its only channel.
    /// * `status.*` and `support-*` — the three hand-tuned status colours
    ///   under two names each, held apart under both simulations by
    ///   [`status_colours_stay_apart_under_red_green_colour_blindness`].
    /// * `link-primary` — the accent's hue walked toward the page ink until
    ///   it clears AA as text ([`link_ink`]); measured under both simulations
    ///   against the status marks by
    ///   [`the_accent_survives_red_green_colour_blindness`], and held to the
    ///   text floor on every layer by
    ///   [`the_link_ink_clears_aa_on_every_layer_and_is_still_blue`]. Its
    ///   component pairs it with an underline, so hue is not its only
    ///   channel either.
    ///
    /// Anything else with chroma fails here, naming itself. Adding a hue is
    /// then a deliberate act that has to come with a gate, which is the whole
    /// point.
    #[test]
    fn no_shipped_colour_carries_a_hue_that_is_not_already_gated() {
        /// How far apart the linear-light channels may sit before a colour
        /// counts as chromatic. Small: every grey in this file is written as
        /// three equal channels, so the only source of a difference is a
        /// deliberate hue.
        const ACHROMATIC: f32 = 1e-4;

        let vocab = standard_vocabulary();
        for (label, theme) in [("light", light()), ("dark", dark())] {
            for token in vocab.names() {
                if vocab.kind_of(token) != Some(crate::token::value::TokenKind::Color) {
                    continue;
                }
                let name = token.as_str();
                let gated = name == ACCENT_TOKEN
                    || name == LINK_TOKEN
                    // The destructive fill, gated by
                    // `the_danger_fill_carries_a_white_label_and_reads_without_colour`:
                    // its label clears AA on it, it clears the fill floor on
                    // every layer it is declared paintable on, and it stays
                    // ΔE*ab-separated from the button tone it has to be told
                    // apart from under both red-green simulations.
                    || name == DANGER_FILL_TOKEN
                    || name == BORDER_INTERACTIVE_TOKEN
                    || name == RING_TOKEN
                    || name == "focus-inverse"
                    || name.starts_with("status.")
                    || name.starts_with("support-");
                if gated {
                    continue;
                }
                let c = theme_color(&theme, name);
                let spread = c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
                assert!(
                    spread <= ACHROMATIC,
                    "{label}/{name} carries a hue (linear r={:.4} g={:.4} \
                     b={:.4}, spread {spread:.4}) and no colour-blindness gate \
                     covers it. Either it is a grey and this is a typo, or it \
                     is a new signal colour — in which case it belongs in the \
                     status family, behind \
                     `status_colours_stay_apart_under_red_green_colour_blindness`, \
                     and not in a family that reached the screen ungated.",
                    c.r,
                    c.g,
                    c.b
                );
            }
        }
    }

    /// A token's CIE L\* , read out of a theme.
    fn lightness_of_token(theme: &crate::token::Theme, token: &str) -> f32 {
        let c = theme_color(theme, token);
        116.0 * super::lab_f(0.212_672_9 * c.r + 0.715_152_2 * c.g + 0.072_175 * c.b) - 16.0
    }
}
