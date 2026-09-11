//! `button_group` — a horizontal row of already-built buttons (spec 009 T027).
//!
//! This is the container only. Our seven button variants and six sizes in
//! [`super::button`] stay. Callers pass `button()` / `primary_button()` /
//! etc. as children; this file does not restyle them and does not grow a
//! button vocabulary of its own.
//!
//! # Anatomy
//!
//! A horizontal [`super::stack`] with a small gap and no other chrome: no
//! fill, no shadow, no four-sided container border. The children already
//! carry their own chrome, role, and label. This node is the row. `Role`
//! has no group entry, and a container with no actions is outside
//! `ActionableNeedsRoleAndLabel`.
//!
//! # Squared inner edges wait on T002
//!
//! A flush-attached group wants [`crate::token::CornerRole::Grouping`] on
//! the outer two corners of the first and last child, and square corners
//! on every inner edge. That is per-corner radius (spec 009 T002). This
//! leaf does not have it. Until the painter can name four corners, inner
//! corners stay at Grouping — this constructor does not rewrite children —
//! and the row gaps by [`super::tokens::SPACING_02`] so two Grouping
//! radii do not collide. Zero-gap flush attachment is T002's job, not a
//! fake we can ship by dropping the gap today.

use super::stack;
use super::tokens::SPACING_02;
use crate::geom::{Align, Axis};
use crate::tree::{Key, ViewNode};

/// A horizontal row of already-built buttons.
///
/// `children` are `button()` / `primary_button()` / etc. nodes. This
/// constructor does not inspect them, does not restyle them, and does
/// not take a variant argument. No role of its own: the buttons carry
/// chrome, role, and label; this node is the row.
///
/// No fill, no shadow, no four-sided border. Gap is [`SPACING_02`] so
/// two Grouping radii do not collide; squared inner edges wait on T002
/// (per-corner radius). Until then, inner corners stay at
/// [`crate::token::CornerRole::Grouping`].
#[must_use]
pub fn button_group(key: impl Into<Key>, children: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_02), children);
    node.props.align = Some(Align::Center);
    node
}
