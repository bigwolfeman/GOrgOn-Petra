//! Placements: exactly one final rect per node per frame.

use std::collections::BTreeMap;

use crate::geom::Rect;
use crate::tree::{Interaction, NodeKind, Role, TextWrap};

/// Paint-relevant state that is not geometry but does change the picture.
///
/// These three fields are digest inputs (`contracts/frame-identity.md` §3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintState {
    /// Hash of the node's rendered text, or zero when it renders none.
    pub content_hash: u64,
    /// Whether content was hidden by a truncation rule this frame.
    pub truncated: bool,
    /// Revision of the theme snapshot this node's tokens resolved against.
    pub token_revision: u64,
}

/// The semantic payload a placement carries into the projection.
///
/// Kept beside the geometry rather than recomputed later: the semantic tree,
/// the AccessKit tree, and the driver's finders must describe the frame that
/// was actually placed, and the cheapest way to guarantee that is to project
/// them from the placements themselves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacementSemantics {
    /// What this node is.
    pub role: Option<Role>,
    /// Human-readable name.
    pub label: Option<String>,
    /// Current value for inputs and status readouts.
    pub value: Option<String>,
    /// Declared disabled state.
    pub disabled: bool,
    /// Declared selected state.
    pub selected: bool,
    /// Declared expanded state.
    pub expanded: Option<bool>,
    /// Declared staleness of the projection behind this node.
    pub stale: bool,
    /// Hosts a deliberately endless animation, so it never blocks settle.
    pub ambient: bool,
    /// Driver action kinds this node accepts.
    pub actions: Vec<Interaction>,
    /// Total rows behind a virtualized collection, materialized or not.
    pub total_count: Option<usize>,
}

/// One node's final geometry and paint state for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// Canonical key-path id.
    pub id: String,
    /// The node's kind.
    pub kind: NodeKind,
    /// Final rect in logical units. Device rounding happens once, at petrify.
    pub rect: Rect,
    /// Paint order within the frame. Ties break on placement order.
    pub z: i32,
    /// The clip in force, in logical units.
    pub clip: Rect,
    /// Cumulative opacity in `[0, 1]`.
    pub opacity: f32,
    /// Paint-relevant state.
    pub paint: PaintState,
    /// Semantic payload.
    pub semantics: PlacementSemantics,
    /// Index of the parent placement, or `None` for the root.
    pub parent: Option<usize>,
}

/// A text run a placement draws, in the form the shaper needs to reproduce
/// exactly the galley the layout was measured against.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPaint {
    /// The content.
    pub text: String,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<String>,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
}

/// What a placement draws, beyond its rect.
///
/// Kept beside the placements rather than inside them. A [`Placement`] is
/// geometry and semantics — exactly what the frame digest hashes
/// (`contracts/frame-identity.md`) — and folding the text of every label into
/// it would put a string in the hash input twice, once as itself and once as
/// its `content_hash`. The renderer needs the string; the digest needs the
/// hash. They are different jobs, so they are different arrays, and
/// `PetrifiedFrame` holds the two at equal length with the same index.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintContent {
    /// The text run to draw, if this node draws one.
    pub text: Option<TextPaint>,
    /// Image source, if this node draws one.
    pub image: Option<String>,
    /// Registered painter name, if this node is a custom kind.
    pub custom: Option<String>,
    /// Token references by role name, resolved against the frame's theme
    /// snapshot at paint time.
    pub tokens: BTreeMap<String, String>,
}

impl PaintContent {
    /// Whether this node draws nothing of its own — a bare container.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.image.is_none()
            && self.custom.is_none()
            && self.tokens.is_empty()
    }
}

/// Where a container sends the placements it produces.
///
/// Placements arrive in tree pre-order; the sink assigns indices and is what
/// makes `parent` correct without every container tracking it.
pub trait PlacementSink {
    /// Record one placement and return its index.
    fn push(&mut self, placement: Placement) -> usize;
    /// Index of the placement a child should name as its parent.
    fn current_parent(&self) -> Option<usize>;
    /// Make `index` the parent for placements pushed until the matching
    /// [`PlacementSink::leave`].
    fn enter(&mut self, index: usize);
    /// Restore the previous parent.
    fn leave(&mut self);
    /// Placements recorded so far, in pre-order.
    fn placed(&self) -> &[Placement];
    /// Attach the paint payload for the placement at `index`.
    ///
    /// Called by the dispatcher, not by containers: every container pushes its
    /// own placement first, so the dispatcher knows the index without the
    /// container having to hand it back.
    fn attach(&mut self, index: usize, content: PaintContent);

    /// How many placements have been recorded.
    fn len(&self) -> usize {
        self.placed().len()
    }

    /// Whether nothing has been placed yet.
    fn is_empty(&self) -> bool {
        self.placed().is_empty()
    }
}

/// A sink that collects placements into a vector in pre-order.
#[derive(Debug, Default)]
pub struct PlacementList {
    placements: Vec<Placement>,
    content: Vec<PaintContent>,
    stack: Vec<usize>,
}

impl PlacementList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The collected placements, in tree pre-order.
    #[must_use]
    pub fn as_slice(&self) -> &[Placement] {
        &self.placements
    }

    /// Take just the collected placements.
    #[must_use]
    pub fn into_vec(self) -> Vec<Placement> {
        self.placements
    }

    /// Take the collected placements and their paint payloads, in the same
    /// order and at equal length.
    #[must_use]
    pub fn into_parts(self) -> (Vec<Placement>, Vec<PaintContent>) {
        debug_assert_eq!(self.placements.len(), self.content.len());
        (self.placements, self.content)
    }

    /// The paint payloads, indexed alongside [`PlacementList::as_slice`].
    #[must_use]
    pub fn content(&self) -> &[PaintContent] {
        &self.content
    }

    /// How many placements were collected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.placements.len()
    }

    /// Whether nothing was placed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }
}

impl PlacementSink for PlacementList {
    fn push(&mut self, mut placement: Placement) -> usize {
        placement.parent = self.stack.last().copied();
        self.placements.push(placement);
        self.content.push(PaintContent::default());
        self.placements.len() - 1
    }

    fn placed(&self) -> &[Placement] {
        &self.placements
    }

    fn attach(&mut self, index: usize, content: PaintContent) {
        if let Some(slot) = self.content.get_mut(index) {
            *slot = content;
        }
    }

    fn current_parent(&self) -> Option<usize> {
        self.stack.last().copied()
    }

    fn enter(&mut self, index: usize) {
        self.stack.push(index);
    }

    fn leave(&mut self) {
        self.stack.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::{PaintState, Placement, PlacementList, PlacementSemantics, PlacementSink};
    use crate::geom::Rect;
    use crate::tree::NodeKind;

    fn placement(id: &str) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect: Rect::ZERO,
            z: 0,
            clip: Rect::ZERO,
            opacity: 1.0,
            paint: PaintState::default(),
            semantics: PlacementSemantics::default(),
            parent: None,
        }
    }

    #[test]
    fn the_sink_wires_parents_from_the_walk() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let a = list.push(placement("/root/a"));
        list.enter(a);
        list.push(placement("/root/a/x"));
        list.leave();
        list.push(placement("/root/b"));
        list.leave();

        let out = list.as_slice();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].parent, None);
        assert_eq!(out[1].parent, Some(0));
        assert_eq!(out[2].parent, Some(1));
        assert_eq!(out[3].parent, Some(0));
        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["/root", "/root/a", "/root/a/x", "/root/b"]);
    }

    #[test]
    fn a_placement_overrides_a_caller_supplied_parent() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let mut lying = placement("/root/a");
        lying.parent = Some(99);
        list.push(lying);
        assert_eq!(list.as_slice()[1].parent, Some(0));
        assert_eq!(list.current_parent(), Some(0));
    }
}
