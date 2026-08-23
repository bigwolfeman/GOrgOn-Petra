//! Wire types for the Petra UI driver protocol.
//!
//! Binding: `specs/003-petra-layout-engine/contracts/driver-protocol.md`.
//! Field names here are the wire contract, not a style choice — three things
//! read them: [`crate::server`] (the `testkit`-only endpoint), the future
//! importable client (T033, FR-041), and any second implementation the
//! contract's own reference vectors exist for. This module carries no
//! feature gate on purpose: FR-041 makes the client importable everywhere, so
//! the types it builds requests from and parses responses into must compile
//! with no feature on, exactly like [`gorgon_petra::frame::PetrifiedFrame`]
//! itself does.
//!
//! # What lives here and what does not
//!
//! Request/response envelopes, the closed error-kind vocabulary, and the
//! `frame` verb's placement wire shape — the one thing `gorgon-petra` has no
//! serializable form for, because [`gorgon_petra::frame::Placement`] is an
//! internal engine type, not a wire type, and the driver contract needs a
//! *subset* of it (everything `contracts/frame-identity.md` §2's leaf hash
//! reads, so a consumer can recompute the digest from `(viewport,
//! placements)` alone) in device pixels, not logical units.
//!
//! `tree`'s result is deliberately **not** wrapped in a type here:
//! [`gorgon_petra::semantic::SemanticNode`] and
//! [`gorgon_petra::semantic::SemanticTree`] already serialize to the exact
//! wire shape `contracts/semantic-tree.md` fixes, and wrapping them a second
//! time would be a parallel definition of a shape one crate already owns —
//! [`crate::server`] serializes them directly.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use gorgon_petra::frame::{PetrifiedFrame, round_rect};

/// One NDJSON request line: `{id, verb, params}`.
///
/// `id` is carried as a bare [`Value`] rather than a fixed type: the contract
/// does not constrain its shape, and a server that echoes back exactly what
/// it was sent — the same choice `gorgond`'s ctl protocol makes
/// (`gorgon/gorgond/src/server.rs`) — cannot itself be the source of an id
/// mismatch a client has to debug.
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    /// Echoed back on the response unchanged. Missing on the wire is treated
    /// as `null`, the same way a request with no id at all is answered.
    #[serde(default)]
    pub id: Value,
    /// The verb name. An empty or absent verb is `unknown-verb`, not a parse
    /// failure, so a client sees one error vocabulary rather than two.
    pub verb: String,
    /// Verb-specific parameters. Missing on the wire is `{}`, so a verb that
    /// takes no parameters (`health`, `frame`) does not force every caller to
    /// write `"params": {}` by hand.
    #[serde(default = "empty_params")]
    pub params: Value,
}

fn empty_params() -> Value {
    Value::Object(serde_json::Map::new())
}

/// The closed error-kind vocabulary (`driver-protocol.md`).
///
/// Closed and additive-never: a new kind is a contract change, not a call
/// site's decision. There is deliberately no `Internal`/`Unknown` catch-all —
/// see [`crate::server::dispatch`] for what a handler does instead of
/// inventing one when something that should be unreachable happens anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The verb name is not one the server recognizes.
    UnknownVerb,
    /// The params for a known verb do not parse or type-check.
    InvalidParams,
    /// A `node_id` the request named no longer resolves.
    StaleNode,
    /// A wait exceeded its deadline; the message names what is pending.
    Timeout,
    /// The verb is known, but this build has no feature behind it. The
    /// message names why (`contracts/driver-protocol.md`: "carries why").
    Unsupported,
}

impl ErrorKind {
    /// This kind's wire spelling.
    #[must_use]
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::UnknownVerb => "unknown-verb",
            Self::InvalidParams => "invalid-params",
            Self::StaleNode => "stale-node",
            Self::Timeout => "timeout",
            Self::Unsupported => "unsupported",
        }
    }

    /// Parse a wire spelling back to its kind, or `None` for anything else.
    /// The other half of [`ErrorKind::as_wire`]; see
    /// `gorgon-petra-testkit`'s `src/invariant.rs` for the round-trip check
    /// this crate owns over the pair.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "unknown-verb" => Self::UnknownVerb,
            "invalid-params" => Self::InvalidParams,
            "stale-node" => Self::StaleNode,
            "timeout" => Self::Timeout,
            "unsupported" => Self::Unsupported,
            _ => return None,
        })
    }

    /// Every kind, in the order the contract's table lists them. What
    /// [`crate::invariant::install`] walks.
    pub const ALL: [Self; 5] = [
        Self::UnknownVerb,
        Self::InvalidParams,
        Self::StaleNode,
        Self::Timeout,
        Self::Unsupported,
    ];
}

/// A verb error: `{kind, message}` on the wire, nested under `error`.
#[derive(Debug, Clone)]
pub struct WireError {
    /// The closed kind.
    pub kind: ErrorKind,
    /// Teaching text. For `unsupported`, names the task that will implement
    /// the verb — an honest not-yet, never a fabricated result.
    pub message: String,
}

impl WireError {
    /// A new error.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Build a success envelope: `{id, ok: true, result}`.
#[must_use]
pub fn ok_response(id: &Value, result: Value) -> Value {
    serde_json::json!({"id": id, "ok": true, "result": result})
}

/// Build a failure envelope: `{id, ok: false, error: {kind, message}}`.
#[must_use]
pub fn err_response(id: &Value, err: &WireError) -> Value {
    serde_json::json!({
        "id": id,
        "ok": false,
        "error": {"kind": err.kind.as_wire(), "message": err.message},
    })
}

/// `health`'s result: `{app, pid, testkit_version, frame_seq}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResult {
    /// The application name the server was started with.
    pub app: String,
    /// This process's pid — the same one the socket path is named for.
    pub pid: u32,
    /// This crate's own version, so a client can tell a protocol mismatch
    /// from a bug.
    pub testkit_version: String,
    /// The most recently published frame's sequence, or `0` before the first
    /// one — `PetrifiedFrame::seq` starts at 1 and is never reused
    /// (`contracts/frame-identity.md`), so `0` cannot collide with a real
    /// frame and unambiguously means "none yet".
    pub frame_seq: u64,
}

/// A rect in device pixels, the same rounding the renderer and the digest use
/// ([`gorgon_petra::frame::round_rect`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireRect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
}

/// `frame`'s `viewport` member.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireViewport {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
    /// Device pixels per logical unit.
    pub scale: f32,
    /// Theme snapshot revision in force.
    pub theme_rev: u64,
    /// `"light"` or `"dark"`.
    pub theme_mode: String,
}

/// One placement, in exactly the fields `contracts/frame-identity.md` §2's
/// leaf hash reads — enough for a consumer to recompute the frame digest from
/// `(viewport, placements)` alone, which is the whole reason the `frame`
/// response ships placements at all rather than only a digest.
///
/// `parent` is not a digest input (frame-identity.md, "Not covered"), but it
/// is carried anyway: it is what a consumer needs to reconstruct §3's subtree
/// grouping, and without it a flat placement list would say nothing about
/// tree shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WirePlacement {
    /// Canonical key-path id.
    pub id: String,
    /// The node's kind, e.g. `"stack"`, `"text"`.
    pub kind: String,
    /// Final rect, device pixels.
    pub rect: WireRect,
    /// Paint order within the frame.
    pub z: i32,
    /// The clip in force, device pixels.
    pub clip: WireRect,
    /// Cumulative opacity in `[0, 1]`.
    pub opacity: f32,
    /// Hash of the node's rendered text, or zero.
    pub content_hash: u64,
    /// Whether content was hidden by truncation this frame.
    pub truncated: bool,
    /// Theme snapshot revision this node's tokens resolved against.
    pub token_revision: u64,
    /// Hash of this node's paint payload (`frame-identity.md`, "Paint
    /// payload hash").
    pub paint_hash: u64,
    /// Whether this placement holds keyboard focus this frame — the one
    /// semantic-payload member that is a digest input.
    pub focused: bool,
    /// Index of the parent placement in this same array, or `None` for the
    /// root.
    pub parent: Option<usize>,
}

/// `frame`'s result: `{seq, digest, viewport, placements, hosted}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameResult {
    /// Monotone within one application run.
    pub seq: u64,
    /// Hex-encoded 32-byte content fingerprint.
    pub digest: String,
    /// What the frame was negotiated against.
    pub viewport: WireViewport,
    /// Every node's final geometry, in tree pre-order.
    pub placements: Vec<WirePlacement>,
    /// Whether any placement was drawn by a host-registered custom painter
    /// (FR-060). Read from [`PetrifiedFrame::hosted`] — nothing on the wire
    /// otherwise distinguishes a host-drawn placement from a shipped one, so
    /// a client cannot derive this itself (`contracts/driver-protocol.md`,
    /// "Hosted content").
    pub hosted: bool,
}

/// Build the `frame` verb's result from a live [`PetrifiedFrame`].
///
/// Every field here is read straight off `frame`; nothing is recomputed. In
/// particular `hosted` is [`PetrifiedFrame::hosted`], not re-derived from the
/// placements this function emits — the wire placements below carry none of
/// the paint payload `hosted` inspects (image source, custom painter name),
/// only the payload's *hash*, so there would be nothing left here to derive
/// it from even if that were the goal.
#[must_use]
pub fn frame_result(frame: &PetrifiedFrame) -> FrameResult {
    let scale = frame.viewport.scale;
    let placements = frame
        .placements
        .iter()
        .map(|p| {
            let rect = round_rect(p.rect, scale);
            let clip = round_rect(p.clip, scale);
            WirePlacement {
                id: p.id.clone(),
                kind: p.kind.as_str().to_owned(),
                rect: WireRect {
                    x: rect.x,
                    y: rect.y,
                    w: rect.w,
                    h: rect.h,
                },
                z: p.z,
                clip: WireRect {
                    x: clip.x,
                    y: clip.y,
                    w: clip.w,
                    h: clip.h,
                },
                opacity: p.opacity,
                content_hash: p.paint.content_hash,
                truncated: p.paint.truncated,
                token_revision: p.paint.token_revision,
                paint_hash: p.paint.paint_hash,
                focused: p.semantics.focused,
                parent: p.parent,
            }
        })
        .collect();
    FrameResult {
        seq: frame.seq,
        digest: frame.digest.hex(),
        viewport: WireViewport {
            width: frame.viewport.size.w,
            height: frame.viewport.size.h,
            scale: scale.factor(),
            theme_rev: frame.viewport.theme_rev,
            theme_mode: frame.viewport.theme_mode.as_str().to_owned(),
        },
        placements,
        hosted: frame.hosted(),
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorKind, Request};

    #[test]
    fn a_request_with_no_params_defaults_to_an_empty_object() {
        let req: Request = serde_json::from_str(r#"{"id":1,"verb":"health"}"#).unwrap();
        assert_eq!(req.params, serde_json::json!({}));
        assert_eq!(req.id, serde_json::json!(1));
    }

    #[test]
    fn every_error_kind_round_trips_its_wire_spelling() {
        for kind in ErrorKind::ALL {
            assert_eq!(ErrorKind::parse(kind.as_wire()), Some(kind));
        }
    }

    #[test]
    fn an_unknown_spelling_does_not_parse() {
        assert_eq!(ErrorKind::parse("nope"), None);
    }
}
