//! Carbon Data table (slice-b). No column resize.
//!
//! Anatomy (usage page + `_data-table.scss`), the parts this file ships:
//! 1. Container — [`Role::Table`].
//! 2. Header row — [`Role::Row`] of [`Role::Cell`]s, not interactive.
//!    Sortable headers are a [`Role::Button`] labelled `"Sort {name}"`
//!    with [`Semantics.value`] `"ascending"` / `"descending"` (the word
//!    is the second channel; arrows are not the only one).
//! 3. Body rows — [`data_table_row`]: [`Role::Row`], selectable,
//!    `Semantics.selected` plus [`LAYER_SELECTED`].
//! 4. Expandable row — [`data_table_row_expandable`]: `Semantics.expanded`
//!    plus the word `"expanded"` / `"collapsed"`, body child only while
//!    open.
//!
//! Five row heights (style-page Rows table): xs 24, sm 32, md 40, lg 48,
//! xl 64. Petra default is md (40), matching [`SIZE_MD`]. Carbon's
//! unmodified class is lg 48.
//!
//! Omitted, honestly: batch-actions toolbar, sticky header (layout has
//! no sticky), column resize (Carbon v11 does not ship it). Zebra is
//! [`data_table_zebra`].

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SIZE_MD, SPACING_03,
    SPACING_05, SURFACE_BASE, SURFACE_RAISED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{Interaction, Key, Role, Semantics, ViewNode};

/// Carbon extra-small row height.
const HEIGHT_XS: f32 = 24.0;
/// Carbon small row height.
const HEIGHT_SM: f32 = 32.0;
/// Carbon large row height (unmodified Carbon default).
const HEIGHT_LG: f32 = 48.0;
/// Carbon extra-large row height.
const HEIGHT_XL: f32 = 64.0;

const _: () = assert!(HEIGHT_XS == 24.0);
const _: () = assert!(HEIGHT_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(HEIGHT_LG == 48.0);
const _: () = assert!(HEIGHT_XL == 64.0);

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];
const SORT_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

#[derive(Clone, Copy)]
enum RowSize {
    Xs,
    Sm,
    Md,
    Lg,
    Xl,
}

impl RowSize {
    fn height(self) -> f32 {
        match self {
            Self::Xs => HEIGHT_XS,
            Self::Sm => HEIGHT_SM,
            Self::Md => SIZE_MD,
            Self::Lg => HEIGHT_LG,
            Self::Xl => HEIGHT_XL,
        }
    }
}

/// Data table: header + rows under [`Role::Table`].
pub fn data_table(key: impl Into<Key>, header: Vec<ViewNode>, rows: Vec<ViewNode>) -> ViewNode {
    let mut children = vec![header_row("header", header, RowSize::Md)];
    children.extend(rows.into_iter().map(ensure_row));
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics = Semantics {
        role: Some(Role::Table),
        ..Semantics::default()
    };
    node
}

/// [`data_table`] with zebra striping: odd body rows take [`SURFACE_RAISED`].
///
/// Even rows keep the [`SURFACE_BASE`] [`data_table_row`] already bound.
/// Selected still wins through `background@selected`.
pub fn data_table_zebra(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let rows = rows
        .into_iter()
        .enumerate()
        .map(|(i, mut row)| {
            if i % 2 == 1 {
                row.props
                    .tokens
                    .insert("background".into(), t(SURFACE_RAISED));
            }
            row
        })
        .collect();
    data_table(key, header, rows)
}

/// One selectable body row at md (40).
pub fn data_table_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Md)
}

/// [`data_table_row`] at xs (24).
pub fn data_table_row_xs(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Xs)
}

/// [`data_table_row`] at sm (32).
pub fn data_table_row_sm(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Sm)
}

/// [`data_table_row`] at lg (48).
pub fn data_table_row_lg(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Lg)
}

/// [`data_table_row`] at xl (64).
pub fn data_table_row_xl(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Xl)
}

/// Expandable body row at md. `body` is mounted only while `expanded`.
pub fn data_table_row_expandable(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    selected: bool,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    let key = key.into();
    let disclosure = if expanded { "expanded" } else { "collapsed" };
    let mut chevron = text("chevron", disclosure);
    chevron
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut cells_with_caret = vec![chevron];
    cells_with_caret.extend(cells);
    let label = row_label(key.as_str(), &cells_with_caret);

    let head = row_shell("cells", cells_with_caret, RowSize::Md);
    let mut parts = vec![head];
    if expanded {
        let mut panel = stack(
            "body",
            Axis::Vertical,
            None,
            vec![text("body-text", body.into())],
        );
        panel.props.padding = Some(pad(SPACING_05, SPACING_05));
        parts.push(panel);
    }

    let mut node = stack(key, Axis::Vertical, None, parts);
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
    node.semantics.expanded = Some(expanded);
    node
}

/// Sortable header cell: a [`Role::Button`] labelled `"Sort {name}"`.
///
/// `ascending` selects [`Semantics.value`] `"ascending"` / `"descending"`.
/// The same word is visible text, so sort direction is never an arrow
/// alone. The cell role is stamped here so [`as_cell`] will not wrap again.
pub fn data_table_sort_header(
    key: impl Into<Key>,
    name: impl Into<String>,
    ascending: bool,
) -> ViewNode {
    let name = name.into();
    let direction = if ascending { "ascending" } else { "descending" };
    let accessible = format!("Sort {name}");
    let mut caption = text("name", name);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut dir = text("direction", direction);
    dir.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut button = stack(
        "sort",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![caption, dir],
    );
    button.props.align = Some(Align::Center);
    // Resting background: the same ground the sort button sits on
    // (`header_row` binds `SURFACE_RAISED`). Without this, `background@hover`
    // has no resting `background` beneath it and resolves to nothing at
    // rest — the Accordion/Modal/AI-label defect, generalised (see
    // `a_state_decorated_token_always_has_a_resting_binding`).
    button
        .props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    button
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut button = button.interactive(Role::Button, accessible, SORT_INTENTS);
    button.semantics.value = Some(direction.into());

    let mut cell = stack(key, Axis::Horizontal, None, vec![button]);
    cell.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    cell
}

fn data_table_row_sized(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    selected: bool,
    size: RowSize,
) -> ViewNode {
    let key = key.into();
    let label = row_label(key.as_str(), &cells);
    let mut node = row_shell(key, cells, size);
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

fn header_row(key: impl Into<Key>, cells: Vec<ViewNode>, size: RowSize) -> ViewNode {
    let mut node = row_shell(key, cells, size);
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node
}

fn row_shell(key: impl Into<Key>, cells: Vec<ViewNode>, size: RowSize) -> ViewNode {
    let cells = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| as_cell(i, cell))
        .collect();
    let mut node = stack(key, Axis::Horizontal, None, cells);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.constraints.vertical.min = Some(size.height());
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
        header_row(node.key.clone(), vec![node], RowSize::Md)
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
    use super::{
        HEIGHT_LG, HEIGHT_SM, HEIGHT_XL, HEIGHT_XS, SIZE_MD, data_table, data_table_row,
        data_table_row_expandable, data_table_row_lg, data_table_row_sm, data_table_row_xl,
        data_table_row_xs, data_table_sort_header, data_table_zebra,
    };
    use crate::component::text::text;
    use crate::component::tokens::{LAYER_SELECTED, SURFACE_BASE, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn no_drag(node: &ViewNode) {
        assert!(
            !node.interactions.contains(&Interaction::Drag),
            "data table `{}` declared Drag; Carbon v11 has no column resize",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    fn sample_table() -> ViewNode {
        data_table(
            "jobs",
            vec![text("h0", "Name"), text("h1", "Status")],
            vec![data_table_row(
                "r0",
                vec![text("n", "alpha"), text("s", "ready")],
                false,
            )],
        )
    }

    #[test]
    fn data_table_is_a_table_of_rows_and_cells() {
        let node = sample_table();
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert!(node.interactions.is_empty());
        let header = named(&node, "header");
        assert_eq!(header.semantics.role, Some(Role::Row));
        assert!(header.interactions.is_empty());
        assert_eq!(header.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(named(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(header, "c1").semantics.role, Some(Role::Cell));
        let row = named(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(named(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(row, "c1").semantics.role, Some(Role::Cell));
        no_drag(&node);
    }

    #[test]
    fn row_declares_selected_and_is_interactive() {
        let on = data_table_row("r0", vec![text("n", "alpha"), text("s", "ready")], true);
        assert_eq!(on.semantics.role, Some(Role::Row));
        assert!(on.semantics.selected);
        assert_eq!(on.semantics.label.as_deref(), Some("alpha ready"));
        assert!(on.interactions.contains(&Interaction::Click));
        assert!(on.interactions.contains(&Interaction::Focus));
        assert!(on.interactions.contains(&Interaction::Hover));
        assert!(!on.interactions.contains(&Interaction::Drag));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(on.constraints.vertical.min, Some(SIZE_MD));

        let off = data_table_row("r0", vec![text("n", "alpha")], false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }

    #[test]
    fn five_row_heights() {
        let cells = vec![text("n", "x")];
        assert_eq!(
            data_table_row_xs("r", cells.clone(), false)
                .constraints
                .vertical
                .min,
            Some(HEIGHT_XS)
        );
        assert_eq!(HEIGHT_XS, 24.0);
        assert_eq!(
            data_table_row_sm("r", cells.clone(), false)
                .constraints
                .vertical
                .min,
            Some(HEIGHT_SM)
        );
        assert_eq!(HEIGHT_SM, 32.0);
        assert_eq!(
            data_table_row("r", cells.clone(), false)
                .constraints
                .vertical
                .min,
            Some(SIZE_MD)
        );
        assert_eq!(
            data_table_row_lg("r", cells.clone(), false)
                .constraints
                .vertical
                .min,
            Some(HEIGHT_LG)
        );
        assert_eq!(HEIGHT_LG, 48.0);
        assert_eq!(
            data_table_row_xl("r", cells, false)
                .constraints
                .vertical
                .min,
            Some(HEIGHT_XL)
        );
        assert_eq!(HEIGHT_XL, 64.0);
    }

    #[test]
    fn sort_header_is_a_button_labelled_sort_name() {
        let cell = data_table_sort_header("col-name", "Name", true);
        assert_eq!(cell.semantics.role, Some(Role::Cell));
        let button = named(&cell, "sort");
        assert_eq!(button.semantics.role, Some(Role::Button));
        assert_eq!(button.semantics.label.as_deref(), Some("Sort Name"));
        assert_eq!(button.semantics.value.as_deref(), Some("ascending"));
        assert_eq!(
            named(button, "direction").props.text.as_deref(),
            Some("ascending")
        );
        assert!(button.interactions.contains(&Interaction::Click));
        assert!(!button.interactions.contains(&Interaction::Drag));
        assert_eq!(
            token(button, "background"),
            Some(SURFACE_RAISED),
            "the sort button needs a resting `background` under its \
             `background@hover`, or the hover binding resolves to nothing \
             and paints silent"
        );

        let desc = data_table_sort_header("col-name", "Name", false);
        assert_eq!(
            named(&desc, "sort").semantics.value.as_deref(),
            Some("descending")
        );
        assert_eq!(
            named(&desc, "direction").props.text.as_deref(),
            Some("descending")
        );

        let table = data_table(
            "jobs",
            vec![
                data_table_sort_header("h0", "Name", true),
                text("h1", "Status"),
            ],
            vec![],
        );
        let header = named(&table, "header");
        assert_eq!(named(header, "h0").semantics.role, Some(Role::Cell));
        assert_eq!(named(header, "c1").semantics.role, Some(Role::Cell));
        no_drag(&table);
    }

    #[test]
    fn expandable_row_mounts_body_only_when_open() {
        let open =
            data_table_row_expandable("r0", vec![text("n", "alpha")], false, true, "more detail");
        assert_eq!(open.semantics.role, Some(Role::Row));
        assert_eq!(open.semantics.expanded, Some(true));
        assert_eq!(
            named(&open, "chevron").props.text.as_deref(),
            Some("expanded")
        );
        assert_eq!(
            named(&open, "body-text").props.text.as_deref(),
            Some("more detail")
        );
        no_drag(&open);

        let shut =
            data_table_row_expandable("r0", vec![text("n", "alpha")], false, false, "more detail");
        assert_eq!(shut.semantics.expanded, Some(false));
        assert_eq!(
            named(&shut, "chevron").props.text.as_deref(),
            Some("collapsed")
        );
        assert!(
            shut.children.iter().all(|c| c.key.as_str() != "body"),
            "collapsed row must not mount a body child"
        );
    }

    #[test]
    fn zebra_alternates_raised_on_odd_rows() {
        let node = data_table_zebra(
            "jobs",
            vec![text("h0", "Name")],
            vec![
                data_table_row("r0", vec![text("n", "a")], false),
                data_table_row("r1", vec![text("n", "b")], false),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert_eq!(token(named(&node, "r0"), "background"), Some(SURFACE_BASE));
        assert_eq!(
            token(named(&node, "r1"), "background"),
            Some(SURFACE_RAISED)
        );
        no_drag(&node);
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn sample_table_with_states() -> ViewNode {
        data_table(
            "jobs",
            vec![
                data_table_sort_header("h0", "Name", true),
                text("h1", "Status"),
            ],
            vec![
                data_table_row("r0", vec![text("n0", "alpha"), text("s0", "ready")], false),
                data_table_row("r1", vec![text("n1", "bravo"), text("s1", "ready")], true),
                data_table_row_xs("r2", vec![text("n2", "charlie")], false),
                data_table_row_lg("r3", vec![text("n3", "delta")], false),
                data_table_row_expandable(
                    "r4",
                    vec![text("n4", "echo")],
                    false,
                    true,
                    "more detail",
                ),
                super::super::disabled(data_table_row("r5", vec![text("n5", "foxtrot")], false)),
            ],
        )
    }

    /// Check C/D: header (with a sort button), every row height, an
    /// expanded expandable row, and a disabled row all place with real
    /// rects, none of them outside their row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(sample_table_with_states());
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
            }
        }
    }

    /// Check F: enabled rows and the sort button are reachable; a disabled
    /// row is not.
    #[test]
    fn rows_and_the_sort_button_are_reachable_unless_disabled() {
        let frame = petrify_lone(sample_table_with_states());
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        for suffix in ["/r0", "/r1", "/r2", "/r3", "/r4"] {
            let p = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"));
            assert!(
                order.iter().any(|o| o == &p.id),
                "{suffix} declares Focus but is not in focus order"
            );
        }
        let sort = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/h0/sort"))
            .expect("the sort button is placed");
        assert!(
            order.iter().any(|o| o == &sort.id),
            "the sort button declares Focus but is not in focus order"
        );
        let disabled_row = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/r5"))
            .expect("the disabled row is placed");
        assert!(
            !order.iter().any(|o| o == &disabled_row.id),
            "a disabled row must not be reachable"
        );
    }

    /// Check E: every cell's text against the resting fill of the row it is
    /// read on — header (raised), a plain body row (base), a selected row
    /// (the selected layer), and a zebra-striped odd row (raised) — read
    /// through `Props.opacity`.
    #[test]
    fn cell_text_clears_aa_contrast_against_its_own_rows_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let table = data_table_zebra(
                "jobs",
                vec![text("h0", "Name")],
                vec![
                    data_table_row("r0", vec![text("n0", "alpha")], false),
                    data_table_row("r1", vec![text("n1", "bravo")], false),
                    data_table_row("r2", vec![text("n2", "charlie")], true),
                ],
            );
            for row_key in ["header", "r0", "r1", "r2"] {
                let row = named(&table, row_key);
                let bg_name = row
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{row_key} binds no resting background"));
                let bg = color(&theme, bg_name.as_str());
                fn walk_text(node: &ViewNode, bg: ColorValue, theme: &Theme, min: f32, row: &str) {
                    if node.props.text.is_some()
                        && let Some(fg_name) = node.props.tokens.get("foreground")
                    {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        let fg = color(theme, fg_name.as_str()).faded(opacity).over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= min,
                            "{row}/{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                            node.key,
                            fg_name.as_str()
                        );
                    }
                    for child in &node.children {
                        walk_text(child, bg, theme, min, row);
                    }
                }
                walk_text(row, bg, &theme, MIN_TEXT_CONTRAST, row_key);
            }
        }
    }
}
