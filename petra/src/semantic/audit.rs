//! The audit obligations, as a function a gate can call.
//!
//! `contracts/semantic-tree.md` ends with four obligations and calls them
//! "machine-checkable". [`audit`] is that machine. It lives in the library,
//! not in a test file, because SC-010 and spec 004's gates have to call it —
//! a checker only a `#[test]` can reach is prose with extra steps.
//!
//! Three more rules sit beside the four. They check that the tree describes
//! *this* frame rather than some frame: no node without a placement (the
//! FR-009 anti-fabrication rule), no placement without a node, no two nodes
//! sharing an id (FR-039's `stale-node` cannot name a node that appears
//! twice), and no node claiming another frame's sequence number.

use std::collections::BTreeMap;
use std::fmt;

use crate::focus::FocusTree;
use crate::frame::PetrifiedFrame;
use crate::semantic::node::{SemanticNode, SemanticTree};
use crate::tree::{Interaction, Role};

/// One thing an audited tree may get wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AuditRule {
    /// Contract obligation 1: a node with actions carries a role and a label.
    ActionableNeedsRoleAndLabel,
    /// Contract obligation 2: a `status` role carries a label — the
    /// shape-plus-text rule, so state is never colour alone.
    StatusNeedsLabel,
    /// Contract obligation 3: focus order equals child order (SC-010).
    FocusOrderIsChildOrder,
    /// Contract obligation 4: a `truncated` flag corresponds to a real
    /// truncation, and an `overflowed` flag to a real overflow.
    ///
    /// The two are separate facts about a node and this rule checks both:
    /// `truncated` says a policy hid something on purpose, `overflowed` says
    /// the box was too small and the remainder is clipped away. They were one
    /// flag until 2026-08-24, which meant a working ellipsis and a corrupted
    /// panel reached this rule as the same report.
    TruncationIsReal,
    /// Projection integrity: a node whose id names no placement in the frame.
    /// The FR-009 honesty rule — an unmaterialized row is a `total_count`,
    /// never a node.
    NodeIsNotAPlacement,
    /// Projection integrity: a placement the tree left out.
    PlacementIsNotANode,
    /// Projection integrity: two nodes in one tree sharing an id.
    DuplicateNodeId,
    /// Projection integrity: a node stamped with another frame's sequence
    /// number.
    FrameSeqMismatch,
    /// `contracts/interaction-state.md` §5: read-only is not a weaker
    /// disabled.
    ///
    /// Two things go wrong under one name because they are one mistake made
    /// at two depths. Declaring both flags on one node is the author saying
    /// two incompatible things — a node the user can reach and read, and a
    /// node the user cannot reach at all — and an assistive technology has to
    /// pick one. A read-only node that declares `Focus` and is *missing from
    /// focus order* is the same mistake made in the engine: it means
    /// something added a `read_only` branch to the focus filter or to
    /// `hit_test`, which §5 forbids by name. Read-only is enforced
    /// declaratively — a read-only component declares fewer interactions —
    /// never by a second refusal path.
    ReadOnlyIsNotDisabled,
}

/// Every rule [`audit`] can report, in declaration order. A gate that prints a
/// per-rule tally reads this so a new rule cannot be silently uncounted.
pub const AUDIT_RULES: &[AuditRule] = &[
    AuditRule::ActionableNeedsRoleAndLabel,
    AuditRule::StatusNeedsLabel,
    AuditRule::FocusOrderIsChildOrder,
    AuditRule::TruncationIsReal,
    AuditRule::NodeIsNotAPlacement,
    AuditRule::PlacementIsNotANode,
    AuditRule::DuplicateNodeId,
    AuditRule::FrameSeqMismatch,
    AuditRule::ReadOnlyIsNotDisabled,
];

impl AuditRule {
    /// The rule's stable name, for gate output and for a driver error body.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActionableNeedsRoleAndLabel => "actionable-needs-role-and-label",
            Self::StatusNeedsLabel => "status-needs-label",
            Self::FocusOrderIsChildOrder => "focus-order-is-child-order",
            Self::TruncationIsReal => "truncation-is-real",
            Self::NodeIsNotAPlacement => "node-is-not-a-placement",
            Self::PlacementIsNotANode => "placement-is-not-a-node",
            Self::DuplicateNodeId => "duplicate-node-id",
            Self::FrameSeqMismatch => "frame-seq-mismatch",
            Self::ReadOnlyIsNotDisabled => "read-only-is-not-disabled",
        }
    }
}

impl fmt::Display for AuditRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One violation, naming the rule, the node, and what was wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditViolation {
    /// Which obligation was broken.
    pub rule: AuditRule,
    /// The node id the violation is about.
    pub node_id: String,
    /// What was actually wrong, in words a maintainer can act on.
    pub detail: String,
}

impl fmt::Display for AuditViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.rule, self.node_id, self.detail)
    }
}

fn violation(rule: AuditRule, node_id: &str, detail: impl Into<String>) -> AuditViolation {
    AuditViolation {
        rule,
        node_id: node_id.to_owned(),
        detail: detail.into(),
    }
}

/// Check `tree` against the contract's audit obligations for the frame it
/// claims to project.
///
/// The frame is a parameter, not an optional convenience: obligation 4 asks
/// whether a `truncated` flag is *real*, and only the frame knows whether the
/// node it sits on had content to hide. Passing the two separately also lets
/// the audit catch a tree paired with the wrong frame, which is the failure a
/// screenshot-plus-tree consumer would otherwise see as a mystery.
///
/// An empty result is a pass. Violations come back in a stable order —
/// per-node in tree pre-order, then the frame-wide rules — so a gate can diff
/// two runs.
#[must_use]
pub fn audit(tree: &SemanticTree, frame: &PetrifiedFrame) -> Vec<AuditViolation> {
    let mut out = Vec::new();
    let mut by_id: BTreeMap<&str, &SemanticNode> = BTreeMap::new();

    for node in tree.iter() {
        if by_id.insert(node.id.as_str(), node).is_some() {
            out.push(violation(
                AuditRule::DuplicateNodeId,
                &node.id,
                "two nodes in one tree carry this id, so no consumer can resolve it to one node",
            ));
        }
        if !node.actions.is_empty() {
            if node.role.is_none() {
                out.push(violation(
                    AuditRule::ActionableNeedsRoleAndLabel,
                    &node.id,
                    format!("accepts {} but declares no role", action_list(node)),
                ));
            }
            if node.label.trim().is_empty() {
                out.push(violation(
                    AuditRule::ActionableNeedsRoleAndLabel,
                    &node.id,
                    format!("accepts {} but carries no label", action_list(node)),
                ));
            }
        }
        if node.state.read_only && node.state.disabled {
            out.push(violation(
                AuditRule::ReadOnlyIsNotDisabled,
                &node.id,
                "declares both read-only and disabled; read-only stays reachable and \
                 disabled does not, so a consumer told both has to guess which one the \
                 author meant",
            ));
        }
        if node.role.as_ref() == Some(&Role::Status) && node.label.trim().is_empty() {
            out.push(violation(
                AuditRule::StatusNeedsLabel,
                &node.id,
                "a status readout with no label leaves colour as its only channel",
            ));
        }
        if node.frame_seq != frame.seq {
            out.push(violation(
                AuditRule::FrameSeqMismatch,
                &node.id,
                format!(
                    "node claims frame {} but the frame is {}",
                    node.frame_seq, frame.seq
                ),
            ));
        }
    }

    let placed: BTreeMap<&str, usize> = frame
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| (placement.id.as_str(), index))
        .collect();

    for node in tree.iter() {
        if !placed.contains_key(node.id.as_str()) {
            out.push(violation(
                AuditRule::NodeIsNotAPlacement,
                &node.id,
                "no placement in this frame has this id, so the node describes nothing that was laid out",
            ));
        }
    }

    for placement in &frame.placements {
        if !by_id.contains_key(placement.id.as_str()) {
            out.push(violation(
                AuditRule::PlacementIsNotANode,
                &placement.id,
                "the frame placed this node and the tree does not mention it",
            ));
        }
    }

    for (index, placement) in frame.placements.iter().enumerate() {
        let Some(node) = by_id.get(placement.id.as_str()) else {
            continue;
        };
        if node.state.truncated != placement.paint.truncated {
            out.push(violation(
                AuditRule::TruncationIsReal,
                &node.id,
                format!(
                    "tree says truncated={}, the placement says {}",
                    node.state.truncated, placement.paint.truncated
                ),
            ));
        }
        if node.state.overflowed != placement.paint.overflowed {
            out.push(violation(
                AuditRule::TruncationIsReal,
                &node.id,
                format!(
                    "tree says overflowed={}, the placement says {}",
                    node.state.overflowed, placement.paint.overflowed
                ),
            ));
        }
        if node.state.overflowed {
            // An overflow is a fact about content against a rect, so a node
            // that draws nothing cannot have one. Same shape as the
            // `truncated` check below, and it is the check that stops
            // `overflowed` from becoming a bit a container sets out of habit.
            let draws_text = frame
                .content
                .get(index)
                .is_some_and(|content| content.text.is_some());
            if !draws_text {
                out.push(violation(
                    AuditRule::TruncationIsReal,
                    &node.id,
                    "flagged overflowed while drawing no text, so it had no content to outgrow                      its box",
                ));
            }
        }
        if node.state.truncated {
            let draws_text = frame
                .content
                .get(index)
                .is_some_and(|content| content.text.is_some());
            if !draws_text && node.children.is_empty() {
                out.push(violation(
                    AuditRule::TruncationIsReal,
                    &node.id,
                    "flagged truncated while drawing no text and holding no children, so it had nothing to hide",
                ));
            }
        }
    }

    out.extend(focus_violations(tree, frame));
    out
}

/// Obligation 3 and the engine half of [`AuditRule::ReadOnlyIsNotDisabled`],
/// checked against the module that owns focus order rather than against a
/// second copy of its rule.
///
/// The two rules share this function because they share the one expensive
/// thing in it: the [`FocusTree`] built from the frame's own placements.
/// Building it twice to keep the rules in separate functions would double the
/// cost of the audit to save a paragraph.
///
/// [`FocusTree::from_placements`] is handed an empty scope map on purpose:
/// overlay scopes decide where traversal may *go*, never what order the
/// focusables are in (`crate::focus`'s `index` builds `order` by filtering the
/// placement slice and consults the scope map only for `scope_chain`). The
/// order this compares against is therefore the real one.
fn focus_violations(tree: &SemanticTree, frame: &PetrifiedFrame) -> Vec<AuditViolation> {
    let sane_parents = frame
        .placements
        .iter()
        .enumerate()
        .all(|(index, placement)| placement.parent.is_none_or(|parent| parent < index));
    if !sane_parents {
        // A frame whose parent links do not run backwards is already reported
        // by `PlacementIsNotANode`; walking it here would only add noise, and
        // `FocusTree` would index a parent that is not there.
        return Vec::new();
    }

    let focus = FocusTree::from_placements(&frame.placements, &BTreeMap::new());
    let expected: Vec<&str> = focus.order().iter().map(String::as_str).collect();

    // §5's engine half: read-only must not have been folded into the focus
    // filter. A read-only node that declares `Focus` and is not disabled is
    // Tab-reachable, full stop — so if it is missing from the order the
    // filter has grown a `read_only` branch, which is the drift this rule
    // exists to catch. A node that declares both flags is already reported
    // above and is skipped here, so one mistake produces one violation.
    let mut out: Vec<AuditViolation> = tree
        .iter()
        .filter(|node| {
            node.state.read_only
                && !node.state.disabled
                && node.actions.contains(&Interaction::Focus)
                && !expected.contains(&node.id.as_str())
        })
        .map(|node| {
            violation(
                AuditRule::ReadOnlyIsNotDisabled,
                &node.id,
                "read-only, focusable and not disabled, yet absent from focus order; \
                 read-only must never be a second refusal path (§5)",
            )
        })
        .collect();

    let projected: Vec<&str> = tree
        .iter()
        .filter(|node| node.is_focusable())
        .map(|node| node.id.as_str())
        .collect();
    if projected == expected {
        return out;
    }

    let at = projected
        .iter()
        .zip(expected.iter())
        .position(|(got, want)| got != want)
        .unwrap_or(projected.len().min(expected.len()));
    let blamed = projected
        .get(at)
        .or_else(|| expected.get(at))
        .copied()
        .unwrap_or("/");
    out.push(violation(
        AuditRule::FocusOrderIsChildOrder,
        blamed,
        format!(
            "reading order of focusables diverges from focus order at position {at}: \
             tree has {:?}, focus order has {:?}",
            projected.get(at),
            expected.get(at)
        ),
    ));
    out
}

/// The node's action kinds as a readable list, for a violation message.
fn action_list(node: &SemanticNode) -> String {
    node.actions
        .iter()
        .map(|action| format!("{action:?}").to_lowercase())
        .collect::<Vec<_>>()
        .join(", ")
}
