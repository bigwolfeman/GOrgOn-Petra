//! Constraint clamping and the deterministic concession order.
//!
//! NOT YET IMPLEMENTED — task T020 of specs/003-petra-layout-engine/tasks.md.
//! The signatures below are the contract the dispatcher in `layout/mod.rs`
//! calls; the bodies are what T020 writes. This file must not reach a commit
//! in this state: the `petra-boundary` gate greps for PETRA_UNIMPLEMENTED.

use crate::geom::Size;
use crate::tree::Constraints;

/// The order in which a container gives ground when the fit is impossible
/// (FR-005): flexible slack first, then declared scroll regions, then
/// truncation. Deterministic, and the same order every time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Concession {
    /// Take from the most flexible children first.
    FlexibleSlack,
    /// Let a declared scroll region absorb the overflow.
    ScrollRegion,
    /// Truncate content, recording it for the semantic tree.
    Truncate,
}

/// Clamp `size` into `constraints`, returning what was conceded.
pub fn concede(size: Size, constraints: Constraints) -> (Size, Option<Concession>) {
    let _ = (size, constraints);
    unimplemented!("PETRA_UNIMPLEMENTED T020: concede")
}
