//! `input_group` — a horizontal row of already-built fields and buttons
//! (spec 009 T039 spaced form, T040 seamless form).
//!
//! This is the container only. Callers pass `field()` / `search()` /
//! `button()` / etc. as children; this file does not inspect them, does
//! not restyle them, and does not grow a field vocabulary of its own.
//! Do not edit `field.rs` for this leaf.
//!
//! # Anatomy
//!
//! A horizontal [`super::stack`] with no other chrome: no fill, no shadow,
//! no four-sided container border. The children already carry their own
//! chrome, role, and label. This node is the row. `Role` has no group
//! entry, and a container with no actions is outside
//! `ActionableNeedsRoleAndLabel`.
//!
//! Four constructors, two rows:
//!
//! 1. [`input_group`] — N already-built children, gapped, in paint order.
//! 2. [`input_group_with_addon`] — one addon, then one field, gapped.
//!    Trailing addons are [`input_group`] with the field first.
//! 3. [`input_group_seamless`] — N already-built children, zero gap.
//! 4. [`input_group_with_addon_seamless`] — the addon form of 3.
//!
//! # Seamless joined edges (spec 009 T040)
//!
//! The seamless forms reach [`crate::token::corners_for`] (spec 009 T002)
//! through [`bind_corners`]: each child's two corners a neighbour touches
//! are squared, and its two free corners take
//! [`crate::token::CornerRole::Tiled`]'s radius — `shape.corner-none` at
//! every extent, the same role [`CornerRole::Tiled`]'s own doc names "text
//! inputs" under. A seamless row's visible change is therefore the gap
//! going to zero; the corner-slot bindings this leaf now writes are what
//! keep that true regardless of what `Tiled`'s ramp step happens to be,
//! rather than a fact this file assumes.

use super::tokens::SPACING_03;
use super::{bind_corners, stack};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, Joined, corners_for};
use crate::tree::{Key, ViewNode};

/// A representative row height for [`corners_for`]'s `shorter_edge`
/// argument. `CornerRole::Tiled` returns `shape.corner-none` at every
/// extent — its radius is zero, so the half-edge clause that `shorter_edge`
/// feeds never fires — so this number never changes what
/// [`input_group_seamless`] binds; it exists so the call site does not
/// invent one out of the children it does not inspect.
const REPRESENTATIVE_HEIGHT: f32 = 40.0;

/// A horizontal, gapped row of already-built fields and buttons.
///
/// `children` are `field()` / `search()` / `button()` / etc. nodes. This
/// constructor does not inspect them, does not restyle them, and does
/// not take a variant argument. No role of its own: the children carry
/// chrome, role, and label; this node is the row.
///
/// No fill, no shadow, no four-sided border. Gap is [`SPACING_03`]. See
/// [`input_group_seamless`] for the zero-gap, squared-seam form.
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
/// A trailing addon is [`input_group`] with the field first.
#[must_use]
pub fn input_group_with_addon(key: impl Into<Key>, addon: ViewNode, field: ViewNode) -> ViewNode {
    input_group(key, vec![addon, field])
}

/// A horizontal, zero-gap row of already-built fields and buttons whose
/// adjoining corners are square and whose two free ends take
/// [`CornerRole::Tiled`]'s radius — Carbon's seamless input-with-addon
/// anatomy generalised to N children.
///
/// Each child's radius is overridden by [`bind_corners`] writing the four
/// corner slots directly, in place of whatever `radius` (or nothing, as
/// `field()` binds today) the child's own constructor left there.
#[must_use]
pub fn input_group_seamless(key: impl Into<Key>, mut children: Vec<ViewNode>) -> ViewNode {
    let count = children.len();
    for (i, child) in children.iter_mut().enumerate() {
        let joined = Joined {
            left: i > 0,
            right: i + 1 < count,
            ..Joined::NONE
        };
        let corners = corners_for(CornerRole::Tiled, REPRESENTATIVE_HEIGHT, joined);
        bind_corners(&mut child.props.tokens, corners);
    }
    let mut node = stack(key, Axis::Horizontal, None, children);
    node.props.align = Some(Align::Center);
    node
}

/// [`input_group_seamless`] of one addon, then one field.
#[must_use]
pub fn input_group_with_addon_seamless(
    key: impl Into<Key>,
    addon: ViewNode,
    field: ViewNode,
) -> ViewNode {
    input_group_seamless(key, vec![addon, field])
}

#[cfg(test)]
mod tests {
    use super::{
        input_group, input_group_seamless, input_group_with_addon, input_group_with_addon_seamless,
    };
    use crate::geom::{Align, Axis};
    use crate::tree::{NodeKind, ViewNode};

    fn leaf(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key)
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn input_group_keeps_every_child_and_gaps_them() {
        let node = input_group("row", vec![leaf("a"), leaf("b"), leaf("c")]);
        assert_eq!(node.key.as_str(), "row");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(node.props.align, Some(Align::Center));
        assert!(node.props.spacing.is_some(), "the spaced form must gap");
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
        for child in &node.children {
            assert_eq!(
                token(child, "radius-top-left"),
                None,
                "the spaced form does not rewrite a child's corners"
            );
        }
    }

    #[test]
    fn input_group_with_addon_is_addon_then_field_with_the_same_gap() {
        let node = input_group_with_addon("prefixed", leaf("addon"), leaf("field"));
        assert_eq!(node.key.as_str(), "prefixed");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert!(node.props.spacing.is_some());
        assert_eq!(node.children.len(), 2);
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["addon", "field"]);
        assert_eq!(token(&node, "border"), None);
    }

    #[test]
    fn input_group_seamless_gaps_nothing_and_squares_only_the_touching_corners() {
        let node = input_group_seamless("row", vec![leaf("a"), leaf("b"), leaf("c")]);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert!(
            node.props.spacing.is_none(),
            "the seamless form must not gap: the seam is the point"
        );
        assert_eq!(node.children.len(), 3);

        // `CornerRole::Tiled` is `shape.corner-none` at every extent, so the
        // free corners and the squared corners are the same name here — the
        // binding is still exercised, and stays correct the moment `Tiled`
        // stops being zero.
        let square = "shape.corner-none";
        let free = "shape.corner-none";

        let first = &node.children[0];
        assert_eq!(token(first, "radius-top-left"), Some(free));
        assert_eq!(token(first, "radius-bottom-left"), Some(free));
        assert_eq!(
            token(first, "radius-top-right"),
            Some(square),
            "the first child's right edge touches the second child"
        );
        assert_eq!(token(first, "radius-bottom-right"), Some(square));

        let middle = &node.children[1];
        for slot in [
            "radius-top-left",
            "radius-top-right",
            "radius-bottom-right",
            "radius-bottom-left",
        ] {
            assert_eq!(
                token(middle, slot),
                Some(square),
                "a middle child touches a neighbour on both sides"
            );
        }

        let last = &node.children[2];
        assert_eq!(token(last, "radius-top-left"), Some(square));
        assert_eq!(token(last, "radius-bottom-left"), Some(square));
        assert_eq!(token(last, "radius-top-right"), Some(free));
        assert_eq!(token(last, "radius-bottom-right"), Some(free));
    }

    #[test]
    fn input_group_with_addon_seamless_is_addon_then_field_with_no_gap() {
        let node = input_group_with_addon_seamless("prefixed", leaf("addon"), leaf("field"));
        assert!(node.props.spacing.is_none());
        assert_eq!(node.children.len(), 2);
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["addon", "field"]);
        let addon = &node.children[0];
        let field = &node.children[1];
        assert_eq!(
            token(addon, "radius-top-right"),
            Some("shape.corner-none"),
            "the addon's right edge touches the field"
        );
        assert_eq!(
            token(field, "radius-top-left"),
            Some("shape.corner-none"),
            "the field's left edge touches the addon"
        );
    }
}
