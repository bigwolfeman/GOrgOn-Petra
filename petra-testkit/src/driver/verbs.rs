//! Typed request/result shapes for the four verbs `wire.rs` does not already
//! cover ([`crate::wire::HealthResult`] and [`crate::wire::FrameResult`] are
//! reused directly for `health` and `frame`).
//!
//! `act`'s `kind` reuses [`gorgon_petra::tree::Interaction`] rather than a
//! client-local enum: its `#[serde(rename_all = "kebab-case")]` already
//! prints the exact wire spellings `semantic-tree.md`'s Action vocabulary
//! fixes (`click`, `drag`, `hover`, `focus`, `text-edit`, `scroll`, `key`),
//! and reusing it is what keeps "what a driver asks for" and "what the tree
//! advertises in `actions`" the same type by construction — the exact
//! divergence FR-027 forbids, restated for the client side of the wire.

use serde_json::Value;

use gorgon_petra::tree::Interaction;

use crate::wire::WireRect;

/// `act`'s `target`: `{node_id}` or `{pos}` on the wire
/// (`contracts/driver-protocol.md`: "target: {node_id | pos}").
#[derive(Debug, Clone, PartialEq)]
pub enum ActTarget {
    /// Target a specific semantic node by its stable id. A stale id fails
    /// with [`crate::wire::ErrorKind::StaleNode`], never delivered to
    /// whatever occupies those coordinates now (FR-039).
    NodeId(String),
    /// Target a raw device-pixel point, for actions that are not about a
    /// specific node (e.g. a drag that starts off any widget).
    Pos {
        /// Device-pixel x.
        x: f32,
        /// Device-pixel y.
        y: f32,
    },
}

impl ActTarget {
    /// This target's wire form.
    #[must_use]
    pub fn to_wire(&self) -> Value {
        match self {
            Self::NodeId(id) => serde_json::json!({"node_id": id}),
            Self::Pos { x, y } => serde_json::json!({"pos": {"x": x, "y": y}}),
        }
    }
}

/// `act`'s and `wait_settle`'s results, re-exported from [`crate::wire`].
///
/// They are defined there, beside `HealthResult` and `FrameResult`, because
/// the server serializes exactly these structs — the same definition, not a
/// matching one. A client-local mirror would be a second copy of the protocol
/// free to drift from the server's, which is the one failure this crate is
/// laid out to make impossible. `SettleResult::pending` is a
/// [`crate::wire::Pending`], carrying both the sentences a person reads and
/// the counts a program branches on.
pub use crate::wire::{ActResult, SettleResult as WaitSettleResult};

/// `screenshot`'s result, re-exported from [`crate::wire`] for the same
/// reason [`ActResult`] is: the server serializes that exact struct.
/// [`crate::wire::ScreenshotResult::png_bytes`] hands back the PNG bytes, so
/// a caller does not need a base64 crate of its own.
pub use crate::wire::ScreenshotResult;

/// `screenshot`'s params: an optional capture region, device pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenshotRegion(pub Option<WireRect>);

impl ScreenshotRegion {
    pub(super) fn to_params(self) -> Value {
        match self.0 {
            Some(rect) => serde_json::json!({"region": rect}),
            None => serde_json::json!({}),
        }
    }
}

pub(super) fn act_params(kind: Interaction, target: &ActTarget, payload: Option<Value>) -> Value {
    let mut params = serde_json::json!({"kind": kind, "target": target.to_wire()});
    if let Some(payload) = payload {
        params
            .as_object_mut()
            .expect("built as an object above")
            .insert("payload".to_owned(), payload);
    }
    params
}

pub(super) fn wait_settle_params(timeout_ms: u64) -> Value {
    serde_json::json!({"timeout_ms": timeout_ms})
}

#[cfg(test)]
mod tests {
    use super::{ActTarget, act_params, wait_settle_params};
    use gorgon_petra::tree::Interaction;

    #[test]
    fn a_node_id_target_wires_as_node_id() {
        let wire = ActTarget::NodeId("/root/go".to_owned()).to_wire();
        assert_eq!(wire, serde_json::json!({"node_id": "/root/go"}));
    }

    #[test]
    fn a_pos_target_wires_as_pos() {
        let wire = ActTarget::Pos { x: 1.0, y: 2.0 }.to_wire();
        assert_eq!(wire, serde_json::json!({"pos": {"x": 1.0, "y": 2.0}}));
    }

    #[test]
    fn act_params_use_the_shared_interaction_vocabulary() {
        let params = act_params(Interaction::TextEdit, &ActTarget::NodeId("/x".into()), None);
        assert_eq!(params["kind"], "text-edit", "{params}");
        assert!(params.get("payload").is_none(), "{params}");
    }

    #[test]
    fn act_params_carry_a_payload_when_given() {
        let params = act_params(
            Interaction::Key,
            &ActTarget::NodeId("/x".into()),
            Some(serde_json::json!("Enter")),
        );
        assert_eq!(params["payload"], "Enter", "{params}");
    }

    #[test]
    fn wait_settle_params_carry_the_timeout() {
        assert_eq!(
            wait_settle_params(500),
            serde_json::json!({"timeout_ms": 500})
        );
    }
}
