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
use crate::layout::{LayoutCtx, Slot, SizeProposal, Proposal};
use crate::tree::{KeyPath, ViewNode};

pub use digest::{FrameDigest, canonical_decimal, hash_text};
pub use placement::{PaintState, Placement, PlacementList, PlacementSemantics, PlacementSink};
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
/// The root is offered the viewport exactly; what it answers is what it takes,
/// and it is placed at the origin. A root that asks for more than the viewport
/// is placed at its own size and clipped — the overflow is visible in the
/// placements rather than silently absorbed.
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
    let taken = crate::layout::measure(tree, ctx, &mut path, offer);
    let viewport_rect = Rect::new(0.0, 0.0, viewport.size.w, viewport.size.h);
    let slot = Slot {
        rect: Rect::new(0.0, 0.0, taken.w, taken.h),
        z: 0,
        clip: viewport_rect,
        opacity: 1.0,
    };
    let mut sink = PlacementList::new();
    crate::layout::place(tree, ctx, &mut path, slot, &mut sink);
    debug_assert!(path.is_empty(), "the walk must leave the path as it found it");
    let placements = sink.into_vec();
    let digest = digest::digest(&viewport, &placements);
    PetrifiedFrame {
        seq,
        digest,
        placements,
        viewport,
        transitions,
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameCounter, TransitionActivity};

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
        assert!(TransitionActivity {
            running: 0,
            ambient: 3
        }
        .is_settled());
        assert!(!TransitionActivity {
            running: 1,
            ambient: 0
        }
        .is_settled());
    }
}
