//! Real-socket tests for the driver client (T033).
//!
//! Every test here drives a real `tokio::net::UnixListener` with a scripted
//! peer that speaks the real NDJSON protocol back at [`Client`] — no test
//! calls a parsing function directly and calls it proven, because that
//! proves nothing about framing or id routing (the wave brief's own words).
//! This file builds with **no** feature on (`#![no_std]`-style ungated
//! crate, minus the `no_std` part): it exercises exactly the dependency set
//! FR-041 allows the client itself — `gorgon-petra-testkit`, `gorgon-petra`,
//! `tokio`, `serde_json` — proving the "importable everywhere" claim rather
//! than merely asserting it in a doc comment.
//!
//! Test function names are all prefixed `driver_` so `cargo test -p
//! gorgon-petra-testkit --no-default-features driver` (the T033 gate)
//! selects every one of them regardless of how cargo's test-name filtering
//! happens to interact with this file's own target name.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use gorgon_petra::tree::Interaction;
use gorgon_petra_testkit::driver::{ActTarget, Client, DriverError, TreeAnswer, TreeQuery};
use gorgon_petra_testkit::wire::ErrorKind;

/// Bind a fresh scripted-server listener under a temp dir, returning it
/// alongside the socket path a [`Client`] connects to.
async fn bind(dir: &Path) -> (UnixListener, PathBuf) {
    let path = dir.join("scripted.sock");
    let listener = UnixListener::bind(&path).expect("bind scripted server socket");
    (listener, path)
}

/// One accepted connection's read/write halves, framed the same way
/// `Client` and the real `Server` both frame NDJSON — hand-rolled here
/// exactly once, in the test harness that stands in for the real server,
/// which is the whole reason a driver client exists at all.
struct Peer {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl Peer {
    fn new(stream: tokio::net::UnixStream) -> Self {
        let (r, w) = stream.into_split();
        Self {
            reader: BufReader::new(r),
            writer: w,
        }
    }

    /// Read one request line, parsed enough to answer it: `(id, verb)`.
    async fn read_request(&mut self) -> (Value, String) {
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .expect("read request line");
        assert!(!line.is_empty(), "client closed before sending a request");
        let req: Value = serde_json::from_str(&line).expect("request parses as JSON");
        (req["id"].clone(), req["verb"].as_str().unwrap().to_owned())
    }

    async fn reply_ok(&mut self, id: &Value, result: Value) {
        let envelope = json!({"id": id, "ok": true, "result": result}).to_string();
        self.writer
            .write_all(format!("{envelope}\n").as_bytes())
            .await
            .expect("write reply");
    }

    async fn reply_err(&mut self, id: &Value, kind: &str, message: &str) {
        let envelope =
            json!({"id": id, "ok": false, "error": {"kind": kind, "message": message}}).to_string();
        self.writer
            .write_all(format!("{envelope}\n").as_bytes())
            .await
            .expect("write reply");
    }
}

fn canned_health() -> Value {
    json!({"app": "scripted", "pid": 4242, "testkit_version": "9.9.9", "frame_seq": 7})
}

fn canned_frame(seq: u64, digest: &str) -> Value {
    json!({
        "seq": seq,
        "digest": digest,
        "viewport": {"width": 100.0, "height": 100.0, "scale": 1.0, "theme_rev": 1, "theme_mode": "dark"},
        "placements": [],
        "hosted": false,
    })
}

fn canned_node(id: &str) -> Value {
    json!({
        "id": id,
        "label": "",
        "state": {},
        "bounds": {"x": 0, "y": 0, "w": 1, "h": 1},
        "frame_seq": 1,
        "actions": [],
        "children": [],
    })
}

fn canned_button(id: &str, label: &str) -> Value {
    json!({
        "id": id,
        "role": "button",
        "label": label,
        "state": {},
        "bounds": {"x": 0, "y": 0, "w": 1, "h": 1},
        "frame_seq": 1,
        "actions": ["click"],
        "children": [],
    })
}

/// `health` round-trips over a real socket: request framed correctly,
/// reply parsed into the typed result.
#[tokio::test]
async fn driver_health_round_trips_over_a_real_socket() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);
        let (id, verb) = peer.read_request().await;
        assert_eq!(verb, "health");
        peer.reply_ok(&id, canned_health()).await;
    });

    let client = Client::connect(&path).await.expect("connect");
    let health = client.health().await.expect("health succeeds");
    assert_eq!(health.app, "scripted");
    assert_eq!(health.pid, 4242);
    assert_eq!(health.frame_seq, 7);
}

/// The load-bearing property: two requests in flight on one connection are
/// matched to their replies **by id**, not by the order replies arrive on
/// the wire. The scripted server deliberately answers out of arrival order.
#[tokio::test]
async fn driver_matches_replies_by_id_not_by_arrival_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);

        let (id_a, verb_a) = peer.read_request().await;
        let (id_b, verb_b) = peer.read_request().await;

        // Figure out which request was `health` and which was `frame`,
        // whichever order the client happened to send them in, then answer
        // in the REVERSE of that arrival order — the second-arriving
        // request's reply is written first.
        let (health_id, frame_id) = if verb_a == "health" {
            assert_eq!(verb_b, "frame");
            (id_a, id_b)
        } else {
            assert_eq!(verb_a, "frame");
            assert_eq!(verb_b, "health");
            (id_b, id_a)
        };
        peer.reply_ok(&frame_id, canned_frame(99, "frame-digest"))
            .await;
        peer.reply_ok(&health_id, canned_health()).await;
    });

    let client = Client::connect(&path).await.expect("connect");
    let (health, frame) = tokio::join!(client.health(), client.frame());
    let health = health.expect("health succeeds");
    let frame = frame.expect("frame succeeds");

    // If id-routing were broken (e.g. "return whichever reply arrived
    // first, to whichever caller asked first"), `health` would receive the
    // frame envelope's `result` and fail to decode as `HealthResult`, or
    // `frame` would silently receive health's fields. Both must be exactly
    // their own typed result.
    assert_eq!(health.app, "scripted");
    assert_eq!(frame.seq, 99);
    assert_eq!(frame.digest, "frame-digest");
}

/// A server error's closed `kind` survives the client intact — never
/// flattened to a string, and never confused with a different kind.
#[tokio::test]
async fn driver_preserves_the_closed_error_kind_verbatim() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);
        let (id, verb) = peer.read_request().await;
        assert_eq!(verb, "tree");
        peer.reply_err(&id, "stale-node", "no node with id `/gone`")
            .await;
    });

    let client = Client::connect(&path).await.expect("connect");
    let err = client.find_by_id("/gone").await.unwrap_err();
    match err {
        DriverError::Server(wire_err) => {
            assert_eq!(wire_err.kind, ErrorKind::StaleNode, "{wire_err:?}");
            assert!(wire_err.message.contains("/gone"), "{}", wire_err.message);
        }
        other => panic!("expected DriverError::Server(StaleNode), got {other:?}"),
    }
}

/// `tree`'s asymmetry: an unfiltered query decodes as a single node, a
/// filtered query decodes as an array — and a finder that gets more than
/// one match reports the count rather than silently taking the first.
#[tokio::test]
async fn driver_tree_asymmetry_and_finder_arity_are_both_respected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);

        let (id1, verb1) = peer.read_request().await;
        assert_eq!(verb1, "tree");
        peer.reply_ok(&id1, canned_node("/root")).await;

        let (id2, verb2) = peer.read_request().await;
        assert_eq!(verb2, "tree");
        peer.reply_ok(
            &id2,
            json!([canned_button("/root/a", "A"), canned_button("/root/b", "B")]),
        )
        .await;
    });

    let client = Client::connect(&path).await.expect("connect");

    let unfiltered = client
        .tree(&TreeQuery::new())
        .await
        .expect("unfiltered tree succeeds");
    assert!(matches!(unfiltered, TreeAnswer::Node(node) if node.id == "/root"));

    let err = client.find_by_role("button").await.unwrap_err();
    match err {
        DriverError::NotExactlyOneMatch { found, .. } => assert_eq!(found, 2),
        other => panic!("expected NotExactlyOneMatch, got {other:?}"),
    }
}

/// A connection that closes while a request is still pending resolves that
/// request to `ConnectionClosed` instead of hanging forever.
#[tokio::test]
async fn driver_reports_connection_closed_instead_of_hanging() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);
        let _ = peer.read_request().await;
        // Deliberately drop the connection without ever replying.
        drop(peer);
    });

    let client = Client::connect(&path).await.expect("connect");
    let result = tokio::time::timeout(Duration::from_secs(5), client.health()).await;
    let result = result.expect("must not hang past the timeout");
    assert!(
        matches!(result, Err(DriverError::ConnectionClosed)),
        "{result:?}"
    );
}

/// A journey's evidence is real: each step's before/after frame seq/digest
/// come from actual `frame` round trips around the step's action, not from
/// restating the request.
#[tokio::test]
async fn driver_journey_captures_real_before_after_evidence() {
    use gorgon_petra_testkit::driver::JourneyStep;

    let dir = tempfile::tempdir().expect("tempdir");
    let (listener, path) = bind(dir.path()).await;
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut peer = Peer::new(stream);

        let (id1, verb1) = peer.read_request().await;
        assert_eq!(verb1, "frame");
        peer.reply_ok(&id1, canned_frame(1, "before-digest")).await;

        let (id2, verb2) = peer.read_request().await;
        assert_eq!(verb2, "act");
        peer.reply_ok(
            &id2,
            json!({"applied_frame_seq": 2, "settled_frame_seq": 2}),
        )
        .await;

        let (id3, verb3) = peer.read_request().await;
        assert_eq!(verb3, "frame");
        peer.reply_ok(&id3, canned_frame(2, "after-digest")).await;
    });

    let client = Client::connect(&path).await.expect("connect");
    let steps = vec![JourneyStep::act(
        "click go",
        Interaction::Click,
        ActTarget::NodeId("/root/go".to_owned()),
        None,
    )];
    let report = client.run_journey(steps).await;

    assert!(report.all_ok(), "{:?}", report.failure_report());
    assert_eq!(report.steps.len(), 1);
    let step = &report.steps[0];
    assert_eq!(step.before.frame_seq, 1);
    assert_eq!(step.before.digest, "before-digest");
    assert_eq!(step.after.frame_seq, 2);
    assert_eq!(step.after.digest, "after-digest");
}
