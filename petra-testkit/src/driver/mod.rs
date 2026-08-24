//! The importable driver client (T033, FR-041).
//!
//! `contracts/driver-protocol.md`: "The testkit ships an importable driver
//! (`gorgon-petra-testkit::driver`) used by gates, tests, and agents alike
//! ... connect, query with finders (by role/label/id), act, wait,
//! screenshot-and-verify, and journey helpers with per-step evidence
//! capture. Hand-rolled per-test HTTP/socket code is a review-rejectable
//! defect."
//!
//! # Ungated, on purpose
//!
//! Unlike [`crate::server`], this module carries **no** `testkit` feature
//! gate, and must not gain one — the whole point of FR-041 is that any crate
//! can depend on `gorgon-petra-testkit` and drive a running application's UI
//! without pulling in the server, `gorgon-petra-egui`, or `nix`. That
//! constrains what this module (and everything under it) may import: only
//! [`crate::wire`], `gorgon-petra`, `tokio`, `serde`, `serde_json` — the same
//! four dependencies `wire.rs`'s own module docs hold itself to, and for the
//! identical reason. `cargo test -p gorgon-petra-testkit
//! --no-default-features driver` is what proves this stays true; see
//! `tests/driver.rs`.
//!
//! # Layout
//!
//! - [`client`]: [`Client`] — connect, the six typed verb methods, and the
//!   role/label/id finders built on `tree`.
//! - [`node`]: [`DriverNode`] and [`TreeQuery`]/[`TreeAnswer`] — the
//!   client-side mirror of the semantic tree's wire shape (`SemanticNode`
//!   itself has no `Deserialize` impl; see `node.rs`'s module docs for why
//!   that means a parallel type here rather than a shared one).
//! - [`verbs`]: typed shapes for `act`/`wait_settle`/`screenshot` — `health`
//!   and `frame` reuse [`crate::wire::HealthResult`] and
//!   [`crate::wire::FrameResult`] directly.
//! - [`journey`]: [`JourneyStep`]/[`JourneyReport`] — ordered steps with
//!   real per-step evidence (FR-044).
//! - [`discover`]: socket-path helpers that agree with
//!   `Server::socket_path_under` without importing it (that function is
//!   `testkit`-gated).
//! - [`error`]: [`DriverError`] — the client's own failures, plus
//!   [`crate::wire::WireError`] preserved verbatim for server-side ones.

pub mod client;
pub mod discover;
pub mod error;
pub mod journey;
pub mod node;
pub mod verbs;

pub use client::Client;
pub use discover::{discover_socket, socket_path, this_process_socket_path, xdg_runtime_dir};
pub use error::DriverError;
pub use journey::{Evidence, JourneyReport, JourneyStep, StepAction, StepOutcome, StepRecord};
pub use node::{DriverBounds, DriverNode, DriverState, TreeAnswer, TreeQuery};
pub use verbs::{ActResult, ActTarget, ScreenshotRegion, ScreenshotResult, WaitSettleResult};
