//! A text editor's model: buffer, selections, transactions, history,
//! motions, commands, and a keymap (spec 006).
//!
//! Generic toolkit code: nothing here knows about files, the daemon, syntax,
//! or egui. An application owns an [`EditorState`], feeds it
//! [`EditCommand`]s (from [`Keymap`], from pointer presses, from its own
//! menus), reads the buffer and selections to draw, and drains
//! [`EditorState::take_records`] to keep a syntax tree or a diff in step.
//!
//! ```text
//! key ──Keymap──▶ KeyAction::Edit(EditCommand) ─┐
//! text event ───▶ EditCommand::Insert ──────────┼─▶ EditorState::apply
//! pointer ──────▶ EditCommand::PlaceCaret … ────┘        │
//!                                    Transaction ◀───────┘
//!                                     │      │
//!                     TextBuffer::replace   History (undo groups)
//!                                     │
//!                               EditRecord ─▶ syntax, find, diff
//! ```
//!
//! The decision record is
//! `.agents/notes/implemented/feature/2026-11-27-vs-code-editor-not-a-modal-port.md`
//! in the GOrgOn repository; the plan is `specs/006-editor/plan.md` there.

pub mod buffer;
pub mod command;
mod fenwick;
pub mod find;
pub mod history;
pub mod keymap;
pub mod motion;
pub mod selection;
pub mod state;
pub mod transaction;

pub use buffer::TextBuffer;
pub use command::{EditCommand, Motion};
pub use find::{FindOptions, Finder};
pub use history::{EditKind, History};
pub use keymap::{KeyAction, Keymap};
pub use selection::{Selection, Selections};
pub use state::{EditorConfig, EditorState, Outcome};
pub use transaction::{Assoc, Change, EditRecord, Transaction};
