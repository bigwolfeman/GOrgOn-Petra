//! `list_row` — one selectable entry in a list.

use super::text::text;
use super::tokens::{
    LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SPACING_03, SPACING_04,
    SPACING_05, SURFACE_BASE, t,
};
use super::{pad, stack};
use crate::geom::Axis;
use crate::tree::{FocusFigure, Interaction, Key, Role, TextWrap, ViewNode};

/// The block extent one [`list_row`] takes at the shipped theme, in logical
/// units: one `typography.body` line (20) plus the row's own `spacing-04`
/// (12) above and below.
///
/// # Why this is public, and why it is a number rather than a call
///
/// A virtualized list has to tell the engine how tall a row is before it has
/// measured one (`Props::estimated_extent`). The inspector's fiber list wrote
/// that number out by hand, in a comment reading "one line of
/// `typography.body` (20) plus `list_row`'s own `spacing.xs` above and below
/// (4 + 4)" — true when it was written, stale the day this file moved its
/// padding to `$spacing-04` for the reason two comments below. The list then
/// declared 28 for rows that measure 44, and every row was drawn 16 units on
/// top of the row above it, in the shipped demo.
///
/// So the number lives here, beside the padding it is derived from, and the
/// list names it instead of restating it.
/// [`tests::the_declared_extent_matches_the_padding_this_file_actually_binds`]
/// resolves both halves out of the shipped theme and off the constructed node
/// itself, so changing the padding fails a test in this file rather than
/// silently mis-sizing somebody else's list.
///
/// It is a constant rather than a function of a theme snapshot because
/// `estimated_extent` is declared at view-build time, where no snapshot is in
/// hand. A theme that rescales its type or its spacing wants the derivation
/// recomputed; the test below is what says so out loud.
pub const LIST_ROW_EXTENT: f32 = 44.0;

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

    // One line, elided. A list row is a fixed-height thing — Carbon's
    // `.cds--contained-list-item` is, and a virtualizer needs it to be, since
    // `estimated_extent` is one number for the whole list. Left wrapping, the
    // inspector's fiber list grew rows to two and three lines for the fibers
    // with long paths, and no single estimate could describe it. What a row
    // cannot show, the detail beside it can.
    let mut label_node = text("label", label.clone());
    label_node.props.wrap = Some(TextWrap::Ellipsis);
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![label_node]);
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

#[cfg(test)]
mod tests {
    use super::{LIST_ROW_EXTENT, list_row};
    use crate::token::{TokenValue, shipped};
    use crate::tree::TextWrap;

    /// [`LIST_ROW_EXTENT`] is the derivation, not a remembered number.
    ///
    /// Every input is read back out of the constructed node and the shipped
    /// theme rather than restated here: the padding token names come off
    /// `props.padding`, the type step comes off the label's own `style`, and
    /// both resolve through `shipped::dark()`. Change this file's padding to
    /// another step and this fails naming the new total, which is the whole
    /// point — a stale copy of this arithmetic in the inspector's fiber list
    /// is what drew thirteen rows on top of each other.
    #[test]
    fn the_declared_extent_matches_the_padding_this_file_actually_binds() {
        let node = list_row("row", "a fiber", false);
        let theme = shipped::dark();
        let gap = |name: &crate::token::TokenName| match theme.value(name) {
            Some(TokenValue::Spacing(units)) => *units,
            other => panic!("{name} is not a spacing token: {other:?}"),
        };

        let padding = node.props.padding.as_ref().expect("a list row is padded");
        let top = gap(padding.top.as_ref().expect("a top inset"));
        let bottom = gap(padding.bottom.as_ref().expect("a bottom inset"));

        let label = node.children.first().expect("a list row has a label");
        let style = label
            .props
            .style
            .as_ref()
            .expect("the label names a type step");
        let line = match theme.value(style) {
            Some(TokenValue::Typography(value)) => value.line_height,
            other => panic!("{style} is not a typography token: {other:?}"),
        };

        assert_eq!(
            line + top + bottom,
            LIST_ROW_EXTENT,
            "a row is {line} of type inside {top}/{bottom} of padding, which is \
             {}, and LIST_ROW_EXTENT still says {LIST_ROW_EXTENT}",
            line + top + bottom
        );
    }

    /// A row is one line. `estimated_extent` is one number for a whole list,
    /// so a row that can grow to two lines has no honest estimate.
    #[test]
    fn a_rows_label_is_one_elided_line() {
        let node = list_row("row", "a very long fiber path that will not fit", false);
        let label = node.children.first().expect("a list row has a label");
        assert_eq!(label.props.wrap, Some(TextWrap::Ellipsis));
    }
}
