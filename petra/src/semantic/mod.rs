//! The semantic projection: one tree, three consumers.
//!
//! A [`crate::frame::PetrifiedFrame`] is geometry plus a semantic payload per
//! placement. [`project`] turns that into the tree defined by
//! `specs/003-petra-layout-engine/contracts/semantic-tree.md`: the SAME tree
//! that feeds the AccessKit node tree, the agent's UI-state query, and the
//! driver's finders. No consumer gets a private variant — a divergence
//! between what a screen reader hears and what a test asserts is a bug by
//! definition (FR-027).
//!
//! # What makes an id stable
//!
//! Nothing here allocates one. A node's `id` is [`crate::frame::Placement::id`],
//! which is the canonical form of the node's key-path
//! ([`crate::tree::KeyPath::id`]). So an id is stable exactly while the
//! key-path is — across frames, scrolls, re-parents that preserve the path,
//! and theme changes — and is never recycled within a run, because a retired
//! id can only come back by its key-path coming back, and a key-path *is* the
//! node's identity. FR-039's `stale-node` error rests on that: an id that no
//! longer resolves names a node that is genuinely gone, never a different node
//! that inherited the slot. `an_absent_key_path_never_hands_its_id_to_a_new_node`
//! is the test.
//!
//! # What makes a repeat query byte-identical
//!
//! Every collection on the wire is ordered by construction:
//! [`SemanticNode::children`] is placement pre-order (see below),
//! [`SemanticNode::actions`] is sorted and deduplicated, and no field is
//! backed by a hash container. Two projections of one frame are therefore
//! equal as values and identical as bytes (FR-036).
//!
//! # What makes `children` reading order
//!
//! Placements arrive in tree pre-order — [`crate::frame::PlacementSink`]'s
//! contract — and every container places its children in declaration order.
//! [`project`] rebuilds the tree from [`crate::frame::Placement::parent`]
//! without sorting anything, so child order is declaration order, which is
//! visual order. That is the same argument [`crate::focus`] makes for
//! traversal order, and [`audit`] checks the two agree (SC-010).
//!
//! # Honesty about what is not there
//!
//! A virtualized collection projects the rows the frame materialized and
//! reports [`SemanticNode::total_count`]. It never invents a node for a row
//! that was not placed (FR-009): every node in the tree comes from a
//! placement, and [`AuditRule::NodeIsNotAPlacement`] is the machine-checkable
//! form of that promise.
//!
//! # Who authored a node, and how far behind it is
//!
//! Two more facts about the same tree, both for surfaces a plugin contributed
//! rather than the host. [`owner_of`] answers *whose* a node is, from the
//! `ui:7` segment its id already carries. [`ContributionLedger`] answers how
//! old the snapshot on screen is, and whether the fiber owes the shell a
//! newer one.
//! Neither adds a field to [`SemanticNode`] and neither calls anything: the
//! projection stays a pure function of a frame.

mod attribution;
mod audit;
mod node;
mod project;
mod query;
mod staleness;

pub use attribution::{
    CONTRIBUTION_KEY_PREFIX, ContributionId, contribution_key, owner_chain, owner_of,
};
pub use audit::{AUDIT_RULES, AuditRule, AuditViolation, audit};
pub use node::{Bounds, NodeState, PreOrder, SemanticNode, SemanticTree};
pub use project::project;
pub use query::{StateFlag, TreeQuery};
pub use staleness::{ContributionLedger, ContributionStatus, PushOutcome};

#[cfg(test)]
mod tests;
