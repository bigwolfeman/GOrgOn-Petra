//! The syntax token family: one colour name per highlight class.
//!
//! Spec 016 FR-058 and FR-059. The file service names stretches of code with
//! the classes in `specs/016-projects-files/contracts/projections.md`
//! ("Highlight classes"), and the viewer maps each class to a token here.
//! Carbon ships no syntax palette (`.cds--snippet` sets every variant in one
//! ink), so every value below is this library's own, and
//! `specs/005-petra-carbon-authoring/contracts/token-vocabulary.md` section
//! 12 records why the inventory-driven rule of section 3 admits the family.
//!
//! # The shape of the family
//!
//! A class `keyword.control` is the token `syntax.keyword.control`. Siblings
//! (a class and its dotted children) may share a value, so there are 37
//! names and 11 inks. The inks are chosen for a red-green colour-blind
//! reader, who is the reader this library is built for:
//!
//! * **No pair rests on hue alone.** Adjacent classes (the ones that sit
//!   side by side in real code) differ by at least
//!   [`MIN_ADJACENT_SEPARATION`] CIE delta E after simulating both
//!   red-green dichromacies, in both model families.
//! * **The diff pair rests on lightness.** `syntax.diff.plus` and
//!   `syntax.diff.minus` differ by at least [`MIN_DIFF_LIGHTNESS_STEP`] L\*
//!   in each theme, on top of the delta E floor above. The diff view also
//!   adds a `+` or `-` glyph, so the colour is never the only channel.
//! * **Every ink is text.** Each clears 4.5:1 on the grounds a code well
//!   paints on (see [`GROUNDS`]).
//!
//! The values are tuned by script in L\*C\*h with a hue band per ink, then
//! checked by the tests at the bottom, which are the gate. A value move is
//! digest-free (`token-vocabulary.md` section 1), so the operator can retune
//! the palette without touching a name.

use std::collections::BTreeMap;

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::value::{ColorValue, TokenKind, TokenValue};
use crate::token::vocabulary::{DesignToken, Vocabulary};

/// The 37 highlight classes of `contracts/projections.md`, in that file's
/// order. The daemon sends these names, and a capture with no listed prefix
/// gets no class and paints as plain text, which is the viewer's rule and
/// not this table's.
pub const SYNTAX_CLASSES: [&str; 37] = [
    "attribute",
    "comment",
    "comment.doc",
    "constant",
    "constant.builtin",
    "constructor",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "keyword.control",
    "label",
    "module",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.escape",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
    "markup.heading",
    "markup.bold",
    "markup.italic",
    "markup.link",
    "markup.raw",
    "markup.list",
    "markup.quote",
    "diff.plus",
    "diff.minus",
];

/// The token a highlight class paints in, or `None` when `class` is not one
/// of [`SYNTAX_CLASSES`]. Exact match: reducing a grammar capture to its
/// longest listed prefix is the caller's step.
#[must_use]
pub fn syntax_token(class: &str) -> Option<TokenName> {
    SYNTAX_CLASSES
        .contains(&class)
        .then(|| TokenName::new(format!("syntax.{class}")).expect("a syntax class is a name"))
}

/// One of the eleven inks the 37 classes share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ink {
    /// Plain code: identifiers and the words with no class of their own.
    Variable,
    /// Comments and quotes: the quiet tone.
    Comment,
    /// Keywords, labels, `self`.
    Keyword,
    /// Functions, macros, constructors, headings.
    Function,
    /// Types, modules, links.
    Type,
    /// Strings, escapes, raw spans.
    String,
    /// Numbers and constants.
    Number,
    /// Properties, attributes, tags.
    Property,
    /// Operators, brackets, delimiters, list markers.
    Punct,
    /// Added lines.
    Plus,
    /// Removed lines.
    Minus,
}

impl Ink {
    /// The sRGB value in `mode`.
    ///
    /// Light tones are the dark end of each hue band (contrast against white
    /// and gray 10), dark tones the light end (contrast against the dark
    /// layers). `Variable` is the polarity's own text ink.
    pub(crate) fn rgb(self, mode: ThemeMode) -> [u8; 3] {
        let (light, dark) = match self {
            Self::Variable => ([0x1a, 0x1a, 0x1a], [0xf2, 0xf2, 0xf2]),
            Self::Comment => ([0x75, 0x6d, 0x55], [0x9a, 0xa0, 0xb6]),
            Self::Keyword => ([0x2b, 0x60, 0xc0], [0x79, 0x9b, 0xf5]),
            Self::Function => ([0x9f, 0x5a, 0x08], [0xd1, 0xb5, 0x51]),
            Self::Type => ([0x14, 0x75, 0x9e], [0x70, 0xc4, 0xe3]),
            Self::String => ([0x77, 0x19, 0x00], [0xe8, 0x82, 0x52]),
            Self::Number => ([0x24, 0x7b, 0x52], [0x81, 0xf6, 0xa8]),
            Self::Property => ([0x0b, 0x2a, 0x5c], [0xcb, 0x90, 0xc3]),
            Self::Punct => ([0x3b, 0x4b, 0x55], [0xb8, 0xab, 0x98]),
            Self::Plus => ([0x17, 0x42, 0x1f], [0xc4, 0xee, 0xb4]),
            Self::Minus => ([0xb8, 0x41, 0x6b], [0xe9, 0x77, 0xb1]),
        };
        match mode {
            ThemeMode::Light => light,
            ThemeMode::Dark => dark,
        }
    }
}

/// Which ink a class takes.
pub(crate) fn ink_of(class: &str) -> Option<Ink> {
    Some(match class {
        "variable" | "variable.parameter" | "markup.bold" | "markup.italic" => Ink::Variable,
        "comment" | "comment.doc" | "markup.quote" => Ink::Comment,
        "keyword" | "keyword.control" | "label" | "variable.builtin" => Ink::Keyword,
        "function" | "function.builtin" | "function.macro" | "constructor" | "markup.heading" => {
            Ink::Function
        }
        "type" | "type.builtin" | "module" | "markup.link" => Ink::Type,
        "string" | "string.escape" | "string.special" | "markup.raw" => Ink::String,
        "number" | "constant" | "constant.builtin" => Ink::Number,
        "property" | "attribute" | "tag" => Ink::Property,
        "operator"
        | "punctuation"
        | "punctuation.bracket"
        | "punctuation.delimiter"
        | "markup.list" => Ink::Punct,
        "diff.plus" => Ink::Plus,
        "diff.minus" => Ink::Minus,
        _ => return None,
    })
}

/// Declare every `syntax.*` name and the two diff bands, at kind colour.
pub(crate) fn declare(vocab: &mut Vocabulary) {
    for class in SYNTAX_CLASSES {
        let name = syntax_token(class).expect("every listed class has a token");
        vocab.declare(DesignToken::new(name, TokenKind::Color));
    }
    for (band, _, _) in DIFF_BANDS {
        vocab.declare(DesignToken::new(
            TokenName::new(band).expect("a band is a name"),
            TokenKind::Color,
        ));
    }
}

/// The row bands of a diff view: `(band token, the class whose ink tints
/// it, its step off the reference layer in CIE L*)`.
///
/// `syntax.diff.plus` and `syntax.diff.minus` are inks, tuned to be read as
/// text. A diff row also wants a band behind the whole line, and an ink is
/// the wrong colour for that: every other ink has to stay legible on it.
/// So a band is a near-ground tone tinted at up to [`BAND_CHROMA`] with its
/// ink's hue. The steps are the lightness channel: the removed band stands
/// twice as far off the reference as the added one, so the pair differs by
/// lightness after any colour-blind simulation. The `+` and `-` glyphs stay
/// the channel that carries the meaning; a band is an extra cue
/// (`specs/017-version-control/contracts/projections.md`, "Diff lines").
///
/// The reference is the polarity's outermost layer: the lightest one in a
/// light theme, stepped darker, and the darkest one in a dark theme, stepped
/// lighter. Both light themes carry white among their layers and every ink
/// was tuned to clear `#f2f2f2`, so a band inside that span walks no ink;
/// both dark themes keep their bands under layer two, the darkest code
/// ground their inks clear. The steps are as large as that span allows.
/// The bands still join [`GROUNDS`], so a later retune that moved them past
/// it would walk the inks rather than leave one illegible.
pub const DIFF_BANDS: [(&str, &str, f32); 2] = [
    ("diff.plus", "diff.plus", PLUS_BAND_STEP),
    ("diff.minus", "diff.minus", MINUS_BAND_STEP),
];

/// How far the added band stands off the reference layer, in CIE L*.
const PLUS_BAND_STEP: f32 = 2.5;

/// How far the removed band stands off the reference layer, in CIE L*:
/// twice [`PLUS_BAND_STEP`].
const MINUS_BAND_STEP: f32 = 5.0;

/// The most chroma, in CIE C*ab, a band carries in its ink's hue. A tint, not
/// a colour: it adds hue for a reader who sees it and costs nothing a reader
/// who does not would need. Near white the sRGB gamut is narrow, so a band
/// takes less where this much would clip a channel and move its lightness.
const BAND_CHROMA: f32 = 10.0;

/// The layer a band steps off: the lightest of `layers` in a light theme,
/// the darkest in a dark one.
pub(crate) fn band_reference(layers: &[ColorValue], mode: ThemeMode) -> ColorValue {
    let lightness = |c: &ColorValue| crate::token::measure::to_lab([c.r, c.g, c.b])[0];
    let pick = layers.iter().copied().max_by(|a, b| {
        let (la, lb) = (lightness(a), lightness(b));
        match mode {
            ThemeMode::Light => la.total_cmp(&lb),
            ThemeMode::Dark => lb.total_cmp(&la),
        }
    });
    pick.expect("a theme has layers")
}

/// Assign the two [`DIFF_BANDS`], stepped off `reference`
/// ([`band_reference`]).
pub(crate) fn insert_bands(
    values: &mut BTreeMap<TokenName, TokenValue>,
    mode: ThemeMode,
    reference: ColorValue,
) {
    let base = crate::token::measure::to_lab([reference.r, reference.g, reference.b]);
    let direction = match mode {
        ThemeMode::Light => -1.0,
        ThemeMode::Dark => 1.0,
    };
    for (band, class, step) in DIFF_BANDS {
        let [r, g, b] = ink_of(class)
            .unwrap_or_else(|| panic!("band {band:?} names class {class:?}, which has no ink"))
            .rgb(mode);
        let ink = ColorValue::from_srgb8(r, g, b, 0xff);
        let [_, a, bb] = crate::token::measure::to_lab([ink.r, ink.g, ink.b]);
        let chroma = a.hypot(bb).max(f32::EPSILON);
        let target = base[0] + direction * step;
        // The most chroma, in whole steps, that lands on the target
        // lightness: `from_lab` clamps a channel out of gamut, and a clamped
        // channel moves the lightness the band was stepped to.
        let rgb = (0..=BAND_CHROMA as u8)
            .rev()
            .map(|c| {
                let scale = f32::from(c) / chroma;
                crate::token::measure::from_lab([target, a * scale, bb * scale])
            })
            .find(|rgb| (crate::token::measure::to_lab(*rgb)[0] - target).abs() < 0.01)
            .unwrap_or_else(|| panic!("band {band:?} cannot reach L* {target}"));
        values.insert(
            TokenName::new(band).expect("a band is a name"),
            TokenValue::Color(ColorValue {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
                a: 1.0,
            }),
        );
    }
}

/// The text floor every ink clears on every code ground: WCAG AA for body
/// text.
pub(crate) const MIN_INK_CONTRAST: f32 = 4.5;

/// How far one step of [`legible_on`]'s walk moves an ink, in L\*.
const INK_WALK_STEP: f32 = 0.5;

/// Assign every `syntax.*` name in `mode`, each ink legible on every one of
/// the theme's code `grounds` ([`GROUNDS`], resolved by the caller).
///
/// The table above is tuned for the grounds White and Gray 100 ship. A
/// theme with a nearer ground (Gray 90's layer two is `#3a3a3a`, Gray 100's
/// `#333333`) would leave the dimmest inks under the floor, so each ink is
/// walked by [`legible_on`] until it clears. An ink that already clears is
/// returned untouched, so the themes the table was tuned on carry it
/// exactly, and no theme carries a second hand-picked table.
pub(crate) fn insert(
    values: &mut BTreeMap<TokenName, TokenValue>,
    mode: ThemeMode,
    grounds: &[ColorValue],
) {
    for class in SYNTAX_CLASSES {
        let name = syntax_token(class).expect("every listed class has a token");
        let [r, g, b] = ink_of(class)
            .unwrap_or_else(|| panic!("syntax class {class:?} has no ink"))
            .rgb(mode);
        let ink =
            legible_on(ColorValue::from_srgb8(r, g, b, 0xff), mode, grounds).unwrap_or_else(|| {
                panic!("syntax class {class:?} cannot be made legible on {grounds:?}")
            });
        values.insert(name, TokenValue::Color(ink));
    }
}

/// `ink`, moved away from the grounds in CIE L\* (lighter in a dark theme,
/// darker in a light one) with its a\* and b\* kept, in [`INK_WALK_STEP`]
/// steps until it clears [`MIN_INK_CONTRAST`] on every ground.
///
/// Lightness only, because hue and chroma are what keep adjacent classes
/// apart for a red-green reader (`MIN_ADJACENT_SEPARATION`); a walk toward
/// white would wash them out together and close the very gaps the table was
/// tuned to hold. `None` when no lightness clears the floor, which the
/// caller turns into a loud failure: a theme whose code ground no ink can
/// be read on is a theme bug, not a value to clamp.
fn legible_on(ink: ColorValue, mode: ThemeMode, grounds: &[ColorValue]) -> Option<ColorValue> {
    let clears = |c: ColorValue| {
        grounds
            .iter()
            .all(|g| c.contrast_ratio(*g) >= MIN_INK_CONTRAST)
    };
    if clears(ink) {
        return Some(ink);
    }
    let lab = crate::token::measure::to_lab([ink.r, ink.g, ink.b]);
    let direction = match mode {
        ThemeMode::Light => -1.0,
        ThemeMode::Dark => 1.0,
    };
    (1..=200u16)
        .map(|step| lab[0] + direction * INK_WALK_STEP * f32::from(step))
        .take_while(|l| (0.0..=100.0).contains(l))
        .map(|l| {
            let [r, g, b] = crate::token::measure::from_lab([l, lab[1], lab[2]]);
            ColorValue { r, g, b, a: ink.a }
        })
        .find(|c| clears(*c))
}

/// The grounds a code well paints on: the three surfaces it sits on, and
/// the two [`DIFF_BANDS`] a diff row sits on.
pub(crate) const GROUNDS: [&str; 5] = [
    "surface.base",
    "surface.raised",
    "surface.layer-two",
    "diff.plus",
    "diff.minus",
];

/// The floor, in CIE delta E, between two adjacent classes after a
/// red-green simulation. The status floor is 30
/// ([`crate::token::measure::MIN_STATUS_SEPARATION`]). Eleven inks cannot all
/// sit 30 apart in a dichromat's two-axis space, and code has position and
/// shape to help where a status dot has only colour, so syntax takes 18.
#[cfg(test)]
pub(crate) const MIN_ADJACENT_SEPARATION: f32 = 18.0;

/// The floor, in L\*, between the diff pair in either theme.
#[cfg(test)]
pub(crate) const MIN_DIFF_LIGHTNESS_STEP: f32 = 20.0;

/// The floor, in delta E after simulation, for the diff pair. Higher than
/// [`MIN_ADJACENT_SEPARATION`]: plus and minus are the pair a reader most
/// needs to tell apart, and the colour-blind pair they fail by default.
#[cfg(test)]
pub(crate) const MIN_DIFF_SEPARATION: f32 = 25.0;

/// Classes that sit side by side in real code, by the class they paint.
///
/// Not every pair: eleven inks have 55, and most never touch (a comment
/// beside a diff marker). These are the neighbours in source text: a keyword
/// then a name, a name then a bracket, a string beside a number in an
/// argument list, a property after a dot. A class that varies the ink of a
/// sibling (`keyword.control`) is covered through its sibling.
#[cfg(test)]
pub(crate) const ADJACENT: &[(&str, &str)] = &[
    ("keyword", "function"),
    ("keyword", "type"),
    ("keyword", "variable"),
    ("keyword", "string"),
    ("keyword", "number"),
    ("keyword", "property"),
    ("function", "variable"),
    ("function", "punctuation"),
    ("function", "property"),
    ("function", "type"),
    ("function", "number"),
    ("type", "variable"),
    ("type", "punctuation"),
    ("type", "number"),
    ("string", "number"),
    ("string", "comment"),
    ("string", "punctuation"),
    ("string", "variable"),
    ("string", "type"),
    ("number", "punctuation"),
    ("number", "variable"),
    ("property", "variable"),
    ("property", "punctuation"),
    ("punctuation", "variable"),
    ("comment", "punctuation"),
    ("comment", "variable"),
    ("diff.plus", "diff.minus"),
    ("diff.plus", "variable"),
    ("diff.minus", "variable"),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::token::colourblind::{contrast, theme_color};
    use crate::token::measure::{SIMULATION_MODELS, separation, to_lab};
    use crate::token::shipped::standard_vocabulary;
    use crate::token::{ColorValue, Theme};

    fn themes() -> [(&'static str, Theme); 4] {
        crate::token::colourblind::shipped_themes()
    }

    fn ink_colour(theme: &Theme, class: &str) -> ColorValue {
        theme_color(theme, syntax_token(class).expect("a class").as_str())
    }

    fn lightness(c: ColorValue) -> f32 {
        to_lab([c.r, c.g, c.b])[0]
    }

    /// Every class has a token in the vocabulary, as a colour, and a value
    /// in both themes. A class with no token is the failure the shell's
    /// mapping test (T055) repeats from its side.
    #[test]
    fn every_class_has_a_colour_token_in_both_themes() {
        let vocab = standard_vocabulary();
        assert_eq!(SYNTAX_CLASSES.len(), 37);
        let unique: BTreeSet<&str> = SYNTAX_CLASSES.iter().copied().collect();
        assert_eq!(unique.len(), 37, "a class is listed twice");
        for class in SYNTAX_CLASSES {
            let name = syntax_token(class).expect("a token for a listed class");
            assert_eq!(name.as_str(), format!("syntax.{class}"));
            assert_eq!(
                vocab.kind_of(&name),
                Some(TokenKind::Color),
                "{class}: not declared as a colour"
            );
            assert!(ink_of(class).is_some(), "{class}: no ink");
            for (mode, theme) in themes() {
                assert!(theme.value(&name).is_some(), "{mode}: {name} has no value");
            }
        }
        assert!(syntax_token("not.a.class").is_none());
        assert!(
            syntax_token("syntax.keyword").is_none(),
            "a token name is not a class"
        );
    }

    /// The projection contract's class list is the source, so the table
    /// here cannot drift from it: every class in the contract's backtick
    /// list is in [`SYNTAX_CLASSES`] and the reverse.
    #[test]
    fn the_class_list_is_the_projection_contracts() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../specs/016-projects-files/contracts/projections.md"
        );
        let Ok(text) = std::fs::read_to_string(path) else {
            // The Petra repository on its own does not carry the parent
            // repository's specs; the gate runs in the parent checkout.
            return;
        };
        let section = text
            .split("## Highlight classes")
            .nth(1)
            .expect("the contract has a Highlight classes section");
        let line = section
            .lines()
            .find(|l| l.starts_with("`attribute`"))
            .expect("the class list line");
        let listed: Vec<&str> = line
            .trim_end_matches('.')
            .split(", ")
            .map(|c| c.trim_matches('`'))
            .collect();
        assert_eq!(
            listed, SYNTAX_CLASSES,
            "contracts/projections.md and SYNTAX_CLASSES disagree"
        );
    }

    /// Every ink is text on every ground a code well paints on, in both
    /// themes: 4.5:1, the same floor `text.muted` holds.
    #[test]
    fn every_syntax_ink_clears_text_contrast_on_every_code_ground() {
        for (mode, theme) in themes() {
            for class in SYNTAX_CLASSES {
                let ink = ink_colour(&theme, class);
                for ground in GROUNDS {
                    let bg = theme_color(&theme, ground);
                    let ratio = contrast(ink, bg);
                    assert!(
                        ratio >= MIN_INK_CONTRAST,
                        "{mode}: syntax.{class} on {ground} is {ratio:.2}:1, under {MIN_INK_CONTRAST}:1"
                    );
                }
            }
        }
    }

    /// The derivation in [`super::insert`]: White and Gray 100, the grounds
    /// the table was tuned on, carry the table exactly; where a theme's ground
    /// moves an ink, only its L\* moved, so its hue and chroma (the channels
    /// that keep neighbours apart for a red-green reader) are the table's.
    #[test]
    fn the_ink_walk_moves_lightness_only_and_only_where_a_ground_needs_it() {
        for (mode, theme) in themes() {
            let tuned = matches!(
                theme.id(),
                Some(crate::token::ThemeId::White | crate::token::ThemeId::Gray100)
            );
            for class in SYNTAX_CLASSES {
                let [r, g, b] = ink_of(class).unwrap().rgb(theme.mode());
                let table = ColorValue::from_srgb8(r, g, b, 0xff);
                let ink = ink_colour(&theme, class);
                if tuned {
                    assert_eq!(ink, table, "{mode}: syntax.{class} moved off the table");
                    continue;
                }
                let (was, now) = (
                    to_lab([table.r, table.g, table.b]),
                    to_lab([ink.r, ink.g, ink.b]),
                );
                assert!(
                    (was[1] - now[1]).abs() < 0.5 && (was[2] - now[2]).abs() < 0.5,
                    "{mode}: syntax.{class} changed hue or chroma: {was:?} -> {now:?}"
                );
            }
        }
    }

    /// The colour-blind sweep, extended to adjacent classes: every pair in
    /// [`ADJACENT`] reads apart to a deuteranope and a protanope, under both
    /// model families, by at least [`MIN_ADJACENT_SEPARATION`].
    ///
    /// Falsified by setting `Ink::Keyword`'s dark value to `Ink::Function`'s:
    /// the `keyword`/`function` pair prints a separation of 0.
    #[test]
    fn adjacent_classes_stay_apart_under_red_green_colour_blindness() {
        for (mode, theme) in themes() {
            for (a, b) in ADJACENT {
                let (ca, cb) = (ink_colour(&theme, a), ink_colour(&theme, b));
                for (model, matrix) in SIMULATION_MODELS {
                    let apart = separation(ca, cb, matrix);
                    assert!(
                        apart >= MIN_ADJACENT_SEPARATION,
                        "{mode}: syntax.{a} vs syntax.{b} under {model} is \
                         {apart:.1} delta E, under {MIN_ADJACENT_SEPARATION}"
                    );
                }
            }
        }
    }

    /// The adjacency table names real classes only, so a rename in the
    /// class list cannot turn a sweep row into a test of nothing.
    #[test]
    fn the_adjacency_table_names_only_listed_classes() {
        for (a, b) in ADJACENT {
            for class in [a, b] {
                assert!(
                    SYNTAX_CLASSES.contains(class),
                    "{class} is not a highlight class"
                );
            }
        }
    }

    /// Plus and minus differ by lightness, in each theme, and the
    /// simulated pair is far apart too. A hue-only difference collapses
    /// under a red-green simulation and would fail the second assertion
    /// first; the lightness step is what survives any simulation.
    #[test]
    fn the_diff_pair_differs_by_lightness_not_hue_alone() {
        for (mode, theme) in themes() {
            let plus = ink_colour(&theme, "diff.plus");
            let minus = ink_colour(&theme, "diff.minus");
            let step = (lightness(plus) - lightness(minus)).abs();
            assert!(
                step >= MIN_DIFF_LIGHTNESS_STEP,
                "{mode}: plus and minus differ by {step:.1} L*, under {MIN_DIFF_LIGHTNESS_STEP}"
            );
            for (model, matrix) in SIMULATION_MODELS {
                let apart = separation(plus, minus, matrix);
                assert!(
                    apart >= MIN_DIFF_SEPARATION,
                    "{mode}: plus vs minus under {model} is {apart:.1} delta E, \
                     under {MIN_DIFF_SEPARATION}"
                );
            }
        }
    }

    /// Each diff band stands its own step off its theme's reference layer
    /// ([`band_reference`]), in the polarity's direction, and the removed
    /// band stands further off than the added one under every simulation
    /// model, so the pair keeps its lightness order for a red-green reader.
    /// Plain text reads on both.
    ///
    /// Falsified by swapping the two steps in [`DIFF_BANDS`]: the order
    /// assertion names the theme.
    #[test]
    fn the_diff_bands_step_off_the_ground_by_lightness() {
        /// The band is stored as linear floats and reached by a Lab round
        /// trip; it lands within a hundredth of its target.
        const ROUNDING: f32 = 0.02;
        for (mode, theme) in themes() {
            let layers: Vec<ColorValue> = crate::token::shipped::LAYER_TOKENS
                .iter()
                .map(|t| theme_color(&theme, t))
                .collect();
            let reference = band_reference(&layers, theme.mode());
            let base = lightness(reference);
            let direction = match theme.mode() {
                ThemeMode::Light => -1.0,
                ThemeMode::Dark => 1.0,
            };
            for (band, _, step) in DIFF_BANDS {
                let off = (lightness(theme_color(&theme, band)) - base) * direction;
                assert!(
                    (off - step).abs() < ROUNDING,
                    "{mode}: {band} stands {off:.2} L* off its reference layer, not {step}"
                );
                let text = theme_color(&theme, "text.primary");
                let ratio = contrast(text, theme_color(&theme, band));
                assert!(
                    ratio >= MIN_INK_CONTRAST,
                    "{mode}: text.primary on {band} is {ratio:.2}:1"
                );
            }
            let (plus, minus) = (
                theme_color(&theme, "diff.plus"),
                theme_color(&theme, "diff.minus"),
            );
            for (model, matrix) in SIMULATION_MODELS {
                let seen = |c: ColorValue| to_lab(crate::token::measure::simulate(c, matrix))[0];
                let g = seen(reference);
                assert!(
                    (seen(minus) - g).abs() > (seen(plus) - g).abs(),
                    "{mode}: under {model} the removed band does not stand further \
                     off the reference than the added one"
                );
            }
        }
    }

    /// Siblings may share an ink, and nothing else does by accident: two
    /// different inks never collapse to one value in a theme. Eleven inks,
    /// eleven values.
    #[test]
    fn eleven_inks_are_eleven_values_in_each_theme() {
        for (mode, theme) in themes() {
            let values: BTreeSet<[u32; 3]> = SYNTAX_CLASSES
                .iter()
                .map(|c| {
                    let v = ink_colour(&theme, c);
                    [v.r.to_bits(), v.g.to_bits(), v.b.to_bits()]
                })
                .collect();
            assert_eq!(values.len(), 11, "{mode}: inks collapsed");
        }
    }
}
