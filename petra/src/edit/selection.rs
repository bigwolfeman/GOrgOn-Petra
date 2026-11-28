//! Selections: an anchor, a caret, and the column a vertical move aims for.
//!
//! An editor holds one or more [`Selection`]s in a [`Selections`] set. The set
//! is kept sorted by start and merged where two touch, so every edit can walk
//! it once from the end of the text to the start without offsets shifting
//! under it. One member is the primary: the caret the view follows and the
//! status bar reports.

use std::ops::Range;

/// One selection. `anchor == head` is a plain caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    /// Where the selection started, as a byte offset.
    pub anchor: usize,
    /// Where the caret is, as a byte offset.
    pub head: usize,
    /// The drawn column (in cells) an Up or Down aims for, kept across
    /// lines too short to reach it. `None` until a vertical move sets it.
    pub goal: Option<usize>,
}

impl Selection {
    /// A caret at `at`.
    #[must_use]
    pub const fn caret(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
            goal: None,
        }
    }

    /// `anchor..head`, either direction.
    #[must_use]
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self {
            anchor,
            head,
            goal: None,
        }
    }

    /// The ordered byte range.
    #[must_use]
    pub fn range(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The selection with its caret moved to `head`: kept anchor when
    /// `extend`, a caret otherwise. The goal column is dropped.
    #[must_use]
    pub fn moved_to(self, head: usize, extend: bool) -> Self {
        Self {
            anchor: if extend { self.anchor } else { head },
            head,
            goal: None,
        }
    }
}

/// A sorted, merged set of selections with one primary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selections {
    items: Vec<Selection>,
    primary: usize,
}

impl Default for Selections {
    fn default() -> Self {
        Self::single(Selection::caret(0))
    }
}

impl Selections {
    /// One selection.
    #[must_use]
    pub fn single(selection: Selection) -> Self {
        Self {
            items: vec![selection],
            primary: 0,
        }
    }

    /// A set from `items` (at least one), `primary` indexing into it. Sorted
    /// and merged before it is returned.
    #[must_use]
    pub fn from_vec(items: Vec<Selection>, primary: usize) -> Self {
        let mut set = Self {
            primary: primary.min(items.len().saturating_sub(1)),
            items,
        };
        if set.items.is_empty() {
            set.items.push(Selection::caret(0));
            set.primary = 0;
        }
        set.normalize();
        set
    }

    /// The selections in text order.
    pub fn iter(&self) -> std::slice::Iter<'_, Selection> {
        self.items.iter()
    }

    /// How many selections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Always false: a set holds at least one selection.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The primary selection.
    #[must_use]
    pub fn primary(&self) -> Selection {
        self.items[self.primary]
    }

    /// The primary's index in text order.
    #[must_use]
    pub fn primary_index(&self) -> usize {
        self.primary
    }

    /// Replace every selection by `f(selection)` and normalize.
    pub fn map(&mut self, mut f: impl FnMut(Selection) -> Selection) {
        for s in &mut self.items {
            *s = f(*s);
        }
        self.normalize();
    }

    /// Add `selection` and make it the primary.
    pub fn push(&mut self, selection: Selection) {
        self.items.push(selection);
        self.primary = self.items.len() - 1;
        self.normalize();
    }

    /// Keep the primary alone.
    pub fn collapse_to_primary(&mut self) {
        let p = self.primary();
        self.items = vec![p];
        self.primary = 0;
    }

    /// Clamp every offset to `len` (after a reload shortened the text).
    pub fn clamp(&mut self, len: usize) {
        self.map(|s| Selection {
            anchor: s.anchor.min(len),
            head: s.head.min(len),
            goal: s.goal,
        });
    }

    /// Sort by start and merge overlapping selections, and touching ones
    /// when either is non-empty, keeping the primary pointed at the merged
    /// member that holds it.
    fn normalize(&mut self) {
        if self.items.len() <= 1 {
            self.primary = 0;
            return;
        }
        let primary = self.items[self.primary];
        let mut indexed: Vec<(usize, Selection)> = self.items.drain(..).enumerate().collect();
        indexed.sort_by_key(|(_, s)| (s.range().start, s.range().end));
        let mut out: Vec<Selection> = Vec::with_capacity(indexed.len());
        let mut primary_at = 0;
        for (_, s) in indexed {
            let is_primary = s == primary;
            match out.last_mut() {
                Some(last) if merges(last, &s) => {
                    let start = last.range().start.min(s.range().start);
                    let end = last.range().end.max(s.range().end);
                    // Keep the direction of the later selection.
                    let forward = s.head >= s.anchor;
                    *last = if forward {
                        Selection::new(start, end)
                    } else {
                        Selection::new(end, start)
                    };
                    if is_primary {
                        primary_at = out.len() - 1;
                    }
                }
                _ => {
                    out.push(s);
                    if is_primary {
                        primary_at = out.len() - 1;
                    }
                }
            }
        }
        self.items = out;
        self.primary = primary_at;
    }
}

/// Whether `next` (which starts at or after `last`) joins `last`: they
/// overlap, they are two non-empty selections that touch, or they are the
/// same caret. A caret touching a selection's end stays separate.
fn merges(last: &Selection, next: &Selection) -> bool {
    let (a, b) = (last.range(), next.range());
    b.start < a.end || (b.start == a.end && !last.is_empty() && !next.is_empty()) || a == b
}

#[cfg(test)]
mod tests {
    use super::{Selection, Selections};

    #[test]
    fn overlapping_selections_merge_and_keep_the_primary() {
        let set = Selections::from_vec(
            vec![
                Selection::new(10, 15),
                Selection::new(0, 3),
                Selection::new(12, 20),
                Selection::caret(3),
            ],
            2,
        );
        let ranges: Vec<_> = set.iter().map(Selection::range).collect();
        assert_eq!(ranges, vec![0..3, 3..3, 10..20]);
        assert_eq!(set.primary().range(), 10..20);
    }

    #[test]
    fn equal_carets_merge() {
        let set = Selections::from_vec(vec![Selection::caret(4), Selection::caret(4)], 1);
        assert_eq!(set.len(), 1);
    }
}
