//! Carbon Select (slice-e). Closed state only.
//!
//! Anatomy of the closed field (`_select.scss`):
//! 1. Field — [`SURFACE_RAISED`] + [`BORDER_SUBTLE`], height md 40.
//! 2. Current value (visible text).
//! 3. Chevron as the word `"closed"` — never an icon-only mark (FR-026).
//!
//! The open menu is Wave 3 Popover. This module does not mount an overlay,
//! a listbox, or option rows. `Role::Button` is the closed field: it is
//! what would open the menu. `select_sm` 32 / `select_lg` 48 follow the
//! shared layout scale.
//!
//! The field label is the accessible name. Carbon's label-above anatomy
//! would make the control taller than 40; it is not stacked here.

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SHAPE_SM, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon Default sm.
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);

const CLOSED_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed select at Carbon md (40). `label` is the accessible name;
/// `value` is the visible current option.
pub fn select(key: impl Into<Key>, label: impl Into<String>, value: impl Into<String>) -> ViewNode {
    select_sized(key, label, value, SIZE_MD)
}

/// Carbon sm (32).
pub fn select_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    select_sized(key, label, value, SIZE_SM)
}

/// Carbon lg (48).
pub fn select_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    select_sized(key, label, value, SIZE_LG)
}

fn select_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    height: f32,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron = text("chevron", "closed");
    chevron
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, chevron],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(height))
        .interactive(Role::Button, label, CLOSED_INTENTS)
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
    use super::{SIZE_LG, SIZE_MD, SIZE_SM, select, select_lg, select_sm};
    use crate::component::tokens::{BORDER_SUBTLE, SURFACE_RAISED};
    use crate::tree::{Interaction, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn descendant_roles(node: &ViewNode) -> Vec<Role> {
        let mut roles = Vec::new();
        if let Some(role) = node.semantics.role.clone() {
            roles.push(role);
        }
        for child in &node.children {
            roles.extend(descendant_roles(child));
        }
        roles
    }

    #[test]
    fn select_is_a_closed_button_at_height_40() {
        let node = select("theme", "Theme", "Dark");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(child(&node, "value").props.text.as_deref(), Some("Dark"));
        assert_eq!(
            child(&node, "chevron").props.text.as_deref(),
            Some("closed")
        );
        assert_eq!(
            child(&node, "chevron").semantics.role,
            None,
            "chevron is a second channel, not its own control"
        );
    }

    #[test]
    fn select_does_not_fake_an_open_menu() {
        let node = select("theme", "Theme", "Dark");
        assert_eq!(node.semantics.expanded, None);
        assert!(
            !descendant_roles(&node)
                .iter()
                .any(|role| { matches!(role, Role::List | Role::Overlay | Role::Dialog) })
        );
        assert_eq!(
            node.children.len(),
            2,
            "closed field is value + chevron only"
        );
    }

    #[test]
    fn select_sm_is_32_and_lg_is_48() {
        let sm = select_sm("theme", "Theme", "Dark");
        assert_eq!(sm.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(sm.constraints.vertical.max, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(sm.semantics.role, Some(Role::Button));
        let lg = select_lg("theme", "Theme", "Dark");
        assert_eq!(lg.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(lg.constraints.vertical.max, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(lg.semantics.role, Some(Role::Button));
        assert_eq!(child(&lg, "chevron").props.text.as_deref(), Some("closed"));
    }
}
