//! `list_box` — the beakless panel a Carbon list box opens under its field.
//!
//! Carbon's `ListBox` menu (`_list-box.scss`, the surface Dropdown, Select,
//! Combo box and the pagination pickers all open) is a `$layer` panel with a
//! `box-shadow` and **no** caret beak: it butts against the field's bottom
//! edge and starts at the field's start edge. [`super::popover_with`] is the
//! other anchored surface, and it is right for Popover, Toggletip and
//! Tooltip, which do point at their trigger with a beak — see
//! `.agents/carbon-waves/ORCHESTRATOR-FINDINGS.md`, "Both panels draw a
//! caret beak". This is the surface for the list boxes.
//!
//! A [`Role::Overlay`] on [`Layer::Popup`], anchored to a sibling by key
//! ([`Anchor::Sibling`], bottom edge, start-aligned), flipped when it would
//! run off the window, dismissed by a press outside. Rows are the caller's;
//! build them with [`super::dropdown::dropdown_option`] or
//! [`super::menu::menu_item`].

use super::stack;
use super::tokens::{SHADOW_RAISED, SURFACE_RAISED, t};
use crate::geom::{Align, Axis};
use crate::tree::{
    Align as AnchorAlign, Anchor, ClampRule, Edge, InputPolicy, Key, Layer, NodeKind, Props, Role,
    Semantics, ViewNode,
};

/// An anchored, beakless list panel. `label` is the accessible name; `anchor`
/// is the key of the field it hangs under, which must sit in the same child
/// list as this node. `rows` fill the panel's width.
pub fn list_box(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor: impl Into<Key>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let mut content = stack("content", Axis::Vertical, None, rows);
    content.props.align = Some(Align::Stretch);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: anchor.into(),
                edge: Edge::Bottom,
                align: AnchorAlign::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::list_box;
    use crate::component::dropdown::dropdown_option;
    use crate::component::tokens::{SHADOW_RAISED, SURFACE_RAISED};
    use crate::tree::{Anchor, Edge, InputPolicy, Layer, NodeKind, Role};

    #[test]
    fn a_list_box_is_a_beakless_popup_anchored_under_its_field_start() {
        let node = list_box(
            "menu",
            "Items per page",
            "field",
            vec![dropdown_option("s10", "10", true)],
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Items per page"));
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        match &node.props.anchor {
            Some(Anchor::Sibling {
                key, edge, align, ..
            }) => {
                assert_eq!(key.as_str(), "field");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(*align, crate::tree::Align::Start);
            }
            other => panic!("not a sibling anchor: {other:?}"),
        }
        let content = node.children.first().expect("content");
        assert!(
            content
                .children
                .iter()
                .all(|c| c.props.text.as_deref() != Some("^")),
            "a list box has no beak"
        );
        assert_eq!(content.children.len(), 1, "only the caller's rows");
        assert_eq!(
            node.props.tokens.get("background").map(|t| t.as_str()),
            Some(SURFACE_RAISED)
        );
        assert_eq!(
            node.props.tokens.get("shadow").map(|t| t.as_str()),
            Some(SHADOW_RAISED)
        );
        assert!(
            !node.props.tokens.contains_key("border"),
            "Carbon's list box is a shadowed layer, not a boxed one"
        );
    }
}
