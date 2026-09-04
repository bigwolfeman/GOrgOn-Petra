//! Carbon Tree view (slice-f). No drag-to-reorder.
//!
//! Anatomy (`_treeview.scss` + usage page):
//! 1. [`tree_view`] — the hierarchy container (`Role::Tree`).
//! 2. Branch / leaf [`tree_item`] — `Role::TreeItem`.
//! 3. Caret as the word `"expanded"` / `"collapsed"`, never an icon-only
//!    mark (FR-026). Present on branches only.
//! 4. Node label.
//!
//! Nested children exist in the tree only while `expanded` is true.
//! Selection is `Semantics.selected` plus [`LAYER_SELECTED`], never colour
//! alone. Carbon does not ship drag-to-reorder; items do not declare
//! [`Interaction::Drag`].
//!
//! Default node height is Carbon small: 32. Extra-small 24 is
//! [`tree_item_xs`].

use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05, SURFACE_BASE, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{InsetRefs, Interaction, Key, Role, Semantics, ViewNode};

/// Carbon small / default node height.
const HEIGHT: f32 = 32.0;
/// Carbon extra-small node height.
const HEIGHT_XS: f32 = 24.0;

const _: () = assert!(HEIGHT == 32.0);
const _: () = assert!(HEIGHT_XS == 24.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A tree of items. `Role::Tree`, no interactions.
pub fn tree_view(key: impl Into<Key>, nodes: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, nodes);
    node.semantics = Semantics {
        role: Some(Role::Tree),
        ..Semantics::default()
    };
    node
}

/// One tree item at Carbon small (32).
///
/// `label` is required (FR-058). Nested `children` are mounted only when
/// `expanded` is true. Leaves still record `Semantics.expanded` because
/// the constructor takes the flag; they do not grow a caret.
pub fn tree_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    tree_item_sized(key, label, expanded, selected, children, HEIGHT)
}

/// [`tree_item`] at Carbon extra-small (24).
pub fn tree_item_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    tree_item_sized(key, label, expanded, selected, children, HEIGHT_XS)
}

fn tree_item_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
    height: f32,
) -> ViewNode {
    let label = label.into();
    let is_branch = !children.is_empty();
    let mut row_parts = Vec::new();
    if is_branch {
        let disclosure = if expanded { "expanded" } else { "collapsed" };
        row_parts.push(text("chevron", disclosure));
    }
    row_parts.push(text("label", label.clone()));

    let mut row = stack("row", Axis::Horizontal, Some(SPACING_03), row_parts);
    row.props.align = Some(Align::Center);
    row.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    row.constraints.vertical.min = Some(height);

    let mut parts = vec![row];
    if expanded && is_branch {
        let mut nest = stack("children", Axis::Vertical, None, children);
        nest.props.padding = Some(InsetRefs {
            left: Some(t(SPACING_05)),
            ..InsetRefs::default()
        });
        parts.push(nest);
    }

    let mut node = stack(key, Axis::Vertical, None, parts);
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    let mut node = node.interactive(Role::TreeItem, label, ITEM_INTENTS);
    node.semantics.selected = selected;
    node.semantics.expanded = Some(expanded);
    node
}

#[cfg(test)]
mod tests {
    use super::{HEIGHT, HEIGHT_XS, tree_item, tree_item_xs, tree_view};
    use crate::component::tokens::LAYER_SELECTED;
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

    fn has_key(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| has_key(child, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn no_drag(node: &ViewNode) {
        assert!(
            !node.interactions.contains(&Interaction::Drag),
            "tree item `{}` declared Drag; Carbon ships no reorder",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    #[test]
    fn tree_view_sets_role_tree() {
        let node = tree_view("fs", vec![tree_item("root", "src", true, false, vec![])]);
        assert_eq!(node.semantics.role, Some(Role::Tree));
        assert!(node.interactions.is_empty());
        assert_eq!(child_role(&node, "root"), Some(Role::TreeItem));
    }

    fn child_role(node: &ViewNode, key: &str) -> Option<Role> {
        named(node, key).semantics.role.clone()
    }

    #[test]
    fn tree_item_declares_expanded_and_selected_never_colour_only() {
        let on = tree_item(
            "src",
            "src",
            true,
            true,
            vec![tree_item("lib", "lib.rs", false, false, vec![])],
        );
        assert_eq!(on.semantics.role, Some(Role::TreeItem));
        assert_eq!(on.semantics.label.as_deref(), Some("src"));
        assert_eq!(on.semantics.expanded, Some(true));
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            named(&on, "chevron").props.text.as_deref(),
            Some("expanded")
        );
        assert_eq!(named(&on, "label").props.text.as_deref(), Some("src"));
        assert!(has_key(&on, "lib"));
        assert_eq!(named(&on, "row").constraints.vertical.min, Some(HEIGHT));
        assert_eq!(HEIGHT, 32.0);

        let off = tree_item(
            "src",
            "src",
            false,
            false,
            vec![tree_item("lib", "lib.rs", false, false, vec![])],
        );
        assert_eq!(off.semantics.expanded, Some(false));
        assert!(!off.semantics.selected);
        assert_eq!(
            named(&off, "chevron").props.text.as_deref(),
            Some("collapsed")
        );
        assert!(
            !has_key(&off, "lib"),
            "collapsed branch must not mount children"
        );
        assert!(
            !has_key(&off, "children"),
            "collapsed branch must not keep an empty nest"
        );
    }

    #[test]
    fn tree_items_do_not_declare_drag() {
        let node = tree_view(
            "fs",
            vec![tree_item(
                "src",
                "src",
                true,
                false,
                vec![tree_item("lib", "lib.rs", false, true, vec![])],
            )],
        );
        no_drag(&node);
        let leaf = tree_item("leaf", "README", false, false, vec![]);
        no_drag(&leaf);
        assert!(!has_key(&leaf, "chevron"), "a leaf has no expand caret");
    }

    #[test]
    fn extra_small_item_is_24() {
        let node = tree_item_xs("xs", "tiny", false, false, vec![]);
        assert_eq!(
            named(&node, "row").constraints.vertical.min,
            Some(HEIGHT_XS)
        );
        assert_eq!(HEIGHT_XS, 24.0);
        no_drag(&node);
    }
}
