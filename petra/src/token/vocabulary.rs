//! The declared token vocabulary a [`Theme`](crate::token::theme::Theme) is
//! checked complete against.

use std::collections::BTreeMap;

use crate::token::name::TokenName;
use crate::token::status::StatusToken;
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

    /// All declared status entries, in a stable (sorted-by-name) order.
    pub fn statuses(&self) -> impl Iterator<Item = &StatusToken> {
        self.statuses.values()
    }

    /// How many tokens are declared.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tokens.len()
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
}
