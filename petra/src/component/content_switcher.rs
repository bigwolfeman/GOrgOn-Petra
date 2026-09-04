//! Carbon Content switcher (slice-b).
//!
//! Anatomy (docs + `_content-switcher.scss`):
//! 1. [`content_switcher`] — the row (`Role::TabList`), radius 4
//!    ([`SHAPE_SM`]), 1px [`BORDER_SUBTLE`] outline. High-contrast
//!    `$border-inverse` is not in the component vocabulary.
//! 2. [`content_switcher_item`] — one tab button, height 40 ([`SIZE_MD`]).
//!    Selected is never colour alone: [`LAYER_SELECTED`] plus
//!    `Semantics.selected` plus heading type.
//!
//! Icon-only and high-contrast-inverse fills are omitted: no extra icon
//! marks, and `layer-selected-inverse` is not a shipped token.

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SIZE_MD,
    SPACING_03, SPACING_05, SURFACE_BASE, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, Semantics, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// The switcher row. `Role::TabList`, no interactions of its own.
pub fn content_switcher(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, items);
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}

/// One switcher tab. `Role::Button`, selected declared in `Semantics`.
///
/// Height is Carbon md 40. Label padding is `$spacing-05` inline.
pub fn content_switcher_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let mut label_node = text("label", label.clone());
    label_node.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING
    } else {
        TYPOGRAPHY_BODY
    }));
    label_node.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
    );

    let mut node = stack(key, Axis::Horizontal, None, vec![label_node]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            LAYER_SELECTED
        } else {
            SURFACE_BASE
        }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));

    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, ITEM_INTENTS);
    node.semantics.selected = selected;
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
    use super::{
        BORDER_SUBTLE, LAYER_SELECTED, SHAPE_SM, SIZE_MD, content_switcher, content_switcher_item,
    };
    use crate::geom::Axis;
    use crate::tree::{Interaction, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn content_switcher_sets_role_tablist() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                content_switcher_item("b", "Grid", false),
            ],
        );
        assert_eq!(node.key.as_str(), "sw");
        assert_eq!(node.semantics.role, Some(Role::TabList));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(token(&node, "radius"), Some(SHAPE_SM));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn content_switcher_item_is_a_labelled_button_at_height_40() {
        let node = content_switcher_item("a", "List", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("List"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Hover));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(!node.semantics.selected);
    }

    #[test]
    fn content_switcher_item_declares_selected_not_colour_alone() {
        let on = content_switcher_item("a", "List", true);
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            named(&on, "label").props.style.as_ref().map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_HEADING)
        );

        let off = content_switcher_item("a", "List", false);
        assert!(!off.semantics.selected);
        assert_eq!(
            named(&off, "label")
                .props
                .style
                .as_ref()
                .map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_BODY)
        );
    }
}
