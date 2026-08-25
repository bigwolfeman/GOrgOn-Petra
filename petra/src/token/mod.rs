//! Design tokens, themes, and immutable theme snapshots.
//!
//! Feature code references token *names*; a theme resolves names to values,
//! and exactly one immutable snapshot is in force for any one frame (FR-013,
//! FR-014, FR-016). The status subset pairs every colour with a shape or text
//! channel, so no state is conveyed by colour alone (FR-015); [`focus`] is the
//! same rule applied to keyboard focus, which is a status too.

use serde::{Deserialize, Serialize};

pub mod focus;
pub mod name;
pub mod presenter;
pub mod selection;
pub mod shipped;
pub mod slot;
pub mod snapshot;
pub mod status;
pub mod theme;
pub mod value;
pub mod vocabulary;

pub use focus::{FocusBand, FocusRing};
pub use name::{TokenName, TokenNameError};
pub use presenter::Presenter;
pub use selection::ThemeSelection;
pub use shipped::{dark, light, standard_vocabulary};
pub use slot::{SlotSchema, SlotSpec, standard_slots};
pub use snapshot::ThemeSnapshot;
pub use status::{StatusShape, StatusToken, StatusTokenError};
pub use theme::{Theme, ThemeError, ThemeMismatch, ThemeUnusable};
pub use value::{
    ColorValue, MotionEasing, MotionValue, ShapeValue, Silhouette, SpringValue, TokenKind,
    TokenValue, TypographyValue, TypographyWeight,
};
pub use vocabulary::{DesignToken, Vocabulary};

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
