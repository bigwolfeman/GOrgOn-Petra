//! The only crate that knows egui.
//!
//! Petra owns layout; this crate owns the substrate (D-069). It hosts an
//! `eframe` application, paints a petrified frame's placements through
//! `egui::Context::layer_painter`, shapes text into cached galleys behind
//! [`gorgon_petra::layout::ContentMeasure`], and translates platform input at
//! the boundary.
//!
//! It pushes AccessKit nodes from [`gorgon_petra::semantic::SemanticTree`]
//! through [`accesskit::publish`] (T028, FR-027) — the same projection that
//! answers the agent's UI-state query and the driver's finders, so a screen
//! reader and a test can never be told different things. [`host::Host::pass`]
//! calls it once per frame. Delivery is an `egui::Plugin` rather than a plain
//! assignment, because `egui::Context::end_pass` writes its own AccessKit
//! update after the pass and would overwrite ours; `accesskit`'s module doc
//! carries the two source citations for why that is the only seam that works.
//!
//! What still does not work, so nobody reads the above as more than it is: no
//! live window has been run against a real screen reader, and an incoming
//! `AccessKitActionRequest` is still counted by name in
//! [`input::EventTranslator::untranslated`] and dropped. Routing one into
//! effect is a driver-input concern (T030's boundary), not this line's.
//!
//! egui's `Ui` layout is never used. No egui type appears on Petra's authoring
//! surface, and the `petra-boundary` gate walks `gorgon-petra`'s resolved
//! dependency graph — not just its manifest — asserting no egui-family crate
//! is reachable from it by any path.

pub mod invariant;

pub mod accesskit;
pub mod draw;
pub mod focus_caret;
pub mod fonts;
pub mod host;
pub mod image;
pub mod inject;
pub mod input;
pub mod paint;
#[cfg(not(target_arch = "wasm32"))]
mod scene_cache;
pub mod schedule;
pub mod shadow;
pub mod text;
pub mod triangle;
