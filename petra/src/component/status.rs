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

use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{SHAPE_FULL, SHAPE_SM, SPACING_SM};
use crate::geom::Axis;
use crate::token::{StatusShape, StatusToken};
use crate::tree::{Key, Role, ViewNode};

const DOT: f32 = 10.0;

/// The corner radius standing in for the shape channel.
///
/// Only two silhouettes are reachable through a corner-radius swatch: round
/// ([`StatusShape::Circle`]) and not-round (everything else). That is the
/// same ceiling `gallery.rs`'s own `status_table` comment already records —
/// two of `StatusShape`'s four variants render as tofu in the shipped font,
/// and nothing in the painter consumes `StatusShape` directly — so
/// `Triangle`, `Square`, and `Diamond` all draw as the square swatch today.
/// The text channel is what actually carries the distinction between them;
/// the shape channel here is real but coarser than the type it reads.
fn corner_for(shape: StatusShape) -> &'static str {
    match shape {
        StatusShape::Circle => SHAPE_FULL,
        StatusShape::Triangle | StatusShape::Square | StatusShape::Diamond => SHAPE_SM,
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
    let dot = swatch(
        "dot",
        DOT,
        DOT,
        Some(status.name().as_str()),
        None,
        Some(corner_for(status.shape())),
    );
    let label_node = text("label", status.text());
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_SM),
        vec![dot, label_node],
    );
    node.semantics.role = Some(Role::Status);
    node.semantics.label = Some(status.text().to_owned());
    node
}
