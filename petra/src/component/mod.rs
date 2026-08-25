//! Petra's component library: the default authoring vocabulary above the
//! primitives (FR-052, FR-058).
//!
//! `crate::tree` gives an author twelve node kinds and a bag of style
//! tokens; nothing there stops a tree from declaring a clickable node with
//! no role, or a status readout that is a colour and nothing else. This
//! module is the layer where that stops being possible. Contract C13 (see
//! `.agents/research/08-22-2026/Petra-Vocabulary-Build/PLAN.md`) fixes the
//! set at exactly thirteen components, chosen from what `gallery.rs` and the
//! inspector's planned panels actually compose rather than from what FR-052
//! could in principle mean:
//!
//! | Component | Role it sets | Interactive |
//! |---|---|---|
//! | [`button`] / [`primary_button`] | `Role::Button` | yes |
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
//! Thirteen **components**, fourteen public constructors: [`primary_button`]
//! is [`button`] with the accent spent on it, not a fourteenth thing. It has
//! the same role, the same interactions, the same body (`button::labelled`)
//! and the same obligations; what differs is two colours and an elevation.
//! C13 fixed the set of *shapes an author can compose*, and emphasis is a
//! property of one of those shapes rather than a new one. The alternative —
//! an `emphasis: Emphasis` parameter on `button` — was refused because every
//! existing call site would have had to name a default, which is a lot of
//! churn to express "this one is louder".
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

pub use button::{button, primary_button};
pub use controls::{checkbox, radio, toggle};
pub use field::field;
pub use list_row::list_row;
pub use progress::progress;
pub use section::section;
pub use status::status;
pub use tabs::{tab, tab_bar};
pub use text::{heading, text};

use crate::geom::Axis;
use crate::token::{LAYER_TOKENS, TokenName};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, NodeKind, Props, ViewNode};

/// The deepest seat [`on_layer`] will honour.
///
/// The shipped layer set has four entries and a control needs two of them —
/// one for the ground it sits on, one for itself — so the deepest seat that
/// still has a step left in it is index 2 (`surface.layer-two` under
/// `surface.layer-three`). That covers page under card under control, which
/// is every nesting `gallery.rs` and the inspector's panels actually build.
///
/// A deeper `depth` is clamped to this rather than allowed to run off the
/// end of the set, because the alternative is worse in a specific way: the
/// end of the array is `surface.layer-three` twice, so a node and its ground
/// would resolve to the **same colour** and the control would vanish. A
/// clamped seat draws the deepest step the ramp can express; an unclamped
/// one draws nothing at all and reports success.
pub const MAX_LAYER_DEPTH: usize = 2;

/// Re-seat a component onto the layer it is actually being placed on.
///
/// # The problem this solves
///
/// A component cannot know how deeply it is nested, so every one of them
/// picks its fill from a two-name vocabulary: [`tokens::SURFACE_BASE`] when
/// it wants to disappear into its ground (an unselected [`tab`] or
/// [`list_row`]), [`tokens::SURFACE_RAISED`] when it wants to stand out from
/// it ([`button`], [`field`], a selected `tab`). Both choices are right on a
/// page and both are wrong on a card, where the card has already spent
/// `surface.raised`: a raised control on a raised card has no edge at all,
/// and a base-filled control on a raised card reads as the *selected* one.
///
/// `gallery.rs` carried a private fix for this that gave the control a
/// `text.muted` outline instead of a fill, with a comment naming the reason:
/// *"the third level of elevation is drawn with a line, because there is no
/// third fill to spend."* There are now four fills. This is the same rule
/// with the line taken out and the missing tones put in, and it lives in the
/// library because it is the library's vocabulary that makes it necessary.
///
/// # What it does
///
/// `depth` is the [`crate::token::LAYER_TOKENS`] index of the surface this
/// node is being placed **on**. Only the node's own `background` binding is
/// rewritten, and only when it names one of the two tones above:
///
/// | bound background | becomes |
/// |---|---|
/// | `surface.base` | `LAYER_TOKENS[depth]` — the ground itself, so the node disappears into it |
/// | `surface.raised` / `surface.layer-one` | `LAYER_TOKENS[depth + 1]` — one step ahead of the ground |
/// | anything else | untouched |
///
/// "Anything else" is load-bearing, not a fall-through: an accent fill, a
/// status colour, or a background already written as an ordinal layer is a
/// deliberate choice by whoever bound it, and a re-seating pass that
/// second-guessed those would be doing something other than what its name
/// says.
///
/// Children are not touched. That is the same line
/// [`crate::tree::ViewNode`]'s public fields already draw between composing
/// a component and forking one — reaching into [`button`]'s label node to
/// repaint it would make this function a fork of every component it is
/// applied to. A caller that needs a whole subtree re-seated calls this on
/// each node it owns.
///
/// # What it deliberately does not do
///
/// It never adds a border. The whole point of having four fills is that the
/// depth cue is tonal, and an operator that quietly re-introduced an outline
/// would put back exactly the wireframe this replaced.
///
/// # The shape this is standing in for
///
/// The right long-term home for this is the tree: a `Surface` that carries
/// its own layer index, with the presenter resolving a relative fill against
/// the nearest such ancestor. That reaches `Props`, acceptance, the frame
/// digest and every capture baseline, so it is not this change. Until then
/// the depth is an argument at the call site, which is honest about the fact
/// that somebody has to know it.
#[must_use]
pub fn on_layer(mut node: ViewNode, depth: usize) -> ViewNode {
    let depth = depth.min(MAX_LAYER_DEPTH);
    let reseated = match node
        .props
        .tokens
        .get("background")
        .map(TokenName::as_str)
        .unwrap_or_default()
    {
        tokens::SURFACE_BASE => LAYER_TOKENS[depth],
        tokens::SURFACE_RAISED | "surface.layer-one" => LAYER_TOKENS[depth + 1],
        _ => return node,
    };
    node.props
        .tokens
        .insert("background".into(), tokens::t(reseated));
    node
}

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
