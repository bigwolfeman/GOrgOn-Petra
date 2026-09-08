//! A contribution is a retained snapshot: the fiber pushes, the shell pulls
//! the last push.
//!
//! `specs/005-petra-carbon-authoring/contracts/surface-contribution.md` §8.
//! The shell must not call into a fiber during a frame — `App::view()` is
//! synchronous on the render thread inside `Host::pass`, and a call into a
//! coroutine there is the stall FR-043 forbids. So the frame path makes no
//! call, and because it makes no call there is nothing to wait on, no clock to
//! read, and no expiry to tune. A slow, blocked or budget-killed fiber simply
//! stops pushing.
//!
//! What the shell renders in that case is the last snapshot it accepted, and
//! what it says about it is the truth: `semantics.stale = true`, plus the
//! snapshot's age beside it. Both facts come from this ledger, which is the
//! only thing here that remembers one frame to the next.
//!
//! # What makes a snapshot stale
//!
//! Not age. A status bar that pushed once and has had nothing to say since is
//! current, not stale, and marking it stale would make the flag mean "old",
//! which is not a defect and not actionable. A snapshot is stale exactly when
//! **a push is outstanding**: the shell told the fiber something happened
//! ([`ContributionLedger::mark_behind`]) and the answering push has not
//! arrived.
//! That is a fact the shell already holds, needs no clock to decide, and
//! clears itself the moment the fiber answers.
//!
//! # What an age is measured in
//!
//! Frames. [`crate::frame::PetrifiedFrame::seq`] is petra's own monotone
//! counter, it is already stamped on every projected node
//! ([`crate::semantic::SemanticNode::frame_seq`]), and it is the same number a
//! driver and a screenshot consumer quote. A wall clock would put a second,
//! unrelated ordering beside the frame sequence and make two runs of one
//! fixture disagree.
//!
//! # How the flag reaches the picture
//!
//! Through the chain that is already there, unmodified:
//! [`crate::tree::Semantics::stale`] → [`crate::frame::PlacementSemantics`] →
//! [`crate::semantic::NodeState::stale`] →
//! [`crate::semantic::StateFlag::Stale`]. [`ContributionStatus::mount`] is the
//! one place that sets the first link for a contribution, so there is no
//! second path a contribution's staleness could travel by.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as MapEntry;

use serde::Serialize;

use crate::semantic::attribution::{ContributionId, contribution_key, owner_of};
use crate::tree::ViewNode;

/// What became of a push handed to [`ContributionLedger::accept`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub enum PushOutcome {
    /// The ledger holds this push now: it is the newest snapshot of that
    /// contribution, and any outstanding request is answered.
    Advanced,
    /// Dropped. Its revision is not newer than the snapshot already held, so
    /// it is a duplicate or an out-of-order arrival, and taking it would
    /// replace a newer picture with an older one.
    Superseded,
}

/// One contribution's retained snapshot, as of one frame.
///
/// Published beside the surface — this is the "age published beside it" §8
/// asks for. Every member is a raw fact; the two derived questions a reader
/// asks are [`ContributionStatus::stale`] and
/// [`ContributionStatus::stale_for_frames`], which are methods rather than
/// fields so no two members of one status can disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ContributionStatus {
    /// Which contribution this is, as `ui:7`.
    pub id: ContributionId,
    /// The revision of the snapshot on screen — the fiber's last push that
    /// the ledger accepted.
    pub revision: u64,
    /// The frame that push was accepted at.
    pub accepted_at: u64,
    /// The frame the shell last told this contribution something happened,
    /// while the answering push is still outstanding. `None` when nothing is
    /// outstanding, which is the not-stale case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_since: Option<u64>,
    /// The frame this status describes.
    pub observed_at: u64,
    /// Frames since the snapshot was accepted: `observed_at - accepted_at`.
    ///
    /// Published rather than left to the reader because it is the number §8
    /// asks for by name. Saturating, so a status observed at a frame older
    /// than the snapshot reads 0 rather than wrapping to a huge age.
    pub age_frames: u64,
}

impl ContributionStatus {
    /// Whether a push is outstanding, which is what makes the snapshot on
    /// screen stale. See the module docs: not "old", *behind*.
    #[must_use]
    pub const fn stale(&self) -> bool {
        self.stale_since.is_some()
    }

    /// Frames the snapshot has been behind, or `None` when it is not.
    #[must_use]
    pub fn stale_for_frames(&self) -> Option<u64> {
        self.stale_since
            .map(|since| self.observed_at.saturating_sub(since))
    }

    /// `tree` keyed and flagged for the slot: the whole splice, in one call.
    ///
    /// Keyed by [`contribution_key`] so every node beneath it is attributable
    /// (`attribution`), and flagged with this status's staleness so the
    /// existing chain carries it to the projection. Doing the two together is
    /// deliberate: a splice that keyed the tree and forgot the flag would
    /// render a hung fiber's stale picture as a current one, and nothing
    /// downstream could tell.
    ///
    /// The flag is raised, never lowered. A fiber that declared its own root
    /// stale — its data is behind, whatever the shell thinks — keeps saying
    /// so, because the shell knows nothing about that claim and must not
    /// overwrite it.
    ///
    /// The root only. A contribution is one surface and one snapshot, so one
    /// node says how old it is; flagging every descendant would be the shell
    /// writing declarations onto nodes the plugin authored, and a reader with
    /// any node's id has the whole subtree's answer already — the id names
    /// the contribution.
    ///
    /// Call it **after** `crate::component::registry::expand`. Expanding a
    /// component-rooted tree returns what the constructor built, under the
    /// constructor's own key, so a tree mounted first loses the key it was
    /// mounted under.
    /// `a_contribution_is_mounted_after_its_components_are_expanded` pins it.
    #[must_use]
    pub fn mount(&self, mut tree: ViewNode) -> ViewNode {
        tree.key = contribution_key(self.id);
        tree.semantics.stale |= self.stale();
        tree
    }
}

/// What the shell retains between frames for each contribution it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    revision: u64,
    accepted_at: u64,
    stale_since: Option<u64>,
}

/// The retained snapshots, one per live contribution.
///
/// Owned by the shell: written as pushes arrive, read as the frame is built.
/// Nothing here calls a fiber, blocks, or reads a clock — every method is a
/// map operation over facts the caller already has — but nothing here locks
/// either. It is plain data with a `&mut` API, so a shell that takes pushes on
/// one thread and renders on another wraps it in its own lock; that choice
/// belongs to the shell, which knows where its frame boundary is.
///
/// Keyed by [`ContributionId`], so two contributions from one fiber are two
/// independent entries — one of them going stale says nothing about the other,
/// and one being retracted leaves the other alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContributionLedger {
    entries: BTreeMap<ContributionId, Entry>,
}

impl ContributionLedger {
    /// An empty ledger.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take a push: contribution `id` at `revision`, accepted at frame
    /// `frame_seq`.
    ///
    /// `revision` must be monotone per contribution — the daemon's registry
    /// revision, or any counter that only goes up while the contribution
    /// lives. It is compared, never interpreted.
    ///
    /// The first push of a contribution puts it in the ledger. A later push
    /// replaces the snapshot and answers any outstanding request. A push whose
    /// revision is not newer than the one held is [`PushOutcome::Superseded`]
    /// and changes nothing — an out-of-order arrival must not put an older
    /// picture back on screen, and must not report a hung fiber as answered.
    pub fn accept(&mut self, id: ContributionId, revision: u64, frame_seq: u64) -> PushOutcome {
        match self.entries.entry(id) {
            MapEntry::Vacant(slot) => {
                slot.insert(Entry {
                    revision,
                    accepted_at: frame_seq,
                    stale_since: None,
                });
                PushOutcome::Advanced
            }
            MapEntry::Occupied(mut slot) => {
                if revision <= slot.get().revision {
                    return PushOutcome::Superseded;
                }
                slot.insert(Entry {
                    revision,
                    accepted_at: frame_seq,
                    stale_since: None,
                });
                PushOutcome::Advanced
            }
        }
    }

    /// Record that the shell told contribution `id` something happened at
    /// frame `frame_seq` and has not had the answering push. The snapshot on
    /// screen is behind from this frame until [`Self::accept`] takes that
    /// push.
    ///
    /// Nothing blocks here and nothing blocks later. This is a note in a map,
    /// written by whichever thread routed the intent; the render thread reads
    /// it and carries on drawing what it already has.
    ///
    /// Idempotent while the request is outstanding: the frame recorded is the
    /// frame the contribution *first* fell behind, so an age keeps growing
    /// instead of resetting every time the shell asks again.
    ///
    /// Returns `false` for a contribution the ledger does not hold. There is
    /// nothing on screen to call behind, and inventing an entry would publish
    /// a status for a snapshot that does not exist.
    pub fn mark_behind(&mut self, id: ContributionId, frame_seq: u64) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else {
            return false;
        };
        entry.stale_since.get_or_insert(frame_seq);
        true
    }

    /// Drop contribution `id`. The ordinary unload path (§7) — a `failed`
    /// fiber's contribution reverts through this, like every other, with no
    /// special case here.
    ///
    /// Returns whether the ledger held it, so a caller can tell a real
    /// retraction from a repeat. Idempotent.
    pub fn retract(&mut self, id: ContributionId) -> bool {
        self.entries.remove(&id).is_some()
    }

    /// Contribution `id` as of frame `observed_at`, or `None` when the ledger
    /// holds no snapshot of it — it was never accepted, or it has been
    /// retracted.
    #[must_use]
    pub fn status(&self, id: ContributionId, observed_at: u64) -> Option<ContributionStatus> {
        self.entries
            .get(&id)
            .map(|entry| entry.status(id, observed_at))
    }

    /// Every retained contribution as of frame `observed_at`, in ascending id
    /// order — which is `contribute` order, since ids are minted in sequence.
    #[must_use]
    pub fn statuses(&self, observed_at: u64) -> Vec<ContributionStatus> {
        self.entries
            .iter()
            .map(|(id, entry)| entry.status(*id, observed_at))
            .collect()
    }

    /// The contribution that owns the node with this id, resolved against
    /// what the shell is actually holding.
    ///
    /// [`owner_of`] reads the key path alone and would answer for a forged or
    /// long-retracted `ui:<n>` key. This answers only for a contribution with
    /// a snapshot in the ledger, so a caller that has the ledger to hand gets
    /// the stricter answer for free.
    #[must_use]
    pub fn owner_of(&self, node_id: &str) -> Option<ContributionId> {
        owner_of(node_id).filter(|id| self.entries.contains_key(id))
    }

    /// How many contributions are retained.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Entry {
    fn status(&self, id: ContributionId, observed_at: u64) -> ContributionStatus {
        ContributionStatus {
            id,
            revision: self.revision,
            accepted_at: self.accepted_at,
            stale_since: self.stale_since,
            observed_at,
            age_frames: observed_at.saturating_sub(self.accepted_at),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ContributionLedger, PushOutcome};
    use crate::semantic::attribution::ContributionId;
    use crate::tree::{NodeKind, Semantics, ViewNode};

    const SEVEN: ContributionId = ContributionId::new(7);
    const NINE: ContributionId = ContributionId::new(9);

    /// A push that must land. Setup lines go through this, so no test can be
    /// set up by a push the ledger quietly dropped.
    fn push(ledger: &mut ContributionLedger, id: ContributionId, revision: u64, frame_seq: u64) {
        assert_eq!(
            ledger.accept(id, revision, frame_seq),
            PushOutcome::Advanced,
            "setup push for {id} at revision {revision}"
        );
    }

    #[test]
    fn a_pushed_snapshot_is_current_and_ages_by_the_frame() {
        let mut ledger = ContributionLedger::new();
        assert_eq!(ledger.accept(SEVEN, 1, 10), PushOutcome::Advanced);

        let at_push = ledger.status(SEVEN, 10).expect("a retained snapshot");
        assert_eq!(at_push.age_frames, 0);
        assert!(!at_push.stale(), "nothing is outstanding");
        assert_eq!(at_push.stale_for_frames(), None);

        let later = ledger.status(SEVEN, 34).expect("still retained");
        assert_eq!(later.age_frames, 24, "34 - 10");
        assert!(
            !later.stale(),
            "a contribution with nothing to say is current, not stale"
        );
    }

    /// The §8 case: the shell asked, the fiber never answered. The snapshot
    /// stays, its revision does not move, and its age keeps growing.
    #[test]
    fn a_fiber_that_never_answers_leaves_its_snapshot_behind() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 4, 10);
        assert!(ledger.mark_behind(SEVEN, 12));

        let status = ledger.status(SEVEN, 90).expect("the snapshot is retained");
        assert!(status.stale());
        assert_eq!(status.stale_since, Some(12));
        assert_eq!(status.stale_for_frames(), Some(78), "90 - 12");
        assert_eq!(status.age_frames, 80, "90 - 10");
        assert_eq!(status.revision, 4, "the last push, unchanged");
    }

    /// Asking again does not restart the clock, or a fiber hung under a
    /// stream of intents would read as freshly behind every frame.
    #[test]
    fn a_repeated_request_keeps_the_frame_the_snapshot_first_fell_behind() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        ledger.mark_behind(SEVEN, 5);
        ledger.mark_behind(SEVEN, 6);
        ledger.mark_behind(SEVEN, 7);
        assert_eq!(
            ledger
                .status(SEVEN, 8)
                .expect("retained")
                .stale_for_frames(),
            Some(3),
            "8 - 5, not 8 - 7"
        );
    }

    #[test]
    fn the_answering_push_clears_the_staleness_and_resets_the_age() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        ledger.mark_behind(SEVEN, 5);
        assert_eq!(ledger.accept(SEVEN, 2, 9), PushOutcome::Advanced);

        let status = ledger.status(SEVEN, 9).expect("retained");
        assert!(!status.stale());
        assert_eq!(status.stale_since, None);
        assert_eq!(status.age_frames, 0);
        assert_eq!(status.revision, 2);
    }

    /// An out-of-order arrival must not put an older picture back, and must
    /// not report a hung fiber as answered.
    #[test]
    fn a_push_no_newer_than_the_one_held_is_dropped() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 5, 1);
        ledger.mark_behind(SEVEN, 2);

        assert_eq!(ledger.accept(SEVEN, 4, 3), PushOutcome::Superseded);
        assert_eq!(ledger.accept(SEVEN, 5, 3), PushOutcome::Superseded);

        let status = ledger.status(SEVEN, 3).expect("retained");
        assert_eq!(status.revision, 5, "the newer snapshot survives");
        assert_eq!(status.accepted_at, 1);
        assert!(
            status.stale(),
            "still waiting on a push that is actually new"
        );
    }

    #[test]
    fn a_contribution_the_ledger_does_not_hold_cannot_fall_behind() {
        let mut ledger = ContributionLedger::new();
        assert!(!ledger.mark_behind(SEVEN, 1));
        assert!(ledger.is_empty(), "no entry was invented");
        assert_eq!(ledger.status(SEVEN, 1), None);
    }

    #[test]
    fn retracting_drops_the_snapshot_and_is_idempotent() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        assert!(ledger.retract(SEVEN));
        assert!(!ledger.retract(SEVEN), "a repeat retraction held nothing");
        assert_eq!(ledger.status(SEVEN, 2), None);
        assert!(ledger.statuses(2).is_empty());
    }

    /// Two surfaces from one fiber are two entries. One hanging, or being
    /// retracted, says nothing about the other.
    #[test]
    fn two_contributions_from_one_author_are_tracked_apart() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        push(&mut ledger, NINE, 1, 1);
        ledger.mark_behind(SEVEN, 2);

        assert!(ledger.status(SEVEN, 3).expect("retained").stale());
        assert!(!ledger.status(NINE, 3).expect("retained").stale());

        ledger.retract(SEVEN);
        assert_eq!(ledger.status(SEVEN, 3), None);
        assert!(
            ledger.status(NINE, 3).is_some(),
            "one plugin's retraction must not take another's surface down"
        );
    }

    /// Ascending id, which is `contribute` order. A published list must not
    /// depend on which order pushes happened to arrive in.
    #[test]
    fn statuses_are_ordered_by_contribution_id() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, NINE, 1, 1);
        push(&mut ledger, SEVEN, 1, 1);
        push(&mut ledger, ContributionId::new(8), 1, 1);
        let ids: Vec<u64> = ledger
            .statuses(1)
            .iter()
            .map(|status| status.id.get())
            .collect();
        assert_eq!(ids, [7, 8, 9]);
    }

    /// A status observed at a frame older than its snapshot — a caller
    /// quoting a frame that has already gone by — reads 0, never a wrapped
    /// age of eighteen quintillion frames.
    #[test]
    fn an_age_never_wraps() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 100);
        ledger.mark_behind(SEVEN, 100);
        let status = ledger.status(SEVEN, 40).expect("retained");
        assert_eq!(status.age_frames, 0);
        assert_eq!(status.stale_for_frames(), Some(0));
    }

    #[test]
    fn mounting_keys_the_tree_and_carries_the_staleness() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);

        let fresh = ledger
            .status(SEVEN, 1)
            .expect("retained")
            .mount(ViewNode::new(NodeKind::Stack, "the-plugins-own-key"));
        assert_eq!(fresh.key.as_str(), "ui:7");
        assert!(!fresh.semantics.stale);

        ledger.mark_behind(SEVEN, 2);
        let behind = ledger
            .status(SEVEN, 3)
            .expect("retained")
            .mount(ViewNode::new(NodeKind::Stack, "the-plugins-own-key"));
        assert_eq!(behind.key.as_str(), "ui:7");
        assert!(
            behind.semantics.stale,
            "the shell's own fact reaches the tree"
        );
    }

    /// The shell can raise the flag. It cannot lower one the author raised:
    /// a fiber saying "my data is behind" knows something the shell does not.
    #[test]
    fn mounting_never_clears_a_staleness_the_author_declared() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        let declared = ViewNode::new(NodeKind::Stack, "own").with_semantics(Semantics {
            stale: true,
            ..Semantics::default()
        });
        let mounted = ledger.status(SEVEN, 1).expect("retained").mount(declared);
        assert!(mounted.semantics.stale);
    }

    #[test]
    fn an_owner_is_resolved_against_what_the_shell_holds() {
        let mut ledger = ContributionLedger::new();
        push(&mut ledger, SEVEN, 1, 1);
        assert_eq!(ledger.owner_of("/shell/bar/ui:7/row"), Some(SEVEN));
        assert_eq!(
            ledger.owner_of("/shell/bar/ui:9/row"),
            None,
            "a key naming a contribution the shell does not hold owns nothing"
        );
        assert_eq!(ledger.owner_of("/shell/bar/clock"), None);
    }
}
