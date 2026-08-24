//! A sample driven application — one source, two shapes (T034/T035/T036).
//!
//! # Without `--features testkit`
//!
//! Builds one `gorgon-petra` view tree and prints it. No `egui`, no
//! `gorgon-petra-egui`, no socket, no server symbol: this crate's `testkit`
//! feature is the only thing that pulls those in (see `Cargo.toml`'s
//! `[features]` block), and this binary's `main` below is `cfg`-split so the
//! no-feature shape never even names them. `tests/exclusion.rs` (T036)
//! builds exactly this shape and checks the artifact with `nm`/`readelf` for
//! proof, not by trusting this doc comment.
//!
//! Run it: `cargo run -p gorgon-petra-testkit --example driven`
//!
//! # With `--features testkit`
//!
//! The same tree, hosted for real: a driver socket at
//! `$XDG_RUNTIME_DIR/gorgon/ui-<pid>.sock` (`Server::bind`), a
//! [`gorgon_petra_testkit::driver_host::DriverHost`] stepping a headless
//! [`gorgon_petra_egui::host::Host`] on its own thread, and a click counter
//! wired through the real router — the same shape
//! `gorgon/petra-testkit/tests/support/mod.rs`'s `CountingApp` uses, so a
//! `tree` query after an `act click` proves the click reached `App::handle`
//! rather than merely that the verb answered (see that module's doc comment
//! for why that distinction is the whole point).
//!
//! Prints exactly one machine-readable line before doing anything else:
//! `PETRA_DRIVEN_SOCKET=<path>` — what `xtask`'s `petra-journey` and
//! `petra-snapshots` lanes (T034) read to find this process's socket. Runs
//! for `PETRA_DRIVEN_SECONDS` (default 60) and exits; nothing here waits
//! unboundedly.
//!
//! Run it: `cargo run -p gorgon-petra-testkit --example driven --features testkit`

use gorgon_petra::component::button;
use gorgon_petra::tree::{NodeKind, ViewNode};

/// The application both shapes describe: one button, whose label carries a
/// click count once the `testkit` shape is driving it for real.
fn view(clicks: usize) -> ViewNode {
    ViewNode::new(NodeKind::Stack, "root").child(button("go", format!("Go {clicks}")))
}

#[cfg(not(feature = "testkit"))]
fn main() {
    let root = view(0);
    println!(
        "driven: plain build (no testkit feature); root kind={:?} key={:?} children={}",
        root.kind,
        root.key,
        root.children.len()
    );
}

#[cfg(feature = "testkit")]
mod driven {
    use std::ops::Range;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use egui::{Context, Pos2, RawInput};

    use gorgon_petra::input::{InputEvent, Route};
    use gorgon_petra::layout::{ChangeSet, RowSource};
    use gorgon_petra::token::{Presenter, dark};
    use gorgon_petra::tree::ViewNode;
    use gorgon_petra_egui::host::{App, Host};
    use gorgon_petra_testkit::driver_host::DriverHost;
    use gorgon_petra_testkit::server::Server;
    use gorgon_petra_testkit::snapshot::Snapshotter;

    /// The driven application: counts clicks landed by the real router, the
    /// same contract `tests/support::CountingApp` proves against the socket
    /// tests (`tests/server.rs`'s `act_clicks_a_real_button_and_reports_both_frames`).
    #[derive(Default)]
    pub struct DrivenApp {
        clicks: usize,
    }

    impl RowSource for DrivenApp {
        fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
            Vec::new()
        }
    }

    impl App for DrivenApp {
        fn view(&mut self) -> ViewNode {
            super::view(self.clicks)
        }

        fn handle(&mut self, event: &InputEvent, route: &Route) {
            // The real-input path (T035's sabotage target): a click only
            // counts when the router itself delivered a press to this
            // node — never a shortcut that increments on the request
            // alone. Bypassing this condition is exactly the sabotage the
            // wave contract asks T035 to prove red.
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

    /// `PETRA_DRIVEN_SECONDS`, or the contract's own default of 60 — a run
    /// with a bad value fails loudly rather than silently picking a default
    /// that hides a caller's typo (FR-043's discipline applies here too: a
    /// gate that misreads its own environment is worse than one that
    /// refuses).
    fn run_seconds() -> u64 {
        match std::env::var("PETRA_DRIVEN_SECONDS") {
            Err(std::env::VarError::NotPresent) => 60,
            Ok(raw) => raw
                .parse()
                .unwrap_or_else(|err| panic!("PETRA_DRIVEN_SECONDS={raw:?} is not a u64: {err}")),
            Err(err) => panic!("PETRA_DRIVEN_SECONDS: {err}"),
        }
    }

    pub fn main() {
        let rt = tokio::runtime::Runtime::new().expect("build a tokio runtime");
        rt.block_on(async {
            let (server, hub, bridge) = Server::new("petra-driven-example");
            let listener = Server::bind().await.unwrap_or_else(|err| {
                eprintln!("driven: failed to bind the driver socket: {err}");
                std::process::exit(1);
            });
            let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .expect("XDG_RUNTIME_DIR set — Server::bind already required it to succeed");
            let socket = Server::socket_path_under(&runtime_dir);

            // The one machine-readable line the contract promises. Printed
            // before anything else runs, and before the socket could
            // possibly have a peer, so a reader never has to guess whether
            // the line arrived before the endpoint existed.
            println!("PETRA_DRIVEN_SOCKET={}", socket.display());

            tokio::spawn(server.serve(listener));

            let deadline = Instant::now() + Duration::from_secs(run_seconds());
            let handle = std::thread::spawn(move || {
                let ctx = headless();
                let host = Host::new(&ctx, DrivenApp::default(), Presenter::new(dark()));
                let mut driver = DriverHost::new(&ctx, host, hub, bridge, Snapshotter::new());
                while Instant::now() < deadline {
                    driver
                        .step(&ctx, sized(RawInput::default()))
                        .drop_without_applying_deltas();
                    std::thread::sleep(Duration::from_millis(2));
                }
            });

            // Bounded by `deadline` above (hazard #4: nothing may hang) —
            // the loop thread exits on its own once the clock runs out, and
            // this join only waits for that already-bounded exit.
            handle.join().expect("driver loop thread panicked");
        });
    }
}

#[cfg(feature = "testkit")]
fn main() {
    driven::main();
}
