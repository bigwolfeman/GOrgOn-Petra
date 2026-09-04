//! Carbon Menu buttons (slice-c). A labelled button that opens a Menu.
//!
//! Anatomy (`_menu-button.scss`): trigger button + caret + Menu. Combo
//! button and Overflow menu are omitted (split trigger / icon-only).
//!
//! The caret is the word `"open"` / `"closed"`, never an icon-only mark
//! (FR-026). Carbon rotates a chevron 180°; the word is the second channel
//! that rotation cannot be.

use super::menu::menu;
use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, SHADOW_RAISED, SHAPE_MD, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled menu trigger. When `open` is true the node also carries a
/// [`menu`] popover of `items`. `label` is required (FR-058).
pub fn menu_button(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger("trigger", label.clone(), open);
    let mut children = vec![trigger];
    if open {
        children.push(menu("menu", label, items));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn trigger(key: impl Into<Key>, label: String, open: bool) -> ViewNode {
    let caret = if open { "open" } else { "closed" };
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron = text("caret", caret);
    chevron
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![caption, chevron],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props.tokens.insert("radius".into(), t(SHAPE_MD));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    let mut node = node.with_constraints(pin_height(SIZE_MD)).interactive(
        Role::Button,
        label,
        TRIGGER_INTENTS,
    );
    node.semantics.expanded = Some(open);
    node
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
    use super::{SIZE_MD, menu_button};
    use crate::component::menu::menu_item;
    use crate::tree::{Anchor, Interaction, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn items() -> Vec<ViewNode> {
        vec![menu_item("rename", "Rename")]
    }

    #[test]
    fn menu_button_closed_is_a_labelled_trigger_without_a_menu() {
        let node = menu_button("more", "More", false, items());
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1, "closed trigger has no popover");
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("More"));
        assert_eq!(trigger.semantics.expanded, Some(false));
        assert_eq!(trigger.constraints.vertical.min, Some(SIZE_MD));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert_eq!(child(trigger, "label").props.text.as_deref(), Some("More"));
        assert_eq!(
            child(trigger, "caret").props.text.as_deref(),
            Some("closed")
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    #[test]
    fn menu_button_open_includes_a_menu_popover() {
        let node = menu_button("more", "More", true, items());
        assert_eq!(node.semantics.expanded, Some(true));
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.expanded, Some(true));
        assert_eq!(child(trigger, "caret").props.text.as_deref(), Some("open"));

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("More"));
        match &menu.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "trigger"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(menu, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(
            child(content, "rename").semantics.label.as_deref(),
            Some("Rename")
        );
    }
}
