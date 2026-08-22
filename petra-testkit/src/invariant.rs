//! Package-owned invariant companion for `gorgon-petra-testkit`.

/// No runtime invariant: `gorgon-petra-testkit` carries no driver server, no
/// protocol codec, and no settle tracking yet (tasks T029-T037), so there is
/// no observable runtime relationship to assert. The first one this crate will
/// own is that the `testkit` feature and the presence of a listening socket
/// agree; it lands with the server.
pub fn install() {}
