//! The editor's commands: a closed set (spec 006 FR-005).
//!
//! Keys never reach [`super::EditorState`]; a keymap
//! ([`super::keymap`]) turns a key into an [`EditCommand`], and a pointer
//! handler turns a press into one. A modal profile later is another table in
//! front of the same commands.

/// Where a caret move goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// One char left.
    Left,
    /// One char right.
    Right,
    /// Start of the word to the left.
    WordLeft,
    /// End of the word to the right.
    WordRight,
    /// One line up, keeping the drawn column.
    Up,
    /// One line down, keeping the drawn column.
    Down,
    /// Smart Home: first non-blank, then column 0.
    LineStart,
    /// End of the line.
    LineEnd,
    /// Start of the text.
    DocStart,
    /// End of the text.
    DocEnd,
    /// One page up ([`super::EditorState::set_page_lines`]).
    PageUp,
    /// One page down.
    PageDown,
}

/// One editor command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditCommand {
    /// Move every caret; `extend` keeps the anchors.
    Move {
        /// Where.
        motion: Motion,
        /// Shift held.
        extend: bool,
    },
    /// Select the whole text.
    SelectAll,
    /// Back to the primary selection alone (Escape).
    SingleSelection,
    /// Type text at every selection.
    Insert(String),
    /// Enter, with auto-indent.
    Newline,
    /// Delete the selection, or the char before each caret.
    Backspace,
    /// Delete the selection, or the char after each caret.
    Delete,
    /// Delete to the start of the word on the left.
    DeleteWordLeft,
    /// Delete to the end of the word on the right.
    DeleteWordRight,
    /// Tab: indent selected lines, or insert indentation at the caret.
    Indent,
    /// Shift+Tab: outdent the selected lines.
    Outdent,
    /// Undo one group.
    Undo,
    /// Redo one group.
    Redo,
    /// Copy the selections (whole lines when nothing is selected).
    Copy,
    /// Cut the selections (whole lines when nothing is selected).
    Cut,
    /// Paste text from the clipboard.
    Paste(String),
    /// A primary press at byte `at`: a caret there, or the selection
    /// extended to it with Shift.
    PlaceCaret {
        /// Where.
        at: usize,
        /// Shift held.
        extend: bool,
    },
    /// A double press: the word at `at`.
    SelectWord {
        /// Where.
        at: usize,
    },
    /// A triple press: the line at `at`, with its line end.
    SelectLine {
        /// Where.
        at: usize,
    },
    /// A drag with the button down: the primary's caret to `at`.
    DragTo {
        /// Where.
        at: usize,
    },
    /// Replace every selection with its own text (find and replace). Kept
    /// here so a replace is one transaction and one undo group.
    ReplaceRanges {
        /// Byte ranges and their replacements, non-overlapping.
        edits: Vec<(std::ops::Range<usize>, String)>,
    },
    /// Select these ranges (find results).
    Select {
        /// The ranges, the last one primary.
        ranges: Vec<std::ops::Range<usize>>,
    },
}
