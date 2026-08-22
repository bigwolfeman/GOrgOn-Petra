//! The only crate that knows egui.
//!
//! Petra owns layout; this crate owns the substrate (D-069). It hosts an
//! `eframe` application, paints a petrified frame's placements through
//! `egui::Context::layer_painter`, shapes text into cached galleys behind
//! [`gorgon_petra::layout::ContentMeasure`], and translates platform input at
//! the boundary.
//!
//! It does **not** push AccessKit nodes. There is no `accesskit` dependency
//! here; an incoming `AccessKitActionRequest` is counted by name in
//! [`input::EventTranslator::untranslated`] and dropped. Screen-reader support
//! is task T028, and this line is what an agent reading the crate should find
//! instead of a claim that it already works.
//!
//! egui's `Ui` layout is never used. No egui type appears on Petra's authoring
//! surface, and the `petra-boundary` gate walks `gorgon-petra`'s resolved
//! dependency graph — not just its manifest — asserting no egui-family crate
//! is reachable from it by any path.

pub mod invariant;

pub mod host;
pub mod input;
pub mod paint;
pub mod text;
