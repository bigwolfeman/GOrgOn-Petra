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
    /// Whether content was hidden by a truncation rule this frame.
    pub truncated: bool,
    /// Whether the content is bigger than the rect, so the remainder is
    /// clipped away rather than elided.
    pub overflowed: bool,
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
                overflowed: p.paint.overflowed,
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

// ---------------------------------------------------------------------------
// Settle vocabulary
//
// `SettleState`, `Pending`, `SettleResult` and `ActResult` live here rather
// than beside the settle logic in `crate::settle`, for the same reason
// `HealthResult` and `FrameResult` do: three things read them and only one
// may depend on `egui`. The application writes a `SettleState` per pass
// (`crate::bridge`, `testkit`-only), `crate::settle` decides settledness from
// it (`testkit`-only), and the importable driver client parses the results
// off the wire with no feature on at all (FR-041). A copy in the client would
// be a second definition of the protocol, which is exactly the drift this
// module exists to prevent.
// ---------------------------------------------------------------------------

/// What one published frame reports about whether the UI is done moving.
///
/// Written by `crate::driver_host::DriverHost::step` at publish time, read
/// by [`crate::settle`]. Every field is measured at that moment, not
/// predicted: `queued_jobs` is `crate::bridge::UiBridge::queued` after the pass,
/// `repaint_pending` is what egui itself asked for, and the two transition
/// counts are the frame's own [`gorgon_petra::frame::TransitionActivity`].
///
/// Ambient is carried but **excluded from settled** (FR-039): a declared
/// endless animation never stops, so a settle that waited for it would never
/// return. Carrying it anyway is what lets `wait_settle`'s timeout say
/// "3 ambient animations are running, which do not block settle" instead of
/// leaving a caller to guess.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SettleState {
    /// The published frame's own sequence.
    pub frame_seq: u64,
    /// The sequence of the most recent frame that consumed an injected
    /// action, or `0` if none ever has. `PetrifiedFrame::seq` starts at 1, so
    /// `0` cannot collide with a real frame.
    pub applied_frame_seq: u64,
    /// Transitions still moving toward a target. Blocks settle.
    pub running_transitions: usize,
    /// Declared-endless animations. Does **not** block settle.
    pub ambient_transitions: usize,
    /// Jobs queued on the bridge and not yet serviced. Blocks settle.
    pub queued_jobs: usize,
    /// Whether egui asked for another frame. Blocks settle.
    pub repaint_pending: bool,
}

impl SettleState {
    /// The state a frame reports about itself, with nothing else known.
    ///
    /// What `crate::server::FrameHub::publish` records for an application
    /// that has no `crate::bridge::UiBridge`: the frame's own transition counts are real,
    /// and `queued_jobs`/`repaint_pending` are zero because with no bridge
    /// attached nothing can queue work and no driver is waiting on a
    /// repaint. An application that *does* run a bridge must go through
    /// `crate::server::FrameHub::publish_with` instead — this constructor
    /// would understate it.
    #[must_use]
    pub fn from_frame(frame: &gorgon_petra::frame::PetrifiedFrame) -> Self {
        Self {
            frame_seq: frame.seq,
            applied_frame_seq: 0,
            running_transitions: frame.transitions.running,
            ambient_transitions: frame.transitions.ambient,
            queued_jobs: 0,
            repaint_pending: false,
        }
    }
}

/// Why the UI is (or is not) done moving, in words a human can act on.
///
/// This is the `pending` member of `wait_settle`'s result and the body of
/// `act`'s timeout message. It is deliberately not a bitmask and not a bare
/// boolean: a caller that hits a timeout has to be able to read the answer
/// and know what to do next.
///
/// Both string lists and the raw counts are carried. The lists are what a
/// person reads; the counts are what a program branches on, so nobody has to
/// parse English to find out how many transitions were running.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// Every reason the UI has not settled, one sentence each. Empty means
    /// settled — see [`Pending::is_settled`].
    pub blocking: Vec<String>,
    /// Reasons that were observed and deliberately do **not** block settle:
    /// today, exactly declared-ambient animation (FR-039). Reported so a
    /// timeout does not read as silence about a UI that is visibly moving.
    pub not_blocking: Vec<String>,
    /// Transitions still moving toward a target. Blocks settle.
    pub running_transitions: usize,
    /// Jobs queued on the `crate::bridge::UiBridge` and not yet serviced.
    /// Blocks settle.
    pub queued_jobs: usize,
    /// Whether egui asked for another frame. Blocks settle.
    pub repaint_pending: bool,
    /// Declared-ambient animations. Reported, never blocking.
    pub ambient_transitions: usize,
}

/// `""` or `"s"`, so a reason reads "1 job queued" and not "1 jobs queued".
fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

impl Pending {
    /// Read one published frame's settle state into words.
    ///
    /// **This function is the settle predicate.** A condition blocks settle
    /// exactly when it pushes onto [`Pending::blocking`] here; nothing else
    /// in this crate decides settledness. Ambient goes to
    /// [`Pending::not_blocking`] instead (FR-039), which is why a frame with
    /// ambient animation and nothing else is settled.
    #[must_use]
    pub fn of(state: SettleState) -> Self {
        let mut blocking = Vec::new();
        if state.running_transitions > 0 {
            let n = state.running_transitions;
            blocking.push(format!("{n} transition{} running", plural(n)));
        }
        if state.queued_jobs > 0 {
            let n = state.queued_jobs;
            blocking.push(format!("{n} job{} queued", plural(n)));
        }
        if state.repaint_pending {
            blocking.push("a repaint is pending".to_owned());
        }
        let mut not_blocking = Vec::new();
        if state.ambient_transitions > 0 {
            let n = state.ambient_transitions;
            not_blocking.push(if n == 1 {
                "1 ambient animation running, which does not block settle (FR-039)".to_owned()
            } else {
                format!("{n} ambient animations running, which do not block settle (FR-039)")
            });
        }
        Self {
            blocking,
            not_blocking,
            running_transitions: state.running_transitions,
            queued_jobs: state.queued_jobs,
            repaint_pending: state.repaint_pending,
            ambient_transitions: state.ambient_transitions,
        }
    }

    /// The reason a wait that never saw a single frame is not settled.
    ///
    /// Distinct from "settled with nothing pending": an application that has
    /// not completed a `Host::pass` has not settled, it has not started.
    #[must_use]
    pub fn no_frame_yet() -> Self {
        Self {
            blocking: vec![
                "no frame has been published yet; the application has not completed a \
                 Host::pass since the driver server started"
                    .to_owned(),
            ],
            ..Self::default()
        }
    }

    /// The reason an `act` wait is not settled while the frame that applied
    /// the action has not been published yet.
    #[must_use]
    pub fn awaiting_applied_frame(applied_frame_seq: u64, newest_frame_seq: u64) -> Self {
        Self {
            blocking: vec![format!(
                "the frame that applied the action (seq {applied_frame_seq}) has not been \
                 published yet; the newest published frame is seq {newest_frame_seq}"
            )],
            ..Self::default()
        }
    }

    /// The reason a wait ended because the hub stopped existing.
    ///
    /// Unreachable while the caller holds a [`FrameHub`] — the hub *is* the
    /// sender — but handled in words rather than by panicking, so a future
    /// caller that holds only a receiver gets an answer instead of an abort.
    #[must_use]
    pub fn publisher_gone() -> Self {
        Self {
            blocking: vec![
                "the application dropped its frame publisher; no further frames can arrive"
                    .to_owned(),
            ],
            ..Self::default()
        }
    }

    /// Whether nothing blocks settle. Ambient in [`Pending::not_blocking`]
    /// does not make this false — that is FR-039, and it is the whole reason
    /// the two lists are separate.
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.blocking.is_empty()
    }

    /// One line naming every reason, blocking first, non-blocking in
    /// parentheses. What a `timeout` error message carries.
    #[must_use]
    pub fn describe(&self) -> String {
        let head = if self.blocking.is_empty() {
            "nothing blocks settle".to_owned()
        } else {
            self.blocking.join("; ")
        };
        if self.not_blocking.is_empty() {
            head
        } else {
            format!("{head} ({})", self.not_blocking.join("; "))
        }
    }
}

/// Whether a published frame's settle state means the UI is done moving.
///
/// Delegates to [`Pending::of`] so there is one definition of the rule, not
/// two that can drift. Ambient is excluded (FR-039).
#[must_use]
pub fn is_settled(state: SettleState) -> bool {
    Pending::of(state).is_settled()
}

/// `wait_settle`'s result: `{settled, frame_seq, pending}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettleResult {
    /// Whether a settled frame was observed before the deadline.
    pub settled: bool,
    /// The frame the answer is about: the settled frame when `settled`, else
    /// the newest frame the wait ever saw. `0` means no frame was ever seen —
    /// `PetrifiedFrame::seq` starts at 1, so it cannot collide.
    ///
    /// This is the frame's own `seq`, the same number `health`, `frame` and
    /// `screenshot` report, never [`SettleState::frame_seq`] (the publisher's
    /// copy of it) — so a driver can never see one frame under two numbers.
    pub frame_seq: u64,
    /// Why, in words. On a timeout, every reason that was actually true.
    pub pending: Pending,
}

/// `act`'s result: `{applied_frame_seq, settled_frame_seq}`.
///
/// See the module docs' "`applied_frame_seq` vs `settled_frame_seq`" for
/// which one a test author should assert on. In short: `applied` answers "did
/// my input land", `settled` answers "is the UI done reacting to it".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActResult {
    /// The frame that consumed the injected events. Never zero for a
    /// serviced action.
    pub applied_frame_seq: u64,
    /// The first frame at or after `applied_frame_seq` with nothing pending.
    /// Greater than or equal to `applied_frame_seq`, and equal to it when the
    /// action started nothing that animates.
    pub settled_frame_seq: u64,
}

// ---------------------------------------------------------------------------
// `act`'s parameters
//
// `contracts/driver-protocol.md` fixes `{kind, target: {node_id | pos},
// payload?}` and leaves the payload's shape to the implementation. It is
// spelled out here rather than in the server because the client has to build
// exactly what the server parses, and `gorgon_petra::KeyCode` and
// `Modifiers` carry no serde derives — so without one shared spelling the two
// sides would each invent one.
// ---------------------------------------------------------------------------

/// A point on the wire, in **device pixels** — the same units and rounding
/// [`WirePlacement::rect`] carries.
///
/// Device rather than logical, so a client that read a placement out of a
/// `frame` response can aim at it without knowing the display scale. The
/// server divides by `viewport.scale` before handing the point to the
/// injector, which works in logical units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WirePoint {
    /// Device-pixel x.
    pub x: f32,
    /// Device-pixel y.
    pub y: f32,
}

/// `act`'s `target`: exactly one of `{node_id}` or `{pos}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireTarget {
    /// A stable semantic-tree id. A stale one fails with
    /// [`ErrorKind::StaleNode`] and delivers nothing (FR-039) — it never
    /// falls through to whatever now occupies those coordinates.
    NodeId(String),
    /// A raw device-pixel point. Nothing about it can go stale.
    Pos(WirePoint),
}

/// Chord modifiers on the wire. Absent means none held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireModifiers {
    /// Shift.
    #[serde(default)]
    pub shift: bool,
    /// Control.
    #[serde(default)]
    pub ctrl: bool,
    /// Alt / Option.
    #[serde(default)]
    pub alt: bool,
    /// Command / Super / Windows.
    #[serde(default)]
    pub meta: bool,
}

/// `act`'s `payload`: the members the requested `kind` needs, and no others.
///
/// One flat optional set rather than a per-kind enum, because the wire's
/// discriminant is already `kind` and a second one would let a request name
/// two different actions. Which members a kind requires is checked by the
/// server, which is the only side that can answer "this frame cannot do
/// that".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActPayload {
    /// `text-edit`: the committed text, as a paste or an IME commit would
    /// deliver it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// `key`: the key's name — see [`key_name`] for the spelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// `drag`: where the gesture ends, device pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<WirePoint>,
    /// `scroll`: the wheel delta, device pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delta: Option<WirePoint>,
    /// Modifiers held for the whole gesture. `scroll` carries none — Petra's
    /// own scroll event has no modifier field, so a value here would be
    /// discarded rather than honoured.
    #[serde(default)]
    pub modifiers: WireModifiers,
}

/// A key's wire spelling.
///
/// Lower-case, kebab for the two-word names, `f1`..`f24` for function keys,
/// and the character itself for a printable key. Closed by
/// [`gorgon_petra::KeyCode`] being closed; [`parse_key`] is the exact
/// inverse, which `crate::invariant` checks over every non-parameterised
/// variant.
#[must_use]
pub fn key_name(key: gorgon_petra::KeyCode) -> String {
    use gorgon_petra::KeyCode as K;
    match key {
        K::Tab => "tab".to_owned(),
        K::Enter => "enter".to_owned(),
        K::Escape => "escape".to_owned(),
        K::Space => "space".to_owned(),
        K::Backspace => "backspace".to_owned(),
        K::Delete => "delete".to_owned(),
        K::Up => "up".to_owned(),
        K::Down => "down".to_owned(),
        K::Left => "left".to_owned(),
        K::Right => "right".to_owned(),
        K::Home => "home".to_owned(),
        K::End => "end".to_owned(),
        K::PageUp => "page-up".to_owned(),
        K::PageDown => "page-down".to_owned(),
        K::Function(n) => format!("f{n}"),
        K::Char(c) => c.to_string(),
    }
}

/// Parse a key's wire spelling, or `None` for anything that names no key.
///
/// A single character is a printable key, lower-cased the same way
/// [`gorgon_petra::KeyCode::Char`] documents. `f0` and `f25` name no key: the
/// range is 1..=24, and a caller that meant a character gets the one-character
/// arm instead.
#[must_use]
pub fn parse_key(raw: &str) -> Option<gorgon_petra::KeyCode> {
    use gorgon_petra::KeyCode as K;
    Some(match raw {
        "tab" => K::Tab,
        "enter" => K::Enter,
        "escape" => K::Escape,
        "space" => K::Space,
        "backspace" => K::Backspace,
        "delete" => K::Delete,
        "up" => K::Up,
        "down" => K::Down,
        "left" => K::Left,
        "right" => K::Right,
        "home" => K::Home,
        "end" => K::End,
        "page-up" => K::PageUp,
        "page-down" => K::PageDown,
        other => {
            if let Some(digits) = other.strip_prefix('f')
                && digits.len() <= 2
                && let Ok(n) = digits.parse::<u8>()
                && (1..=24).contains(&n)
            {
                return Some(K::Function(n));
            }
            let mut chars = other.chars();
            let (first, rest) = (chars.next()?, chars.next());
            if rest.is_some() {
                return None;
            }
            K::Char(first.to_ascii_lowercase())
        }
    })
}

/// `screenshot`'s result: `{seq, digest, png_base64, hosted}`.
///
/// `png_base64` is text because the envelope is JSON, which has no byte
/// type. [`ScreenshotResult::png_bytes`] is the way back; a client never has
/// to know which base64 alphabet was used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenshotResult {
    /// The sequence of the frame that was actually captured and verified
    /// (FR-040) — not whichever frame happened to be current when the
    /// request arrived.
    pub seq: u64,
    /// That frame's digest, hex-encoded.
    pub digest: String,
    /// Standard-alphabet base64 of the PNG bytes, with padding.
    pub png_base64: String,
    /// Whether any placement in the captured frame was drawn by a
    /// host-registered custom painter (FR-060). Load-bearing: a consumer
    /// comparing screenshots by digest must refuse or qualify the comparison
    /// when this is true (`contracts/driver-protocol.md`, "Hosted content").
    pub hosted: bool,
}

impl ScreenshotResult {
    /// Build one from a capture's raw parts.
    #[must_use]
    pub fn new(seq: u64, digest: String, png: &[u8], hosted: bool) -> Self {
        use base64::Engine as _;
        Self {
            seq,
            digest,
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            hosted,
        }
    }

    /// The PNG bytes.
    ///
    /// # Errors
    /// The decode error, when `png_base64` is not valid base64 — which for a
    /// response this crate produced can only mean the transport corrupted it.
    pub fn png_bytes(&self) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.decode(&self.png_base64)
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
