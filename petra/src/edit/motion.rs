//! Caret motions over a [`TextBuffer`].
//!
//! Columns for vertical moves are drawn cells: one per `char`, a tab to the
//! next stop of the tab width. That is the same column the line rows draw
//! (tabs expand to spaces there), so Up and Down land under the caret on
//! screen, not under the same byte.
//!
//! Word motions use VS Code's classes: word characters (alphanumerics and
//! `_`), punctuation runs, and whitespace. A line end is a stop of its own:
//! a word move at the end of a line goes to the start of the next, and never
//! past it in one press.

use super::buffer::TextBuffer;

/// The class of a character for word motions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharClass {
    /// Spaces and tabs (not the line end).
    Space,
    /// `\n`.
    LineEnd,
    /// Letters, digits, `_`.
    Word,
    /// Everything else.
    Punct,
}

/// The class of `c`.
#[must_use]
pub fn class_of(c: char) -> CharClass {
    if c == '\n' {
        CharClass::LineEnd
    } else if c.is_whitespace() {
        CharClass::Space
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punct
    }
}

/// The end of the next word to the right of `at` (Ctrl+Right).
#[must_use]
pub fn word_right(buffer: &TextBuffer, at: usize) -> usize {
    let mut pos = at;
    let Some(c) = buffer.char_at(pos) else {
        return pos;
    };
    if class_of(c) == CharClass::LineEnd {
        return pos + 1;
    }
    while let Some(c) = buffer.char_at(pos) {
        if class_of(c) != CharClass::Space {
            break;
        }
        pos += c.len_utf8();
    }
    let Some(first) = buffer.char_at(pos) else {
        return pos;
    };
    let class = class_of(first);
    if class == CharClass::LineEnd {
        return pos;
    }
    while let Some(c) = buffer.char_at(pos) {
        if class_of(c) != class {
            break;
        }
        pos += c.len_utf8();
    }
    pos
}

/// The start of the word to the left of `at` (Ctrl+Left).
#[must_use]
pub fn word_left(buffer: &TextBuffer, at: usize) -> usize {
    let mut pos = at;
    let Some(c) = buffer.char_before(pos) else {
        return pos;
    };
    if class_of(c) == CharClass::LineEnd {
        return pos - 1;
    }
    while let Some(c) = buffer.char_before(pos) {
        if class_of(c) != CharClass::Space {
            break;
        }
        pos -= c.len_utf8();
    }
    let Some(last) = buffer.char_before(pos) else {
        return pos;
    };
    let class = class_of(last);
    if class == CharClass::LineEnd {
        return pos;
    }
    while let Some(c) = buffer.char_before(pos) {
        if class_of(c) != class {
            break;
        }
        pos -= c.len_utf8();
    }
    pos
}

/// The word around `at` (a double click): the run of `at`'s class on its
/// line, or the run before `at` when `at` is at a run's end. Whitespace
/// selects its run too, as VS Code does.
#[must_use]
pub fn word_at(buffer: &TextBuffer, at: usize) -> std::ops::Range<usize> {
    let class = buffer
        .char_at(at)
        .map(class_of)
        .filter(|c| *c != CharClass::LineEnd)
        .or_else(|| buffer.char_before(at).map(class_of))
        .filter(|c| *c != CharClass::LineEnd);
    let Some(class) = class else {
        return at..at;
    };
    let mut start = at;
    while let Some(c) = buffer.char_before(start) {
        if class_of(c) != class {
            break;
        }
        start -= c.len_utf8();
    }
    let mut end = at;
    while let Some(c) = buffer.char_at(end) {
        if class_of(c) != class {
            break;
        }
        end += c.len_utf8();
    }
    start..end
}

/// Smart Home: the first non-blank of `at`'s line, or column 0 when the
/// caret is already there (or the line is blank).
#[must_use]
pub fn smart_home(buffer: &TextBuffer, at: usize) -> usize {
    let line = buffer.line_of(at);
    let start = buffer.line_start(line);
    let text = buffer.line(line);
    let indent = text.len() - text.trim_start_matches([' ', '\t']).len();
    let first = start + indent;
    if at == first || indent == text.len() {
        start
    } else {
        first
    }
}

/// Drawn cells of `text` up to its byte `upto`, with tabs to stops of
/// `tab` cells.
#[must_use]
pub fn cells_to(text: &str, upto: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let mut cells = 0;
    for (i, c) in text.char_indices() {
        if i >= upto {
            break;
        }
        cells = if c == '\t' {
            (cells / tab + 1) * tab
        } else {
            cells + 1
        };
    }
    cells
}

/// The byte offset in `text` of the char edge nearest to drawn column
/// `cells` without passing it, clamped to the text's end.
#[must_use]
pub fn byte_at_cells(text: &str, cells: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let mut at = 0;
    for (i, c) in text.char_indices() {
        let next = if c == '\t' {
            (at / tab + 1) * tab
        } else {
            at + 1
        };
        if next > cells {
            // Round to the nearer edge, as a press between two cells does.
            return if cells - at > (next - at) / 2 && next - at > 1 {
                i + c.len_utf8()
            } else {
                i
            };
        }
        at = next;
    }
    text.len()
}

/// The caret one or more lines up (`delta < 0`) or down from `head`, aiming
/// at drawn column `goal` (computed from `head` when `None`). Returns the new
/// head and the goal to keep. Up from the first line goes to its start, down
/// from the last line to its end, as VS Code does.
#[must_use]
pub fn vertical(
    buffer: &TextBuffer,
    head: usize,
    goal: Option<usize>,
    delta: isize,
    tab: usize,
) -> (usize, usize) {
    let line = buffer.line_of(head);
    let start = buffer.line_start(line);
    let goal = goal.unwrap_or_else(|| cells_to(&buffer.line(line), head - start, tab));
    let last = buffer.len_lines() - 1;
    let target = line as isize + delta;
    if target < 0 {
        return (0, goal);
    }
    let target = target as usize;
    if target > last {
        return (buffer.len_bytes(), goal);
    }
    let text = buffer.line(target);
    (
        buffer.line_start(target) + byte_at_cells_floor(&text, goal, tab),
        goal,
    )
}

/// The byte offset of the last char edge at or before drawn column `cells`.
fn byte_at_cells_floor(text: &str, cells: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let mut at = 0;
    for (i, c) in text.char_indices() {
        let next = if c == '\t' {
            (at / tab + 1) * tab
        } else {
            at + 1
        };
        if next > cells {
            return i;
        }
        at = next;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_moves_follow_vs_code_classes() {
        let b = TextBuffer::from_str("let foo_bar = x.y();\n  next");
        assert_eq!(word_right(&b, 0), 3);
        assert_eq!(word_right(&b, 3), 11);
        assert_eq!(word_right(&b, 11), 13);
        assert_eq!(word_left(&b, 11), 4);
        assert_eq!(word_left(&b, 4), 0);
        // A line end is its own stop.
        assert_eq!(word_right(&b, 20), 21);
        assert_eq!(word_left(&b, 23), 21);
        assert_eq!(word_left(&b, 21), 20);
        assert_eq!(word_at(&b, 6), 4..11);
    }

    #[test]
    fn smart_home_toggles_between_indent_and_column_zero() {
        let b = TextBuffer::from_str("x\n    body");
        assert_eq!(smart_home(&b, 10), 6);
        assert_eq!(smart_home(&b, 6), 2);
    }

    #[test]
    fn vertical_moves_keep_the_drawn_column_across_tabs() {
        let b = TextBuffer::from_str("\tab\nabcdefgh\nx");
        // Caret after the tab and `a`: drawn column 5 with tab 4.
        let (head, goal) = vertical(&b, 2, None, 1, 4);
        assert_eq!(goal, 5);
        assert_eq!(head, 4 + 5);
        // A short line clamps, and the goal survives it.
        let (head, goal) = vertical(&b, head, Some(goal), 1, 4);
        assert_eq!(head, b.len_bytes());
        let (head, _) = vertical(&b, head, Some(goal), -1, 4);
        assert_eq!(head, 9);
        assert_eq!(vertical(&b, 1, None, -1, 4).0, 0);
    }

    #[test]
    fn cells_count_tabs_to_stops() {
        assert_eq!(cells_to("a\tb", 3, 4), 5);
        assert_eq!(byte_at_cells("a\tb", 4, 4), 2);
        assert_eq!(byte_at_cells("a\tb", 1, 4), 1);
        assert_eq!(byte_at_cells("abc", 9, 4), 3);
    }
}
