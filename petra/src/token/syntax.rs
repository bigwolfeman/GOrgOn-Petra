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

/// Declare every `syntax.*` name, at kind colour.
pub(crate) fn declare(vocab: &mut Vocabulary) {
    for class in SYNTAX_CLASSES {
        let name = syntax_token(class).expect("every listed class has a token");
        vocab.declare(DesignToken::new(name, TokenKind::Color));
    }
}

/// Assign every `syntax.*` name in `mode`.
pub(crate) fn insert(values: &mut BTreeMap<TokenName, TokenValue>, mode: ThemeMode) {
    for class in SYNTAX_CLASSES {
        let name = syntax_token(class).expect("every listed class has a token");
        let [r, g, b] = ink_of(class)
            .unwrap_or_else(|| panic!("syntax class {class:?} has no ink"))
            .rgb(mode);
        values.insert(
            name,
            TokenValue::Color(ColorValue::from_srgb8(r, g, b, 0xff)),
        );
    }
}

/// The grounds a code well paints on: the page (`surface.base`), the raised
/// well (`surface.raised`, which is `surface.layer-one`), and the layer
/// above it (`surface.layer-two`).
#[cfg(test)]
pub(crate) const GROUNDS: [&str; 3] = ["surface.base", "surface.raised", "surface.layer-two"];

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
    use crate::token::shipped::{dark, light, standard_vocabulary};
    use crate::token::{ColorValue, Theme};

    fn themes() -> [(&'static str, Theme); 2] {
        [("light", light()), ("dark", dark())]
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
                        ratio >= 4.5,
                        "{mode}: syntax.{class} on {ground} is {ratio:.2}:1, under 4.5:1"
                    );
                }
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
