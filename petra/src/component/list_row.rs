//! `list_row` — one selectable entry in a list.

use super::text::text;
use super::tokens::{
    LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SPACING_03, SPACING_04,
    SPACING_05, SURFACE_BASE, t,
};
use super::{pad, stack};
use crate::geom::Axis;
use crate::tree::{FocusFigure, Interaction, Key, Role, ViewNode};

/// One row of a list: a label, `Role::ListItem`, and a declared `selected`
/// state that is never the only way a reader can tell a row is selected —
/// the row's fill changes too.
///
/// # Four fills, declared once, chosen by the engine
///
/// This function used to pick the fill itself, with an `if selected` around
/// two token names. It now binds all four surfaces a row can have —
/// `background`, `background@hover`, `background@selected`,
/// `background@selected-hover` — and lets `crate::token::state`'s precedence
/// chain decide which is in force (`contracts/interaction-state.md` §4, §6).
///
/// That is not a tidying. A row is the component where the difference between
/// *naming a combination* and *composing states* is visible: hovering a
/// selected row has to land on `layer-selected-hover`, a tone Carbon
/// publishes as its own entry, and neither "the hover tone" nor "the selected
/// tone" is it. An `if` chain here would have needed a third and then a
/// fourth branch, and a component that branches on hover needs to be told
/// when the pointer moves — which is exactly the second hit test by a second
/// owner that FR-009 forbids.
pub fn list_row(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![text("label", label.clone())],
    );
    // Carbon's `.cds--contained-list-item__content`: `padding: $spacing-04
    // $spacing-05`. It was `$spacing-03 $spacing-02` (8 and 4), which put
    // the rows 8px inside the header's own 16px inset — the two lined up
    // nowhere, and a list whose header does not share an edge with its rows
    // reads as two things rather than one.
    node.props.padding = Some(pad(SPACING_05, SPACING_04));
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));

    let mut node = node.interactive(
        Role::ListItem,
        label,
        // `Hover` is what makes the four fills above reachable: without it
        // the engine never hit-tests this row for hover, and two of the four
        // bindings are tokens nothing reads.
        &[Interaction::Focus, Interaction::Click, Interaction::Hover],
    );
    // `BarInside`, for the reason `component::menu` gives: list rows stack
    // flush, so a bar *under* one lands on the next. Seated on the row's own
    // bottom edge the same stripe cannot, and it is still an underline.
    node.semantics.focus_figure = FocusFigure::BarInside;
    node.semantics.selected = selected;
    node
}
