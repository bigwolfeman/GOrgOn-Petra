//! [`EditorState`]: a buffer, its selections, its history, and the commands
//! that change them.
//!
//! Every edit is built the same way: each selection, in text order, yields
//! one [`Change`] and the caret it leaves (as an offset from the change's
//! start). The changes form one [`Transaction`]; after it applies, the new
//! carets are placed by adding up the size deltas of the changes before
//! them. That is why multi-cursor editing costs nothing extra here: one
//! selection is the case of a set with one member.

use std::ops::Range;

use super::buffer::TextBuffer;
use super::command::{EditCommand, Motion};
use super::history::{EditKind, History};
use super::motion::{self, cells_to};
use super::selection::{Selection, Selections};
use super::transaction::{Assoc, Change, EditRecord, Transaction};

mod lines;

/// The editor's settings (spec 006 FR-026).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorConfig {
    /// Cells per tab stop, and spaces per indent level.
    pub tab_size: usize,
    /// Indent with spaces (true) or a tab.
    pub insert_spaces: bool,
    /// Typing within this many milliseconds merges into one undo group.
    pub undo_merge_ms: u64,
    /// Brackets and quotes close themselves, and typing the closer steps
    /// over it (`editor.auto_close_brackets`).
    pub auto_close: bool,
    /// The language's line comment (`//`, `#`, `--`), set by the host from
    /// the file's grammar. [`None`] makes Ctrl+/ do nothing.
    pub line_comment: Option<&'static str>,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            undo_merge_ms: 1000,
            auto_close: true,
            line_comment: None,
        }
    }
}

impl EditorConfig {
    /// One indent level.
    #[must_use]
    pub fn indent_unit(&self) -> String {
        if self.insert_spaces {
            " ".repeat(self.tab_size.max(1))
        } else {
            "\t".to_owned()
        }
    }
}

/// What one command did, for the view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// The text changed.
    pub edited: bool,
    /// The selections changed (always true when edited).
    pub moved: bool,
    /// Text for the system clipboard (copy, cut).
    pub clipboard: Option<String>,
}

/// One document's editing state. See the module docs.
#[derive(Clone, Debug)]
pub struct EditorState {
    buffer: TextBuffer,
    selections: Selections,
    history: History,
    config: EditorConfig,
    /// Lines a page move travels; the view sets it from its height.
    page_lines: usize,
    /// Increments on every change to the text.
    version: u64,
    /// Changes since the consumer last took them ([`Self::take_records`]).
    records: Vec<EditRecord>,
    /// The last copy or cut, and whether it was whole lines; a paste of the
    /// same text with empty selections goes in above the caret line.
    last_copy: Option<(String, bool)>,
    /// Where closers typed by auto-close sit, mapped through every edit:
    /// typing that closer there steps over it, and Backspace between the
    /// pair deletes both. Cleared when a caret moves on its own.
    auto_closed: Vec<usize>,
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new("", EditorConfig::default())
    }
}

impl EditorState {
    /// A clean state over `text`, caret at the start.
    #[must_use]
    pub fn new(text: &str, config: EditorConfig) -> Self {
        Self {
            buffer: TextBuffer::from_str(text),
            selections: Selections::default(),
            history: History::new(config.undo_merge_ms),
            config,
            page_lines: 30,
            version: 0,
            records: Vec::new(),
            last_copy: None,
            auto_closed: Vec::new(),
        }
    }

    /// The text.
    #[must_use]
    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    /// The selections.
    #[must_use]
    pub fn selections(&self) -> &Selections {
        &self.selections
    }

    /// The settings.
    #[must_use]
    pub fn config(&self) -> EditorConfig {
        self.config
    }

    /// Change the settings. The history's merge window follows.
    pub fn set_config(&mut self, config: EditorConfig) {
        self.config = config;
        self.history.set_merge_ms(config.undo_merge_ms);
    }

    /// Lines a page move travels.
    pub fn set_page_lines(&mut self, lines: usize) {
        self.page_lines = lines.max(1);
    }

    /// Increments on every change to the text, undo and redo included.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The edit records since the last call, in the order they applied.
    pub fn take_records(&mut self) -> Vec<EditRecord> {
        std::mem::take(&mut self.records)
    }

    /// Whether the text differs from the last [`Self::mark_saved`] (or the
    /// load), by history position.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.history.is_saved()
    }

    /// Record that the current text is what is on disk.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// The history's state id, to remember which state a save wrote.
    #[must_use]
    pub fn state_id(&self) -> u64 {
        self.history.state_id()
    }

    /// Replace the whole text with `text` (a reload from disk) as one
    /// undoable group, keep each caret on its line and column clamped, and
    /// mark the result saved.
    pub fn reload(&mut self, text: &str) {
        let old = self.selections.clone();
        let points: Vec<(usize, usize, usize, usize)> = old
            .iter()
            .map(|s| {
                let (al, ac) = self.buffer.point_of(s.anchor);
                let (hl, hc) = self.buffer.point_of(s.head);
                (al, ac, hl, hc)
            })
            .collect();
        let transaction = Transaction::single(0..self.buffer.len_bytes(), text);
        let inverse = transaction.apply(&mut self.buffer, &mut self.records);
        let place = |b: &TextBuffer, line: usize, col: usize| {
            let line = line.min(b.len_lines() - 1);
            let range = b.line_range(line);
            b.floor_boundary(range.start + col.min(range.len()))
        };
        let items = points
            .iter()
            .map(|&(al, ac, hl, hc)| {
                Selection::new(place(&self.buffer, al, ac), place(&self.buffer, hl, hc))
            })
            .collect();
        self.selections = Selections::from_vec(items, old.primary_index());
        self.history.record(
            transaction,
            inverse,
            &old,
            &self.selections,
            EditKind::Other,
            false,
            0,
        );
        self.history.mark_saved();
        self.version += 1;
    }

    /// Apply `command` at time `now_ms` (for undo grouping).
    pub fn apply(&mut self, command: EditCommand, now_ms: u64) -> Outcome {
        // A caret moved on its own leaves the auto-closed pairs behind.
        if matches!(
            command,
            EditCommand::Move { .. }
                | EditCommand::SelectAll
                | EditCommand::SingleSelection
                | EditCommand::PlaceCaret { .. }
                | EditCommand::SelectWord { .. }
                | EditCommand::SelectLine { .. }
                | EditCommand::DragTo { .. }
                | EditCommand::Select { .. }
        ) {
            self.forget_auto_closed();
        }
        match command {
            EditCommand::Move { motion, extend } => self.move_carets(motion, extend),
            EditCommand::SelectAll => {
                self.selections = Selections::single(Selection::new(0, self.buffer.len_bytes()));
                self.moved()
            }
            EditCommand::SingleSelection => {
                self.selections.collapse_to_primary();
                self.moved()
            }
            EditCommand::Insert(text) => self.insert(&text, now_ms),
            EditCommand::Newline => self.newline(now_ms),
            EditCommand::Backspace => self.delete_back(now_ms),
            EditCommand::Delete => self.delete_forward(now_ms),
            EditCommand::DeleteWordLeft => self.delete_by(motion::word_left, now_ms),
            EditCommand::DeleteWordRight => self.delete_by(motion::word_right, now_ms),
            EditCommand::Indent => self.indent(now_ms),
            EditCommand::Outdent => self.outdent(now_ms),
            EditCommand::IndentLines => self.indent_lines(now_ms),
            EditCommand::MoveLines { down } => self.move_lines(down, now_ms),
            EditCommand::CopyLines { down } => self.copy_lines(down, now_ms),
            EditCommand::DeleteLines => self.delete_lines(now_ms),
            EditCommand::ToggleLineComment => self.toggle_line_comment(now_ms),
            EditCommand::Undo => {
                self.forget_auto_closed();
                self.undo()
            }
            EditCommand::Redo => {
                self.forget_auto_closed();
                self.redo()
            }
            EditCommand::Copy => Outcome {
                clipboard: Some(self.copy_text()),
                ..Outcome::default()
            },
            EditCommand::Cut => self.cut(now_ms),
            EditCommand::Paste(text) => self.paste(&text, now_ms),
            EditCommand::PlaceCaret { at, extend } => {
                let at = self.buffer.floor_boundary(at);
                if extend {
                    let mut p = self.selections.primary();
                    p = p.moved_to(at, true);
                    self.selections = Selections::single(p);
                } else {
                    self.selections = Selections::single(Selection::caret(at));
                }
                self.history.break_group();
                self.moved()
            }
            EditCommand::SelectWord { at } => {
                let range = motion::word_at(&self.buffer, self.buffer.floor_boundary(at));
                self.selections = Selections::single(Selection::new(range.start, range.end));
                self.history.break_group();
                self.moved()
            }
            EditCommand::SelectLine { at } => {
                let line = self.buffer.line_of(at);
                let start = self.buffer.line_start(line);
                let end = self.buffer.line_start(line + 1);
                self.selections = Selections::single(Selection::new(start, end));
                self.history.break_group();
                self.moved()
            }
            EditCommand::DragTo { at } => {
                let at = self.buffer.floor_boundary(at);
                let p = self.selections.primary().moved_to(at, true);
                self.selections = Selections::single(p);
                self.moved()
            }
            EditCommand::ReplaceRanges { edits } => {
                let changes: Vec<Change> = edits
                    .into_iter()
                    .map(|(range, text)| Change { range, text })
                    .collect();
                self.commit_changes(changes, EditKind::Other, false, now_ms)
            }
            EditCommand::Select { ranges } => {
                if ranges.is_empty() {
                    return Outcome::default();
                }
                let primary = ranges.len() - 1;
                let items = ranges
                    .into_iter()
                    .map(|r| Selection::new(r.start, r.end))
                    .collect();
                self.selections = Selections::from_vec(items, primary);
                self.history.break_group();
                self.moved()
            }
        }
    }

    fn moved(&self) -> Outcome {
        Outcome {
            moved: true,
            ..Outcome::default()
        }
    }

    fn move_carets(&mut self, motion: Motion, extend: bool) -> Outcome {
        let tab = self.config.tab_size;
        let page = self.page_lines as isize;
        let buffer = &self.buffer;
        self.selections.map(|s| {
            let collapse = !extend && !s.is_empty();
            match motion {
                Motion::Left if collapse => Selection::caret(s.range().start),
                Motion::Right if collapse => Selection::caret(s.range().end),
                Motion::Left => s.moved_to(buffer.prev_boundary(s.head), extend),
                Motion::Right => s.moved_to(buffer.next_boundary(s.head), extend),
                Motion::WordLeft => s.moved_to(motion::word_left(buffer, s.head), extend),
                Motion::WordRight => s.moved_to(motion::word_right(buffer, s.head), extend),
                Motion::LineStart => s.moved_to(motion::smart_home(buffer, s.head), extend),
                Motion::LineEnd => {
                    let line = buffer.line_of(s.head);
                    s.moved_to(buffer.line_end(line), extend)
                }
                Motion::DocStart => s.moved_to(0, extend),
                Motion::DocEnd => s.moved_to(buffer.len_bytes(), extend),
                Motion::Up | Motion::Down | Motion::PageUp | Motion::PageDown => {
                    let delta = match motion {
                        Motion::Up => -1,
                        Motion::Down => 1,
                        Motion::PageUp => -page,
                        _ => page,
                    };
                    let (head, goal) = motion::vertical(buffer, s.head, s.goal, delta, tab);
                    let mut moved = s.moved_to(head, extend);
                    moved.goal = Some(goal);
                    moved
                }
            }
        });
        self.history.break_group();
        self.moved()
    }

    /// Build one change per selection with `f(selection) -> (change, caret
    /// offset from the change's start)`, apply them as one transaction, and
    /// put each caret where `f` said.
    fn edit_each(
        &mut self,
        kind: EditKind,
        mergeable: bool,
        now_ms: u64,
        mut f: impl FnMut(&TextBuffer, &EditorConfig, Selection) -> Option<(Change, usize)>,
    ) -> Outcome {
        let mut changes: Vec<Change> = Vec::with_capacity(self.selections.len());
        let mut carets = Vec::with_capacity(self.selections.len());
        for s in self.selections.iter() {
            match f(&self.buffer, &self.config, *s) {
                Some((mut change, caret)) => {
                    // Two carets can reach into the same bytes (Backspace to
                    // a tab stop, word deletes); the later change gives way.
                    if let Some(prev) = changes.last()
                        && change.range.start < prev.range.end
                    {
                        change.range.start = prev.range.end;
                        change.range.end = change.range.end.max(change.range.start);
                    }
                    carets.push(Some(caret));
                    changes.push(change);
                }
                None => carets.push(None),
            }
        }
        if changes
            .iter()
            .all(|c| c.range.is_empty() && c.text.is_empty())
        {
            return Outcome::default();
        }
        let before = self.selections.clone();
        let primary = before.primary_index();
        let transaction = Transaction::new(changes.clone());
        let inverse = transaction.apply(&mut self.buffer, &mut self.records);
        self.map_auto_closed(&transaction);
        // New carets: each change's start shifted by the deltas before it.
        let mut delta: isize = 0;
        let mut next_change = changes.iter();
        let mut items = Vec::with_capacity(carets.len());
        for (s, caret) in before.iter().zip(carets) {
            match caret {
                Some(offset) => {
                    let change = next_change.next().expect("one change per caret");
                    let start = change.range.start.wrapping_add_signed(delta);
                    items.push(Selection::caret(start + offset));
                    delta += change.text.len() as isize - change.range.len() as isize;
                }
                None => {
                    let head = transaction.map(s.head, Assoc::After);
                    let anchor = transaction.map(s.anchor, Assoc::After);
                    items.push(Selection::new(anchor, head));
                }
            }
        }
        self.selections = Selections::from_vec(items, primary);
        self.history.record(
            transaction,
            inverse,
            &before,
            &self.selections,
            kind,
            mergeable,
            now_ms,
        );
        self.version += 1;
        Outcome {
            edited: true,
            moved: true,
            clipboard: None,
        }
    }

    /// Apply prepared `changes` and map every selection through them (find
    /// and replace, indentation).
    fn commit_changes(
        &mut self,
        changes: Vec<Change>,
        kind: EditKind,
        mergeable: bool,
        now_ms: u64,
    ) -> Outcome {
        let transaction = Transaction::new(changes);
        if transaction.is_empty() {
            return Outcome::default();
        }
        let before = self.selections.clone();
        let inverse = transaction.apply(&mut self.buffer, &mut self.records);
        self.map_auto_closed(&transaction);
        self.selections.map(|s| {
            let r = s.range();
            let (start, end) = (
                transaction.map(r.start, Assoc::Before),
                transaction.map(r.end, Assoc::After),
            );
            let (start, end) = if r.is_empty() {
                let at = transaction.map(s.head, Assoc::After);
                (at, at)
            } else {
                (start, end)
            };
            if s.head >= s.anchor {
                Selection::new(start, end)
            } else {
                Selection::new(end, start)
            }
        });
        self.history.record(
            transaction,
            inverse,
            &before,
            &self.selections,
            kind,
            mergeable,
            now_ms,
        );
        self.version += 1;
        Outcome {
            edited: true,
            moved: true,
            clipboard: None,
        }
    }

    fn insert(&mut self, text: &str, now_ms: u64) -> Outcome {
        if text.is_empty() {
            return Outcome::default();
        }
        if let Some(outcome) = self.auto_close(text, now_ms) {
            return outcome;
        }
        // Typing a word after a space starts a new undo group (FR-014).
        let starts_word = !text.starts_with(char::is_whitespace);
        let after_space = self
            .buffer
            .char_before(self.selections.primary().range().start)
            .is_some_and(char::is_whitespace);
        let mergeable = !text.contains('\n') && !(starts_word && after_space);
        let any_selected = self.selections.iter().any(|s| !s.is_empty());
        let kind = if any_selected {
            EditKind::Other
        } else {
            EditKind::Typing
        };
        self.edit_each(kind, mergeable, now_ms, |_, _, s| {
            Some((
                Change {
                    range: s.range(),
                    text: text.to_owned(),
                },
                text.len(),
            ))
        })
    }

    fn newline(&mut self, now_ms: u64) -> Outcome {
        self.edit_each(EditKind::Other, false, now_ms, |b, config, s| {
            let range = s.range();
            let line = b.line_of(range.start);
            let line_range = b.line_range(line);
            let text = b.line(line);
            let col = range.start - line_range.start;
            let before = &text[..col];
            let after_end =
                (range.end.min(line_range.end)).max(line_range.start) - line_range.start;
            let after = &text[after_end.max(col)..];
            let indent: String = text
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .collect::<String>();
            let indent = &indent[..indent.len().min(col)];
            // A whitespace-only line loses its indentation when Enter
            // leaves it, as VS Code trims auto-inserted whitespace.
            if before.trim().is_empty() && after.trim().is_empty() && range.end <= line_range.end {
                let insert = format!("\n{indent}");
                return Some((
                    Change {
                        range: line_range.start..range.end.max(line_range.start + col),
                        text: insert.clone(),
                    },
                    insert.len(),
                ));
            }
            let opens = before.trim_end().chars().last();
            let closes = after.chars().next();
            let unit = config.indent_unit();
            match opens {
                Some(open @ ('{' | '(' | '[')) => {
                    let inner = format!("\n{indent}{unit}");
                    let pair = matches!(
                        (open, closes),
                        ('{', Some('}')) | ('(', Some(')')) | ('[', Some(']'))
                    );
                    let text = if pair {
                        format!("{inner}\n{indent}")
                    } else {
                        inner.clone()
                    };
                    Some((Change { range, text }, inner.len()))
                }
                _ => {
                    let text = format!("\n{indent}");
                    let len = text.len();
                    Some((Change { range, text }, len))
                }
            }
        })
    }

    fn delete_back(&mut self, now_ms: u64) -> Outcome {
        if let Some(outcome) = self.delete_auto_pair(now_ms) {
            return outcome;
        }
        let any_selected = self.selections.iter().any(|s| !s.is_empty());
        let kind = if any_selected {
            EditKind::Other
        } else {
            EditKind::Deleting
        };
        self.edit_each(kind, true, now_ms, |b, config, s| {
            if !s.is_empty() {
                return Some((
                    Change {
                        range: s.range(),
                        text: String::new(),
                    },
                    0,
                ));
            }
            let at = s.head;
            if at == 0 {
                return None;
            }
            let line_start = b.line_start(b.line_of(at));
            let lead = b.slice(line_start..at);
            let from = if config.insert_spaces && at > line_start && lead.bytes().all(|c| c == b' ')
            {
                // Back to the previous tab stop inside leading spaces.
                let cols = lead.len();
                let tab = config.tab_size.max(1);
                let stop = (cols - 1) / tab * tab;
                line_start + stop
            } else {
                b.prev_boundary(at)
            };
            Some((
                Change {
                    range: from..at,
                    text: String::new(),
                },
                0,
            ))
        })
    }

    fn delete_forward(&mut self, now_ms: u64) -> Outcome {
        let any_selected = self.selections.iter().any(|s| !s.is_empty());
        let kind = if any_selected {
            EditKind::Other
        } else {
            EditKind::Deleting
        };
        self.edit_each(kind, true, now_ms, |b, _, s| {
            let range = if s.is_empty() {
                s.head..b.next_boundary(s.head)
            } else {
                s.range()
            };
            (!range.is_empty()).then(|| {
                (
                    Change {
                        range,
                        text: String::new(),
                    },
                    0,
                )
            })
        })
    }

    fn delete_by(&mut self, to: impl Fn(&TextBuffer, usize) -> usize, now_ms: u64) -> Outcome {
        self.edit_each(EditKind::Other, false, now_ms, |b, _, s| {
            let range = if s.is_empty() {
                let other = to(b, s.head);
                s.head.min(other)..s.head.max(other)
            } else {
                s.range()
            };
            (!range.is_empty()).then(|| {
                (
                    Change {
                        range,
                        text: String::new(),
                    },
                    0,
                )
            })
        })
    }

    /// The line range a selection covers for line commands: every line it
    /// touches, minus a last line it only reaches at column 0.
    fn lines_of(&self, s: Selection) -> Range<usize> {
        let r = s.range();
        let first = self.buffer.line_of(r.start);
        let mut last = self.buffer.line_of(r.end);
        if last > first && self.buffer.line_start(last) == r.end {
            last -= 1;
        }
        first..last + 1
    }

    fn indent(&mut self, now_ms: u64) -> Outcome {
        let multi_line = self.selections.iter().any(|s| {
            !s.is_empty()
                && self.buffer.line_of(s.range().start) != self.buffer.line_of(s.range().end)
        });
        if !multi_line {
            let tab = self.config.tab_size.max(1);
            return self.edit_each(EditKind::Other, false, now_ms, |b, config, s| {
                let start = s.range().start;
                let line = b.line_of(start);
                let ls = b.line_start(line);
                let text = if config.insert_spaces {
                    let col = cells_to(&b.slice(ls..start), start - ls, tab);
                    " ".repeat(tab - col % tab)
                } else {
                    "\t".to_owned()
                };
                let len = text.len();
                Some((
                    Change {
                        range: s.range(),
                        text,
                    },
                    len,
                ))
            });
        }
        let unit = self.config.indent_unit();
        let mut lines: Vec<usize> = self
            .selections
            .iter()
            .flat_map(|s| self.lines_of(*s))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        let changes = lines
            .into_iter()
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

    fn outdent(&mut self, now_ms: u64) -> Outcome {
        let tab = self.config.tab_size.max(1);
        let mut lines: Vec<usize> = self
            .selections
            .iter()
            .flat_map(|s| self.lines_of(*s))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        let changes = lines
            .into_iter()
            .filter_map(|l| {
                let text = self.buffer.line(l);
                let n = if text.starts_with('\t') {
                    1
                } else {
                    text.bytes().take(tab).take_while(|b| *b == b' ').count()
                };
                let at = self.buffer.line_start(l);
                (n > 0).then(|| Change {
                    range: at..at + n,
                    text: String::new(),
                })
            })
            .collect();
        self.commit_changes(changes, EditKind::Other, false, now_ms)
    }

    fn undo(&mut self) -> Outcome {
        match self.history.undo(&mut self.buffer, &mut self.records) {
            Some(selections) => {
                self.selections = selections;
                self.version += 1;
                Outcome {
                    edited: true,
                    moved: true,
                    clipboard: None,
                }
            }
            None => Outcome::default(),
        }
    }

    fn redo(&mut self) -> Outcome {
        match self.history.redo(&mut self.buffer, &mut self.records) {
            Some(selections) => {
                self.selections = selections;
                self.version += 1;
                Outcome {
                    edited: true,
                    moved: true,
                    clipboard: None,
                }
            }
            None => Outcome::default(),
        }
    }

    /// The text a copy takes: the selections joined by `\n`, or, when all
    /// are empty, each caret's whole line with its line end.
    fn copy_text(&mut self) -> String {
        let linewise = self.selections.iter().all(Selection::is_empty);
        let text = if linewise {
            let mut lines: Vec<usize> = self
                .selections
                .iter()
                .map(|s| self.buffer.line_of(s.head))
                .collect();
            lines.dedup();
            let mut out = String::new();
            for l in lines {
                out.push_str(&self.buffer.line(l));
                out.push('\n');
            }
            out
        } else {
            self.selections
                .iter()
                .filter(|s| !s.is_empty())
                .map(|s| self.buffer.slice(s.range()).into_owned())
                .collect::<Vec<_>>()
                .join("\n")
        };
        self.last_copy = Some((text.clone(), linewise));
        text
    }

    fn cut(&mut self, now_ms: u64) -> Outcome {
        let text = self.copy_text();
        let linewise = self.last_copy.as_ref().is_some_and(|(_, l)| *l);
        let mut outcome = if linewise {
            let mut lines: Vec<usize> = self
                .selections
                .iter()
                .map(|s| self.buffer.line_of(s.head))
                .collect();
            lines.dedup();
            let last = self.buffer.len_lines() - 1;
            let changes = lines
                .into_iter()
                .map(|l| {
                    let start = self.buffer.line_start(l);
                    let end = self.buffer.line_start(l + 1);
                    // The last line has no line end of its own; take the
                    // one before it so no empty line is left behind.
                    if l == last && l > 0 {
                        Change {
                            range: start - 1..end,
                            text: String::new(),
                        }
                    } else {
                        Change {
                            range: start..end,
                            text: String::new(),
                        }
                    }
                })
                .collect();
            self.commit_changes(changes, EditKind::Other, false, now_ms)
        } else {
            self.edit_each(EditKind::Other, false, now_ms, |_, _, s| {
                (!s.is_empty()).then(|| {
                    (
                        Change {
                            range: s.range(),
                            text: String::new(),
                        },
                        0,
                    )
                })
            })
        };
        outcome.clipboard = Some(text);
        outcome
    }

    fn paste(&mut self, text: &str, now_ms: u64) -> Outcome {
        if text.is_empty() {
            return Outcome::default();
        }
        let linewise = self
            .last_copy
            .as_ref()
            .is_some_and(|(copied, l)| *l && copied == text)
            && self.selections.iter().all(Selection::is_empty);
        if linewise {
            // Whole lines go in above each caret's line; the caret keeps
            // its place in its own line.
            return self.edit_each(EditKind::Other, false, now_ms, |b, _, s| {
                let start = b.line_start(b.line_of(s.head));
                Some((
                    Change {
                        range: start..start,
                        text: text.to_owned(),
                    },
                    text.len() + (s.head - start),
                ))
            });
        }
        // One line per selection when the counts match.
        let lines: Vec<&str> = text.split('\n').collect();
        let count = self.selections.len();
        if count > 1 && lines.len() == count {
            let mut k = 0;
            return self.edit_each(EditKind::Other, false, now_ms, |_, _, s| {
                let piece = lines[k];
                k += 1;
                Some((
                    Change {
                        range: s.range(),
                        text: piece.to_owned(),
                    },
                    piece.len(),
                ))
            });
        }
        self.edit_each(EditKind::Other, false, now_ms, |_, _, s| {
            Some((
                Change {
                    range: s.range(),
                    text: text.to_owned(),
                },
                text.len(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{EditCommand as C, EditorConfig, EditorState, Motion};

    fn state(text: &str) -> EditorState {
        EditorState::new(text, EditorConfig::default())
    }

    fn caret(s: &EditorState) -> usize {
        s.selections().primary().head
    }

    fn typ(s: &mut EditorState, text: &str, now: u64) {
        for c in text.chars() {
            s.apply(C::Insert(c.to_string()), now);
        }
    }

    #[test]
    fn typing_a_word_is_one_undo_group_and_a_space_splits_words() {
        let mut s = state("");
        typ(&mut s, "hello world", 0);
        assert_eq!(s.buffer().to_string(), "hello world");
        s.apply(C::Undo, 1);
        assert_eq!(s.buffer().to_string(), "hello ");
        s.apply(C::Undo, 1);
        assert_eq!(s.buffer().to_string(), "");
        assert!(!s.is_dirty());
        s.apply(C::Redo, 1);
        assert_eq!(s.buffer().to_string(), "hello ");
        assert_eq!(caret(&s), 6);
    }

    #[test]
    fn a_pause_or_a_caret_move_splits_groups() {
        let mut s = state("");
        typ(&mut s, "ab", 0);
        typ(&mut s, "cd", 5_000);
        s.apply(C::Undo, 6_000);
        assert_eq!(s.buffer().to_string(), "ab");
        typ(&mut s, "x", 6_000);
        s.apply(
            C::Move {
                motion: Motion::Left,
                extend: false,
            },
            6_000,
        );
        typ(&mut s, "y", 6_000);
        s.apply(C::Undo, 6_000);
        assert_eq!(s.buffer().to_string(), "abx");
    }

    #[test]
    fn saving_then_undoing_back_is_clean() {
        let mut s = state("x");
        typ(&mut s, "a", 0);
        assert!(s.is_dirty());
        s.mark_saved();
        assert!(!s.is_dirty());
        typ(&mut s, "b", 1);
        assert!(
            s.is_dirty(),
            "typing after a save does not merge into the saved group"
        );
        s.apply(C::Undo, 2);
        assert!(!s.is_dirty());
        assert_eq!(s.buffer().to_string(), "ax");
    }

    #[test]
    fn enter_keeps_indentation_and_opens_brackets() {
        let mut s = state("    fn x() {}");
        s.apply(
            C::PlaceCaret {
                at: 12,
                extend: false,
            },
            0,
        );
        s.apply(C::Newline, 0);
        assert_eq!(s.buffer().to_string(), "    fn x() {\n        \n    }");
        assert_eq!(caret(&s), 21);
        // Enter on the whitespace-only line trims it.
        s.apply(C::Newline, 0);
        assert_eq!(s.buffer().to_string(), "    fn x() {\n\n        \n    }");
    }

    #[test]
    fn backspace_in_leading_spaces_goes_to_the_previous_stop() {
        let mut s = state("      x");
        s.apply(
            C::PlaceCaret {
                at: 6,
                extend: false,
            },
            0,
        );
        s.apply(C::Backspace, 0);
        assert_eq!(s.buffer().to_string(), "    x");
        s.apply(C::Backspace, 0);
        assert_eq!(s.buffer().to_string(), "x");
    }

    #[test]
    fn tab_indents_lines_and_shift_tab_outdents() {
        let mut s = state("a\nb\nc");
        #[allow(clippy::single_range_in_vec_init)]
        let ranges = vec![0..3];
        s.apply(C::Select { ranges }, 0);
        s.apply(C::Indent, 0);
        assert_eq!(s.buffer().to_string(), "    a\n    b\nc");
        s.apply(C::Outdent, 0);
        assert_eq!(s.buffer().to_string(), "a\nb\nc");
        s.apply(
            C::PlaceCaret {
                at: 1,
                extend: false,
            },
            0,
        );
        s.apply(C::Indent, 0);
        assert_eq!(s.buffer().to_string(), "a   \nb\nc");
    }

    #[test]
    fn copy_and_cut_without_a_selection_take_the_line() {
        let mut s = state("one\ntwo\nthree");
        s.apply(
            C::PlaceCaret {
                at: 5,
                extend: false,
            },
            0,
        );
        let out = s.apply(C::Cut, 0);
        assert_eq!(out.clipboard.as_deref(), Some("two\n"));
        assert_eq!(s.buffer().to_string(), "one\nthree");
        s.apply(
            C::Move {
                motion: Motion::DocEnd,
                extend: false,
            },
            0,
        );
        s.apply(C::Paste("two\n".into()), 0);
        assert_eq!(s.buffer().to_string(), "one\ntwo\nthree");
        s.apply(
            C::Move {
                motion: Motion::DocEnd,
                extend: false,
            },
            0,
        );
        let out = s.apply(C::Cut, 0);
        assert_eq!(out.clipboard.as_deref(), Some("three\n"));
        assert_eq!(s.buffer().to_string(), "one\ntwo");
    }

    #[test]
    fn multi_caret_typing_is_one_transaction() {
        let mut s = state("a\nb\nc");
        s.apply(
            C::Select {
                ranges: vec![1..1, 3..3, 5..5],
            },
            0,
        );
        typ(&mut s, "!", 0);
        assert_eq!(s.buffer().to_string(), "a!\nb!\nc!");
        assert_eq!(s.selections().len(), 3);
        s.apply(C::Paste("1\n2\n3".into()), 0);
        assert_eq!(s.buffer().to_string(), "a!1\nb!2\nc!3");
        s.apply(C::Undo, 0);
        s.apply(C::Undo, 0);
        assert_eq!(s.buffer().to_string(), "a\nb\nc");
    }

    #[test]
    fn reload_keeps_the_caret_line_and_is_clean() {
        let mut s = state("aaa\nbbb\nccc");
        s.apply(
            C::PlaceCaret {
                at: 9,
                extend: false,
            },
            0,
        );
        s.reload("x\ny\nzzzz\nw");
        assert_eq!(caret(&s), s.buffer().line_start(2) + 1);
        assert!(!s.is_dirty());
        s.apply(C::Undo, 0);
        assert_eq!(s.buffer().to_string(), "aaa\nbbb\nccc");
        assert!(s.is_dirty());
    }

    #[test]
    fn records_follow_every_edit() {
        let mut s = state("ab");
        s.apply(C::Insert("x".into()), 0);
        let r = s.take_records();
        assert_eq!(r.len(), 1);
        assert_eq!(
            (r[0].start_byte, r[0].old_end_byte, r[0].new_end_byte),
            (0, 0, 1)
        );
        s.apply(C::Undo, 0);
        assert_eq!(s.take_records()[0].old_end_byte, 1);
    }
}
