//! Verb dispatch: the handler seam T030 (`act`), T031 (`wait_settle` and real
//! settle-blocking for `act`) and T032 (`screenshot`) fill in.
//!
//! `health`, `tree` and `frame` are real, end to end, reading only what
//! already existed: [`gorgon_petra::semantic`]'s projection (via
//! [`super::hub::FrameHub`], which projects once per publish) and
//! [`gorgon_petra::frame::PetrifiedFrame::hosted`]. `act`, `screenshot` and
//! `wait_settle` answer [`ErrorKind::Unsupported`] naming the task that will
//! implement them — an honest not-yet, never a fabricated result, and never
//! a stub that returns as if it had run.

use serde_json::Value;

use gorgon_petra::semantic::{StateFlag, TreeQuery};

use super::Server;
use crate::wire::{self, ErrorKind, Request, WireError};

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
        "act" => {
            action(
                server,
                "act needs T030 (synthetic input injection over the real \
             platform-input boundary, gorgon/petra-egui/src/inject.rs) and T031 (settle \
             detection, gorgon/petra-testkit/src/settle.rs); neither has landed yet",
            )
            .await
        }
        "wait_settle" => {
            action(
                server,
                "wait_settle needs T031 (settle detection, \
             gorgon/petra-testkit/src/settle.rs); it has not landed yet",
            )
            .await
        }
        "screenshot" => Err(WireError::new(
            ErrorKind::Unsupported,
            "screenshot needs T032 (wgpu readback + identity verification, \
             gorgon/petra-testkit/src/snapshot/); it has not landed yet",
        )),
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
        "stale" => Ok(StateFlag::Stale),
        "ambient" => Ok(StateFlag::Ambient),
        other => Err(WireError::new(
            ErrorKind::InvalidParams,
            format!(
                "unknown state flag `{other}`; known flags: focused, disabled, selected, \
                 expanded, truncated, stale, ambient"
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

/// `act`/`wait_settle`: acquire the cross-connection ordering slot (see
/// `server/mod.rs`'s module docs), then answer `unsupported` naming `why`.
/// The acquire-then-release happens even though the body below does nothing
/// with the slot yet, so the ordering guarantee is already live: T030/T031
/// replace this function's body, not its entry point.
async fn action(server: &Server, why: &str) -> Result<Value, WireError> {
    let _guard = server.action_order().lock().await;
    Err(WireError::new(ErrorKind::Unsupported, why))
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
        let (server, _hub) = Server::new("test-app");
        let err = dispatch(&server, &req("no-such-verb", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::UnknownVerb);
        assert!(err.message.contains("no-such-verb"));
    }

    #[tokio::test]
    async fn health_answers_before_any_frame_is_published() {
        let (server, _hub) = Server::new("test-app");
        let result = dispatch(&server, &req("health", json!({}))).await.unwrap();
        assert_eq!(result["app"], "test-app");
        assert_eq!(result["pid"], std::process::id());
        assert_eq!(result["frame_seq"], 0);
    }

    #[tokio::test]
    async fn tree_before_any_publish_is_an_honest_not_yet() {
        let (server, _hub) = Server::new("test-app");
        let err = dispatch(&server, &req("tree", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Timeout);
    }

    #[tokio::test]
    async fn a_malformed_role_filter_is_invalid_params() {
        let (server, _hub) = Server::new("test-app");
        let err = dispatch(&server, &req("tree", json!({"role": 5})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
    }

    #[tokio::test]
    async fn act_and_wait_settle_are_unsupported_and_name_a_task() {
        let (server, _hub) = Server::new("test-app");
        let err = dispatch(&server, &req("act", json!({}))).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::Unsupported);
        assert!(err.message.contains("T030"), "{}", err.message);

        let err = dispatch(&server, &req("wait_settle", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Unsupported);
        assert!(err.message.contains("T031"), "{}", err.message);

        let err = dispatch(&server, &req("screenshot", json!({})))
            .await
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::Unsupported);
        assert!(err.message.contains("T032"), "{}", err.message);
    }

    /// The seam `act`/`wait_settle` share: the ordering slot serializes
    /// concurrent holders, proven directly rather than through the (still
    /// unimplemented) verb behavior. A counter that would exceed 1 inside the
    /// critical section if two holders ever overlapped.
    #[tokio::test]
    async fn the_action_order_slot_admits_one_holder_at_a_time() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let (server, _hub) = Server::new("test-app");
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
