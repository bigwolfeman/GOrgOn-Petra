//! Carbon Structured list (slice-e).
//!
//! A definition-style table, not [`super::list`] (ordered/unordered
//! markers). Anatomy (`_structured-list.scss`):
//! 1. Container — `display:table` → [`Role::Table`].
//! 2. Header row — [`Role::Row`] of [`Role::Cell`]s. Not interactive.
//! 3. Data rows — [`structured_list_row`]: [`Role::Row`], selectable,
//!    `Semantics.selected` never colour alone.
//!
//! Padding is `$spacing-05` (16) on both axes. Default row min-height is
//! Carbon's 60. The 10-colour tag set is unrelated; this file does not
//! invent hues.

use super::pad;
use super::stack;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_05, SURFACE_BASE, t,
};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, Role, Semantics, ViewNode};

/// Carbon default structured-list row height (style page Size table).
const ROW_HEIGHT: f32 = 60.0;

const _: () = assert!(ROW_HEIGHT == 60.0);

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Structured list: header + rows under [`Role::Table`].
pub fn structured_list(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let mut children = vec![plain_row("header", header)];
    children.extend(rows.into_iter().map(ensure_row));
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics = Semantics {
        role: Some(Role::Table),
        ..Semantics::default()
    };
    node
}

/// One selectable data row. Interactive, [`Role::Row`], cells stamped
/// [`Role::Cell`]. `selected` is a declared fact plus the four-fill set
/// [`super::list_row`] pioneered.
pub fn structured_list_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = row_label(key.as_str(), &cells);
    let mut node = row_shell(key, cells);
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    let mut node = node.interactive(Role::Row, label, ROW_INTENTS);
    node.semantics.selected = selected;
    node
}

fn plain_row(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let mut node = row_shell(key, cells);
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node
}

fn row_shell(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let cells = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| as_cell(i, cell))
        .collect();
    let mut node = stack(key, Axis::Horizontal, None, cells);
    node.props.padding = Some(pad(SPACING_05, SPACING_05));
    node.constraints.vertical.min = Some(ROW_HEIGHT);
    node
}

fn as_cell(index: usize, node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Cell) {
        return node;
    }
    let mut wrap = stack(format!("c{index}"), Axis::Horizontal, None, vec![node]);
    wrap.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    wrap
}

fn ensure_row(node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Row) {
        node
    } else {
        plain_row(node.key.clone(), vec![node])
    }
}

fn row_label(key: &str, cells: &[ViewNode]) -> String {
    let from_cells: Vec<String> = cells
        .iter()
        .map(collect_text)
        .filter(|s| !s.is_empty())
        .collect();
    if from_cells.is_empty() {
        key.to_string()
    } else {
        from_cells.join(" ")
    }
}

fn collect_text(node: &ViewNode) -> String {
    if let Some(text) = node.props.text.as_ref().filter(|s| !s.is_empty()) {
        return text.clone();
    }
    if let Some(label) = node.semantics.label.as_ref().filter(|s| !s.is_empty()) {
        return label.clone();
    }
    node.children
        .iter()
        .map(|child| collect_text(child))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{ROW_HEIGHT, SPACING_05, structured_list, structured_list_row};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED};
    use crate::tree::{Interaction, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn padding_token(node: &ViewNode) -> Option<(&str, &str)> {
        let pad = node.props.padding.as_ref()?;
        Some((pad.left.as_ref()?.as_str(), pad.top.as_ref()?.as_str()))
    }

    #[test]
    fn structured_list_is_a_table_of_rows_and_cells() {
        let node = structured_list(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![structured_list_row(
                "r0",
                vec![text("p", "Alpha"), text("c", "12")],
                false,
            )],
        );
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert!(node.interactions.is_empty());
        assert_ne!(node.semantics.role, Some(Role::List));

        let header = child(&node, "header");
        assert_eq!(header.semantics.role, Some(Role::Row));
        assert!(header.interactions.is_empty());
        assert_eq!(header.constraints.vertical.min, Some(ROW_HEIGHT));
        assert_eq!(ROW_HEIGHT, 60.0);
        assert_eq!(padding_token(header), Some((SPACING_05, SPACING_05)));
        assert_eq!(child(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(header, "c1").semantics.role, Some(Role::Cell));

        let row = child(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(child(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(row, "c1").semantics.role, Some(Role::Cell));
    }

    #[test]
    fn structured_list_row_declares_selected_and_is_interactive() {
        let on = structured_list_row("r0", vec![text("p", "Alpha"), text("c", "12")], true);
        assert_eq!(on.semantics.role, Some(Role::Row));
        assert!(on.semantics.selected);
        assert_eq!(on.semantics.label.as_deref(), Some("Alpha 12"));
        assert!(on.interactions.contains(&Interaction::Click));
        assert!(on.interactions.contains(&Interaction::Focus));
        assert!(on.interactions.contains(&Interaction::Hover));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "border"), Some(BORDER_SUBTLE));
        assert_eq!(padding_token(&on), Some((SPACING_05, SPACING_05)));
        assert_eq!(on.constraints.vertical.min, Some(ROW_HEIGHT));

        let off = structured_list_row("r0", vec![text("p", "Alpha")], false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }
}
