//! `checkbox`, `radio`, and `toggle` — the three binary controls.
//!
//! All three carry `Role::Button` plus a declared `selected` state (C13's
//! table): a checked checkbox, a chosen radio, and an "on" toggle are the
//! same semantic fact — one control is presently true — worn by three
//! different shapes. The shape channel is corner radius, which is enough
//! here and only here: a square and a circle are the two ends of the rect
//! family, so unlike `super::status` this pair needs no `silhouette` token
//! to separate them. A checkbox is a sharp square ([`SHAPE_NONE`]), a radio
//! and a toggle's track are full pills ([`SHAPE_FULL`]) — a round control
//! reads as "one choice among several" the way a square one does not, which
//! is the same distinction a browser's own checkbox/radio pair makes.
//!
//! The checkbox was `shape.corner-sm`, not [`SHAPE_NONE`], until the two boxes
//! were measured against each other on the 12x12 box they actually paint
//! into: a 4-unit radius against a full one is 0.828 logical units of
//! outline deviation at its widest, under one device pixel at scale 1.0.
//! The shipped corner ramp has no step between `none` and `sm`, so the only
//! honest way to make the distinction visible was to take the rounding off
//! — which also makes the checkbox a *square*, the shape the doc above
//! already claimed it was.
//!
//! Selection is never carried by fill colour alone: every control here also
//! sets `Semantics.selected`, so the state survives with the colour turned
//! off.
//!
//! # Why these three keep a border when the rest of the library dropped one
//!
//! The 2026-08-25 design pass deleted the outline from the card, the field
//! and the progress rail and replaced it with a tonal step, because in each
//! of those the edge was decoration over a shape that already had a fill.
//! These are the exception, and the reason is structural rather than
//! aesthetic: **an unchecked checkbox and an unselected radio are nothing
//! but their outline.** `fill` above is `None` when the control is off, so
//! taking the border away does not quieten the control, it deletes it. The
//! toggle track is the same argument one step weaker — it has a fill, but
//! that fill is what the knob slides *inside*, and a track a reader cannot
//! find the ends of does not read as a track.
//!
//! What changed for all three is the tone. They bound `text.muted`, a text
//! colour at 10.73:1 on a card; they now bind [`BORDER_SUBTLE`], which is
//! held at or above SC 1.4.11's 3:1 on every layer and pinned *below* both
//! text tones by
//! `crate::token::shipped`'s `the_border_tone_is_visible_everywhere_and_quieter_than_every_text_tone`.

use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, SHAPE_FULL, SHAPE_NONE, SPACING_01, SPACING_02, SPACING_03, SURFACE_RAISED,
    TEXT_PRIMARY, t,
};
use super::{pad, stack, swatch};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, Role, ViewNode};

const BOX: f32 = 12.0;
const TRACK_PAD: f32 = 10.0;

/// Shared shape of `checkbox` and `radio`: a box that fills when selected,
/// beside a label, the whole row focusable and clickable.
fn box_control(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
    box_shape: &str,
) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let fill = selected.then_some(TEXT_PRIMARY);
    let mut row = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![
            swatch("box", BOX, BOX, fill, Some(BORDER_SUBTLE), Some(box_shape)),
            text("label", label.clone()),
        ],
    );
    row.props.padding = Some(pad(SPACING_02, SPACING_01));
    let mut node = row.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click],
    );
    node.semantics.selected = selected;
    node
}

/// A checkbox: an independent on/off choice, drawn as a sharp square.
pub fn checkbox(key: impl Into<Key>, label: impl Into<String>, checked: bool) -> ViewNode {
    box_control(key, label, checked, SHAPE_NONE)
}

/// A radio button: one choice among a group, drawn as a filled circle when
/// selected.
pub fn radio(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    box_control(key, label, selected, SHAPE_FULL)
}

/// A toggle: an on/off choice drawn as a knob sliding inside a pill track.
pub fn toggle(key: impl Into<Key>, label: impl Into<String>, on: bool) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut segments = Vec::new();
    if on {
        segments.push(swatch("pad", TRACK_PAD, BOX, None, None, None));
    }
    segments.push(swatch(
        "knob",
        BOX,
        BOX,
        Some(TEXT_PRIMARY),
        None,
        Some(SHAPE_FULL),
    ));
    if !on {
        segments.push(swatch("pad", TRACK_PAD, BOX, None, None, None));
    }
    let mut track = stack("track", Axis::Horizontal, None, segments);
    track
        .props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    track.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    track.props.tokens.insert("radius".into(), t(SHAPE_FULL));

    let mut row = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![track, text("label", label.clone())],
    );
    row.props.padding = Some(pad(SPACING_02, SPACING_01));
    let mut node = row.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click],
    );
    node.semantics.selected = on;
    node
}
