//! Carbon Menu (slice-c). Floating action list on Popover.
//!
//! Anatomy (`_menu.scss`):
//! 1. Menu container — [`super::popover::popover_with`], [`Role::Overlay`].
//! 2. Action item — [`menu_item`]: [`Role::Button`], height md 40.
//!
//! Container width is min 160 / max 288 (SCSS `$supported-sizes` map and
//! style-page). Item padding is [`SPACING_05`] inline. Submenus, danger
//! hover, and the `--with-icons` column are omitted.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon menu min-inline (`10rem`).
const MIN_INLINE: f32 = 160.0;
/// Carbon menu max-inline (`18rem`).
const MAX_INLINE: f32 = 288.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MIN_INLINE == 160.0);
const _: () = assert!(MAX_INLINE == 288.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A floating menu of `items`, hosted on a popover.
///
/// Anchored to a sibling keyed `"trigger"` (the key [`super::menu_button`]
/// uses for its button). `label` is the accessible name of the overlay.
pub fn menu(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = popover_with(key, label, "trigger", items);
    node.constraints.horizontal = AxisConstraint {
        min: Some(MIN_INLINE),
        max: Some(MAX_INLINE),
        priority: 0,
    };
    node
}

/// One action row. `label` is required (FR-058).
pub fn menu_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, ITEM_INTENTS)
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, MIN_INLINE, SIZE_MD, menu, menu_item};
    use crate::tree::{Anchor, Interaction, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn menu_is_an_overlay_popover_not_a_dialog() {
        let node = menu(
            "actions",
            "Actions",
            vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")],
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.interactions.is_empty());
        match &node.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "trigger"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MIN_INLINE, 160.0);
        assert_eq!(MAX_INLINE, 288.0);
        let content = child(&node, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        let rename = child(content, "rename");
        assert_eq!(rename.semantics.role, Some(Role::Button));
        assert_eq!(rename.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(
            child(content, "delete").semantics.label.as_deref(),
            Some("Delete")
        );
    }

    #[test]
    fn menu_item_is_a_labelled_button_at_height_40() {
        let node = menu_item("rename", "Rename");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Rename"));
    }
}
