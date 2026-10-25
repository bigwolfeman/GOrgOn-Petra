//! Declare a node and everything under it unavailable.
//!
//! The library's disabled variant, and the replacement for the composition
//! every caller was writing by hand — clear the interactions, set the flag,
//! and then reach for a third channel because the first two are invisible.
//! The third channel used to be `Props.opacity`, which is
//! `contracts/interaction-state.md` §5's named example of the thing this
//! exists to stop: a whole subtree faded at paint time is a state no gate can
//! read, no driver can query, and no theme can retune.
//!
//! This module is also the home of the spec 008 US5 record: which components
//! bind their own `@disabled` tokens and which are covered by this wrapper
//! alone lives in `specs/008-petra-carbon-variants/contracts/disabled.md`,
//! and the per-unit cases that read those bindings back out of the built
//! gallery cell live beside the cells they read
//! (`petra/petra-egui/src/bin/gallery/disabled_states.rs`).

use std::sync::Arc;

use crate::tree::ViewNode;

/// Declare `node` and everything under it unavailable.
///
/// # What it does
///
/// * `Semantics.disabled = true` on every node in the subtree, and
/// * `interactions.clear()` on every node in the subtree.
///
/// # Why the whole subtree
///
/// `disabled` is a per-node flag with no inheritance — `layout::semantics_of`
/// reads each node's own declaration — and a component's chrome and its label
/// are two nodes. Setting the flag on the wrapper alone would leave
/// [`crate::component::button`]'s label resolving its *enabled* `foreground`,
/// so an unavailable button would draw live ink on a flat card. Every node in
/// the subtree is unavailable, so every node says so.
///
/// # Where the two visible channels are
///
/// Neither is here, and that is the point (FR-010: never colour alone, and
/// never opacity alone):
///
/// * the **ink** comes from the `@disabled` token family the component
///   already declared ([`crate::component::button`] binds
///   `foreground@disabled`), resolved by `crate::token::state`'s precedence
///   chain;
/// * the **elevation** is dropped by the painter for any node whose resolved
///   rank is disabled, which is the channel that survives a reader who cannot
///   separate the colours at all.
///
/// So this function declares a fact and the two channels follow from it,
/// rather than a caller painting the fact three times and hoping.
#[must_use]
pub fn disabled(mut node: ViewNode) -> ViewNode {
    node.semantics.disabled = true;
    node.interactions.clear();
    // `Arc::unwrap_or_clone` rather than a walk over `&mut`: children are
    // shared handles, and a shared subtree that is disabled in one place and
    // live in another has to become two subtrees. The clone is what makes it
    // two. It costs this subtree its pointer-identity against the previous
    // frame — `layout::reuse` compares children by `Arc::ptr_eq` — which is
    // the right trade: a control that just became unavailable is not a
    // subtree worth carrying over unchanged.
    node.children = node
        .children
        .into_iter()
        .map(|child| Arc::new(disabled(Arc::unwrap_or_clone(child))))
        .collect();
    node
}
