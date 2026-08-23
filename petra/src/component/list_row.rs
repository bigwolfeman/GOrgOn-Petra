//! `list_row` — one selectable entry in a list.

use super::text::text;
use super::tokens::{SHAPE_SM, SPACING_SM, SPACING_XS, SURFACE_BASE, SURFACE_RAISED, t};
use super::{pad, stack};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, Role, ViewNode};

/// One row of a list: a label, `Role::ListItem`, and a declared `selected`
/// state that is never the only way a reader can tell a row is selected —
/// the row's fill changes too.
pub fn list_row(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_SM),
        vec![text("label", label.clone())],
    );
    node.props.padding = Some(pad(SPACING_SM, SPACING_XS));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            SURFACE_RAISED
        } else {
            SURFACE_BASE
        }),
    );
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));

    let mut node = node.interactive(
        Role::ListItem,
        label,
        &[Interaction::Focus, Interaction::Click],
    );
    node.semantics.selected = selected;
    node
}
