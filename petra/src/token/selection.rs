//! Theme selection — what was asked for, resolved to exactly one mode
//! (FR-014).

use crate::token::ThemeMode;

/// What the user, or the host platform, asked for.
///
/// Distinct from [`ThemeMode`]: `ThemeMode` is the *resolved outcome* a
/// frame renders under, while `ThemeSelection` is the *request*, and
/// `System` is not itself a mode — it is a rule for picking one from
/// whatever the platform currently reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeSelection {
    /// Always light, regardless of the platform.
    Light,
    /// Always dark, regardless of the platform.
    Dark,
    /// Track the host platform's reported mode.
    #[default]
    System,
}

impl ThemeSelection {
    /// Resolve this selection to exactly one mode, given what the host
    /// platform currently reports. `system_mode` is read only for
    /// `Self::System`; `Light`/`Dark` ignore it, which is what makes them
    /// an override rather than a hint.
    #[must_use]
    pub fn resolve(self, system_mode: ThemeMode) -> ThemeMode {
        match self {
            Self::Light => ThemeMode::Light,
            Self::Dark => ThemeMode::Dark,
            Self::System => system_mode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeSelection;
    use crate::token::ThemeMode;

    #[test]
    fn explicit_selections_ignore_what_the_platform_reports() {
        assert_eq!(
            ThemeSelection::Light.resolve(ThemeMode::Dark),
            ThemeMode::Light
        );
        assert_eq!(
            ThemeSelection::Dark.resolve(ThemeMode::Light),
            ThemeMode::Dark
        );
    }

    #[test]
    fn system_selection_follows_the_platform() {
        assert_eq!(
            ThemeSelection::System.resolve(ThemeMode::Light),
            ThemeMode::Light
        );
        assert_eq!(
            ThemeSelection::System.resolve(ThemeMode::Dark),
            ThemeMode::Dark
        );
    }

    #[test]
    fn default_selection_is_system() {
        assert_eq!(ThemeSelection::default(), ThemeSelection::System);
    }
}
