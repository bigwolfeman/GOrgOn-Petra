//! The wire shape of a semantic node.
//!
//! Field names here are binding: `contracts/semantic-tree.md` fixes them and
//! three consumers read them. Renaming one is a wire break, not a refactor.

use serde::Serialize;

use crate::frame::rounding::DeviceRect;
use crate::tree::{Interaction, Role};

/// A node's rect in **device pixels**, rounded by
/// [`crate::frame::round_rect`] — the one rounding rule the renderer and the
/// frame digest also use, so a bound reported here is the bound painted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
pub struct Bounds {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

impl From<DeviceRect> for Bounds {
    fn from(rect: DeviceRect) -> Self {
        Self {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: rect.h,
        }
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(v: &bool) -> bool {
    !*v
}

/// The `state` block. Absent flag means `false`, per the contract, so only
/// the flags in force reach the wire.
///
/// Every member here is filled from [`crate::frame::PlacementSemantics`] or
/// from the placement's paint state, never re-derived.
/// [`crate::semantic::project`] destructures `PlacementSemantics` with no rest
/// pattern, so a flag added there stops the projection compiling and points at
/// the one line that must fill this struct's new member — which is how
/// `focused` and, after it, the five interaction flags arrived. Inventing a
/// `false` here instead would ship a flag that is a lie on every node the
/// state is actually in force on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct NodeState {
    /// The one node holding keyboard focus this frame.
    ///
    /// Sourced from the placement, not from a side channel: the focus ring is
    /// painted from the same flag, so a tree that disagreed with the picture
    /// would be the exact divergence FR-027 bans.
    #[serde(skip_serializing_if = "is_false")]
    pub focused: bool,
    /// The pointer is inside this node's hit region this frame.
    ///
    /// Engine-derived, from the same hit test the click router uses, so the
    /// tree and the picture cannot disagree about which control is lit.
    #[serde(skip_serializing_if = "is_false")]
    pub hovered: bool,
    /// The node is pressed: it holds pointer capture and the pointer is still
    /// inside its rect.
    #[serde(skip_serializing_if = "is_false")]
    pub active: bool,
    /// The node holds pointer capture this frame.
    ///
    /// Reported separately from `active` because it outlives it: a button
    /// pressed and then dragged off is `captured` without being `active`, and
    /// a driver asserting on a drag needs to see which of the two it has.
    #[serde(skip_serializing_if = "is_false")]
    pub captured: bool,
    /// The node shows a value it will not let this author edit.
    ///
    /// Published as its own flag, never folded into `disabled`: a read-only
    /// node is still focusable and still in tab order, and an assistive
    /// technology told it was disabled would skip a node the user can reach.
    #[serde(skip_serializing_if = "is_false")]
    pub read_only: bool,
    /// The node is a placeholder for content that has not arrived.
    #[serde(skip_serializing_if = "is_false")]
    pub skeleton: bool,
    /// The node refuses interaction this frame.
    #[serde(skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// The node is part of the current selection.
    #[serde(skip_serializing_if = "is_false")]
    pub selected: bool,
    /// Expansion state, for expandables only; `None` on everything else.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// Content was hidden by a truncation rule this frame (FR-020): an
    /// ellipsis policy, a line cap, or a container conceding under FR-005.
    /// A rule that fired is a rule working.
    #[serde(skip_serializing_if = "is_false")]
    pub truncated: bool,
    /// Content is larger than the rect this node was given, so the part that
    /// did not fit is clipped away rather than elided.
    ///
    /// The opposite of `truncated` in what it tells a reader: nothing chose
    /// to hide this, the box was simply too small. Kept apart from
    /// `truncated` since 2026-08-24, when one flag carrying both made
    /// `audit::AuditRule::TruncationIsReal` unable to distinguish a working
    /// ellipsis from a corrupted panel.
    ///
    /// Absent from the wire form when `false`, like every other flag here, so
    /// a consumer written against the older shape reads an unchanged tree
    /// until a node actually overflows.
    #[serde(skip_serializing_if = "is_false")]
    pub overflowed: bool,
    /// The projection behind this node is past its freshness bound.
    #[serde(skip_serializing_if = "is_false")]
    pub stale: bool,
    /// Hosts a declared-endless animation, so it never blocks settle.
    #[serde(skip_serializing_if = "is_false")]
    pub ambient: bool,
}

impl NodeState {
    /// Whether every flag is at its default, so the block would serialize to
    /// `{}`. The block itself is still emitted — a consumer indexing
    /// `state.disabled` must not have to special-case a missing `state`.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// One node of the semantic tree.
///
/// `id`, `label`, `state`, `bounds`, `frame_seq`, `actions` and `children` are
/// always present. `role`, `value` and `total_count` are omitted when the node
/// has none: a `spacer` genuinely has no role, and
/// [`crate::layout::default_role`] refuses to invent one rather than put a
/// node in the accessibility tree that describes nothing. An interactive node
/// missing its role is a defect, and [`crate::semantic::audit`] reports it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SemanticNode {
    /// Stable node id: the canonical key-path (`crate::tree::KeyPath::id`).
    pub id: String,
    /// What this node is. Absent only where the node declares no role and its
    /// kind has no default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// Human-readable name. Empty when undeclared; required non-empty on any
    /// node with actions.
    pub label: String,
    /// Current value for inputs and status readouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// State flags in force this frame.
    pub state: NodeState,
    /// Device-pixel rect from this frame's placement.
    pub bounds: Bounds,
    /// The frame this node projects. Carried per node, not only per response,
    /// so a subtree is as self-describing as a whole tree.
    pub frame_seq: u64,
    /// Driver action kinds this node accepts, sorted and deduplicated.
    ///
    /// Sorted because `actions` is a *set*: two nodes accepting the same kinds
    /// must serialize identically whatever order the author declared them in,
    /// or FR-036's byte-identity would depend on authoring accident.
    pub actions: Vec<Interaction>,
    /// Rows behind a virtualized collection, materialized or not (FR-009).
    /// Present only on nodes that declare one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
    /// Children in focus/reading order, which is placement pre-order.
    pub children: Vec<SemanticNode>,
}

impl SemanticNode {
    /// Pre-order iterator over this node and its descendants.
    #[must_use]
    pub fn iter(&self) -> PreOrder<'_> {
        PreOrder { stack: vec![self] }
    }

    /// The descendant with `id`, or this node when it matches.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Self> {
        self.iter().find(|node| node.id == id)
    }

    /// Whether this node can take keyboard focus this frame: it accepts
    /// [`Interaction::Focus`] and is not disabled. The same rule
    /// [`crate::focus`] applies to placements, restated over the projection so
    /// [`crate::semantic::audit`] can compare the two.
    #[must_use]
    pub fn is_focusable(&self) -> bool {
        self.actions.contains(&Interaction::Focus) && !self.state.disabled
    }
}

impl<'a> IntoIterator for &'a SemanticNode {
    type Item = &'a SemanticNode;
    type IntoIter = PreOrder<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Depth-first pre-order walk of a [`SemanticNode`].
///
/// Explicitly stacked rather than recursive: a virtualized collection can put
/// tens of thousands of siblings under one parent, and a gate must not depend
/// on stack depth to audit a tree.
#[derive(Debug)]
pub struct PreOrder<'a> {
    stack: Vec<&'a SemanticNode>,
}

impl<'a> Iterator for PreOrder<'a> {
    type Item = &'a SemanticNode;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        self.stack.extend(node.children.iter().rev());
        Some(node)
    }
}

/// A whole tree, or a subtree, as served for one frame.
///
/// Serializes *as its root node*: the contract's `tree` verb answers with a
/// node, and a subtree response has the same shape as a full one.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SemanticTree {
    root: SemanticNode,
}

impl SemanticTree {
    /// A tree rooted at `root`.
    #[must_use]
    pub fn new(root: SemanticNode) -> Self {
        Self { root }
    }

    /// The root node.
    #[must_use]
    pub fn root(&self) -> &SemanticNode {
        &self.root
    }

    /// The frame this tree projects.
    #[must_use]
    pub fn frame_seq(&self) -> u64 {
        self.root.frame_seq
    }

    /// Pre-order walk of every node.
    #[must_use]
    pub fn iter(&self) -> PreOrder<'_> {
        self.root.iter()
    }

    /// The node with `id`.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&SemanticNode> {
        self.root.find(id)
    }

    /// The subtree rooted at `id`, as a tree in its own right — the contract's
    /// "or a subtree by `id`".
    #[must_use]
    pub fn subtree(&self, id: &str) -> Option<Self> {
        self.find(id).cloned().map(Self::new)
    }

    /// Every node matching `query`, in pre-order.
    #[must_use]
    pub fn find_all(&self, query: &crate::semantic::TreeQuery) -> Vec<&SemanticNode> {
        self.iter().filter(|node| query.matches(node)).collect()
    }

    /// How many nodes the tree holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Always false: a tree always has a root. Present because clippy asks for
    /// it beside [`SemanticTree::len`], and answering honestly is cheaper than
    /// an allow attribute that would also silence a future real case.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
}

impl<'a> IntoIterator for &'a SemanticTree {
    type Item = &'a SemanticNode;
    type IntoIter = PreOrder<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
