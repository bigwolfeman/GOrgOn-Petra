//! Immutable, revisioned theme snapshots — what a frame actually reads.

use crate::token::ThemeMode;
use crate::token::name::TokenName;
use crate::token::theme::Theme;
use crate::token::value::{Silhouette, TokenValue};

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

    /// The gap `name` resolves to, in logical units.
    ///
    /// `None` means one of two things, and they are the same thing to a
    /// caller: the snapshot does not define `name`, or it defines it as
    /// something that is not a gap. Both are authoring bugs a validated tree
    /// cannot reach — [`crate::token::Theme::build`] proves a theme assigns
    /// every vocabulary name at its declared kind — so no caller needs an
    /// arm per case.
    ///
    /// This exists so that no container ever writes
    /// `match snapshot.value(n) { Some(TokenValue::Spacing(v)) => …, … }`
    /// itself. That `match` has a wrong answer available (treat a
    /// non-spacing token as a zero gap) and would be written once per
    /// container; here it is written once, and the wrong answer is not
    /// spelled at all.
    #[must_use]
    pub fn spacing(&self, name: &TokenName) -> Option<f32> {
        match self.value(name)? {
            TokenValue::Spacing(units) => Some(*units),
            _ => None,
        }
    }

    /// The corner radius `name` resolves to, in logical units. The shape
    /// sibling of [`ThemeSnapshot::spacing`], and `None` for the same two
    /// reasons.
    #[must_use]
    pub fn corner(&self, name: &TokenName) -> Option<f32> {
        match self.value(name)? {
            TokenValue::Shape(shape) => Some(shape.corner_radius),
            _ => None,
        }
    }

    /// The outline family `name` resolves to. `None` when the theme has no
    /// such token, and `None` — never a silent [`Silhouette::Rect`] — when
    /// it has one at another kind, for the same reason
    /// [`ThemeSnapshot::corner`] refuses to answer for a colour: a caller
    /// that cannot tell "no such figure" from "the default figure" cannot
    /// report the miss, and an unreported miss is how FR-015's shape
    /// channel went missing the first time.
    #[must_use]
    pub fn silhouette(&self, name: &TokenName) -> Option<Silhouette> {
        match self.value(name)? {
            TokenValue::Silhouette(figure) => Some(*figure),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ThemeSnapshot;
    use crate::token::name::TokenName;
    use crate::token::shipped::light;

    fn n(s: &str) -> TokenName {
        TokenName::new(s).expect("test names are well-formed")
    }

    /// The typed accessors answer at their own kind and refuse every other
    /// kind, rather than coercing. `surface.base` is a real, defined token —
    /// so a `None` here is the accessor discriminating on kind, not on
    /// existence, which is the property that lets containers drop their own
    /// `match`.
    #[test]
    fn a_typed_accessor_answers_only_at_its_own_kind() {
        let snapshot = ThemeSnapshot::new(light(), 1);
        assert_eq!(snapshot.spacing(&n("spacing.md")), Some(12.0));
        assert_eq!(snapshot.corner(&n("shape.corner-lg")), Some(12.0));

        assert!(snapshot.value(&n("surface.base")).is_some());
        assert_eq!(snapshot.spacing(&n("surface.base")), None);
        assert_eq!(snapshot.corner(&n("surface.base")), None);
        assert_eq!(snapshot.spacing(&n("shape.corner-lg")), None);
        assert_eq!(snapshot.corner(&n("spacing.md")), None);

        assert_eq!(snapshot.spacing(&n("not.a.token")), None);
    }

    /// Every step of the shipped ramp is reachable through the typed
    /// accessor, which is the read path every styling prop now takes.
    #[test]
    fn every_ramp_step_resolves_through_the_typed_accessor() {
        let snapshot = ThemeSnapshot::new(light(), 1);
        for step in [
            "spacing.2xs",
            "spacing.xs",
            "spacing.sm",
            "spacing.md",
            "spacing.lg",
            "spacing.xl",
            "spacing.2xl",
            "spacing.3xl",
        ] {
            assert!(
                snapshot.spacing(&n(step)).is_some_and(|v| v > 0.0),
                "{step} does not resolve to a positive gap"
            );
        }
    }

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
