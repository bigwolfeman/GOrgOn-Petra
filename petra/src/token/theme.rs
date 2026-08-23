//! A theme: token → value assignments for one [`ThemeMode`], checked
//! complete against a [`Vocabulary`] at construction time.
//!
//! "Complete" is [`Theme::build`]'s whole job: every vocabulary-declared
//! token must be assigned a value, at the declared kind, or the theme is
//! refused with every offending name listed — never a partial `Theme` that
//! silently falls back for the tokens it forgot.

use std::collections::BTreeMap;
use std::fmt;

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::value::{TokenKind, TokenValue};
use crate::token::vocabulary::Vocabulary;

/// Token → value assignments for one mode, guaranteed complete against the
/// [`Vocabulary`] it was built with.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    mode: ThemeMode,
    values: BTreeMap<TokenName, TokenValue>,
}

impl Theme {
    /// Build a theme, refusing it unless `values` assigns every token
    /// `vocabulary` declares, each at its declared kind, and each at a value
    /// layout can use.
    ///
    /// The third condition is why a styling prop can be a bare token
    /// reference. `Props.spacing` and `Props.padding` used to carry numbers,
    /// and tree acceptance range-checked them (`tree::validate`): a negative
    /// or NaN gap is not a gap. Those props now carry names, so the number
    /// arrives from here instead — and the check has to arrive with it, or
    /// nothing checks it at all. A theme is the one place the number is
    /// written, so a theme is where "a gap is a finite, non-negative extent"
    /// is proved. Layout can then resolve a name without a range arm, the
    /// same way it resolves one without a fallback arm.
    ///
    /// # Errors
    /// Returns [`ThemeError`] naming every missing token, every
    /// kind-mismatched token, and every unusable value — all of them, not
    /// just the first found.
    pub fn build(
        mode: ThemeMode,
        vocabulary: &Vocabulary,
        values: BTreeMap<TokenName, TokenValue>,
    ) -> Result<Self, ThemeError> {
        let mut missing = Vec::new();
        let mut mismatched = Vec::new();
        let mut unusable = Vec::new();
        for name in vocabulary.names() {
            let declared = vocabulary
                .kind_of(name)
                .expect("name was just yielded by vocabulary.names()");
            match values.get(name) {
                None => missing.push(name.clone()),
                Some(value) if value.kind() != declared => {
                    mismatched.push(ThemeMismatch {
                        name: name.clone(),
                        declared,
                        found: value.kind(),
                    });
                }
                Some(value) => {
                    if let Some(why) = unusable_extent(value) {
                        unusable.push(ThemeUnusable {
                            name: name.clone(),
                            why,
                        });
                    }
                }
            }
        }
        if missing.is_empty() && mismatched.is_empty() && unusable.is_empty() {
            Ok(Self { mode, values })
        } else {
            Err(ThemeError {
                mode,
                missing,
                mismatched,
                unusable,
            })
        }
    }

    /// This theme's mode.
    #[must_use]
    pub fn mode(&self) -> ThemeMode {
        self.mode
    }

    /// The value assigned to `name`, if this theme defines it.
    #[must_use]
    pub fn value(&self, name: &TokenName) -> Option<&TokenValue> {
        self.values.get(name)
    }

    /// All token → value assignments.
    #[must_use]
    pub fn values(&self) -> &BTreeMap<TokenName, TokenValue> {
        &self.values
    }
}

/// Why a value that is the right *kind* is still not a value layout can lay
/// anything out with, or `None` when it is fine.
///
/// Only the extent-valued kinds are checked, and only for the two properties
/// every consumer of an extent assumes: finite, and not negative. A colour
/// out of `[0, 1]` clips at paint time and a motion duration is not a
/// distance, so neither is this function's business.
fn unusable_extent(value: &TokenValue) -> Option<&'static str> {
    let extent = match value {
        TokenValue::Spacing(units) => *units,
        TokenValue::Shape(shape) => shape.corner_radius,
        _ => return None,
    };
    if !extent.is_finite() {
        Some("not a finite number")
    } else if extent < 0.0 {
        Some("negative")
    } else {
        None
    }
}

/// One token assigned a value of the right kind that layout still cannot
/// use: a negative gap, a NaN corner radius.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeUnusable {
    /// The token name.
    pub name: TokenName,
    /// What is wrong with the value, in one phrase.
    pub why: &'static str,
}

/// One token whose assigned value's kind disagrees with its vocabulary
/// declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeMismatch {
    /// The token name.
    pub name: TokenName,
    /// What the vocabulary declared.
    pub declared: TokenKind,
    /// What kind the assigned value actually carried.
    pub found: TokenKind,
}

/// A theme that does not define every vocabulary token, or defines one at
/// the wrong kind. [`Theme::build`] never hands back a `Theme` with this
/// problem silently patched over — a missing token stays missing until an
/// author fixes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeError {
    mode: ThemeMode,
    missing: Vec<TokenName>,
    mismatched: Vec<ThemeMismatch>,
    unusable: Vec<ThemeUnusable>,
}

impl ThemeError {
    /// Every token missing a value, in the order the vocabulary was
    /// iterated (its declaration order).
    #[must_use]
    pub fn missing(&self) -> &[TokenName] {
        &self.missing
    }

    /// Every token assigned a value of the wrong kind.
    #[must_use]
    pub fn mismatched(&self) -> &[ThemeMismatch] {
        &self.mismatched
    }

    /// Every token assigned an extent layout cannot use.
    #[must_use]
    pub fn unusable(&self) -> &[ThemeUnusable] {
        &self.unusable
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "petra: {} theme is incomplete:", self.mode.as_str())?;
        for name in &self.missing {
            writeln!(f, "  missing: {name}")?;
        }
        for mismatch in &self.mismatched {
            writeln!(
                f,
                "  {}: expected {:?}, found {:?}",
                mismatch.name, mismatch.declared, mismatch.found
            )?;
        }
        for bad in &self.unusable {
            writeln!(f, "  {}: extent is {}", bad.name, bad.why)?;
        }
        Ok(())
    }
}

impl std::error::Error for ThemeError {}

#[cfg(test)]
mod tests {
    use super::Theme;
    use crate::token::ThemeMode;
    use crate::token::name::TokenName;
    use crate::token::value::{ColorValue, TokenValue};
    use crate::token::vocabulary::{DesignToken, Vocabulary};
    use std::collections::BTreeMap;

    fn two_token_vocabulary() -> Vocabulary {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("surface.raised").unwrap(),
            crate::token::value::TokenKind::Color,
        ));
        vocab.declare(DesignToken::new(
            TokenName::new("text.muted").unwrap(),
            crate::token::value::TokenKind::Color,
        ));
        vocab
    }

    #[test]
    fn a_theme_missing_tokens_is_refused_and_names_every_one() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        // text.muted deliberately absent.
        let err = Theme::build(ThemeMode::Light, &vocab, values).unwrap_err();
        assert_eq!(err.missing(), &[TokenName::new("text.muted").unwrap()]);
        assert!(err.to_string().contains("text.muted"), "{err}");
    }

    #[test]
    fn a_theme_missing_every_token_names_every_one_not_just_the_first() {
        let vocab = two_token_vocabulary();
        let err = Theme::build(ThemeMode::Dark, &vocab, BTreeMap::new()).unwrap_err();
        assert_eq!(err.missing().len(), 2);
        let text = err.to_string();
        assert!(
            text.contains("surface.raised") && text.contains("text.muted"),
            "{text}"
        );
    }

    #[test]
    fn a_value_at_the_wrong_kind_is_refused() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Spacing(8.0),
        );
        values.insert(
            TokenName::new("text.muted").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        let err = Theme::build(ThemeMode::Light, &vocab, values).unwrap_err();
        assert!(err.missing().is_empty());
        assert_eq!(err.mismatched().len(), 1);
        assert_eq!(
            err.mismatched()[0].name,
            TokenName::new("surface.raised").unwrap()
        );
    }

    fn gap_vocabulary() -> Vocabulary {
        let mut vocab = Vocabulary::new();
        vocab.declare(DesignToken::new(
            TokenName::new("spacing.md").unwrap(),
            crate::token::value::TokenKind::Spacing,
        ));
        vocab.declare(DesignToken::new(
            TokenName::new("shape.corner-sm").unwrap(),
            crate::token::value::TokenKind::Shape,
        ));
        vocab
    }

    /// The check `tree::validate` used to run on `Props.spacing` and every
    /// edge of `Props.padding`, now run where the number is actually
    /// written.
    ///
    /// This is not defence in depth — it is the *only* place the number is
    /// checked any more, because a styling prop is a name and a name has no
    /// sign. A negative gap makes a stack lay its children on top of each
    /// other; a NaN one poisons every extent downstream of it.
    #[test]
    fn a_negative_or_non_finite_extent_is_refused_and_named() {
        for (bad, why) in [
            (TokenValue::Spacing(-1.0), "negative"),
            (TokenValue::Spacing(f32::NAN), "not a finite number"),
            (TokenValue::Spacing(f32::INFINITY), "not a finite number"),
        ] {
            let mut values = BTreeMap::new();
            values.insert(TokenName::new("spacing.md").unwrap(), bad);
            values.insert(
                TokenName::new("shape.corner-sm").unwrap(),
                TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 4.0 }),
            );
            let err = Theme::build(ThemeMode::Light, &gap_vocabulary(), values).unwrap_err();
            assert!(err.missing().is_empty(), "{err}");
            assert!(err.mismatched().is_empty(), "{err}");
            assert_eq!(err.unusable().len(), 1, "{bad:?} was accepted: {err}");
            assert_eq!(
                err.unusable()[0].name,
                TokenName::new("spacing.md").unwrap()
            );
            assert_eq!(err.unusable()[0].why, why);
            assert!(err.to_string().contains(why), "{err}");
        }
    }

    /// A corner radius is an extent too, and gets the same treatment — the
    /// rule is about what the number has to be, not about which prop reads
    /// it.
    #[test]
    fn an_unusable_corner_radius_is_refused_as_well() {
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("spacing.md").unwrap(),
            TokenValue::Spacing(12.0),
        );
        values.insert(
            TokenName::new("shape.corner-sm").unwrap(),
            TokenValue::Shape(crate::token::value::ShapeValue {
                corner_radius: -2.0,
            }),
        );
        let err = Theme::build(ThemeMode::Light, &gap_vocabulary(), values).unwrap_err();
        assert_eq!(err.unusable().len(), 1, "{err}");
        assert_eq!(
            err.unusable()[0].name,
            TokenName::new("shape.corner-sm").unwrap()
        );
    }

    /// Zero is a gap an author may want (a flush run of cells), so the floor
    /// is "not negative", not "positive".
    #[test]
    fn a_zero_extent_is_usable() {
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("spacing.md").unwrap(),
            TokenValue::Spacing(0.0),
        );
        values.insert(
            TokenName::new("shape.corner-sm").unwrap(),
            TokenValue::Shape(crate::token::value::ShapeValue { corner_radius: 0.0 }),
        );
        assert!(Theme::build(ThemeMode::Light, &gap_vocabulary(), values).is_ok());
    }

    #[test]
    fn a_complete_theme_is_accepted_and_answers_lookups() {
        let vocab = two_token_vocabulary();
        let mut values = BTreeMap::new();
        values.insert(
            TokenName::new("surface.raised").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        values.insert(
            TokenName::new("text.muted").unwrap(),
            TokenValue::Color(ColorValue::TRANSPARENT),
        );
        let theme = Theme::build(ThemeMode::Light, &vocab, values).unwrap();
        assert_eq!(theme.mode(), ThemeMode::Light);
        assert!(
            theme
                .value(&TokenName::new("surface.raised").unwrap())
                .is_some()
        );
        assert!(
            theme
                .value(&TokenName::new("status.unknown").unwrap())
                .is_none()
        );
    }
}
