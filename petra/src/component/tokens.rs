//! The token names the component library binds, gathered in one place.
//!
//! FR-052's vocabulary sits on top of [`crate::token::shipped::standard_vocabulary`],
//! and the two are decided by different people at different times: the ramp
//! is the design system's call, the component signatures are this module's.
//! Hardcoding a literal like `"spacing.sm"` at each of a dozen call sites
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

pub(crate) const SPACING_2XS: &str = "spacing.2xs";
pub(crate) const SPACING_XS: &str = "spacing.xs";
pub(crate) const SPACING_SM: &str = "spacing.sm";
pub(crate) const SPACING_MD: &str = "spacing.md";
pub(crate) const SPACING_LG: &str = "spacing.lg";

pub(crate) const SHAPE_SM: &str = "shape.corner-sm";
pub(crate) const SHAPE_MD: &str = "shape.corner-md";
pub(crate) const SHAPE_FULL: &str = "shape.corner-full";

pub(crate) const TYPOGRAPHY_BODY: &str = "typography.body";
pub(crate) const TYPOGRAPHY_HEADING: &str = "typography.heading";

pub(crate) const SURFACE_BASE: &str = "surface.base";
pub(crate) const SURFACE_RAISED: &str = "surface.raised";
pub(crate) const TEXT_PRIMARY: &str = "text.primary";
pub(crate) const TEXT_MUTED: &str = "text.muted";

/// Every constant above, for the completeness proof. A name added above and
/// left out of this list would silently stop being covered, so the list is
/// what the test walks rather than the component source.
///
/// `#[cfg(test)]`: nothing in the shipped library reads this back, only the
/// completeness test below does, so a non-test build correctly reports it
/// unused without this — the gate is a test's job, not runtime code's.
#[cfg(test)]
pub(crate) const ALL: &[&str] = &[
    SPACING_2XS,
    SPACING_XS,
    SPACING_SM,
    SPACING_MD,
    SPACING_LG,
    SHAPE_SM,
    SHAPE_MD,
    SHAPE_FULL,
    TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING,
    SURFACE_BASE,
    SURFACE_RAISED,
    TEXT_PRIMARY,
    TEXT_MUTED,
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
