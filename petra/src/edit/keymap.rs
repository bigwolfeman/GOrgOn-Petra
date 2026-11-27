//! Keys to editor actions: a table, not code (spec 006 FR-005, FR-012).
//!
//! [`Keymap::vs_code`] is the default profile. A modal profile, or an
//! operator's rebinding, is another table built the same way; nothing that
//! applies an action reads a key.
//!
//! Printable text does not come through here: it arrives as
//! [`crate::input::InputEvent::Text`] and becomes [`EditCommand::Insert`].
//! Copy, cut, and paste usually arrive as their own events too; their chords
//! are in the table so a host that sends keys instead still works.

use super::command::{EditCommand, Motion};
use crate::input::{KeyCode, Modifiers};

/// What a key asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// A command on the editor state.
    Edit(EditCommand),
    /// Write the document (the host owns the disk).
    Save,
    /// Open find in the document.
    Find,
    /// Open find and replace.
    Replace,
    /// Copy: the host also owns the clipboard, so it hears about it.
    Copy,
    /// Cut.
    Cut,
    /// Leave the editor's raw keys (Escape with one caret and no bar open
    /// is still the editor's; the host decides).
    Escape,
}

/// The modifier combination of a binding, matched exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Mods {
    shift: bool,
    ctrl: bool,
    alt: bool,
}

impl From<Modifiers> for Mods {
    fn from(m: Modifiers) -> Self {
        Self {
            shift: m.shift,
            // Command on macOS plays Control's part.
            ctrl: m.ctrl || m.meta,
            alt: m.alt,
        }
    }
}

/// A table from keys to actions.
#[derive(Clone, Debug, Default)]
pub struct Keymap {
    entries: Vec<(KeyCode, Mods, KeyAction)>,
}

const NONE: Mods = Mods {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: Mods = Mods {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: Mods = Mods {
    shift: false,
    ctrl: true,
    alt: false,
};
const CTRL_SHIFT: Mods = Mods {
    shift: true,
    ctrl: true,
    alt: false,
};

impl Keymap {
    /// The VS Code profile.
    #[must_use]
    pub fn vs_code() -> Self {
        let mut map = Self::default();
        let moves = [
            (KeyCode::Left, NONE, Motion::Left),
            (KeyCode::Right, NONE, Motion::Right),
            (KeyCode::Up, NONE, Motion::Up),
            (KeyCode::Down, NONE, Motion::Down),
            (KeyCode::Left, CTRL, Motion::WordLeft),
            (KeyCode::Right, CTRL, Motion::WordRight),
            (KeyCode::Home, NONE, Motion::LineStart),
            (KeyCode::End, NONE, Motion::LineEnd),
            (KeyCode::Home, CTRL, Motion::DocStart),
            (KeyCode::End, CTRL, Motion::DocEnd),
            (KeyCode::PageUp, NONE, Motion::PageUp),
            (KeyCode::PageDown, NONE, Motion::PageDown),
        ];
        for (key, mods, motion) in moves {
            map.bind(key, mods, KeyAction::Edit(EditCommand::Move { motion, extend: false }));
            let extended = Mods {
                shift: true,
                ..mods
            };
            map.bind(key, extended, KeyAction::Edit(EditCommand::Move { motion, extend: true }));
        }
        let edits = [
            (KeyCode::Backspace, NONE, EditCommand::Backspace),
            (KeyCode::Backspace, SHIFT, EditCommand::Backspace),
            (KeyCode::Delete, NONE, EditCommand::Delete),
            (KeyCode::Backspace, CTRL, EditCommand::DeleteWordLeft),
            (KeyCode::Delete, CTRL, EditCommand::DeleteWordRight),
            (KeyCode::Enter, NONE, EditCommand::Newline),
            (KeyCode::Enter, SHIFT, EditCommand::Newline),
            (KeyCode::Tab, NONE, EditCommand::Indent),
            (KeyCode::Tab, SHIFT, EditCommand::Outdent),
            (KeyCode::Char('a'), CTRL, EditCommand::SelectAll),
            (KeyCode::Char('z'), CTRL, EditCommand::Undo),
            (KeyCode::Char('z'), CTRL_SHIFT, EditCommand::Redo),
            (KeyCode::Char('y'), CTRL, EditCommand::Redo),
        ];
        for (key, mods, command) in edits {
            map.bind(key, mods, KeyAction::Edit(command));
        }
        map.bind(KeyCode::Char('s'), CTRL, KeyAction::Save);
        map.bind(KeyCode::Char('f'), CTRL, KeyAction::Find);
        map.bind(KeyCode::Char('h'), CTRL, KeyAction::Replace);
        map.bind(KeyCode::Char('c'), CTRL, KeyAction::Copy);
        map.bind(KeyCode::Char('x'), CTRL, KeyAction::Cut);
        map.bind(KeyCode::Escape, NONE, KeyAction::Escape);
        map
    }

    fn bind(&mut self, key: KeyCode, mods: Mods, action: KeyAction) {
        self.entries.retain(|(k, m, _)| !(*k == key && *m == mods));
        self.entries.push((key, mods, action));
    }

    /// The action bound to `key` with exactly `modifiers`.
    #[must_use]
    pub fn lookup(&self, key: KeyCode, modifiers: Modifiers) -> Option<&KeyAction> {
        let mods = Mods::from(modifiers);
        let key = match key {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        };
        self.entries
            .iter()
            .find(|(k, m, _)| *k == key && *m == mods)
            .map(|(_, _, a)| a)
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyAction, Keymap};
    use crate::edit::command::{EditCommand, Motion};
    use crate::input::{KeyCode, Modifiers};

    #[test]
    fn the_vs_code_table_maps_the_spec_chords() {
        let map = Keymap::vs_code();
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        let ctrl_shift = Modifiers { shift: true, ..ctrl };
        assert_eq!(
            map.lookup(KeyCode::Right, ctrl_shift),
            Some(&KeyAction::Edit(EditCommand::Move {
                motion: Motion::WordRight,
                extend: true
            }))
        );
        assert_eq!(map.lookup(KeyCode::Char('S'), ctrl), Some(&KeyAction::Save));
        assert_eq!(
            map.lookup(KeyCode::Char('z'), ctrl_shift),
            Some(&KeyAction::Edit(EditCommand::Redo))
        );
        assert_eq!(map.lookup(KeyCode::Char('q'), ctrl), None);
        assert_eq!(
            map.lookup(KeyCode::Tab, Modifiers::NONE),
            Some(&KeyAction::Edit(EditCommand::Indent))
        );
    }
}
