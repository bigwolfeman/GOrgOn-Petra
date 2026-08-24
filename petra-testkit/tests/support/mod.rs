//! Shared driven-application test harness (T029/T033's `tests/server.rs`,
//! lifted for T035's `tests/journey.rs` to use too — never a second copy).
//!
//! `#[cfg(feature = "testkit")]`-only, same as everything it is built from:
//! this module names `gorgon_petra_egui::host` and
//! `gorgon_petra_testkit::{driver_host, server, snapshot}`, all of which are
//! declared behind that feature. It relies on its two callers
//! (`tests/server.rs`, `tests/journey.rs`) to have already gated themselves
//! with `#![cfg(feature = "testkit")]` at the top of the file, the same way
//! `tests/server.rs` already did before this module existed.
//!
//! Included via `mod support;` from each test binary that needs it — Rust
//! integration tests are separate crates, so this file is recompiled once
//! per binary that pulls it in. That is a build-graph fact, not a second
//! *source*: there is exactly one place this code is written.
//!
//! [`gorgond`], [`inspector`] and [`journey`] are T044's addition: a real,
//! subprocess-booted `gorgond`, a real inspector `Shell` hosted the same
//! `DriverHost` way [`driven_server`] hosts [`CountingApp`] above, and the
//! keyboard-only driver primitives both `tests/inspector_journey.rs` and
//! T045's `tests/inspector_reconnect.rs` drive them with (FS-2: one journey
//! harness, owned by T044).

pub mod gorgond;
pub mod inspector;
pub mod journey;

use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use egui::{Context, Pos2, RawInput};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::token::{Presenter, dark};
use gorgon_petra::tree::{NodeKind, ViewNode};
use gorgon_petra_egui::host::{App, Host};
use gorgon_petra_testkit::driver::Client as Driver;
use gorgon_petra_testkit::driver_host::DriverHost;
use gorgon_petra_testkit::server::Server;
use gorgon_petra_testkit::snapshot::Snapshotter;

/// The window every headless pass in this harness runs at.
const WINDOW: [f32; 2] = [400.0, 300.0];

/// Give `input` this harness's fixed viewport, so every headless pass in
/// this file lays out against the same size.
pub fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

/// A context that has run exactly one empty pass — enough for `egui` to be
/// ready for a real one, never applied.
pub fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx
}

/// The driven application counts clicks and puts the count in the button's
/// own label.
///
/// That is the whole point: a driver test that asserts only on the numbers
/// `act` returns proves the verb answered, not that the click landed. The
/// count travels input -> `App::handle` -> `view` -> a new frame -> the
/// semantic tree, so a `tree` query after the `act` is reading the
/// application's real state through the real projection. T035's sabotage
/// (bypass the real-input path for one action kind) targets the `matches!`
/// condition in [`CountingApp::handle`] below — the one place a click is
/// allowed to count.
#[derive(Default)]
pub struct CountingApp {
    clicks: usize,
}

impl RowSource for CountingApp {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for CountingApp {
    fn view(&mut self) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(gorgon_petra::component::button(
            "go",
            format!("Go {}", self.clicks),
        ))
    }

    fn handle(&mut self, event: &InputEvent, route: &Route) {
        // The real-input path. A click only counts when the router itself
        // — not the request that asked for one — delivered a press to
        // `/root/go`. This condition is the sabotage target: bypassing it
        // (e.g. incrementing unconditionally on any `PointerPressed`) would
        // make the routing assertion in `journey.rs` pass for the wrong
        // reason, so proving the test catches its removal is what makes the
        // test load-bearing rather than decorative.
        if matches!(event, InputEvent::PointerPressed { .. })
            && matches!(route, Route::Pointer { node } if node == "/root/go")
        {
            self.clicks += 1;
        }
    }

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::All
    }
}

/// A driver-controlled application running its own loop on its own thread.
///
/// The loop polls at 2 ms rather than sleeping until woken. That is a test
/// harness, not the shipped shape: a real application blocks in its windowing
/// event loop and `UiBridge::submit`'s `request_repaint` wakes it. Polling
/// here keeps the test free of a winit dependency, and costs only that the
/// zero-idle property (SC-002) is not what these tests measure.
pub struct DrivenApp {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for DrivenApp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A request/response pair over one persistent connection, so a test can
/// issue several calls without reconnecting.
///
/// This is the raw NDJSON protocol client, hand-rolled exactly once, in the
/// test harness that stands in for a caller who has not yet imported the
/// real driver client — which is the whole reason a driver client ships at
/// all (FR-041). [`Driver`] (the real, importable one) is what
/// `driven_with_client` hands back to a caller that wants it; this `Client`
/// is only for the harness's own startup handshake and for the raw-protocol
/// tests in `tests/server.rs` that exist specifically to prove the wire
/// shape independent of the typed client.
pub struct Client {
    reader: BufReader<OwnedReadHalf>,
    writer: OwnedWriteHalf,
}

impl Client {
    pub fn new(stream: UnixStream) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            reader: BufReader::new(reader),
            writer,
        }
    }

    pub async fn call(&mut self, id: i64, verb: &str, params: Value) -> Value {
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
    ///
    /// This module is recompiled per test binary that pulls it in
    /// (`mod support;`), and only `tests/server.rs`'s malformed-line test
    /// calls this one — `tests/journey.rs` never sends anything but
    /// well-formed requests. `#[allow(dead_code)]` rather than dropping the
    /// method: it is real, used, load-bearing API for the binary that does
    /// call it, not leftover code.
    #[allow(dead_code)]
    pub async fn send_raw(&mut self, line: &str) -> Value {
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

/// A server, a stepping application, and a connected raw client.
pub async fn driven_server(dir: &Path) -> (DrivenApp, Client) {
    let (server, hub, bridge) = Server::new("driven-test-app");
    let listener = Server::bind_under(dir)
        .await
        .expect("bind under a fresh temp dir");
    let socket = Server::socket_path_under(dir);
    tokio::spawn(server.serve(listener));

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        let ctx = headless();
        let host = Host::new(&ctx, CountingApp::default(), Presenter::new(dark()));
        let mut driver = DriverHost::new(&ctx, host, hub, bridge, Snapshotter::new());
        while !thread_stop.load(Ordering::SeqCst) {
            driver
                .step(&ctx, sized(RawInput::default()))
                .drop_without_applying_deltas();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });

    let stream = UnixStream::connect(&socket)
        .await
        .unwrap_or_else(|err| panic!("connect to {}: {err}", socket.display()));
    // Wait for the first frame, so a test never races the application's
    // startup and reads a "no frame yet" it did not mean to test.
    let mut client = Client::new(stream);
    for attempt in 1..=200 {
        let health = client.call(attempt, "health", json!({})).await;
        if health["result"]["frame_seq"].as_u64().unwrap_or(0) > 0 {
            break;
        }
        assert!(attempt < 200, "the application never published a frame");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    (
        DrivenApp {
            stop,
            thread: Some(thread),
        },
        client,
    )
}

/// A driven server plus a connected importable client.
pub async fn driven_with_client(dir: &Path) -> (DrivenApp, Driver) {
    // `driven_server` already waits for the first frame, so the client below
    // never races startup. Its raw `Client` is dropped (it goes out of
    // scope unused below); the socket stays.
    let (app, _raw) = driven_server(dir).await;
    // The two halves of the socket-path formula, checked against each other
    // rather than trusted. The client cannot import `Server::socket_path_under`
    // — it is `testkit`-gated and the client must build with no feature on —
    // so it recomputes the formula, and a drift would show up as "nothing ever
    // connects" with no error naming the cause.
    assert_eq!(
        gorgon_petra_testkit::driver::this_process_socket_path(dir),
        Server::socket_path_under(dir),
        "the client's socket-path formula drifted from the server's"
    );
    let driver = Driver::connect(&Server::socket_path_under(dir))
        .await
        .expect("the importable client connects to the real server");
    (app, driver)
}
