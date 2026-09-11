//! `input_group` — a horizontal row of already-built fields and buttons
//! (spec 009 T039, spaced form).
//!
//! This is the container only. Callers pass `field()` / `search()` /
//! `button()` / etc. as children; this file does not inspect them, does
//! not restyle them, and does not grow a field vocabulary of its own.
//! Do not edit `field.rs` for this leaf.
//!
//! # Anatomy
//!
//! A horizontal [`super::stack`] with a visible gap and no other chrome:
//! no fill, no shadow, no four-sided container border. The children
//! already carry their own chrome, role, and label. This node is the
//! row. `Role` has no group entry, and a container with no actions is
//! outside `ActionableNeedsRoleAndLabel`.
//!
//! Two constructors, one row:
//!
//! 1. [`input_group`] — N already-built children, in paint order.
//! 2. [`input_group_with_addon`] — one addon, then one field. Trailing
//!    addons are [`input_group`] with the field first.
//!
//! # Seamless joined edges wait on T040
//!
//! A flush-attached group wants square corners on every inner edge and
//! the role's radius on the outer two. That is per-corner radius (spec
//! 009 T002 / T040). This leaf does not have it and does not fake it
//! by dropping the gap. The gap is [`super::tokens::SPACING_03`]: the
//! spaced form, not the flush one. Zero-gap seamless attachment is
//! T040's job.

use super::stack;
use super::tokens::SPACING_03;
use crate::geom::{Align, Axis};
use crate::tree::{Key, ViewNode};

/// A horizontal row of already-built fields and buttons.
///
/// `children` are `field()` / `search()` / `button()` / etc. nodes. This
/// constructor does not inspect them, does not restyle them, and does
/// not take a variant argument. No role of its own: the children carry
/// chrome, role, and label; this node is the row.
///
/// No fill, no shadow, no four-sided border. Gap is [`SPACING_03`]:
/// the spaced form. Seamless joined edges wait on T040 (per-corner
/// radius). Until then this constructor does not rewrite children.
#[must_use]
pub fn input_group(key: impl Into<Key>, children: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), children);
    node.props.align = Some(Align::Center);
    node
}

/// [`input_group`] of one addon, then one field.
///
/// `addon` is typically a button or a prefix control; `field` is
/// typically `field()` / `search()` / `select()`. Neither is inspected.
/// The gap is the same [`SPACING_03`] the N-child constructor spends.
/// A trailing addon is [`input_group`] with the field first. Seamless
/// joined edges wait on T040.
#[must_use]
pub fn input_group_with_addon(key: impl Into<Key>, addon: ViewNode, field: ViewNode) -> ViewNode {
    input_group(key, vec![addon, field])
}

#[cfg(test)]
mod tests {
    use super::{SPACING_03, input_group, input_group_with_addon};
    use crate::geom::{Align, Axis};
    use crate::tree::{NodeKind, ViewNode};

    fn leaf(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key)
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn input_group_keeps_every_child_and_gaps_them_by_spacing_03() {
        let node = input_group("row", vec![leaf("a"), leaf("b"), leaf("c")]);
        assert_eq!(node.key.as_str(), "row");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(node.props.align, Some(Align::Center));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_03),
            "spaced form, not flush"
        );
        assert_eq!(node.children.len(), 3);
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["a", "b", "c"]);
        assert!(node.semantics.role.is_none());
        assert!(node.interactions.is_empty());
        assert_eq!(
            token(&node, "border"),
            None,
            "the group is a row, not a bordered well"
        );
        assert_eq!(token(&node, "background"), None);
        assert_eq!(token(&node, "shadow"), None);
    }

    #[test]
    fn input_group_with_addon_is_addon_then_field_with_the_same_gap() {
        let node = input_group_with_addon("prefixed", leaf("addon"), leaf("field"));
        assert_eq!(node.key.as_str(), "prefixed");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_03),
            "addon form spends the same gap as the N-child constructor"
        );
        assert_eq!(node.children.len(), 2);
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["addon", "field"]);
        assert_eq!(token(&node, "border"), None);
    }
}
