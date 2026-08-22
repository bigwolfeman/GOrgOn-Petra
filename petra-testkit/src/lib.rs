//! The Petra UI driver.
//!
//! One semantic tree, three consumers (`contracts/semantic-tree.md`); one
//! socket protocol, used by gates, by tests, and by agents alike
//! (`contracts/driver-protocol.md`). The server exists only when the `testkit`
//! feature is on: production binaries do not contain it, and no runtime flag
//! turns it on.
//!
//! NOT YET IMPLEMENTED — tasks T029-T037 of
//! specs/003-petra-layout-engine/tasks.md.

pub mod invariant;
