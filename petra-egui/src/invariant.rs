//! Package-owned invariant companion for `gorgon-petra-egui`.
//!
//! `gorgon-petra-egui` has a real runtime invariant, so this companion asserts
//! it instead of opting out: **the paint accounting must be able to detect a
//! placement that never reached a painter**.
//!
//! The way the D-069 posture fails quietly is a panel that is simply never
//! drawn. The frame digest cannot see it — `contracts/frame-identity.md` hashes
//! placements, not pixels — so every gate stays green while the picture is
//! wrong. [`crate::paint::PaintReport`] is what notices.
//!
//! This companion previously asserted that a `PaintReport` literal with
//! `visited: 2, placements: 3` was incomplete, which proved that two is not
//! three. The shipped counter meanwhile incremented once per loop iteration
//! over a zip of two vectors that are always pushed together, so it could not
//! disagree with `placements` in any reachable pass. The check ran a real
//! paint pass over no frames and the invariant read green over an invariant
//! that did not exist.
//!
//! It now paints a real petrified frame whose only content is an image — a
//! kind this crate has no loader for — and asserts the pass reports it as
//! silent and incomplete, then paints a text frame and asserts that one reads
//! complete. Both directions, against the real painter.

/// Assert the paint accounting can report an incomplete pass.
///
/// # Panics
/// Panics when a real paint pass that emitted nothing for a placement still
/// reports as complete, or when one that painted normally does not — see
/// [`crate::paint::verify_paint_accounting`].
pub fn install() {
    crate::paint::verify_paint_accounting();
}

#[cfg(test)]
mod tests {
    /// The shipping accounting distinguishes a pass that drew from one that
    /// did not. This is the companion the `verify-invariants` gate runs.
    #[test]
    fn install_accepts_the_shipping_accounting() {
        super::install();
    }
}
