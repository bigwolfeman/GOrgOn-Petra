//! Name to constructor, the one place a component is built.
//!
//! `specs/005-petra-carbon-authoring/contracts/surface-contribution.md` §3
//! originally had a plugin's Lua builders compose primitives themselves. That
//! reading did not survive contact with the library: there are 180 public
//! constructors across the 42 components, so composing them in Lua means 180
//! second implementations in a second language, free to drift from the ones
//! that actually draw.
//!
//! Instead a plugin names a constructor and its parameters, and this registry
//! is what turns that name into the shipped `ViewNode`. One implementation.
//! Drift is not caught, it is impossible.
//!
//! # Where expansion happens
//!
//! On the shell side, immediately before stage-2 `validate`. Not in the
//! daemon: a component reference is a fraction of the size of the tree it
//! expands to, and a full tree is re-sent per publication under a 16 MiB ctl
//! frame cap. The daemon still refuses an unknown name or a bad parameter
//! table synchronously, on the fiber's own call, using [`shape_of`] — the
//! guardrail binds before mount, it is only the expansion that is deferred.
//!
//! `NodeKind::Component` therefore never reaches layout or the frame digest.
//! `validate` refuses an unexpanded one by name rather than ignoring it.

use std::collections::BTreeMap;

use crate::component::params::ParamError;
use crate::tree::ViewNode;

mod containment;
mod data;
mod feedback;
mod form;
mod navigation;

/// Build one component from its wire parameter table.
pub type Ctor = fn(&serde_json::Value) -> Result<ViewNode, ParamError>;

/// One registry row: the name a plugin writes, how to build it, and the Luau
/// type its parameter table has in `plugin.d.luau`.
#[derive(Clone, Copy)]
pub struct Entry {
    /// The name a plugin writes, matching the Rust constructor exactly.
    pub name: &'static str,
    /// Builds the node.
    pub ctor: Ctor,
    /// The rendered Luau parameter type, from `ParamShape::LUAU`.
    pub luau: &'static str,
}

/// The five group files, merged by [`entries`].
const GROUPS: &[(&str, &[Entry])] = &[
    ("containment", containment::ENTRIES),
    ("navigation", navigation::ENTRIES),
    ("form", form::ENTRIES),
    ("feedback", feedback::ENTRIES),
    ("data", data::ENTRIES),
];

/// The merged table, built once.
///
/// `expand` calls [`lookup`] once per component node and the daemon's stage-1
/// walk calls it again for every node it finds inside a parameter table, so a
/// per-call rebuild of a 180-row vector with an O(n²) uniqueness assert would
/// be paid on every node of every contribution. The merge and its checks
/// happen once per process instead, and `lookup` becomes a hash probe.
static TABLE: std::sync::OnceLock<BTreeMap<&'static str, Entry>> = std::sync::OnceLock::new();

fn table() -> &'static BTreeMap<&'static str, Entry> {
    TABLE.get_or_init(|| {
        let mut out: BTreeMap<&'static str, Entry> = BTreeMap::new();
        for (group, rows) in GROUPS {
            for row in *rows {
                assert!(
                    out.insert(row.name, *row).is_none(),
                    "component `{}` is registered twice; `{group}` cannot re-register a name \
                     another group already owns",
                    row.name
                );
            }
        }
        out
    })
}

/// Every registered constructor, in name order.
///
/// # Panics
/// When two groups register one name. That is a merge error, never a
/// last-one-wins: the rows are written by different hands and a silent shadow
/// would give a plugin whichever module happened to be linked last.
#[must_use]
pub fn entries() -> Vec<Entry> {
    table().values().copied().collect()
}

/// Look one constructor up by name. A hash probe, not a scan.
#[must_use]
pub fn lookup(name: &str) -> Option<Entry> {
    table().get(name).copied()
}

/// Build `name` from `params`, or say why not.
///
/// # Errors
/// When no such constructor is registered, or the parameter table does not
/// deserialize into that constructor's shape.
pub fn build(name: &str, params: &serde_json::Value) -> Result<ViewNode, ParamError> {
    let entry = lookup(name).ok_or_else(|| ParamError {
        component: name.to_owned(),
        reason: format!("no component named `{name}`; see docs/catalogs/components.md"),
    })?;
    (entry.ctor)(params)
}

/// The Luau parameter type for `name`, for stub generation and for the
/// daemon's stage-1 refusal message.
#[must_use]
pub fn shape_of(name: &str) -> Option<&'static str> {
    lookup(name).map(|e| e.luau)
}

/// How deep a contributed tree may nest before expansion refuses it.
///
/// Expansion recurses over attacker-controlled input: the tree comes from a
/// plugin, and `params` may itself hold nodes holding nodes. A depth limit is
/// what keeps a hostile or merely broken plugin from expanding the stack
/// instead of the tree. The number is generous against real surfaces — the
/// deepest shipped component nests six — and small enough to fail long before
/// the stack does.
pub const MAX_DEPTH: usize = 128;

/// Rewrite every component reference in `node` into the subtree its
/// constructor builds.
///
/// Bottom-up: a component's children arrive inside its `params`, so those are
/// expanded first and the constructor is handed real nodes. The result is
/// primitives only, because a constructor has no way to name another
/// component — the registry is not reachable from `crate::component`'s
/// constructors, only from here.
///
/// # Errors
/// When a reference names no registered constructor, when a parameter table
/// does not fit its constructor's shape, or when the tree nests deeper than
/// [`MAX_DEPTH`].
pub fn expand(node: &ViewNode) -> Result<ViewNode, ParamError> {
    expand_node(node, 0)
}

fn too_deep(what: &str) -> ParamError {
    ParamError {
        component: what.to_owned(),
        reason: format!(
            "contributed tree nests deeper than {MAX_DEPTH}; expansion refuses it rather than \
             recursing further"
        ),
    }
}

fn expand_node(node: &ViewNode, depth: usize) -> Result<ViewNode, ParamError> {
    if depth >= MAX_DEPTH {
        return Err(too_deep(node.key.as_str()));
    }
    if node.kind == crate::tree::NodeKind::Component {
        let reference = node.component.as_ref().ok_or_else(|| ParamError {
            component: node.key.as_str().to_owned(),
            reason: "kind is `component` but no component reference is declared; `validate` \
                     should have refused this tree before expansion"
                .to_owned(),
        })?;
        let params = expand_params(&reference.params, depth + 1)?;
        // The built subtree is primitives: a constructor in `crate::component`
        // cannot name another component, so there is nothing left to expand
        // and no second walk to pay for.
        return build(&reference.name, &params);
    }
    let mut out = node.clone();
    out.children = node
        .children
        .iter()
        .map(|child| expand_node(child, depth + 1).map(std::sync::Arc::new))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// Expand any component references embedded in a parameter table.
///
/// Parameters are still JSON here, so a nested node is an object carrying
/// `"kind": "component"`. Walking JSON rather than deserializing first is what
/// lets one function serve every parameter shape: a shape holding
/// `Vec<ViewNode>` and one holding a single `ViewNode` need no separate case.
fn expand_params(
    params: &serde_json::Value,
    depth: usize,
) -> Result<serde_json::Value, ParamError> {
    if depth >= MAX_DEPTH {
        return Err(too_deep("<params>"));
    }
    match params {
        serde_json::Value::Object(fields) => {
            if fields.get("kind").and_then(serde_json::Value::as_str) == Some("component") {
                let node: ViewNode =
                    serde_json::from_value(params.clone()).map_err(|err| ParamError {
                        component: "<nested>".to_owned(),
                        reason: format!("nested component reference does not deserialize: {err}"),
                    })?;
                let built = expand_node(&node, depth)?;
                return serde_json::to_value(built).map_err(|err| ParamError {
                    component: "<nested>".to_owned(),
                    reason: format!("expanded subtree does not re-serialize: {err}"),
                });
            }
            let mut out = serde_json::Map::with_capacity(fields.len());
            for (name, value) in fields {
                out.insert(name.clone(), expand_params(value, depth + 1)?);
            }
            Ok(serde_json::Value::Object(out))
        }
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| expand_params(item, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(serde_json::Value::Array),
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tree::NodeKind;
    use crate::tree::node::ComponentRef;

    fn reference(key: &str, name: &str, params: serde_json::Value) -> ViewNode {
        let mut node = ViewNode::new(NodeKind::Component, key);
        node.component = Some(ComponentRef {
            name: name.to_owned(),
            params,
        });
        node
    }

    /// Two groups registering one name is a merge error, not a last-one-wins.
    /// The rows are written by different hands; a silent shadow would give a
    /// plugin whichever module happened to be linked last.
    #[test]
    fn every_registered_name_is_unique() {
        let names: Vec<&str> = entries().iter().map(|e| e.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            names.len(),
            "a component name is registered twice"
        );
    }

    /// A tree of primitives is returned unchanged. Expansion is not allowed to
    /// be a rewrite pass that quietly normalizes something else on the way
    /// through.
    #[test]
    fn a_tree_with_no_references_is_returned_unchanged() {
        let mut root = ViewNode::new(NodeKind::Stack, "root");
        root.children = vec![
            std::sync::Arc::new(ViewNode::new(NodeKind::Text, "a")),
            std::sync::Arc::new(ViewNode::new(NodeKind::Spacer, "b")),
        ];
        assert_eq!(expand(&root).expect("primitives expand"), root);
    }

    /// An unregistered name refuses, and the message names it. A plugin author
    /// reading the refusal has to learn which call was wrong.
    #[test]
    fn an_unregistered_name_refuses_and_says_which() {
        let err = expand(&reference("x", "definitely_not_a_component", json!({})))
            .expect_err("no such component");
        assert_eq!(err.component, "definitely_not_a_component");
        assert!(
            err.reason.contains("definitely_not_a_component"),
            "the reason must name the component, got: {}",
            err.reason
        );
    }

    /// The recursion reaches inside `params`, not only `children`. A
    /// component's children arrive as parameters, so a walk that only
    /// descended `node.children` would leave nested references unexpanded and
    /// they would reach layout, where `place_kind` panics.
    #[test]
    fn a_reference_nested_inside_params_is_reached() {
        let outer = reference(
            "outer",
            "also_not_real",
            json!({ "children": [ { "kind": "component", "key": "inner",
                                    "component": { "name": "nested_not_real", "params": {} } } ] }),
        );
        let err = expand(&outer).expect_err("the nested reference is reached first");
        assert_eq!(
            err.component, "nested_not_real",
            "expansion must be bottom-up: the nested name fails before the outer one"
        );
    }

    /// Expansion recurses over plugin-supplied input. Depth is capped so a
    /// hostile or merely broken tree exhausts the limit rather than the stack.
    #[test]
    fn a_tree_deeper_than_the_limit_refuses_rather_than_recursing() {
        let mut node = ViewNode::new(NodeKind::Stack, "leaf");
        for i in 0..=MAX_DEPTH {
            let mut parent = ViewNode::new(NodeKind::Stack, format!("n{i}"));
            parent.children = vec![std::sync::Arc::new(node)];
            node = parent;
        }
        let err = expand(&node).expect_err("deeper than MAX_DEPTH");
        assert!(err.reason.contains("nests deeper"), "got: {}", err.reason);
    }

    /// A tree exactly at the limit still expands. A cap that fired one node
    /// early would refuse legitimate surfaces and be discovered in production.
    #[test]
    fn a_tree_at_the_limit_still_expands() {
        let mut node = ViewNode::new(NodeKind::Stack, "leaf");
        for i in 0..MAX_DEPTH - 1 {
            let mut parent = ViewNode::new(NodeKind::Stack, format!("n{i}"));
            parent.children = vec![std::sync::Arc::new(node)];
            node = parent;
        }
        assert!(
            expand(&node).is_ok(),
            "a tree at MAX_DEPTH must still expand"
        );
    }

    /// `kind = component` with nothing to expand is a tree `validate` should
    /// already have refused. Expansion says so rather than panicking, because
    /// it also runs from the daemon's own tests.
    #[test]
    fn a_component_kind_with_no_reference_refuses() {
        let bare = ViewNode::new(NodeKind::Component, "bare");
        let err = expand(&bare).expect_err("no reference to expand");
        assert!(
            err.reason.contains("no component reference"),
            "got: {}",
            err.reason
        );
    }
}
