//! The declared token vocabulary a [`Theme`](crate::token::theme::Theme) is
//! checked complete against.

use std::collections::BTreeMap;

use crate::token::name::TokenName;
use crate::token::status::StatusToken;
use crate::token::theme::Theme;
use crate::token::value::TokenKind;

/// One entry in the vocabulary: a semantic name and the value shape it must
/// resolve to in every theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignToken {
    name: TokenName,
    kind: TokenKind,
}

impl DesignToken {
    /// Declare a token by name and value shape.
    #[must_use]
    pub fn new(name: TokenName, kind: TokenKind) -> Self {
        Self { name, kind }
    }

    /// The token's name.
    #[must_use]
    pub fn name(&self) -> &TokenName {
        &self.name
    }

    /// The value shape every theme must assign this name.
    #[must_use]
    pub fn kind(&self) -> TokenKind {
        self.kind
    }
}

/// The set of tokens a [`Theme`](crate::token::theme::Theme) must define,
/// in full, to be accepted. Includes the status subset (FR-015): declaring
/// a status via [`Vocabulary::declare_status`] both adds its colour token
/// to the ordinary completeness check and records its shape/text pairing
/// for [`Vocabulary::status`].
#[derive(Clone, Debug, Default)]
pub struct Vocabulary {
    tokens: BTreeMap<TokenName, TokenKind>,
    statuses: BTreeMap<TokenName, StatusToken>,
}

impl Vocabulary {
    /// An empty vocabulary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare a token. A later call under the same name overwrites the
    /// earlier declaration — vocabularies are authored once, in
    /// [`crate::token::shipped::standard_vocabulary`], and this exists for
    /// that authoring and for tests, not for runtime churn.
    pub fn declare(&mut self, token: DesignToken) -> &mut Self {
        self.tokens.insert(token.name, token.kind);
        self
    }

    /// Declare a status: its colour token (kind [`TokenKind::Color`]) joins
    /// the ordinary completeness check, and its shape/text pairing becomes
    /// queryable through [`Vocabulary::status`].
    pub fn declare_status(&mut self, status: StatusToken) -> &mut Self {
        self.tokens.insert(status.name().clone(), TokenKind::Color);
        self.statuses.insert(status.name().clone(), status);
        self
    }

    /// Whether `name` is declared.
    #[must_use]
    pub fn contains(&self, name: &TokenName) -> bool {
        self.tokens.contains_key(name)
    }

    /// The declared value shape for `name`, if declared.
    #[must_use]
    pub fn kind_of(&self, name: &TokenName) -> Option<TokenKind> {
        self.tokens.get(name).copied()
    }

    /// The status entry declared under `name`, if any.
    #[must_use]
    pub fn status(&self, name: &TokenName) -> Option<&StatusToken> {
        self.statuses.get(name)
    }

    /// All declared names, in a stable (sorted) order.
    pub fn names(&self) -> impl Iterator<Item = &TokenName> {
        self.tokens.keys()
    }

    /// Declared names of exactly `kind`, in a stable (sorted) order.
    ///
    /// This is the "legal set" `tree::validate::Violation::UnknownTokenRef`
    /// and `TokenKindMismatch` (FR-056) name in their message: the answer to
    /// "what could this prop have said instead" is never the whole
    /// vocabulary, because a spacing prop was never going to accept a colour
    /// name — it is the subset that shares the slot's own shape.
    #[must_use]
    pub fn names_of_kind(&self, kind: TokenKind) -> Vec<&TokenName> {
        self.tokens
            .iter()
            .filter(move |(_, k)| **k == kind)
            .map(|(name, _)| name)
            .collect()
    }

    /// All declared status entries, in a stable (sorted-by-name) order.
    pub fn statuses(&self) -> impl Iterator<Item = &StatusToken> {
        self.statuses.values()
    }

    /// How many tokens are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// The vocabulary `theme` was proved complete against, reconstructed
    /// from what it actually assigns.
    ///
    /// [`Theme::build`] already proves `theme` assigns a value, at a
    /// consistent kind, to every name in *some* vocabulary — this is that
    /// vocabulary, read back off the theme rather than kept as a second,
    /// hand-written copy that could drift from it. The motivating caller is
    /// a host wiring up a [`crate::tree::validate::Registry`] from the theme
    /// it already has: the registry that accepts a tree and the theme that
    /// resolves it should start in agreement about what exists, and this is
    /// how they do, with nothing to keep in sync by hand.
    #[must_use]
    pub fn from_theme(theme: &Theme) -> Self {
        let mut vocab = Self::new();
        for (name, value) in theme.values() {
            vocab.declare(DesignToken::new(name.clone(), value.kind()));
        }
        vocab
    }

    /// Whether no tokens are declared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{DesignToken, Vocabulary};
    use crate::token::name::TokenName;
    use crate::token::status::{StatusShape, StatusToken};
    use crate::token::value::TokenKind;

    #[test]
    fn declaring_a_status_also_declares_its_color_token() {
        let mut vocab = Vocabulary::new();
        let name = TokenName::new("status.ok").unwrap();
        vocab.declare_status(StatusToken::new(name.clone(), StatusShape::Circle, "OK").unwrap());
        assert!(vocab.contains(&name));
        assert_eq!(vocab.kind_of(&name), Some(TokenKind::Color));
        assert_eq!(vocab.status(&name).unwrap().text(), "OK");
    }

    #[test]
    fn a_plain_token_has_no_status_entry() {
        let mut vocab = Vocabulary::new();
        let name = TokenName::new("surface.raised").unwrap();
        vocab.declare(DesignToken::new(name.clone(), TokenKind::Color));
        assert!(vocab.contains(&name));
        assert!(vocab.status(&name).is_none());
    }

    /// The legal set for one kind excludes every other kind, and comes back
    /// sorted — the same order `names()` already promises.
    #[test]
    fn names_of_kind_is_scoped_and_sorted() {
        let mut vocab = Vocabulary::new();
        vocab
            .declare(DesignToken::new(
                TokenName::new("spacing.md").unwrap(),
                TokenKind::Spacing,
            ))
            .declare(DesignToken::new(
                TokenName::new("spacing.2xs").unwrap(),
                TokenKind::Spacing,
            ))
            .declare(DesignToken::new(
                TokenName::new("surface.raised").unwrap(),
                TokenKind::Color,
            ));
        let spacing = vocab.names_of_kind(TokenKind::Spacing);
        assert_eq!(
            spacing,
            vec![
                &TokenName::new("spacing.2xs").unwrap(),
                &TokenName::new("spacing.md").unwrap(),
            ],
            "a colour token must not leak into the spacing legal set"
        );
        assert_eq!(vocab.names_of_kind(TokenKind::Motion).len(), 0);
    }

    /// The vocabulary read back off a theme accepts exactly the trees the
    /// vocabulary that built the theme would have — same names, same kinds
    /// — which is the round trip a host relies on when it wires a
    /// `Registry` from a theme it already has.
    #[test]
    fn from_theme_reconstructs_the_vocabulary_the_theme_was_built_against() {
        let original = crate::token::standard_vocabulary();
        let theme = crate::token::Theme::build(
            crate::token::ThemeMode::Light,
            &original,
            crate::token::light().values().clone(),
        )
        .expect("the shipped light theme is complete");

        let reconstructed = Vocabulary::from_theme(&theme);
        assert_eq!(reconstructed.len(), original.len());
        for name in original.names() {
            assert_eq!(
                reconstructed.kind_of(name),
                original.kind_of(name),
                "{name} must round-trip at the same kind"
            );
        }
    }
}
