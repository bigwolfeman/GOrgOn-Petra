//! Settle detection: when has the UI stopped moving, and which frame proves
//! it.
//!
//! Binding: `specs/003-petra-layout-engine/contracts/driver-protocol.md`
//! ("Action semantics" rules 2 and 4) and `contracts/animation.md` (FR-039,
//! FR-032). This module answers the `wait_settle` verb and supplies `act`
//! with the second half of its `{applied_frame_seq, settled_frame_seq}` pair.
//!
//! # What "settled" means, and the one thing it deliberately ignores
//!
//! A published frame is settled when all three of these hold:
//!
//! * `running_transitions == 0` — nothing is still moving toward a target;
//! * `queued_jobs == 0` — no [`crate::bridge::Job`] is waiting for the UI
//!   thread, so no synthetic input is still undelivered;
//! * `!repaint_pending` — egui did not ask for another frame.
//!
//! `ambient_transitions` is **excluded** (FR-039). A declared-ambient
//! animation never settles by construction (`contracts/animation.md`: "`ambient`
//! animations never settle and are excluded from the driver's settle wait"),
//! so a settle that waited on one would never return and would take every
//! driver journey with it. Ambient is still *reported* — see [`Pending`] — so
//! a caller reading a timeout can tell "3 ambient animations are running,
//! which do not block settle" from a reason that actually blocked.
//!
//! # Why the words and the boolean cannot disagree
//!
//! The predicate is expressed exactly once, in [`Pending::of`]: a condition
//! blocks settle if and only if it pushes a sentence onto
//! [`Pending::blocking`], and [`is_settled`] is `blocking.is_empty()`. There
//! is no second copy of the rule to drift, and a `settled: true` beside a
//! non-empty reason list is unrepresentable rather than merely unlikely.
//!
//! # Why a `watch`, and which race that closes
//!
//! The wait is driven by [`FrameHub::subscribe`] — a
//! [`tokio::sync::watch`] receiver — and never polls the hub on a timer.
//! Petra paints no frames at idle (FR-033, SC-002), so a poll loop would burn
//! wakeups proving nothing changed, and the settle wait is on the hot path of
//! every single `act` a driver issues.
//!
//! The race that has to be closed is **a frame published between the
//! subscribe and the first check**. It is closed by checking
//! [`watch::Receiver::borrow_and_update`] *before* the first
//! `changed().await`, never after. Two facts make that sufficient, and both
//! are properties of `watch` rather than of timing:
//!
//! 1. A receiver from `Sender::subscribe` starts marked as having seen the
//!    value current at that instant. That value is therefore reachable only
//!    through `borrow`/`borrow_and_update` — `changed()` will never report
//!    it. Checking the borrow first is what makes an already-settled UI
//!    answer immediately instead of hanging until something else happens to
//!    repaint.
//! 2. `changed()` is **level-triggered on a version counter**, not
//!    edge-triggered on a wakeup. A publish that lands after
//!    `borrow_and_update` but before the `changed()` future is first polled
//!    bumps the version, so `changed()` resolves immediately rather than
//!    losing the edge.
//!
//! Tests `a_frame_published_before_the_wait_is_seen_immediately` and
//! `the_wait_wakes_on_a_publish_rather_than_after_a_fixed_sleep` pin case 1
//! and case 2 respectively; the second asserts on elapsed time being a small
//! fraction of the timeout, so a wait that had degraded into a sleep would
//! fail it rather than pass slowly.
//!
//! **What a `watch` gives up, and why it is safe here:** it keeps only the
//! latest value, so two frames published between two checks collapse into
//! one and the earlier is never observed. That cannot hide a settle, because
//! a settled state is *terminal* until new input arrives — the host stops
//! scheduling passes the moment the last non-ambient transition settles
//! (`contracts/animation.md` rule 2), so the settled frame is the value that
//! stays in the cell, not one that flies past. What coalescing can hide is an
//! intermediate *unsettled* frame, which no caller asked about.
//!
//! # `applied_frame_seq` vs `settled_frame_seq` (read this before asserting)
//!
//! `act` answers with both, and they are allowed to differ — the difference
//! is the point:
//!
//! * **`applied_frame_seq`** is the frame that *consumed the injected
//!   events*. The pass ran with the synthetic events in its input, so
//!   hit-testing, focus, and event ordering already happened. It comes from
//!   [`crate::bridge::Answer::Acted`], measured on the UI thread; this module
//!   only carries it through.
//! * **`settled_frame_seq`** is the first frame at or after `applied` that
//!   reports nothing pending. It is where the UI *stopped moving* again.
//!
//! Click a button that starts a 200 ms transition and they differ by however
//! many frames that transition took. Click one that changes nothing animated
//! and they are equal. Assert on `applied` to ask "did my input land"; assert
//! on `settled` (or on a `tree`/`screenshot` taken after it) to ask "what does
//! the UI look like now that it is done". Frames published *before* `applied`
//! are never eligible: a stale settled frame from before the action would
//! otherwise answer instantly and every subsequent assertion would race the
//! UI.
//!
//! # Timeouts
//!
//! Enforced with [`tokio::time::sleep_until`] against a deadline taken once,
//! before the first check, so a wait can never outlive its budget by looping.
//! Two special cases are decided here rather than left to a caller:
//!
//! * **`timeout_ms` absent (`None`)** — [`DEFAULT_TIMEOUT_MS`]. `wait_settle`
//!   takes `{timeout_ms}` in the contract, but a driver that omits it must
//!   not get an unbounded wait: the server would park a connection task
//!   forever on an application that stopped stepping, which is the exact
//!   failure mode [`crate::bridge::UiBridge::submit`] refuses for jobs. A
//!   bounded default answers, with words, instead of hanging.
//! * **`timeout_ms == 0`** — answer from what is published right now, without
//!   waiting at all. The deadline is already in the past, so the first check
//!   runs and the sleep arm fires immediately; a caller gets "is it settled
//!   *at this instant*", which is a genuinely useful non-blocking probe and
//!   can never hang. Zero is deliberately **not** a synonym for "no timeout"
//!   for the same reason `None` is not.

use std::time::Duration;

use tokio::time::Instant;

use crate::server::FrameHub;
use crate::wire::{ErrorKind, WireError};

// The vocabulary this module decides with lives in `crate::wire`, not here:
// the importable driver client parses the same three shapes off the wire with
// no feature on, and a second copy over there would be a second definition of
// the protocol. Re-exported so `settle::Pending` still names the one type.
pub use crate::wire::{ActResult, Pending, SettleResult, is_settled};

/// The wait budget used when a request carries no `timeout_ms`.
///
/// Five seconds: long enough for any declared transition
/// (`contracts/animation.md` durations are in the hundreds of milliseconds)
/// plus a slow debug-build frame, short enough that a driver blocked on a
/// stopped application learns about it inside one test's patience.
pub const DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// Wait for a settled frame whose `seq` is at least `min_frame_seq`.
///
/// The general form both verbs are built on. `min_frame_seq == 0` accepts any
/// frame (`seq` starts at 1), which is what `wait_settle` wants; `act` passes
/// its `applied_frame_seq`, which is what makes a stale pre-action frame
/// ineligible to answer.
///
/// Never panics and never waits longer than the timeout. See the module docs
/// for the `None`/zero timeout policy and for why this does not busy-poll.
pub async fn settle_at_or_after(
    hub: &FrameHub,
    min_frame_seq: u64,
    timeout_ms: Option<u64>,
) -> SettleResult {
    // Subscribe first, deadline second, check third. The receiver holds the
    // value current at subscribe time, so nothing published before this line
    // is lost and nothing published after it is missed — see the module docs.
    let mut rx = hub.subscribe();
    let deadline = Instant::now() + Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));
    let mut newest_seq = 0_u64;
    let mut pending = Pending::no_frame_yet();
    loop {
        // The borrow guard is scoped to this block: a `watch::Ref` must never
        // be held across an await, and holding one would block publishes.
        let observed = {
            let guard = rx.borrow_and_update();
            guard.as_ref().map(|p| (p.frame.seq, p.settle))
        };
        if let Some((seq, state)) = observed {
            newest_seq = seq;
            if seq >= min_frame_seq {
                let reported = Pending::of(state);
                if reported.is_settled() {
                    return SettleResult {
                        settled: true,
                        frame_seq: seq,
                        pending: reported,
                    };
                }
                pending = reported;
            } else {
                pending = Pending::awaiting_applied_frame(min_frame_seq, seq);
            }
        }
        tokio::select! {
            biased;
            woken = rx.changed() => {
                if woken.is_err() {
                    return SettleResult {
                        settled: false,
                        frame_seq: newest_seq,
                        pending: Pending::publisher_gone(),
                    };
                }
            }
            () = tokio::time::sleep_until(deadline) => {
                return SettleResult { settled: false, frame_seq: newest_seq, pending };
            }
        }
    }
}

/// Answer the `wait_settle` verb.
///
/// Resolves as soon as a settled frame is published — including one that was
/// already published before the call, so an idle UI answers immediately
/// rather than waiting for a repaint that "no frames at idle" guarantees will
/// never come. On the deadline it returns `settled: false` with every pending
/// reason named, which is a *result*, not an error: the contract's verb table
/// gives `wait_settle` a `settled: bool`, so a timeout is an answer the
/// caller reads, not an exception it catches.
pub async fn wait_settle(hub: &FrameHub, timeout_ms: Option<u64>) -> SettleResult {
    settle_at_or_after(hub, 0, timeout_ms).await
}

/// The second half of `act`: given the frame that applied the action, resolve
/// the frame at which the UI stopped moving.
///
/// This is what the `act` handler calls after
/// [`crate::bridge::UiBridge::submit`] answers
/// [`crate::bridge::Answer::Acted`]. Frames older than `applied_frame_seq`
/// cannot answer it (rule 2 of "Action semantics": `act` responds only after
/// the action is applied *and* the UI settles).
///
/// # Errors
///
/// [`ErrorKind::Timeout`] when the UI had not settled by the deadline. The
/// message names both the applied frame and every reason that was still
/// pending, per the contract's error table ("`timeout` (names pending work)").
pub async fn settle_after_act(
    hub: &FrameHub,
    applied_frame_seq: u64,
    timeout_ms: Option<u64>,
) -> Result<ActResult, WireError> {
    let result = settle_at_or_after(hub, applied_frame_seq, timeout_ms).await;
    if result.settled {
        Ok(ActResult {
            applied_frame_seq,
            settled_frame_seq: result.frame_seq,
        })
    } else {
        Err(WireError::new(
            ErrorKind::Timeout,
            format!(
                "the action was applied by frame {applied_frame_seq}, but the UI had not \
                 settled when the wait expired: {}",
                result.pending.describe()
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        Pending, SettleResult, is_settled, settle_after_act, settle_at_or_after, wait_settle,
    };
    use crate::bridge::SettleState;
    use crate::server::FrameHub;
    use crate::wire::ErrorKind;

    use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, digest};
    use gorgon_petra::geom::Size;
    use gorgon_petra::token::ThemeMode;

    /// A hand-built frame, exactly as `server/hub.rs`'s own tests build one.
    /// What is under test here is settle accounting over published state, not
    /// layout, so a real `Host::pass` would add cost and no coverage.
    fn frame(seq: u64) -> PetrifiedFrame {
        let viewport = Viewport::new(Size::new(100.0, 100.0), ThemeMode::Dark);
        PetrifiedFrame {
            seq,
            digest: digest::digest(&viewport, &[]),
            placements: Vec::new(),
            content: Vec::new(),
            subtree_hashes: Vec::new(),
            subtree_len: Vec::new(),
            slots: Vec::new(),
            viewport,
            transitions: TransitionActivity::default(),
        }
    }

    /// A settle state for frame `seq` with everything quiet.
    fn quiet(seq: u64) -> SettleState {
        SettleState {
            frame_seq: seq,
            ..SettleState::default()
        }
    }

    fn publish(hub: &FrameHub, state: SettleState) {
        hub.publish_with(&frame(state.frame_seq), state);
    }

    fn joined(result: &SettleResult) -> String {
        result.pending.describe()
    }

    // ---------------------------------------------------------------- FR-039

    #[test]
    fn ambient_only_is_settled() {
        let state = SettleState {
            frame_seq: 1,
            ambient_transitions: 3,
            ..SettleState::default()
        };
        assert!(
            is_settled(state),
            "FR-039: declared-ambient animation never settles, so waiting on it would \
             never return; it must not block settle"
        );
        let pending = Pending::of(state);
        assert!(pending.blocking.is_empty(), "{:?}", pending.blocking);
        assert_eq!(pending.not_blocking.len(), 1);
        assert!(
            pending.not_blocking[0].contains("do not block settle"),
            "{}",
            pending.not_blocking[0]
        );
    }

    #[tokio::test]
    async fn an_ambient_only_frame_resolves_a_real_wait() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                ambient_transitions: 2,
                ..SettleState::default()
            },
        );
        let result = wait_settle(&hub, Some(200)).await;
        assert!(result.settled, "{}", joined(&result));
        assert_eq!(result.frame_seq, 1);
        assert_eq!(result.pending.ambient_transitions, 2);
        assert!(result.pending.blocking.is_empty());
    }

    #[test]
    fn the_words_and_the_boolean_never_disagree() {
        for running in [0_usize, 1, 2] {
            for queued in [0_usize, 1] {
                for repaint in [false, true] {
                    for ambient in [0_usize, 4] {
                        let state = SettleState {
                            frame_seq: 1,
                            applied_frame_seq: 0,
                            running_transitions: running,
                            ambient_transitions: ambient,
                            queued_jobs: queued,
                            repaint_pending: repaint,
                        };
                        let expected = running == 0 && queued == 0 && !repaint;
                        assert_eq!(
                            is_settled(state),
                            expected,
                            "settled predicate disagreed for {state:?}"
                        );
                        assert_eq!(
                            Pending::of(state).blocking.is_empty(),
                            expected,
                            "the reason list disagreed with the predicate for {state:?}"
                        );
                    }
                }
            }
        }
    }

    // ------------------------------------------------------- blocking reasons

    #[tokio::test]
    async fn running_transitions_block_and_clear_on_a_later_frame() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                running_transitions: 2,
                ..SettleState::default()
            },
        );
        let blocked = wait_settle(&hub, Some(0)).await;
        assert!(!blocked.settled);
        assert_eq!(blocked.frame_seq, 1);
        assert!(
            blocked
                .pending
                .blocking
                .contains(&"2 transitions running".to_owned()),
            "{:?}",
            blocked.pending.blocking
        );

        // A later frame reports zero running: the same hub now settles.
        publish(&hub, quiet(2));
        let cleared = wait_settle(&hub, Some(0)).await;
        assert!(cleared.settled, "{}", joined(&cleared));
        assert_eq!(cleared.frame_seq, 2);
    }

    #[tokio::test]
    async fn a_queued_job_blocks_even_when_the_frame_itself_looks_quiet() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                queued_jobs: 1,
                ..SettleState::default()
            },
        );
        let result = wait_settle(&hub, Some(0)).await;
        assert!(
            !result.settled,
            "a job the UI thread has not serviced is undelivered synthetic input; the UI \
             is not done moving however quiet the frame looks"
        );
        assert_eq!(result.pending.blocking, vec!["1 job queued".to_owned()]);
        assert_eq!(result.pending.running_transitions, 0);
    }

    #[tokio::test]
    async fn a_timeout_names_every_pending_reason_that_was_actually_true() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 9,
                applied_frame_seq: 9,
                running_transitions: 2,
                ambient_transitions: 3,
                queued_jobs: 1,
                repaint_pending: true,
            },
        );
        let result = wait_settle(&hub, Some(30)).await;
        assert!(!result.settled);
        assert_eq!(result.frame_seq, 9);
        assert_eq!(
            result.pending.blocking,
            vec![
                "2 transitions running".to_owned(),
                "1 job queued".to_owned(),
                "a repaint is pending".to_owned(),
            ]
        );
        assert_eq!(
            result.pending.not_blocking,
            vec!["3 ambient animations running, which do not block settle (FR-039)".to_owned()]
        );
        let described = result.pending.describe();
        for phrase in [
            "2 transitions running",
            "1 job queued",
            "a repaint is pending",
            "3 ambient animations running",
        ] {
            assert!(described.contains(phrase), "{described}");
        }
    }

    #[tokio::test]
    async fn a_wait_with_no_frame_at_all_says_so() {
        let hub = FrameHub::new();
        let result = wait_settle(&hub, Some(20)).await;
        assert!(!result.settled);
        assert_eq!(result.frame_seq, 0, "seq starts at 1; 0 means none seen");
        assert!(result.pending.blocking[0].contains("no frame has been published yet"));
    }

    // -------------------------------------------------------------- wake-ups

    #[tokio::test]
    async fn a_frame_published_before_the_wait_is_seen_immediately() {
        // Race 1: the value current at subscribe time is never reported by
        // `changed()`. If the wait awaited first and checked second, this
        // would hang until the timeout and the elapsed assertion would fail.
        let hub = FrameHub::new();
        publish(&hub, quiet(4));
        let started = std::time::Instant::now();
        let result = wait_settle(&hub, Some(30_000)).await;
        let elapsed = started.elapsed();
        assert!(result.settled, "{}", joined(&result));
        assert_eq!(result.frame_seq, 4);
        assert!(
            elapsed < Duration::from_secs(1),
            "answered after {elapsed:?}; that is a poll, not a pre-check"
        );
    }

    #[tokio::test]
    async fn the_wait_wakes_on_a_publish_rather_than_after_a_fixed_sleep() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                running_transitions: 1,
                ..SettleState::default()
            },
        );
        let publisher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            publish(&publisher, quiet(2));
        });
        let started = std::time::Instant::now();
        let result = wait_settle(&hub, Some(30_000)).await;
        let elapsed = started.elapsed();
        assert!(result.settled, "{}", joined(&result));
        assert_eq!(
            result.frame_seq, 2,
            "resolved at the frame that reported zero running transitions"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "a 30s timeout answered in {elapsed:?}; anything near the timeout means the \
             wait slept instead of waking on the publish"
        );
    }

    // -------------------------------------------------------------- timeouts

    #[tokio::test]
    async fn a_zero_timeout_answers_from_the_current_frame_without_waiting() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                repaint_pending: true,
                ..SettleState::default()
            },
        );
        let started = std::time::Instant::now();
        let result = wait_settle(&hub, Some(0)).await;
        let elapsed = started.elapsed();
        assert!(!result.settled);
        assert_eq!(
            result.pending.blocking,
            vec!["a repaint is pending".to_owned()]
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "timeout_ms: 0 must not wait; it took {elapsed:?}"
        );

        // ...and it still answers `true` when the current frame is settled.
        publish(&hub, quiet(2));
        let settled = wait_settle(&hub, Some(0)).await;
        assert!(settled.settled, "{}", joined(&settled));
        assert_eq!(settled.frame_seq, 2);
    }

    #[tokio::test]
    async fn an_absent_timeout_waits_rather_than_answering_immediately() {
        // `None` must be the default budget, not zero: a settled frame that
        // arrives 25 ms later has to be caught. `DEFAULT_TIMEOUT_MS` is what
        // makes that true, and shrinking it below the publish delay below is
        // what would break this test.
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 1,
                queued_jobs: 2,
                ..SettleState::default()
            },
        );
        let publisher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(25)).await;
            publish(&publisher, quiet(2));
        });
        let result = wait_settle(&hub, None).await;
        assert!(result.settled, "{}", joined(&result));
        assert_eq!(result.frame_seq, 2);
    }

    // ------------------------------------------- applied vs settled, for act

    #[tokio::test]
    async fn act_reports_both_frames_and_they_are_allowed_to_differ() {
        let hub = FrameHub::new();
        // Frame 5 applied the action and started a transition.
        publish(
            &hub,
            SettleState {
                frame_seq: 5,
                applied_frame_seq: 5,
                running_transitions: 1,
                ..SettleState::default()
            },
        );
        let publisher = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(15)).await;
            publish(
                &publisher,
                SettleState {
                    frame_seq: 7,
                    applied_frame_seq: 5,
                    ..SettleState::default()
                },
            );
        });
        let result = settle_after_act(&hub, 5, Some(30_000)).await.unwrap();
        assert_eq!(result.applied_frame_seq, 5);
        assert_eq!(result.settled_frame_seq, 7);
    }

    #[tokio::test]
    async fn act_settles_at_the_applied_frame_when_nothing_animates() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 5,
                applied_frame_seq: 5,
                ..SettleState::default()
            },
        );
        let result = settle_after_act(&hub, 5, Some(200)).await.unwrap();
        assert_eq!(result.applied_frame_seq, 5);
        assert_eq!(
            result.settled_frame_seq, 5,
            "an action that starts nothing animated settles on the frame that applied it"
        );
    }

    #[tokio::test]
    async fn a_settled_frame_older_than_the_action_cannot_answer_for_it() {
        let hub = FrameHub::new();
        publish(&hub, quiet(3));
        let err = settle_after_act(&hub, 5, Some(30)).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::Timeout);
        assert!(err.message.contains("seq 5"), "{}", err.message);
        assert!(
            err.message.contains("newest published frame is seq 3"),
            "{}",
            err.message
        );
    }

    #[tokio::test]
    async fn an_act_timeout_is_a_named_timeout_error() {
        let hub = FrameHub::new();
        publish(
            &hub,
            SettleState {
                frame_seq: 5,
                applied_frame_seq: 5,
                running_transitions: 1,
                ambient_transitions: 1,
                ..SettleState::default()
            },
        );
        let err = settle_after_act(&hub, 5, Some(30)).await.unwrap_err();
        assert_eq!(err.kind, ErrorKind::Timeout);
        assert!(
            err.message.contains("applied by frame 5"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains("1 transition running"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains("does not block settle"),
            "ambient must be reported as non-blocking, not hidden: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn a_minimum_seq_of_zero_accepts_the_first_frame_there_is() {
        let hub = FrameHub::new();
        publish(&hub, quiet(1));
        let result = settle_at_or_after(&hub, 0, Some(200)).await;
        assert!(result.settled, "{}", joined(&result));
        assert_eq!(result.frame_seq, 1);
    }
}
