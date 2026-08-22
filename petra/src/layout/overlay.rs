//! The overlay container: every child gets the container's proposal; z-order is child order unless declared.
//!
//! NOT YET IMPLEMENTED — task T018 of specs/003-petra-layout-engine/tasks.md.
//! The signatures below are the contract the dispatcher in `layout/mod.rs`
//! calls; the bodies are what T018 writes. This file must not reach a commit
//! in this state: the `petra-boundary` gate greps for PETRA_UNIMPLEMENTED.

use crate::frame::placement::PlacementSink;
use crate::geom::Size;
use crate::layout::{LayoutCtx, SizeProposal, Slot};
use crate::tree::{KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let _ = (node, ctx, path, proposal);
    unimplemented!("PETRA_UNIMPLEMENTED T018: measure")
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let _ = (node, ctx, path, slot, sink);
    unimplemented!("PETRA_UNIMPLEMENTED T018: place")
}
