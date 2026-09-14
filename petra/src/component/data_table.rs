//! Carbon Data table (slice-b). Column resize is on by default.
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
//!    `"ascending"` / `"descending"` / `"sortable"` (the word is the second
//!    channel; [`SortDirection`] is the closed set).
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
//! Body text stays `TEXT_PRIMARY` where Carbon uses `$text-secondary`
//! at rest and `$text-primary` on the selected row; a child's foreground
//! cannot follow its row's state in this engine. Zebra is
//! [`data_table_zebra`].
//!
//! Still omitted, honestly: sticky header (this layout engine has no
//! scroll-region/`position: sticky` equivalent — `reserve_expand_column`'s
//! trailing sibling, [`reserve_menu_column`], is the closest thing to a
//! per-column mechanism it has, and a scrolling `thead` is a different
//! problem). Column resize is a deliberate departure: Carbon v11 does not
//! ship it, the operator asked for it by name on every columnar surface,
//! and [`data_table_sized`]'s `dividers` flag is how a caller declines.
//! Row reorder is the same shape: `reorderable` inserts a grip cell that
//! declares [`Interaction::Drag`].
//!
//! # T034: the toolbar tier (spec 009)
//!
//! Six more anatomy pieces, all Carbon-numbered in the usage page's own
//! "Formatting > Anatomy" list:
//!
//! - **Anatomy 2, Toolbar** — [`data_table_toolbar`]: a search field at the
//!   leading edge (grows), trailing controls (e.g. a column-visibility
//!   [`super::menu_button`]) hugging the trailing edge. A `Grid` with one
//!   [`TrackSize::Weight`] column and one [`TrackSize::FitContent`] column,
//!   not [`super::list_box::edge_row`]'s `SpaceBetween` stack: `edge_row`
//!   gives slack to the *gap*, and a toolbar wants the slack inside the
//!   search field, the way `data_table` itself already grows every row to
//!   the table's width.
//! - **Anatomy 2a, Batch action bar** — [`data_table_batch_bar`]: replaces
//!   the toolbar (Carbon: "slides in over the toolbar", slice-b) while any
//!   row is selected. `ACCENT_PRIMARY`/`TEXT_ON_ACCENT`: the vocabulary has
//!   no `background-brand` token Carbon's own SCSS names, and this is the
//!   same accent/on-accent pair [`super::menu_button`]'s primary trigger
//!   already spends, so the substitution is provably readable rather than
//!   invented. [`data_table_batch_cancel`] is its leading "×" control.
//! - **Anatomy 5, Row menu** — [`data_table_row_menu_trigger`] plus
//!   [`reserve_menu_column`]: a trailing `FitContent` column, the mirror of
//!   the leading chevron's [`reserve_expand_column`]. Carbon's own glyph is
//!   `OverflowMenuVertical` (a kebab); this icon set has no kebab mark, so
//!   the trigger reuses [`IconMark::Menu`] — the closest shape available —
//!   and, as everywhere else in this file, the glyph is never the only
//!   channel: `Semantics.label` carries the real word. Always visible
//!   rather than opacity-0-until-hover
//!   (`.cds--data-table--visible-overflow-menu` is Carbon's own always-on
//!   modifier, not an invention).
//! - **Skeleton** — [`data_table_skeleton`]: placeholder bars
//!   ([`super::kit::swatch`], Carbon's measured 16×64), `Semantics.skeleton`
//!   on every bar. Deliberately carries **no** `Role` anywhere in the tree
//!   — not `Role::Table`, not `Role::Row`, not `Role::Cell` — so a screen
//!   reader never announces placeholder content as a real table, row or
//!   cell, and [`super::super::focus::FocusTree`] never seats one: no node
//!   here declares [`Interaction::Focus`], so nothing skeleton ever enters
//!   focus order or a selection count.
//!
//! Column visibility, search/filter state, batch-action identity and the
//! per-row menu's own open/shut and item list are the *compound's* job
//! (`gorgon_petra_compound::data_table`), not this file's: they are all
//! state a caller holds across renders, and every atomic in this crate is
//! an argument-driven pure function with none.

use std::sync::Arc;

use super::controls::{CheckState, checkbox_box};
use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::edge_row;
use super::menu::menu;
use super::pad;
use super::stack;
use super::swatch;
use super::text::{as_compact_heading, ellipsis_text, text};
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_ACCENT, LAYER_ACCENT_HOVER, LAYER_HOVER, LAYER_SELECTED,
    LAYER_SELECTED_HOVER, SIZE_MD, SPACING_03, SPACING_04, SPACING_05, SURFACE_BASE,
    TEXT_ON_ACCENT, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::frame::PetrifiedFrame;
use crate::geom::{Align, Axis, Point};
use crate::tree::{
    Anchor, AxisConstraint, Edge, Fit, FocusFigure, InsetRefs, Interaction, Justify, Key, NodeKind,
    Props, Role, Semantics, TrackSize, ViewNode,
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
/// The key of a row's trailing row-menu cell, and of the blank
/// [`reserve_menu_column`] stands in for it on a row with no menu of its
/// own.
const ROW_MENU: &str = "row-menu";

/// Skeleton placeholder bar (style page "Structure", `data-table-skeleton.scss`
/// MEASURED): header/cell bars are 16 tall, 64 wide.
const SKELETON_BAR_W: f32 = 64.0;
/// See [`SKELETON_BAR_W`].
const SKELETON_BAR_H: f32 = 16.0;

const _: () = assert!(SKELETON_BAR_W == 64.0);
const _: () = assert!(SKELETON_BAR_H == 16.0);

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];
const SORT_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];
const DIVIDER: f32 = 8.0;
const DIVIDER_KEY: &str = "div";
const GRIP_KEY: &str = "grip";
/// Narrowest a data column may be dragged.
///
/// A cell spends `$spacing-05` (16) on each side, so a 32-unit floor is
/// all padding. Wrap then stacks one glyph per line and the row grows —
/// the compound table shot of 2026-09-13. 96 is that padding plus 64 of
/// content: a short header still reads, and [`TextWrap::Ellipsis`] keeps
/// the row at one line.
const MIN_COLUMN: f32 = 96.0;

const _: () = assert!(MIN_COLUMN == 96.0);
/// Carbon `.cds--table-column-checkbox` `min-inline-size: 2.5rem`.
const SELECT_COL: f32 = 40.0;
/// [`SELECT_COL`] plus a 16-unit chevron and [`SPACING_03`] (8) between.
const SELECT_COL_EXPAND: f32 = 64.0;
/// Icon 16 plus the cell's `$spacing-05` inline pad on both sides.
const MENU_COL: f32 = 48.0;
const GRIP_COL: f32 = 48.0;
const DIVIDER_INTENTS: &[Interaction] = &[Interaction::Drag];
const GRIP_INTENTS: &[Interaction] = &[Interaction::Drag];

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
/// header and every other row reserve the chevron column. Column dividers
/// are on; row reorder is off. See [`data_table_sized`].
pub fn data_table(key: impl Into<Key>, header: Vec<ViewNode>, rows: Vec<ViewNode>) -> ViewNode {
    let ncols = header.len().max(1);
    data_table_sized(key, header, rows, &vec![1.0; ncols], true, false)
}

/// [`data_table`] with column weights, divider policy, and row reorder.
///
/// `weights` is one entry per data column (not the leading select cell).
/// `dividers` is the "toggled off in code" half: `false` builds the same
/// table with no divider tracks. `reorderable` inserts a grip cell after
/// the select column; the page owns the order, the same way it owns the
/// weights.
pub fn data_table_sized(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
    weights: &[f32],
    dividers: bool,
    reorderable: bool,
) -> ViewNode {
    let names: Vec<String> = header.iter().map(collect_text).collect();
    let weights = normalise_weights(weights, names.len().max(1));
    let mut node = assemble_data_table(key, header, rows);
    for row in &mut node.children {
        lay_out_columns(Arc::make_mut(row), &weights, dividers, reorderable, &names);
    }
    pin_chrome_columns(&mut node);
    node
}

fn assemble_data_table(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
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
    // Trailing counterpart of `expandable`/`reserve_expand_column`, above:
    // a row built by `data_table_row_actions` ends its own top-level
    // children with a cell keyed `row-menu`; every row (and the header)
    // that did not build its own gets a matching blank, so every row's
    // `Grid` still declares the identical column-track sequence the
    // `Stretch` note below depends on.
    let has_menu = rows.iter().any(|row| {
        row.children
            .last()
            .is_some_and(|c| c.key.as_str() == ROW_MENU)
    });
    let rows: Vec<ViewNode> = if has_menu {
        rows.into_iter().map(reserve_menu_column).collect()
    } else {
        rows
    };

    let selected = rows.iter().filter(|row| row.semantics.selected).count();
    let all = match selected {
        0 => CheckState::Unchecked,
        n if n == rows.len() => CheckState::Checked,
        _ => CheckState::Mixed,
    };

    let mut header = header_row("header", header, height, all, expandable);
    if has_menu {
        header = reserve_menu_column(header);
    }
    let mut children = vec![header];
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
    let ncols = header.len().max(1);
    data_table_zebra_sized(key, header, rows, &vec![1.0; ncols], true, false)
}

/// [`data_table_zebra`] with the same extra arguments as [`data_table_sized`].
pub fn data_table_zebra_sized(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
    weights: &[f32],
    dividers: bool,
    reorderable: bool,
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
    data_table_sized(key, header, rows, weights, dividers, reorderable)
}

/// The column weights a drag on the divider `node` to `pos` asks for.
/// Peer of [`super::structured_list_weights_at`].
#[must_use]
pub fn data_table_weights_at(
    frame: &PetrifiedFrame,
    node: &str,
    pos: Point,
    weights: &[f32],
) -> Option<Vec<f32>> {
    let (row, index) = divider_of(node)?;
    if index + 1 >= weights.len() {
        return None;
    }
    let columns = data_column_rects(frame, row);
    let left = columns.get(index)?.rect;
    let right = columns.get(index + 1)?.rect;
    let travel = right.right() - left.x - DIVIDER;
    if travel <= 2.0 * MIN_COLUMN {
        return None;
    }
    let want = (pos.x - DIVIDER / 2.0 - left.x).clamp(MIN_COLUMN, travel - MIN_COLUMN);
    let pair = weights[index] + weights[index + 1];
    let mut out = weights.to_vec();
    out[index] = pair * want / travel;
    out[index + 1] = pair - out[index];
    Some(out)
}

/// Direct data-cell placements of `row`, in column order.
///
/// Sort headers keep their own keys (`name`, `h0`) because they already
/// declare `Role::Cell`, so looking up `{row}/c0` misses the header. Skip
/// select, grip, dividers, and the trailing row-menu.
fn data_column_rects<'a>(frame: &'a PetrifiedFrame, row: &str) -> Vec<&'a crate::frame::Placement> {
    let prefix = format!("{row}/");
    frame
        .placements
        .iter()
        .filter(|p| {
            let Some(rest) = p.id.strip_prefix(&prefix) else {
                return false;
            };
            if rest.contains('/') {
                return false;
            }
            if rest == SELECT_CELL || rest == GRIP_KEY || rest == ROW_MENU {
                return false;
            }
            if rest
                .strip_prefix(DIVIDER_KEY)
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
            {
                return false;
            }
            true
        })
        .collect()
}

/// The row key a drag on a grip should move, if `node` names a grip.
#[must_use]
pub fn data_table_grip_row(node: &str) -> Option<&str> {
    let mut prev = "";
    for segment in node.split('/') {
        if segment == GRIP_KEY {
            return Some(prev).filter(|s| !s.is_empty());
        }
        prev = segment;
    }
    None
}

fn divider_of(node: &str) -> Option<(&str, usize)> {
    let mut offset = 0usize;
    for segment in node.split('/') {
        let start = offset;
        offset += segment.len() + 1;
        if let Some(index) = segment
            .strip_prefix(DIVIDER_KEY)
            .and_then(|digits| digits.parse::<usize>().ok())
        {
            return Some((&node[..start.saturating_sub(1)], index));
        }
    }
    None
}

fn normalise_weights(weights: &[f32], ncols: usize) -> Vec<f32> {
    (0..ncols)
        .map(|i| match weights.get(i) {
            Some(w) if *w > 0.0 && w.is_finite() => *w,
            _ => 1.0,
        })
        .collect()
}

fn lay_out_columns(
    row: &mut ViewNode,
    weights: &[f32],
    dividers: bool,
    reorderable: bool,
    names: &[String],
) {
    let target: &mut ViewNode = if row.kind == NodeKind::Grid {
        row
    } else {
        match row.children.iter_mut().find(|c| c.key.as_str() == "cells") {
            Some(cells) => Arc::make_mut(cells),
            None => return,
        }
    };
    if target.kind != NodeKind::Grid {
        return;
    }
    let leading = target
        .children
        .first()
        .is_some_and(|c| c.key.as_str() == SELECT_CELL);
    let trailing = target
        .children
        .last()
        .is_some_and(|c| c.key.as_str() == ROW_MENU);
    let start = usize::from(leading);
    let end = target.children.len() - usize::from(trailing);
    if end <= start {
        return;
    }
    let is_header = target.key.as_str() == "header";
    let old = std::mem::take(&mut target.children);
    let mut children: Vec<Arc<ViewNode>> = Vec::with_capacity(old.len() + end);
    let mut columns: Vec<TrackSize> = Vec::with_capacity(old.len() + end);
    let mut data_i = 0usize;
    for (i, cell) in old.into_iter().enumerate() {
        if i < start {
            children.push(cell);
            columns.push(TrackSize::FitContent);
            if reorderable {
                children.push(Arc::new(if is_header { grip_blank() } else { grip_cell() }));
                columns.push(TrackSize::FitContent);
            }
            continue;
        }
        if i >= end {
            children.push(cell);
            columns.push(TrackSize::FitContent);
            continue;
        }
        if data_i > 0 && dividers {
            children.push(Arc::new(column_divider(data_i - 1, names.get(data_i - 1))));
            columns.push(TrackSize::Fixed { value: DIVIDER });
        }
        children.push(cell);
        columns.push(TrackSize::Weight {
            weight: weights.get(data_i).copied().unwrap_or(1.0),
        });
        data_i += 1;
    }
    target.props.columns = columns;
    target.children = children;
}

/// FitContent chrome (select, grip, row-menu) sizes independently on each
/// row. A header blank is narrower than a hamburger, a chevron row is
/// wider than a spacer row, and the Weight columns then start at three
/// different x positions. Pin those tracks to one width so a resize
/// cannot un-align the table.
fn pin_chrome_columns(table: &mut ViewNode) {
    let mut expand = false;
    for row in &table.children {
        visit_row_grid(row, &mut |grid| {
            if grid
                .children
                .iter()
                .any(|c| c.key.as_str() == SELECT_CELL && has_expand(c))
            {
                expand = true;
            }
        });
    }
    let select_w = if expand {
        SELECT_COL_EXPAND
    } else {
        SELECT_COL
    };
    for row in &mut table.children {
        visit_row_grid_mut(Arc::make_mut(row), &mut |grid| {
            for (i, child) in grid.children.iter().enumerate() {
                let w = match child.key.as_str() {
                    SELECT_CELL => Some(select_w),
                    GRIP_KEY => Some(GRIP_COL),
                    ROW_MENU => Some(MENU_COL),
                    _ => None,
                };
                if let Some(w) = w {
                    if let Some(col) = grid.props.columns.get_mut(i) {
                        *col = TrackSize::Fixed { value: w };
                    }
                }
            }
        });
    }
}

fn has_expand(select: &ViewNode) -> bool {
    select.children.iter().any(|c| c.key.as_str() == EXPAND)
}

fn visit_row_grid(row: &ViewNode, f: &mut impl FnMut(&ViewNode)) {
    if row.kind == NodeKind::Grid {
        f(row);
        return;
    }
    if let Some(cells) = row.children.iter().find(|c| c.key.as_str() == "cells") {
        f(cells);
    }
}

fn visit_row_grid_mut(row: &mut ViewNode, f: &mut impl FnMut(&mut ViewNode)) {
    if row.kind == NodeKind::Grid {
        f(row);
        return;
    }
    if let Some(cells) = row.children.iter_mut().find(|c| c.key.as_str() == "cells") {
        f(Arc::make_mut(cells));
    }
}

fn column_divider(index: usize, name: Option<&String>) -> ViewNode {
    let rule = super::rule("rule", Axis::Vertical, BORDER_SUBTLE);
    let label = match name {
        Some(name) if !name.trim().is_empty() => format!("Resize column {name}"),
        _ => format!("Resize column {}", index + 1),
    };
    let mut node = stack(
        format!("{DIVIDER_KEY}{index}"),
        Axis::Horizontal,
        None,
        vec![rule],
    );
    node.props.align = Some(Align::Stretch);
    node.props.justify = Some(Justify::Center);
    node.interactive(Role::Separator, label, DIVIDER_INTENTS)
}

fn grip_cell() -> ViewNode {
    let mark = icon_toned("icon", IconMark::Menu, IconTone::Primary);
    let mut cell = stack(GRIP_KEY, Axis::Horizontal, None, vec![mark]);
    cell.props.padding = Some(cell_padding_inline());
    cell.props.align = Some(Align::Center);
    cell.interactive(Role::Button, "Reorder row", GRIP_INTENTS)
}

fn grip_blank() -> ViewNode {
    let mut cell = stack(GRIP_KEY, Axis::Horizontal, None, Vec::new());
    cell.props.padding = Some(cell_padding_inline());
    cell.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    cell
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

/// [`data_table_row`] with a trailing row-menu column (T034, anatomy 5).
///
/// `menu` is the whole trigger-plus-overlay control, built by the caller —
/// this atomic does not know whether the menu is open, only where its
/// column goes. Appended *after* the row is fully built (fills, `Role::Row`,
/// focus figure already bound), the same way [`reserve_expand_column`]
/// mutates an already-built row rather than threading a flag through
/// [`row_shell`]: neither `data_table_row`'s own five callers nor
/// [`row_shell`]'s signature change, so this is the one row constructor
/// that opts in rather than a new parameter every other row gained and
/// never uses.
pub fn data_table_row_actions(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    selected: bool,
    menu: ViewNode,
) -> ViewNode {
    let mut row = data_table_row_sized(key, cells, selected, RowSize::Lg);
    append_menu_column(&mut row, menu);
    row
}

/// Push a trailing `FitContent` column and its cell onto an already-built
/// row's own `Grid` props/children — see [`data_table_row_actions`].
///
/// Finds that `Grid` the same way [`reserve_menu_column`] does, and for the
/// same reason: an expandable row's root is a vertical `stack` of `cells`
/// and an optional body, not the `Grid` every other row is, so pushing onto
/// whichever node the caller handed over would put a column on the wrong
/// node. One walk, both callers, so a blank reservation and a real trigger
/// can never disagree about which node carries the trailing column.
fn append_menu_column(row: &mut ViewNode, menu: ViewNode) {
    let target: &mut ViewNode = if row.kind == NodeKind::Grid {
        row
    } else {
        match row.children.iter_mut().find(|c| c.key.as_str() == "cells") {
            Some(cells) => Arc::make_mut(cells),
            None => row,
        }
    };
    target.props.columns.push(TrackSize::FitContent);
    target.children.push(Arc::new(as_cell(ROW_MENU, menu)));
}

/// [`data_table_row_expandable`] with a trailing row-menu column (T034,
/// anatomy 5) — the expandable row's [`data_table_row_actions`].
///
/// Until 2026-09-11 there was no such constructor and the compound
/// documented the absence as "a documented scope limit, not an oversight",
/// on the grounds that "the chevron and the menu trigger both want the
/// leading/trailing edge of the same row shell". They do not: the chevron
/// sits *inside* the leading selection cell and the menu column is a
/// trailing top-level sibling, and [`reserve_menu_column`] already walks
/// into an expandable row's nested `cells` `Grid` to put a blank there.
/// The visible cost of the limit was a table whose rows disagreed — three
/// of four carrying a menu glyph and the expandable one carrying a blank,
/// which reads as a bug rather than as a rule.
///
/// Same append-after-build shape as [`data_table_row_actions`], through the
/// same [`append_menu_column`], so neither row constructor grows a
/// parameter the other four never use.
#[must_use]
pub fn data_table_row_expandable_actions(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    selected: bool,
    expanded: bool,
    body: impl Into<String>,
    menu: ViewNode,
) -> ViewNode {
    let mut row = data_table_row_expandable(key, cells, selected, expanded, body);
    append_menu_column(&mut row, menu);
    row
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
    // `Sides` on every row this module builds, the operator's call of
    // 2026-09-06: *"instead of underlining the label try the v bars again,
    // it looked better"*.
    //
    // A table is rows packed flush, so the default bar *under* a row paints
    // on the next one and `BarInside` was the answer for a day. What an
    // underline cannot do here is say which of a row's two columns it
    // belongs to: the stripe lands on the row's content run, which is the
    // name cell, and reads as marking that cell rather than the row. A pair
    // of brackets marks the row and nothing narrower.
    //
    // They stand outside the row, and a row spans the table exactly, so this
    // is the one figure choice in the library whose containment depends on
    // the table having padding of its own.
    // `no_focus_figure_paints_outside_the_box_it_belongs_to` is what checks
    // that and will say so if the table ever loses it.
    let mut node = node
        .interactive(Role::Row, label, ROW_INTENTS)
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.selected = selected;
    node.semantics.expanded = Some(expanded);
    node
}

/// The direction a sortable column header declares: which way the active
/// sort runs, or that the column takes no part in it yet.
///
/// Three real states, not a boolean. A boolean can only ever spell
/// "ascending" or "descending", which forces every column that is *not*
/// the active sort to claim one of those anyway — see
/// `gorgon_petra_compound::data_table::header_cells`'s prior doc comment,
/// which said so outright: *"A column that is not the active sort shows as
/// ascending: the atomic has no unsorted spelling."* A two-column table
/// with one sorted column then rendered both columns claiming the same
/// direction, because there was no third word to reach for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    /// The active sort, ascending.
    Ascending,
    /// The active sort, descending.
    Descending,
    /// Sortable, but not the active sort. Carbon's own neutral sort
    /// affordance on an inactive sortable column: the column can be
    /// sorted, nothing yet says which way it would run.
    Sortable,
}

impl SortDirection {
    /// The word this spends as both the visible caption and
    /// [`Semantics.value`] — the accessible second channel a hue or an
    /// arrow alone cannot carry. [`Self::Sortable`] gets its own word
    /// rather than an empty caption, so an inactive sortable column still
    /// reads as "you can sort this," not as nothing at all.
    fn word(self) -> &'static str {
        match self {
            Self::Ascending => "ascending",
            Self::Descending => "descending",
            Self::Sortable => "sortable",
        }
    }
}

/// Sortable header cell: a [`Role::Button`] labelled `"Sort {name}"`.
///
/// `direction` selects [`Semantics.value`] `"ascending"` / `"descending"` /
/// `"sortable"`. The same word is visible text, so sort direction is never
/// an arrow alone. The cell role is stamped here so [`as_cell`] will not
/// wrap again.
pub fn data_table_sort_header(
    key: impl Into<Key>,
    name: impl Into<String>,
    direction: SortDirection,
) -> ViewNode {
    let name = name.into();
    let word = direction.word();
    let accessible = format!("Sort {name}");
    let caption = header_text("name", name);
    let dir = header_text("direction", word);
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
    let mut button = button
        .interactive(Role::Button, accessible, SORT_INTENTS)
        .owning_its_text();
    button.semantics.value = Some(word.into());

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

/// A trailing toolbar control that opens a menu: the column-visibility
/// picker is its one caller today.
///
/// **Not [`super::menu_button`], and the difference is not cosmetic.** A
/// Carbon Menu button is a *primary* button — `ACCENT_PRIMARY` fill,
/// `TEXT_ON_ACCENT` label, height md 40, `min-inline-size` 160 — because it
/// is the page's own call to action (`menu_button.rs`'s module doc states
/// each of those as Carbon facts). A table toolbar's settings control is
/// none of those things: it sits *inside* the toolbar, shares the toolbar's
/// own ground, and is one of several trailing controls rather than the
/// thing the page is for. Reusing `menu_button` here put a blue call to
/// action beside a search field and stood it 40 tall inside a 48-tall
/// toolbar, so its bottom edge floated eight units above the field it sits
/// flush against — measured on the row 56 capture, 2026-09-11, at 81 device
/// pixels against the field's 95.
///
/// So: [`SURFACE_BASE`], the toolbar's own fill, which is Carbon's ghost
/// treatment; [`TEXT_PRIMARY`] label and an [`IconTone::Primary`] chevron;
/// and [`HEIGHT_LG`] pinned both ways so it can only ever be the toolbar's
/// own height.
///
/// [`FocusFigure::Border`], not `Sides`. A `Sides` figure draws its two
/// bars seven units *outside* the control, which is right for a control
/// with padding around it and wrong for one flush against a toolbar edge:
/// the bars land on the card behind the toolbar and on the neighbour. Not
/// `BarInside` either — a bar figure marks the *content run* and
/// `focus::marked_rect` excludes an `Input` from that run, so a bar here
/// would mark the chevron.
///
/// `Semantics.expanded` is declared and the menu is mounted only while
/// open, so shut-versus-open is never carried by colour alone.
#[must_use]
pub fn data_table_toolbar_menu(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let chevron = icon_toned(
        "caret",
        if open {
            IconMark::ChevronUp
        } else {
            IconMark::ChevronDown
        },
        IconTone::Primary,
    );
    let mut trigger = edge_row("trigger", caption, Some(chevron));
    trigger
        .props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    trigger
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut trigger = trigger
        .with_constraints(crate::tree::Constraints {
            horizontal: AxisConstraint::default(),
            vertical: AxisConstraint {
                min: Some(HEIGHT_LG),
                max: Some(HEIGHT_LG),
                priority: 0,
            },
        })
        .interactive(Role::Button, label.clone(), TOOLBAR_MENU_INTENTS)
        .owning_its_text()
        .with_focus_figure(FocusFigure::Border);
    trigger.semantics.expanded = Some(open);
    let mut children = vec![trigger];
    if open {
        children.push(data_table_menu("menu", label, items));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// Overflow menu for a table toolbar control or a row's own trigger.
///
/// [`menu`]'s default [`Fit::Anchor`] copies the trigger's width. A Columns
/// trigger is one word plus a chevron; a row-menu trigger is a 16-unit
/// glyph. Checkbox rows and "Rename"/"Delete" need more than that, and a
/// menu item ellipsizes rather than growing the panel, so the labels clip
/// or the 160-wide box hangs off the table's trailing edge. [`Fit::Content`]
/// lets the 160/288 bound size to the items. [`Align::End`] hangs the panel
/// from the trigger's trailing edge so a trailing control stays inside the
/// table.
#[must_use]
pub fn data_table_menu(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut node = menu(key, label, items);
    node.props.fit = Some(Fit::Content);
    node.props.anchor = Some(Anchor::Sibling {
        key: "trigger".into(),
        edge: Edge::Bottom,
        align: crate::tree::Align::End,
        offset: None,
    });
    node
}

/// What [`data_table_toolbar_menu`]'s trigger accepts. Same three a
/// [`super::menu_button`] trigger accepts, for the same reasons.
const TOOLBAR_MENU_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Carbon Toolbar (T034, anatomy 2): `search` grows, `trailing` controls
/// (the column-visibility [`data_table_toolbar_menu`]) hug the trailing edge.
///
/// A `Grid` with one [`TrackSize::Weight`] column and one
/// [`TrackSize::FitContent`] column — see the module doc's "T034" section
/// for why this is not [`edge_row`]. Height pairs with the table's own row
/// size (style page "Toolbar": large 48 with lg/xl rows, small 32 with
/// xs/sm); this file's tables are always lg, so the toolbar is always
/// [`HEIGHT_LG`].
pub fn data_table_toolbar(
    key: impl Into<Key>,
    search: ViewNode,
    trailing: Vec<ViewNode>,
) -> ViewNode {
    let actions = stack(
        "toolbar-actions",
        Axis::Horizontal,
        Some(SPACING_03),
        trailing,
    );
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }, TrackSize::FitContent],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![reseat_flush_focus(search), actions]);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.constraints.vertical.min = Some(HEIGHT_LG);
    node
}

/// Re-seat a control's focus figure for a container with no padding.
///
/// `search` seats [`FocusFigure::Sides`], which draws its two bars seven
/// units *outside* the control. That is right for a field with a card's
/// padding around it and wrong at a toolbar's leading edge: the field's own
/// left edge is the toolbar's, so the leading bar lands on the card behind
/// the toolbar and the trailing one lands on whichever control sits flush
/// beside it. Both were visible on the row 56 capture, 2026-09-11, as a
/// six-pixel blue tick outside the toolbar and a second one in the seam.
///
/// [`FocusFigure::Border`] and not `BarInside`, which is the other figure
/// that stays inside its control: a bar figure marks the *content run*, and
/// `focus::marked_rect` excludes a [`NodeKind::Input`] from that run because
/// an input is its own focus target. A search field's only other content is
/// its magnifier, so `BarInside` would draw a stub under the icon and
/// nothing under the text.
///
/// The well and the holder must agree — focus is *shown on* the field and
/// *held by* its `input` child — so the children are re-seated too. The
/// gallery's `a_control_and_the_node_it_shows_focus_on_agree_about_the_figure`
/// refuses a holder that declares a shape nothing draws.
fn reseat_flush_focus(mut node: ViewNode) -> ViewNode {
    node.semantics.focus_figure = FocusFigure::Border;
    for child in &mut node.children {
        Arc::make_mut(child).semantics.focus_figure = FocusFigure::Border;
    }
    node
}

/// Carbon Batch action bar (T034, anatomy 2a): replaces
/// [`data_table_toolbar`] while any row is selected ("slides in over the
/// toolbar", slice-b). `cancel` is [`data_table_batch_cancel`]; `actions`
/// are the author's own batch buttons.
///
/// `ACCENT_PRIMARY`/[`TEXT_ON_ACCENT`] stand in for Carbon's
/// `$background-brand`: that name is not in this vocabulary, and this is
/// the same accent/on-accent pair [`super::menu_button`]'s primary trigger
/// already spends against the same fill, which is how the contrast test
/// below is provably not a guess.
pub fn data_table_batch_bar(
    key: impl Into<Key>,
    count: usize,
    cancel: ViewNode,
    actions: Vec<ViewNode>,
) -> ViewNode {
    let mut count_text = text(
        "count",
        format!("{count} item{} selected", if count == 1 { "" } else { "s" }),
    );
    count_text
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_ON_ACCENT));
    let mut leading = stack(
        "batch-leading",
        Axis::Horizontal,
        Some(SPACING_04),
        vec![cancel, count_text],
    );
    leading.props.align = Some(Align::Center);
    let mut trailing = stack("batch-actions", Axis::Horizontal, Some(SPACING_03), actions);
    trailing.props.align = Some(Align::Center);

    let mut node = edge_row(key, leading, Some(trailing));
    node.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    node.constraints.vertical.min = Some(HEIGHT_LG);
    node
}

/// The batch bar's leading "cancel selection" control: an "×" on the same
/// [`ACCENT_PRIMARY`] fill the rest of [`data_table_batch_bar`] draws on.
pub fn data_table_batch_cancel(key: impl Into<Key>) -> ViewNode {
    icon_only_button(key, IconMark::Close, IconTone::OnAccent, "Cancel selection")
}

/// One of [`data_table_batch_bar`]'s own action buttons: `label` in
/// [`TEXT_ON_ACCENT`], never [`super::ghost_button`] or [`super::menu_item`]
/// — both of those bind link/primary ink tuned for a resting `surface.raised`
/// ground, not [`ACCENT_PRIMARY`], and reusing either here risks the same
/// contrast failure `tokens.rs`'s own `SUPPORT_ERROR` doc warns about
/// (`field_invalid` drawing in the same blue a focused field used).
pub fn data_table_batch_action(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_ON_ACCENT));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.interactive(Role::Button, label, ICON_BUTTON_INTENTS)
        .with_focus_figure(FocusFigure::Sides)
}

/// Row-menu trigger (T034, anatomy 5): an icon-only [`Role::Button`] in a
/// row's trailing [`ROW_MENU`] column, always visible
/// (`.cds--data-table--visible-overflow-menu`, Carbon's own always-on
/// modifier — not opacity-0-until-hover, which this engine's frame model
/// has no per-row hover state to key off outside the row's own click
/// target).
///
/// Carbon's own glyph is `OverflowMenuVertical`, a kebab; this icon set
/// ships no kebab mark ([`super::icon::IconMark`]'s own list), so this
/// reuses [`IconMark::Menu`] — the closest shape available. Never the only
/// channel: `label` is the real accessible name, the same rule every other
/// icon-only mark in this file already keeps (the search magnifier, the
/// sort arrows).
pub fn data_table_row_menu_trigger(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    icon_only_button(key, IconMark::Menu, IconTone::Primary, label)
}

const ICON_BUTTON_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Shared shape of [`data_table_batch_cancel`] and
/// [`data_table_row_menu_trigger`]: one glyph, no visible text, a real
/// accessible name.
fn icon_only_button(
    key: impl Into<Key>,
    mark: IconMark,
    tone: IconTone,
    label: impl Into<String>,
) -> ViewNode {
    let glyph = icon_toned("glyph", mark, tone);
    let mut node = stack(key, Axis::Horizontal, None, vec![glyph]);
    node.props.align = Some(Align::Center);
    node.interactive(Role::Button, label, ICON_BUTTON_INTENTS)
        .with_focus_figure(FocusFigure::Sides)
}

/// Carbon Skeleton (T034): a whole-table loading placeholder
/// (`data-table-skeleton.scss`), `ncols` header bars and `nrows` body-row
/// bars, all [`super::kit::swatch`] rectangles measured 16 tall, 64 wide
/// (style page "Structure").
///
/// Deliberately carries **no [`Role`] anywhere** — not `Table`, not `Row`,
/// not `Cell` — and no node here declares [`Interaction::Focus`]. A
/// skeleton table is not reachable in focus order, cannot be selected (no
/// `select-all`, no row checkboxes, no `Semantics.selected` field exists to
/// set), and a screen reader has nothing shaped like a table row to
/// announce. `Semantics.skeleton` is set on every bar — the engine's own
/// state-resolution rank (`token::state::InteractionRank::Skeleton`,
/// "outranks everything: a placeholder is not hoverable, pressable, or
/// disabled-looking") — so a future `background@skeleton` binding on this
/// module's own tokens would already reach the right nodes; today the bars
/// bind a plain resting [`LAYER_ACCENT`] ("a recessed fill... where an area
/// is filled to recede, spend this" — this module's own tokens doc) because
/// nothing here has a hover/active state to switch the fill by.
pub fn data_table_skeleton(key: impl Into<Key>, ncols: usize, nrows: usize) -> ViewNode {
    let ncols = ncols.max(1);
    let mut children = vec![skeleton_row("header", ncols)];
    for i in 0..nrows {
        children.push(skeleton_row(format!("skeleton-row-{i}"), ncols));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.props.align = Some(Align::Stretch);
    node
}

fn skeleton_row(key: impl Into<Key>, ncols: usize) -> ViewNode {
    let bars: Vec<ViewNode> = (0..ncols).map(skeleton_cell).collect();
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: std::iter::repeat_n(TrackSize::Weight { weight: 1.0 }, ncols).collect(),
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(bars);
    node.constraints.vertical.min = Some(HEIGHT_LG);
    node.props
        .tokens
        .insert("border-bottom".into(), t(BORDER_SUBTLE));
    node
}

fn skeleton_cell(index: usize) -> ViewNode {
    let mut bar = swatch(
        "bar",
        SKELETON_BAR_W,
        SKELETON_BAR_H,
        Some(LAYER_ACCENT),
        None,
        None,
    );
    bar.semantics.skeleton = true;
    let mut cell = stack(format!("c{index}"), Axis::Horizontal, None, vec![bar]);
    cell.props.padding = Some(cell_padding_inline());
    cell.props.align = Some(Align::Center);
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
    // `Sides`, for the reason the expandable row above gives.
    let mut node = node
        .interactive(Role::Row, label, ROW_INTENTS)
        .with_focus_figure(FocusFigure::Sides);
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
    let mut node = node
        .interactive(Role::Button, SELECT_ALL_LABEL, ROW_INTENTS)
        .owning_its_text();
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

/// Give a row with no row-menu of its own the trailing blank
/// [`data_table_row_actions`]'s real trigger occupies elsewhere in the
/// table — the mirror of [`reserve_expand_column`], on the trailing edge
/// instead of nested inside the leading cell, because a row-menu column is
/// a plain top-level sibling of every other cell rather than something
/// packed inside the selection cell.
fn reserve_menu_column(mut row: ViewNode) -> ViewNode {
    // An expandable row's own root is `stack(key, Vertical, [cells, body?])`
    // (`data_table_row_expandable`), not the `Grid` every other row is — the
    // real trailing column always lives on the *cells* `Grid`, so the blank
    // reservation has to find that same nested node rather than pushing
    // onto whichever node it was handed. `reserve_expand_column` already
    // walks into a child keyed `cells` for exactly this shape; this mirrors
    // it on the trailing side.
    let target: &mut ViewNode = if row.kind == NodeKind::Grid {
        &mut row
    } else {
        match row.children.iter_mut().find(|c| c.key.as_str() == "cells") {
            Some(cells) => Arc::make_mut(cells),
            None => &mut row,
        }
    };
    let has = target
        .children
        .last()
        .is_some_and(|c| c.key.as_str() == ROW_MENU);
    if !has {
        target.props.columns.push(TrackSize::FitContent);
        target.children.push(Arc::new(menu_column_blank()));
    }
    row
}

/// The blank [`reserve_menu_column`] inserts: a [`Role::Cell`] with nothing
/// in it, the same width class ([`TrackSize::FitContent`]) as a real
/// [`data_table_row_menu_trigger`] cell so every row's columns still match.
fn menu_column_blank() -> ViewNode {
    let mut cell = stack(ROW_MENU, Axis::Horizontal, None, Vec::new());
    cell.props.padding = Some(cell_padding_inline());
    cell.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    cell
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
            .map(|(i, cell)| as_cell(format!("c{i}"), cell)),
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
    node.constraints.vertical.max = Some(height);
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

/// Wrap `node` as a [`Role::Cell`] keyed `key`, unless it already declared
/// the role itself (e.g. [`data_table_sort_header`]'s own doc comment,
/// which built its cell by hand for exactly this reason).
///
/// Takes the key directly rather than a column index — [`row_shell`]'s
/// call site still spells `format!("c{i}")` positionally, but
/// [`append_menu_column`] wants the stable [`ROW_MENU`] key regardless of
/// how many data columns came before it.
fn as_cell(key: impl Into<Key>, node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Cell) {
        let mut node = node;
        ellipsis_text(&mut node);
        return node;
    }
    let mut inner = node;
    ellipsis_text(&mut inner);
    let mut wrap = stack(key, Axis::Horizontal, None, vec![inner]);
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
        HEIGHT_LG, HEIGHT_SM, HEIGHT_XL, HEIGHT_XS, IconMark, IconTone, SIZE_MD, SortDirection,
        data_table, data_table_batch_bar, data_table_batch_cancel, data_table_menu, data_table_row,
        data_table_row_actions, data_table_row_expandable, data_table_row_expandable_actions,
        data_table_row_lg, data_table_row_md, data_table_row_menu_trigger, data_table_row_sm,
        data_table_row_xl, data_table_row_xs, data_table_skeleton, data_table_sort_header,
        data_table_toolbar, data_table_toolbar_menu, data_table_weights_at, data_table_zebra,
        icon_toned,
    };
    use crate::component::checkbox;
    use crate::component::controls::{CheckState, checkbox_box};
    use crate::component::search;
    use crate::component::text::text;
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_ACCENT, LAYER_SELECTED, SURFACE_BASE,
        TYPOGRAPHY_HEADING_SM,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Point, Size};
    use crate::testing::{Harness, inks, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Anchor, Edge, Fit, Interaction, NodeKind, Props, Registry, Role, ViewNode};

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
            "data table `{}` declared Drag",
            node.key
        );
        for child in &node.children {
            no_drag(child);
        }
    }

    fn drag_only_on_dividers_and_grips(node: &ViewNode) {
        if node.interactions.contains(&Interaction::Drag) {
            let key = node.key.as_str();
            assert!(
                key.starts_with(super::DIVIDER_KEY) || key == super::GRIP_KEY,
                "data table `{}` declared Drag but is not a divider or grip",
                node.key
            );
        }
        for child in &node.children {
            drag_only_on_dividers_and_grips(child);
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
        assert_eq!(
            header.constraints.vertical.max,
            Some(HEIGHT_LG),
            "a dragged-narrow column must not grow the row by wrapping"
        );
        assert_eq!(HEIGHT_LG, 48.0);
        assert_eq!(named(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(header, "c1").semantics.role, Some(Role::Cell));
        let row = named(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(named(row, "select").semantics.role, Some(Role::Cell));
        assert_eq!(named(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(named(row, "c1").semantics.role, Some(Role::Cell));
        drag_only_on_dividers_and_grips(&node);
        assert!(
            named(row, "div0").interactions.contains(&Interaction::Drag),
            "the default table exposes a draggable column separator"
        );
    }

    #[test]
    fn table_cell_text_ellipsizes_rather_than_wrapping() {
        use crate::tree::TextWrap;
        let row = data_table_row("r0", vec![text("n0", "scheduler")], false);
        assert_eq!(named(&row, "n0").props.wrap, Some(TextWrap::Ellipsis));
        let header = data_table_sort_header("h0", "Name", SortDirection::Ascending);
        assert_eq!(named(&header, "name").props.wrap, Some(TextWrap::Ellipsis));
        assert_eq!(
            named(&header, "direction").props.wrap,
            Some(TextWrap::Ellipsis)
        );
    }

    #[test]
    fn dragging_a_divider_stops_at_the_minimum_column() {
        let table = data_table(
            "jobs",
            vec![text("h0", "Name"), text("h1", "Status")],
            vec![data_table_row(
                "r0",
                vec![text("n0", "alpha"), text("s0", "ready")],
                false,
            )],
        );
        let frame = petrify_lone(table);
        let node = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/header/div0"))
            .expect("the header divider is placed")
            .id
            .clone();
        let left = placed(&frame, "/header/c0");
        let right = placed(&frame, "/header/c1");
        let travel = right.rect.right() - left.rect.x - super::DIVIDER;
        let weights = [1.0f32, 1.0];
        let squashed = data_table_weights_at(
            &frame,
            &node,
            Point::new(left.rect.x - 1000.0, left.rect.y + 1.0),
            &weights,
        )
        .expect("a drag on a live divider resolves");
        let width = squashed[0] / (squashed[0] + squashed[1]) * travel;
        assert!(
            (width - super::MIN_COLUMN).abs() < 1.0,
            "the first column was dragged to {width}, past the {} floor",
            super::MIN_COLUMN
        );
        let other = data_table_weights_at(
            &frame,
            &node,
            Point::new(right.rect.right() + 1000.0, left.rect.y + 1.0),
            &weights,
        )
        .expect("a drag past the right end resolves");
        let other_w = other[1] / (other[0] + other[1]) * travel;
        assert!(
            (other_w - super::MIN_COLUMN).abs() < 1.0,
            "the second column was dragged to {other_w}, past the {} floor",
            super::MIN_COLUMN
        );
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
        let cell = data_table_sort_header("col-name", "Name", SortDirection::Ascending);
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

        let desc = data_table_sort_header("col-name", "Name", SortDirection::Descending);
        assert_eq!(
            named(&desc, "sort").semantics.value.as_deref(),
            Some("descending")
        );

        let table = data_table(
            "jobs",
            vec![
                data_table_sort_header("h0", "Name", SortDirection::Ascending),
                text("h1", "Status"),
            ],
            vec![],
        );
        let header = named(&table, "header");
        assert_eq!(named(header, "h0").semantics.role, Some(Role::Cell));
        assert_eq!(named(header, "c1").semantics.role, Some(Role::Cell));
        drag_only_on_dividers_and_grips(&table);
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
        drag_only_on_dividers_and_grips(&node);
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
                data_table_sort_header("h0", "Name", SortDirection::Ascending),
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

    /// A hamburger is wider than the header's blank menu cell, and an
    /// expanded row's chevron is wider than a spacer. FitContent chrome
    /// then shifts every Weight column. Compound row 56 showed that as
    /// three different Name/Status splits.
    #[test]
    fn a_menu_and_an_expand_do_not_shift_the_data_columns() {
        let table = data_table(
            "jobs",
            vec![
                data_table_sort_header("h0", "Name", SortDirection::Ascending),
                text("h1", "Status"),
            ],
            vec![
                data_table_row_actions(
                    "r0",
                    vec![text("n", "a"), text("s", "ready")],
                    false,
                    data_table_row_menu_trigger("m0", "Row"),
                ),
                data_table_row_expandable_actions(
                    "r1",
                    vec![text("n", "b"), text("s", "idle")],
                    false,
                    true,
                    "more",
                    data_table_row_menu_trigger("m1", "Row"),
                ),
            ],
        );
        let frame = petrify_lone(table);
        let x_of = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is not placed"))
                .rect
                .x
        };
        let header_div = x_of("/header/div0");
        assert_eq!(x_of("/r0/div0"), header_div, "plain row divider drifted");
        assert_eq!(
            x_of("/r1/cells/div0"),
            header_div,
            "expandable row divider drifted"
        );
        assert_eq!(x_of("/header/h0"), x_of("/r0/c0"));
        assert_eq!(x_of("/header/h0"), x_of("/r1/cells/c0"));
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

    // ===== T034: the toolbar tier =====

    #[test]
    fn toolbar_grows_search_and_hugs_trailing_controls() {
        let node = data_table_toolbar(
            "toolbar",
            search("search", "Filter rows"),
            vec![text("columns-trigger", "Columns")],
        );
        assert_eq!(node.kind, NodeKind::Grid);
        assert_eq!(node.constraints.vertical.min, Some(HEIGHT_LG));
        assert_eq!(HEIGHT_LG, 48.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_BASE));
        assert_eq!(
            node.props.columns,
            vec![
                crate::tree::TrackSize::Weight { weight: 1.0 },
                crate::tree::TrackSize::FitContent
            ],
            "search grows, the trailing group hugs its content"
        );
        assert_eq!(named(&node, "input").semantics.role, Some(Role::TextInput));
        assert_eq!(
            named(&node, "columns-trigger").props.text.as_deref(),
            Some("Columns")
        );
    }

    /// Falsify by swapping the two `TrackSize`s: the search column would
    /// hug and the trailing column would grow, exactly backwards.
    #[test]
    fn toolbar_search_column_is_the_one_that_grows() {
        let node = data_table_toolbar("toolbar", search("search", "Filter"), vec![]);
        assert_eq!(
            node.props.columns[0],
            crate::tree::TrackSize::Weight { weight: 1.0 }
        );
        assert_eq!(node.props.columns[1], crate::tree::TrackSize::FitContent);
    }

    #[test]
    fn data_table_menu_fits_content_and_hangs_from_the_trailing_edge() {
        let node = data_table_menu("menu", "Columns", vec![checkbox("col-name", "Name", true)]);
        assert_eq!(node.props.fit, Some(Fit::Content));
        match &node.props.anchor {
            Some(Anchor::Sibling {
                key, edge, align, ..
            }) => {
                assert_eq!(key.as_str(), "trigger");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(*align, crate::tree::Align::End);
            }
            other => panic!("expected Sibling trailing-bottom, got {other:?}"),
        }
    }

    fn placed<'a>(frame: &'a PetrifiedFrame, tail: &str) -> &'a crate::frame::Placement {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(tail))
            .unwrap_or_else(|| {
                let ids: Vec<&str> = frame.placements.iter().map(|p| p.id.as_str()).collect();
                panic!("no placement ending {tail}, have {ids:?}")
            })
    }

    #[test]
    fn toolbar_search_shares_the_table_leading_edge() {
        let toolbar = data_table_toolbar(
            "toolbar",
            search("search", "Filter"),
            vec![text("columns-trigger", "Columns")],
        );
        let table = data_table(
            "table",
            vec![text("h0", "Name")],
            vec![data_table_row("r0", vec![text("n0", "alpha")], false)],
        );
        let node = ViewNode::new(NodeKind::Stack, "data-table")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![toolbar, table]);
        let frame = petrify_lone2(node);
        let search = placed(&frame, "/search");
        let table = placed(&frame, "/table");
        assert!(
            (search.rect.x - table.rect.x).abs() < 0.5,
            "search.x={} table.x={}",
            search.rect.x,
            table.rect.x
        );
    }

    #[test]
    fn an_open_columns_menu_wraps_its_rows_and_stays_inside_the_toolbar() {
        let menu = data_table_toolbar_menu(
            "columns",
            "Columns",
            true,
            vec![
                checkbox("col-name", "Name", true),
                checkbox("col-status", "Status", true),
            ],
        );
        let frame = petrify_lone2(data_table_toolbar(
            "toolbar",
            search("search", "Filter"),
            vec![menu],
        ));
        let panel = placed(&frame, "/columns/menu");
        let name = placed(&frame, "/col-name");
        let status = placed(&frame, "/col-status");
        assert!(
            name.rect.x >= panel.rect.x - 0.5
                && name.rect.x + name.rect.w <= panel.rect.x + panel.rect.w + 0.5,
            "Name row is not inside the menu: name={:?} menu={:?}",
            name.rect,
            panel.rect
        );
        assert!(
            status.rect.x >= panel.rect.x - 0.5
                && status.rect.x + status.rect.w <= panel.rect.x + panel.rect.w + 0.5,
            "Status row is not inside the menu: status={:?} menu={:?}",
            status.rect,
            panel.rect
        );
        let toolbar = placed(&frame, "/toolbar");
        assert!(
            panel.rect.x + panel.rect.w <= toolbar.rect.x + toolbar.rect.w + 1.0,
            "menu overflows toolbar: menu={:?} toolbar={:?}",
            panel.rect,
            toolbar.rect
        );
        assert!(
            panel.rect.w >= 160.0 - 0.5,
            "Carbon menu min is 160, got {}",
            panel.rect.w
        );
    }

    #[test]
    fn batch_bar_shows_count_cancel_and_actions_on_the_accent_fill() {
        let node = data_table_batch_bar(
            "batch",
            2,
            data_table_batch_cancel("cancel"),
            vec![text("delete", "Delete")],
        );
        assert_eq!(node.constraints.vertical.min, Some(HEIGHT_LG));
        assert_eq!(token(&node, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(
            named(&node, "count").props.text.as_deref(),
            Some("2 items selected")
        );
        assert_eq!(named(&node, "cancel").semantics.role, Some(Role::Button));
        assert_eq!(
            named(&node, "cancel").semantics.label.as_deref(),
            Some("Cancel selection")
        );
        assert_eq!(named(&node, "delete").props.text.as_deref(), Some("Delete"));
    }

    /// Singular/plural agreement, the second channel a colour-blind reader
    /// gets beside the count itself.
    #[test]
    fn batch_bar_count_text_is_singular_for_exactly_one() {
        let one = data_table_batch_bar("batch", 1, data_table_batch_cancel("cancel"), vec![]);
        assert_eq!(
            named(&one, "count").props.text.as_deref(),
            Some("1 item selected")
        );
        let three = data_table_batch_bar("batch", 3, data_table_batch_cancel("cancel"), vec![]);
        assert_eq!(
            named(&three, "count").props.text.as_deref(),
            Some("3 items selected")
        );
    }

    #[test]
    fn batch_cancel_and_row_menu_trigger_are_icon_only_buttons_with_a_real_label() {
        let cancel = data_table_batch_cancel("cancel");
        assert_eq!(cancel.semantics.role, Some(Role::Button));
        assert_eq!(cancel.semantics.label.as_deref(), Some("Cancel selection"));
        assert!(cancel.interactions.contains(&Interaction::Click));
        assert!(
            !cancel.children.iter().any(|c| c.props.text.is_some()),
            "no visible text; the label is the accessible name alone"
        );

        let trigger = data_table_row_menu_trigger("trigger", "Row actions for alpha");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(
            trigger.semantics.label.as_deref(),
            Some("Row actions for alpha")
        );
        assert_eq!(
            named(&trigger, "glyph").props.canvas,
            icon_toned("glyph", IconMark::Menu, IconTone::Primary)
                .props
                .canvas,
            "no dedicated kebab mark exists; the trigger reuses IconMark::Menu"
        );
    }

    /// [`data_table_row_actions`] gets a real trailing `row-menu` cell; a
    /// plain [`data_table_row`] mixed into the same table gets the blank
    /// [`super::reserve_menu_column`] stands in, and every row (header
    /// included) ends up with the identical column-track count.
    #[test]
    fn a_row_menu_column_is_reserved_on_every_row_and_the_header() {
        let table = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![
                data_table_row("r0", vec![text("n0", "alpha")], false),
                data_table_row_actions(
                    "r1",
                    vec![text("n1", "bravo")],
                    false,
                    data_table_row_menu_trigger("trigger", "Row actions for bravo"),
                ),
            ],
        );
        let header = named(&table, "header");
        let r0 = named(&table, "r0");
        let r1 = named(&table, "r1");
        assert_eq!(header.props.columns.len(), r0.props.columns.len());
        assert_eq!(r0.props.columns.len(), r1.props.columns.len());
        assert_eq!(
            header.children.last().map(|c| c.key.as_str()),
            Some("row-menu")
        );
        assert_eq!(r0.children.last().map(|c| c.key.as_str()), Some("row-menu"));
        assert_eq!(r1.children.last().map(|c| c.key.as_str()), Some("row-menu"));
        assert!(
            named(r0, "row-menu").children.is_empty(),
            "r0 built no menu of its own; its reserved cell is blank"
        );
        assert!(
            named(r1, "row-menu")
                .children
                .iter()
                .any(|c| c.key.as_str() == "trigger"),
            "r1's own trigger survives under its real row-menu cell"
        );

        let plain = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![data_table_row("r0", vec![text("n0", "alpha")], false)],
        );
        assert_ne!(
            named(&plain, "r0").children.last().map(|c| c.key.as_str()),
            Some("row-menu"),
            "a table with no row-menu column reserves nothing"
        );
    }

    /// An expandable row mixed with a row-actions row in the same table:
    /// `data_table_row_expandable`'s own root is a `Stack` of `[cells,
    /// body?]`, not the `Grid` `reserve_menu_column` assumes every row is —
    /// exactly the shape [`reserve_expand_column`] already special-cases by
    /// walking into a child keyed `cells`. The blank reservation must land
    /// on that same nested `Grid`, never as a stray third sibling of `cells`
    /// and `body`.
    #[test]
    fn the_blank_row_menu_column_lands_inside_an_expandable_rows_own_cells_grid() {
        let table = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![
                data_table_row_expandable(
                    "r0",
                    vec![text("n0", "alpha")],
                    false,
                    true,
                    "more detail",
                ),
                data_table_row_actions(
                    "r1",
                    vec![text("n1", "bravo")],
                    false,
                    data_table_row_menu_trigger("trigger", "Row actions for bravo"),
                ),
            ],
        );
        let r0 = named(&table, "r0");
        assert_eq!(
            r0.kind,
            NodeKind::Stack,
            "an expandable row's own root is the vertical stack of cells+body"
        );
        assert!(
            r0.children.iter().all(|c| c.key.as_str() != "row-menu"),
            "the blank must not land as a stray third sibling of cells/body: {:?}",
            r0.children
                .iter()
                .map(|c| c.key.as_str())
                .collect::<Vec<_>>()
        );
        let cells = named(r0, "cells");
        assert_eq!(
            cells.children.last().map(|c| c.key.as_str()),
            Some("row-menu"),
            "the blank belongs on the nested cells Grid, the same place a real trigger would sit"
        );
        assert!(named(cells, "row-menu").children.is_empty());
    }

    /// A skeleton table carries no `Role` and no `Interaction::Focus`
    /// anywhere — falsify by giving `skeleton_row` `Role::Row`: this test
    /// would then find a role and fail its own first assertion.
    #[test]
    fn skeleton_declares_no_role_and_nothing_reachable() {
        let node = data_table_skeleton("skeleton", 3, 4);
        fn walk(node: &ViewNode, roles: &mut Vec<Role>, focusable: &mut bool) {
            if let Some(role) = &node.semantics.role {
                roles.push(role.clone());
            }
            if node.interactions.contains(&Interaction::Focus) {
                *focusable = true;
            }
            for child in &node.children {
                walk(child, roles, focusable);
            }
        }
        let mut roles = Vec::new();
        let mut focusable = false;
        walk(&node, &mut roles, &mut focusable);
        assert!(
            roles.is_empty(),
            "a skeleton must declare no Role: {roles:?}"
        );
        assert!(!focusable, "a skeleton must declare no Interaction::Focus");
    }

    #[test]
    fn skeleton_bars_are_marked_skeleton_ncols_by_nrows_plus_one_header() {
        let node = data_table_skeleton("skeleton", 2, 3);
        fn bars(node: &ViewNode, count: &mut usize) {
            if node.semantics.skeleton {
                *count += 1;
                assert_eq!(
                    node.kind,
                    NodeKind::Spacer,
                    "the skeleton flag belongs to the bar itself, key {:?}",
                    node.key
                );
            }
            for child in &node.children {
                bars(child, count);
            }
        }
        let mut count = 0;
        bars(&node, &mut count);
        assert_eq!(count, 2 * (3 + 1), "2 columns across the header + 3 rows");
    }

    const VIEWPORT2: Size = Size { w: 900.0, h: 700.0 };

    fn petrify_lone2(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT2, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    /// Check C/D across the toolbar, the batch bar and a skeleton table:
    /// real rects, nothing overflowing its parent.
    #[test]
    fn toolbar_tier_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for node in [
            data_table_toolbar(
                "toolbar",
                search("search", "Filter rows"),
                vec![text("columns-trigger", "Columns")],
            ),
            data_table_batch_bar(
                "batch",
                2,
                data_table_batch_cancel("cancel"),
                vec![text("delete", "Delete")],
            ),
            data_table_skeleton("skeleton", 3, 2),
        ] {
            let frame = petrify_lone2(node);
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
    }

    /// Check F: the batch bar's cancel control and a row-menu trigger are
    /// reachable; nothing in a skeleton table is.
    #[test]
    fn row_menu_and_batch_cancel_are_reachable_and_skeleton_is_not() {
        let batch = data_table_batch_bar(
            "batch",
            1,
            data_table_batch_cancel("cancel"),
            vec![text("delete", "Delete")],
        );
        let frame = petrify_lone2(batch);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let cancel = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/cancel"))
            .expect("cancel is placed");
        assert!(order.iter().any(|o| o == &cancel.id));

        let table = data_table(
            "jobs",
            vec![text("h0", "Name")],
            vec![data_table_row_actions(
                "r0",
                vec![text("n0", "alpha")],
                false,
                data_table_row_menu_trigger("trigger", "Row actions for alpha"),
            )],
        );
        let frame = petrify_lone2(table);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let trigger = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the row-menu trigger is placed");
        assert!(order.iter().any(|o| o == &trigger.id));

        let skeleton = data_table_skeleton("skeleton", 2, 2);
        let frame = petrify_lone2(skeleton);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        assert!(
            focus.order().is_empty(),
            "nothing in a skeleton table is reachable"
        );
    }

    /// Check E: the batch bar's count text and cancel glyph against its
    /// own `ACCENT_PRIMARY` fill, in both themes — the substitution this
    /// module's doc comment claims for Carbon's `$background-brand`.
    #[test]
    fn batch_bar_text_and_glyph_clear_aa_contrast_against_the_accent_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = data_table_batch_bar(
                "batch",
                2,
                data_table_batch_cancel("cancel"),
                vec![text("delete", "Delete")],
            );
            let bg_name = node
                .props
                .tokens
                .get("background")
                .expect("batch bar binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let cancel = named(&node, "cancel");
            for (label, part) in [
                ("count", named(&node, "count")),
                ("cancel/glyph", named(cancel, "glyph")),
            ] {
                let names = inks(part);
                assert!(!names.is_empty(), "{label} binds an ink");
                let opacity = part.props.opacity.unwrap_or(1.0);
                for fg_name in names {
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{label} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        fg_name.as_str()
                    );
                }
            }
        }
    }
}
