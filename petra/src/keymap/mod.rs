//! Key-binding layer: chords, scoped commands, and the trie resolver that
//! turns a chord sequence into a dispatched [`Binding`].
//!
//! Spec 010 (`specs/010-input-routing/spec.md`) is the source of truth this
//! module answers to. It supplies the **bindings** layer (FR-007…FR-013a)
//! on top of the event vocabulary already in [`crate::input`]
//! ([`crate::input::KeyCode`], [`crate::input::Modifiers`]) — this module
//! reuses that vocabulary rather than redefining it. It also supplies
//! [`reserved::ReservedChords`], the non-empty set of chords a raw claim
//! cannot receive (FR-021, FR-021a); the decision of *when* that set
//! escapes a raw claim is [`crate::input::route_with_reserved`]'s, not this
//! module's. Everything else about routing (FR-003…FR-006, the
//! ancestor-walk repair) or raw pass-through (FR-019, FR-020, FR-022,
//! FR-023) is another leaf of the same spec.
//!
//! # The four types
//!
//! * [`chord::Chord`] — one key plus the modifiers held with it, and the
//!   canonical text form (`ctrl-k`, `shift-esc`) an operator's config file
//!   and a plugin's Lua both write (FR-001).
//! * [`command::CommandName`] — a command name, scoped to the plugin that
//!   defines it (decision 010-5). `plugin.open` and `other.open` are
//!   different names, so two plugins both defining `open` is not a
//!   collision to refuse; it never arises.
//! * [`scope::Scope`] — exactly two shapes a binding can be reachable from
//!   (FR-008): a subtree, or global. No when-clause language.
//! * [`binding::Binding`] — a trigger (a non-empty chord sequence), a
//!   command, a scope, and an owner, refusing at construction the two ways
//!   a binding can be invalid on its own (FR-002, FR-013a).
//!
//! [`resolver::Resolver`] is the trie that turns an arriving chord sequence
//! into a [`resolver::Resolution`], and is where FR-010's conflict refusal
//! lives.
//!
//! # What this module does not do
//!
//! It does not know about focus, the frame tree, or the contribution
//! ledger. [`resolver::Resolver::resolve`] is handed the currently
//! reachable scopes on every call rather than discovering them itself
//! (FR-008a), and FR-009's "gone in the same frame the fiber unloads" and
//! FR-011's "operator configuration outranks a plugin binding" are the
//! caller's lifecycle and displacement policy to apply, layered on
//! [`resolver::Resolver::insert`]'s refuse-on-conflict primitive.

pub mod binding;
pub mod chord;
pub mod command;
pub mod reserved;
pub mod resolver;
pub mod scope;

pub use binding::{Binding, BindingError, Owner};
pub use chord::{Chord, ChordParseError};
pub use command::CommandName;
pub use reserved::{ReservedChords, ReservedChordsError};
pub use resolver::{ConflictError, DEFAULT_TIMEOUT, Resolution, Resolver};
pub use scope::Scope;
