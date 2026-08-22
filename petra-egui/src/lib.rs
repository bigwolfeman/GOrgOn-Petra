//! The only crate that knows egui.
//!
//! Petra owns layout; this crate owns the substrate (D-069). It hosts an
//! `eframe` application, paints a petrified frame's placements through
//! `egui::Context::layer_painter`, shapes text into cached galleys behind
//! [`gorgon_petra::layout::ContentMeasure`], translates platform input at the
//! boundary, and pushes AccessKit nodes from the semantic tree.
//!
//! egui's `Ui` layout is never used. No egui type appears on Petra's authoring
//! surface, and the `petra-boundary` gate asserts `gorgon-petra`'s manifest
//! carries no egui-family dependency.

pub mod invariant;

pub mod host;
pub mod input;
pub mod paint;
pub mod text;
