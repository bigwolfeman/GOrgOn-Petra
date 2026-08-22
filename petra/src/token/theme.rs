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
    /// `vocabulary` declares, each at its declared kind.
    ///
    /// # Errors
    /// Returns [`ThemeError`] naming every missing token and every
    /// kind-mismatched token, not just the first found.
    pub fn build(
        mode: ThemeMode,
        vocabulary: &Vocabulary,
        values: BTreeMap<TokenName, TokenValue>,
    ) -> Result<Self, ThemeError> {
        let mut missing = Vec::new();
        let mut mismatched = Vec::new();
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
                Some(_) => {}
            }
        }
        if missing.is_empty() && mismatched.is_empty() {
            Ok(Self { mode, values })
        } else {
            Err(ThemeError {
                mode,
                missing,
                mismatched,
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
