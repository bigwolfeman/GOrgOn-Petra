//! Keyboard-only driver primitives shared by T044's
//! `tests/inspector_journey.rs` and T045's `tests/inspector_reconnect.rs`
//! (FS-2: one journey harness, owned by T044; never a second copy of the
//! same query logic).
//!
//! Every function here goes through [`Driver`] — [`gorgon_petra_testkit::driver::Client`]
//! — and only `Interaction::Key`: no `Click`, `Drag`, `Hover` or `Scroll`
//! appears anywhere below, which is what makes [`tab_until`] a real
//! keyboard-traversal loop rather than a driver shortcut standing in for
//! one.

// See `support::gorgond`'s identical attribute for why: this file is
// recompiled into every test binary that pulls it in, and T044/T045 do not
// both call every item here (`percent` is T044-only, for one).
#![allow(dead_code)]

use std::time::{Duration, Instant};

use serde_json::json;

use gorgon_petra::tree::Interaction;
use gorgon_petra_testkit::driver::{
    ActTarget, Client as Driver, DriverNode, TreeAnswer, TreeQuery,
};

/// An unfiltered `tree` query, unwrapped to its one root node.
pub async fn tree_root(driver: &Driver) -> DriverNode {
    match driver
        .tree(&TreeQuery::default())
        .await
        .expect("`tree` query decodes")
    {
        TreeAnswer::Node(root) => root,
        TreeAnswer::Matches(nodes) => panic!(
            "an unfiltered `tree` query must answer one node, not an array of {}",
            nodes.len()
        ),
    }
}

/// Poll a fresh tree every 50 ms until `probe` returns `Some`, or panic
/// naming `what` and every labeled node the last tree carried.
pub async fn wait_tree<T>(
    driver: &Driver,
    what: &str,
    budget: Duration,
    mut probe: impl FnMut(&DriverNode) -> Option<T>,
) -> T {
    let deadline = Instant::now() + budget;
    loop {
        let root = tree_root(driver).await;
        if let Some(value) = probe(&root) {
            return value;
        }
        if Instant::now() >= deadline {
            let labels: Vec<String> = root
                .iter()
                .filter(|n| !n.label.is_empty())
                .map(|n| format!("[{}] {:?}", n.id, n.label))
                .collect();
            panic!(
                "never {what} within {budget:?}; last labeled nodes:\n{}",
                labels.join("\n")
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Press `Interaction::Key` "tab" until the currently-focused node
/// satisfies `pred`, checking before every press (so a node already
/// focused when this is called can match on the first check, with zero
/// presses). Panics naming the last focused node after `max_presses`.
pub async fn tab_until(
    driver: &Driver,
    what: &str,
    max_presses: usize,
    mut pred: impl FnMut(&DriverNode) -> bool,
) -> DriverNode {
    for _ in 0..=max_presses {
        let root = tree_root(driver).await;
        if let Some(focused) = root.iter().find(|n| n.state.focused)
            && pred(focused)
        {
            return focused.clone();
        }
        press_key(driver, "tab").await;
    }
    let root = tree_root(driver).await;
    let last = root
        .iter()
        .find(|n| n.state.focused)
        .map(|n| format!("[{}] {:?} role={:?}", n.id, n.label, n.role));
    panic!("never {what} within {max_presses} Tab presses; last focused: {last:?}");
}

/// Synthesize one key press+release, targeted at whatever currently holds
/// focus — `key` actions route by focus, not by target
/// (`gorgon_petra_egui::inject`'s own module doc), so `ActTarget::Pos`
/// carries nothing but a syntactically valid target for the wire.
pub async fn press_key(driver: &Driver, name: &str) {
    driver
        .act(
            Interaction::Key,
            ActTarget::Pos { x: 0.0, y: 0.0 },
            Some(json!({ "key": name })),
        )
        .await
        .unwrap_or_else(|err| panic!("key {name:?} was refused: {err}"));
}

pub async fn press_enter(driver: &Driver) {
    press_key(driver, "enter").await;
}

/// A divider handle's `Semantics::value` ("42%") as a bare integer.
#[must_use]
pub fn percent(value: &Option<String>) -> Option<u32> {
    value.as_deref()?.trim_end_matches('%').parse().ok()
}
