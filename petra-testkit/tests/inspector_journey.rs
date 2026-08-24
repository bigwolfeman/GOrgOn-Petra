//! T044: the US3 acceptance journey — connect, locate a named fiber, read
//! its effects, preview its unload, split the layout, follow the trace
//! tail — keyboard only, against a real `gorgond` booted from
//! `examples/demo/gorgon.yaml`, driven through the real, importable
//! [`gorgon_petra_testkit::driver::Client`] (SC-001, SC-005, FR-049).
//!
//! `support::gorgond::Daemon::boot_demo` spawns the shipped `gorgond`
//! binary (never `gorgond::boot` in process — see that module's own doc for
//! why); `support::inspector::inspector_with_client` hosts the real
//! `gorgon_inspector::app::Shell::mounted` composition under this crate's
//! `DriverHost`. Every action below is `Interaction::Key` (Tab, Enter,
//! arrow keys) — never `Click`, `Drag`, `Hover` or `Scroll` — and locating
//! a row or a tab is a real Tab-press loop reading the tree's own
//! `state.focused` flag back (`support::journey::tab_until`), not a driver
//! `focus` shortcut: this file drives the same input an operator without a
//! mouse would.
//!
//! # A driver-protocol limitation this journey works around, and names
//!
//! [`gorgon_petra::semantic::project`] only carries a node's `label` when
//! the view author set `semantics.label` — which every *interactive* node
//! and every [`gorgon_petra::component::status`] readout does, but a plain
//! [`gorgon_petra::component::text`]/`heading` node does not
//! (`contracts/semantic-tree.md`: "label ... REQUIRED when interactive",
//! never promised otherwise). The `tree` verb therefore cannot read back
//! the inspector's plain informational prose — the unload preview's
//! "no dependents" sentence, the trace tail's per-event lines, the raw
//! event count. This journey's assertions are built entirely from what
//! genuinely reaches the wire: node ids (always present, so *which*
//! branch rendered is provable even when its literal text is not) and
//! semantic labels on interactive/status nodes (present by construction).
//! Where this narrows what a specific task step can prove, the step's own
//! comment says so — most concretely in Task 6, where the demo
//! composition's own determinism (every timer it arms is a one-shot) means
//! this journey also cannot prove indefinite trace growth without
//! contriving activity the task list never asked for.

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
use support::journey::{percent, press_enter, press_key, tab_until, wait_tree};

/// Generous headroom over the focusable nodes this window ever mounts
/// against the shipped demo (11 fiber rows, 2 dividers, 4 tabs, at most one
/// tab-body button) — several full laps of the focus ring, so a real
/// regression (a node that stopped being focusable) fails loudly rather
/// than the suite hanging on an unbounded search.
const MAX_TAB_PRESSES: usize = 80;
/// Budget for a wait on something that depends on a real round trip to the
/// daemon: a bridge poll interval away, never instant (hazard: nothing may
/// hang, so every wait below is bounded by this, not by chance).
const DATA_WAIT: Duration = Duration::from_secs(15);
/// The row separator `gorgon_inspector::panels::fibers::SEP` joins a row's
/// fields with. Not importable — that module is private — so pinned here
/// as a comment-cited literal, the same value that module's own `const SEP`
/// holds. Used to build an exact-segment match: `"ticker"` alone would also
/// match the demo's `"agent-tools/ticker"` row.
const ROW_SEP: &str = "  ·  ";

fn fiber_needle(row: &str) -> String {
    format!("{ROW_SEP}{row}{ROW_SEP}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_us3_task_list_completes_keyboard_only_against_a_live_daemon() {
    let daemon = Daemon::boot_demo().await;
    let ui_dir = tempfile::tempdir().expect("tempdir for the driver socket");
    let (_driven, driver) = inspector_with_client(ui_dir.path(), daemon.endpoint()).await;

    // ---- Task 1: connect ------------------------------------------------
    // Scenario 1: "shows the daemon's identity ... and a live fiber list
    // ... within one refresh interval." `ConnState` only reaches `Live`
    // once the identity round trip has succeeded (`data/conn.rs`), so the
    // chrome's own connection status readout naming this daemon's endpoint
    // is exactly that proof — read from `status("conn", ...)`, which is
    // labeled (unlike the identity panel's own plain-text instance/lineage
    // pairs; see this file's module doc). The literal instance/lineage
    // text is a real gap this journey does not close — named at the end.
    let endpoint = daemon.endpoint();
    wait_tree(&driver, "connected to the daemon", DATA_WAIT, |root| {
        root.iter()
            .any(|n| n.label.starts_with("connected to ") && n.label.contains(&endpoint))
            .then_some(())
    })
    .await;
    wait_tree(
        &driver,
        "showed a live fiber row for `ticker`",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| {
                    n.role == Some(Role::ListItem) && n.label.contains(&fiber_needle("ticker"))
                })
                .then_some(())
        },
    )
    .await;

    // ---- Task 2: locate a named fiber ------------------------------------
    // Real Tab presses, not a `focus` shortcut: repeatedly reads the tree's
    // `state.focused` node back and presses Tab again when it is not yet
    // the row named `ticker` — exactly what an operator without a mouse
    // does.
    let needle = fiber_needle("ticker");
    tab_until(
        &driver,
        "reached the `ticker` fiber row",
        MAX_TAB_PRESSES,
        |node| node.role == Some(Role::ListItem) && node.label.contains(&needle),
    )
    .await;
    press_enter(&driver).await;

    // ---- Task 3: read its effects ----------------------------------------
    // Scenario 2: owned effects render from a live query, no sample data.
    // `EffectRow::view` (fiber_detail.rs) renders each slot through
    // `component::status`, which *is* labeled, and `timer.set` is
    // `ticker`'s real effect label — confirmed against the running daemon
    // (`info` on `fiber:8`) before this file was written, not guessed.
    wait_tree(
        &driver,
        "rendered ticker's real effect tree",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| n.label.contains("timer.set"))
                .then_some(())
        },
    )
    .await;

    // ---- Task 4: preview its unload --------------------------------------
    // Scenario 3: the daemon's own answer is shown, not a client guess.
    // The plan's headline/dependents sentences are plain text (invisible to
    // `tree`, see the module doc), but every inverse step's `class`/`landed`
    // marks are `component::status` — labeled — and their content only
    // exists once the daemon's real preview answer has been parsed and
    // rendered. Eight `StrongInverse`, all landed, zero dependents: the
    // exact shape probed from the running daemon (`unload-preview` on
    // `fiber:8`) before this file was written.
    tab_until(
        &driver,
        "reached the Unload preview tab",
        MAX_TAB_PRESSES,
        |node| node.role == Some(Role::Tab) && node.label.contains("Unload preview"),
    )
    .await;
    press_enter(&driver).await;
    let request = tab_until(
        &driver,
        "reached the unload-preview request button",
        MAX_TAB_PRESSES,
        |node| node.role == Some(Role::Button) && node.label.contains("preview the unload"),
    )
    .await;
    assert!(
        request.label.contains("fiber 8"),
        "the request button did not name fiber 8 (`ticker`): {:?}",
        request.label
    );
    press_enter(&driver).await;
    wait_tree(
        &driver,
        "rendered the daemon's real unload-preview answer for `ticker`",
        DATA_WAIT,
        |root| {
            let answered = root.iter().any(|n| n.id.ends_with("/answer-age"));
            let no_dependents = root.iter().any(|n| n.id.ends_with("/dependents-none"));
            let strong_inverses = root
                .iter()
                .filter(|n| n.label.starts_with("strong inverse — restores exactly"))
                .count();
            let all_landed = root
                .iter()
                .filter(|n| n.label.starts_with("landed — the disposer is attached"))
                .count();
            (answered && no_dependents && strong_inverses == 8 && all_landed == 8).then_some(())
        },
    )
    .await;

    // ---- Task 5: split the layout ------------------------------------------
    // FR-046: the divider is keyboard-operable, not only pointer-draggable.
    // Its percentage is `Semantics::value`, which is on the wire whether or
    // not the node carries a label.
    let handle = tab_until(
        &driver,
        "reached the body divider",
        MAX_TAB_PRESSES,
        |node| node.role == Some(Role::Separator) && node.id.ends_with("body-handle"),
    )
    .await;
    let before = percent(&handle.value)
        .unwrap_or_else(|| panic!("the divider handle carried no percent value: {handle:?}"));
    for _ in 0..5 {
        press_key(&driver, "right").await;
    }
    let handle_id = handle.id.clone();
    let after = wait_tree(
        &driver,
        "the body divider moved after 5 Right presses",
        DATA_WAIT,
        |root| {
            root.iter()
                .find(|n| n.id == handle_id)
                .and_then(|n| percent(&n.value))
                .filter(|value| *value > before)
        },
    )
    .await;
    assert!(
        after > before,
        "5 Right presses on the body divider did not move it: {before}% -> {after}%"
    );

    // ---- Task 6: follow the trace tail ---------------------------------------
    // Every timer the demo composition arms is a one-shot
    // (`examples/demo/plugins/ticker.lua`'s own comment: "arms a one-shot
    // that fires timer/fired", confirmed against the running daemon before
    // this file was written: both `ticker` fibers fire once near boot and
    // then produce nothing further). By the time this step runs the
    // daemon's trace stream is genuinely quiet — FR-033's "an idle
    // application paints zero frames" holding all the way down to the
    // daemon this build is driving, not a reason to fake activity. What
    // this step proves instead, honestly, within what a deterministic demo
    // composition can show: the follow subscription is really open (its
    // absence is exactly the state FR-047 requires this panel to name), and
    // the tail already holds real records from this daemon's own boot
    // (mount/transition trace kinds) rather than an empty "no events" tail.
    tab_until(&driver, "reached the Trace tab", MAX_TAB_PRESSES, |node| {
        node.role == Some(Role::Tab) && node.label.contains("Trace")
    })
    .await;
    press_enter(&driver).await;
    wait_tree(
        &driver,
        "opened the trace follow subscription",
        DATA_WAIT,
        |root| {
            (!root
                .iter()
                .any(|n| n.label.contains("no `trace --follow` subscription is open")))
            .then_some(())
        },
    )
    .await;
    wait_tree(
        &driver,
        "rendered real trace records from the daemon's own boot, not an empty tail",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| n.id.contains("/trace/event-"))
                .then_some(())
        },
    )
    .await;
}
