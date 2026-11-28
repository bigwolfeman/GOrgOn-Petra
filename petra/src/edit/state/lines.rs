//! Line commands and auto-close (spec 006 US6).
//!
//! The line commands work on blocks: every line a selection touches, with
//! blocks that touch or overlap merged, so two carets on one line move that
//! line once. Each command is one transaction and one undo group.

use std::ops::Range;

use super::{EditorState, Outcome};
use crate::edit::history::EditKind;
use crate::edit::selection::{Selection, Selections};
use crate::edit::transaction::{Change, Transaction};

/// Pairs auto-close inserts, opener to closer.
const PAIRS: [(char, char); 6] = [
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
];

fn closer_of(open: char) -> Option<char> {
    PAIRS.iter().find(|(o, _)| *o == open).map(|(_, c)| *c)
}

fn is_closer(c: char) -> bool {
    PAIRS.iter().any(|(_, close)| *close == c)
}

fn is_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl EditorState {
    /// The line blocks of every selection, sorted, merged when they touch.
    fn blocks(&self) -> Vec<Range<usize>> {
        let mut blocks: Vec<Range<usize>> =
            self.selections.iter().map(|s| self.lines_of(*s)).collect();
        blocks.sort_by_key(|b| b.start);
        let mut merged: Vec<Range<usize>> = Vec::with_capacity(blocks.len());
        for b in blocks {
            match merged.last_mut() {
                Some(last) if b.start <= last.end => last.end = last.end.max(b.end),
                _ => merged.push(b),
            }
        }
        merged
    }

    /// The text of lines `block`, joined by `\n`, without a final break.
    fn block_text(&self, block: &Range<usize>) -> String {
        let start = self.buffer.line_start(block.start);
        let end = self.buffer.line_end(block.end - 1);
        self.buffer.slice(start..end).into_owned()
    }

    /// Apply `changes` and set `selections` as given (the commands know
    /// where each selection lands better than mapping does).
    fn commit_with(
        &mut self,
        changes: Vec<Change>,
        selections: Selections,
        now_ms: u64,
    ) -> Outcome {
        let transaction = Transaction::new(changes);
        if transaction.is_empty() {
            return Outcome::default();
        }
        let before = self.selections.clone();
        let inverse = transaction.apply(&mut self.buffer, &mut self.records);
        self.map_auto_closed(&transaction);
        self.selections = selections;
        self.history.record(
            transaction,
            inverse,
            &before,
            &self.selections,
            EditKind::Other,
            false,
            now_ms,
        );
        self.version += 1;
        Outcome {
            edited: true,
            moved: true,
            clipboard: None,
        }
    }

    /// Selections shifted by `shift(selection)` bytes.
    fn shifted(&self, shift: impl Fn(&Selection) -> isize) -> Selections {
        let primary = self.selections.primary_index();
        let items = self
            .selections
            .iter()
            .map(|s| {
                let by = shift(s);
                Selection {
                    anchor: s.anchor.wrapping_add_signed(by),
                    head: s.head.wrapping_add_signed(by),
                    goal: s.goal,
                }
            })
            .collect();
        Selections::from_vec(items, primary)
    }

    /// The block of `blocks` that holds selection `s`.
    fn block_of<'b>(&self, blocks: &'b [Range<usize>], s: &Selection) -> &'b Range<usize> {
        let line = self.buffer.line_of(s.range().start);
        blocks
            .iter()
            .find(|b| b.contains(&line))
            .expect("every selection lies in its block")
    }

    /// Alt+Up and Alt+Down: swap each block with the line past it. A block
    /// at the edge of the file stays, and so do the others (VS Code moves
    /// all or none).
    pub(super) fn move_lines(&mut self, down: bool, now_ms: u64) -> Outcome {
        let blocks = self.blocks();
        let total = self.buffer.len_lines();
        let at_edge = blocks
            .iter()
            .any(|b| if down { b.end >= total } else { b.start == 0 });
        if blocks.is_empty() || at_edge {
            return Outcome::default();
        }
        let mut changes = Vec::with_capacity(blocks.len());
        for b in &blocks {
            let block = self.block_text(b);
            let (span, text) = if down {
                let other = self.buffer.line(b.end).into_owned();
                (b.start..b.end + 1, format!("{other}\n{block}"))
            } else {
                let other = self.buffer.line(b.start - 1).into_owned();
                (b.start - 1..b.end, format!("{block}\n{other}"))
            };
            let range = self.buffer.line_start(span.start)..self.buffer.line_end(span.end - 1);
            changes.push(Change { range, text });
        }
        let selections = self.shifted(|s| {
            let b = self.block_of(&blocks, s);
            if down {
                self.buffer.line(b.end).len() as isize + 1
            } else {
                -(self.buffer.line(b.start - 1).len() as isize + 1)
            }
        });
        self.commit_with(changes, selections, now_ms)
    }

    /// Shift+Alt+Down and Shift+Alt+Up: copy each block below (the
    /// selection moves onto the copy) or above (it stays on the upper copy).
    pub(super) fn copy_lines(&mut self, down: bool, now_ms: u64) -> Outcome {
        let blocks = self.blocks();
        let mut changes = Vec::with_capacity(blocks.len());
        for b in &blocks {
            let block = self.block_text(b);
            changes.push(if down {
                let at = self.buffer.line_end(b.end - 1);
                Change {
                    range: at..at,
                    text: format!("\n{block}"),
                }
            } else {
                let at = self.buffer.line_start(b.start);
                Change {
                    range: at..at,
                    text: format!("{block}\n"),
                }
            });
        }
        // Each selection moves past every copy inserted before it, plus,
        // copying down, its own block's copy.
        let selections = self.shifted(|s| {
            let own = self.block_of(&blocks, s).start;
            blocks
                .iter()
                .filter(|b| if down { b.start <= own } else { b.start < own })
                .map(|b| self.block_text(b).len() as isize + 1)
                .sum()
        });
        self.commit_with(changes, selections, now_ms)
    }

    /// Ctrl+Shift+K: delete each block with its line break. The caret lands
    /// on the line that took the block's place, at the caret's column.
    pub(super) fn delete_lines(&mut self, now_ms: u64) -> Outcome {
        let blocks = self.blocks();
        let total = self.buffer.len_lines();
        let len = self.buffer.len_bytes();
        let mut changes = Vec::with_capacity(blocks.len());
        for b in &blocks {
            let range = if b.end < total {
                self.buffer.line_start(b.start)..self.buffer.line_start(b.end)
            } else if b.start > 0 {
                // The last lines: take the break before them instead.
                self.buffer.line_end(b.start - 1)..len
            } else {
                0..len
            };
            changes.push(Change {
                range,
                text: String::new(),
            });
        }
        let transaction = Transaction::new(changes.clone());
        let primary = self.selections.primary_index();
        let carets: Vec<Selection> = self
            .selections
            .iter()
            .map(|s| {
                let line = self.buffer.line_of(s.head);
                let column = s.head - self.buffer.line_start(line);
                let b = self.block_of(&blocks, s);
                let start = self.buffer.line_start(b.start);
                (start, column)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(start, column)| {
                let at = transaction.map(start, crate::edit::transaction::Assoc::After);
                Selection::caret(at + column)
            })
            .collect();
        let before = self.selections.clone();
        let inverse = transaction.apply(&mut self.buffer, &mut self.records);
        self.map_auto_closed(&transaction);
        // Clamp each caret to its new line, now the text is in place.
        let items = carets
            .into_iter()
            .map(|c| {
                let at = c.head.min(self.buffer.len_bytes());
                let line = self.buffer.line_of(at);
                let at = at.min(self.buffer.line_end(line));
                Selection::caret(self.buffer.floor_boundary(at))
            })
            .collect();
        self.selections = Selections::from_vec(items, primary);
        self.history.record(
            transaction,
            inverse,
            &before,
            &self.selections,
            EditKind::Other,
            false,
            now_ms,
        );
        self.version += 1;
        Outcome {
            edited: true,
            moved: true,
            clipboard: None,
        }
    }

    /// Ctrl+]: indent every touched line, with or without a selection.
    pub(super) fn indent_lines(&mut self, now_ms: u64) -> Outcome {
        let unit = self.config.indent_unit();
        let changes = self
            .blocks()
            .into_iter()
            .flatten()
            .filter(|l| !self.buffer.line_range(*l).is_empty())
            .map(|l| {
                let at = self.buffer.line_start(l);
                Change {
                    range: at..at,
                    text: unit.clone(),
                }
            })
            .collect();
        self.commit_changes(changes, EditKind::Other, false, now_ms)
    }

    /// Ctrl+/: comment every touched line at the block's least indentation,
    /// or, when every non-blank line already starts with the comment,
    /// remove it and one space after it.
    pub(super) fn toggle_line_comment(&mut self, now_ms: u64) -> Outcome {
        let Some(token) = self.config.line_comment else {
            return Outcome::default();
        };
        let mut changes = Vec::new();
        for b in self.blocks() {
            let lines: Vec<(usize, String)> = b
                .clone()
                .map(|l| (l, self.buffer.line(l).into_owned()))
                .filter(|(_, text)| !text.trim().is_empty())
                .collect();
            if lines.is_empty() {
                continue;
            }
            let indent = |text: &str| text.len() - text.trim_start().len();
            let commented = lines
                .iter()
                .all(|(_, text)| text.trim_start().starts_with(token));
            if commented {
                for (l, text) in &lines {
                    let at = self.buffer.line_start(*l) + indent(text);
                    let rest = &text[indent(text) + token.len()..];
                    let space = usize::from(rest.starts_with(' '));
                    changes.push(Change {
                        range: at..at + token.len() + space,
                        text: String::new(),
                    });
                }
            } else {
                let column = lines.iter().map(|(_, t)| indent(t)).min().unwrap_or(0);
                for (l, _) in &lines {
                    let at = self.buffer.line_start(*l) + column;
                    changes.push(Change {
                        range: at..at,
                        text: format!("{token} "),
                    });
                }
            }
        }
        self.commit_changes(changes, EditKind::Other, false, now_ms)
    }

    // ---- auto-close ----

    /// Move every remembered closer through `transaction`, dropping the
    /// ones it deleted.
    pub(super) fn map_auto_closed(&mut self, transaction: &Transaction) {
        if self.auto_closed.is_empty() {
            return;
        }
        let deleted = |at: usize| {
            transaction
                .changes()
                .iter()
                .any(|c| c.range.start <= at && at < c.range.end)
        };
        self.auto_closed = self
            .auto_closed
            .iter()
            .filter(|at| !deleted(**at))
            .map(|at| transaction.map(*at, crate::edit::transaction::Assoc::After))
            .collect();
    }

    /// A caret moved on its own: the remembered closers no longer count.
    pub(super) fn forget_auto_closed(&mut self) {
        self.auto_closed.clear();
    }

    /// Typing `typed` with auto-close on: step over remembered closers,
    /// wrap selections in a pair, or insert a pair. `None` when the typing
    /// is plain and the normal insert should run.
    pub(super) fn auto_close(&mut self, typed: &str, now_ms: u64) -> Option<Outcome> {
        if !self.config.auto_close {
            return None;
        }
        let mut chars = typed.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            return None;
        };
        let all_carets = self.selections.iter().all(Selection::is_empty);
        // Step over: every caret sits before a closer auto-close typed.
        if all_carets
            && is_closer(c)
            && self.selections.iter().all(|s| {
                self.buffer.char_at(s.head) == Some(c) && self.auto_closed.contains(&s.head)
            })
        {
            let heads: Vec<usize> = self.selections.iter().map(|s| s.head).collect();
            self.auto_closed.retain(|at| !heads.contains(at));
            self.selections
                .map(|s| Selection::caret(s.head + c.len_utf8()));
            self.history.break_group();
            return Some(Outcome {
                moved: true,
                ..Outcome::default()
            });
        }
        let close = closer_of(c)?;
        // Wrap: every selection is non-empty.
        if self.selections.iter().all(|s| !s.is_empty()) {
            let changes = self
                .selections
                .iter()
                .flat_map(|s| {
                    let r = s.range();
                    [
                        Change {
                            range: r.start..r.start,
                            text: c.to_string(),
                        },
                        Change {
                            range: r.end..r.end,
                            text: close.to_string(),
                        },
                    ]
                })
                .collect();
            let width = c.len_utf8() as isize;
            let primary = self.selections.primary_index();
            let items = self
                .selections
                .iter()
                .enumerate()
                .map(|(k, s)| {
                    // Each earlier selection added an opener and a closer.
                    let before = k as isize * (width + close.len_utf8() as isize);
                    let shift = |at: usize| at.wrapping_add_signed(before + width);
                    Selection::new(shift(s.anchor), shift(s.head))
                })
                .collect();
            return Some(self.commit_with(changes, Selections::from_vec(items, primary), now_ms));
        }
        // Pair: only where a closer reads naturally, at every caret.
        let fits = |s: &Selection| {
            let next = self.buffer.char_at(s.head);
            let prev = self.buffer.char_before(s.head);
            let free_after =
                next.is_none_or(|n| n.is_whitespace() || is_closer(n) || n == ';' || n == ',');
            let free_before = !is_quote(c) || prev.is_none_or(|p| !is_word(p) && p != c);
            free_after && free_before
        };
        if !all_carets || !self.selections.iter().all(fits) {
            return None;
        }
        let pair = format!("{c}{close}");
        let outcome = self.edit_each(EditKind::Typing, true, now_ms, |_, _, s| {
            Some((
                Change {
                    range: s.range(),
                    text: pair.clone(),
                },
                c.len_utf8(),
            ))
        });
        let heads: Vec<usize> = self.selections.iter().map(|s| s.head).collect();
        self.auto_closed.extend(heads);
        Some(outcome)
    }

    /// Backspace with every caret between a pair auto-close typed: delete
    /// both halves. `None` otherwise.
    pub(super) fn delete_auto_pair(&mut self, now_ms: u64) -> Option<Outcome> {
        if !self.config.auto_close || self.auto_closed.is_empty() {
            return None;
        }
        let paired = self.selections.iter().all(|s| {
            s.is_empty()
                && self.auto_closed.contains(&s.head)
                && self
                    .buffer
                    .char_before(s.head)
                    .and_then(closer_of)
                    .is_some_and(|close| self.buffer.char_at(s.head) == Some(close))
        });
        if !paired {
            return None;
        }
        Some(self.edit_each(EditKind::Deleting, true, now_ms, |b, _, s| {
            let from = b.prev_boundary(s.head);
            let to = b.next_boundary(s.head);
            Some((
                Change {
                    range: from..to,
                    text: String::new(),
                },
                0,
            ))
        }))
    }
}

#[cfg(test)]
// A list of one selection is a list of ranges that happens to hold one.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use crate::edit::{EditCommand as C, EditorConfig, EditorState};

    fn state(text: &str) -> EditorState {
        EditorState::new(
            text,
            EditorConfig {
                line_comment: Some("//"),
                ..EditorConfig::default()
            },
        )
    }

    fn at(s: &mut EditorState, at: usize) {
        s.apply(C::PlaceCaret { at, extend: false }, 0);
    }

    fn text(s: &EditorState) -> String {
        s.buffer().to_string()
    }

    fn head(s: &EditorState) -> usize {
        s.selections().primary().head
    }

    #[test]
    fn alt_arrows_move_the_line_and_undo_puts_it_back() {
        let mut s = state("a\nbb\nc");
        at(&mut s, 3); // "bb", column 1
        s.apply(C::MoveLines { down: true }, 0);
        assert_eq!(text(&s), "a\nc\nbb");
        assert_eq!(head(&s), 5, "the caret moves with its line");
        s.apply(C::MoveLines { down: true }, 0);
        assert_eq!(text(&s), "a\nc\nbb", "the last line stays");
        s.apply(C::MoveLines { down: false }, 0);
        s.apply(C::MoveLines { down: false }, 0);
        assert_eq!(text(&s), "bb\na\nc");
        assert_eq!(head(&s), 1);
        s.apply(C::Undo, 0);
        s.apply(C::Undo, 0);
        s.apply(C::Undo, 0);
        assert_eq!(text(&s), "a\nbb\nc");
    }

    #[test]
    fn a_multi_line_selection_moves_as_one_block() {
        let mut s = state("1\n2\n3\n4");
        s.apply(C::Select { ranges: vec![2..5] }, 0); // "2\n3"
        s.apply(C::MoveLines { down: true }, 0);
        assert_eq!(text(&s), "1\n4\n2\n3");
        assert_eq!(s.selections().primary().range(), 4..7);
    }

    #[test]
    fn shift_alt_arrows_copy_the_line() {
        let mut s = state("a\nb");
        at(&mut s, 0);
        s.apply(C::CopyLines { down: true }, 0);
        assert_eq!(text(&s), "a\na\nb");
        assert_eq!(head(&s), 2, "the caret follows the copy below");
        s.apply(C::CopyLines { down: false }, 0);
        assert_eq!(text(&s), "a\na\na\nb");
        assert_eq!(head(&s), 2, "copying up keeps the caret on the upper copy");
        s.apply(C::Undo, 0);
        s.apply(C::Undo, 0);
        assert_eq!(text(&s), "a\nb");
    }

    #[test]
    fn ctrl_shift_k_deletes_the_line_and_keeps_the_column() {
        let mut s = state("abc\ndef\ngh");
        at(&mut s, 5); // "def", column 1
        s.apply(C::DeleteLines, 0);
        assert_eq!(text(&s), "abc\ngh");
        assert_eq!(head(&s), 5, "column 1 of the line that took its place");
        at(&mut s, 5);
        s.apply(C::DeleteLines, 0);
        assert_eq!(text(&s), "abc", "the last line takes the break before it");
        s.apply(C::DeleteLines, 0);
        assert_eq!(text(&s), "");
        s.apply(C::Undo, 0);
        assert_eq!(text(&s), "abc");
    }

    #[test]
    fn ctrl_slash_comments_at_the_least_indent_and_back() {
        let mut s = state("    a\n\n        b\n");
        s.apply(C::SelectAll, 0);
        s.apply(C::ToggleLineComment, 0);
        assert_eq!(
            text(&s),
            "    // a\n\n    //     b\n",
            "blank lines stay blank"
        );
        s.apply(C::ToggleLineComment, 0);
        assert_eq!(text(&s), "    a\n\n        b\n");
        let mut plain = EditorState::new("x", EditorConfig::default());
        let outcome = plain.apply(C::ToggleLineComment, 0);
        assert!(!outcome.edited, "no comment token, no edit");
    }

    #[test]
    fn ctrl_brackets_indent_and_outdent_the_line() {
        let mut s = state("a");
        s.apply(C::IndentLines, 0);
        assert_eq!(text(&s), "    a");
        s.apply(C::Outdent, 0);
        assert_eq!(text(&s), "a");
    }

    #[test]
    fn a_bracket_closes_itself_and_the_closer_steps_over() {
        let mut s = state("");
        s.apply(C::Insert("(".into()), 0);
        assert_eq!(text(&s), "()");
        assert_eq!(head(&s), 1);
        s.apply(C::Insert("x".into()), 0);
        s.apply(C::Insert(")".into()), 0);
        assert_eq!(text(&s), "(x)", "typing the closer steps over it");
        assert_eq!(head(&s), 3);
        s.apply(C::Insert(")".into()), 0);
        assert_eq!(text(&s), "(x))", "a closer nobody auto-typed is typed");
    }

    #[test]
    fn backspace_between_a_fresh_pair_deletes_both() {
        let mut s = state("");
        s.apply(C::Insert("[".into()), 0);
        s.apply(C::Backspace, 0);
        assert_eq!(text(&s), "");
    }

    #[test]
    fn a_quote_after_a_word_does_not_pair() {
        let mut s = state("don");
        at(&mut s, 3);
        s.apply(C::Insert("'".into()), 0);
        assert_eq!(text(&s), "don'");
        let mut s = state("x = ");
        at(&mut s, 4);
        s.apply(C::Insert("\"".into()), 0);
        assert_eq!(text(&s), "x = \"\"");
    }

    #[test]
    fn a_bracket_before_a_word_does_not_pair() {
        let mut s = state("word");
        at(&mut s, 0);
        s.apply(C::Insert("(".into()), 0);
        assert_eq!(text(&s), "(word");
    }

    #[test]
    fn an_opener_over_a_selection_wraps_it() {
        let mut s = state("a b");
        s.apply(C::Select { ranges: vec![2..3] }, 0);
        s.apply(C::Insert("(".into()), 0);
        assert_eq!(text(&s), "a (b)");
        assert_eq!(
            s.selections().primary().range(),
            3..4,
            "the word stays selected"
        );
    }

    #[test]
    fn auto_close_off_types_plainly() {
        let mut s = EditorState::new(
            "",
            EditorConfig {
                auto_close: false,
                ..EditorConfig::default()
            },
        );
        s.apply(C::Insert("(".into()), 0);
        assert_eq!(text(&s), "(");
    }
}
