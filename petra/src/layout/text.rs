//! Text measurement, wrapping, and truncation.
//!
//! NOT YET IMPLEMENTED — task T015 of specs/003-petra-layout-engine/tasks.md.
//! The signatures below are the contract the dispatcher in `layout/mod.rs`
//! calls; the bodies are what T015 writes. This file must not reach a commit
//! in this state: the `petra-boundary` gate greps for PETRA_UNIMPLEMENTED.

use crate::frame::placement::PlacementSink;
use crate::geom::Size;
use crate::layout::{LayoutCtx, SizeProposal, Slot};
use crate::tree::{KeyPath, ViewNode};

/// Measure a text run under `proposal`, applying the node's wrap policy.
pub fn measure(node: &ViewNode, ctx: &mut LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    let _ = (node, ctx, proposal);
    unimplemented!("PETRA_UNIMPLEMENTED T015: measure")
}

/// Place a text run, recording truncation in the placement's paint state so
/// the semantic tree's `truncated` flag is real.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let _ = (node, ctx, path, slot, sink);
    unimplemented!("PETRA_UNIMPLEMENTED T015: place")
}
