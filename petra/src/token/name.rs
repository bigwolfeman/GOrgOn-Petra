//! Semantic token names.
//!
//! A token name identifies a *role* (`surface.raised`, `text.muted`,
//! `status.degraded`), never a value (`grey700`, `#3a3a3a`) — FR-013:
//! feature code references names, and a gate refuses a literal reaching a
//! token slot (`tree::validate::is_style_literal`, the sibling check on the
//! consumption side). This module is the production side of the same idea:
//! a name that could not survive that gate should never be constructible as
//! a `TokenName` in the first place.
//!
//! Two rules, both structural rather than a banned-word list:
//!
//! 1. the candidate must not *look like* a style literal — the same shapes
//!    `is_style_literal` refuses (hex colour, bare number, `rgb(...)` /
//!    `hsl(...)` function calls);
//! 2. the candidate must be namespaced: two or more role segments joined by
//!    `.`, `-`, or `_`, each segment plain lowercase letters. This is what
//!    rejects `grey700` and `red` without hand-maintaining a colour-word
//!    list, while still accepting `text-muted` — the hyphenated form
//!    `is_style_literal`'s own doc comment gives as a valid token name
//!    alongside `surface.raised`.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A validated, semantic design-token name.
///
/// Construction is the only way in ([`TokenName::new`]); there is no way to
/// hold a `TokenName` that failed the checks below.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct TokenName(String);

impl TokenName {
    /// Validate and wrap a candidate token name.
    ///
    /// # Errors
    /// Returns [`TokenNameError`] naming why the candidate was refused.
    pub fn new(candidate: impl Into<String>) -> Result<Self, TokenNameError> {
        let candidate = candidate.into();
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return Err(TokenNameError::Empty);
        }
        if looks_like_style_literal(trimmed) {
            return Err(TokenNameError::LooksLikeValue { candidate });
        }
        let segments: Vec<&str> = trimmed.split(['.', '-', '_']).collect();
        if segments.len() < 2 || segments.iter().any(|segment| !is_semantic_segment(segment)) {
            return Err(TokenNameError::NotNamespaced { candidate });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// The name's wire form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TokenName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for TokenName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::new(raw).map_err(serde::de::Error::custom)
    }
}

/// Why a candidate was refused as a token name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenNameError {
    /// The candidate was empty once trimmed.
    Empty,
    /// The candidate parses as a style value (hex, number, `rgb(...)`-style
    /// function), not a role.
    LooksLikeValue {
        /// The rejected candidate, unmodified.
        candidate: String,
    },
    /// The candidate has no `role.detail` (or `role-detail`) namespacing, or
    /// a segment carries something other than plain lowercase letters.
    NotNamespaced {
        /// The rejected candidate, unmodified.
        candidate: String,
    },
}

impl fmt::Display for TokenNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "token name is empty"),
            Self::LooksLikeValue { candidate } => write!(
                f,
                "{candidate:?} looks like a style value, not a token name; tokens are role-named (e.g. `surface.raised`), never value-named"
            ),
            Self::NotNamespaced { candidate } => write!(
                f,
                "{candidate:?} is not namespaced; a token name is two or more role segments (e.g. `surface.raised`, `text-muted`), not a bare word or shade"
            ),
        }
    }
}

impl std::error::Error for TokenNameError {}

/// Whether `candidate` parses as one of the literal shapes
/// `tree::validate::is_style_literal` refuses: a hex colour, a bare number,
/// or an `rgb(...)`/`hsl(...)`-style function call. Kept in step with that
/// function by construction (both read the same shapes) even though the two
/// live in different modules owned by different agents in this wave.
fn looks_like_style_literal(trimmed: &str) -> bool {
    if trimmed.starts_with('#') {
        return true;
    }
    if trimmed.parse::<f64>().is_ok() {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    ["rgb(", "rgba(", "hsl(", "hsla("]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// A semantic name segment: one or more plain lowercase ASCII letters.
///
/// Excluding digits is deliberate, not incidental: `grey700`'s digits are
/// exactly what makes it a value (a shade index) rather than a role.
fn is_semantic_segment(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{TokenName, TokenNameError};

    #[test]
    fn semantic_dotted_and_hyphenated_names_are_accepted() {
        assert!(TokenName::new("surface.raised").is_ok());
        assert!(TokenName::new("text-muted").is_ok());
        assert!(TokenName::new("status.degraded").is_ok());
    }

    #[test]
    fn value_named_tokens_are_refused() {
        assert_eq!(
            TokenName::new("grey700").unwrap_err(),
            TokenNameError::NotNamespaced {
                candidate: "grey700".into()
            }
        );
        assert_eq!(
            TokenName::new("red").unwrap_err(),
            TokenNameError::NotNamespaced {
                candidate: "red".into()
            }
        );
    }

    #[test]
    fn literal_style_values_are_refused_even_when_namespaced_in_shape() {
        assert_eq!(
            TokenName::new("#3a3a3a").unwrap_err(),
            TokenNameError::LooksLikeValue {
                candidate: "#3a3a3a".into()
            }
        );
        assert_eq!(
            TokenName::new("16").unwrap_err(),
            TokenNameError::LooksLikeValue {
                candidate: "16".into()
            }
        );
        assert_eq!(
            TokenName::new("rgba(0,0,0,1)").unwrap_err(),
            TokenNameError::LooksLikeValue {
                candidate: "rgba(0,0,0,1)".into()
            }
        );
    }

    #[test]
    fn empty_and_whitespace_only_names_are_refused() {
        assert_eq!(TokenName::new("").unwrap_err(), TokenNameError::Empty);
        assert_eq!(TokenName::new("   ").unwrap_err(), TokenNameError::Empty);
    }

    #[test]
    fn display_prints_the_wire_form() {
        let name = TokenName::new("surface.raised").unwrap();
        assert_eq!(name.to_string(), "surface.raised");
        assert_eq!(name.as_str(), "surface.raised");
    }

    #[test]
    fn json_round_trips_and_rejects_a_literal_on_the_way_in() {
        let name = TokenName::new("text.muted").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"text.muted\"");
        let back: TokenName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, name);

        let err = serde_json::from_str::<TokenName>("\"#fff\"").unwrap_err();
        assert!(
            err.to_string().contains("looks like a style value"),
            "{err}"
        );
    }
}
