//! Status tokens: the FR-015 pairing.
//!
//! The user of this system is red-green colourblind, and Petra's status
//! vocabulary is written for that constraint rather than around it: no
//! Petra-provided status element may carry meaning by colour alone, so
//! every status entry pairs its colour with a [`StatusShape`] (a
//! distinguishable outline, not just a colour swatch) and a text label.
//! [`StatusToken::new`] takes all three as required, non-optional
//! constructor arguments — there is no builder, no `Default`, and no setter.
//!
//! A constructor is not by itself a guarantee, and this module learned that
//! the hard way: it originally derived [`Deserialize`], which builds a struct
//! field by field and never calls `new`, so `{"text": "   "}` produced exactly
//! the colour-and-shape-but-no-label status the constructor refuses — while
//! three comments in this file said that state was unrepresentable. The
//! deserializer below is hand-written and funnels through `new`, the same way
//! [`crate::token::name::TokenName`] does, so *every* way into the type
//! enforces FR-015 and not just the one a Rust caller happens to use.

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
    /// A regular octagon. The shipped Down marker: a stop-sign, not a box.
    Octagon,
}

/// One status's colour, shape, and text, always constructed together
/// (FR-015). `name` is the same [`TokenName`] used to look up the status's
/// colour value in a resolved theme (`Theme::value`); shape and text do not
/// vary by theme mode, only the colour painted into the shape does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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

impl<'de> Deserialize<'de> for StatusToken {
    /// Deserialize through [`StatusToken::new`], so a serialized status that
    /// omits its text channel is refused on the way in rather than becoming a
    /// value the rest of the engine trusts.
    ///
    /// The private `Raw` mirror exists only to borrow the derive's field
    /// parsing; it is never handed out. `deny_unknown_fields` is on it for the
    /// same reason it is on the tree types: a misspelled `shpae` should be an
    /// error an author sees, not a silent fall back to some default — and
    /// there is no default here to fall back to.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            name: TokenName,
            shape: StatusShape,
            text: String,
        }
        let raw = Raw::deserialize(deserializer)?;
        Self::new(raw.name, raw.shape, raw.text).map_err(serde::de::Error::custom)
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

    // The Rust call path is closed by the compiler: `StatusToken::new`'s
    // signature is `(TokenName, StatusShape, impl Into<String>)` with no
    // `Option`, no `Default` derive, and no setters, and the fields are
    // private. The *serde* path is not closed by the compiler, which is why
    // these two tests exist — an earlier version of this file derived
    // `Deserialize` and let exactly this value through while the module doc
    // said it could not exist.

    #[test]
    fn deserializing_a_status_with_a_blank_text_channel_is_refused() {
        let err = serde_json::from_str::<StatusToken>(
            r#"{"name":"status.down","shape":"square","text":"   "}"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("colour, shape, and text together"),
            "the serde path must fail with the FR-015 reason, not a type \
             error: {err}"
        );
        let err = serde_json::from_str::<StatusToken>(
            r#"{"name":"status.down","shape":"square","text":""}"#,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("colour, shape, and text together"),
            "{err}"
        );
    }

    #[test]
    fn a_well_formed_status_still_round_trips_through_serde() {
        let status = StatusToken::new(
            TokenName::new("status.ok").unwrap(),
            StatusShape::Circle,
            "OK",
        )
        .unwrap();
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(serde_json::from_str::<StatusToken>(&json).unwrap(), status);
    }

    #[test]
    fn a_misspelled_field_is_refused_rather_than_defaulted() {
        let err = serde_json::from_str::<StatusToken>(
            r#"{"name":"status.ok","shpae":"circle","text":"OK"}"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("shpae"), "{err}");
    }
}
