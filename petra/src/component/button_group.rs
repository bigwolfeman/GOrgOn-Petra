//! `button_group` — a horizontal row of already-built buttons (spec 009 T027).
//!
//! This is the container only. Our seven button variants and six sizes in
//! [`super::button`] stay. Callers pass `button()` / `primary_button()` /
//! etc. as children; this file does not grow a button vocabulary of its
//! own. It does re-seat each child's [`crate::tree::FocusFigure`] to
//! [`crate::tree::FocusFigure::BarUnder`]: a lone button wears
//! [`crate::tree::FocusFigure::Sides`], and those bars paint in the gap
//! (spaced) or the seam (flush). The under bar sits below the row, which
//! both forms have room for. The flush constructor also rewrites corners.
//!
//! # Anatomy
//!
//! A horizontal [`super::stack`] with no other chrome: no fill, no shadow,
//! no four-sided container border. The children already carry their own
//! chrome, role, and label. This node is the row. `Role` has no group
//! entry, and a container with no actions is outside
//! `ActionableNeedsRoleAndLabel`.
//!
//! Two constructors, one row:
//!
//! 1. [`button_group`] — a gapped row. Every child keeps whatever radius it
//!    built with.
//! 2. [`button_group_flush`] — a zero-gap row. Each child's outer corners
//!    (the ones no neighbour touches) take [`crate::token::CornerRole::Grouping`]'s
//!    radius; every corner a neighbour touches is squared, so two rounded
//!    corners never meet across the seam and leave a lens-shaped gap.
//!
//! # Squared inner edges (spec 009 T002)
//!
//! [`button_group_flush`] reaches this through
//! [`crate::token::corners_for`], which spec 009 T002 added: one role plus
//! one adjacency fact (`crate::token::Joined`) in, four corner tokens out.
//! `bind_corners` writes those four onto a child's own `props.tokens`,
//! overriding whatever `radius` the child's own constructor bound. This
//! file otherwise does not inspect a child: it touches the four corner
//! slots (flush only), the focus figure, never the fill, the border, or
//! the label.

use super::tokens::SPACING_02;
use super::{bind_corners, stack};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, Joined, corners_for};
use crate::tree::{FocusFigure, Key, ViewNode};

/// A horizontal, gapped row of already-built buttons.
///
/// `children` are `button()` / `primary_button()` / etc. nodes. This
/// constructor does not take a variant argument. No role of its own: the
/// buttons carry chrome, role, and label; this node is the row. Each
/// child's focus figure is re-seated to [`FocusFigure::BarUnder`] (see
/// [`seat_bar_under`]).
///
/// No fill, no shadow, no four-sided border. Gap is [`SPACING_02`]. See
/// [`button_group_flush`] for the zero-gap, squared-seam form.
#[must_use]
pub fn button_group(key: impl Into<Key>, mut children: Vec<ViewNode>) -> ViewNode {
    seat_bar_under(&mut children);
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_02), children);
    node.props.align = Some(Align::Center);
    node
}

/// A horizontal, zero-gap row of already-built buttons whose adjoining
/// corners are square and whose two free ends take
/// [`CornerRole::Grouping`]'s radius — Carbon's flush segmented-button
/// anatomy.
///
/// `height` is every child's shared block extent (Carbon segmented buttons
/// are one uniform height), and only feeds
/// [`crate::token::corner_for`]'s half-edge clause; it does not resize a
/// child, which still carries whatever `Constraints` its own constructor
/// pinned.
///
/// Each child's radius is overridden by [`bind_corners`] writing the four
/// corner slots directly, in place of whatever `radius` the child bound —
/// for every shipped button that is `CornerRole::Tiled`'s `shape.corner-none`,
/// so the visible change is the two free ends gaining `Grouping`'s radius
/// and the gap between children going to zero. Focus figure is then
/// re-seated to [`FocusFigure::BarUnder`] (see [`seat_bar_under`]): `Sides`
/// would paint in the seam.
#[must_use]
pub fn button_group_flush(
    key: impl Into<Key>,
    height: f32,
    mut children: Vec<ViewNode>,
) -> ViewNode {
    let count = children.len();
    for (i, child) in children.iter_mut().enumerate() {
        let joined = Joined {
            left: i > 0,
            right: i + 1 < count,
            ..Joined::NONE
        };
        let corners = corners_for(CornerRole::Grouping, height, joined);
        bind_corners(&mut child.props.tokens, corners);
    }
    seat_bar_under(&mut children);
    let mut node = stack(key, Axis::Horizontal, None, children);
    node.props.align = Some(Align::Center);
    node
}

/// Re-seat every child's focus figure to [`FocusFigure::BarUnder`].
///
/// A lone `button()` wears [`FocusFigure::Sides`]. In a group those bars
/// paint in the gap between neighbours, and in a flush group they paint
/// in the seam. The under bar sits below the row, which both forms have
/// room for. Same pattern as
/// `gorgon_petra_compound::selection_palette`'s `flush_mark`, which
/// re-seats `toggle_button`'s `Sides` to `BarInside`.
fn seat_bar_under(children: &mut [ViewNode]) {
    for child in children {
        child.semantics.focus_figure = FocusFigure::BarUnder;
    }
}

#[cfg(test)]
mod tests {
    use super::super::button::button;
    use super::{button_group, button_group_flush};
    use crate::geom::{Align, Axis};
    use crate::tree::{FocusFigure, NodeKind, ViewNode};

    fn leaf(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key)
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn button_group_keeps_every_child_and_gaps_them() {
        let node = button_group("row", vec![leaf("a"), leaf("b"), leaf("c")]);
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
        assert_eq!(token(&node, "background"), None);
        assert_eq!(token(&node, "border"), None);
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
    fn button_group_flush_gaps_nothing_and_squares_only_the_touching_corners() {
        let node = button_group_flush("row", 40.0, vec![leaf("a"), leaf("b"), leaf("c")]);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert!(
            node.props.spacing.is_none(),
            "the flush form must not gap: the seam is the point"
        );
        assert_eq!(node.children.len(), 3);

        let square = "shape.corner-none";
        let free = "shape.corner-sm"; // CornerRole::Grouping at 40 units

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
    fn button_group_flush_of_one_child_squares_nothing() {
        let node = button_group_flush("row", 40.0, vec![leaf("solo")]);
        let solo = &node.children[0];
        let free = "shape.corner-sm";
        for slot in [
            "radius-top-left",
            "radius-top-right",
            "radius-bottom-right",
            "radius-bottom-left",
        ] {
            assert_eq!(
                token(solo, slot),
                Some(free),
                "a lone child touches nothing, so every corner is free"
            );
        }
    }

    #[test]
    fn button_group_reseats_child_focus_figure_to_bar_under() {
        let lone = button("save", "Save");
        assert_eq!(
            lone.semantics.focus_figure,
            FocusFigure::Sides,
            "the override is load-bearing only while a lone button wears Sides"
        );

        let spaced = button_group("row", vec![button("save", "Save")]);
        assert_eq!(
            spaced.children[0].semantics.focus_figure,
            FocusFigure::BarUnder
        );

        let flush = button_group_flush("row", 40.0, vec![button("save", "Save")]);
        assert_eq!(
            flush.children[0].semantics.focus_figure,
            FocusFigure::BarUnder
        );
    }
}
