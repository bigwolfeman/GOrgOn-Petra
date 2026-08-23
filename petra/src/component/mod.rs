//! Petra's component library: the default authoring vocabulary above the
//! primitives (FR-052, FR-058).
//!
//! `crate::tree` gives an author twelve node kinds and a bag of style
//! tokens; nothing there stops a tree from declaring a clickable node with
//! no role, or a status readout that is a colour and nothing else. This
//! module is the layer where that stops being possible. Contract C13 (see
//! `.agents/research/08-22-2026/Petra-Vocabulary-Build/PLAN.md`) fixes the
//! set at exactly thirteen names, chosen from what `gallery.rs` and the
//! inspector's planned panels actually compose rather than from what FR-052
//! could in principle mean:
//!
//! | Component | Role it sets | Interactive |
//! |---|---|---|
//! | [`button`] | `Role::Button` | yes |
//! | [`text`] | none | no |
//! | [`heading`] | none | no |
//! | [`field`] | `Role::TextInput` | yes |
//! | [`checkbox`] | `Role::Button` + selected | yes |
//! | [`radio`] | `Role::Button` + selected | yes |
//! | [`toggle`] | `Role::Button` + selected | yes |
//! | [`tab_bar`] | `Role::TabList` | no |
//! | [`tab`] | `Role::Tab` + selected | yes |
//! | [`progress`] | `Role::Progress` | no |
//! | [`status`] | `Role::Status` | no |
//! | [`section`] | none | no |
//! | [`list_row`] | `Role::ListItem` + selected | yes |
//!
//! `Role::Dialog`, `Role::List`, and `Role::Table` each appear once in the
//! gallery and stay expressed through the primitives directly — a component
//! with one consumer is a helper, not a library member, and C13 draws the
//! line there deliberately.
//!
//! # How FR-058 is enforced
//!
//! Every interactive entry above takes its label as a required, non-`Option`
//! positional parameter and sets its own role and interactions inside the
//! function body — never through a builder step a caller could choose not
//! to take. There is no `Button::new(key)` anywhere in this module, and no
//! `with_role`/`with_label`/`set_role`/`set_label` method exists to undo a
//! role or label after construction (gate C1-4): the only way to get a
//! `ViewNode` out of [`button`], [`field`], [`checkbox`], [`radio`],
//! [`toggle`], [`tab`], or [`list_row`] is to supply the label they demand.
//! [`status`] enforces the parallel FR-015 guarantee by taking a whole,
//! already-validated [`crate::token::StatusToken`] rather than reassembling
//! one from separate colour/shape/text arguments — see that module's doc
//! for why a constructor alone was once not enough.
//!
//! # Where the token names live
//!
//! [`tokens`] is the one place a design-token name is spelled as a literal
//! string. Every component reaches for a constant there rather than writing
//! `"spacing.md"` at its own call site, so a rename in the shipped
//! vocabulary is a one-line fix instead of a sweep across a dozen files —
//! and [`tokens`]'s own test proves every one of those constants is
//! actually declared in [`crate::token::standard_vocabulary`] (gate C1-7).
//!
//! # What this module does not do
//!
//! It does not replace the primitives: every function here returns a
//! `ViewNode` built from `NodeKind::Stack`, `Grid`, `Text`, `Input`, or
//! `Spacer` — the same twelve-variant enum `crate::tree` has always had —
//! and any layout this library does not cover (an arbitrary grid, a custom
//! surface) is still composed from those primitives directly, which stay
//! public (gate C1-10).

mod button;
mod controls;
mod field;
mod list_row;
mod progress;
mod section;
mod status;
mod tabs;
mod text;
mod tokens;

pub use button::button;
pub use controls::{checkbox, radio, toggle};
pub use field::field;
pub use list_row::list_row;
pub use progress::progress;
pub use section::section;
pub use status::status;
pub use tabs::{tab, tab_bar};
pub use text::{heading, text};

use crate::geom::Axis;
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, NodeKind, Props, ViewNode};

/// A `Stack` on `axis`, gapped by `spacing`, with no other props set.
///
/// Every component with more than one visual part is built from this — the
/// one place `NodeKind::Stack` is spelled inside the library, so a bug in
/// how a row or column is assembled has one place to be found and fixed
/// rather than a dozen.
pub(crate) fn stack(
    key: impl Into<Key>,
    axis: Axis,
    spacing: Option<&str>,
    children: Vec<ViewNode>,
) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(axis),
            spacing: spacing.map(tokens::t),
            ..Props::default()
        })
        .with_children(children)
}

/// A fixed-extent, unlabelled rectangle: the drawn box every swatch-shaped
/// piece of chrome in the library is built from (a checkbox's box, a
/// toggle's knob and pad, a status dot, a progress fill and track).
///
/// Never exported. A bare coloured box carries no semantics of its own —
/// FR-058 has nothing to say about it because nothing here is interactive
/// or labelled — which is exactly why it is a building block a component
/// composes rather than a component the library ships on its own.
pub(crate) fn swatch(
    key: impl Into<Key>,
    w: f32,
    h: f32,
    background: Option<&str>,
    border: Option<&str>,
    radius: Option<&str>,
) -> ViewNode {
    let mut props = Props::default();
    if let Some(name) = background {
        props.tokens.insert("background".into(), tokens::t(name));
    }
    if let Some(name) = border {
        props.tokens.insert("border".into(), tokens::t(name));
    }
    if let Some(name) = radius {
        props.tokens.insert("radius".into(), tokens::t(name));
    }
    ViewNode::new(NodeKind::Spacer, key)
        .with_props(props)
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(w),
                max: Some(w),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(h),
                max: Some(h),
                priority: 0,
            },
        })
}

/// [`InsetRefs::symmetric`] over two of this module's spacing constants,
/// named the way `InsetRefs::symmetric` itself is: horizontal first.
pub(crate) fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tokens::t(horizontal), tokens::t(vertical))
}

#[cfg(test)]
mod tests;
