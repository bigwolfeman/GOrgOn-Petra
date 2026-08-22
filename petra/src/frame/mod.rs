//! Petrify: turning a view tree into a frame with an identity.
//!
//! One pass produces every placement, one rounding rule turns logical units
//! into device pixels, and one digest fingerprints the result. A frame that
//! completed petrify has an identity; a query mid-negotiation answers the last
//! completed frame (`contracts/frame-identity.md`).

pub mod digest;
pub mod placement;
pub mod rounding;
pub mod viewport;

use crate::geom::Rect;
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot};
use crate::tree::{KeyPath, ViewNode};

pub use digest::{FrameDigest, canonical_decimal, hash_text};
pub use placement::{
    PaintContent, PaintState, Placement, PlacementList, PlacementSemantics, PlacementSink,
    TextPaint,
};
pub use rounding::{DeviceRect, round_coord, round_rect};
pub use viewport::Viewport;

/// How much motion the frame carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransitionActivity {
    /// Transitions still moving toward a target.
    pub running: usize,
    /// Declared-endless animations. These never settle and never block a
    /// driver's settle wait (`contracts/animation.md`).
    pub ambient: usize,
}

impl TransitionActivity {
    /// Whether the frame is settled: nothing is running that is expected to
    /// stop.
    #[must_use]
    pub fn is_settled(self) -> bool {
        self.running == 0
    }
}

/// The finished output of one layout pass.
#[derive(Clone, Debug, PartialEq)]
pub struct PetrifiedFrame {
    /// Monotone within one application run, starting at 1, never reused.
    pub seq: u64,
    /// Content fingerprint over placements and paint state.
    pub digest: FrameDigest,
    /// Every node's final geometry, in tree pre-order.
    pub placements: Vec<Placement>,
    /// What each placement draws, at the same index and the same length.
    ///
    /// Not hashed *directly* — the digest must be recomputable from
    /// `(viewport, placements)` alone, because that is all a driver `frame`
    /// response carries (`contracts/driver-protocol.md`). It reaches the
    /// digest as `placements[i].paint.paint_hash`, written when the payload
    /// was attached. [`PetrifiedFrame::paint_hashes_agree`] is what checks the
    /// two have not drifted.
    pub content: Vec<PaintContent>,
    /// Every placement's Merkle subtree hash, at the same index and the same
    /// length as [`PetrifiedFrame::placements`].
    ///
    /// `subtree_hashes[0]` — the root's — is the value
    /// [`PetrifiedFrame::digest`] was built from
    /// (`digest::root_hash_from`). This is what a reused subtree hands back
    /// to its parent: the incremental placement path this crate does not yet
    /// have can copy a subtree from the previous frame and reuse the hash
    /// recorded here instead of re-walking it
    /// (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`).
    pub subtree_hashes: Vec<[u8; 32]>,
    /// How many placements each index's subtree occupies, itself included —
    /// see [`crate::frame::placement::PlacementList`] — at the same index
    /// and the same length as [`PetrifiedFrame::placements`].
    ///
    /// This is the range a reuse pass copies: placement `i`'s subtree is
    /// `placements[i..i + subtree_len[i]]`.
    pub subtree_len: Vec<usize>,
    /// The [`Slot`] each placement was *offered*, at the same index and the
    /// same length.
    ///
    /// Not part of the frame's identity and not on the wire: a driver
    /// `frame` response carries placements, and the digest is recomputable
    /// from `(viewport, placements)` alone. This array exists for the
    /// incremental path, which cannot decide whether re-placing a subtree
    /// would change anything by looking at the rect the node *took* — a
    /// container may hand a child less than it asked for, and two different
    /// offers can produce the same taken rect.
    pub slots: Vec<Slot>,
    /// What the frame was negotiated against.
    pub viewport: Viewport,
    /// Motion in force at petrify time.
    pub transitions: TransitionActivity,
}

impl PetrifiedFrame {
    /// The placement with `id`, if the frame has one.
    #[must_use]
    pub fn placement(&self, id: &str) -> Option<&Placement> {
        self.placements.iter().find(|p| p.id == id)
    }

    /// Every placement paired with what it draws, in tree pre-order.
    pub fn drawn(&self) -> impl Iterator<Item = (&Placement, &PaintContent)> {
        self.placements.iter().zip(self.content.iter())
    }

    /// Every placement paired with what it draws, in paint order: ascending
    /// `z`, then tree order. The sort is stable, so two frames with identical
    /// input paint in identical order.
    #[must_use]
    pub fn paint_pairs(&self) -> Vec<(&Placement, &PaintContent)> {
        let mut out: Vec<(&Placement, &PaintContent)> = self.drawn().collect();
        out.sort_by_key(|(p, _)| p.z);
        out
    }

    /// Whether every placement's paint hash still describes the payload at the
    /// same index.
    ///
    /// The digest hashes `paint.paint_hash`; the renderer draws `content`.
    /// Both are public fields, so the two can be made to disagree — and a
    /// frame whose payload was edited after petrify would keep the digest of
    /// the picture it no longer draws. This is the check that says so; a
    /// length mismatch is a disagreement too, since the tail has no hash to
    /// compare against.
    #[must_use]
    pub fn paint_hashes_agree(&self) -> bool {
        self.placements.len() == self.content.len()
            && self
                .drawn()
                .all(|(p, c)| p.paint.paint_hash == digest::hash_paint_content(c))
    }

    /// Placements in paint order: ascending `z`, then placement order.
    ///
    /// A stable sort is required, not merely convenient: ties inside one layer
    /// must resolve to tree order, or two frames with identical input could
    /// paint in different orders.
    #[must_use]
    pub fn paint_order(&self) -> Vec<&Placement> {
        let mut out: Vec<&Placement> = self.placements.iter().collect();
        out.sort_by_key(|p| p.z);
        out
    }
}

/// Allocates frame sequence numbers for one application run.
#[derive(Debug)]
pub struct FrameCounter {
    next: u64,
}

impl Default for FrameCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCounter {
    /// A counter whose first frame is 1.
    #[must_use]
    pub fn new() -> Self {
        Self { next: 1 }
    }

    /// The sequence number the next petrify will take.
    #[must_use]
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Take the next sequence number.
    pub fn take(&mut self) -> u64 {
        let seq = self.next;
        self.next += 1;
        seq
    }
}

/// Negotiate `tree` against `viewport` and produce a frame with an identity.
///
/// The root is offered the viewport exactly and **placed into the viewport**,
/// whatever it answers. `measure` stays honest — a root whose minimums do not
/// fit answers with what it actually needs, and `crate::layout::stack` has a
/// test pinning that it does not clamp — but placement is where the budget is
/// enforced, so the root is handed the real window and concedes into it.
///
/// This was the other way round at first: the slot was built from the root's
/// own response. Two things followed, and both were wrong. A root asking for
/// more than the window was placed oversized and clipped, so
/// `place` saw `wanted == available`, never called `concede`, and the whole
/// FR-005 concession order was unreachable at the top level — the one place a
/// window is guaranteed to be a hard budget. And a root answering *smaller*
/// than the window did not fill it: an app whose root is a stack around one
/// short label rendered as a narrow column against the left edge.
pub fn petrify(
    seq: u64,
    tree: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    viewport: Viewport,
    transitions: TransitionActivity,
) -> PetrifiedFrame {
    let mut path = KeyPath::root();
    let offer = SizeProposal {
        horizontal: Proposal::Exact(viewport.size.w),
        vertical: Proposal::Exact(viewport.size.h),
    };
    // The measure pass still runs: it populates the measurement cache that
    // `place` reads back, and skipping it would make every container
    // re-negotiate from scratch during placement.
    let _ = crate::layout::measure(tree, ctx, &mut path, offer);
    let viewport_rect = Rect::new(0.0, 0.0, viewport.size.w, viewport.size.h);
    let slot = Slot {
        rect: viewport_rect,
        z: 0,
        clip: viewport_rect,
        opacity: 1.0,
    };
    let mut sink = PlacementList::new();
    crate::layout::place(tree, ctx, &mut path, slot, &mut sink);
    debug_assert!(
        path.is_empty(),
        "the walk must leave the path as it found it"
    );
    let placed = sink.into_parts();
    let (placements, content, subtree_len, slots) = (
        placed.placements,
        placed.content,
        placed.subtree_len,
        placed.slots,
    );
    // Computed once: the array is what a reuse pass needs, and the root
    // entry is what the frame digest is built from — no reason to walk the
    // Merkle tree a second time to get the same root hash.
    let subtree_hashes = digest::subtree_hashes(viewport.scale, &placements);
    let digest = digest::digest_from_root(&viewport, digest::root_hash_from(&subtree_hashes));
    let frame = PetrifiedFrame {
        seq,
        digest,
        placements,
        content,
        subtree_hashes,
        subtree_len,
        slots,
        viewport,
        transitions,
    };
    debug_assert!(
        frame.paint_hashes_agree(),
        "a sink attached a paint payload without hashing it into the \
         placement: the digest of this frame is blind to what it draws"
    );
    frame
}

#[cfg(test)]
mod tests {
    use super::{FrameCounter, TransitionActivity, Viewport, petrify};
    use crate::geom::Size;
    use crate::testing::{Harness, MonoContent, NoRows};
    use crate::token::ThemeMode;
    use crate::tree::{AxisConstraint, Constraints, NodeKind, Props, ViewNode};

    fn frame_of(tree: &ViewNode, w: f32, h: f32) -> super::PetrifiedFrame {
        let mut harness = Harness::with(MonoContent::default(), NoRows);
        petrify(
            1,
            tree,
            &mut harness.ctx(),
            Viewport::new(Size::new(w, h), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    fn text(key: &str, body: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key).with_props(Props {
            text: Some(body.to_string()),
            ..Props::default()
        })
    }

    /// A root that wants less than the window still gets the window.
    ///
    /// The slot used to be built from the root's own response, so an app whose
    /// root was a stack around one short label rendered in a column as wide as
    /// that label, hard against the left edge, with the rest of the window
    /// blank. Nothing asserted the root rect, so nothing noticed.
    #[test]
    fn a_root_smaller_than_the_window_still_fills_it() {
        let tree = ViewNode::new(NodeKind::Stack, "root").child(text("t", "hi"));
        let frame = frame_of(&tree, 800.0, 600.0);
        let root = &frame.placements[0];
        assert_eq!(root.id, "/root");
        assert_eq!(
            (root.rect.x, root.rect.y, root.rect.w, root.rect.h),
            (0.0, 0.0, 800.0, 600.0),
            "the root is the window, not its own preferred size"
        );
    }

    /// A root that wants more than the window concedes into it.
    ///
    /// With the slot taken from the root's response, `place` saw
    /// `wanted == available`, so `concede` never ran and the overflow was
    /// clipped with no truncation flag anywhere — FR-005's concession order was
    /// unreachable at the top level, which is the one place the budget is
    /// genuinely hard.
    #[test]
    fn a_root_larger_than_the_window_concedes_into_it_and_says_so() {
        let tall = Constraints {
            vertical: AxisConstraint {
                min: Some(400.0),
                ..AxisConstraint::default()
            },
            ..Constraints::default()
        };
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(crate::geom::Axis::Vertical),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "a").with_constraints(tall))
            .child(ViewNode::new(NodeKind::Spacer, "b").with_constraints(tall));

        let frame = frame_of(&tree, 300.0, 200.0);
        let root = &frame.placements[0];
        assert_eq!(
            (root.rect.w, root.rect.h),
            (300.0, 200.0),
            "the root is clamped to the window it was given"
        );
        assert!(
            root.paint.truncated,
            "800 units of declared minimum in a 200-unit window must be \
             reported as truncated, not clipped in silence: {:?}",
            root.paint
        );
        for p in &frame.placements {
            assert!(
                p.rect.bottom() <= 200.0 + 1e-3,
                "{} runs to {} in a 200-unit window",
                p.id,
                p.rect.bottom()
            );
        }
    }

    #[test]
    fn sequence_numbers_start_at_one_and_never_repeat() {
        let mut counter = FrameCounter::new();
        assert_eq!(counter.peek(), 1);
        let seen: Vec<u64> = (0..5).map(|_| counter.take()).collect();
        assert_eq!(seen, [1, 2, 3, 4, 5]);
        assert_eq!(counter.peek(), 6);
    }

    /// Ambient animations run forever by declaration, so a frame with only
    /// ambient motion is settled. The driver's settle wait depends on this.
    #[test]
    fn ambient_motion_does_not_block_settle() {
        assert!(
            TransitionActivity {
                running: 0,
                ambient: 3
            }
            .is_settled()
        );
        assert!(
            !TransitionActivity {
                running: 1,
                ambient: 0
            }
            .is_settled()
        );
    }
}
