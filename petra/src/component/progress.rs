//! `progress` — a determinate readout: a filled track plus its role, label,
//! and formatted value.

use super::swatch;
use super::tokens::{SHAPE_FULL, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, t};
use crate::tree::{Key, NodeKind, Props, Role, Semantics, TrackSize, ViewNode};

/// A progress bar.
///
/// `value` is clamped into `[0, 1]` once, here, before it drives either the
/// drawn fill (the grid's column weights) or the reported percentage
/// (`Semantics.value`) — so the two can never read as disagreeing with each
/// other the way two independently-rounded copies of the same number could.
///
/// The fill and track colours are ink-on-paper (`TEXT_PRIMARY` on
/// `SURFACE_RAISED`), not a status colour: a `progress` bar says how much
/// of something is done, not whether that something is healthy, and
/// spending a status hue on it would blur that distinction the day an
/// author puts a `status` component next to one.
pub fn progress(key: impl Into<Key>, label: impl Into<String>, value: f32) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let done = value.clamp(0.0, 1.0);
    let rest = (1.0 - done).max(0.001);

    let mut bar_props = Props {
        columns: vec![
            TrackSize::Weight {
                weight: done.max(0.001),
            },
            TrackSize::Weight { weight: rest },
        ],
        ..Props::default()
    };
    bar_props.tokens.insert("border".into(), t(TEXT_MUTED));
    bar_props.tokens.insert("radius".into(), t(SHAPE_FULL));

    let fill = swatch("fill", 0.0, 10.0, Some(TEXT_PRIMARY), None, None);
    let track = swatch("track", 0.0, 10.0, Some(SURFACE_RAISED), None, None);

    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(bar_props)
        .child(fill)
        .child(track);
    node.semantics = Semantics {
        role: Some(Role::Progress),
        label: Some(label),
        value: Some(format!("{:.0}%", done * 100.0)),
        ..Semantics::default()
    };
    node
}
