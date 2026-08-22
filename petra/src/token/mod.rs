//! Design tokens, themes, and immutable theme snapshots.
//!
//! Feature code references token *names*; a theme resolves names to values,
//! and exactly one immutable snapshot is in force for any one frame (FR-013,
//! FR-014, FR-016). The status subset pairs every colour with a shape or text
//! channel, so no state is conveyed by colour alone (FR-015).

use serde::{Deserialize, Serialize};

/// Resolved light or dark mode. A digest input: the same tree under two modes
/// is two frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Light mode.
    Light,
    /// Dark mode.
    #[default]
    Dark,
}

impl ThemeMode {
    /// The mode's wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeMode;

    #[test]
    fn mode_names_are_stable() {
        assert_eq!(ThemeMode::Light.as_str(), "light");
        assert_eq!(ThemeMode::Dark.as_str(), "dark");
    }
}
