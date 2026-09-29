//! Keep what did not change: graft a regrown subtree onto the one it
//! replaces, reusing every equal descendant's `Arc` and naming the nodes
//! that actually differ.
//!
//! A re-expanded component builds every node afresh, so without this every
//! node under it would carry a new `Arc` and `layout::reuse` would have to
//! re-place the whole subtree even when one knob moved. With it, a node
//! whose own fields and children are unchanged hands back the old `Arc` —
//! so its placements and Merkle hash are reused as-is — and only the nodes
//! whose content moved are named for invalidation
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).

use std::collections::HashMap;
use std::sync::Arc;

use super::key::{Key, KeyPath};
use super::node::ViewNode;

/// Graft `new` onto `old`, matching children by key.
///
/// `path` holds the ancestors of the node being grafted (the node's own key
/// is pushed here). Every canonical id whose node is new, whose own fields
/// differ, or whose child key list differs is pushed to `changed`, in tree
/// pre-order; a node that differs only because a descendant differs gets a
/// new `Arc` but is not named — `ChangeSet::Nodes` already reaches it as an
/// ancestor of the named descendant.
pub(crate) fn graft(
    old: Option<&Arc<ViewNode>>,
    new: ViewNode,
    path: &mut KeyPath,
    changed: &mut Vec<String>,
) -> Arc<ViewNode> {
    path.push(new.key.clone());
    let Some(old) = old else {
        // A node with no counterpart: naming it covers its whole subtree.
        changed.push(path.id());
        path.pop();
        return Arc::new(new);
    };
    let at = changed.len();
    let own_differs = !own_eq(old, &new);
    let keys_differ = !new
        .children
        .iter()
        .map(|c| &c.key)
        .eq(old.children.iter().map(|c| &c.key));
    if own_differs || keys_differ {
        changed.push(path.id());
    }
    let by_key: HashMap<&Key, &Arc<ViewNode>> = old
        .children
        .iter()
        .map(|child| (&child.key, child))
        .collect();
    let mut new = new;
    let children: Vec<Arc<ViewNode>> = std::mem::take(&mut new.children)
        .into_iter()
        .map(|child| {
            let counterpart = by_key.get(&child.key).copied();
            graft(counterpart, Arc::unwrap_or_clone(child), path, changed)
        })
        .collect();
    let same_children = children.len() == old.children.len()
        && children
            .iter()
            .zip(&old.children)
            .all(|(a, b)| Arc::ptr_eq(a, b));
    path.pop();
    if !own_differs && same_children {
        changed.truncate(at);
        return Arc::clone(old);
    }
    new.children = children;
    Arc::new(new)
}

/// Whether two nodes declare the same fields, children aside.
fn own_eq(a: &ViewNode, b: &ViewNode) -> bool {
    let strip = |n: &ViewNode| ViewNode {
        children: Vec::new(),
        ..n.clone()
    };
    strip(a) == strip(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::NodeKind;

    fn text(key: &str, body: &str) -> ViewNode {
        let mut node = ViewNode::new(NodeKind::Text, key);
        node.props.text = Some(body.to_owned());
        node
    }

    fn row(knob: &str) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "row")
            .child(text("a", "same"))
            .child(text("b", knob))
    }

    #[test]
    fn an_identical_regrowth_hands_back_the_old_arc_and_names_nothing() {
        let old = Arc::new(row("x"));
        let mut changed = Vec::new();
        let kept = graft(Some(&old), row("x"), &mut KeyPath::root(), &mut changed);
        assert!(Arc::ptr_eq(&kept, &old));
        assert!(changed.is_empty(), "{changed:?}");
    }

    #[test]
    fn one_moved_leaf_is_named_and_its_sibling_keeps_its_arc() {
        let old = Arc::new(row("x"));
        let mut changed = Vec::new();
        let kept = graft(Some(&old), row("y"), &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row/b"]);
        assert!(!Arc::ptr_eq(&kept, &old), "the parent carries a new child");
        assert!(Arc::ptr_eq(&kept.children[0], &old.children[0]));
    }

    #[test]
    fn a_new_or_dropped_child_names_its_parent_and_the_newcomer() {
        let old = Arc::new(row("x"));
        let grown = row("x").child(text("c", "new"));
        let mut changed = Vec::new();
        graft(Some(&old), grown, &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row", "/row/c"]);

        let shrunk = ViewNode::new(NodeKind::Stack, "row").child(text("a", "same"));
        let mut changed = Vec::new();
        graft(Some(&old), shrunk, &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row"]);
    }
}
