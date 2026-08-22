//! Package-owned invariant companion for `gorgon-petra-egui`.
//!
//! `gorgon-petra-egui` has a real runtime invariant, so this companion asserts
//! it instead of opting out: **the paint accounting must be able to detect a
//! placement that never reached a painter**.
//!
//! The way the D-069 posture fails quietly is a panel that is simply never
//! drawn. The frame digest cannot see it — `contracts/frame-identity.md` hashes
//! placements, not pixels — so every gate stays green while the picture is
//! wrong. [`crate::paint::PaintReport`] is what notices, and an accounting that
//! cannot report an incomplete pass would be worse than none: it would look
//! like a check.

/// Assert the paint accounting can report an incomplete pass.
///
/// Panics when a report that visited fewer placements than the frame holds
/// still reads as complete — see [`crate::paint::verify_paint_accounting`].
pub fn install() {
    crate::paint::verify_paint_accounting();
}

#[cfg(test)]
mod tests {
    /// The shipping accounting distinguishes a complete pass from a short one.
    #[test]
    fn install_accepts_the_shipping_accounting() {
        super::install();
    }
}
