//! A field's own insertion point and selection, as data on the tree.
//!
//! [`TextCursor`] is to an `input`'s caret what [`super::Props::text`] is to
//! its content: a fact the application owns and writes on every frame.
//!
//! # Why this is a tree field and not interaction state
//!
//! [`crate::input::TextSelection`] is interaction state: the host derives it
//! from a pointer drag over glyphs no application can see, and it spans many
//! nodes of a read-only document. A field's caret is the other case. The
//! application already owns the field's text (it receives
//! [`crate::input::InputEvent::Text`] and writes the next string), so it is
//! the only party that knows where the next character goes. A caret the host
//! kept would have to guess what the application did with each keystroke.
//! So the application states it, the same way it states the text, and the
//! engine draws it.
//!
//! # Byte offsets, clamped at paint time
//!
//! Offsets are bytes into `text`, the unit `str` slices in. The engine clamps
//! them to the string and floors them to a character boundary before it
//! paints ([`TextCursor::clamped`]), so a cursor left behind by a shorter
//! string draws at the end instead of panicking inside a layout pass.
//!
//! # The editing helpers are optional
//!
//! [`TextCursor::insert`], [`TextCursor::backspace`], [`TextCursor::delete`]
//! and [`TextCursor::step`] are the single-line edits every field that owns its own typing needs. They
//! keep `text` and the cursor consistent in one call. An application with its
//! own editor model ignores them and writes the two fields itself.

use std::ops::Range;

use serde::{Deserialize, Serialize};

/// The insertion point and the selection inside an `input`'s text.
///
/// `anchor` is where the selection started and `focus` is where the caret
/// is. They are equal for a plain caret. `focus` may sit before `anchor`,
/// which is a selection made leftward; [`Self::range`] orders them.
///
/// An empty selection draws a caret bar at `focus`. A non-empty one draws the
/// selection highlight and no bar, the convention of the platform text fields
/// Carbon's web field inherits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextCursor {
    /// Where the selection started, as a byte offset into the text.
    pub anchor: usize,
    /// Where the caret is, as a byte offset into the text.
    pub focus: usize,
}

/// One caret movement ([`TextCursor::step`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorMotion {
    /// One character toward the start.
    Left,
    /// One character toward the end.
    Right,
    /// To the start of the text.
    Start,
    /// To the end of the text.
    End,
}

impl TextCursor {
    /// A caret at `offset`, with nothing selected.
    #[must_use]
    pub const fn at(offset: usize) -> Self {
        Self {
            anchor: offset,
            focus: offset,
        }
    }

    /// `range` selected, with the caret at its end.
    #[must_use]
    pub const fn spanning(range: Range<usize>) -> Self {
        Self {
            anchor: range.start,
            focus: range.end,
        }
    }

    /// The selected bytes, start before end. Empty for a plain caret.
    #[must_use]
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    /// Whether nothing is selected.
    #[must_use]
    pub const fn is_collapsed(self) -> bool {
        self.anchor == self.focus
    }

    /// Both ends moved into `text`: at most its length, and floored to a
    /// character boundary.
    #[must_use]
    pub fn clamped(self, text: &str) -> Self {
        Self {
            anchor: floor_boundary(text, self.anchor),
            focus: floor_boundary(text, self.focus),
        }
    }

    /// Type `typed` over the selection, or at the caret when nothing is
    /// selected. The caret lands after the inserted text.
    pub fn insert(&mut self, text: &mut String, typed: &str) {
        let range = self.clamped(text).range();
        text.replace_range(range.clone(), typed);
        *self = Self::at(range.start + typed.len());
    }

    /// Delete the selection, or the character before the caret when nothing
    /// is selected. A caret at the start deletes nothing.
    pub fn backspace(&mut self, text: &mut String) {
        let mut range = self.clamped(text).range();
        if range.is_empty() {
            let Some(before) = text[..range.start].chars().next_back() else {
                *self = Self::at(0);
                return;
            };
            range.start -= before.len_utf8();
        }
        text.replace_range(range.clone(), "");
        *self = Self::at(range.start);
    }

    /// Delete the selection, or the character after the caret when nothing
    /// is selected. A caret at the end deletes nothing.
    pub fn delete(&mut self, text: &mut String) {
        let mut range = self.clamped(text).range();
        if range.is_empty() {
            let Some(after) = text[range.end..].chars().next() else {
                *self = Self::at(text.len());
                return;
            };
            range.end += after.len_utf8();
        }
        text.replace_range(range.clone(), "");
        *self = Self::at(range.start);
    }

    /// Move the caret by `motion`. With `extend` the anchor stays and the
    /// selection grows or shrinks. Without it the selection collapses: a
    /// `Left` or `Right` over a selection lands on that side of it, as in
    /// every platform field, and does not move a further character.
    pub fn step(&mut self, text: &str, motion: CursorMotion, extend: bool) {
        let here = self.clamped(text);
        let range = here.range();
        let focus = match motion {
            CursorMotion::Left if !extend && !range.is_empty() => range.start,
            CursorMotion::Right if !extend && !range.is_empty() => range.end,
            CursorMotion::Left => text[..here.focus]
                .chars()
                .next_back()
                .map_or(0, |c| here.focus - c.len_utf8()),
            CursorMotion::Right => text[here.focus..]
                .chars()
                .next()
                .map_or(text.len(), |c| here.focus + c.len_utf8()),
            CursorMotion::Start => 0,
            CursorMotion::End => text.len(),
        };
        *self = if extend {
            Self {
                anchor: here.anchor,
                focus,
            }
        } else {
            Self::at(focus)
        };
    }
}

/// `offset` clamped to `text` and floored to a character boundary.
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut at = offset.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::{CursorMotion, TextCursor};

    #[test]
    fn range_orders_a_leftward_selection() {
        let cursor = TextCursor {
            anchor: 5,
            focus: 2,
        };
        assert_eq!(cursor.range(), 2..5);
        assert!(!cursor.is_collapsed());
        assert!(TextCursor::at(3).is_collapsed());
        assert_eq!(TextCursor::spanning(1..4).focus, 4);
    }

    #[test]
    fn clamped_snaps_into_the_string_and_onto_a_boundary() {
        // `é` is two bytes, so 2 is inside it.
        let cursor = TextCursor {
            anchor: 2,
            focus: 99,
        }
        .clamped("aé");
        assert_eq!(
            cursor,
            TextCursor {
                anchor: 1,
                focus: 3
            }
        );
    }

    #[test]
    fn typing_replaces_the_selection_and_lands_after_the_new_text() {
        let mut text = "notes.md".to_owned();
        let mut cursor = TextCursor::spanning(0..5);
        cursor.insert(&mut text, "draft");
        assert_eq!((text.as_str(), cursor), ("draft.md", TextCursor::at(5)));
        cursor.insert(&mut text, "s");
        assert_eq!((text.as_str(), cursor), ("drafts.md", TextCursor::at(6)));
    }

    #[test]
    fn backspace_takes_the_selection_or_one_character() {
        let mut text = "añb".to_owned();
        let mut cursor = TextCursor::at(3);
        cursor.backspace(&mut text);
        assert_eq!((text.as_str(), cursor), ("ab", TextCursor::at(1)));
        let mut cursor = TextCursor {
            anchor: 2,
            focus: 0,
        };
        cursor.backspace(&mut text);
        assert_eq!((text.as_str(), cursor), ("", TextCursor::at(0)));
        cursor.backspace(&mut text);
        assert_eq!((text.as_str(), cursor), ("", TextCursor::at(0)));
    }

    #[test]
    fn delete_takes_the_selection_or_the_next_character() {
        let mut text = "añb".to_owned();
        let mut cursor = TextCursor::at(1);
        cursor.delete(&mut text);
        assert_eq!((text.as_str(), cursor), ("ab", TextCursor::at(1)));
        cursor.delete(&mut text);
        assert_eq!((text.as_str(), cursor), ("a", TextCursor::at(1)));
        cursor.delete(&mut text);
        assert_eq!((text.as_str(), cursor), ("a", TextCursor::at(1)), "the end");
        let mut cursor = TextCursor {
            anchor: 1,
            focus: 0,
        };
        cursor.delete(&mut text);
        assert_eq!((text.as_str(), cursor), ("", TextCursor::at(0)));
    }

    #[test]
    fn a_step_over_a_selection_collapses_to_its_side() {
        let text = "a.txt";
        let mut cursor = TextCursor::spanning(0..1);
        cursor.step(text, CursorMotion::Right, false);
        assert_eq!(cursor, TextCursor::at(1));
        let mut cursor = TextCursor::spanning(0..1);
        cursor.step(text, CursorMotion::Left, false);
        assert_eq!(cursor, TextCursor::at(0));
        cursor.step(text, CursorMotion::Left, false);
        assert_eq!(cursor, TextCursor::at(0), "the start is a wall");
        cursor.step(text, CursorMotion::End, false);
        assert_eq!(cursor, TextCursor::at(5));
        cursor.step(text, CursorMotion::Right, false);
        assert_eq!(cursor, TextCursor::at(5), "the end is a wall");
    }

    #[test]
    fn an_extending_step_keeps_the_anchor() {
        let text = "añb";
        let mut cursor = TextCursor::at(1);
        cursor.step(text, CursorMotion::Right, true);
        assert_eq!(
            cursor,
            TextCursor {
                anchor: 1,
                focus: 3
            }
        );
        cursor.step(text, CursorMotion::Start, true);
        assert_eq!(
            cursor,
            TextCursor {
                anchor: 1,
                focus: 0
            }
        );
    }

    #[test]
    fn the_wire_shape_is_two_named_offsets() {
        let json = serde_json::to_string(&TextCursor::spanning(1..4)).expect("serializes");
        assert_eq!(json, r#"{"anchor":1,"focus":4}"#);
        assert!(serde_json::from_str::<TextCursor>(r#"{"anchor":1,"focus":4,"x":0}"#).is_err());
    }
}
