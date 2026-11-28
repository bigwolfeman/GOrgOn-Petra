//! Transactions: the one way the buffer changes (spec 006 FR-004).
//!
//! A [`Transaction`] is a set of non-overlapping replacements, each stated in
//! the coordinates of the text before the transaction. Applying it walks the
//! changes from the end of the text to the start, so no change moves the
//! offsets of one still to come, and returns the inverse transaction, which
//! is what undo applies.
//!
//! Every applied change also leaves an [`EditRecord`]: the byte and
//! line/column extents of the change in the coordinates current when it was
//! applied. That is exactly the shape a tree-sitter `InputEdit` needs, so a
//! syntax tree, a find model, or a diff can follow the buffer without being
//! told what kind of edit happened.

use std::ops::Range;

use super::buffer::{TextBuffer, count_newlines};

/// Replace `range` (in pre-transaction bytes) with `text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The bytes replaced, in the text before the transaction.
    pub range: Range<usize>,
    /// The text put in their place.
    pub text: String,
}

/// One applied change, in the coordinates of the moment it applied. Points
/// are `(line, byte column)`, both 0-based.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditRecord {
    /// First changed byte.
    pub start_byte: usize,
    /// End of the replaced bytes, before the change.
    pub old_end_byte: usize,
    /// End of the inserted bytes, after the change.
    pub new_end_byte: usize,
    /// Point of `start_byte`.
    pub start_point: (usize, usize),
    /// Point of `old_end_byte`, before the change.
    pub old_end_point: (usize, usize),
    /// Point of `new_end_byte`, after the change.
    pub new_end_point: (usize, usize),
}

/// Which way an offset sitting exactly on an insertion goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assoc {
    /// Stay before the inserted text.
    Before,
    /// Move past the inserted text.
    After,
}

/// A set of non-overlapping changes, sorted by start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transaction {
    changes: Vec<Change>,
}

impl Transaction {
    /// A transaction of `changes`. They are sorted here; empty changes
    /// (nothing replaced, nothing inserted) are dropped.
    ///
    /// # Panics
    ///
    /// When two changes overlap. Callers build changes from merged
    /// selections, so an overlap is a bug in the caller.
    #[must_use]
    pub fn new(mut changes: Vec<Change>) -> Self {
        changes.retain(|c| !(c.range.is_empty() && c.text.is_empty()));
        changes.sort_by_key(|c| (c.range.start, c.range.end));
        for pair in changes.windows(2) {
            assert!(
                pair[0].range.end <= pair[1].range.start,
                "a transaction's changes overlap: {:?} and {:?}",
                pair[0].range,
                pair[1].range
            );
        }
        Self { changes }
    }

    /// One change.
    #[must_use]
    pub fn single(range: Range<usize>, text: impl Into<String>) -> Self {
        Self::new(vec![Change {
            range,
            text: text.into(),
        }])
    }

    /// The changes, by start.
    #[must_use]
    pub fn changes(&self) -> &[Change] {
        &self.changes
    }

    /// Whether the transaction changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Apply to `buffer`, push one record per change onto `records`, and
    /// return the inverse.
    pub fn apply(&self, buffer: &mut TextBuffer, records: &mut Vec<EditRecord>) -> Transaction {
        let mut removed = vec![String::new(); self.changes.len()];
        for (k, change) in self.changes.iter().enumerate().rev() {
            let start_point = buffer.point_of(change.range.start);
            let old_end_point = buffer.point_of(change.range.end);
            removed[k] = buffer.slice(change.range.clone()).into_owned();
            buffer.replace(change.range.clone(), &change.text);
            records.push(EditRecord {
                start_byte: change.range.start,
                old_end_byte: change.range.end,
                new_end_byte: change.range.start + change.text.len(),
                start_point,
                old_end_point,
                new_end_point: point_after(start_point, &change.text),
            });
        }
        let mut delta: isize = 0;
        let mut inverse = Vec::with_capacity(self.changes.len());
        for (change, old) in self.changes.iter().zip(removed) {
            let start = change.range.start.wrapping_add_signed(delta);
            inverse.push(Change {
                range: start..start + change.text.len(),
                text: old,
            });
            delta += change.text.len() as isize - change.range.len() as isize;
        }
        Transaction { changes: inverse }
    }

    /// Where pre-transaction offset `at` lands after the transaction. An
    /// offset inside a replaced range goes to the replacement's start
    /// (`Before`) or end (`After`); one exactly on an insertion point follows
    /// `assoc` too.
    #[must_use]
    pub fn map(&self, at: usize, assoc: Assoc) -> usize {
        let mut delta: isize = 0;
        for change in &self.changes {
            let Range { start, end } = change.range;
            if at < start || (at == start && start == end && assoc == Assoc::Before) {
                break;
            }
            if at > end || (at == end && start != end) {
                delta += change.text.len() as isize - (end - start) as isize;
                continue;
            }
            // Inside the replaced range, or on a pure insertion with `After`.
            let base = start.wrapping_add_signed(delta);
            return match assoc {
                Assoc::Before => base,
                Assoc::After => base + change.text.len(),
            };
        }
        at.wrapping_add_signed(delta)
    }
}

/// The point after inserting `text` at `start`.
fn point_after(start: (usize, usize), text: &str) -> (usize, usize) {
    let newlines = count_newlines(text.as_bytes());
    if newlines == 0 {
        (start.0, start.1 + text.len())
    } else {
        let tail = text.len() - text.rfind('\n').map_or(0, |i| i + 1);
        (start.0 + newlines, tail)
    }
}

#[cfg(test)]
mod tests {
    use super::{Assoc, Change, Transaction};
    use crate::edit::buffer::TextBuffer;

    #[test]
    fn apply_then_inverse_restores_the_text() {
        let mut b = TextBuffer::from_str("one\ntwo\nthree");
        let t = Transaction::new(vec![
            Change {
                range: 8..13,
                text: "3".into(),
            },
            Change {
                range: 0..3,
                text: "1\n1".into(),
            },
        ]);
        let mut records = Vec::new();
        let inverse = t.apply(&mut b, &mut records);
        assert_eq!(b.to_string(), "1\n1\ntwo\n3");
        // Applied from the end: the later change is recorded first.
        assert_eq!(records[0].start_byte, 8);
        assert_eq!(records[1].new_end_point, (1, 1));
        inverse.apply(&mut b, &mut records);
        assert_eq!(b.to_string(), "one\ntwo\nthree");
    }

    #[test]
    fn map_follows_insertions_and_replacements() {
        let t = Transaction::new(vec![
            Change {
                range: 2..2,
                text: "xx".into(),
            },
            Change {
                range: 5..8,
                text: "y".into(),
            },
        ]);
        assert_eq!(t.map(1, Assoc::After), 1);
        assert_eq!(t.map(2, Assoc::Before), 2);
        assert_eq!(t.map(2, Assoc::After), 4);
        assert_eq!(t.map(4, Assoc::After), 6);
        assert_eq!(t.map(6, Assoc::Before), 7);
        assert_eq!(t.map(6, Assoc::After), 8);
        assert_eq!(t.map(8, Assoc::After), 8);
        assert_eq!(t.map(10, Assoc::After), 10);
    }
}
