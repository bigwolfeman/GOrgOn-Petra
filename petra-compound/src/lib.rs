//! The pure triple a view-fiber hosts.
//!
//! Binding contract: `specs/009-petra-design-language/contracts/view-fiber.md`
//! §4.1. The trait is [`Compound`], never `Behaviour`:
//! `gorgon_petra::tree::Behaviour` is already a node's intent-and-phase pair.
//!
//! This crate depends on `gorgon-petra` and `serde` only. No egui, no kernel,
//! no tokio. A compound that needs those is not a compound.

use gorgon_petra::tree::{Intent as EngineIntent, Phase};
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

    /// Map a fired engine intent onto this compound's closed [`Self::Intent`].
    ///
    /// Spec 010 delivers `{contribution, node, intent, phase, value?, weights?}`
    /// on `ui:intent`, where `intent`/`phase` are the engine vocabulary and
    /// `node` is the leaf key the shell attributed (last path segment that
    /// the contribution owns). Compounds whose `view` stamps known keys —
    /// `prev-month`, `day-15`, `bold`, … — implement this so a real click
    /// reaches `update` without stand-in buttons or a richer wire schema.
    ///
    /// Default: `None`. [`gorgon_view_fiber::ViewFiber`] then tries serde on
    /// the payload (whole body, then the `intent` field). A miss is dropped
    /// with a warning, never defaulted.
    ///
    /// `state` is in scope because some mappings (calendar day pick) need
    /// the month on show; `value` / `weights` are the Adjust payloads when
    /// present.
    fn intent_from_fire(
        state: &Self::State,
        node: &str,
        intent: EngineIntent,
        phase: Phase,
        value: Option<f64>,
        weights: Option<&[f64]>,
    ) -> Option<Self::Intent> {
        let _ = (state, node, intent, phase, value, weights);
        None
    }
}

pub mod calendar;
pub mod combobox;
pub mod command;
pub mod data_table;
pub mod filter;
pub mod invariant;
pub mod registry;
pub mod selection_palette;

pub use calendar::Calendar;
pub use combobox::Combobox;
pub use command::Command;
pub use data_table::DataTable;
pub use filter::filter_indices;
pub use selection_palette::SelectionPalette;
