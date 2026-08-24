//! Baseline-measurement helpers for T046/T064: SC-002's idle-frame half,
//! X-01's RSS half, and SC-003's interaction-latency seed — all against the
//! real US3 journey in `tests/inspector_journey.rs` (never a synthetic
//! stand-in journey).
//!
//! # Why `tab_until` is duplicated here rather than imported
//!
//! `support::journey::tab_until` is the obvious place a Tab-press timer
//! would live, but this leaf does not own `journey.rs` — a sibling leaf
//! (T058) is reading it concurrently this wave, and the ownership split
//! frozen in the wave plan gives `journey.rs` to nobody but its original
//! author. [`tab_until`] below reuses `journey::tree_root` (public, so
//! calling it is fine — only editing `journey.rs` is out of bounds) and
//! duplicates only the outer press/poll loop, adding the one thing
//! `journey::tab_until` has no hook for: timing each Tab press it issues
//! into a caller-supplied sample vec. A real duplication, named as one, not
//! a copy-for-convenience.

#![allow(dead_code)]

use std::time::{Duration, Instant};

use serde_json::json;

use gorgon_petra::tree::Interaction;
use gorgon_petra_testkit::driver::{ActTarget, Client as Driver, DriverNode};

use super::journey::tree_root;

/// Press `name`, timed. The client's `act` call already blocks until the
/// server reports the press *applied* (`Client::act`'s own doc comment:
/// "synthesize `kind` at `target`, block until applied and settled"), so
/// the wall-clock span from the request being written to the reply landing
/// is exactly SC-003's "produce their visual response" latency at the
/// driver-protocol boundary — the tightest thing a client on this side of
/// the wire can honestly measure, short of an in-process frame-paint hook
/// this harness does not have.
pub async fn timed_key(driver: &Driver, samples: &mut Vec<Duration>, name: &str) {
    let start = Instant::now();
    driver
        .act(
            Interaction::Key,
            ActTarget::Pos { x: 0.0, y: 0.0 },
            Some(json!({ "key": name })),
        )
        .await
        .unwrap_or_else(|err| panic!("key {name:?} was refused: {err}"));
    samples.push(start.elapsed());
}

/// [`timed_key`] with `"enter"` — the timed twin of `journey::press_enter`.
pub async fn timed_enter(driver: &Driver, samples: &mut Vec<Duration>) {
    timed_key(driver, samples, "enter").await;
}

/// The timed twin of `journey::tab_until` — see the module doc for why this
/// duplicates rather than imports. Same predicate loop, same
/// panic-on-exhaustion diagnostic; the only behavioral difference is that
/// every Tab press lands in `samples`.
pub async fn tab_until(
    driver: &Driver,
    what: &str,
    max_presses: usize,
    samples: &mut Vec<Duration>,
    mut pred: impl FnMut(&DriverNode) -> bool,
) -> DriverNode {
    for _ in 0..=max_presses {
        let root = tree_root(driver).await;
        if let Some(focused) = root.iter().find(|n| n.state.focused)
            && pred(focused)
        {
            return focused.clone();
        }
        timed_key(driver, samples, "tab").await;
    }
    let root = tree_root(driver).await;
    let last = root
        .iter()
        .find(|n| n.state.focused)
        .map(|n| format!("[{}] {:?} role={:?}", n.id, n.label, n.role));
    panic!("never {what} within {max_presses} Tab presses; last focused: {last:?}");
}

/// The 99th percentile of `samples`, nearest-rank method: index
/// `ceil(0.99 * n) - 1`. With few samples (this journey's own count is in
/// the low tens — see `tests/inspector_journey.rs`) that lands at or near
/// the maximum, which is the honest answer for "p99 of a small sample" —
/// never a fabricated interpolation between points that do not exist.
///
/// # Panics
/// If `samples` is empty — a p99 of nothing is not a measurement.
#[must_use]
pub fn p99(samples: &[Duration]) -> Duration {
    assert!(!samples.is_empty(), "p99 of zero samples is undefined");
    let mut sorted: Vec<Duration> = samples.to_vec();
    sorted.sort();
    #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
    let rank = ((sorted.len() as f64) * 0.99).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

/// `VmRSS` from `/proc/<pid>/status`, in kB, exactly as the kernel reports
/// it. `pid` should be the driver server's own pid (`HealthResult::pid`) —
/// the process actually hosting the inspector `Shell` under test. T044's
/// harness runs that `Shell` as a thread inside this test binary, never as
/// a standalone `gorgon-inspector` process (`support::inspector`'s own doc
/// comment), so this process's RSS *is* the honest measurement of "the idle
/// inspector's RSS" in this harness — not a proxy for a separate process
/// that does not exist here.
///
/// `None` if `/proc` is unavailable (a non-Linux host) or the field is
/// missing — named as `unavailable`, never silently reported as `0`.
#[must_use]
pub fn vm_rss_kb(pid: u32) -> Option<u64> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.trim().trim_end_matches("kB").trim().parse().ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{p99, vm_rss_kb};
    use std::time::Duration;

    #[test]
    fn p99_of_nine_samples_is_the_maximum() {
        let samples: Vec<Duration> = (1..=9).map(Duration::from_millis).collect();
        assert_eq!(p99(&samples), Duration::from_millis(9));
    }

    #[test]
    fn p99_of_one_sample_is_that_sample() {
        let samples = vec![Duration::from_millis(42)];
        assert_eq!(p99(&samples), Duration::from_millis(42));
    }

    #[test]
    #[should_panic(expected = "p99 of zero samples is undefined")]
    fn p99_of_zero_samples_panics_rather_than_lying() {
        let _ = p99(&[]);
    }

    #[test]
    fn vm_rss_kb_reads_this_process_own_status() {
        let pid = std::process::id();
        let kb = vm_rss_kb(pid).expect("this process's own /proc/<pid>/status is readable");
        assert!(
            kb > 0,
            "a running process reporting 0 kB RSS is not credible: {kb}"
        );
    }

    #[test]
    fn vm_rss_kb_names_unavailable_rather_than_zero_for_a_bogus_pid() {
        // pid 0 is never a real process's status file on Linux.
        assert_eq!(vm_rss_kb(0), None);
    }
}
