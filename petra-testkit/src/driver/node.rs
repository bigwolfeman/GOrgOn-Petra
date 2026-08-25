//! The client-side semantic tree shape.
//!
//! `contracts/semantic-tree.md` fixes the wire shape; the producer-side type
//! is [`gorgon_petra::semantic::SemanticNode`], which derives `Serialize`
//! only — it has no `Deserialize` impl, because nothing on the server side
//! ever needs to parse one back in. The client does, so this module carries
//! a parallel, `Deserialize`-only mirror of the same wire shape. Field names
//! and optionality match the contract exactly; [`gorgon_petra::tree::Role`]
//! and [`gorgon_petra::tree::Interaction`] are reused directly for `role`
//! and `actions` rather than re-declared, since both already round-trip the
//! exact wire spellings the contract fixes (`Role`'s hand-written
//! `Deserialize`, `Interaction`'s `#[serde(rename_all = "kebab-case")]`) —
//! redeclaring them here would be exactly the second-definition-of-one-shape
//! risk `wire.rs`'s own module docs warn against for placements.

use serde::Deserialize;

use gorgon_petra::tree::{Interaction, Role};

/// A node's rect in device pixels — the client-side mirror of
/// `gorgon_petra::semantic::Bounds`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub struct DriverBounds {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// The `state` block — client-side mirror of
/// `gorgon_petra::semantic::NodeState`. Every flag defaults to `false`/`None`
/// so a node whose author declared none of them still deserializes, matching
/// the server's own promise that the block is always emitted even at all
/// defaults.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DriverState {
    /// Holds keyboard focus this frame.
    pub focused: bool,
    /// Refuses interaction this frame.
    pub disabled: bool,
    /// Part of the current selection.
    pub selected: bool,
    /// Expansion state, `None` on non-expandables.
    pub expanded: Option<bool>,
    /// Content was hidden by a truncation rule this frame.
    pub truncated: bool,
    /// Content is bigger than the rect it was given, so the remainder is
    /// clipped away. Absent from the wire form when `false`.
    #[serde(default)]
    pub overflowed: bool,
    /// Rendered from a projection past its freshness bound.
    pub stale: bool,
    /// Hosts a declared-endless animation, settle-exempt.
    pub ambient: bool,
}

/// One semantic tree node, as received from the `tree` verb.
///
/// Mirrors `gorgon_petra::semantic::SemanticNode`'s wire shape field for
/// field; see the module docs for why this is a separate type rather than a
/// shared one.
#[derive(Debug, Clone, Deserialize)]
pub struct DriverNode {
    /// Stable node id (the canonical key-path).
    pub id: String,
    /// Absent when the node declares no role.
    #[serde(default)]
    pub role: Option<Role>,
    /// Human-readable name.
    pub label: String,
    /// Current value for inputs/status readouts.
    #[serde(default)]
    pub value: Option<String>,
    /// State flags in force this frame.
    #[serde(default)]
    pub state: DriverState,
    /// Device-pixel rect from this frame's placement.
    pub bounds: DriverBounds,
    /// The frame this node projects.
    pub frame_seq: u64,
    /// Driver action kinds this node accepts, sorted and deduplicated on the
    /// wire.
    #[serde(default)]
    pub actions: Vec<Interaction>,
    /// Rows behind a virtualized collection; present only when the node
    /// declares one.
    #[serde(default)]
    pub total_count: Option<usize>,
    /// Children in focus/reading order.
    #[serde(default)]
    pub children: Vec<DriverNode>,
}

impl DriverNode {
    /// Depth-first pre-order walk of this node and its descendants — a local
    /// convenience for a tree already in hand. The finders in
    /// [`super::client`] do not use this: they ask the server to filter, per
    /// `contracts/driver-protocol.md`'s "Finders ... built on the `tree`
    /// verb", which is authoritative over a client-side re-search of a
    /// possibly-stale local copy.
    #[must_use]
    pub fn iter(&self) -> DriverPreOrder<'_> {
        DriverPreOrder { stack: vec![self] }
    }

    /// The descendant with `id`, or this node when it matches, searched
    /// locally over whatever subtree is already in hand.
    #[must_use]
    pub fn find_local(&self, id: &str) -> Option<&Self> {
        self.iter().find(|node| node.id == id)
    }

    /// Whether this node can take keyboard focus this frame: it accepts
    /// [`Interaction::Focus`] and is not disabled.
    #[must_use]
    pub fn is_focusable(&self) -> bool {
        self.actions.contains(&Interaction::Focus) && !self.state.disabled
    }
}

/// Depth-first pre-order walk of a [`DriverNode`].
#[derive(Debug)]
pub struct DriverPreOrder<'a> {
    stack: Vec<&'a DriverNode>,
}

impl<'a> Iterator for DriverPreOrder<'a> {
    type Item = &'a DriverNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        self.stack.extend(node.children.iter().rev());
        Some(node)
    }
}

/// A `tree` verb query. `id` narrows the search space before any of the
/// other filters apply (`dispatch.rs`: "`{id, role}` means 'nodes with this
/// role under this subtree'"). An entirely-default query (no filters at all)
/// is what makes the server answer with one node instead of an array; see
/// [`TreeAnswer`] and `contracts/driver-protocol.md`'s asymmetry.
#[derive(Debug, Clone, Default)]
pub struct TreeQuery {
    /// Narrow the search to the subtree rooted at this id first.
    pub id: Option<String>,
    /// Match this role's wire spelling exactly (e.g. `"button"`,
    /// `"custom:gutter"`).
    pub role: Option<String>,
    /// Match nodes whose label contains this substring.
    pub label_contains: Option<String>,
    /// Match nodes carrying this state flag
    /// (`focused`/`disabled`/`selected`/`expanded`/`truncated`/`stale`/`ambient`).
    pub state: Option<String>,
}

impl TreeQuery {
    /// An empty query: the full tree, or the subtree named by `id` alone.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Narrow to the subtree rooted at `id`.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Filter by role's wire spelling.
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Filter by label substring.
    #[must_use]
    pub fn with_label_contains(mut self, needle: impl Into<String>) -> Self {
        self.label_contains = Some(needle.into());
        self
    }

    /// Filter by state flag name.
    #[must_use]
    pub fn with_state(mut self, flag: impl Into<String>) -> Self {
        self.state = Some(flag.into());
        self
    }

    /// Whether any filter (role/label/state) is set. Deliberately excludes
    /// `id`: this is the same rule `dispatch.rs`'s `tree` handler uses to
    /// decide single-node vs. array — `{id}` alone is unfiltered.
    #[must_use]
    pub fn is_filtered(&self) -> bool {
        self.role.is_some() || self.label_contains.is_some() || self.state.is_some()
    }

    pub(super) fn to_params(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        if let Some(id) = &self.id {
            map.insert("id".to_owned(), serde_json::Value::String(id.clone()));
        }
        if let Some(role) = &self.role {
            map.insert("role".to_owned(), serde_json::Value::String(role.clone()));
        }
        if let Some(needle) = &self.label_contains {
            map.insert(
                "label_contains".to_owned(),
                serde_json::Value::String(needle.clone()),
            );
        }
        if let Some(state) = &self.state {
            map.insert("state".to_owned(), serde_json::Value::String(state.clone()));
        }
        serde_json::Value::Object(map)
    }

    /// A short, human-readable description of this query, for error
    /// messages (`DriverError::NotExactlyOneMatch`'s `query` field).
    #[must_use]
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(id) = &self.id {
            parts.push(format!("id == {id:?}"));
        }
        if let Some(role) = &self.role {
            parts.push(format!("role == {role:?}"));
        }
        if let Some(needle) = &self.label_contains {
            parts.push(format!("label contains {needle:?}"));
        }
        if let Some(state) = &self.state {
            parts.push(format!("state == {state:?}"));
        }
        if parts.is_empty() {
            "(unfiltered)".to_owned()
        } else {
            parts.join(" and ")
        }
    }
}

/// The `tree` verb's asymmetric result shape
/// (`contracts/driver-protocol.md`, `dispatch.rs`'s own doc comment): an
/// **unfiltered** query answers one node, a **filtered** one answers an
/// array — never the reverse, and never guessed at by shape alone at the
/// call site.
#[derive(Debug, Clone)]
pub enum TreeAnswer {
    /// The full tree, or the subtree named by `id`, for an unfiltered query.
    Node(DriverNode),
    /// Every match, for a filtered query.
    Matches(Vec<DriverNode>),
}

impl TreeAnswer {
    /// The single node, or an error naming the actual (wrong) shape.
    pub fn into_node(self) -> Result<DriverNode, super::error::DriverError> {
        match self {
            Self::Node(node) => Ok(node),
            Self::Matches(matches) => Err(super::error::DriverError::UnexpectedShape(format!(
                "expected a single node, got an array of {} matches",
                matches.len()
            ))),
        }
    }

    /// The array of matches, or an error naming the actual (wrong) shape.
    pub fn into_matches(self) -> Result<Vec<DriverNode>, super::error::DriverError> {
        match self {
            Self::Matches(matches) => Ok(matches),
            Self::Node(_) => Err(super::error::DriverError::UnexpectedShape(
                "expected an array of matches, got a single node".to_owned(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DriverNode, TreeQuery};

    #[test]
    fn a_node_deserializes_from_the_contracts_wire_shape() {
        let json = serde_json::json!({
            "id": "/root/go",
            "role": "button",
            "label": "Go",
            "state": {},
            "bounds": {"x": 0, "y": 0, "w": 40, "h": 20},
            "frame_seq": 3,
            "actions": ["click", "focus"],
            "children": []
        });
        let node: DriverNode = serde_json::from_value(json).expect("parses");
        assert_eq!(node.id, "/root/go");
        assert_eq!(node.label, "Go");
        assert!(node.is_focusable());
        assert!(!node.state.disabled);
    }

    #[test]
    fn a_custom_role_round_trips_through_deserialize() {
        let json = serde_json::json!({
            "id": "/root/gutter",
            "role": "custom:gutter",
            "label": "Gutter",
            "state": {},
            "bounds": {"x": 0, "y": 0, "w": 1, "h": 1},
            "frame_seq": 1,
            "actions": [],
            "children": []
        });
        let node: DriverNode = serde_json::from_value(json).expect("parses");
        assert_eq!(
            node.role.as_ref().map(gorgon_petra::tree::Role::as_wire),
            Some("custom:gutter".to_owned())
        );
    }

    #[test]
    fn is_filtered_ignores_id_alone() {
        let unfiltered = TreeQuery::new().with_id("/root");
        assert!(!unfiltered.is_filtered());
        let filtered = TreeQuery::new().with_id("/root").with_role("button");
        assert!(filtered.is_filtered());
    }

    #[test]
    fn pre_order_visits_the_node_then_its_children_in_order() {
        let json = serde_json::json!({
            "id": "/root",
            "label": "",
            "state": {},
            "bounds": {"x": 0, "y": 0, "w": 1, "h": 1},
            "frame_seq": 1,
            "actions": [],
            "children": [
                {"id": "/root/a", "label": "a", "state": {}, "bounds": {"x":0,"y":0,"w":1,"h":1}, "frame_seq": 1, "actions": [], "children": []},
                {"id": "/root/b", "label": "b", "state": {}, "bounds": {"x":0,"y":0,"w":1,"h":1}, "frame_seq": 1, "actions": [], "children": []}
            ]
        });
        let node: DriverNode = serde_json::from_value(json).expect("parses");
        let ids: Vec<&str> = node.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["/root", "/root/a", "/root/b"]);
        assert!(node.find_local("/root/b").is_some());
        assert!(node.find_local("/root/z").is_none());
    }
}
