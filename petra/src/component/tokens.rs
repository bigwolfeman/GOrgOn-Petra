//! The token names the component library binds, gathered in one place.
//!
//! FR-052's vocabulary sits on top of [`crate::token::shipped::standard_vocabulary`],
//! and the two are decided by different people at different times: the ramp
//! is the design system's call, the component signatures are this module's.
//! Hardcoding a literal like `"spacing-03"` at each of a dozen call sites
//! means a ramp rename becomes a sweep; naming it once here and having every
//! component read the constant means it becomes a one-line edit — which
//! matters concretely in this build, because the ramp was still being
//! extended in a sibling leaf while this module was written.
//!
//! [`ALL`] is also the fixture [`super::tests::every_bound_token_is_in_the_standard_vocabulary`]
//! walks (gate C1-7): the test proves the constants below are real tokens,
//! not that any particular component call site used one, so it keeps
//! covering the whole library as components are added.

use crate::token::TokenName;

/// The five gaps the component library spends, on Carbon's ordinal ramp.
///
/// These moved off the `spacing.2xs … spacing.lg` t-shirt names on
/// 2026-08-25, when the ramp grew from eight steps to Carbon's thirteen and
/// the t-shirt names became documented aliases
/// (`token::shipped::SPACING_ALIASES`). **No gap changed size** — the eight
/// old values are Carbon's `spacing-01 … spacing-09` verbatim, so this is a
/// rename of the name and not of the number.
///
/// This module is the reason that migration was a five-line edit rather than
/// a sweep, which is the claim its own doc comment above makes and this is
/// the first change to test it.
pub(crate) const SPACING_01: &str = "spacing-01";
/// See [`SPACING_01`]. 4 units.
pub(crate) const SPACING_02: &str = "spacing-02";
/// See [`SPACING_01`]. 8 units.
pub(crate) const SPACING_03: &str = "spacing-03";
/// See [`SPACING_01`]. 12 units.
pub(crate) const SPACING_04: &str = "spacing-04";
/// See [`SPACING_01`]. 16 units.
pub(crate) const SPACING_05: &str = "spacing-05";

pub(crate) const SHAPE_NONE: &str = "shape.corner-none";
pub(crate) const SHAPE_SM: &str = "shape.corner-sm";
pub(crate) const SHAPE_MD: &str = "shape.corner-md";
pub(crate) const SHAPE_FULL: &str = "shape.corner-full";

pub(crate) const SILHOUETTE_RECT: &str = "shape.silhouette-rect";
pub(crate) const SILHOUETTE_TRIANGLE: &str = "shape.silhouette-triangle";
pub(crate) const SILHOUETTE_DIAMOND: &str = "shape.silhouette-diamond";

pub(crate) const TYPOGRAPHY_BODY: &str = "typography.body";
pub(crate) const TYPOGRAPHY_HEADING: &str = "typography.heading";

/// The elevation a resting container casts. Colour only: the offset, blur
/// and spread live in `token::SHADOW_GEOMETRY`, because they are the same in
/// both themes and a theme is the wrong place to keep a fact that does not
/// change with the lights.
pub(crate) const SHADOW_RAISED: &str = "shadow.raised";

pub(crate) const SURFACE_BASE: &str = "surface.base";
pub(crate) const SURFACE_RAISED: &str = "surface.raised";
pub(crate) const TEXT_PRIMARY: &str = "text.primary";
pub(crate) const TEXT_MUTED: &str = "text.muted";

/// The one colour a drawn boundary is allowed to be.
///
/// Every border in this library bound [`TEXT_MUTED`] until 2026-08-25 —
/// 10.73:1 against the card it was drawn on, as loud as the prose inside
/// it, because there was no border colour in the theme and a text one had
/// been conscripted. Most of those borders are now gone entirely; the ones
/// that survive are the ones where the edge *is* the control (an unchecked
/// checkbox is nothing but its outline), and they bind this.
pub(crate) const BORDER_SUBTLE: &str = "border.subtle";

/// The accent fill, and the ink that goes on top of it.
///
/// Spent by [`super::primary_button`] and nothing else in this library. See
/// `crate::token::shipped`'s own comments for why the hue is blue (it is the
/// axis red-green colour blindness does not collapse) and why the pair is
/// two names rather than one (neither shipped text tone clears AA on it).
pub(crate) const ACCENT_PRIMARY: &str = "accent.primary";
/// See [`ACCENT_PRIMARY`].
pub(crate) const TEXT_ON_ACCENT: &str = "text.on-accent";

/// Every constant above, for the completeness proof. A name added above and
/// left out of this list would silently stop being covered, so the list is
/// what the test walks rather than the component source.
///
/// `#[cfg(test)]`: nothing in the shipped library reads this back, only the
/// completeness test below does, so a non-test build correctly reports it
/// unused without this — the gate is a test's job, not runtime code's.
#[cfg(test)]
pub(crate) const ALL: &[&str] = &[
    SPACING_01,
    SPACING_02,
    SPACING_03,
    SPACING_04,
    SPACING_05,
    SHAPE_NONE,
    SHAPE_SM,
    SHAPE_MD,
    SHAPE_FULL,
    SILHOUETTE_RECT,
    SILHOUETTE_TRIANGLE,
    SILHOUETTE_DIAMOND,
    TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING,
    SHADOW_RAISED,
    SURFACE_BASE,
    SURFACE_RAISED,
    TEXT_PRIMARY,
    TEXT_MUTED,
    BORDER_SUBTLE,
    ACCENT_PRIMARY,
    TEXT_ON_ACCENT,
];

/// `name` as a [`TokenName`].
///
/// # Panics
/// Never, for any constant declared in this file — each one is checked
/// well-formed by `every_constant_is_a_well_formed_token_name` below. A
/// panic here means a constant above was mistyped, not that a caller passed
/// something bad: every call site in `component/` passes one of the named
/// constants, never a computed or author-supplied string (gate C1-8).
pub(crate) fn t(name: &str) -> TokenName {
    TokenName::new(name).unwrap_or_else(|err| panic!("component token name {name:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{ALL, t};
    use crate::token::standard_vocabulary;

    #[test]
    fn every_constant_is_a_well_formed_token_name() {
        for name in ALL {
            let _ = t(name);
        }
    }

    /// Gate C1-7: every token the component library binds is declared in
    /// the shipped vocabulary. Proved by walking [`ALL`] — the list the
    /// components themselves are built from — against a freshly built
    /// `standard_vocabulary()`, so this keeps being true no matter what the
    /// vocabulary looks like by the time this runs.
    #[test]
    fn every_bound_token_is_in_the_standard_vocabulary() {
        let vocab = standard_vocabulary();
        for name in ALL {
            let token = t(name);
            assert!(
                vocab.contains(&token),
                "component token `{name}` is not declared in standard_vocabulary()"
            );
        }
    }
}
