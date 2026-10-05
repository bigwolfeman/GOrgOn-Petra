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

mod support;

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

use egui::{Context, RawInput};
use serde_json::{Value, json};
use tokio::net::UnixStream;

use gorgon_petra::component::button;
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Presenter, dark};
use gorgon_petra::tree::{NodeKind, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_testkit::server::Server;
use support::{Client, driven_server, driven_with_client, headless, sized};

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

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
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
    let (server, hub, _bridge) = Server::new("driver-test-app");
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

/// `health` over the real socket: well-formed, and identity matches this
/// process.
#[tokio::test]
async fn health_is_well_formed_over_the_real_socket() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "health", json!({})).await;
    assert_eq!(reply["id"], 1);
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
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
    assert!(full["ok"].as_bool() == Some(true), "{full}");
    let root = &full["result"];
    assert_eq!(root["id"], "/root");

    let filtered = client.call(2, "tree", json!({"role": "button"})).await;
    assert!(filtered["ok"].as_bool() == Some(true), "{filtered}");
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
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
    let result = &reply["result"];
    assert!(result["hosted"].as_bool() == Some(false), "{result}");
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
    assert!(reply["ok"].as_bool() == Some(false), "{reply}");
    assert_eq!(reply["error"]["kind"], "unknown-verb", "{reply}");
}

/// Malformed params (`role` is a number, not a string) is `invalid-params`.
#[tokio::test]
async fn malformed_params_are_invalid_params_over_the_wire() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stream = running_server(dir.path()).await;
    let mut client = Client::new(stream);

    let reply = client.call(1, "tree", json!({"role": 5})).await;
    assert!(reply["ok"].as_bool() == Some(false), "{reply}");
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
    assert!(reply["ok"].as_bool() == Some(false), "{reply}");
    assert_eq!(reply["error"]["kind"], "invalid-params", "{reply}");

    // The connection survives: a well-formed request right after still works.
    let health = client.call(2, "health", json!({})).await;
    assert!(health["ok"].as_bool() == Some(true), "{health}");
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
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
}

// ---------------------------------------------------------------------------
// The three verbs that need the UI thread.
//
// `health`, `tree` and `frame` above answer from a published snapshot, so a
// test can publish one by hand and never run a loop. `act`, `wait_settle` and
// `screenshot` cannot: an action has to be injected into the input of a real
// pass, and a capture has to happen while the pass's shapes still exist. So
// these tests stand up a real driver-controlled application on its own thread
// — `DriverHost` stepping a real `Host` — and talk to it over the real
// socket. Nothing here calls a handler directly.
//
// This is also where the check deleted from `dispatch.rs` went. That test
// asserted `act`/`wait_settle`/`screenshot` answered `unsupported` naming
// their task; it could not survive them being implemented. What replaces it is
// stronger in both directions: the parameter-validation tests that stayed in
// `dispatch.rs` (a missing `kind`, an unknown kind, a `drag` with no
// destination, an unknown key name, a malformed region), and the end-to-end
// tests below, which assert the verbs do the thing rather than that they
// decline to.
// ---------------------------------------------------------------------------

/// `act` all the way down: the request crosses the socket, the events are
/// injected into a real pass's input, and both frame numbers come back.
#[tokio::test]
async fn act_clicks_a_real_button_and_reports_both_frames() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, mut client) = driven_server(dir.path()).await;

    let before = button_label(&mut client, 99).await;
    assert_eq!(
        before, "Go 0",
        "the application did not start at zero clicks"
    );

    let reply = client
        .call(
            100,
            "act",
            json!({"kind": "click", "target": {"node_id": "/root/go"}, "timeout_ms": 5000}),
        )
        .await;
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
    let applied = reply["result"]["applied_frame_seq"]
        .as_u64()
        .unwrap_or_else(|| panic!("no applied_frame_seq: {reply}"));
    let settled = reply["result"]["settled_frame_seq"]
        .as_u64()
        .unwrap_or_else(|| panic!("no settled_frame_seq: {reply}"));
    assert!(applied > 0, "an action was applied by no frame: {reply}");
    assert!(
        settled >= applied,
        "the UI settled before the action was applied: {reply}"
    );

    // The load-bearing assertion. `act` returning two plausible numbers
    // proves the verb answered; only the application's own state proves the
    // click was delivered, and this reads it back through the semantic tree.
    let after = button_label(&mut client, 101).await;
    assert_eq!(
        after, "Go 1",
        "the click never reached the application: the button still reads {before:?}"
    );
}

/// The driven application's button label, read over the socket.
async fn button_label(client: &mut Client, id: i64) -> String {
    let reply = client.call(id, "tree", json!({"role": "button"})).await;
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
    let matches = reply["result"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an array of matches: {reply}"));
    assert_eq!(matches.len(), 1, "{reply}");
    matches[0]["label"]
        .as_str()
        .unwrap_or_else(|| panic!("no label: {reply}"))
        .to_owned()
}

/// Rule 3: a stale target fails loudly and delivers nothing.
#[tokio::test]
async fn a_stale_node_id_fails_with_stale_node_and_names_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, mut client) = driven_server(dir.path()).await;

    let reply = client
        .call(
            100,
            "act",
            json!({"kind": "click", "target": {"node_id": "/root/never-existed"}}),
        )
        .await;
    assert!(reply["ok"].as_bool() == Some(false), "{reply}");
    assert_eq!(reply["error"]["kind"], "stale-node", "{reply}");
    assert!(
        reply["error"]["message"]
            .as_str()
            .expect("a message")
            .contains("never-existed"),
        "the error does not name the node: {reply}"
    );
}

/// A UI with nothing moving settles, and says so with no reason left over.
#[tokio::test]
async fn wait_settle_answers_settled_for_a_quiet_ui() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, mut client) = driven_server(dir.path()).await;

    let reply = client
        .call(100, "wait_settle", json!({"timeout_ms": 5000}))
        .await;
    assert!(reply["ok"].as_bool() == Some(true), "{reply}");
    assert!(reply["result"]["settled"].as_bool() == Some(true), "{reply}");
    assert!(
        reply["result"]["frame_seq"].as_u64().unwrap_or(0) > 0,
        "settled on no frame at all: {reply}"
    );
    let blocking = reply["result"]["pending"]["blocking"]
        .as_array()
        .unwrap_or_else(|| panic!("no pending.blocking: {reply}"));
    assert!(
        blocking.is_empty(),
        "settled: true beside a blocking reason: {reply}"
    );
}

/// FR-040: the PNG is of the frame the response names. Proved by comparing
/// the capture's digest against what `frame` reports for the same quiet UI —
/// a capture that answered with a different frame's identity would differ.
#[tokio::test]
async fn screenshot_returns_a_png_of_the_frame_it_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, mut client) = driven_server(dir.path()).await;

    // Change what is on screen first, so "the frame it names" is a real
    // claim. Against a static UI every frame carries the same digest, and a
    // capture that answered with the wrong frame's identity would be
    // indistinguishable from one that answered correctly.
    let before = client.call(98, "frame", json!({})).await["result"]["digest"]
        .as_str()
        .expect("a digest")
        .to_owned();
    let acted = client
        .call(
            99,
            "act",
            json!({"kind": "click", "target": {"node_id": "/root/go"}, "timeout_ms": 5000}),
        )
        .await;
    assert!(acted["ok"].as_bool() == Some(true), "{acted}");

    let reply = client.call(100, "screenshot", json!({})).await;
    if reply["ok"].as_bool() == Some(false) {
        // A build with no reachable GPU adapter cannot capture. That is a
        // missing prerequisite, and the contract says a gate names it rather
        // than skipping (FR-043) — so this asserts the *shape* of the honest
        // refusal instead of passing silently.
        assert_eq!(reply["error"]["kind"], "unsupported", "{reply}");
        panic!(
            "screenshot could not capture on this machine; the driver refused honestly, but \
             this test proves nothing here: {reply}"
        );
    }
    let seq = reply["result"]["seq"].as_u64().expect("a seq");
    let digest = reply["result"]["digest"].as_str().expect("a digest");
    assert!(seq > 0, "{reply}");
    assert_eq!(digest.len(), 64, "a digest is 32 hex bytes: {reply}");

    let frame = client.call(101, "frame", json!({})).await;
    assert_eq!(
        frame["result"]["digest"].as_str().expect("a digest"),
        digest,
        "the capture named a different frame's content than the one on screen"
    );
    assert_eq!(
        reply["result"]["hosted"], frame["result"]["hosted"],
        "{reply}"
    );
    assert_ne!(
        digest, before,
        "the capture named the pre-click frame's content, so this test could not tell a \
         correct capture from a stale one"
    );

    let png = reply["result"]["png_base64"].as_str().expect("base64");
    assert!(!png.is_empty(), "{reply}");
    let bytes = gorgon_petra_testkit::wire::ScreenshotResult {
        seq,
        digest: digest.to_owned(),
        png_base64: png.to_owned(),
        hosted: false,
    }
    .png_bytes()
    .expect("the base64 decodes");
    assert_eq!(
        &bytes[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        "the payload is not a PNG"
    );
}

/// A region outside the frame is refused rather than clamped, so a caller
/// never gets a picture of somewhere else.
#[tokio::test]
async fn a_region_outside_the_viewport_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, mut client) = driven_server(dir.path()).await;

    let reply = client
        .call(
            100,
            "screenshot",
            json!({"region": {"x": 100000, "y": 100000, "w": 10, "h": 10}}),
        )
        .await;
    assert!(reply["ok"].as_bool() == Some(false), "{reply}");
    assert_eq!(reply["error"]["kind"], "invalid-params", "{reply}");
}

// ---------------------------------------------------------------------------
// The importable client against the real server.
//
// T033's own tests drive a scripted listener, which proves framing, id routing
// and error mapping without needing the `testkit` feature — and deliberately
// cannot prove that the shapes it parses are the shapes this server emits.
// That is what these two tests are for, and why they live here: this is the
// only file in the crate where a real `Client` and a real `Server` can meet.
// ---------------------------------------------------------------------------

use gorgon_petra::tree::Interaction;
use gorgon_petra_testkit::driver::{ActTarget, JourneyStep, TreeAnswer, TreeQuery};

/// Every verb the client types, against the server that actually emits those
/// shapes. A field the client named differently would fail to decode here,
/// which is the whole point.
#[tokio::test]
async fn the_importable_client_speaks_to_the_real_server() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, driver) = driven_with_client(dir.path()).await;

    let health = driver.health().await.expect("health decodes");
    assert_eq!(health.app, "driven-test-app");
    assert_eq!(health.pid, std::process::id());
    assert!(health.frame_seq > 0);

    let frame = driver.frame().await.expect("frame decodes");
    assert!(!frame.placements.is_empty(), "a real frame has placements");
    assert_eq!(frame.digest.len(), 64);

    // The finder, and the server's documented asymmetry: a filtered query
    // answers an array, an unfiltered one answers a node.
    let button = driver
        .find_by_role("button")
        .await
        .expect("exactly one button");
    assert_eq!(button.label, "Go 0");
    match driver
        .tree(&TreeQuery::default())
        .await
        .expect("an unfiltered tree decodes")
    {
        TreeAnswer::Node(root) => assert_eq!(root.id, "/root"),
        TreeAnswer::Matches(matches) => {
            panic!("an unfiltered query answered an array of {}", matches.len())
        }
    }

    let settled = driver
        .wait_settle(5_000)
        .await
        .expect("wait_settle decodes");
    assert!(settled.settled, "{settled:?}");
    assert!(
        settled.pending.blocking.is_empty(),
        "settled beside a blocking reason: {settled:?}"
    );

    let acted = driver
        .act(
            Interaction::Click,
            ActTarget::NodeId(button.id.clone()),
            None,
        )
        .await
        .expect("act decodes");
    assert!(acted.applied_frame_seq > 0);
    assert!(acted.settled_frame_seq >= acted.applied_frame_seq);

    let after = driver
        .find_by_role("button")
        .await
        .expect("the button is still there");
    assert_eq!(
        after.label, "Go 1",
        "the client's act did not reach the application"
    );
}

/// FR-044: a journey records real evidence per step, taken by round-tripping
/// the server — never a restatement of the request.
#[tokio::test]
async fn a_journey_records_evidence_that_moved_with_the_ui() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, driver) = driven_with_client(dir.path()).await;

    let report = driver
        .run_journey(vec![
            JourneyStep::wait_settle("settle before starting", 5_000),
            JourneyStep::act(
                "click the button",
                Interaction::Click,
                ActTarget::NodeId("/root/go".into()),
                None,
            ),
            JourneyStep::query(
                "read the label back",
                TreeQuery::default().with_role("button"),
            ),
        ])
        .await;

    assert_eq!(report.steps.len(), 3, "{report:#?}");
    for step in &report.steps {
        assert!(
            step.failure_report().is_none(),
            "{:?}",
            step.failure_report()
        );
        assert!(
            step.before.frame_seq > 0 && step.after.frame_seq > 0,
            "a step recorded no frame evidence: {step:#?}"
        );
        assert_eq!(step.before.digest.len(), 64, "{step:#?}");
    }

    // The click step's evidence has to show the UI actually moved: the frame
    // after it is later than the frame before it. Evidence that merely echoed
    // the request would show the same number twice.
    let click = &report.steps[1];
    assert!(
        click.after.frame_seq > click.before.frame_seq,
        "the click step's after-evidence is not later than its before-evidence: {click:#?}"
    );

    let last = report.steps[2]
        .outcome
        .as_ref()
        .expect("the query step succeeded");
    match last {
        gorgon_petra_testkit::driver::StepOutcome::Queried(TreeAnswer::Matches(nodes)) => {
            assert_eq!(nodes.len(), 1, "{nodes:#?}");
            assert_eq!(nodes[0].label, "Go 1");
        }
        other => panic!("expected a filtered query's matches, got {other:?}"),
    }
}
