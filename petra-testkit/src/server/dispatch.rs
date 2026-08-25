//! Verb dispatch: all six verbs, for real.
//!
//! `health`, `tree` and `frame` answer from what the application last
//! published — [`super::hub::FrameHub`]'s snapshot — and never touch the UI
//! thread. `act`, `screenshot` and `wait_settle` cannot work that way, and
//! the shapes they use are documented where they live:
//! [`crate::bridge`] for the job queue, [`crate::settle`] for what "settled"
//! means, [`crate::snapshot`] for the pixels.
//!
//! # Which verbs take the ordering slot, and why
//!
//! [`super::Server::action_order`] is acquired by `act` and by `wait_settle`,
//! for the whole verb, and by nothing else.
//!
//! * `act` holds it across both halves — inject, then wait for settle — which
//!   is what makes two clients' actions execute in arrival order (rule 4)
//!   rather than interleaving mid-gesture.
//! * `wait_settle` holds it so that an `act` arriving during a wait queues
//!   behind it instead of overtaking it. The cost is real and worth stating:
//!   one client's long `wait_settle` delays another client's `act` by up to
//!   its timeout. That is the same ordering promise applied consistently, and
//!   a driver that does not want it has `frame` and `tree`, which never take
//!   the slot.
//! * `screenshot` does **not** take it. It changes nothing, so serializing it
//!   behind actions would buy no ordering and would only make captures wait.
//!   It still goes through the bridge, so the frame it captures is a real
//!   painted one — see [`crate::snapshot`] for what "identity-verified"
//!   covers.

use serde_json::Value;

use gorgon_petra::semantic::{StateFlag, TreeQuery};
use gorgon_petra::tree::Interaction;
use gorgon_petra::{Modifiers, Point, Size};
use gorgon_petra_egui::inject::{Action, Target};

use super::Server;
use crate::bridge::{Answer, Job};
use crate::settle;
use crate::wire::{
    self, ActPayload, ErrorKind, Request, WireError, WireModifiers, WirePoint, WireRect, WireTarget,
};

/// Every verb this server recognizes.
const VERBS: &[&str] = &[
    "health",
    "tree",
    "frame",
    "act",
    "screenshot",
    "wait_settle",
];

pub(super) async fn dispatch(server: &Server, req: &Request) -> Result<Value, WireError> {
    if !VERBS.contains(&req.verb.as_str()) {
        return Err(WireError::new(
            ErrorKind::UnknownVerb,
            format!("unknown verb `{}`", req.verb),
        ));
    }
    match req.verb.as_str() {
        "health" => Ok(health(server)),
        "tree" => tree(server, &req.params),
        "frame" => frame(server),
        "act" => act(server, &req.params).await,
        "wait_settle" => wait_settle(server, &req.params).await,
        "screenshot" => screenshot(server, &req.params).await,
        _ => unreachable!("filtered by the VERBS check above"),
    }
}

/// `health`: `{app, pid, testkit_version, frame_seq}`. Never fails — liveness
/// and identity are answerable the instant the server exists, before any
/// frame has been published.
fn health(server: &Server) -> Value {
    let frame_seq = server.hub().current().map_or(0, |p| p.frame.seq);
    serde_json::to_value(wire::HealthResult {
        app: server.app_name().to_owned(),
        pid: std::process::id(),
        testkit_version: env!("CARGO_PKG_VERSION").to_owned(),
        frame_seq,
    })
    // `HealthResult` is four primitives; nothing here can produce a value
    // `serde_json` refuses (a non-finite float, an unpaired surrogate) short
    // of a bug in this function itself, so a failure here is exactly that —
    // not a wire-protocol case the closed error-kind set has room for.
    .expect("HealthResult serializes")
}

/// `tree`: `{id?, role?, label_contains?, state?}` -> a semantic node, or an
/// array of them.
///
/// The contract specifies the query semantics (`semantic-tree.md`) but
/// deliberately leaves open what a *filtered* response looks like — "a flat
/// list of matches, or the tree pruned to matches and their ancestors — is
/// the driver protocol's call" (`semantic/query.rs`'s own doc comment). This
/// server's call: an unfiltered request (no `role`/`label_contains`/`state`)
/// answers with one node — the full tree, or the subtree named by `id` — and
/// a filtered request answers with a flat array of matches, each carrying its
/// own subtree intact, via [`gorgon_petra::semantic::SemanticTree::find_all`]
/// directly rather than a second definition of "match". `id` narrows the
/// search space before filtering when both are given, so `{id, role}` means
/// "nodes with this role under this subtree", not "this id, if it also has
/// this role".
fn tree(server: &Server, params: &Value) -> Result<Value, WireError> {
    // Parsed before anything about server state is even looked at: a
    // malformed request is `invalid-params` whether or not a frame has been
    // published, and checking shape first means a caller with a typo in
    // `role` learns about the typo instead of a transient "not ready yet".
    let query = parse_tree_query(params)?;
    let Some(published) = server.hub().current() else {
        return Err(no_frame_yet());
    };
    let Some(whole) = published.tree.as_ref() else {
        // `semantic::project` returns `None` only for a frame with zero
        // placements, which `petrify` never produces. Reachable only from a
        // hand-built frame (`FrameHub`'s own tests do this); a real
        // application never publishes one.
        return Err(WireError::new(
            ErrorKind::Timeout,
            "the published frame has no placements to project a tree from",
        ));
    };
    let base = match params.get("id").and_then(Value::as_str) {
        Some(id) => match whole.subtree(id) {
            Some(sub) => sub,
            None => {
                return Err(WireError::new(
                    ErrorKind::StaleNode,
                    format!("no node with id `{id}` in frame {}", whole.frame_seq()),
                ));
            }
        },
        None => whole.clone(),
    };
    if query == TreeQuery::default() {
        return Ok(serde_json::to_value(base.root()).expect("SemanticNode serializes"));
    }
    let matches = base.find_all(&query);
    Ok(serde_json::to_value(matches).expect("a slice of SemanticNode serializes"))
}

fn parse_tree_query(params: &Value) -> Result<TreeQuery, WireError> {
    let mut query = TreeQuery::new();
    if let Some(v) = params.get("role") {
        let role = v
            .as_str()
            .ok_or_else(|| WireError::new(ErrorKind::InvalidParams, "`role` must be a string"))?;
        query = query.with_role(role);
    }
    if let Some(v) = params.get("label_contains") {
        let needle = v.as_str().ok_or_else(|| {
            WireError::new(
                ErrorKind::InvalidParams,
                "`label_contains` must be a string",
            )
        })?;
        query = query.with_label_contains(needle);
    }
    if let Some(v) = params.get("state") {
        let raw = v
            .as_str()
            .ok_or_else(|| WireError::new(ErrorKind::InvalidParams, "`state` must be a string"))?;
        query = query.with_state(parse_state_flag(raw)?);
    }
    Ok(query)
}

fn parse_state_flag(raw: &str) -> Result<StateFlag, WireError> {
    match raw {
        "focused" => Ok(StateFlag::Focused),
        "disabled" => Ok(StateFlag::Disabled),
        "selected" => Ok(StateFlag::Selected),
        "expanded" => Ok(StateFlag::Expanded),
        "truncated" => Ok(StateFlag::Truncated),
        "overflowed" => Ok(StateFlag::Overflowed),
        "stale" => Ok(StateFlag::Stale),
        "ambient" => Ok(StateFlag::Ambient),
        other => Err(WireError::new(
            ErrorKind::InvalidParams,
            format!(
                "unknown state flag `{other}`; known flags: focused, disabled, selected, \
                 expanded, truncated, overflowed, stale, ambient"
            ),
        )),
    }
}

/// `frame`: `{seq, digest, viewport, placements, hosted}`, built by
/// [`wire::frame_result`] straight off the published [`PetrifiedFrame`] —
/// `hosted` in particular is read from
/// [`gorgon_petra::frame::PetrifiedFrame::hosted`], never re-derived, because
/// nothing on the wire placements below (which carry payload *hashes*, not
/// payload content) would let a client recompute it.
fn frame(server: &Server) -> Result<Value, WireError> {
    let Some(published) = server.hub().current() else {
        return Err(no_frame_yet());
    };
    Ok(serde_json::to_value(wire::frame_result(&published.frame)).expect("FrameResult serializes"))
}

/// No frame has been published yet — the application exists and the socket
/// answers, but `Host::pass` has not run (or the application has not called
/// [`super::hub::FrameHub::publish`]) since the server started.
///
/// `driver-protocol.md`'s closed error-kind set has no "not ready yet" entry.
/// Of the five, [`ErrorKind::Timeout`] ("names pending work") is the closest
/// honest fit — the pending work is "the first frame" — and is used here
/// rather than inventing a sixth kind. This is a judgment call on a corner
/// the contract does not name explicitly; recorded as one in this crate's
/// Agent Note rather than left silent.
fn no_frame_yet() -> WireError {
    WireError::new(
        ErrorKind::Timeout,
        "no frame has been published yet; the application has not completed a Host::pass \
         since the driver server started",
    )
}

/// `act`: `{kind, target, payload?, timeout_ms?}` ->
/// `{applied_frame_seq, settled_frame_seq}`.
///
/// Both halves of "Action semantics" rule 2 happen here and in this order:
/// the events are injected and a pass runs
/// ([`crate::bridge::UiBridge::submit`], answered on the UI thread by
/// [`crate::driver_host::DriverHost::step`]), and only then does the settle
/// wait start, from the frame that applied them
/// ([`settle::settle_after_act`]). A frame older than the action can never
/// answer for it.
///
/// `timeout_ms` is not in the contract's parameter list. It is accepted
/// anyway, because rule 2 makes this verb blocking and the contract gives a
/// caller no other way to bound the wait; absent, [`settle::DEFAULT_TIMEOUT_MS`]
/// applies. A contract-conformant client that never sends it is unaffected.
async fn act(server: &Server, params: &Value) -> Result<Value, WireError> {
    let request = ActRequest::parse(params)?;
    // The slot is taken before anything is injected and held until the wait
    // finishes, so two clients' actions cannot interleave mid-gesture.
    let _guard = server.action_order().lock().await;
    // The scale comes from the frame on screen, so a device-pixel target
    // means the same thing to the caller and to the injector. No frame means
    // there is nothing to act on, and saying so beats guessing a scale.
    let Some(published) = server.hub().current() else {
        return Err(no_frame_yet());
    };
    let scale = published.frame.viewport.scale.factor();
    let (target, action) = request.to_action(scale)?;
    match server.bridge().submit(Job::Act { target, action }).await {
        Answer::Acted { applied_frame_seq } => {
            let result =
                settle::settle_after_act(server.hub(), applied_frame_seq, request.timeout_ms)
                    .await?;
            Ok(serde_json::to_value(result).expect("ActResult serializes"))
        }
        Answer::Refused(err) => Err(err),
        // `Job::Act` is answered by exactly one arm of
        // `DriverHost::step`, and it is not the capture arm.
        Answer::Captured { .. } => Err(WireError::new(
            ErrorKind::Timeout,
            "the UI thread answered an action with a capture; this is a bug in \
             gorgon-petra-testkit, not a protocol case",
        )),
    }
}

/// `wait_settle`: `{timeout_ms}` -> `{settled, frame_seq, pending}`.
///
/// Never fails. A deadline reached is `settled: false` with every pending
/// reason named — the contract's verb table gives this verb a `settled: bool`,
/// so a timeout is an answer the caller reads, not an error it catches.
async fn wait_settle(server: &Server, params: &Value) -> Result<Value, WireError> {
    let timeout_ms = parse_timeout(params)?;
    let _guard = server.action_order().lock().await;
    let result = settle::wait_settle(server.hub(), timeout_ms).await;
    Ok(serde_json::to_value(result).expect("SettleResult serializes"))
}

/// `screenshot`: `{region?}` -> `{seq, digest, png_base64, hosted}`.
///
/// The capture happens on the UI thread, against the frame that pass painted,
/// and the `(seq, digest)` on the response is that frame's — verified before
/// the PNG was produced, not asserted afterwards (FR-040).
async fn screenshot(server: &Server, params: &Value) -> Result<Value, WireError> {
    let region = parse_region(params)?;
    match server.bridge().submit(Job::Capture { region }).await {
        Answer::Captured {
            seq,
            digest,
            png,
            hosted,
        } => Ok(
            serde_json::to_value(wire::ScreenshotResult::new(seq, digest, &png, hosted))
                .expect("ScreenshotResult serializes"),
        ),
        Answer::Refused(err) => Err(err),
        Answer::Acted { .. } => Err(WireError::new(
            ErrorKind::Timeout,
            "the UI thread answered a capture with an action; this is a bug in \
             gorgon-petra-testkit, not a protocol case",
        )),
    }
}

/// `act`'s parsed parameters.
struct ActRequest {
    kind: Interaction,
    target: WireTarget,
    payload: ActPayload,
    timeout_ms: Option<u64>,
}

impl ActRequest {
    fn parse(params: &Value) -> Result<Self, WireError> {
        let kind = params
            .get("kind")
            .ok_or_else(|| WireError::new(ErrorKind::InvalidParams, "`act` needs a `kind`"))?;
        let kind: Interaction = serde_json::from_value(kind.clone()).map_err(|err| {
            WireError::new(
                ErrorKind::InvalidParams,
                format!(
                    "`kind` must be one of click, drag, hover, focus, text-edit, scroll, \
                     key: {err}"
                ),
            )
        })?;
        let target = params.get("target").ok_or_else(|| {
            WireError::new(
                ErrorKind::InvalidParams,
                "`act` needs a `target` of `{node_id}` or `{pos}`",
            )
        })?;
        let target: WireTarget = serde_json::from_value(target.clone()).map_err(|err| {
            WireError::new(
                ErrorKind::InvalidParams,
                format!("`target` must be `{{node_id}}` or `{{pos: {{x, y}}}}`: {err}"),
            )
        })?;
        let payload: ActPayload = match params.get("payload") {
            Some(raw) => serde_json::from_value(raw.clone()).map_err(|err| {
                WireError::new(ErrorKind::InvalidParams, format!("`payload`: {err}"))
            })?,
            None => ActPayload::default(),
        };
        Ok(Self {
            kind,
            target,
            payload,
            timeout_ms: parse_timeout(params)?,
        })
    }

    /// The request in the injector's own vocabulary.
    ///
    /// `scale` converts every device-pixel point on the wire into the logical
    /// units [`gorgon_petra_egui::inject`] works in — one conversion, here,
    /// so no other call site has to remember which units it holds.
    fn to_action(&self, scale: f32) -> Result<(Target, Action), WireError> {
        let target = match &self.target {
            WireTarget::NodeId(id) => Target::NodeId(id.clone()),
            WireTarget::Pos(p) => Target::Pos(logical_point(*p, scale)),
        };
        let modifiers = modifiers(self.payload.modifiers);
        let action = match self.kind {
            Interaction::Click => Action::Click { modifiers },
            Interaction::Hover => Action::Hover,
            Interaction::Focus => Action::Focus,
            Interaction::Drag => {
                let to = self.payload.to.ok_or_else(|| {
                    WireError::new(
                        ErrorKind::InvalidParams,
                        "`drag` needs `payload.to: {x, y}` — where the gesture ends",
                    )
                })?;
                Action::Drag {
                    to: logical_point(to, scale),
                    modifiers,
                }
            }
            Interaction::TextEdit => {
                let text = self.payload.text.clone().ok_or_else(|| {
                    WireError::new(
                        ErrorKind::InvalidParams,
                        "`text-edit` needs `payload.text` — the committed text",
                    )
                })?;
                Action::TextEdit { text }
            }
            Interaction::Scroll => {
                let delta = self.payload.delta.ok_or_else(|| {
                    WireError::new(
                        ErrorKind::InvalidParams,
                        "`scroll` needs `payload.delta: {x, y}` — the wheel delta",
                    )
                })?;
                Action::Scroll {
                    delta: Size::new(delta.x / scale, delta.y / scale),
                }
            }
            Interaction::Key => {
                let raw = self.payload.key.as_deref().ok_or_else(|| {
                    WireError::new(
                        ErrorKind::InvalidParams,
                        "`key` needs `payload.key` — the key's name, e.g. `enter`, `page-up`, \
                         `f5`, or a single character",
                    )
                })?;
                let key = wire::parse_key(raw).ok_or_else(|| {
                    WireError::new(
                        ErrorKind::InvalidParams,
                        format!("`{raw}` names no key; see `wire::key_name` for the spelling"),
                    )
                })?;
                Action::Key { key, modifiers }
            }
        };
        Ok((target, action))
    }
}

fn logical_point(p: WirePoint, scale: f32) -> Point {
    Point::new(p.x / scale, p.y / scale)
}

fn modifiers(m: WireModifiers) -> Modifiers {
    Modifiers {
        shift: m.shift,
        ctrl: m.ctrl,
        alt: m.alt,
        meta: m.meta,
    }
}

fn parse_timeout(params: &Value) -> Result<Option<u64>, WireError> {
    match params.get("timeout_ms") {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => raw.as_u64().map(Some).ok_or_else(|| {
            WireError::new(
                ErrorKind::InvalidParams,
                "`timeout_ms` must be a non-negative integer number of milliseconds",
            )
        }),
    }
}

fn parse_region(params: &Value) -> Result<Option<WireRect>, WireError> {
    match params.get("region") {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => serde_json::from_value::<WireRect>(raw.clone())
            .map(Some)
            .map_err(|err| {
                WireError::new(
                    ErrorKind::InvalidParams,
                    format!("`region` must be `{{x, y, w, h}}` in device pixels: {err}"),
                )
            }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{Server, dispatch};
    use crate::wire::{ErrorKind, Request};

    fn req(verb: &str, params: Value) -> Request {
        Request {
            id: json!(1),
            verb: verb.to_owned(),
            params,
        }
    }

    #[tokio::test]
    async fn an_unknown_verb_is_reported_as_such() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(&server, &req("no-such-verb", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::UnknownVerb);
        assert!(err.message.contains("no-such-verb"));
    }

    #[tokio::test]
    async fn health_answers_before_any_frame_is_published() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let result = dispatch(&server, &req("health", json!({}))).await.unwrap();
        assert_eq!(result["app"], "test-app");
        assert_eq!(result["pid"], std::process::id());
        assert_eq!(result["frame_seq"], 0);
    }

    #[tokio::test]
    async fn tree_before_any_publish_is_an_honest_not_yet() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(&server, &req("tree", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Timeout);
    }

    #[tokio::test]
    async fn a_malformed_role_filter_is_invalid_params() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(&server, &req("tree", json!({"role": 5})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
    }

    /// A frame with no placements: enough for the parameter checks below,
    /// which need only the viewport's scale, and never enough to be mistaken
    /// for a real UI. `petrify` never produces one.
    fn scale_only_frame() -> gorgon_petra::frame::PetrifiedFrame {
        use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, digest};
        use gorgon_petra::geom::Size;
        use gorgon_petra::token::ThemeMode;
        let viewport = Viewport::new(Size::new(100.0, 100.0), ThemeMode::Dark);
        PetrifiedFrame {
            seq: 1,
            digest: digest::digest(&viewport, &[]),
            placements: Vec::new(),
            content: Vec::new(),
            subtree_hashes: Vec::new(),
            subtree_len: Vec::new(),
            slots: Vec::new(),
            viewport,
            transitions: TransitionActivity::default(),
        }
    }

    #[tokio::test]
    async fn act_before_any_frame_is_an_honest_not_yet() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(
            &server,
            &req(
                "act",
                json!({"kind": "click", "target": {"node_id": "/root"}}),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Timeout);
        assert!(err.message.contains("Host::pass"), "{}", err.message);
    }

    #[tokio::test]
    async fn act_without_a_kind_is_invalid_params() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(&server, &req("act", json!({"target": {"node_id": "/x"}})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("kind"), "{}", err.message);
    }

    #[tokio::test]
    async fn an_unknown_action_kind_is_invalid_params_and_lists_the_seven() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(
            &server,
            &req(
                "act",
                json!({"kind": "double-click", "target": {"node_id": "/x"}}),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("text-edit"), "{}", err.message);
    }

    #[tokio::test]
    async fn a_drag_without_a_destination_is_invalid_params() {
        let (server, hub, _bridge) = Server::new("test-app");
        hub.publish(&scale_only_frame());
        let err = dispatch(
            &server,
            &req("act", json!({"kind": "drag", "target": {"node_id": "/x"}})),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("payload.to"), "{}", err.message);
    }

    #[tokio::test]
    async fn an_unknown_key_name_is_invalid_params_and_quotes_it() {
        let (server, hub, _bridge) = Server::new("test-app");
        hub.publish(&scale_only_frame());
        let err = dispatch(
            &server,
            &req(
                "act",
                json!({
                    "kind": "key",
                    "target": {"node_id": "/x"},
                    "payload": {"key": "super-enter"}
                }),
            ),
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("super-enter"), "{}", err.message);
    }

    #[tokio::test]
    async fn wait_settle_answers_rather_than_failing_when_nothing_was_published() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let result = dispatch(&server, &req("wait_settle", json!({"timeout_ms": 0})))
            .await
            .expect("wait_settle never errors");
        assert_eq!(result["settled"], false);
        assert_eq!(result["frame_seq"], 0);
        assert!(
            !result["pending"]["blocking"]
                .as_array()
                .expect("blocking is an array")
                .is_empty(),
            "a timeout with no reason named: {result}"
        );
    }

    #[tokio::test]
    async fn a_malformed_region_is_invalid_params() {
        let (server, _hub, _bridge) = Server::new("test-app");
        let err = dispatch(&server, &req("screenshot", json!({"region": "all of it"})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("device pixels"), "{}", err.message);
    }

    /// The seam `act`/`wait_settle` share: the ordering slot serializes
    /// concurrent holders, proven directly rather than through the (still
    /// unimplemented) verb behavior. A counter that would exceed 1 inside the
    /// critical section if two holders ever overlapped.
    #[tokio::test]
    async fn the_action_order_slot_admits_one_holder_at_a_time() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (server, _hub, _bridge) = Server::new("test-app");
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let server = Arc::clone(&server);
            let concurrent = Arc::clone(&concurrent);
            let max_seen = Arc::clone(&max_seen);
            tasks.push(tokio::spawn(async move {
                let _guard = server.action_order().lock().await;
                let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                max_seen.fetch_max(now, Ordering::SeqCst);
                tokio::task::yield_now().await;
                concurrent.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(
            max_seen.load(Ordering::SeqCst),
            1,
            "two action-order holders were inside the critical section at once"
        );
    }
}
