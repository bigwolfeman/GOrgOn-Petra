//! Status tokens: the FR-015 pairing.
//!
//! The user of this system is red-green colourblind, and Petra's status
//! vocabulary is written for that constraint rather than around it: no
//! Petra-provided status element may carry meaning by colour alone, so
//! every status entry pairs its colour with a [`StatusShape`] (a
//! distinguishable outline, not just a colour swatch) and a text label.
//! [`StatusToken::new`] takes all three as required, non-optional
//! constructor arguments — there is no builder, no `Default`, and no
//! setter, so a `StatusToken` value missing its shape or its text cannot be
//! named: the type has no such state to be in.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::token::name::TokenName;

/// A shape channel for a status, chosen to stay distinguishable in outline
/// alone (silhouette, not fill colour) so it survives colourblindness,
/// greyscale printing, and small sizes alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StatusShape {
    /// A filled circle.
    Circle,
    /// A triangle carrying an exclamation mark.
    Triangle,
    /// A square carrying a cross.
    Square,
    /// A diamond.
    Diamond,
}

/// One status's colour, shape, and text, always constructed together
/// (FR-015). `name` is the same [`TokenName`] used to look up the status's
/// colour value in a resolved theme (`Theme::value`); shape and text do not
/// vary by theme mode, only the colour painted into the shape does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusToken {
    name: TokenName,
    shape: StatusShape,
    text: String,
}

impl StatusToken {
    /// Pair a status name with its shape and text channels.
    ///
    /// # Errors
    /// Returns [`StatusTokenError::EmptyText`] when `text` is empty or
    /// whitespace-only: a colour-plus-shape-but-no-label status is still a
    /// status a screen reader cannot announce, so it is refused the same as
    /// a missing shape would be if the type allowed one.
    pub fn new(
        name: TokenName,
        shape: StatusShape,
        text: impl Into<String>,
    ) -> Result<Self, StatusTokenError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(StatusTokenError::EmptyText { name });
        }
        Ok(Self { name, shape, text })
    }

    /// The status's token name (shared with its colour token in a theme).
    #[must_use]
    pub fn name(&self) -> &TokenName {
        &self.name
    }

    /// The status's shape channel.
    #[must_use]
    pub fn shape(&self) -> StatusShape {
        self.shape
    }

    /// The status's text channel.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Why a [`StatusToken`] was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusTokenError {
    /// The text channel was empty or whitespace-only.
    EmptyText {
        /// The status name the empty text was attached to.
        name: TokenName,
    },
}

impl fmt::Display for StatusTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyText { name } => write!(
                f,
                "status token {name} has no text channel; FR-015 requires colour, shape, and text together"
            ),
        }
    }
}

impl std::error::Error for StatusTokenError {}

#[cfg(test)]
mod tests {
    use super::{StatusShape, StatusToken, StatusTokenError};
    use crate::token::name::TokenName;

    #[test]
    fn a_status_token_carries_its_shape_and_text() {
        let status = StatusToken::new(
            TokenName::new("status.degraded").unwrap(),
            StatusShape::Triangle,
            "Degraded",
        )
        .unwrap();
        assert_eq!(status.shape(), StatusShape::Triangle);
        assert_eq!(status.text(), "Degraded");
        assert_eq!(status.name().as_str(), "status.degraded");
    }

    #[test]
    fn empty_text_is_refused_because_a_status_needs_a_text_channel_too() {
        let err = StatusToken::new(
            TokenName::new("status.down").unwrap(),
            StatusShape::Square,
            "   ",
        )
        .unwrap_err();
        assert_eq!(
            err,
            StatusTokenError::EmptyText {
                name: TokenName::new("status.down").unwrap()
            }
        );
        assert!(
            err.to_string().contains("colour, shape, and text together"),
            "{err}"
        );
    }

    // There is no test for "a status token without a shape" or "without a
    // text field": `StatusToken::new`'s signature is
    // `(TokenName, StatusShape, impl Into<String>)` with no `Option`, no
    // `Default` derive, and no setters — the compiler refuses any call site
    // missing an argument before a test could ever run. The struct's private
    // fields mean the only way to build one at all is through `new`.
}
