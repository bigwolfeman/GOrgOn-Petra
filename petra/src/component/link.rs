//! `link` — Carbon Link (slice-c): an activatable text control.
//!
//! There is no `Role::Link`. The activatable control is [`Role::Button`].
//! Colour is not the only channel: role + label name it. No underline token
//! exists in `tokens.rs`, so none is bound. Quiet fill is [`SURFACE_BASE`]
//! so [`super::on_layer`] can disappear the control into its ground.
//!
//! Carbon `$link-primary` is not in the component token list; ink is
//! [`TEXT_PRIMARY`].

use super::stack;
use super::text::text;
use super::tokens::{SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, Role, ViewNode};

/// A navigational text control. `label` is required (FR-058).
///
/// Focus and Click only. Carbon underline-on-hover has no token here.
pub fn link(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click],
    )
}

#[cfg(test)]
mod tests {
    use super::link;
    use crate::component::tokens::{SURFACE_BASE, TEXT_PRIMARY};
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

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
    fn link_is_a_labelled_button() {
        let node = link("docs", "Open docs");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open docs"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(!node.interactions.contains(&Interaction::Drag));
        assert_eq!(token(&node, "background"), Some(SURFACE_BASE));
        assert!(node.props.tokens.get("border").is_none());
        let caption = child(&node, "label");
        assert_eq!(caption.props.text.as_deref(), Some("Open docs"));
        assert_eq!(token(caption, "foreground"), Some(TEXT_PRIMARY));
    }
}
