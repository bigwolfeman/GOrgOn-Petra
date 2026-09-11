//! Carbon context menu: a Menu at a point.
//!
//! Spec 009 T015. This file is the overlay, not the press that opens it.
//!
//! Anatomy is [`super::menu::menu`]: a [`super::list_box::list_box`] with
//! Carbon's min/max inline (160 / 288) and `takes_focus`. What differs is
//! the anchor. A dropdown menu names a sibling trigger
//! ([`crate::tree::Anchor::Sibling`]); a context menu sits at
//! [`crate::tree::Anchor::Point`]. Raised `$layer`, overlay shadow, no
//! four-sided border, no caret. Rows are [`menu_item`]. Callers pass them
//! as `items`.
//!
//! The point is an argument because a constructor is a pure function and
//! cannot know a coordinate. The fiber that owns the surface records the
//! press position as State and passes `x`, `y` on the next `view`
//! (`contracts/view-fiber.md` §4.5). A button or a compound does that
//! recording. Secondary click as an [`crate::tree::Interaction`] a node
//! can ask for is T014 and is not this leaf. A primary trigger can already
//! supply a point.

use super::menu::menu;
use crate::tree::{Anchor, Key, ViewNode};

/// A menu of `items` at viewport point `(x, y)`.
///
/// `label` is the accessible name of the overlay. `x` and `y` are logical
/// units, the top-left of the panel (how [`Anchor::Point`] is read). The
/// panel is the same one [`super::menu::menu`] builds: raised, unbordered,
/// focused while open. The point is State on the owning fiber, not
/// something this function invents.
#[must_use]
pub fn context_menu(
    key: impl Into<Key>,
    label: impl Into<String>,
    x: f32,
    y: f32,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut node = menu(key, label, items);
    node.props.anchor = Some(Anchor::Point { x, y });
    node
}

#[cfg(test)]
mod tests {
    use super::super::list_box::menu_item;
    use super::context_menu;
    use crate::component::tokens::{SHADOW_OVERLAY, SURFACE_RAISED};
    use crate::tree::{Anchor, Fit, InputPolicy, Layer, NodeKind, Role, Tip, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn items() -> Vec<ViewNode> {
        vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")]
    }

    /// The overlay is a list box at a caller-supplied point, not a sibling
    /// trigger and not a popover.
    #[test]
    fn context_menu_is_a_list_box_at_a_point() {
        let node = context_menu("actions", "Actions", 24.0, 48.0, items());
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.interactions.is_empty());
        match &node.props.anchor {
            Some(Anchor::Point { x, y }) => {
                assert_eq!(*x, 24.0);
                assert_eq!(*y, 48.0);
            }
            other => panic!("expected Anchor::Point, got {other:?}"),
        }
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(
            node.props.tip,
            Some(Tip::Flush),
            "Carbon's menu has no beak"
        );
        assert_eq!(node.props.fit, Some(Fit::Anchor));
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        assert_eq!(
            node.props.takes_focus,
            Some(true),
            "Carbon's Menu moves focus into itself when it opens"
        );
        assert_eq!(node.constraints.horizontal.min, Some(160.0));
        assert_eq!(node.constraints.horizontal.max, Some(288.0));
        assert!(node.props.padding.is_none(), "rows run edge to edge");
        assert!(
            !node.props.tokens.contains_key("border"),
            "no four-sided border; the shadow separates it"
        );
        assert!(!node.props.tokens.contains_key("radius"));
        assert_eq!(
            node.props.tokens.get("background").map(|t| t.as_str()),
            Some(SURFACE_RAISED)
        );
        assert_eq!(
            node.props.tokens.get("shadow").map(|t| t.as_str()),
            Some(SHADOW_OVERLAY)
        );
        let content = child(&node, "content");
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "no caret word in a context menu"
        );
        let rename = child(content, "rename");
        assert_eq!(rename.semantics.role, Some(Role::Button));
        assert_eq!(rename.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(
            child(content, "delete").semantics.label.as_deref(),
            Some("Delete")
        );
    }

    /// `x` and `y` are the caller's State, not a constant inside the
    /// constructor. Two points produce two anchors.
    #[test]
    fn the_point_is_the_argument() {
        let a = context_menu("m", "Menu", 12.0, 34.0, items());
        let b = context_menu("m", "Menu", 80.0, 16.0, items());
        assert_eq!(a.props.anchor, Some(Anchor::Point { x: 12.0, y: 34.0 }));
        assert_eq!(b.props.anchor, Some(Anchor::Point { x: 80.0, y: 16.0 }));
        assert_ne!(a.props.anchor, b.props.anchor);
    }
}
