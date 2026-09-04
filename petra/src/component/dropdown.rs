//! Carbon Dropdown (slice-b). Closed field plus an open list on Popover.
//!
//! Anatomy (`_dropdown.scss` + `_list-box.scss`):
//! 1. Field — [`SURFACE_RAISED`] + [`BORDER_SUBTLE`], height md 40.
//! 2. Current value (visible text).
//! 3. Chevron as the word `"closed"` / `"open"` — never an icon-only mark
//!    (FR-026).
//! 4. Open menu — [`super::popover::popover_with`] listing option rows.
//! 5. Option — [`Role::Button`] + `Semantics.selected`. Selected is also
//!    the word `"selected"` and [`LAYER_SELECTED`], never a hue alone.
//!
//! [`dropdown`] is the closed field (like [`super::select`]). [`dropdown_open`]
//! wraps that field and a popover of caller-supplied option nodes. Combo box
//! and Multiselect are omitted (clear icon + tags).

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SIZE_MD,
    SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const FIELD_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed dropdown at Carbon md (40). `label` is the accessible name;
/// `value` is the visible current option.
pub fn dropdown(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    closed_field(key, label, value, "closed", None)
}

/// Open dropdown: the closed field plus a popover listing `options`.
///
/// The field child is keyed `"field"`; the popover is keyed `"menu"` and
/// anchored to `"field"`. Callers that place this node under a parent must
/// keep that child key so [`Anchor::Node`](crate::tree::Anchor) can name it.
pub fn dropdown_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    options: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field("field", label.clone(), value, "open", Some(true));
    let menu = popover_with("menu", label, "field", options);
    let mut node = stack(key, Axis::Vertical, None, vec![field, menu]);
    node.semantics.expanded = Some(true);
    node
}

/// One option row. `selected` is a declared fact plus the word `"selected"`
/// and [`LAYER_SELECTED`] — never colour alone.
pub fn dropdown_option(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut parts = vec![caption];
    if selected {
        let mut mark = text("mark", "selected");
        mark.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
        parts.push(mark);
    }
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), parts);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            LAYER_SELECTED
        } else {
            SURFACE_RAISED
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
    let mut node = node.interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.selected = selected;
    node
}

fn closed_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    chevron: &'static str,
    expanded: Option<bool>,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron_node = text("chevron", chevron);
    chevron_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, chevron_node],
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
    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.expanded = expanded;
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
    use super::{SIZE_MD, dropdown, dropdown_open, dropdown_option};
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED, SURFACE_RAISED};
    use crate::tree::{Anchor, Interaction, NodeKind, Role, ViewNode};

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

    #[test]
    fn dropdown_is_a_closed_button_at_height_40() {
        let node = dropdown("theme", "Theme", "Dark");
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
        assert!(child(&node, "chevron").semantics.role.is_none());
        assert_eq!(node.semantics.expanded, None);
        assert_ne!(node.semantics.role, Some(Role::Overlay));
    }

    #[test]
    fn dropdown_open_hosts_options_in_a_popover() {
        let node = dropdown_open(
            "theme",
            "Theme",
            "Dark",
            vec![
                dropdown_option("dark", "Dark", true),
                dropdown_option("light", "Light", false),
            ],
        );
        assert_eq!(node.semantics.expanded, Some(true));
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.semantics.expanded, Some(true));
        assert_eq!(child(field, "chevron").props.text.as_deref(), Some("open"));

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("Theme"));
        match &menu.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "field"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(menu, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        let dark = child(content, "dark");
        assert_eq!(dark.semantics.role, Some(Role::Button));
        assert!(dark.semantics.selected);
        assert_eq!(child(dark, "mark").props.text.as_deref(), Some("selected"));
        assert_eq!(token(dark, "background"), Some(LAYER_SELECTED));
        let light = child(content, "light");
        assert_eq!(light.semantics.role, Some(Role::Button));
        assert!(!light.semantics.selected);
        assert!(light.children.iter().all(|c| c.key.as_str() != "mark"));
    }

    #[test]
    fn dropdown_option_sets_role_label_and_selected() {
        let on = dropdown_option("dark", "Dark", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("Dark"));
        assert!(on.semantics.selected);
        assert!(on.interactions.contains(&Interaction::Click));
        let off = dropdown_option("light", "Light", false);
        assert!(!off.semantics.selected);
        assert_eq!(off.semantics.label.as_deref(), Some("Light"));
    }
}
