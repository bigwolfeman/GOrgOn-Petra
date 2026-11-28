//! Find in a [`TextBuffer`]: the matches of one query as byte ranges, kept
//! in step with edits through the same [`EditRecord`]s the syntax tree
//! takes (spec 006 FR-004, FR-023).
//!
//! A query never holds a line break (the find field is one line), so every
//! match sits inside one line. That makes the update after an edit local:
//! the matches on the lines an edit touched are found again, the ones
//! after it shift by the edit's length change, and the rest stay. A paste
//! of the whole file re-searches the whole file; a keystroke re-searches
//! one line.
//!
//! Matching:
//! - with match case, plain byte equality (`str::match_indices`);
//! - without it, each `char` folds to the first `char` of its lowercase
//!   form, so a match keeps the byte length of the text it covers (a full
//!   lowercase can grow a `char`, `İ` to two, and would not). An ASCII
//!   query takes a byte scan that folds ASCII letters only, so the few
//!   non-ASCII letters whose fold is ASCII (`İ`, the Kelvin sign) do not
//!   match it;
//! - whole word asks for no word `char` (alphanumeric or `_`) on either
//!   side of the match.
//!
//! Matches never overlap: after one, the search goes on from its end.
//! Replace builds an [`EditCommand::ReplaceRanges`](super::EditCommand)
//! from [`Finder::replacements`], so a replace all is one transaction and
//! one undo group.

use std::ops::Range;

use super::buffer::TextBuffer;
use super::transaction::EditRecord;

/// How a query matches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FindOptions {
    /// Letters match only in the same case.
    pub match_case: bool,
    /// A match has no word character on either side.
    pub whole_word: bool,
}

/// One query's matches in a buffer, sorted and non-overlapping.
#[derive(Clone, Debug, Default)]
pub struct Finder {
    query: String,
    options: FindOptions,
    /// The query folded `char` by `char`, for the case-blind search.
    folded: Vec<char>,
    matches: Vec<Range<usize>>,
}

impl Finder {
    /// A finder for `query` under `options`, searched over `buffer`.
    #[must_use]
    pub fn new(query: impl Into<String>, options: FindOptions, buffer: &TextBuffer) -> Self {
        let mut finder = Self {
            query: query.into(),
            options,
            ..Self::default()
        };
        finder.refold();
        finder.search(buffer);
        finder
    }

    /// The query.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The options.
    #[must_use]
    pub fn options(&self) -> FindOptions {
        self.options
    }

    /// Search for `query` instead.
    pub fn set_query(&mut self, query: impl Into<String>, buffer: &TextBuffer) {
        let query = query.into();
        if query == self.query {
            return;
        }
        self.query = query;
        self.refold();
        self.search(buffer);
    }

    /// Match under `options` instead.
    pub fn set_options(&mut self, options: FindOptions, buffer: &TextBuffer) {
        if options == self.options {
            return;
        }
        self.options = options;
        self.search(buffer);
    }

    /// Every match, in order.
    #[must_use]
    pub fn matches(&self) -> &[Range<usize>] {
        &self.matches
    }

    /// The matches that start inside `range`, a line's bytes for drawing.
    #[must_use]
    pub fn within(&self, range: Range<usize>) -> &[Range<usize>] {
        let from = self.matches.partition_point(|m| m.start < range.start);
        let to = self.matches.partition_point(|m| m.start < range.end);
        &self.matches[from..to.max(from)]
    }

    /// The index of the match that is exactly `range`, as "n of m" counts.
    #[must_use]
    pub fn index_of(&self, range: &Range<usize>) -> Option<usize> {
        let at = self.matches.partition_point(|m| m.start < range.start);
        (self.matches.get(at) == Some(range)).then_some(at)
    }

    /// The first match that starts at or after `at`, wrapping to the first
    /// match. [`None`] with no matches.
    #[must_use]
    pub fn next_from(&self, at: usize) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        let i = self.matches.partition_point(|m| m.start < at);
        Some(if i == self.matches.len() { 0 } else { i })
    }

    /// The last match that starts before `at`, wrapping to the last match.
    /// [`None`] with no matches.
    #[must_use]
    pub fn prev_before(&self, at: usize) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        let i = self.matches.partition_point(|m| m.start < at);
        Some(if i == 0 {
            self.matches.len() - 1
        } else {
            i - 1
        })
    }

    /// Every match paired with `with`, for one
    /// [`EditCommand::ReplaceRanges`](super::EditCommand).
    #[must_use]
    pub fn replacements(&self, with: &str) -> Vec<(Range<usize>, String)> {
        self.matches
            .iter()
            .map(|m| (m.clone(), with.to_owned()))
            .collect()
    }

    /// Follow `records`, applied in order, to `buffer` as it now stands.
    /// Only the lines they touched are searched again.
    pub fn apply(&mut self, records: &[EditRecord], buffer: &TextBuffer) {
        if records.is_empty() || self.query.is_empty() || self.query.contains('\n') {
            return;
        }
        // The bytes each record left changed, in the final text: a span
        // maps through every later record as the matches do.
        let mut dirty: Vec<Range<usize>> = Vec::with_capacity(records.len());
        for record in records {
            let old = record.start_byte..record.old_end_byte;
            let grow = record.new_end_byte as isize - record.old_end_byte as isize;
            let shift = |at: usize| at.saturating_add_signed(grow);
            self.matches.retain_mut(|m| {
                if m.end < old.start || (m.end == old.start && old.start != old.end) {
                    true
                } else if m.start > old.end || (m.start == old.end && old.start != old.end) {
                    *m = shift(m.start)..shift(m.end);
                    true
                } else {
                    // Touched, or an insertion right at its edge: the line
                    // is searched again.
                    false
                }
            });
            for span in &mut dirty {
                span.start = map_point(span.start, record);
                span.end = map_point(span.end, record);
            }
            dirty.push(record.start_byte..record.new_end_byte);
        }
        // The lines the spans cover, merged.
        let mut lines: Vec<Range<usize>> = dirty
            .iter()
            .map(|span| buffer.line_of(span.start)..buffer.line_of(span.end) + 1)
            .collect();
        lines.sort_by_key(|r| r.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(lines.len());
        for r in lines {
            match merged.last_mut() {
                Some(last) if r.start <= last.end => last.end = last.end.max(r.end),
                _ => merged.push(r),
            }
        }
        for lines in merged {
            let bytes = buffer.line_start(lines.start)..buffer.line_end(lines.end - 1);
            let from = self.matches.partition_point(|m| m.start < bytes.start);
            let to = self.matches.partition_point(|m| m.start <= bytes.end);
            let mut found = Vec::new();
            self.find_in(&buffer.slice(bytes.clone()), bytes.start, &mut found);
            self.matches.splice(from..to.max(from), found);
        }
    }

    /// Search the whole buffer again.
    fn search(&mut self, buffer: &TextBuffer) {
        self.matches.clear();
        if self.query.is_empty() || self.query.contains('\n') {
            return;
        }
        let mut found = Vec::new();
        self.find_in(&buffer.slice(0..buffer.len_bytes()), 0, &mut found);
        self.matches = found;
    }

    fn refold(&mut self) {
        self.folded = self.query.chars().map(fold).collect();
    }

    /// Push the matches in `text`, which starts at byte `base`, onto `out`.
    fn find_in(&self, text: &str, base: usize, out: &mut Vec<Range<usize>>) {
        let whole = self.options.whole_word;
        let mut keep = |start: usize, end: usize| {
            if !whole || bounded(text, start, end) {
                out.push(base + start..base + end);
                true
            } else {
                false
            }
        };
        if self.options.match_case {
            let len = self.query.len();
            let mut from = 0;
            while let Some(i) = text[from..].find(self.query.as_str()) {
                let start = from + i;
                if keep(start, start + len) {
                    from = start + len;
                } else {
                    from = start + text[start..].chars().next().map_or(1, char::len_utf8);
                }
            }
            return;
        }
        if self.query.is_ascii() {
            // ASCII bytes never sit inside a multi-byte `char`, so a byte
            // scan never splits one.
            let q = self.query.as_bytes();
            let hay = text.as_bytes();
            let (lo, hi) = (q[0].to_ascii_lowercase(), q[0].to_ascii_uppercase());
            let mut i = 0;
            while i + q.len() <= hay.len() {
                let Some(step) = hay[i..=hay.len() - q.len()]
                    .iter()
                    .position(|b| *b == lo || *b == hi)
                else {
                    break;
                };
                let start = i + step;
                if hay[start..start + q.len()].eq_ignore_ascii_case(q)
                    && keep(start, start + q.len())
                {
                    i = start + q.len();
                } else {
                    i = start + 1;
                }
            }
            return;
        }
        let first = self.folded[0];
        let mut starts = text.char_indices();
        while let Some((start, c)) = starts.next() {
            if fold(c) != first {
                continue;
            }
            let mut end = start;
            let mut rest = text[start..].chars();
            let hit = self.folded.iter().all(|want| match rest.next() {
                Some(c) if fold(c) == *want => {
                    end += c.len_utf8();
                    true
                }
                _ => false,
            });
            if hit && keep(start, end) {
                // Resume after the match: skip the `char`s it covered.
                while starts.offset() < end {
                    starts.next();
                }
            }
        }
    }
}

/// `at` in the text before `record` mapped to the text after it. A point
/// inside the replaced bytes goes to the end of the new ones.
fn map_point(at: usize, record: &EditRecord) -> usize {
    if at < record.start_byte {
        at
    } else if at >= record.old_end_byte {
        at - record.old_end_byte + record.new_end_byte
    } else {
        record.new_end_byte
    }
}

/// A `char` folded for the case-blind search.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// No word `char` touches `text[start..end]` from outside.
fn bounded(text: &str, start: usize, end: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(is_word)
        && !text[end..].chars().next().is_some_and(is_word)
}

#[cfg(test)]
// A list of one match is a list of ranges that happens to hold one.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;
    use crate::edit::{EditCommand, EditorConfig, EditorState};

    fn ranges(finder: &Finder) -> Vec<Range<usize>> {
        finder.matches().to_vec()
    }

    fn opts(match_case: bool, whole_word: bool) -> FindOptions {
        FindOptions {
            match_case,
            whole_word,
        }
    }

    #[test]
    fn case_blind_by_default_and_exact_with_match_case() {
        let buffer = TextBuffer::from_str("Foo foo FOO\nfood");
        let blind = Finder::new("foo", opts(false, false), &buffer);
        assert_eq!(ranges(&blind), vec![0..3, 4..7, 8..11, 12..15]);
        let exact = Finder::new("foo", opts(true, false), &buffer);
        assert_eq!(ranges(&exact), vec![4..7, 12..15]);
    }

    #[test]
    fn whole_word_needs_a_boundary_on_both_sides() {
        let buffer = TextBuffer::from_str("foo food _foo foo_ (foo)\nfoo");
        let words = Finder::new("foo", opts(false, true), &buffer);
        assert_eq!(ranges(&words), vec![0..3, 20..23, 25..28]);
        let exact = Finder::new("foo", opts(true, true), &buffer);
        assert_eq!(ranges(&exact), ranges(&words));
    }

    #[test]
    fn a_rejected_word_does_not_hide_the_next_one() {
        let buffer = TextBuffer::from_str("aa a");
        let words = Finder::new("a", opts(true, true), &buffer);
        assert_eq!(ranges(&words), [3..4]);
        let blind = Finder::new("a", opts(false, true), &buffer);
        assert_eq!(ranges(&blind), [3..4]);
    }

    #[test]
    fn matches_never_overlap() {
        let buffer = TextBuffer::from_str("aaaa");
        assert_eq!(
            ranges(&Finder::new("aa", opts(false, false), &buffer)),
            vec![0..2, 2..4]
        );
    }

    #[test]
    fn a_non_ascii_query_folds_per_char_and_keeps_byte_lengths() {
        // `İ` lowercases to two chars; folding keeps the first, so the
        // match covers the original two bytes.
        let buffer = TextBuffer::from_str("Ärger ärger İx");
        let finder = Finder::new("ä", opts(false, false), &buffer);
        assert_eq!(ranges(&finder), vec![0..2, 7..9]);
        let dotted = Finder::new("i̇x", opts(false, false), &buffer);
        assert!(
            dotted.matches().is_empty(),
            "a two-char fold is not a match"
        );
        // An ASCII query folds ASCII only: `İ` is not an `i` to it.
        let i = Finder::new("ix", opts(false, false), &buffer);
        assert!(i.matches().is_empty());
        let x = Finder::new("İX", opts(false, false), &buffer);
        assert_eq!(ranges(&x), [14..17]);
    }

    #[test]
    fn an_ascii_query_skips_over_multibyte_text() {
        let buffer = TextBuffer::from_str("日本 Ab 日AB");
        let finder = Finder::new("ab", opts(false, false), &buffer);
        assert_eq!(ranges(&finder), vec![7..9, 13..15]);
    }

    #[test]
    fn stepping_wraps_both_ways() {
        let buffer = TextBuffer::from_str("x x x");
        let finder = Finder::new("x", FindOptions::default(), &buffer);
        assert_eq!(finder.next_from(1), Some(1));
        assert_eq!(finder.next_from(5), Some(0));
        assert_eq!(finder.prev_before(0), Some(2));
        assert_eq!(finder.prev_before(2), Some(0));
        assert_eq!(finder.index_of(&(2..3)), Some(1));
        assert_eq!(finder.index_of(&(2..4)), None);
        let none = Finder::new("y", FindOptions::default(), &buffer);
        assert_eq!(none.next_from(0), None);
        assert_eq!(none.prev_before(0), None);
    }

    #[test]
    fn an_empty_or_multiline_query_finds_nothing() {
        let buffer = TextBuffer::from_str("a\nb");
        assert!(
            Finder::new("", FindOptions::default(), &buffer)
                .matches()
                .is_empty()
        );
        assert!(
            Finder::new("a\nb", FindOptions::default(), &buffer)
                .matches()
                .is_empty()
        );
    }

    #[test]
    fn within_returns_the_matches_of_one_line() {
        let buffer = TextBuffer::from_str("ab ab\nab\nxx ab");
        let finder = Finder::new("ab", FindOptions::default(), &buffer);
        assert_eq!(finder.within(buffer.line_range(0)), &[0..2, 3..5]);
        assert_eq!(finder.within(buffer.line_range(1)), &[6..8]);
        assert_eq!(finder.within(buffer.line_range(2)), &[12..14]);
    }

    /// After any edit, the incremental matches equal a fresh search.
    fn follows(text: &str, query: &str, options: FindOptions, edits: &[EditCommand]) {
        let mut state = EditorState::new(text, EditorConfig::default());
        let mut finder = Finder::new(query, options, state.buffer());
        for edit in edits {
            state.apply(edit.clone(), 0);
            let records = state.take_records();
            finder.apply(&records, state.buffer());
            let fresh = Finder::new(query, options, state.buffer());
            assert_eq!(
                ranges(&finder),
                ranges(&fresh),
                "after {edit:?} on {:?}",
                state.buffer().to_string()
            );
        }
    }

    #[test]
    fn matches_follow_typing_deleting_and_pasting() {
        let text = "let foo = foo;\nfoo(foo)\n\nfo o\n";
        let blind = FindOptions::default();
        let words = opts(false, true);
        let place = |at| EditCommand::PlaceCaret { at, extend: false };
        let edits = [
            place(3),
            EditCommand::Insert("x".into()),
            EditCommand::Backspace,
            place(30),
            EditCommand::Delete,
            EditCommand::Insert("o".into()),
            place(0),
            EditCommand::Paste("foo\nfoo foo\n".into()),
            EditCommand::SelectAll,
            EditCommand::Insert("foo".into()),
            EditCommand::Undo,
            EditCommand::Redo,
            EditCommand::Undo,
        ];
        follows(text, "foo", blind, &edits);
        follows(text, "foo", words, &edits);
        follows(text, "o", opts(true, false), &edits);
    }

    #[test]
    fn a_letter_typed_against_a_word_match_drops_it() {
        let mut state = EditorState::new("foo bar", EditorConfig::default());
        let mut finder = Finder::new("foo", opts(false, true), state.buffer());
        state.apply(
            EditCommand::PlaceCaret {
                at: 3,
                extend: false,
            },
            0,
        );
        state.apply(EditCommand::Insert("d".into()), 0);
        finder.apply(&state.take_records(), state.buffer());
        assert!(finder.matches().is_empty(), "food is not the word foo");
    }

    #[test]
    fn replace_all_is_one_undo_group() {
        let mut state = EditorState::new("a b a\na", EditorConfig::default());
        let finder = Finder::new("a", FindOptions::default(), state.buffer());
        state.apply(
            EditCommand::ReplaceRanges {
                edits: finder.replacements("xy"),
            },
            0,
        );
        assert_eq!(state.buffer().to_string(), "xy b xy\nxy");
        state.apply(EditCommand::Undo, 0);
        assert_eq!(state.buffer().to_string(), "a b a\na");
    }

    #[test]
    fn options_and_query_changes_search_again() {
        let buffer = TextBuffer::from_str("Ab ab");
        let mut finder = Finder::new("ab", FindOptions::default(), &buffer);
        assert_eq!(finder.matches().len(), 2);
        finder.set_options(opts(true, false), &buffer);
        assert_eq!(ranges(&finder), [3..5]);
        finder.set_query("Ab", &buffer);
        assert_eq!(ranges(&finder), [0..2]);
    }
}
