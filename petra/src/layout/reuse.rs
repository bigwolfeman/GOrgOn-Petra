//! Subtree reuse: what an incremental frame may keep, and how it proves it.
//!
//! The design this implements is
//! `.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`.
//! The short version: a subtree may be kept from the previous frame when the
//! host did not declare a change under it, the state it reads has not moved,
//! and it is the *same allocation* the previous frame placed. Declaration
//! finds the change; pointer identity proves the rest did not move.
//!
//! # Why pointer identity is sound here
//!
//! [`FrameMemo`] owns `tree: Arc<ViewNode>`, and it owns it for exactly as
//! long as the placements derived from it. So no node in the previous tree can
//! be dropped and a new node allocated at its address while a comparison is
//! running, which is the ABA bug this scheme would otherwise be. That field is
//! load-bearing: making it a `Weak` would turn every comparison into a race
//! with the allocator.
//!
//! The comparison itself is `std::ptr::eq` on `&ViewNode`, not
//! `Arc::ptr_eq`. They are the same test — an `Arc`'s referent has a stable
//! address for the `Arc`'s whole life — and taking it on the reference means
//! [`crate::layout::place`] keeps its `&ViewNode` signature and its several
//! hundred call sites.
//!
//! # What a wrong answer costs
//!
//! Both directions are not equal. Refusing to reuse a subtree that was in
//! fact unchanged costs time and nothing else. Reusing one that changed
//! renders a stale picture under a digest that says it is current, which no
//! test in this crate would catch, because the engine would be agreeing with
//! itself. Every condition below is therefore written to fail closed, and
//! [`ReuseState::verify_declaration`] exists to catch a host that
//! under-declares before that host reaches an operator.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::frame::placement::{PaintContent, Placement, SubtreeCopy};
use crate::layout::{ChangeSet, LayoutState, Slot};
use crate::tree::{Key, ViewNode};

/// Everything the previous frame left behind for the next one to compare
/// against.
///
/// Built by [`crate::frame::petrify_with_memo`] and consumed by the frame
/// after it. It owns its arrays rather than borrowing them because the host
/// keeps exactly one of these between frames.
#[derive(Clone, Debug)]
pub struct FrameMemo {
    /// The tree the previous frame was placed from. Load-bearing: see the
    /// module doc.
    pub tree: Arc<ViewNode>,
    /// The previous frame's placements, in tree pre-order.
    pub placements: Vec<Placement>,
    /// Their paint payloads, at the same index.
    pub content: Vec<PaintContent>,
    /// Their subtree extents, at the same index.
    pub subtree_len: Vec<usize>,
    /// Their Merkle subtree hashes, at the same index.
    pub subtree_hashes: Vec<[u8; 32]>,
    /// The slot each was offered, at the same index.
    pub slots: Vec<Slot>,
    /// The state the previous frame read. Diffed against this frame's, so the
    /// host is never asked about scroll or focus.
    pub state: LayoutState,
    /// The theme snapshot revision the previous frame was placed under.
    pub theme_rev: u64,
    /// The display scale the previous frame was placed under.
    pub scale: crate::geom::Scale,
}

impl FrameMemo {
    /// Take a finished frame apart into the memo the next one compares
    /// against.
    ///
    /// Consumes the frame rather than copying it. A memo built by cloning
    /// would allocate once per node per frame, which is the O(N) this whole
    /// design exists to avoid — the caller must be finished painting and
    /// hit-testing the frame before the next negotiation begins, which it is.
    ///
    /// `theme_rev` and `scale` must be the ones the *context* carried, not the
    /// ones on `frame.viewport`. They are supposed to be the same numbers and
    /// in a real host they are, but the engine reads them from two places —
    /// `PaintState::token_revision` is written from `LayoutCtx::theme_rev`
    /// while the digest hashes `Viewport::theme_rev` — and the two disagree in
    /// this crate's own test fixtures. The memo has to hold whichever one
    /// decides the placements, because that is the one whose change makes a
    /// carried-over subtree stale.
    #[must_use]
    pub fn adopt(
        tree: Arc<ViewNode>,
        frame: crate::frame::PetrifiedFrame,
        state: LayoutState,
        theme_rev: u64,
        scale: crate::geom::Scale,
    ) -> Self {
        Self {
            tree,
            theme_rev,
            scale,
            placements: frame.placements,
            content: frame.content,
            subtree_len: frame.subtree_len,
            subtree_hashes: frame.subtree_hashes,
            slots: frame.slots,
            state,
        }
    }

    /// The set of ids whose subtrees this frame must re-place, given what the
    /// host declared and what the engine can see for itself.
    ///
    /// Three sources, and only the first is the host's word:
    ///
    /// 1. The ids in [`ChangeSet::Nodes`].
    /// 2. Every scroll id whose offset differs between the two states. A
    ///    scroll offset decides which rows a `collection` materializes, so a
    ///    moved offset is a changed subtree even though the tree is identical.
    /// 3. The previously focused id and the newly focused one, when they
    ///    differ. Focus is a per-placement flag and a painted ring, so both
    ///    ends of a focus move are dirty.
    ///
    /// Returns `None` when the change set is [`ChangeSet::All`], which is the
    /// host saying it does not know: there is no dirty *set* in that case,
    /// only a dirty everything.
    #[must_use]
    pub fn dirty_ids(&self, changes: &ChangeSet, now: &LayoutState) -> Option<BTreeSet<String>> {
        let mut dirty = match changes {
            ChangeSet::All => return None,
            ChangeSet::None => BTreeSet::new(),
            ChangeSet::Nodes(ids) => ids.clone(),
        };

        // A scroll offset that appears, disappears, or moves. Compared by
        // `LayoutState::scroll_offset` rather than by raw map equality, so a
        // stored NaN and a stored negative both compare as the zero they are
        // actually read as, and neither counts as a change.
        let ids = self
            .state
            .scroll_offsets
            .keys()
            .chain(now.scroll_offsets.keys());
        for id in ids {
            if self.state.scroll_offset(id) != now.scroll_offset(id) {
                dirty.insert(id.clone());
            }
        }

        if self.state.focused != now.focused {
            if let Some(before) = &self.state.focused {
                dirty.insert(before.clone());
            }
            if let Some(after) = &now.focused {
                dirty.insert(after.clone());
            }
        }

        Some(dirty)
    }
}

/// Whether any dirty id lies at `path` or anywhere under it.
///
/// Ids are canonical key paths, so "under" is a prefix test — and unlike the
/// id-prefix guess deleted from `scroll.rs`, this one is sound by
/// construction: the id *is* the path, so a prefix relationship is an
/// ancestry relationship and nothing else. The boundary check matters: `/ab`
/// must not count as a descendant of `/a`, so a match requires the next byte
/// to be the separator.
#[must_use]
pub fn dirty_at_or_under(dirty: &BTreeSet<String>, path: &str) -> bool {
    // Everything at or under `path` sorts contiguously from `path` itself, so
    // one range scan settles it without touching the rest of the set.
    for candidate in dirty.range(path.to_owned()..) {
        if candidate == path {
            return true;
        }
        let Some(rest) = candidate.strip_prefix(path) else {
            // Past the prefix run: nothing further can start with `path`.
            return false;
        };
        if rest.starts_with('/') {
            return true;
        }
        // A sibling like `/ab` when `path` is `/a`. Keep scanning: the run of
        // strings starting with `path` is contiguous, but `/ab` sorts inside
        // it and is not a descendant.
    }
    false
}

/// What one incremental pass carried over and what it rebuilt.
///
/// Reported rather than inferred: "the digest did not move" proves nothing
/// about whether reuse happened, because a correct full negotiation produces
/// the same digest. A test that means to check reuse has to read these.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReuseStats {
    /// Subtrees carried over whole.
    pub reused_subtrees: usize,
    /// Placements those subtrees covered.
    pub reused_nodes: usize,
    /// Placements this pass built itself.
    pub replaced_nodes: usize,
}

/// The walk's cursor into the previous frame.
///
/// Lives in [`crate::layout::LayoutCtx`] for the duration of one incremental
/// pass. The dispatcher maintains it, so no container knows it exists — which
/// is deliberate: twelve node kinds each remembering to advance a cursor is
/// twelve chances to advance it wrong, and a wrong cursor pairs a new node
/// with someone else's old placements.
pub struct ReuseState<'a> {
    memo: &'a FrameMemo,
    dirty: &'a BTreeSet<String>,
    stack: Vec<OldFrame<'a>>,
    /// How many subtrees were carried over whole.
    pub reused_subtrees: usize,
    /// How many placements those subtrees covered.
    pub reused_nodes: usize,
    /// How many placements this pass built itself.
    pub replaced_nodes: usize,
}

/// One open ancestor, and where its counterpart sat in the previous frame.
struct OldFrame<'a> {
    /// The previous frame's node at this position, or `None` when the new
    /// tree has a node the old one did not.
    node: Option<&'a ViewNode>,
    /// That node's placement index in the previous frame.
    index: usize,
    /// Placement index of each of `node`'s children, in order. Built on first
    /// use because a frame whose children are never resolved — a leaf, or a
    /// container that was itself reused — should not pay for it.
    child_index: Option<Vec<usize>>,
}

impl<'a> ReuseState<'a> {
    /// A cursor positioned above the root, over `memo`, with `dirty` naming
    /// the subtrees that must be re-placed.
    #[must_use]
    pub fn new(memo: &'a FrameMemo, dirty: &'a BTreeSet<String>) -> Self {
        Self {
            memo,
            dirty,
            stack: Vec::new(),
            reused_subtrees: 0,
            reused_nodes: 0,
            replaced_nodes: 0,
        }
    }

    /// The memo this cursor reads.
    #[must_use]
    pub fn memo(&self) -> &'a FrameMemo {
        self.memo
    }

    /// The previous frame's counterpart for the child named `key` at the
    /// current position, if it had one.
    ///
    /// At the top level the counterpart is the previous root, placement `0`.
    /// Below it, the counterpart is found among the open ancestor's children
    /// **by key**, never by position: a child inserted or removed in the
    /// middle would otherwise pair every later sibling with the wrong old
    /// placement, and every one of those pairings would pass the slot check
    /// often enough to ship a scrambled frame.
    pub fn counterpart(&mut self, key: &Key) -> Option<(&'a ViewNode, usize)> {
        let Some(frame) = self.stack.last_mut() else {
            let root = &*self.memo.tree;
            return (root.key == *key && !self.memo.placements.is_empty()).then_some((root, 0));
        };
        let parent = frame.node?;
        let position = parent.children.iter().position(|c| c.key == *key)?;
        let index = frame.child_index.get_or_insert_with(|| {
            // Children occupy consecutive ranges starting one past the
            // parent, each as long as its own subtree.
            let mut at = frame.index + 1;
            parent
                .children
                .iter()
                .map(|_| {
                    let start = at;
                    at += memo_len(&self.memo.subtree_len, start);
                    start
                })
                .collect()
        });
        let at = *index.get(position)?;
        Some((&*parent.children[position], at))
    }

    /// Whether the subtree at `old_index` may be carried over whole.
    ///
    /// All three conditions must hold, and each is cheap before the one after
    /// it: an address comparison, then a slot comparison, then a range scan of
    /// the dirty set. `path_id` is built by the caller only if the first two
    /// pass, because building it allocates.
    #[must_use]
    pub fn reusable(
        &self,
        node: &ViewNode,
        old_node: &ViewNode,
        old_index: usize,
        slot: Slot,
    ) -> bool {
        std::ptr::eq(node, old_node)
            && self.memo.slots.get(old_index) == Some(&slot)
            && old_index + memo_len(&self.memo.subtree_len, old_index) <= self.memo.placements.len()
    }

    /// The subtree at `old_index`, as the sink wants it.
    #[must_use]
    pub fn subtree(&self, old_index: usize) -> SubtreeCopy<'a> {
        let len = memo_len(&self.memo.subtree_len, old_index);
        let range = old_index..old_index + len;
        SubtreeCopy {
            placements: &self.memo.placements[range.clone()],
            content: &self.memo.content[range.clone()],
            subtree_len: &self.memo.subtree_len[range.clone()],
            slots: &self.memo.slots[range.clone()],
            hashes: &self.memo.subtree_hashes[range],
            base: old_index,
        }
    }

    /// Whether anything at or under `path_id` was declared or derived dirty.
    #[must_use]
    pub fn dirty(&self, path_id: &str) -> bool {
        dirty_at_or_under(self.dirty, path_id)
    }

    /// Record that a subtree of `len` placements was carried over.
    pub fn note_reuse(&mut self, len: usize) {
        self.reused_subtrees += 1;
        self.reused_nodes += len;
    }

    /// Record that one placement was built rather than carried over.
    pub fn note_replaced(&mut self) {
        self.replaced_nodes += 1;
    }

    /// What this pass carried over and what it rebuilt.
    #[must_use]
    pub fn stats(&self) -> ReuseStats {
        ReuseStats {
            reused_subtrees: self.reused_subtrees,
            reused_nodes: self.reused_nodes,
            replaced_nodes: self.replaced_nodes,
        }
    }

    /// Descend into a node that is being re-placed, remembering its
    /// counterpart so its children can find theirs.
    pub fn enter(&mut self, counterpart: Option<(&'a ViewNode, usize)>) {
        self.stack.push(match counterpart {
            Some((node, index)) => OldFrame {
                node: Some(node),
                index,
                child_index: None,
            },
            None => OldFrame {
                node: None,
                index: 0,
                child_index: None,
            },
        });
    }

    /// Leave the node most recently entered.
    pub fn leave(&mut self) {
        self.stack.pop();
    }
}

/// The subtree length recorded for `index`, or `1` if the memo is too short.
///
/// Falling back to `1` rather than indexing keeps a malformed memo from
/// panicking deep inside a walk; the reuse test above rejects the subtree
/// anyway, because a memo whose arrays disagree cannot satisfy the bounds
/// check in [`ReuseState::reusable`].
fn memo_len(subtree_len: &[usize], index: usize) -> usize {
    subtree_len.get(index).copied().unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::dirty_at_or_under;

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn a_node_is_dirty_when_it_is_named() {
        assert!(dirty_at_or_under(&set(&["/root/a"]), "/root/a"));
    }

    #[test]
    fn a_node_is_dirty_when_a_descendant_is_named() {
        assert!(dirty_at_or_under(&set(&["/root/a/b/c"]), "/root/a"));
        assert!(dirty_at_or_under(&set(&["/root/a/b/c"]), "/root"));
    }

    /// The whole point of reuse: an untouched sibling is not dirty.
    #[test]
    fn a_sibling_of_a_dirty_node_is_clean() {
        assert!(!dirty_at_or_under(&set(&["/root/a"]), "/root/b"));
    }

    /// `/ab` is not a descendant of `/a`, and it sorts between `/a` and
    /// `/a/x`, so a scan that stopped at the first non-descendant would miss
    /// a real one behind it.
    #[test]
    fn a_name_that_merely_starts_the_same_is_not_a_descendant() {
        assert!(!dirty_at_or_under(&set(&["/ab"]), "/a"));
        assert!(dirty_at_or_under(&set(&["/ab", "/a/x"]), "/a"));
    }

    #[test]
    fn an_ancestor_being_dirty_does_not_dirty_the_child() {
        // `/root` naming itself means *its own* placement changed. Its
        // children are re-placed because the walk reaches them, not because
        // they are in the set.
        assert!(!dirty_at_or_under(&set(&["/root"]), "/root/a"));
    }

    #[test]
    fn nothing_is_dirty_in_an_empty_set() {
        assert!(!dirty_at_or_under(&set(&[]), "/root"));
    }
}
