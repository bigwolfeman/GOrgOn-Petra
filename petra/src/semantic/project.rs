//! Frame in, tree out.

use crate::frame::rounding::round_rect;
use crate::frame::{PetrifiedFrame, PlacementSemantics};
use crate::semantic::node::{NodeState, SemanticNode, SemanticTree};
use crate::tree::Interaction;

/// Project a petrified frame into its semantic tree.
///
/// The walk is a single reverse pass over `frame.placements`. Placements
/// arrive in pre-order, so every child has a higher index than its parent;
/// building from the last index backwards means a node's children are already
/// finished when the node itself is built, and no recursion is needed for a
/// collection with tens of thousands of siblings.
///
/// Nothing is sorted, so child order is placement order, which is declaration
/// order, which is reading order (SC-010).
///
/// Returns `None` for a frame with no placements. `petrify` never produces
/// one — the root is always placed — so this is the hand-built-frame case.
///
/// A placement whose `parent` does not point at a lower index cannot be hung
/// anywhere and is left out of the tree rather than guessed at.
/// [`crate::semantic::AuditRule::PlacementIsNotANode`] reports exactly that,
/// so a malformed frame fails an audit instead of projecting a quiet lie.
#[must_use]
pub fn project(frame: &PetrifiedFrame) -> Option<SemanticTree> {
    let count = frame.placements.len();
    if count == 0 {
        return None;
    }
    let mut pending: Vec<Vec<SemanticNode>> = vec![Vec::new(); count];
    let mut root = None;
    for index in (0..count).rev() {
        // Children were pushed as the pass walked down from the last index, so
        // they accumulated newest-first; reversing restores placement order.
        let mut children = std::mem::take(&mut pending[index]);
        children.reverse();
        let node = node_at(frame, index, children);
        match frame.placements[index].parent {
            Some(parent) if parent < index => pending[parent].push(node),
            _ if index == 0 => root = Some(node),
            // Unreachable from `petrify`; dropped rather than guessed at.
            _ => {}
        }
    }
    root.map(SemanticTree::new)
}

/// Build one node from placement `index` and its finished children.
fn node_at(frame: &PetrifiedFrame, index: usize, children: Vec<SemanticNode>) -> SemanticNode {
    let placement = &frame.placements[index];
    // Destructured with no rest pattern on purpose. When `PlacementSemantics`
    // grows a member, this pattern stops compiling and the compiler points
    // here, at the one place the new flag has to be projected from. A `..`
    // would instead have produced a tree silently missing it — which is how
    // `focused` arrived: it landed in a sibling branch, this line failed to
    // compile, and the flag was wired in rather than defaulted to false.
    let PlacementSemantics {
        role,
        focused,
        hovered,
        active,
        captured,
        read_only,
        skeleton,
        label,
        value,
        disabled,
        selected,
        expanded,
        stale,
        ambient,
        actions,
        total_count,
        // Held out on purpose: what the focus indicator looks like and
        // which rect it is drawn on are paint facts, not facts about what
        // the node is. The tree already says which node is `focused`.
        focus_figure: _,
        focus_shown_on: _,
        focus_run: _,
        owns_its_text: _,
        // Held out for a different reason than the two above: `behaviour`
        // and `raw_claim` are routing facts, not accessibility payload —
        // they say how an *event* reaching this node is interpreted, not
        // what the node *is*. `SemanticNode` describes the tree to a screen
        // reader and to `TreeQuery`; the router reads these two straight off
        // `PlacementSemantics` on the placement itself
        // (`contracts/frame-identity.md`), never through this projection.
        behaviour: _,
        raw_claim: _,
    } = &placement.semantics;

    let mut actions: Vec<Interaction> = actions.clone();
    actions.sort_unstable();
    actions.dedup();

    SemanticNode {
        id: placement.id.clone(),
        role: role.clone(),
        label: label.clone().unwrap_or_default(),
        value: value.clone(),
        state: NodeState {
            focused: *focused,
            hovered: *hovered,
            active: *active,
            captured: *captured,
            read_only: *read_only,
            skeleton: *skeleton,
            disabled: *disabled,
            selected: *selected,
            expanded: *expanded,
            // Truncation is decided by the container that could not fit its
            // content, and it records the fact in paint state. The projection
            // reports it; it never re-derives it.
            truncated: placement.paint.truncated,
            overflowed: placement.paint.overflowed,
            stale: *stale,
            ambient: *ambient,
        },
        bounds: round_rect(placement.rect, frame.viewport.scale).into(),
        frame_seq: frame.seq,
        actions,
        total_count: *total_count,
        children,
    }
}
