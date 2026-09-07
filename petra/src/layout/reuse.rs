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
//! behind `debug_assertions`, [`ReuseState::verify_declaration`] walks the
//! previous and current trees and panics naming the first node whose `Arc`
//! moved without being covered by the declared change set.
//!
//! **What that verifier does not catch.** It is an `Arc`-identity check, so
//! it can only ever see a node the host silently rebuilt — a new allocation
//! in place of the old one. A host that mutates the store behind a
//! `collection`'s `RowSource` and reports `ChangeSet::None` (or omits the
//! collection's id) leaves the tree genuinely unchanged: no `Arc` diverges
//! anywhere, so this walk sees nothing wrong and the rows go stale in
//! silence. That case is the host obligation this note's §2 item 5 states
//! directly, not something an `Arc`-comparison walk can discharge — see
//! `.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::frame::placement::{PaintContent, Placement, SubtreeCopy};
use crate::input::TextSelection;
use crate::layout::{ChangeSet, LayoutState, Slot};
use crate::tree::{Key, KeyPath, ViewNode};

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
    /// 4. Both ends of a hover, press, or capture move, for exactly the same
    ///    reason (`contracts/interaction-state.md` §9): each is a
    ///    per-placement flag and a different token family, so a subtree
    ///    carried over from the previous frame would keep painting the state
    ///    it was in when it was built. Focus got this treatment from the
    ///    start and these three did not, which was latent only for as long as
    ///    nothing ever set them.
    /// 5. The node at either end of a text-selection change. Unlike the four
    ///    above, this one moves *within* a node: the id stays put for the
    ///    whole body of a drag while the byte pair grows, so the id-pair test
    ///    would call every frame of a selection unchanged and reuse the
    ///    subtree that has the old highlight painted into it.
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

        // Both ends of every interaction-state move. Written as a table
        // rather than as four copies of the same `if`, so a sixth flag is one
        // row and cannot be added with one end of the move forgotten.
        //
        // `capture` is compared by the id it names rather than by the whole
        // `Capture`, which also carries `last` — the pointer's current
        // position, which moves on every single event of a drag. Comparing
        // the struct would mark the holder dirty on every frame of a gesture
        // and defeat reuse for as long as the drag lasted, while telling the
        // truth about nothing: the placement flag projects the node, not the
        // path the pointer took to it.
        let moves: [(Option<&String>, Option<&String>); 4] = [
            (self.state.focused.as_ref(), now.focused.as_ref()),
            (self.state.hovered.as_ref(), now.hovered.as_ref()),
            (self.state.pressed.as_ref(), now.pressed.as_ref()),
            (
                self.state.capture.as_ref().map(|c| &c.node),
                now.capture.as_ref().map(|c| &c.node),
            ),
        ];
        for (before, after) in moves {
            if before == after {
                continue;
            }
            if let Some(before) = before {
                dirty.insert(before.clone());
            }
            if let Some(after) = after {
                dirty.insert(after.clone());
            }
        }

        // The text selection, which the table above cannot express in either
        // of the two ways it differs. The other four move *between* nodes, so
        // both ends of the move are named ids; a selection moves **inside** a
        // node for the whole body of a drag, with the ids unchanged and only
        // the byte offsets growing, and it covers every run *between* its two
        // ends as well as the two themselves.
        //
        // So both spans are marked whole: the one this frame painted and the
        // one the next frame will. Dirtying only the four endpoint ids would
        // carry over the middle of a selection that had just been dragged
        // across it — a page of running text with the highlight painted into
        // the frame it was built in and no way to repaint it.
        if self.state.text_selection != now.text_selection {
            for selection in [
                self.state.text_selection.as_ref(),
                now.text_selection.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                self.span_ids(selection, &mut dirty);
            }
        }

        anchored_surfaces(&self.tree, &mut KeyPath::root(), &mut dirty);

        Some(dirty)
    }

    /// Every id `selection` reaches in the frame this memo holds, added to
    /// `into`.
    ///
    /// Measured against the memo's own placements, which is the only frame
    /// this type has. For the outgoing selection that is exactly right — it is
    /// the frame the highlight was painted into. For the incoming one it is an
    /// approximation of a frame not built yet, and the approximation errs
    /// towards re-placing a subtree that did not need it, which costs time and
    /// cannot produce a wrong picture.
    ///
    /// Both ends missing from this frame means the selection was made
    /// somewhere this memo never saw; there is nothing here to mark.
    fn span_ids(&self, selection: &TextSelection, into: &mut BTreeSet<String>) {
        let index_of = |node: &str| self.placements.iter().position(|p| p.id == node);
        let (Some(a), Some(b)) = (
            index_of(&selection.anchor.node),
            index_of(&selection.focus.node),
        ) else {
            into.insert(selection.anchor.node.clone());
            into.insert(selection.focus.node.clone());
            return;
        };
        for placement in &self.placements[a.min(b)..=a.max(b)] {
            into.insert(placement.id.clone());
        }
    }
}

/// Every `surface` in `node`'s subtree whose anchor names another node, by
/// canonical id, added to `dirty`.
///
/// `contracts/anchored-placement.md` §6: an anchored surface is
/// *unconditionally* dirty on the incremental path. Nothing cheaper is
/// sound. The reuse test is pointer identity plus slot equality, and both can
/// hold across a frame in which the anchor moved — a scroll under the anchor
/// changes neither the surface's `Arc` nor the slot its parent offers it,
/// while moving the rect the surface is placed against. Reusing there paints
/// a popover beside where its button used to be, under a digest that says the
/// frame is current.
///
/// It walks the *previous* tree, which is the one this memo owns and the only
/// one a reuse decision can be made against: a surface the new tree grew has
/// no counterpart to carry over in the first place.
///
/// The walk is O(N) once per incremental frame. That is the cost
/// `contracts/anchored-placement.md`'s "Open" paragraph leaves to
/// implementation, and it is the same order the harvest walk's own target
/// collection already pays; a tree with no `surface` in it touches no
/// allocation here at all.
fn anchored_surfaces(node: &ViewNode, path: &mut KeyPath, dirty: &mut BTreeSet<String>) {
    path.push(node.key.clone());
    if node
        .props
        .anchor
        .as_ref()
        .is_some_and(crate::tree::Anchor::names_node)
    {
        dirty.insert(path.id());
    }
    for child in &node.children {
        anchored_surfaces(child, path, dirty);
    }
    path.pop();
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
    /// Conditions are checked cheapest first: an address comparison, then a
    /// slot comparison, then the bounds every array [`ReuseState::subtree`]
    /// slices must satisfy. `FrameMemo` is `pub` with every field `pub`, so a
    /// caller from outside this crate can build one whose arrays disagree in
    /// length; the four checks below are what stop such a memo from reaching
    /// an out-of-bounds slice in `subtree` rather than being refused here, by
    /// name, at the point that would have sliced out of range.
    #[must_use]
    pub fn reusable(
        &self,
        node: &ViewNode,
        old_node: &ViewNode,
        old_index: usize,
        slot: Slot,
    ) -> bool {
        if !std::ptr::eq(node, old_node) || self.memo.slots.get(old_index) != Some(&slot) {
            return false;
        }
        let end = old_index + memo_len(&self.memo.subtree_len, old_index);
        end <= self.memo.placements.len()
            && end <= self.memo.content.len()
            && end <= self.memo.subtree_len.len()
            && end <= self.memo.subtree_hashes.len()
            && end <= self.memo.slots.len()
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

    /// Behind `debug_assertions`: walk the previous tree ([`FrameMemo::tree`])
    /// and `tree`, the one this frame was just placed from, together — and
    /// panic naming the first node whose `Arc` moved without being covered by
    /// the declared change set.
    ///
    /// Call after placing a frame from a `Nodes`- or `None`-derived dirty
    /// set (never from `ChangeSet::All`, which has no dirty set to check
    /// against — see [`FrameMemo::dirty_ids`]). The walk is O(N) in the new
    /// tree: exactly the cost the release path refuses to pay, which is why
    /// this only exists behind `debug_assertions`.
    ///
    /// "Covered" is not plain set membership, because the tree this crate
    /// works with is immutable behind `Arc`: rebuilding any one node forces
    /// a fresh `Arc` for every ancestor up to the root too (there is no way
    /// to change what a `Vec<Arc<ViewNode>>` holds without a new `Vec`, and
    /// so a new `ViewNode`, and so a new `Arc`). A host that declares one
    /// leaf therefore has every ancestor of that leaf differ honestly, and
    /// declaring a container is naturally read as covering its whole
    /// subtree, the way `a_subtree_that_grows_still_produces_the_full_frame`
    /// in `tests/incremental_frames.rs` declares only the container. So a
    /// node counts as covered when it is an ancestor of a declared id
    /// ([`dirty_at_or_under`], the same predicate the placement walk uses to
    /// decide whether to descend) or a descendant of one. What it does not
    /// cover is a sibling of a declared id whose `Arc` moved anyway — that is
    /// exactly what the under-declaring test in `tests/incremental_frames.rs`
    /// proves this walk catches.
    ///
    /// See the module doc for what this check does **not** prove.
    #[cfg(debug_assertions)]
    pub fn verify_declaration(&self, tree: &ViewNode) {
        let mut path = KeyPath::root();
        let old = (self.memo.tree.key == tree.key).then(|| self.memo.tree.as_ref());
        Self::verify_node(old, tree, &mut path, self.dirty, false);
    }

    /// One level of [`ReuseState::verify_declaration`]'s walk.
    ///
    /// `declared_ancestor` is `true` once any ancestor on this path was
    /// itself an exact member of `dirty` — inherited downward so a
    /// declared container's whole subtree is covered without every node in
    /// it needing its own entry.
    #[cfg(debug_assertions)]
    fn verify_node(
        old: Option<&ViewNode>,
        new: &ViewNode,
        path: &mut KeyPath,
        dirty: &BTreeSet<String>,
        declared_ancestor: bool,
    ) {
        path.push(new.key.clone());
        let id = path.id();
        let named = dirty.contains(&id);
        if let Some(old) = old
            && old.key == new.key
            && !std::ptr::eq(old, new)
            && !declared_ancestor
            && !named
            && !dirty_at_or_under(dirty, &id)
        {
            panic!(
                "petrify_with_memo: {id} was rebuilt between frames (its Arc \
                 moved) but the change set does not cover it — name it, name \
                 an ancestor of it, or name a descendant of it, in \
                 ChangeSet::Nodes"
            );
        }
        let child_declared = declared_ancestor || named;
        for child in &new.children {
            // `old`, whenever `Some`, is already the counterpart of `new`
            // (guaranteed by this same `find` one level up, or by the
            // top-level key check in `verify_declaration`), so finding
            // `child`'s counterpart among `old`'s children is a plain
            // key lookup with no re-check needed.
            let old_child = old.and_then(|o| o.children.iter().find(|c| c.key == child.key));
            Self::verify_node(
                old_child.map(Arc::as_ref),
                child,
                path,
                dirty,
                child_declared,
            );
        }
        path.pop();
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
/// Falling back to `1` rather than indexing keeps this function itself from
/// panicking on a malformed memo. [`ReuseState::reusable`] is what actually
/// keeps a memo whose arrays disagree from reaching [`ReuseState::subtree`]:
/// it checks `old_index + len` against the length of every array `subtree`
/// slices — `placements`, `content`, `subtree_len`, `subtree_hashes`, and
/// `slots` — not only `placements.len()`, so a memo assembled with
/// mismatched array lengths fails `reusable` rather than panicking on an
/// out-of-bounds slice.
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
