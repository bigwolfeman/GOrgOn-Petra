//! Immutable, revisioned theme snapshots — what a frame actually reads.

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::theme::Theme;
use crate::token::value::TokenValue;

/// An immutable resolution of one [`Theme`] at one revision.
///
/// Every field is private and there is no `&mut self` method anywhere on
/// this type: once built, a `ThemeSnapshot` cannot change under a holder's
/// feet. `revision` is the same number [`crate::frame::Viewport::theme_rev`]
/// and `PaintState::token_revision` carry — the digest hashes it
/// (`contracts/frame-identity.md`), so two frames over an identical tree at
/// two different revisions are, correctly, two different frames.
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSnapshot {
    theme: Theme,
    revision: u64,
}

impl ThemeSnapshot {
    /// Wrap `theme` as the snapshot at `revision`. Public so tests and a
    /// presenter's first snapshot can construct one directly; ordinary
    /// publication after the first goes through
    /// [`crate::token::presenter::Presenter::publish`], which is what
    /// guarantees the monotone, always-changing revision sequence.
    #[must_use]
    pub fn new(theme: Theme, revision: u64) -> Self {
        Self { theme, revision }
    }

    /// The resolved theme this snapshot carries.
    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// This snapshot's revision.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// This snapshot's resolved mode.
    #[must_use]
    pub fn mode(&self) -> ThemeMode {
        self.theme.mode()
    }

    /// The value assigned to `name` in this snapshot, if defined.
    #[must_use]
    pub fn value(&self, name: &TokenName) -> Option<&TokenValue> {
        self.theme.value(name)
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeSnapshot;
    use crate::token::shipped::light;

    #[test]
    fn a_snapshot_exposes_its_theme_through_read_only_accessors() {
        let snapshot = ThemeSnapshot::new(light(), 3);
        assert_eq!(snapshot.revision(), 3);
        assert_eq!(snapshot.mode(), snapshot.theme().mode());
        assert_eq!(
            snapshot.value(&crate::token::name::TokenName::new("surface.base").unwrap()),
            snapshot
                .theme()
                .value(&crate::token::name::TokenName::new("surface.base").unwrap())
        );
    }
}
