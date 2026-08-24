//! Host the real inspector `Shell` under `DriverHost`, pointed at a real
//! `gorgond` endpoint, and hand back the importable driver [`Driver`]
//! client. Mirrors `support::driven_server`/`driven_with_client` — the only
//! changes are which `App` steps (`gorgon_inspector::app::Shell::mounted`,
//! never a hand-built subset of panels — an earlier bug composed the shell
//! with an empty panel vector and the window mounted nothing while every
//! test still passed) and that its data bridge dials a real daemon instead
//! of nothing.
//!
//! The window runs at [`WINDOW`] rather than `support::sized`'s 400x300:
//! `main.rs`'s own size, because a smaller viewport changes what this
//! seven-panel window negotiates and a journey run at a toy size is not
//! proof about the size an operator actually sees it at.

// See `support::gorgond`'s identical attribute for why: this file is
// recompiled into every test binary that pulls it in, and T044/T045 do not
// both call every item here.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use egui::{Context, Pos2, RawInput};

use gorgon_inspector::app::Shell;
use gorgon_inspector::bridge::{self, BridgeOptions};
use gorgon_inspector::data::SessionOptions;
use gorgon_petra::token::{Presenter, dark};
use gorgon_petra_egui::host::Host;
use gorgon_petra_testkit::driver::Client as Driver;
use gorgon_petra_testkit::driver_host::DriverHost;
use gorgon_petra_testkit::server::Server;
use gorgon_petra_testkit::snapshot::Snapshotter;

/// The window size `gorgon-inspector`'s own `main.rs` asks for.
const WINDOW: [f32; 2] = [1400.0, 900.0];

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

/// A driver-controlled inspector window running its own loop on its own
/// thread — the same shape `support::DrivenApp` uses (see that type's own
/// doc comment for why 2 ms polling rather than a wake is the right call for
/// a test harness). Killed by `Drop`, which is hazard #2 (nothing may hang)
/// for this thread: the stop flag is set and the thread is joined, so a
/// panicking test still tears the loop down.
pub struct DrivenInspector {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for DrivenInspector {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Stand up the real inspector `Shell` — `Shell::mounted`, the shipped
/// seven-panel composition — with its data bridge dialing `endpoint`, host
/// it under a real driver server bound under `dir`, and hand back a
/// connected importable client.
///
/// `dir` is `Server::bind_under`'s own reason for existing: `$XDG_RUNTIME_DIR`
/// is process-wide state and `cargo test`'s default parallelism runs many
/// tests in one process, so a caller passes its own temp dir instead of
/// racing every other test that also touches the environment variable.
pub async fn inspector_with_client(dir: &Path, endpoint: String) -> (DrivenInspector, Driver) {
    let (server, hub, ui_bridge) = Server::new("gorgon-inspector-journey-test");
    let listener = Server::bind_under(dir)
        .await
        .expect("bind the driver socket under a fresh temp dir");
    let socket = Server::socket_path_under(dir);
    tokio::spawn(server.serve(listener));

    // Short cadences: this harness wants a real daemon's disconnect/
    // reconnect to show up in a test-sized budget, not `main`'s own
    // production defaults.
    let data_bridge = bridge::spawn(
        &tokio::runtime::Handle::current(),
        endpoint,
        SessionOptions {
            identity_timeout: Duration::from_millis(800),
            era_probe_timeout: Duration::from_millis(300),
            ..SessionOptions::default()
        },
        BridgeOptions {
            tick: Duration::from_millis(20),
            poll: Duration::from_millis(150),
            ..BridgeOptions::default()
        },
        None,
    );

    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread = std::thread::spawn(move || {
        let ctx = headless();
        let shell = Shell::mounted(data_bridge);
        let host = Host::new(&ctx, shell, Presenter::new(dark()));
        let mut driver = DriverHost::new(&ctx, host, hub, ui_bridge, Snapshotter::new());
        while !thread_stop.load(Ordering::SeqCst) {
            driver
                .step(&ctx, sized(RawInput::default()))
                .drop_without_applying_deltas();
            std::thread::sleep(Duration::from_millis(2));
        }
    });

    let driver = Driver::connect(&socket)
        .await
        .expect("connect the importable driver client to the inspector's own socket");
    for attempt in 1..=200 {
        let health = driver.health().await.expect("health decodes");
        if health.frame_seq > 0 {
            break;
        }
        assert!(attempt < 200, "the inspector never published a frame");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    (
        DrivenInspector {
            stop,
            thread: Some(thread),
        },
        driver,
    )
}
