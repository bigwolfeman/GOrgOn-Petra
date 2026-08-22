//! Package-owned invariant companion for `gorgon-petra-egui`.

/// No runtime invariant: `gorgon-petra-egui` carries no host, painter, galley
/// cache, or input translation yet (task T013), so there is no observable
/// runtime relationship to assert. The first one this crate will own is that
/// every placement in a petrified frame reaches a painter in the frame's own
/// paint order — the way the D-069 posture fails quietly is a panel that is
/// simply never drawn, which the digest cannot see because it hashes
/// placements rather than pixels.
pub fn install() {}
