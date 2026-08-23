//! Real-socket tests for the driver server (T029).
//!
//! Every test here drives the actual Unix socket with the actual NDJSON
//! framing — no test calls a handler function directly, because that would
//! prove nothing about framing, permissions, or dispatch (the wave brief's
//! own words). The frame these tests query is a real one: a headless
//! `gorgon-petra-egui::host::Host` pass over a real view tree, published
//! through the same [`gorgon_petra_testkit::server::FrameHub`] an embedding
//! application would use — never a hand-built `PetrifiedFrame`.

#![cfg(feature = "testkit")]

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use gorgon_petra::component::button;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Presenter, dark};
use gorgon_petra::tree::{NodeKind, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_testkit::server::Server;

/// A small tree with one real interactive node (`button`), so `tree` has
/// something with a role, a label, and actions to find.
struct DemoApp;

impl RowSource for DemoApp {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for DemoApp {
    fn view(&mut self) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(button("go", "Go"))
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

const WINDOW: [f32; 2] = [400.0, 300.0];

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx
}

/// A host, already run through one settled pass, so `host.frame()` is real.
fn settled_host() -> (Context, Host<DemoApp>) {
    let ctx = headless();
    let mut host = Host::new(&ctx, DemoApp, Presenter::new(dark()));
    ctx.run_ui(sized(RawInput::default()), |_| host.pass(&ctx))
        .drop_without_applying_deltas();
    (ctx, host)
}

/// A running server over a real socket under a fresh temp `XDG_RUNTIME_DIR`,
/// already carrying one published, real frame.
async fn running_server(dir: &Path) -> UnixStream {
    let (server, hub) = Server::new("driver-test-app");
    let listener = Server::bind_under(dir)
        .await
        .expect("bind under a fresh temp dir");
    let socket = Server::socket_path_under(dir);
    tokio::spawn(server.serve(listener));

    let (_ctx, host) = settled_host();
    hub.publish(host.frame().expect("a settled host has a frame"));

    UnixStream::connect(&socket)
        .await
        .unwrap_or_else(|err| panic!("connect to {}: {err}", socket.display()))
}

/// A request/response pair over one persistent connection, so a test can
/// issue several calls without reconnecting.
struct Client {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl Client {
    fn new(stream: UnixStream) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }

    async fn call(&mut self, id: i64, verb: &str, params: Value) -> Value {
        let request = json!({"id": id, "verb": verb, "params": params}).to_string();
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("write request");
        let mut line = String::new();
        self.reader
            .read_line(&mut line)
            .await
            .expect("read response");
        assert!(!line.is_empty(), "connection closed with no reply");
        serde_json::from_str(&line).unwrap_or_else(|err| panic!("malformed reply {line:?}: {err}"))
    }

    /// Write a raw line, bypassing [`Client::call`]'s request construction —
    /// for the malformed-JSON test.
    async fn send_raw(&mut self, line: &str) -> Value {
        self.writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("write raw line");
        let mut reply = String::new();
        self.reader
            .read_line(&mut reply)
            .await
            .expect("read response");
        serde_json::from_str(&reply)
            .unwrap_or_else(|err| panic!("malformed reply {reply:?}: {err}"))
    }
}

/// `health` over the real socket: well-formed, and identity matches this
/// process.
#[tokio::test]
async fn health_is_well_formed_over_the_real_socket() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "health", json!({})).await;
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["ok"], true, "{reply}");
    let result = &reply["result"];
    assert_eq!(result["app"], "driver-test-app");
    assert_eq!(result["pid"], std::process::id());
    assert!(result["testkit_version"].is_string(), "{result}");
    // A frame was published by `running_server` before the connection was
    // made, so `health` must see it — not the "no frame yet" honest-zero.
    assert!(result["frame_seq"].as_u64().unwrap() >= 1, "{result}");
}

/// `tree` round-trips a real projected tree: the `button` from `DemoApp` is
/// findable by role, carries the label and actions the component set, and
/// `frame_seq` matches `health`'s.
#[tokio::test]
async fn tree_round_trips_a_real_projected_tree() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let full = client.call(1, "tree", json!({})).await;
    assert_eq!(full["ok"], true, "{full}");
    let root = &full["result"];
    assert_eq!(root["id"], "/root");

    let filtered = client.call(2, "tree", json!({"role": "button"})).await;
    assert_eq!(filtered["ok"], true, "{filtered}");
    let matches = filtered["result"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an array of matches: {filtered}"));
    assert_eq!(matches.len(), 1, "{matches:?}");
    let node = &matches[0];
    assert_eq!(node["role"], "button");
    assert_eq!(node["label"], "Go");
    let actions = node["actions"].as_array().expect("actions array");
    let actions: Vec<&str> = actions.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(actions.contains(&"click"), "{actions:?}");
    assert!(actions.contains(&"focus"), "{actions:?}");

    // Same frame both responses came from.
    let health = client.call(3, "health", json!({})).await;
    assert_eq!(
        node["frame_seq"], health["result"]["frame_seq"],
        "tree and health must agree on which frame they answered from"
    );
}

/// `frame`'s wire placements carry the real button, in device pixels, and
/// `hosted` is `false` for a tree with no image or custom painter.
#[tokio::test]
async fn frame_carries_real_placements_and_an_honest_hosted_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "frame", json!({})).await;
    assert_eq!(reply["ok"], true, "{reply}");
    let result = &reply["result"];
    assert_eq!(result["hosted"], false, "{result}");
    assert!(result["digest"].as_str().unwrap().len() == 64, "{result}");
    let placements = result["placements"].as_array().expect("placements array");
    assert!(
        placements.len() >= 2,
        "root plus the button: {placements:?}"
    );
    let root = &placements[0];
    assert_eq!(root["id"], "/root");
    assert_eq!(root["parent"], Value::Null);
    // Every placement after the root names an earlier index as its parent —
    // proves `parent` made it onto the wire as real tree structure, not just
    // a flat list.
    for (i, p) in placements.iter().enumerate().skip(1) {
        let parent = p["parent"]
            .as_u64()
            .unwrap_or_else(|| panic!("placement {i} ({}) has no parent on the wire", p["id"]));
        assert!((parent as usize) < i, "{p}");
    }
}

/// An unknown verb is `unknown-verb`, over the real socket.
#[tokio::test]
async fn an_unknown_verb_is_unknown_verb_over_the_wire() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "not-a-real-verb", json!({})).await;
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(reply["error"]["kind"], "unknown-verb", "{reply}");
}

/// Malformed params (`role` is a number, not a string) is `invalid-params`.
#[tokio::test]
async fn malformed_params_are_invalid_params_over_the_wire() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "tree", json!({"role": 5})).await;
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(reply["error"]["kind"], "invalid-params", "{reply}");
}

/// A line that is not even JSON is also `invalid-params`, not a dropped
/// connection — the server answers rather than silently hanging up on a
/// malformed frame.
#[tokio::test]
async fn a_non_json_line_is_invalid_params_not_a_dropped_connection() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.send_raw("not json at all").await;
    assert_eq!(reply["ok"], false, "{reply}");
    assert_eq!(reply["error"]["kind"], "invalid-params", "{reply}");

    // The connection survives: a well-formed request right after still works.
    let health = client.call(2, "health", json!({})).await;
    assert_eq!(health["ok"], true, "{health}");
}

/// The same-uid check runs on every real connection to this socket: a
/// connection from this same process (the only peer a single-process test
/// can produce) is accepted and answers normally. The reject branch — a
/// different uid — is exercised directly against the pure comparison in
/// `gorgon_petra_testkit::server`'s `peer` module
/// (`src/server/peer.rs::tests::a_different_uid_is_refused`); no test
/// process can fabricate a second real uid to prove it end-to-end, which is
/// exactly the split that module's own doc comment explains.
#[tokio::test]
async fn the_same_uid_peer_check_runs_and_accepts_this_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);
    let reply = client.call(1, "health", json!({})).await;
    assert_eq!(reply["ok"], true, "{reply}");
}
