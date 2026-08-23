//! Petra: GOrgOn's retained layout engine.
//!
//! A view tree is plain nested data. Layout is a proposal/response negotiation
//! that runs top-down and answers bottom-up. The output is a *petrified frame*:
//! one placement per node, plus a sequence number and a content digest that
//! make the frame verifiable by a gate, an agent, or a screenshot consumer.
//!
//! Nothing here knows about a GUI toolkit. Content measurement enters through
//! [`layout::ContentMeasure`] and rows through [`layout::RowSource`];
//! `gorgon-petra-egui` is the only crate that implements them against egui.
//!
//! The binding documents are `specs/003-petra-layout-engine/contracts/`. Where
//! this crate and a contract disagree, the contract wins.

pub mod invariant;

pub mod anim;
pub mod cache;
pub mod component;
pub mod focus;
pub mod frame;
pub mod geom;
pub mod input;
pub mod layout;
pub mod semantic;
pub mod testing;
pub mod token;
pub mod tree;

pub use frame::{PetrifiedFrame, Viewport, petrify};
pub use geom::{Align, Axis, Point, Rect, Scale, Size};
pub use input::{InputEvent, KeyCode, Modifiers, PointerButton, Route};
pub use layout::{LayoutCtx, LayoutState, Proposal, SizeProposal};
pub use tree::{NodeKind, Registry, ViewNode, validate};
