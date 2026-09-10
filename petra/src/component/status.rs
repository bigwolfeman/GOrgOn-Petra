//! `status` — the FR-015 component: colour, shape, and text, always
//! together.
//!
//! The only way into this function is a [`StatusToken`], and
//! `StatusToken::new` already refuses one with no text
//! (`crate::token::status`'s own module doc walks the incident that taught
//! this crate a constructor alone is not a guarantee — it once derived
//! `Deserialize` and let a blank-text status through anyway). Taking the
//! whole, already-validated type as the parameter — rather than a colour
//! token plus a `shape: StatusShape` plus a `text: impl Into<String>` this
//! function could reassemble incorrectly — means a colour-only status
//! cannot reach this function at all. That is gate C1-6: not "discouraged",
//! unrepresentable.
//!
//! # The shape channel, and what it used to be
//!
//! This module used to carry [`StatusShape`] as a corner radius alone:
//! `Circle` bound `shape.corner-full` and the other three bound
//! `shape.corner-sm`. On the 10x10 dot that is a maximum outline deviation
//! of 0.414 logical units between the two — under one device pixel at scale
//! 1.0 — and three of the four variants were the same picture. FR-015 says
//! meaning must not rest on colour alone; the shipped light theme paints
//! `status.degraded` an orange-red and `status.down` a dark red, which is
//! exactly the pair a red-green colourblind operator needs another channel
//! for, and it was exactly the pair that had none.
//!
//! The channel is now two token slots, not one: `silhouette` names the
//! figure ([`crate::token::Silhouette`]) and `radius` rounds its corners.
//! All five variants reach the screen as five different outlines —
//! see [`marker_for`] for the mapping and `gorgon-petra-egui`'s
//! `paint::silhouette_points` for the geometry that draws it.

use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    SILHOUETTE_DIAMOND, SILHOUETTE_OCTAGON, SILHOUETTE_RECT, SILHOUETTE_TRIANGLE, SPACING_03, t,
};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, StatusShape, StatusToken, corner_for};
use crate::tree::{Key, Role, ViewNode};

const DOT: f32 = 10.0;

/// The paint slot naming a node's outline family. The painter's own
/// `SILHOUETTE_SLOT`; spelled here because a slot key is a plain string by
/// design (`Props::tokens`), not a token name this module could import.
const SILHOUETTE_SLOT: &str = "silhouette";
/// The `(radius, silhouette)` token pair one [`StatusShape`] paints as.
///
/// Total over the enum, with no catch-all arm: a fifth variant will not
/// compile until somebody decides what figure it draws, which is the check
/// that stops a new status shape from silently inheriting a square.
///
/// `radius` is `None` for the two polygons on purpose. A triangle and a
/// diamond have no corner radius to round, so binding one would be a
/// declaration the painter ignores — and a slot bound but not drawn is the
/// shape this defect took the first time.
fn marker_for(shape: StatusShape) -> (Option<&'static str>, &'static str) {
    match shape {
        // A full radius on a square box is a disc. The rect family already
        // spans square-to-circle, so a circle needs no figure of its own.
        //
        // FR-022: `Circle` is a `CornerRole::Pill`, which names the stadium
        // rather than arriving at one. A fixed role would also land on
        // `shape.corner-full` at today's 10-unit dot (`Floating`'s 8 clears
        // 10 / 2) and would quietly stop doing so the day the dot grew past
        // 16. `Square` takes `CornerRole::Tiled`, which is always
        // `shape.corner-none` — the dot never had a "tiles/abuts"
        // relationship to anything, but `Tiled` is the only role FR-022
        // offers that stays square at every size, which is what a status
        // square needs.
        StatusShape::Circle => (Some(corner_for(CornerRole::Pill, DOT)), SILHOUETTE_RECT),
        StatusShape::Square => (Some(corner_for(CornerRole::Tiled, DOT)), SILHOUETTE_RECT),
        StatusShape::Triangle => (None, SILHOUETTE_TRIANGLE),
        StatusShape::Diamond => (None, SILHOUETTE_DIAMOND),
        StatusShape::Octagon => (None, SILHOUETTE_OCTAGON),
    }
}

/// A status readout: a coloured, shaped dot beside its text.
///
/// Sets `Role::Status` and the status's own text as the label, so
/// `semantic::audit`'s `StatusNeedsLabel` rule is a real check on every
/// tree that uses this component, not a rule this component happens never
/// to trip.
pub fn status(key: impl Into<Key>, status: &StatusToken) -> ViewNode {
    let key = key.into();
    let (radius, silhouette) = marker_for(status.shape());
    let mut dot = swatch("dot", DOT, DOT, Some(status.name().as_str()), None, radius);
    dot.props
        .tokens
        .insert(SILHOUETTE_SLOT.into(), t(silhouette));
    let label_node = text("label", status.text());
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![dot, label_node],
    );
    // The marker is 10 units; body line-height is 20. Start-align hangs the
    // disc/triangle/square off the top of the words.
    node.props.align = Some(Align::Center);
    node.semantics.role = Some(Role::Status);
    node.semantics.label = Some(status.text().to_owned());
    node
}
