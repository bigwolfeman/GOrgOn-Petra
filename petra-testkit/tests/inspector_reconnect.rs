//! T045: daemon-restart honesty (spec US3 scenario 4, quoted verbatim from
//! `specs/003-petra-layout-engine/spec.md`):
//!
//! > **Given** the daemon stops, **When** the inspector's next query fails,
//! > **Then** every affected panel shows a disconnected state naming the
//! > endpoint — never an empty list styled as healthy — and reconnection
//! > restores live data without restarting the inspector.
//!
//! Boots a real `gorgond` (`support::gorgond::Daemon::boot_demo`), connects
//! the real inspector `Shell` to it (`support::inspector::inspector_with_client`
//! — the same harness `tests/inspector_journey.rs` uses, FS-2), kills the
//! daemon mid-session, asserts every one of the five projection-backed
//! panels (identity, fibers, fiber-detail, approvals, leaks) shows its own
//! named `STALE` caveat rather than silently going quiet, asserts the
//! fiber list keeps its last-known rows on screen *marked* stale rather
//! than emptying, restarts the daemon on the same socket path, and asserts
//! live rendering resumes — all without restarting the inspector process.
//!
//! # G7 sabotage (run once against a working tree, not a standing test)
//!
//! `gorgon/inspector/src/panels/fibers.rs`'s `fiber_row` sets
//! `node.semantics.stale = stale` on every row; deleting that one line
//! makes a disconnected fiber list keep drawing its last-known rows
//! *unmarked* — exactly "an empty list styled as healthy" scenario 4
//! forbids, except non-empty. Running this file against that mutation
//! turns the "kept the `ticker` row on screen, marked stale" assertion red,
//! naming the fibers panel; `cp` the backup back and `diff` it
//! byte-identical restores the working tree. The result of that exercise
//! is reported at the end of the wave, the same way `tests/journey.rs`'s
//! own T035 sabotage note handles it — not committed here as a standing
//! test, which would just be a second, worse copy of the bug it exists to
//! catch.

#![cfg(feature = "testkit")]
// `mod support;` below pulls in the whole shared harness — including
// `support::mod`'s own `driven_server`/`CountingApp` shapes `tests/server.rs`
// and `tests/journey.rs` use, not this file — so `dead_code` would flag
// exactly the part of the shared module this binary legitimately does not
// call, not a real problem in this file.
#![allow(dead_code)]

mod support;

use std::time::Duration;

use gorgon_petra::tree::Role;

use support::gorgond::Daemon;
use support::inspector::inspector_with_client;
use support::journey::{press_enter, tab_until, wait_tree};

const MAX_TAB_PRESSES: usize = 80;
/// Budget for a wait that depends on a real round trip to the daemon, or on
/// this harness's own reconnect-engine cadence — never instant (hazard:
/// nothing may hang).
const DATA_WAIT: Duration = Duration::from_secs(15);
const ROW_SEP: &str = "  ·  ";

fn fiber_needle(row: &str) -> String {
    format!("{ROW_SEP}{row}{ROW_SEP}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_projection_backed_panel_names_its_disconnect_and_recovers_on_restart() {
    let mut daemon = Daemon::boot_demo().await;
    let ui_dir = tempfile::tempdir().expect("tempdir for the driver socket");
    let (_driven, driver) = inspector_with_client(
        ui_dir.path(),
        "gorgon-inspector-reconnect-test",
        daemon.endpoint(),
    )
    .await;
    let endpoint = daemon.endpoint();

    // Connect, and select `ticker` so `info` (fiber-detail's own
    // projection) is live before the kill too, not only `fibers`/
    // `identity` — the more of the five polled projections carry real data
    // going in, the more of them this test can show going stale rather
    // than merely "never loaded".
    wait_tree(&driver, "connected to the daemon", DATA_WAIT, |root| {
        root.iter()
            .any(|n| n.label.starts_with("connected to ") && n.label.contains(&endpoint))
            .then_some(())
    })
    .await;
    let needle = fiber_needle("ticker");
    tab_until(
        &driver,
        "reached the `ticker` fiber row",
        MAX_TAB_PRESSES,
        |node| node.role == Some(Role::ListItem) && node.label.contains(&needle),
    )
    .await;
    press_enter(&driver).await;
    wait_tree(
        &driver,
        "loaded ticker's live effect tree before the kill",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| n.label.contains("timer.set"))
                .then_some(())
        },
    )
    .await;

    // ---- kill mid-session -------------------------------------------------
    assert!(
        daemon.is_running(),
        "the daemon must be running before it is killed"
    );
    daemon.kill();
    assert!(
        !daemon.is_running(),
        "Daemon::kill must leave the daemon not running"
    );

    // The connection line itself: named, and honestly not "connected".
    wait_tree(
        &driver,
        "showed a disconnected state naming the endpoint",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| {
                    !n.label.starts_with("connected to ")
                        && n.label.contains(&endpoint)
                        && (n.label.contains("disconnected") || n.label.contains("connecting"))
                })
                .then_some(())
        },
    )
    .await;

    // Every projection-backed panel already on screen without switching
    // tabs — identity and fibers are `Slot::Leading`, fiber-detail is
    // `Slot::Trailing`, approvals is the tab selected by default — shows
    // its own STALE caveat, naming the endpoint. Never silently reverting
    // to an unmarked "no data".
    for (panel, id_suffix) in [
        ("identity", "identity/freshness"),
        ("fibers", "fibers/freshness"),
        ("fiber-detail", "fiber-detail/freshness"),
        ("approvals", "approvals/caveat"),
    ] {
        let what = format!("the {panel} panel named its own disconnected state");
        wait_tree(&driver, &what, DATA_WAIT, |root| {
            root.iter()
                .any(|n| {
                    n.id.ends_with(id_suffix)
                        && n.label.contains("STALE")
                        && n.label.contains(&endpoint)
                })
                .then_some(())
        })
        .await;
    }

    // The fiber list itself: this is the scenario's own "never an empty
    // list styled as healthy" — the last-known `ticker` row is still drawn
    // (not emptied), but its own `stale` flag is set (not silently
    // current). This is the assertion G7's sabotage targets.
    wait_tree(
        &driver,
        "kept the `ticker` row on screen, marked stale, rather than emptying or drawing it \
         unmarked",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| {
                    n.role == Some(Role::ListItem) && n.label.contains(&needle) && n.state.stale
                })
                .then_some(())
        },
    )
    .await;

    // Leaks is a tab, not on screen by default: switch to it keyboard-only
    // and prove it too — "every" projection-backed panel, not only the
    // ones already visible when the daemon died.
    tab_until(&driver, "reached the Leaks tab", MAX_TAB_PRESSES, |node| {
        node.role == Some(Role::Tab) && node.label.contains("Leaks")
    })
    .await;
    press_enter(&driver).await;
    wait_tree(
        &driver,
        "the Leaks panel named its own disconnected state",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| {
                    n.id.ends_with("leaks/freshness")
                        && n.label.contains("STALE")
                        && n.label.contains(&endpoint)
                })
                .then_some(())
        },
    )
    .await;

    // ---- restart, same socket path -----------------------------------------
    daemon.restart().await;
    assert_eq!(
        daemon.endpoint(),
        endpoint,
        "restart must reuse the same socket path — that is the whole point of scenario 4's \
         'without restarting the inspector'"
    );

    wait_tree(
        &driver,
        "reconnected to the restarted daemon",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| n.label.starts_with("connected to ") && n.label.contains(&endpoint))
                .then_some(())
        },
    )
    .await;
    for (panel, id_suffix) in [
        ("identity", "identity/freshness"),
        ("fibers", "fibers/freshness"),
        ("fiber-detail", "fiber-detail/freshness"),
    ] {
        let what = format!("the {panel} panel's caveat cleared after reconnecting");
        wait_tree(&driver, &what, DATA_WAIT, |root| {
            (!root.iter().any(|n| n.id.ends_with(id_suffix))).then_some(())
        })
        .await;
    }
    wait_tree(
        &driver,
        "the `ticker` row lost its stale mark after reconnecting — live recovery, without \
         restarting the inspector",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| {
                    n.role == Some(Role::ListItem) && n.label.contains(&needle) && !n.state.stale
                })
                .then_some(())
        },
    )
    .await;
}
