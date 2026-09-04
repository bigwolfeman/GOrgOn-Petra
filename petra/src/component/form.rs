//! `form` — Carbon Form (slice-b): a labelled vertical stack of fields.
//!
//! Carbon documents Form as a spacing/label shell, not a control. There is
//! no `Role::Group`, so this constructor sets no role and no interactions.
//! Legend colour is `$text-secondary` → [`TEXT_MUTED`].
//!
//! Gap is [`SPACING_05`] (16). Carbon default form-item `margin-bottom` is
//! `$spacing-07` (32); the constructor contract pins spacing-05.

use super::stack;
use super::text::text;
use super::tokens::{SPACING_05, TEXT_MUTED, t};
use crate::geom::Axis;
use crate::tree::{Key, ViewNode};

/// A vertical field group with a muted legend and no role.
///
/// `children` are the fields. The legend is a `text` child keyed `"legend"`.
/// Not interactive: Form does not submit, focus, or click.
pub fn form(key: impl Into<Key>, legend: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    let mut legend_node = text("legend", legend);
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut rows = Vec::with_capacity(children.len() + 1);
    rows.push(legend_node);
    rows.extend(children);
    stack(key, Axis::Vertical, Some(SPACING_05), rows)
}

#[cfg(test)]
mod tests {
    use super::form;
    use crate::component::field::field;
    use crate::component::tokens::{SPACING_05, TEXT_MUTED};
    use crate::tree::{NodeKind, ViewNode};

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
    fn form_is_a_vertical_stack_with_spacing_05() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(crate::geom::Axis::Vertical));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_05)
        );
    }

    #[test]
    fn form_has_no_role_and_is_not_interactive() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        assert!(node.semantics.role.is_none());
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(node.semantics.label.is_none());
    }

    #[test]
    fn legend_is_muted_text() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        let legend = child(&node, "legend");
        assert_eq!(legend.kind, NodeKind::Text);
        assert_eq!(legend.props.text.as_deref(), Some("Account"));
        assert_eq!(token(legend, "foreground"), Some(TEXT_MUTED));
        let _ = child(&node, "name");
    }
}
