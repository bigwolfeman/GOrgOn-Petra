//! Carbon Data table (slice-b). No column resize.
//!
//! Anatomy (usage page + `_data-table.scss` + `09-data-table.png`), the
//! parts this file ships:
//! 1. Container — [`Role::Table`]. No outline of its own: Carbon draws no
//!    box around a table, only one rule under each row.
//! 2. Header row — [`Role::Row`] of [`Role::Cell`]s on [`LAYER_ACCENT`]
//!    (slice-b:75, `thead { background-color: $layer-accent }`), text in
//!    `heading-compact-01` (`TYPOGRAPHY_HEADING_SM`) and `TEXT_PRIMARY`.
//!    Its leading cell is the **select-all** checkbox, a [`Role::Button`]
//!    whose state is derived from the rows: checked when every row is
//!    selected, mixed when some are, empty when none. Sortable headers are
//!    a [`Role::Button`] labelled `"Sort {name}"` with [`Semantics.value`]
//!    `"ascending"` / `"descending"` (the word is the second channel).
//! 3. Body rows — [`data_table_row`]: [`Role::Row`], selectable,
//!    `Semantics.selected` plus [`LAYER_SELECTED`] **plus a checkbox** in
//!    the leading selection column (`td.cds--table-column-checkbox`,
//!    slice-b anatomy 4). The row is the control; the box is its visible
//!    state, which is the channel a red-green colour-blind reader gets when
//!    the selected fill is a few levels off the resting one.
//! 4. Expandable row — [`data_table_row_expandable`]: `Semantics.expanded`
//!    plus a chevron glyph ([`IconMark::ChevronUp`] open,
//!    [`IconMark::ChevronDown`] shut) ahead of the checkbox, body child only
//!    while open. When any row of a table is expandable, every other row
//!    and the header reserve the chevron's width so the columns still line
//!    up (see [`reserve_expand_column`]).
//! 5. Rules — every row, header included, binds `border-bottom`
//!    ([`BORDER_SUBTLE`]): Carbon's `td { border-block-end: 1px solid
//!    $border-subtle-01 }` (slice-b:76). One line per boundary. The old
//!    four-sided `border` binding on each row drew every seam twice and
//!    every column edge once, which is the "grid of boxes" the operator
//!    named twice (`.agents/carbon-waves/ROUND2-DEFECTS.md` row 9).
//!
//! Five row heights (style-page Rows table): xs 24, sm 32, md 40, lg 48,
//! xl 64. **The default is Carbon's own: lg (48)**, `_data-table.scss:147`
//! `tr { block-size: $spacing-09 }` with no size modifier, and what
//! `09-data-table.png` was captured at. The header row always takes the
//! tallest body row's height (docs, "Rows": the column-header row must
//! match the body row size).
//!
//! Omitted, honestly: batch-actions toolbar, sticky header (layout has
//! no sticky), column resize (Carbon v11 does not ship it), the row menu.
//! Body text stays `TEXT_PRIMARY` where Carbon uses `$text-secondary`
//! at rest and `$text-primary` on the selected row; a child's foreground
//! cannot follow its row's state in this engine. Zebra is
//! [`data_table_zebra`].

use std::sync::Arc;

use super::controls::{CheckState, checkbox_box};
use super::icon::{IconMark, IconTone, icon_toned};
use super::pad;
use super::stack;
use super::text::{as_compact_heading, text};
use super::tokens::{
    BORDER_SUBTLE, LAYER_ACCENT, LAYER_ACCENT_HOVER, LAYER_HOVER, LAYER_SELECTED,
    LAYER_SELECTED_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, InsetRefs, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
};

/// Carbon extra-small row height.
const HEIGHT_XS: f32 = 24.0;
/// Carbon small row height.
const HEIGHT_SM: f32 = 32.0;
/// Carbon large row height: the unmodified default (`_data-table.scss:147`).
const HEIGHT_LG: f32 = 48.0;
/// Carbon extra-large row height.
const HEIGHT_XL: f32 = 64.0;

const _: () = assert!(HEIGHT_XS == 24.0);
const _: () = assert!(HEIGHT_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(HEIGHT_LG == 48.0);
const _: () = assert!(HEIGHT_XL == 64.0);

/// The key of every row's leading selection cell, and of the mark inside it.
const SELECT_CELL: &str = "select";
/// The header's select-all control.
const SELECT_ALL: &str = "select-all";
/// The expand chevron's key, and the key of the spacer that stands in for
/// it on a row that is not expandable.
const EXPAND: &str = "expand";
/// The accessible name of the header's select-all checkbox.
const SELECT_ALL_LABEL: &str = "Select all rows";

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
///
/// The header's select-all checkbox reads the rows' `selected` facts; the
/// header's height is the tallest row's; and if any row is expandable the
/// header and every other row reserve the chevron column.
pub fn data_table(key: impl Into<Key>, header: Vec<ViewNode>, rows: Vec<ViewNode>) -> ViewNode {
    let rows: Vec<ViewNode> = rows.into_iter().map(ensure_row).collect();
    let height = rows
        .iter()
        .filter_map(|row| row.constraints.vertical.min)
        .fold(HEIGHT_LG, f32::max);
    let expandable = rows.iter().any(|row| row.semantics.expanded.is_some());
    let rows: Vec<ViewNode> = if expandable {
        rows.into_iter().map(reserve_expand_column).collect()
    } else {
        rows
    };
    let selected = rows.iter().filter(|row| row.semantics.selected).count();
    let all = match selected {
        0 => CheckState::Unchecked,
        n if n == rows.len() => CheckState::Checked,
        _ => CheckState::Mixed,
    };

    let mut children = vec![header_row("header", header, height, all, expandable)];
    children.extend(rows);
    let mut node = stack(key, Axis::Vertical, None, children);
    // Every row is a `Grid` (see `row_shell`) whose column tracks resolve
    // against whatever width `place` offers it. Without `Stretch` here,
    // each row is measured at its own natural content width — the picture
    // showed three different row widths stair-stepping. `Stretch` offers
    // every row the table's own width, so sibling rows resolve the *same*
    // column pixel widths.
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Table),
        ..Semantics::default()
    };
    node
}

/// [`data_table`] with zebra striping: odd body rows take [`LAYER_ACCENT`]
/// (slice-b, "Color > Row": Zebra `$layer-accent`).
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
                    .insert("background".into(), t(LAYER_ACCENT));
            }
            row
        })
        .collect();
    data_table(key, header, rows)
}

/// One selectable body row at Carbon's default height, lg (48).
pub fn data_table_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Lg)
}

/// [`data_table_row`] at xs (24).
pub fn data_table_row_xs(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Xs)
}

/// [`data_table_row`] at sm (32).
pub fn data_table_row_sm(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Sm)
}

/// [`data_table_row`] at md (40).
pub fn data_table_row_md(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Md)
}

/// [`data_table_row`] at lg (48), spelled out; the same row [`data_table_row`]
/// builds.
pub fn data_table_row_lg(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Lg)
}

/// [`data_table_row`] at xl (64).
pub fn data_table_row_xl(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    data_table_row_sized(key, cells, selected, RowSize::Xl)
}

/// Expandable body row at lg. `body` is mounted only while `expanded`.
///
/// The expand chevron sits ahead of the selection checkbox in the leading
/// cell (usage page, "Expandable + selectable": the chevron is always left
/// of the selection control).
pub fn data_table_row_expandable(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    selected: bool,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    let key = key.into();
    // Carbon's `.cds--table-expand__svg` is `ChevronRight` turned 90° shut
    // / 270° open (`_data-table-expandable.scss`), which lands on
    // ChevronDown / ChevronUp, filled `$icon-primary`.
    let chevron = icon_toned(
        EXPAND,
        if expanded {
            IconMark::ChevronUp
        } else {
            IconMark::ChevronDown
        },
        IconTone::Primary,
    );
    let label = row_label(key.as_str(), &cells);

    let head = row_shell(
        "cells",
        selection_cell(Some(chevron), select_mark(selected)),
        cells,
        HEIGHT_LG,
    );
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
    node.props.align = Some(Align::Stretch);
    bind_row_fills(&mut node);
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
    let caption = header_text("name", name);
    let dir = header_text("direction", direction);
    let mut button = stack(
        "sort",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![caption, dir],
    );
    button.props.align = Some(Align::Center);
    // Resting background: the same ground the header row paints
    // (`header_row` binds `LAYER_ACCENT`). Without this, `background@hover`
    // has no resting `background` beneath it and resolves to nothing at
    // rest (see `a_state_decorated_token_always_has_a_resting_binding`).
    // Hover is Carbon's `$layer-accent-hover` (slice-b, "Column header").
    button
        .props
        .tokens
        .insert("background".into(), t(LAYER_ACCENT));
    button
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_ACCENT_HOVER));
    let mut button = button.interactive(Role::Button, accessible, SORT_INTENTS);
    button.semantics.value = Some(direction.into());

    let mut cell = stack(key, Axis::Horizontal, None, vec![button]);
    // `as_cell` (`row_shell`) leaves a pre-built `Role::Cell` node alone,
    // so this cell needs the same `padding-inline: $spacing-05` every
    // other cell gets.
    cell.props.padding = Some(cell_padding_inline());
    cell.props.align = Some(Align::Center);
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
    let mut node = row_shell(
        key,
        selection_cell(None, select_mark(selected)),
        cells,
        size.height(),
    );
    bind_row_fills(&mut node);
    let mut node = node.interactive(Role::Row, label, ROW_INTENTS);
    node.semantics.selected = selected;
    node
}

/// The four fills a selectable row can show, and its one rule.
fn bind_row_fills(node: &mut ViewNode) {
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    node.props
        .tokens
        .insert("border-bottom".into(), t(BORDER_SUBTLE));
}

/// The column-header row: [`LAYER_ACCENT`] fill, `heading-compact-01` text,
/// the select-all checkbox in the leading cell, one rule underneath.
fn header_row(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    height: f32,
    all: CheckState,
    expandable: bool,
) -> ViewNode {
    let cells = cells.into_iter().map(as_compact_heading).collect();
    let spacer = expandable.then(expand_spacer);
    let mut node = row_shell(key, selection_cell(spacer, select_all(all)), cells, height);
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node.props
        .tokens
        .insert("border-bottom".into(), t(BORDER_SUBTLE));
    node.props
        .tokens
        .insert("background".into(), t(LAYER_ACCENT));
    node
}

/// The header's select-all control: a [`Role::Button`] wrapping the
/// tri-state box. `Semantics.selected` is true only when every row is;
/// `Semantics.value` is `"mixed"` when some are, so the third state is
/// declared rather than faked.
fn select_all(all: CheckState) -> ViewNode {
    let mut node = stack(SELECT_ALL, Axis::Horizontal, None, vec![checkbox_box(all)]);
    node.props.align = Some(Align::Center);
    let mut node = node.interactive(Role::Button, SELECT_ALL_LABEL, ROW_INTENTS);
    node.semantics.selected = all == CheckState::Checked;
    if all == CheckState::Mixed {
        node.semantics.value = Some("mixed".into());
    }
    node
}

/// A body row's mark: the box in the row's own state. Not a control — the
/// row is — so it carries no role and no interactions.
fn select_mark(selected: bool) -> ViewNode {
    checkbox_box(if selected {
        CheckState::Checked
    } else {
        CheckState::Unchecked
    })
}

/// The leading cell of every row: an optional expand slot, then the mark.
///
/// Padding is Carbon's `.cds--table-column-checkbox`: 16 start, 8 end,
/// which with the 16 box makes the column's `min-inline-size: 2.5rem` (40,
/// `_data-table.scss:480`) exactly, and puts the next column's text 16
/// further along — `09-data-table.png` has "Name" 56 from the table's
/// edge.
fn selection_cell(expand: Option<ViewNode>, mark: ViewNode) -> ViewNode {
    let mut parts = Vec::with_capacity(2);
    parts.extend(expand);
    parts.push(mark);
    let mut cell = stack(SELECT_CELL, Axis::Horizontal, Some(SPACING_03), parts);
    cell.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_03)),
        ..InsetRefs::default()
    });
    cell.props.align = Some(Align::Center);
    cell.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    cell
}

/// A chevron-sized blank for a row that has no chevron of its own, so the
/// selection column is one width down the whole table.
fn expand_spacer() -> ViewNode {
    let chevron = icon_toned(EXPAND, IconMark::ChevronDown, IconTone::Primary);
    let w = chevron.constraints.horizontal.min.unwrap_or(0.0);
    let h = chevron.constraints.vertical.min.unwrap_or(0.0);
    let mut spacer = stack(EXPAND, Axis::Horizontal, None, vec![]);
    spacer.constraints.horizontal = AxisConstraint {
        min: Some(w),
        max: Some(w),
        priority: 0,
    };
    spacer.constraints.vertical = AxisConstraint {
        min: Some(h),
        max: Some(h),
        priority: 0,
    };
    spacer
}

/// Give a non-expandable row the chevron column an expandable sibling has.
///
/// Walks to the row's leading `select` cell — directly under a plain row,
/// under `cells` for an expandable one — and, if nothing keyed `expand`
/// leads it, inserts [`expand_spacer`] first.
fn reserve_expand_column(mut row: ViewNode) -> ViewNode {
    fn reserve(node: &mut ViewNode) -> bool {
        if node.key.as_str() == SELECT_CELL {
            let has = node
                .children
                .first()
                .is_some_and(|c| c.key.as_str() == EXPAND);
            if !has {
                node.children.insert(0, Arc::new(expand_spacer()));
            }
            return true;
        }
        for child in &mut node.children {
            if child.key.as_str() == SELECT_CELL || child.key.as_str() == "cells" {
                return reserve(Arc::make_mut(child));
            }
        }
        false
    }
    reserve(&mut row);
    row
}

/// One row's cells, laid out as a `Grid`: the selection column sized to its
/// content, then `ncols` equal [`TrackSize::Weight`] columns.
///
/// A horizontal stack sized each cell to its own text, so column 2's x
/// depended on how wide column 1's *own row* happened to be. A `Grid` with
/// shared column tracks fixes that: every cell in column *i* resolves the
/// same width in every row, because `data_table` offers every row the
/// identical `Stretch` width. Carbon gives no per-column width, so an
/// equal split is the least-invented default.
fn row_shell(
    key: impl Into<Key>,
    leading: ViewNode,
    cells: Vec<ViewNode>,
    height: f32,
) -> ViewNode {
    let ncols = cells.len().max(1);
    let mut columns = Vec::with_capacity(ncols + 1);
    columns.push(TrackSize::FitContent);
    columns.extend(std::iter::repeat_n(
        TrackSize::Weight { weight: 1.0 },
        ncols,
    ));
    let mut children = Vec::with_capacity(ncols + 1);
    children.push(leading);
    children.extend(
        cells
            .into_iter()
            .enumerate()
            .map(|(i, cell)| as_cell(i, cell)),
    );
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns,
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(children);
    node.constraints.vertical.min = Some(height);
    node
}

/// `padding-inline: $spacing-05 $spacing-05` (MEASURED
/// `th, td { padding-inline: $spacing-05 $spacing-05 }`,
/// `.agents/research/08-25-2026/Carbon-Component-Inventory/slice-b.md`
/// line 81). Block (top/bottom) is unset: the row's own fixed height
/// plus `Align::Center` on the cell does that job.
fn cell_padding_inline() -> InsetRefs {
    InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    }
}

fn as_cell(index: usize, node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Cell) {
        return node;
    }
    let mut wrap = stack(format!("c{index}"), Axis::Horizontal, None, vec![node]);
    wrap.props.padding = Some(cell_padding_inline());
    wrap.props.align = Some(Align::Center);
    wrap.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    wrap
}

/// A header caption: `heading-compact-01` in `$text-primary`.
fn header_text(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    as_compact_heading(text(key, content))
}

/// A body row handed in as a bare node becomes a non-selectable row: the
/// body fill and rule, an empty selection mark, no interactions.
fn ensure_row(node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Row) {
        return node;
    }
    let mut row = row_shell(
        node.key.clone(),
        selection_cell(None, select_mark(false)),
        vec![node],
        HEIGHT_LG,
    );
    row.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    row.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    row.props
        .tokens
        .insert("border-bottom".into(), t(BORDER_SUBTLE));
    row
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
        HEIGHT_LG, HEIGHT_SM, HEIGHT_XL, HEIGHT_XS, IconMark, IconTone, SIZE_MD, data_table,
        data_table_row, data_table_row_expandable, data_table_row_lg, data_table_row_md,
        data_table_row_sm, data_table_row_xl, data_table_row_xs, data_table_sort_header,
        data_table_zebra, icon_toned,
    };
    use crate::component::controls::{CheckState, checkbox_box};
    use crate::component::text::text;
    use crate::component::tokens::{
        BORDER_SUBTLE, LAYER_ACCENT, LAYER_SELECTED, SURFACE_BASE, TYPOGRAPHY_HEADING_SM,
    };
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
        assert_eq!(
            header.constraints.vertical.min,
            Some(HEIGHT_LG),
            "the header matches the body row height, and the default is lg"
        );
        assert_eq!(HEIGHT_LG, 48.0);
        assert_eq!(named(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(header, "c1").semantics.role, Some(Role::Cell));
        let row = named(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(named(row, "select").semantics.role, Some(Role::Cell));
        assert_eq!(named(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(row, "c1").semantics.role, Some(Role::Cell));
        no_drag(&node);
    }

    /// Row 9, round 2 ("not in carbon style"): Carbon's column header row
    /// is `$layer-accent` in `heading-compact-01` (slice-b:75), and every
    /// row draws **one** rule, under itself (`td { border-block-end }`,
    /// slice-b:76) — never a four-sided box, which on adjacent rows drew
    /// every seam twice and read as a grid.
    #[test]
    fn header_is_accent_in_compact_heading_and_rows_draw_one_bottom_rule() {
        let node = sample_table();
        let header = named(&node, "header");
        assert_eq!(token(header, "background"), Some(LAYER_ACCENT));
        let caption = named(header, "h0");
        assert_eq!(
            caption.props.style.as_ref().map(|s| s.as_str()),
            Some(TYPOGRAPHY_HEADING_SM),
            "a bare text header cell takes heading-compact-01"
        );
        for key in ["header", "r0"] {
            let row = named(&node, key);
            assert_eq!(
                token(row, "border-bottom"),
                Some(BORDER_SUBTLE),
                "{key}: one rule under the row"
            );
            assert_eq!(
                token(row, "border"),
                None,
                "{key}: a four-sided border is the grid-of-boxes defect"
            );
        }
    }

    /// The selection column: every body row leads with its own checkbox
    /// mark, and the header leads with a select-all control whose state
    /// is derived from the rows — empty, mixed, or checked.
    #[test]
    fn rows_lead_with_a_checkbox_and_the_header_derives_select_all() {
        let table = |a: bool, b: bool| {
            data_table(
                "jobs",
                vec![text("h0", "Name")],
                vec![
                    data_table_row("r0", vec![text("n0", "alpha")], a),
                    data_table_row("r1", vec![text("n1", "bravo")], b),
                ],
            )
        };
        let none = table(false, false);
        let all = named(&none, "select-all");
        assert_eq!(all.semantics.role, Some(Role::Button));
        assert_eq!(all.semantics.label.as_deref(), Some("Select all rows"));
        assert!(all.interactions.contains(&Interaction::Click));
        assert!(!all.semantics.selected);
        assert_eq!(all.semantics.value, None);
        assert_eq!(
            named(all, "box").props.tokens.get("background"),
            checkbox_box(CheckState::Unchecked)
                .props
                .tokens
                .get("background")
        );

        let some = table(true, false);
        let all = named(&some, "select-all");
        assert!(!all.semantics.selected);
        assert_eq!(all.semantics.value.as_deref(), Some("mixed"));
        assert!(
            named(all, "dash").props.tokens.contains_key("background"),
            "mixed draws the dash"
        );
        let r0 = named(&some, "r0");
        assert!(
            named(named(r0, "select"), "tick").props.canvas.is_some(),
            "a selected row's mark carries the tick"
        );
        let r1 = named(&some, "r1");
        assert!(
            named(r1, "select")
                .children
                .iter()
                .all(|c| c.key.as_str() == "box"),
            "an unselected row's mark is the empty box"
        );

        let every = table(true, true);
        let all = named(&every, "select-all");
        assert!(all.semantics.selected);
        assert_eq!(all.semantics.value, None);
        assert!(named(all, "tick").props.canvas.is_some());
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
        assert_eq!(on.constraints.vertical.min, Some(HEIGHT_LG));

        let off = data_table_row("r0", vec![text("n", "alpha")], false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }

    #[test]
    fn five_row_heights() {
        let cells = vec![text("n", "x")];
        let min = |row: ViewNode| row.constraints.vertical.min;
        assert_eq!(
            min(data_table_row_xs("r", cells.clone(), false)),
            Some(HEIGHT_XS)
        );
        assert_eq!(HEIGHT_XS, 24.0);
        assert_eq!(
            min(data_table_row_sm("r", cells.clone(), false)),
            Some(HEIGHT_SM)
        );
        assert_eq!(HEIGHT_SM, 32.0);
        assert_eq!(
            min(data_table_row_md("r", cells.clone(), false)),
            Some(SIZE_MD)
        );
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(
            min(data_table_row_lg("r", cells.clone(), false)),
            Some(HEIGHT_LG)
        );
        assert_eq!(
            min(data_table_row("r", cells.clone(), false)),
            Some(HEIGHT_LG)
        );
        assert_eq!(HEIGHT_LG, 48.0);
        assert_eq!(min(data_table_row_xl("r", cells, false)), Some(HEIGHT_XL));
        assert_eq!(HEIGHT_XL, 64.0);
    }

    /// The header row takes the tallest body row's height (docs, "Rows").
    #[test]
    fn the_header_is_as_tall_as_the_tallest_row() {
        let node = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![
                data_table_row_xs("r0", vec![text("n0", "a")], false),
                data_table_row_xl("r1", vec![text("n1", "b")], false),
            ],
        );
        assert_eq!(
            named(&node, "header").constraints.vertical.min,
            Some(HEIGHT_XL)
        );
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
            Some(LAYER_ACCENT),
            "the sort button rests on the header's own accent fill, so its \
             `background@hover` has a resting binding and it does not paint \
             a lighter box on the header"
        );
        assert_eq!(
            named(button, "name")
                .props
                .style
                .as_ref()
                .map(|s| s.as_str()),
            Some(TYPOGRAPHY_HEADING_SM)
        );

        let desc = data_table_sort_header("col-name", "Name", false);
        assert_eq!(
            named(&desc, "sort").semantics.value.as_deref(),
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
        let chevron = named(&open, "expand");
        assert_eq!(chevron.kind, NodeKind::Canvas);
        assert_eq!(
            chevron.props.canvas,
            icon_toned("expand", IconMark::ChevronUp, IconTone::Primary)
                .props
                .canvas
        );
        assert_eq!(
            named(&open, "body-text").props.text.as_deref(),
            Some("more detail")
        );
        assert_eq!(token(&open, "border-bottom"), Some(BORDER_SUBTLE));
        no_drag(&open);

        let shut =
            data_table_row_expandable("r0", vec![text("n", "alpha")], false, false, "more detail");
        assert_eq!(shut.semantics.expanded, Some(false));
        assert_eq!(
            named(&shut, "expand").props.canvas,
            icon_toned("expand", IconMark::ChevronDown, IconTone::Primary)
                .props
                .canvas
        );
        assert!(
            shut.children.iter().all(|c| c.key.as_str() != "body"),
            "collapsed row must not mount a body child"
        );
    }

    /// A table with one expandable row gives every other row and the header
    /// a chevron-sized blank ahead of the checkbox, so the selection column
    /// is one width down the table and the data columns line up.
    #[test]
    fn an_expandable_row_makes_every_other_row_reserve_the_chevron_column() {
        let node = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![
                data_table_row("r0", vec![text("n0", "alpha")], false),
                data_table_row_expandable("r1", vec![text("n1", "bravo")], false, false, "x"),
            ],
        );
        for key in ["header", "r0"] {
            let cell = named(named(&node, key), "select");
            let first = cell.children.first().expect("leading child");
            assert_eq!(first.key.as_str(), "expand", "{key}: reserves the column");
            assert_eq!(
                first.kind,
                NodeKind::Stack,
                "{key}: as a blank, not a glyph"
            );
        }
        let expandable = named(named(&node, "r1"), "select");
        assert_eq!(
            expandable
                .children
                .iter()
                .filter(|c| c.key.as_str() == "expand")
                .count(),
            1,
            "the expandable row keeps its one real chevron"
        );

        let plain = sample_table();
        assert!(
            named(named(&plain, "r0"), "select")
                .children
                .iter()
                .all(|c| c.key.as_str() != "expand"),
            "a table with no expandable row reserves nothing"
        );
    }

    #[test]
    fn zebra_alternates_accent_on_odd_rows() {
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
            Some(LAYER_ACCENT),
            "slice-b: Zebra rows are `$layer-accent`"
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

    /// The data columns line up: every row's first data cell starts at the
    /// same x, header included, across plain, sized, expandable and
    /// disabled rows.
    #[test]
    fn the_first_data_column_starts_at_one_x_in_every_row() {
        let frame = petrify_lone(sample_table_with_states());
        let x_of = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is not placed"))
                .rect
                .x
        };
        let header = x_of("/header/h0");
        for suffix in [
            "/r0/c0",
            "/r1/c0",
            "/r2/c0",
            "/r3/c0",
            "/r4/cells/c0",
            "/r5/c0",
        ] {
            assert_eq!(
                x_of(suffix),
                header,
                "{suffix} is not under the header's column"
            );
        }
    }

    /// Check F: enabled rows, the sort button and the select-all control
    /// are reachable; a disabled row is not.
    #[test]
    fn rows_and_the_sort_button_are_reachable_unless_disabled() {
        let frame = petrify_lone(sample_table_with_states());
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        for suffix in ["/r0", "/r1", "/r2", "/r3", "/r4", "/h0/sort", "/select-all"] {
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
    /// read on — header (accent), a plain body row (base), a selected row
    /// (the selected layer), and a zebra-striped odd row (accent) — read
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
