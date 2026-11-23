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
///
/// A node with no counterpart names **every** node under it, not only
/// itself. `MeasureCache` is keyed by id, and a path that held a node some
/// frames ago can still have entries in it: naming only the newcomer's root
/// would let a descendant that came back at the same path answer from the
/// measurement its predecessor took.
pub(crate) fn graft(
    old: Option<&Arc<ViewNode>>,
    new: ViewNode,
    path: &mut KeyPath,
    changed: &mut Vec<String>,
) -> Arc<ViewNode> {
    graft_shared(old, Arc::new(new), path, changed)
}

/// [`graft`] over a shared `new`, so a subtree the caller handed back
/// unchanged (the same `Arc` as `old`) costs one pointer comparison and no
/// walk at all.
pub fn graft_shared(
    old: Option<&Arc<ViewNode>>,
    new: Arc<ViewNode>,
    path: &mut KeyPath,
    changed: &mut Vec<String>,
) -> Arc<ViewNode> {
    // A counterpart under another key is not a counterpart. Children are
    // paired by key already; this holds the root to the same rule, so a
    // caller that hands in last frame's tree under a renamed root gets the
    // whole tree named rather than its subtrees paired across two paths.
    let Some(old) = old.filter(|old| old.key == new.key) else {
        name_subtree(&new, path, changed);
        return new;
    };
    if Arc::ptr_eq(old, &new) {
        return new;
    }
    path.push(new.key.clone());
    let at = changed.len();
    let own_differs = !own_eq(old, &new);
    let keys_same = new
        .children
        .iter()
        .map(|c| &c.key)
        .eq(old.children.iter().map(|c| &c.key));
    if own_differs || !keys_same {
        changed.push(path.id());
    }
    let mut new = Arc::unwrap_or_clone(new);
    let fresh = std::mem::take(&mut new.children);
    let children: Vec<Arc<ViewNode>> = if keys_same {
        // The common case, a frame that kept every child in place: pair by
        // position and build no index.
        fresh
            .into_iter()
            .zip(&old.children)
            .map(|(child, counterpart)| graft_shared(Some(counterpart), child, path, changed))
            .collect()
    } else {
        let by_key: HashMap<&Key, &Arc<ViewNode>> = old
            .children
            .iter()
            .map(|child| (&child.key, child))
            .collect();
        fresh
            .into_iter()
            .map(|child| {
                let counterpart = by_key.get(&child.key).copied();
                graft_shared(counterpart, child, path, changed)
            })
            .collect()
    };
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

/// Push the id of `node` and of every node under it, in pre-order.
fn name_subtree(node: &ViewNode, path: &mut KeyPath, changed: &mut Vec<String>) {
    path.push(node.key.clone());
    changed.push(path.id());
    for child in &node.children {
        name_subtree(child, path, changed);
    }
    path.pop();
}

/// Whether two nodes declare the same fields, children aside.
///
/// Destructured field by field rather than compared through a clone with
/// the children stripped: the clone allocated every string a node owns, once
/// per node per frame, to throw it away. The destructuring has no `..`, so a
/// field added to [`ViewNode`] does not compile here until it is compared.
fn own_eq(a: &ViewNode, b: &ViewNode) -> bool {
    let ViewNode {
        kind,
        key,
        props,
        constraints,
        transition,
        interactions,
        semantics,
        ambient,
        children: _,
        component,
        behaviour,
        raw_claim,
        bindings,
        bound,
        reveal,
    } = a;
    *kind == b.kind
        && *key == b.key
        && *ambient == b.ambient
        && *raw_claim == b.raw_claim
        && *reveal == b.reveal
        && *constraints == b.constraints
        && *transition == b.transition
        && *component == b.component
        && *behaviour == b.behaviour
        && *interactions == b.interactions
        && *bindings == b.bindings
        && *bound == b.bound
        && *semantics == b.semantics
        && *props == b.props
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

    /// A subtree handed back as the same allocation is not walked: the
    /// result is that allocation, and nothing is named.
    #[test]
    fn a_shared_subtree_short_circuits_on_pointer_identity() {
        let old = Arc::new(row("x"));
        let mut changed = Vec::new();
        let kept = graft_shared(
            Some(&old),
            Arc::clone(&old),
            &mut KeyPath::root(),
            &mut changed,
        );
        assert!(Arc::ptr_eq(&kept, &old));
        assert!(changed.is_empty());
    }

    /// Children that swapped places are matched by key, not position.
    #[test]
    fn reordered_children_pair_by_key_and_keep_their_arcs() {
        let old = Arc::new(row("x"));
        let swapped = ViewNode::new(NodeKind::Stack, "row")
            .child(text("b", "x"))
            .child(text("a", "same"));
        let mut changed = Vec::new();
        let kept = graft(Some(&old), swapped, &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row"], "only the order moved");
        assert!(Arc::ptr_eq(&kept.children[0], &old.children[1]));
        assert!(Arc::ptr_eq(&kept.children[1], &old.children[0]));
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

        // A newcomer with children names all of them: a path that held a
        // node before must not answer from that node's measurement.
        let nested = row("x").child(ViewNode::new(NodeKind::Stack, "d").child(text("e", "deep")));
        let mut changed = Vec::new();
        graft(Some(&old), nested, &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row", "/row/d", "/row/d/e"]);

        let shrunk = ViewNode::new(NodeKind::Stack, "row").child(text("a", "same"));
        let mut changed = Vec::new();
        graft(Some(&old), shrunk, &mut KeyPath::root(), &mut changed);
        assert_eq!(changed, ["/row"]);
    }
}
