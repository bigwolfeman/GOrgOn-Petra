//! The Petra UI driver.
//!
//! One semantic tree, three consumers (`contracts/semantic-tree.md`); one
//! socket protocol, used by gates, by tests, and by agents alike
//! (`contracts/driver-protocol.md`). The server exists only when the
//! `testkit` feature is on: production binaries do not contain it, and no
//! runtime flag turns it on.
//!
//! [`wire`] carries no feature gate — it is the shared vocabulary the server
//! answers in and the future importable client (T033, FR-041) builds
//! requests and parses responses with. [`server`] is `testkit`-only.
//!
//! T029 (this wave) ships `health`, `tree` and `frame` for real, end to end.
//! `act`, `screenshot` and `wait_settle` answer `unsupported`, naming the
//! task that lands them (T030/T031 for `act`, T031 for `wait_settle`, T032
//! for `screenshot`). Remaining work: T031-T037 of
//! specs/003-petra-layout-engine/tasks.md.

pub mod invariant;
pub mod wire;

/// The driver server: unix socket, NDJSON framing, verb dispatch.
///
/// `testkit`-only (FR-042): declared here, behind this one `cfg`, is the
/// entire reason the endpoint cannot exist in a production build — no code
/// under `src/server/` compiles into the crate without this feature.
#[cfg(feature = "testkit")]
pub mod server;
