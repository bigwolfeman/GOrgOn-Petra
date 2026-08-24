//! The shared frame state the driver server reads from and the embedding
//! application publishes into.
//!
//! # The design decision this wave is actually about
//!
//! `gorgon-petra-egui::host::Host` lives on the UI thread and owns the event
//! loop (`host.rs`'s own doc comment: "the loop is deliberately small"). The
//! driver server runs its own tokio tasks, possibly on other worker threads.
//! Neither side may block the other — the UI thread cannot await a socket
//! read, and a driver query cannot stall a frame — so nothing here borrows a
//! `Host` or steps it. Instead the *application* (which already owns both:
//! it drives `Host::pass` and it started the server) calls
//! [`FrameHub::publish`] once per pass, handing over what `Host::frame()`
//! just produced; every query the server answers reads back a cheap `Arc`
//! clone of whatever was last published. This is why `server/` depends on
//! nothing from `gorgon-petra-egui` beyond what this file needs — `Host`
//! itself, `host.rs`, `input.rs` and `paint.rs` are untouched by this wave.
//!
//! # What this costs
//!
//! One [`PetrifiedFrame`] clone per **actual** UI frame — not per query, and
//! not on a timer. `Host::pass`'s doc comment is explicit that the host
//! schedules another pass only if something is still moving ("no frames at
//! idle"), so the clone rate here is bounded by real visual change, never by
//! polling: an idle window publishes nothing, at any query rate. The clone
//! happens once per publish regardless of how many driver clients are
//! reading: every reader takes an `Arc` clone of the *published* value (a
//! refcount bump, not a frame copy — see [`FrameHub::current`]), so N
//! concurrent `tree`/`frame` queries against one published frame cost one
//! `PetrifiedFrame` clone in total, not N. The semantic tree is projected
//! once per publish for the same reason: "project once, from the petrified
//! frame. No consumer gets a private variant" (`contracts/semantic-tree.md`),
//! so `tree` queries filter an already-built [`SemanticTree`] rather than
//! re-walking placements on every request.
//!
//! The alternative this rejects is publishing on a timer (poll `Host::frame()`
//! from a background task at, say, 60 Hz) — considered and dropped because it
//! would clone a frame the UI never repainted, on an idle window, forever;
//! the whole point of "no frames at idle" is that idle costs nothing, and a
//! polling publisher would spend a clone a frame proving that nothing
//! changed.

use std::sync::Arc;

use tokio::sync::watch;

use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::semantic::{self, SemanticTree};

use crate::bridge::SettleState;

/// One published frame, and its semantic projection, computed together so a
/// reader's `(frame, tree)` pair is always from the same pass.
pub struct Published {
    /// The frame as `Host::pass` petrified it.
    pub frame: PetrifiedFrame,
    /// What this pass reported about whether the UI is done moving.
    ///
    /// Read by [`crate::settle`] to answer `wait_settle` and to decide when
    /// an `act` may return. Measured at publish time by whoever published —
    /// see [`FrameHub::publish_with`] for what the two publishing paths each
    /// promise.
    pub settle: SettleState,
    /// [`semantic::project`] of the same frame. `None` only for a frame with
    /// zero placements — `petrify` never produces one (the root is always
    /// placed), so in practice this is `Some` from the first real publish
    /// onward; the hub reports the same absence [`semantic::project`] would,
    /// rather than manufacturing an empty tree.
    pub tree: Option<SemanticTree>,
}

/// A cheap, `Clone`-able handle onto the latest published frame.
///
/// Backed by a [`tokio::sync::watch`] channel rather than a plain lock,
/// because the driver has two readers with different needs and one cell has
/// to serve both: `tree`/`frame` want the current value with no waiting
/// ([`FrameHub::current`], a borrow-and-clone of an `Arc`), and `wait_settle`
/// wants to be woken when a *new* frame arrives ([`FrameHub::subscribe`])
/// without polling. A `watch` gives both from one write, and a publish never
/// blocks on a reader.
#[derive(Clone)]
pub struct FrameHub(Arc<watch::Sender<Option<Arc<Published>>>>);

impl FrameHub {
    /// A hub with nothing published yet.
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(watch::Sender::new(None)))
    }

    /// Publish `frame` as the latest state driver queries answer from.
    ///
    /// Clones `frame` once and projects its semantic tree once — see the
    /// module docs for why that cost is bounded by real UI frames, not by
    /// query volume or a timer. Call this after every `Host::pass` that
    /// produced a frame you want visible to the driver; a pass whose frame
    /// you do not publish here is simply not seen by `tree`/`frame` (and, in
    /// practice, an application publishes every pass — `Host::pass` already
    /// only runs when something is worth repainting).
    pub fn publish(&self, frame: &PetrifiedFrame) {
        self.publish_with(frame, SettleState::from_frame(frame));
    }

    /// Publish `frame` together with the settle state the publisher measured
    /// for this pass.
    ///
    /// The two paths differ in exactly one way, and it is worth being
    /// explicit because a driver's `wait_settle` believes what is published
    /// here:
    ///
    /// * [`FrameHub::publish`] derives the state from the frame alone. It
    ///   reports the frame's own transition counts and says nothing is
    ///   queued and no repaint is pending — true for an application with no
    ///   [`crate::bridge::UiBridge`], because there is nothing that could
    ///   queue work for it.
    /// * `publish_with` is what
    ///   [`crate::driver_host::DriverHost::step`] calls, and it reports the
    ///   bridge queue and egui's own repaint request as well, because with a
    ///   bridge attached those can be non-empty and a settle that ignored
    ///   them would return while an action was still waiting to be applied.
    pub fn publish_with(&self, frame: &PetrifiedFrame, settle: SettleState) {
        let tree = semantic::project(frame);
        let published = Arc::new(Published {
            frame: frame.clone(),
            tree,
            settle,
        });
        // `send_replace`, not `send`: a publish must not fail because no
        // driver client happens to be subscribed at that instant. The UI
        // thread publishes whether or not anybody is listening.
        self.0.send_replace(Some(published));
    }

    /// The most recently published frame, or `None` before the first
    /// [`FrameHub::publish`].
    ///
    /// Cheap: this clones the `Arc`, not the frame, and holds the read lock
    /// only long enough to do that — so a query never blocks a publish (or
    /// another query) for longer than a refcount bump.
    #[must_use]
    pub fn current(&self) -> Option<Arc<Published>> {
        self.0.borrow().clone()
    }

    /// A receiver that is woken by every [`FrameHub::publish_with`].
    ///
    /// What [`crate::settle`] waits on. The receiver starts up to date, so a
    /// caller that subscribes and then immediately checks
    /// [`FrameHub::current`] cannot miss a frame published in between: it
    /// will see it in the borrow, and the next `changed()` will report the
    /// frame after it.
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<Option<Arc<Published>>> {
        self.0.subscribe()
    }
}

impl Default for FrameHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::FrameHub;
    use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, digest};
    use gorgon_petra::geom::Size;
    use gorgon_petra::token::ThemeMode;

    fn empty_frame(seq: u64) -> PetrifiedFrame {
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

    #[test]
    fn nothing_is_published_before_the_first_call() {
        let hub = FrameHub::new();
        assert!(hub.current().is_none());
    }

    #[test]
    fn a_publish_is_visible_to_every_clone_of_the_hub() {
        let hub = FrameHub::new();
        let reader = hub.clone();
        hub.publish(&empty_frame(1));
        let published = reader.current().expect("published frame");
        assert_eq!(published.frame.seq, 1);
        // `petrify` never produces zero placements; a hand-built empty frame
        // is the one case `project` truly returns `None` for, and the hub
        // must report that rather than inventing a tree.
        assert!(published.tree.is_none());
    }

    #[test]
    fn a_later_publish_replaces_the_earlier_one() {
        let hub = FrameHub::new();
        hub.publish(&empty_frame(1));
        hub.publish(&empty_frame(2));
        assert_eq!(hub.current().unwrap().frame.seq, 2);
    }
}
