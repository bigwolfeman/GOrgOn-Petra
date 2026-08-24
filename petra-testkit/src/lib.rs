//! The Petra UI driver.
//!
//! One semantic tree, three consumers (`contracts/semantic-tree.md`); one
//! socket protocol, used by gates, by tests, and by agents alike
//! (`contracts/driver-protocol.md`). The server exists only when the
//! `testkit` feature is on: production binaries do not contain it, and no
//! runtime flag turns it on.
//!
//! [`wire`] and [`driver`] carry no feature gate — the client is importable
//! everywhere (FR-041), so the vocabulary it builds requests from and parses
//! responses into must compile with no feature on. Everything that can touch
//! the running UI is `testkit`-only: [`server`], [`bridge`],
//! [`driver_host`], [`settle`], [`snapshot`].
//!
//! All six verbs are real. `health`, `tree` and `frame` answer from the frame
//! the application last published; `act`, `wait_settle` and `screenshot` go
//! through [`bridge`] to the UI thread, because injecting input needs a live
//! `Host` and capturing pixels needs the pass's own shapes.
//!
//! # What is shipped and what is not
//!
//! Shipped: the socket and its six verbs, settle accounting with ambient
//! excluded (FR-039), headless GPU capture with server-side identity
//! verification (FR-040), and the importable client with finders and journey
//! helpers.
//!
//! Not shipped, so nobody reads the above as more than it is: driving a
//! *windowed* `eframe` application is not solved — egui 0.36 exposes no way
//! to push events into a running native integration, so a driver-controlled
//! application steps itself (see [`driver_host`]). The xtask lanes
//! (`petra-journey`, `petra-snapshots`) still fail as not-implemented, and
//! the production-exclusion proof is not written: T034-T037 of
//! specs/003-petra-layout-engine/tasks.md.

/// The importable driver client (T033, FR-041). Ungated on purpose — see
/// `driver/mod.rs`'s module docs for why it must build with no feature on.
pub mod driver;
pub mod invariant;
pub mod wire;

/// The driver server: unix socket, NDJSON framing, verb dispatch.
///
/// `testkit`-only (FR-042): declared here, behind this one `cfg`, is the
/// entire reason the endpoint cannot exist in a production build — no code
/// under `src/server/` compiles into the crate without this feature.
#[cfg(feature = "testkit")]
pub mod server;

/// The UI-thread seam: a job queue a driver task submits into and the
/// application drains once per loop iteration.
///
/// `testkit`-only for the same reason [`server`] is — it names
/// `gorgon-petra-egui` types and holds an `egui::Context`.
#[cfg(feature = "testkit")]
pub mod bridge;

/// The application-side loop: services driver jobs, runs a pass, publishes.
///
/// `testkit`-only: it owns a `gorgon-petra-egui` `Host`.
#[cfg(feature = "testkit")]
pub mod driver_host;

/// Settle detection: when the UI stopped moving, and which frame proves it.
///
/// `testkit`-only: it reads `bridge::SettleState` off `server::FrameHub`,
/// both of which are behind this feature.
#[cfg(feature = "testkit")]
pub mod settle;

/// Identity-verified screenshots: real pixels, headless (T032).
///
/// `testkit`-only for the same reason [`server`] is.
#[cfg(feature = "testkit")]
pub mod snapshot;
