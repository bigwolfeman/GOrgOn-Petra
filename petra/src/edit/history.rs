//! Undo and redo, with VS Code's grouping (spec 006 FR-014).
//!
//! History is a stack of groups. A group is one or more transactions that
//! one undo reverses, with the selections before the first and after the
//! last. Typing merges into the open group while the caller says the edit is
//! mergeable, the kind matches, the caret has not moved since, and the pause
//! is shorter than the merge window; anything else opens a new group.
//!
//! # The saved point
//!
//! Each group has an id; the state of the text is named by the id of the
//! group on top of the undo stack (0 for none). Saving records that id, so
//! "dirty" is "the current id is not the saved id", and undoing back to the
//! saved state makes the document clean again, as in VS Code. A group whose
//! id is the saved id never takes a merge, or typing right after a save would
//! change the text without changing the id.

use super::buffer::TextBuffer;
use super::selection::Selections;
use super::transaction::{EditRecord, Transaction};

/// What kind of edit a group holds, for the merge rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditKind {
    /// Typed text.
    Typing,
    /// Backspace or Delete of single characters.
    Deleting,
    /// Anything else: paste, cut, indent, replace. Never merges.
    Other,
}

#[derive(Clone, Debug)]
struct Group {
    id: u64,
    kind: EditKind,
    /// Forward transactions, in order.
    forward: Vec<Transaction>,
    /// Their inverses, in the same order.
    inverse: Vec<Transaction>,
    before: Selections,
    after: Selections,
    at_ms: u64,
}

/// The undo and redo stacks of one document.
#[derive(Clone, Debug)]
pub struct History {
    undo: Vec<Group>,
    redo: Vec<Group>,
    next_id: u64,
    /// The state id at the last save or load; `None` when that state can no
    /// longer be reached (its group was dropped from the redo stack).
    saved: Option<u64>,
    /// Set by any caret move or explicit break; the next edit opens a group.
    broken: bool,
    merge_ms: u64,
}

impl Default for History {
    fn default() -> Self {
        Self::new(1000)
    }
}

/// The most groups kept. Older ones fall off the bottom.
const MAX_GROUPS: usize = 10_000;

impl History {
    /// An empty history, clean, merging typing within `merge_ms`.
    #[must_use]
    pub fn new(merge_ms: u64) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 1,
            saved: Some(0),
            broken: false,
            merge_ms,
        }
    }

    /// Change the merge window.
    pub fn set_merge_ms(&mut self, merge_ms: u64) {
        self.merge_ms = merge_ms;
    }

    /// The id naming the current state.
    #[must_use]
    pub fn state_id(&self) -> u64 {
        self.undo.last().map_or(0, |g| g.id)
    }

    /// Mark the current state as the saved one.
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.state_id());
    }

    /// Whether the current state is the saved one.
    #[must_use]
    pub fn is_saved(&self) -> bool {
        self.saved == Some(self.state_id())
    }

    /// Close the open group: the next edit starts a new one.
    pub fn break_group(&mut self) {
        self.broken = true;
    }

    /// Whether there is anything to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is anything to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Record an applied transaction (`forward`, with its `inverse`) that
    /// moved the selections from `before` to `after`.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        forward: Transaction,
        inverse: Transaction,
        before: &Selections,
        after: &Selections,
        kind: EditKind,
        mergeable: bool,
        now_ms: u64,
    ) {
        if forward.is_empty() {
            return;
        }
        if let Some(saved) = self.saved
            && self.redo.iter().any(|g| g.id == saved)
        {
            self.saved = None;
        }
        self.redo.clear();
        let merge = !self.broken
            && mergeable
            && kind != EditKind::Other
            && self.undo.last().is_some_and(|top| {
                top.kind == kind
                    && Some(top.id) != self.saved
                    && now_ms.saturating_sub(top.at_ms) < self.merge_ms
                    && top.after == *before
            });
        self.broken = false;
        if merge && let Some(top) = self.undo.last_mut() {
            top.forward.push(forward);
            top.inverse.push(inverse);
            top.after = after.clone();
            top.at_ms = now_ms;
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.undo.push(Group {
            id,
            kind,
            forward: vec![forward],
            inverse: vec![inverse],
            before: before.clone(),
            after: after.clone(),
            at_ms: now_ms,
        });
        if self.undo.len() > MAX_GROUPS {
            let dropped = self.undo.remove(0);
            if self.saved == Some(0) || self.saved == Some(dropped.id) {
                // The saved state is now below the bottom of the stack.
                self.saved = None;
            }
        }
    }

    /// Undo the top group on `buffer`. Returns the selections to restore.
    pub fn undo(&mut self, buffer: &mut TextBuffer, records: &mut Vec<EditRecord>) -> Option<Selections> {
        let group = self.undo.pop()?;
        for t in group.inverse.iter().rev() {
            t.apply(buffer, records);
        }
        let restore = group.before.clone();
        self.redo.push(group);
        self.broken = true;
        Some(restore)
    }

    /// Redo the last undone group on `buffer`. Returns the selections to
    /// restore.
    pub fn redo(&mut self, buffer: &mut TextBuffer, records: &mut Vec<EditRecord>) -> Option<Selections> {
        let group = self.redo.pop()?;
        for t in &group.forward {
            t.apply(buffer, records);
        }
        let restore = group.after.clone();
        self.undo.push(group);
        self.broken = true;
        Some(restore)
    }
}
