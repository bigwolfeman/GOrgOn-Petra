//! Filters over a projected tree.
//!
//! The contract's `tree` verb takes "optional filters (role, label substring,
//! state flag)". This module is the *predicate* half of that, shared so the
//! driver's finders, the agent query, and a gate all decide "does this node
//! match" the same way. What a filtered `tree` **response** looks like — a
//! flat list of matches, or the tree pruned to matches and their ancestors —
//! is the driver protocol's call, not the projection's, and is not decided
//! here.

use crate::semantic::node::SemanticNode;

/// One state flag a query can select on.
///
/// One variant per [`crate::semantic::NodeState`] member, and no variant
/// without one. A variant that read no field would be a filter that lies —
/// it would answer "no match" to every query, indistinguishable from a real
/// answer — so a variant lands here only once the projection carries the
/// field it reads. `hovered`, `active` and `captured` read real projected
/// fields as of `frame-v6`; nothing sets them non-`false` until `LayoutState`
/// carries the pointer snapshot, so those three match nothing yet for a
/// reason a caller can act on rather than because the filter is fictional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateFlag {
    /// [`crate::semantic::NodeState::focused`].
    Focused,
    /// [`crate::semantic::NodeState::hovered`].
    Hovered,
    /// [`crate::semantic::NodeState::active`].
    Active,
    /// [`crate::semantic::NodeState::captured`].
    Captured,
    /// [`crate::semantic::NodeState::read_only`].
    ReadOnly,
    /// [`crate::semantic::NodeState::skeleton`].
    Skeleton,
    /// [`crate::semantic::NodeState::disabled`].
    Disabled,
    /// [`crate::semantic::NodeState::selected`].
    Selected,
    /// [`crate::semantic::NodeState::expanded`] is `Some(true)`.
    Expanded,
    /// [`crate::semantic::NodeState::truncated`].
    Truncated,
    /// [`crate::semantic::NodeState::overflowed`].
    Overflowed,
    /// [`crate::semantic::NodeState::stale`].
    Stale,
    /// [`crate::semantic::NodeState::ambient`].
    Ambient,
}

impl StateFlag {
    /// Whether `node` has this flag in force.
    #[must_use]
    pub fn holds(self, node: &SemanticNode) -> bool {
        let state = &node.state;
        match self {
            Self::Focused => state.focused,
            Self::Hovered => state.hovered,
            Self::Active => state.active,
            Self::Captured => state.captured,
            Self::ReadOnly => state.read_only,
            Self::Skeleton => state.skeleton,
            Self::Disabled => state.disabled,
            Self::Selected => state.selected,
            Self::Expanded => state.expanded == Some(true),
            Self::Truncated => state.truncated,
            Self::Overflowed => state.overflowed,
            Self::Stale => state.stale,
            Self::Ambient => state.ambient,
        }
    }
}

/// A conjunction of filters. Every declared clause must hold; a default query
/// matches every node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeQuery {
    /// Exact role wire name, `custom:` forms included.
    pub role: Option<String>,
    /// Substring of the node's label. Case-sensitive: a label is authored
    /// text, and a case-insensitive default would silently match two
    /// different labels that a UI deliberately distinguishes.
    pub label_contains: Option<String>,
    /// A state flag that must be in force.
    pub state: Option<StateFlag>,
}

impl TreeQuery {
    /// A query that matches every node.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Require this exact role wire name.
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Require this substring in the label.
    #[must_use]
    pub fn with_label_contains(mut self, needle: impl Into<String>) -> Self {
        self.label_contains = Some(needle.into());
        self
    }

    /// Require this state flag.
    #[must_use]
    pub fn with_state(mut self, flag: StateFlag) -> Self {
        self.state = Some(flag);
        self
    }

    /// Whether `node` satisfies every declared clause.
    #[must_use]
    pub fn matches(&self, node: &SemanticNode) -> bool {
        if let Some(role) = &self.role
            && node
                .role
                .as_ref()
                .map(crate::tree::Role::as_wire)
                .as_deref()
                != Some(role.as_str())
        {
            return false;
        }
        if let Some(needle) = &self.label_contains
            && !node.label.contains(needle.as_str())
        {
            return false;
        }
        if let Some(flag) = self.state
            && !flag.holds(node)
        {
            return false;
        }
        true
    }
}
