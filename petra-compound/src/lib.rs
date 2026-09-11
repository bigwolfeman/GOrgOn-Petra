//! The pure triple a view-fiber hosts.
//!
//! Binding contract: `specs/009-petra-design-language/contracts/view-fiber.md`
//! §4.1. The trait is [`Compound`], never `Behaviour`:
//! `gorgon_petra::tree::Behaviour` is already a node's intent-and-phase pair.
//!
//! This crate depends on `gorgon-petra` and `serde` only. No egui, no kernel,
//! no tokio. A compound that needs those is not a compound.

use serde::Serialize;
use serde::de::DeserializeOwned;

/// The pure triple a fiber hosts. No `&self`: a compound is a namespace over
/// three functions and four types, never an object with hidden state.
pub trait Compound {
    /// Everything the author supplies and the compound never changes.
    type Props;

    /// Everything the compound changes and the author never supplies.
    ///
    /// Bound to `Serialize + DeserializeOwned` so reload can round-trip
    /// state through a `StateBlob` (`view-fiber.md` §6). A type that cannot
    /// round-trip fails at compile time, not at reload time.
    type State: Serialize + DeserializeOwned;

    /// The closed set of things that can happen to it.
    type Intent;

    /// The closed set of things it asks its host to do. `()` when it asks
    /// for nothing, which is the common case.
    type Request;

    /// Build the initial state from props. Pure. The host owns the result.
    fn init(props: &Self::Props) -> Self::State;

    /// Pure. Total. No I/O, no clock, no allocation the caller cannot bound.
    /// Returns what the host must do; performs nothing itself.
    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request>;

    /// Pure. Built from tier-1 constructors only.
    fn view(state: &Self::State, props: &Self::Props) -> gorgon_petra::tree::ViewNode;
}

pub mod calendar;
pub mod combobox;
pub mod command;
pub mod data_table;
pub mod filter;
pub mod invariant;
pub mod selection_palette;

pub use calendar::Calendar;
pub use combobox::Combobox;
pub use command::Command;
pub use data_table::DataTable;
pub use filter::filter_indices;
pub use selection_palette::SelectionPalette;
