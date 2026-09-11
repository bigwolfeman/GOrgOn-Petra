//! The seam between a driver task and the UI thread.
//!
//! `health`, `tree` and `frame` are answerable from a snapshot: the server
//! reads whatever [`crate::server::FrameHub`] last published and never
//! touches the application. `act` and `screenshot` cannot be. Injecting an
//! action needs `&mut Host`, and capturing pixels needs the live
//! [`egui::Context`] — both of which live on the UI thread, are not `Send`
//! across a pass boundary in any useful way, and must not be reached into
//! from a socket task while a frame is being placed.
//!
//! So the driver does not reach in. It **queues a [`Job`] and waits**. The
//! application drains the queue once per loop iteration
//! ([`crate::driver_host::DriverHost::step`]), services each ticket on the UI
//! thread where the state already is, and answers. The socket task's
//! [`UiBridge::submit`] resolves when that answer arrives.
//!
//! # Why the queue wakes the UI thread
//!
//! Petra paints no frames at idle — that is FR-033 and SC-002, and it is
//! measured, not aspirational. An idle window therefore runs no loop
//! iteration, and a job queued into a silent application would wait forever.
//! [`UiBridge::attach`] gives the bridge a clone of the application's
//! [`egui::Context`] so [`UiBridge::submit`] can call `request_repaint`, the
//! documented thread-safe wake-up. An application that never attaches one
//! still works — it just has to be stepping for its own reasons.
//!
//! # Why the reply is a `oneshot` and not a shared cell
//!
//! Every job has exactly one answer and exactly one waiter. If the UI thread
//! ends without servicing a ticket, the sender drops, the receiver errors,
//! and [`UiBridge::submit`] turns that into an honest `timeout` naming the
//! job — instead of a socket task parked forever on a cell nobody will ever
//! write.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use gorgon_petra_egui::inject::{Action, Target};

use crate::wire::{ErrorKind, WireError, WireRect};

/// Work only the UI thread can do.
#[derive(Debug, Clone)]
pub enum Job {
    /// Inject one driver action's platform events, then run a pass.
    ///
    /// Carries exactly what [`gorgon_petra_egui::inject::inject_action`]
    /// takes, because that is the function that services it — the bridge
    /// adds no second vocabulary for actions.
    Act {
        /// What the action aims at: a node id, or a raw position.
        target: Target,
        /// Which of the eight closed action kinds.
        action: Action,
    },
    /// Capture the frame this pass paints.
    Capture {
        /// A crop in device pixels, or the whole viewport.
        region: Option<WireRect>,
    },
}

impl Job {
    /// A short name for this job, for a message a human has to read.
    #[must_use]
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Act { .. } => "act",
            Self::Capture { .. } => "screenshot",
        }
    }
}

/// What the UI thread answers a [`Job`] with.
///
/// [`Answer::Captured`] carries the capture's *fields* rather than
/// `crate::snapshot`'s own type on purpose: this module is the seam, and a
/// seam that names one side's concrete result type stops being a seam. The
/// one place that converts is
/// [`crate::driver_host::DriverHost`], which calls the snapshotter.
#[derive(Debug, Clone)]
pub enum Answer {
    /// The action's events were injected and consumed by frame
    /// `applied_frame_seq`.
    Acted {
        /// The `seq` of the frame that ran with the injected events in its
        /// input. Never zero: a serviced `act` always produces a pass.
        applied_frame_seq: u64,
    },
    /// A capture of one specific frame.
    Captured {
        /// The captured frame's sequence.
        seq: u64,
        /// The captured frame's digest, hex-encoded.
        digest: String,
        /// PNG bytes.
        png: Vec<u8>,
        /// Whether a host-registered custom painter drew into this frame
        /// (FR-060), read from `PetrifiedFrame::hosted`.
        hosted: bool,
    },
    /// The UI thread refused the job, in the protocol's own error vocabulary.
    Refused(WireError),
}

/// One queued job and the channel its answer goes back on.
///
/// Held only between [`UiBridge::take_pending`] and [`Ticket::answer`]. A
/// ticket dropped without an answer resolves the waiter as a `timeout`
/// naming the job — see [`UiBridge::submit`] — so losing one is loud rather
/// than a hang.
pub struct Ticket {
    job: Job,
    reply: tokio::sync::oneshot::Sender<Answer>,
}

impl Ticket {
    /// What was asked for.
    #[must_use]
    pub fn job(&self) -> &Job {
        &self.job
    }

    /// Answer it. Consumes the ticket, so a job cannot be answered twice.
    ///
    /// The result of the send is deliberately dropped: a waiter that gave up
    /// (its socket closed) is not the UI thread's problem, and there is
    /// nothing useful for a UI loop to do about it.
    pub fn answer(self, answer: Answer) {
        let _ = self.reply.send(answer);
    }
}

struct Inner {
    queue: Mutex<VecDeque<Ticket>>,
    waker: RwLock<Option<egui::Context>>,
}

/// A cheap, `Clone`-able handle onto the job queue.
///
/// Every clone shares one queue (an `Arc` inside), so the application keeps
/// one and each socket task keeps another.
#[derive(Clone)]
pub struct UiBridge(Arc<Inner>);

impl UiBridge {
    /// A bridge with an empty queue and no waker.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(Inner {
            queue: Mutex::new(VecDeque::new()),
            waker: RwLock::new(None),
        }))
    }

    /// Give the bridge the application's context, so a queued job can wake an
    /// idle window. Called by [`crate::driver_host::DriverHost::new`].
    pub fn attach(&self, ctx: &egui::Context) {
        let mut guard = self.0.waker.write().unwrap_or_else(PoisonError::into_inner);
        *guard = Some(ctx.clone());
    }

    /// Queue `job` and wait for the UI thread's answer.
    ///
    /// # Panics
    /// Never. A UI thread that ends without servicing the job drops the
    /// ticket, which resolves here as an [`ErrorKind::Timeout`] naming the
    /// verb rather than parking the caller forever.
    pub async fn submit(&self, job: Job) -> Answer {
        let verb = job.verb();
        let (reply, wait) = tokio::sync::oneshot::channel();
        {
            let mut queue = self.0.queue.lock().unwrap_or_else(PoisonError::into_inner);
            queue.push_back(Ticket { job, reply });
        }
        // After the push, never before: a wake that arrives before the job is
        // visible can service an empty queue and go back to sleep.
        if let Some(ctx) = self
            .0
            .waker
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            ctx.request_repaint();
        }
        wait.await.unwrap_or_else(|_| {
            Answer::Refused(WireError::new(
                ErrorKind::Timeout,
                format!(
                    "the UI thread ended before it serviced this `{verb}`; \
                     the application is no longer running a loop"
                ),
            ))
        })
    }

    /// Take every queued ticket, leaving the queue empty. Non-blocking, and
    /// safe to call from the UI thread every iteration — an empty queue costs
    /// one uncontended lock.
    #[must_use]
    pub fn take_pending(&self) -> Vec<Ticket> {
        let mut queue = self.0.queue.lock().unwrap_or_else(PoisonError::into_inner);
        queue.drain(..).collect()
    }

    /// How many jobs are queued and not yet taken.
    ///
    /// This is a settle input ([`SettleState::queued_jobs`]): a UI with work
    /// waiting for it has not finished moving, whatever the frame says.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.0
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl Default for UiBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Re-exported from [`crate::wire`], where it lives because three things
/// read it and only one of them may depend on `egui`: this module writes it,
/// [`crate::settle`] decides settledness from it, and the ungated driver
/// client parses it off the wire (FR-041).
pub use crate::wire::SettleState;

#[cfg(test)]
mod tests {
    use super::{Answer, Job, UiBridge};
    use crate::wire::ErrorKind;

    fn hover() -> Job {
        Job::Act {
            target: gorgon_petra_egui::inject::Target::NodeId("/root".into()),
            action: gorgon_petra_egui::inject::Action::Hover,
        }
    }

    #[tokio::test]
    async fn a_submitted_job_is_answered_by_whoever_takes_the_ticket() {
        let bridge = UiBridge::new();
        let ui_side = bridge.clone();
        let submit = tokio::spawn(async move { bridge.submit(hover()).await });
        // Stand in for the UI loop: spin until the job shows up, service it.
        let answer = loop {
            let mut pending = ui_side.take_pending();
            if let Some(ticket) = pending.pop() {
                assert_eq!(ticket.job().verb(), "act");
                ticket.answer(Answer::Acted {
                    applied_frame_seq: 7,
                });
                break submit.await.unwrap();
            }
            tokio::task::yield_now().await;
        };
        match answer {
            Answer::Acted { applied_frame_seq } => assert_eq!(applied_frame_seq, 7),
            other => panic!("expected Acted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_ticket_dropped_unanswered_is_a_named_timeout_not_a_hang() {
        let bridge = UiBridge::new();
        let ui_side = bridge.clone();
        let submit = tokio::spawn(async move { bridge.submit(hover()).await });
        let answer = loop {
            let pending = ui_side.take_pending();
            if !pending.is_empty() {
                // The UI thread ends here: the tickets drop unanswered.
                drop(pending);
                break submit.await.unwrap();
            }
            tokio::task::yield_now().await;
        };
        match answer {
            Answer::Refused(err) => {
                assert_eq!(err.kind, ErrorKind::Timeout);
                assert!(err.message.contains("act"), "{}", err.message);
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn queued_counts_what_the_ui_thread_has_not_taken_yet() {
        let bridge = UiBridge::new();
        assert_eq!(bridge.queued(), 0);
        let a = bridge.clone();
        let b = bridge.clone();
        let one = tokio::spawn(async move { a.submit(hover()).await });
        let two = tokio::spawn(async move { b.submit(hover()).await });
        while bridge.queued() < 2 {
            tokio::task::yield_now().await;
        }
        assert_eq!(bridge.queued(), 2);
        for ticket in bridge.take_pending() {
            ticket.answer(Answer::Acted {
                applied_frame_seq: 1,
            });
        }
        assert_eq!(bridge.queued(), 0);
        one.await.unwrap();
        two.await.unwrap();
    }
}
