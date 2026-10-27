//! Declare a node and everything under it read-only.
//!
//! The library's read-only marker, and the shared twin of
//! [`super::disabled`]'s walker: the same subtree-wide declaration, the
//! opposite judgment. `disabled` says the control is unavailable; this says
//! the control is **present, readable, and not editable** — which is why it
//! is its own function and not a `bool` on the other one. The shipped
//! per-control markers ([`super::controls::checkbox_readonly`],
//! [`super::field::field_readonly`], [`super::slider::slider_readonly`])
//! each wrote this fact by hand for their own shape; this is the fact,
//! factored out so the controls that have no marker of their own (the
//! read-only rows of Toggle, Radio, Date picker, Dropdown, Number input and
//! Select in `VARIANTS.md`) can carry it too.
//!
//! # What it does
//!
//! * `Semantics.read_only = true` on every node in the subtree,
//! * drops the edit verbs (`TextEdit`, `Key`, `Click`, `Drag`) on every node
//!   in the subtree, keeping `Focus` and `Hover`, and
//! * drops the node's `behaviour` — the intent the control fires. This is
//!   the boundary [`super::controls::checkbox`]'s own doc names ("does not
//!   inherit a behaviour it will not honour"): a marker that kept the
//!   intent would still report the toggle's flip on release.
//!
//! The verb set is the one [`super::field::field_readonly`] states in its
//! own doc — "Drops `TextEdit` and `Key`. Keeps `Focus`" — widened to the
//! two verbs the other shapes edit through: `Click` (the toggle flip, the
//! menu open, the stepper press) and `Drag` (the slider handle). `Hover` is
//! kept because a pointer crossing a control is a fact about the pointer,
//! not an edit.
//!
//! # Why not [`super::disabled`]
//!
//! A read-only control keeps its ink, its focus ring and its place in the
//! tab order — it is read, not greyed. [`super::disabled`] clears every
//! interaction and sets a different flag; the two are distinct facts and the
//! register's own evidence vocabulary names them separately
//! (`checkbox_readonly` vs `component::disabled()`). Nothing here fades the
//! subtree: the visible channel for a read-only control is the chrome its
//! component already declares (the field family's `FieldChrome::ReadOnly`
//! well, [`super::slider::slider_readonly`]'s collapsed handle) — this
//! function declares the fact and those channels follow from it, the same
//! division of labour [`super::disabled`] documents.
//!
//! This module carries the spec 008 T060 record for the read-only state
//! rows; per-control read-only markers stay where they are.

use std::sync::Arc;

use crate::tree::{Interaction, ViewNode};

/// Declare `node` and everything under it read-only.
///
/// See the module doc for the fact it declares and the verbs it drops.
#[must_use]
pub fn readonly(mut node: ViewNode) -> ViewNode {
    node.semantics.read_only = true;
    node.interactions
        .retain(|interaction| matches!(interaction, Interaction::Focus | Interaction::Hover));
    node.behaviour = None;
    // `Arc::unwrap_or_clone` for the same reason [`super::disabled`] gives:
    // a shared subtree read-only in one place and live in another has to
    // become two subtrees.
    node.children = node
        .children
        .into_iter()
        .map(|child| Arc::new(readonly(Arc::unwrap_or_clone(child))))
        .collect();
    node
}

#[cfg(test)]
mod tests {
    use super::readonly;
    use crate::component::{checkbox, field, field_labeled, toggle};
    use crate::tree::{Interaction, NodeKind, ViewNode};

    fn count_inputs(node: &ViewNode) -> usize {
        let mut n = usize::from(node.kind == NodeKind::Input);
        for child in &node.children {
            n += count_inputs(child);
        }
        n
    }

    fn every(node: &ViewNode, pred: &dyn Fn(&ViewNode) -> bool) -> bool {
        pred(node) && node.children.iter().all(|child| every(child, pred))
    }

    fn all_interactions_in(node: &ViewNode, keep: &[Interaction]) -> bool {
        every(node, &|n| n.interactions.iter().all(|i| keep.contains(i)))
    }

    #[test]
    fn readonly_marks_the_whole_subtree_not_just_the_wrapper() {
        // A labelled field is a wrapper with a nested Input: the marker has
        // to reach past the wrapper to be worth having.
        let node = readonly(field_labeled("name", "Fiber name"));
        assert!(node.children.iter().any(|c| c.kind == NodeKind::Input));
        assert!(
            every(&node, &|n| n.semantics.read_only),
            "every node under the marker carries Semantics.read_only"
        );
    }

    #[test]
    fn readonly_keeps_focus_and_drops_edit() {
        // `NodeKind::Input` is a leaf kind (the anatomy note on
        // `super::field::field`), so a bare `field` *is* the interactive
        // node: the marker lands on the leaf itself.
        let leaf = readonly(field("name", "Fiber name"));
        assert!(leaf.semantics.read_only);
        assert!(leaf.behaviour.is_none());
        assert!(
            all_interactions_in(&leaf, &[Interaction::Focus, Interaction::Hover]),
            "no edit verb survives the marker on the bare leaf"
        );

        // The labelled wrapper is the shape the form constructors actually
        // hand `readonly`: the nested Input is where the edit verbs live.
        let node = readonly(field_labeled("name", "Fiber name"));
        let input = &node.children[1];
        assert_eq!(input.kind, NodeKind::Input);
        assert!(input.semantics.read_only);
        assert!(input.behaviour.is_none());
        assert!(
            all_interactions_in(input, &[Interaction::Focus, Interaction::Hover]),
            "no edit verb survives the marker on the nested Input"
        );
        assert!(
            count_inputs(&node) > 0,
            "the field is still there — read-only, not removed"
        );
    }

    #[test]
    fn readonly_drops_the_intent_it_would_not_honour() {
        // `checkbox`'s own doc: the flag day is that a marker keeping the
        // behaviour "does not inherit a behaviour it will not honour".
        let node = readonly(toggle("t", "Autosave", true));
        assert!(node.behaviour.is_none(), "a read-only toggle fires nothing");
    }

    #[test]
    fn readonly_is_not_disabled() {
        // The two are distinct facts (the register's evidence vocabulary
        // names them separately). A read-only control keeps its ink and its
        // place in the tab order.
        for control in [
            checkbox("c", "Locked", true),
            toggle("t", "Autosave", true),
            field("f", "Name"),
        ] {
            let node = readonly(control);
            assert!(
                every(&node, &|n: &ViewNode| n.semantics.read_only),
                "read-only is set on every node"
            );
            assert!(
                every(&node, &|n| !n.semantics.disabled),
                "read-only never sets disabled"
            );
        }
    }
}
