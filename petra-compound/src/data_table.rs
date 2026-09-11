//! Data table as a [`Compound`]: sort, selection, expansion, and the T034
//! toolbar tier all live here.
//!
//! The constructors in `gorgon_petra::component` stay put. They take every
//! piece of state as an argument (`data_table_row`, `data_table_row_expandable`,
//! `data_table_sort_header`) and derive the tri-state select-all from the
//! rows the caller passed. This module is the caller that holds that state.
//!
//! Binding: spec 009 T006, `view-fiber.md` §4.1. `update` does not rewrite
//! the row list; sort is a presentation of [`Props`] applied in [`DataTable::view`].
//!
//! # T034: search, column visibility, batch actions, row menu, skeleton
//!
//! `data_table.rs:41` (`gorgon_petra::component::data_table`'s own module
//! doc) named five things it deliberately left out and pointed here for
//! them. All five are state a caller holds across renders — exactly the
//! compound's job, never an atomic's.
//!
//! **Search/filter lives in `view`, not `update`.** [`Compound::update`]
//! does not see [`Props`] (`view-fiber.md` §4.1), so it cannot filter
//! anything; [`Combobox::view`](crate::combobox::Combobox::view) and
//! [`Command::view`](crate::command::Command::view) already establish this
//! for exactly the same reason, both through [`crate::filter::filter_indices`].
//! [`visible_indices`] is this module's own version: filter first (the
//! haystack is every visible cell in the row, space-joined, so a search
//! matches any column), then apply the same sort [`sorted_indices`] already
//! did, over the filtered set rather than the full one.
//!
//! **Column visibility is [`State`], not [`Props`].** `State` is
//! everything the compound changes and the author never supplies
//! (`crate::Compound`'s own doc); hiding a column is exactly that — the
//! same shelf `State::selection` and `State::expansion` already sit on,
//! not a second "hidden" list the author would have to keep synchronised
//! with clicks it never sees. A hidden column's header cell and every
//! row's matching cell disappear together because both
//! [`header_cells`] and [`body_row`] filter the *same*
//! [`Props::columns`] by the *same* [`State::hidden_columns`] set.
//!
//! **The batch bar is a function of the selection, never a second flag
//! that could disagree with it.** [`view`](DataTable::view) reads
//! `state.selection.is_empty()` directly; there is no `State::batch_mode`
//! to drift out of sync with `SelectAll`/`SelectRow`. The tri-state
//! select-all already existed; this is that same fact read a second way.
//!
//! **`Intent::BatchAction` and `Intent::RowAction` do not perform anything
//! — same as [`Command`](crate::command::Command)'s own `Intent::Choose`.**
//! That module's doc says it outright: declaring is not the same as
//! dispatching, and the dispatch half does not exist yet anywhere in this
//! tree (spec 010 T046, still open). A host dispatching either intent
//! already holds the raw `Intent` value — the action id is *in* the
//! message it is about to call `update` with — so nothing is lost by
//! `update`'s own handling being "return to rest": clear the selection
//! after a batch action (Carbon's batch action bar closes once its action
//! runs), close the row menu after a row action (the same shape
//! `Intent::Choose` already uses for Combobox and Command).
//!
//! **Skeleton is a [`Props`] field, not a second constructor.** Loading is
//! the author's own fact — network still in flight, nothing the compound
//! decided — so it belongs where [`Props::columns`] and [`Props::rows`]
//! already live. `Compound::view` has exactly one entry point a real host
//! ever calls; a *second* "loading view" function would never be reached
//! by anything, the same reasoning spec 010's still-open T046 note applies
//! to an intent nobody dispatches. [`data_table_skeleton`]'s own doc is
//! the rest of the "not confused with a real row" argument: no `Role`
//! anywhere in a skeleton table, so neither a screen reader nor
//! [`crate::Compound`]'s own selection state has anything there to find.

use std::collections::BTreeSet;

use gorgon_petra::component::kit::stack;
use gorgon_petra::component::{
    SortDirection, checkbox, data_table, data_table_batch_action, data_table_batch_bar,
    data_table_batch_cancel, data_table_row, data_table_row_actions, data_table_row_expandable,
    data_table_row_expandable_actions, data_table_row_menu_trigger, data_table_skeleton,
    data_table_sort_header, data_table_toolbar, data_table_toolbar_menu, menu, menu_item, search,
    text, valued,
};
use gorgon_petra::tree::ViewNode;
use gorgon_petra::{Align, Axis};
use serde::{Deserialize, Serialize};

use crate::Compound;
use crate::filter::filter_indices;

/// Namespace over the data table's triple. Never constructed as a value.
pub struct DataTable;

/// Root key of the node [`DataTable::view`] returns while loading
/// ([`Props::loading`]) — a bare [`data_table_skeleton`], no toolbar tier.
/// The *table itself* keeps this same key when not loading too, nested one
/// level under [`ROOT_KEY`]; `crate::filter_indices`'s callers only ever
/// searched by key, never assumed the compound's own root was the table
/// (`gorgon-petra-egui`'s row-56 page already does `find(&tree, "table")`,
/// not `tree.key == "table"`), so nothing outside this crate breaks.
const TABLE_KEY: &str = "table";
/// Root key of the node [`DataTable::view`] returns while not loading: the
/// toolbar tier (search-and-filter toolbar or the batch action bar) above
/// [`TABLE_KEY`]'s own table.
const ROOT_KEY: &str = "data-table";
/// The column-visibility [`menu_button`]'s own key, and the key every
/// checkbox item inside it is prefixed with.
const COLUMNS_KEY: &str = "columns";
/// Key prefix of a column-visibility checkbox: `col-{Column::id}`.
const COLUMN_CHECK_PREFIX: &str = "col-";
/// Key prefix of a row-menu item: `row-action-{ActionItem::id}`.
const ROW_ACTION_PREFIX: &str = "row-action-";
/// Key prefix of a batch-bar action button: `batch-action-{ActionItem::id}`.
/// Distinct from [`ROW_ACTION_PREFIX`] on purpose: a row menu can be open
/// at the same time the batch bar shows (they key off unrelated `State`
/// fields), so an author naming the same [`ActionItem::id`] in both lists
/// must not collide into one node key.
const BATCH_ACTION_PREFIX: &str = "batch-action-";
/// Placeholder rows to show while [`Props::loading`] and [`Props::rows`]
/// is still empty — a cold load has no row count of its own to draw from.
const DEFAULT_SKELETON_ROWS: usize = 3;

/// A column the author names. `id` is the sort key and the header node key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// Stable identity. [`Intent::SortBy`] names this.
    pub id: String,
    /// Visible header caption.
    pub label: String,
}

impl Column {
    /// A column identified by `id` and labelled `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// One body row the author supplies. `cells` line up with [`Props::columns`]
/// by index. Extra cells are ignored; missing cells render as empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Stable identity. Selection and expansion name this.
    pub id: String,
    /// Cell text, one per column.
    pub cells: Vec<String>,
    /// Present when the row can expand. Mounted only while `id` is in
    /// [`State::expansion`].
    pub body: Option<String>,
}

impl Row {
    /// A body row that does not expand. `id` must be unique among the
    /// table's rows: it is the node key.
    #[must_use]
    pub fn new(id: impl Into<String>, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
            body: None,
        }
    }

    /// Mark this row expandable. `body` mounts only while expanded.
    #[must_use]
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// One action the author names: a batch action in [`data_table_batch_bar`]
/// or a row action in a row's own [`data_table_row_menu_trigger`] menu. The
/// same shape either way — an id [`Intent::BatchAction`]/[`Intent::RowAction`]
/// names back and a visible caption — so one type serves both lists rather
/// than two identical structs with different names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionItem {
    /// Stable identity. What the fired intent names.
    pub id: String,
    /// Visible caption.
    pub label: String,
}

impl ActionItem {
    /// An action identified by `id` and labelled `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Rows and columns. The author owns these; the compound never writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    /// Column order is the header order and the cell index order.
    pub columns: Vec<Column>,
    /// Body rows in author order. [`view`](DataTable::view) may present them
    /// in a different order when a sort or a search is active; `rows`
    /// itself is not rewritten.
    pub rows: Vec<Row>,
    /// Actions [`data_table_batch_bar`] offers while any row is selected.
    /// Empty is fine — the bar still shows Cancel and the selection count.
    pub batch_actions: Vec<ActionItem>,
    /// Actions each row's own menu offers. Empty means no row grows a
    /// [`data_table_row_menu_trigger`] at all — the whole trailing column
    /// disappears rather than showing an always-empty menu.
    pub row_actions: Vec<ActionItem>,
    /// Content has not arrived yet. `true` replaces the whole table (and
    /// its toolbar) with [`data_table_skeleton`]; `rows`/`columns` are
    /// still read for the skeleton's own column count and, when `rows` is
    /// non-empty (a refresh over already-loaded content), its row count.
    pub loading: bool,
}

/// Active sort: which column, and which way.
///
/// Order is lexicographic on the cell text (`str::cmp`), case-sensitive,
/// and stable on ties so author order among equal cells is kept.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sort {
    /// [`Column::id`] being sorted.
    pub column: String,
    /// `true` is ascending.
    pub ascending: bool,
}

/// Sort, selection, expansion, and the toolbar tier's own controls.
/// Round-trips through reload as a whole.
///
/// No field is `serde(skip)`: losing any of these on reload would surprise
/// the operator (`view-fiber.md` §6, round-trip is the default).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// `None` means author order.
    pub sort: Option<Sort>,
    /// Selected row ids. Ids that no longer appear in [`Props::rows`] stay
    /// in the set until an intent removes them; they do not affect the
    /// tri-state select-all, which the atomic derives from the rows `view`
    /// actually builds.
    pub selection: BTreeSet<String>,
    /// Expanded row ids. Same stale-id rule as [`Self::selection`].
    pub expansion: BTreeSet<String>,
    /// Text in the toolbar's search field. Also the filter needle
    /// [`visible_indices`] reads — empty means every row.
    pub query: String,
    /// [`Column::id`]s hidden by the toolbar's column-visibility menu.
    /// Same stale-id rule as [`Self::selection`].
    pub hidden_columns: BTreeSet<String>,
    /// Whether the column-visibility menu is mounted.
    pub column_menu_open: bool,
    /// Which row's own menu is mounted, if any. Only one at a time: opening
    /// a second row's menu replaces this rather than adding to a set,
    /// because Carbon shows one row menu open at a time (opening a second
    /// overflow menu closes the first) and a `BTreeSet` here would let two
    /// disagree about which the operator meant to act on.
    pub row_menu_open: Option<String>,
}

/// Closed set of things that can happen to the table.
///
/// `Deserialize` so [`gorgon_view_fiber::ViewFiber`] can decode it off the
/// `ui:intent` bus (`ViewFiber`'s `C::Intent: DeserializeOwned` bound).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    /// Toggle `id` in [`State::selection`].
    SelectRow {
        /// [`Row::id`].
        id: String,
    },
    /// Select every listed id, or drop those ids if every listed id is
    /// already selected. Ids not in the list are left alone.
    ///
    /// The host passes the current row ids because `update` does not see
    /// [`Props`] (`view-fiber.md` §4.1). An empty `ids` is a no-op.
    SelectAll {
        /// Current [`Row::id`]s, in any order.
        ids: Vec<String>,
    },
    /// Toggle `id` in [`State::expansion`].
    ToggleExpand {
        /// [`Row::id`].
        id: String,
    },
    /// Sort by `column`. A second SortBy on the same column flips
    /// direction. A different column starts ascending.
    SortBy {
        /// [`Column::id`].
        column: String,
    },
    /// Replace [`State::query`]. Filtering itself happens in
    /// [`view`](DataTable::view) (`update` does not see [`Props`]); this
    /// intent only records the needle.
    Search {
        /// New search text.
        query: String,
    },
    /// Toggle [`Column::id`] in [`State::hidden_columns`].
    ToggleColumn {
        /// [`Column::id`].
        id: String,
    },
    /// Mount or unmount the column-visibility menu.
    ToggleColumnMenu,
    /// Empty [`State::selection`] — the batch bar's own "Cancel" control.
    ClearSelection,
    /// A batch action fired. `id` is unused by `update` itself (see the
    /// module doc's "does not perform anything" note); clears the
    /// selection, the same "return to rest" [`Intent::Choose`]-style
    /// handling `Command`'s own doc explains.
    BatchAction {
        /// [`ActionItem::id`].
        id: String,
    },
    /// Mount or unmount `id`'s own row menu. Opening a different row's menu
    /// replaces [`State::row_menu_open`] rather than adding to it.
    ToggleRowMenu {
        /// [`Row::id`].
        id: String,
    },
    /// A row action fired against `id`. `action` is unused by `update`
    /// itself, the same reason [`Intent::BatchAction`]'s is; closes the row
    /// menu.
    RowAction {
        /// [`Row::id`] the menu belonged to.
        id: String,
        /// [`ActionItem::id`].
        action: String,
    },
}

impl Compound for DataTable {
    type Props = Props;
    type State = State;
    type Intent = Intent;
    type Request = ();

    fn init(_props: &Self::Props) -> Self::State {
        State::default()
    }

    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request> {
        match intent {
            Intent::SelectRow { id } => {
                if !state.selection.remove(&id) {
                    state.selection.insert(id);
                }
            }
            Intent::SelectAll { ids } => {
                if ids.is_empty() {
                    return Vec::new();
                }
                let all_on = ids.iter().all(|id| state.selection.contains(id));
                if all_on {
                    for id in ids {
                        state.selection.remove(&id);
                    }
                } else {
                    state.selection.extend(ids);
                }
            }
            Intent::ToggleExpand { id } => {
                if !state.expansion.remove(&id) {
                    state.expansion.insert(id);
                }
            }
            Intent::SortBy { column } => match &mut state.sort {
                Some(sort) if sort.column == column => {
                    sort.ascending = !sort.ascending;
                }
                _ => {
                    state.sort = Some(Sort {
                        column,
                        ascending: true,
                    });
                }
            },
            Intent::Search { query } => {
                state.query = query;
            }
            Intent::ToggleColumn { id } => {
                if !state.hidden_columns.remove(&id) {
                    state.hidden_columns.insert(id);
                }
            }
            Intent::ToggleColumnMenu => {
                state.column_menu_open = !state.column_menu_open;
            }
            Intent::ClearSelection => {
                state.selection.clear();
            }
            Intent::BatchAction { id: _ } => {
                state.selection.clear();
            }
            Intent::ToggleRowMenu { id } => {
                state.row_menu_open = if state.row_menu_open.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    Some(id)
                };
            }
            Intent::RowAction { id: _, action: _ } => {
                state.row_menu_open = None;
            }
        }
        Vec::new()
    }

    fn view(state: &Self::State, props: &Self::Props) -> ViewNode {
        if props.loading {
            let nrows = if props.rows.is_empty() {
                DEFAULT_SKELETON_ROWS
            } else {
                props.rows.len()
            };
            return data_table_skeleton(TABLE_KEY, props.columns.len(), nrows);
        }

        let header = header_cells(state, props);
        let rows = visible_indices(state, props)
            .into_iter()
            .map(|i| body_row(state, props, &props.rows[i]))
            .collect();
        let table = data_table(TABLE_KEY, header, rows);

        let bar = if state.selection.is_empty() {
            toolbar(state, props)
        } else {
            batch_bar(state, props)
        };

        let mut node = stack(ROOT_KEY, Axis::Vertical, None, vec![bar, table]);
        node.props.align = Some(Align::Stretch);
        node
    }
}

fn cell_at(row: &Row, idx: usize) -> &str {
    row.cells.get(idx).map(String::as_str).unwrap_or("")
}

/// [`Props::rows`] indices whose [`State::query`] matches, in [`State::sort`]
/// order. `header_cells`/[`body_row`]/`data_table` never read `Props::rows`
/// positionally; this is the one list [`DataTable::view`] hands the table.
fn visible_indices(state: &State, props: &Props) -> Vec<usize> {
    sort_indices(state, props, matching_rows(state, props))
}

/// The search half of [`visible_indices`]: a row matches if *any* visible
/// column's cell contains [`State::query`] (case-insensitive substring).
///
/// [`filter_indices`] takes one `Fn(&T) -> &str` haystack, borrowed from the
/// item itself — a multi-column row has no single borrowed `&str` to hand
/// it (joining cells would need an owned `String`, which the closure's
/// return type cannot be). So this calls `filter_indices` once *per
/// column* — never touching its signature — and unions the results in a
/// `BTreeSet`, which both dedupes a row matching two columns and keeps
/// ascending index order, i.e. author order, without a separate sort.
/// Skips columns [`State::hidden_columns`] has hidden: a search should not
/// match text the operator just hid from the table entirely.
fn matching_rows(state: &State, props: &Props) -> Vec<usize> {
    if state.query.is_empty() {
        return (0..props.rows.len()).collect();
    }
    let mut matched: BTreeSet<usize> = BTreeSet::new();
    for (col, column) in props.columns.iter().enumerate() {
        if state.hidden_columns.contains(&column.id) {
            continue;
        }
        matched.extend(filter_indices(&props.rows, &state.query, |row| {
            cell_at(row, col)
        }));
    }
    matched.into_iter().collect()
}

/// The sort half of [`visible_indices`]: `indices` in [`State::sort`]
/// order, author order (ties, and no active/unknown sort) preserved by
/// `sort_by`'s stability.
fn sort_indices(state: &State, props: &Props, mut indices: Vec<usize>) -> Vec<usize> {
    let Some(sort) = state.sort.as_ref() else {
        return indices;
    };
    let Some(col) = props.columns.iter().position(|c| c.id == sort.column) else {
        return indices;
    };
    indices.sort_by(|&a, &b| {
        let ord = cell_at(&props.rows[a], col).cmp(cell_at(&props.rows[b], col));
        if sort.ascending { ord } else { ord.reverse() }
    });
    indices
}

/// Every *visible* column is a sort header so a click can become
/// [`Intent::SortBy`]; a column in [`State::hidden_columns`] drops out
/// here exactly as its matching [`body_row`] cell does, so the two never
/// disagree about which columns the table shows.
///
/// A column that is not the active sort shows [`SortDirection::Sortable`],
/// never a forced `Ascending` — [`data_table_sort_header`] has a real
/// unsorted spelling now, so this stops claiming a direction nothing chose.
fn header_cells(state: &State, props: &Props) -> Vec<ViewNode> {
    props
        .columns
        .iter()
        .filter(|col| !state.hidden_columns.contains(&col.id))
        .map(|col| {
            let direction = state.sort.as_ref().filter(|s| s.column == col.id).map_or(
                SortDirection::Sortable,
                |s| {
                    if s.ascending {
                        SortDirection::Ascending
                    } else {
                        SortDirection::Descending
                    }
                },
            );
            data_table_sort_header(col.id.as_str(), col.label.as_str(), direction)
        })
        .collect()
}

/// A body row: visible cells (same [`State::hidden_columns`] filter as
/// [`header_cells`]), plus, when [`Props::row_actions`] is non-empty, a
/// trailing [`row_menu_control`] — on **every** row, expandable or not.
///
/// Until 2026-09-11 an expandable row never grew one, documented here as a
/// scope limit on the grounds that the chevron and the menu trigger wanted
/// the same edge. They do not, and the visible cost was a table whose rows
/// disagreed: three of four carrying a menu glyph and the expanded one
/// carrying a blank, which reads as a bug rather than as a rule.
/// `data_table_row_expandable_actions` is the constructor that closes it.
fn body_row(state: &State, props: &Props, row: &Row) -> ViewNode {
    let selected = state.selection.contains(&row.id);
    let cells: Vec<ViewNode> = props
        .columns
        .iter()
        .enumerate()
        .filter(|(_, col)| !state.hidden_columns.contains(&col.id))
        .map(|(i, col)| text(col.id.as_str(), cell_at(row, i)))
        .collect();
    let menu =
        (!props.row_actions.is_empty()).then(|| row_menu_control(state, props, row.id.as_str()));
    match (row.body.as_deref(), menu) {
        (Some(body), Some(menu)) => {
            let expanded = state.expansion.contains(&row.id);
            data_table_row_expandable_actions(
                row.id.as_str(),
                cells,
                selected,
                expanded,
                body,
                menu,
            )
        }
        (Some(body), None) => {
            let expanded = state.expansion.contains(&row.id);
            data_table_row_expandable(row.id.as_str(), cells, selected, expanded, body)
        }
        (None, Some(menu)) => data_table_row_actions(row.id.as_str(), cells, selected, menu),
        (None, None) => data_table_row(row.id.as_str(), cells, selected),
    }
}

/// A row's own trigger-plus-overlay control: [`data_table_row_menu_trigger`]
/// keyed `"trigger"` (the literal key [`menu`]'s anchor always names), plus
/// — while [`State::row_menu_open`] names this row — a [`menu`] of
/// [`Props::row_actions`] beside it. The same trigger-then-conditional-menu
/// shape [`super::menu_button::menu_button`] builds, hand-rolled here
/// because that constructor's trigger is a full-width primary button and a
/// per-row menu wants an icon-only one instead.
fn row_menu_control(state: &State, props: &Props, row_id: &str) -> ViewNode {
    let open = state.row_menu_open.as_deref() == Some(row_id);
    let label = format!("Row actions for {row_id}");
    let mut trigger = data_table_row_menu_trigger("trigger", label.clone());
    trigger.semantics.expanded = Some(open);
    let mut children = vec![trigger];
    if open {
        let items = props
            .row_actions
            .iter()
            .map(|a| menu_item(format!("{ROW_ACTION_PREFIX}{}", a.id), a.label.as_str()))
            .collect();
        children.push(menu("menu", label, items));
    }
    let mut node = stack("row-menu-ctl", Axis::Horizontal, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// [`data_table_toolbar`]: the search field plus, when there is at least
/// one column to hide, the column-visibility [`menu_button`].
fn toolbar(state: &State, props: &Props) -> ViewNode {
    let field = valued(search("search", "Search"), state.query.as_str());
    let mut trailing = Vec::new();
    if !props.columns.is_empty() {
        trailing.push(column_menu(state, props));
    }
    data_table_toolbar("toolbar", field, trailing)
}

/// The column-visibility trigger: one [`checkbox`] per [`Props::columns`],
/// checked unless [`State::hidden_columns`] holds its id.
fn column_menu(state: &State, props: &Props) -> ViewNode {
    let items = props
        .columns
        .iter()
        .map(|col| {
            checkbox(
                format!("{COLUMN_CHECK_PREFIX}{}", col.id),
                col.label.as_str(),
                !state.hidden_columns.contains(&col.id),
            )
        })
        .collect();
    // `data_table_toolbar_menu`, not `menu_button`: a Carbon Menu button is
    // a primary call to action, 40 tall with a 160 minimum, and this control
    // sits inside a 48-tall toolbar sharing its ground. That constructor's
    // own doc carries the measurement that settled it.
    data_table_toolbar_menu(COLUMNS_KEY, "Columns", state.column_menu_open, items)
}

/// [`data_table_batch_bar`]: the live selection count, [`Intent::ClearSelection`]'s
/// own cancel control, and one [`data_table_batch_action`] per
/// [`Props::batch_actions`].
fn batch_bar(state: &State, props: &Props) -> ViewNode {
    let cancel = data_table_batch_cancel("cancel");
    let actions = props
        .batch_actions
        .iter()
        .map(|a| {
            data_table_batch_action(format!("{BATCH_ACTION_PREFIX}{}", a.id), a.label.as_str())
        })
        .collect();
    data_table_batch_bar("batch", state.selection.len(), cancel, actions)
}

#[cfg(test)]
mod tests {
    use gorgon_petra::tree::{Role, ViewNode};

    use crate::Compound;

    use super::{ActionItem, Column, DataTable, Intent, Props, Row, Sort, TABLE_KEY};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn sample_props() -> Props {
        Props {
            columns: vec![Column::new("name", "Name"), Column::new("status", "Status")],
            rows: vec![
                Row::new("r0", ["charlie", "ready"]),
                Row::new("r1", ["alpha", "busy"]).with_body("more"),
                Row::new("r2", ["bravo", "ready"]),
            ],
            ..Props::default()
        }
    }

    /// The nested table [`DataTable::view`] builds under its own root
    /// (`"data-table"`) — see [`TABLE_KEY`]'s own doc for why the root is
    /// no longer the table itself now that T034's toolbar tier sits above
    /// it.
    fn table_of(node: &ViewNode) -> &ViewNode {
        named(node, TABLE_KEY)
    }

    fn row_keys(table: &ViewNode) -> Vec<&str> {
        table
            .children
            .iter()
            .skip(1)
            .map(|c| c.key.as_str())
            .collect()
    }

    #[test]
    fn init_is_empty_selection_no_sort_no_expansion() {
        let state = DataTable::init(&sample_props());
        assert!(state.sort.is_none());
        assert!(state.selection.is_empty());
        assert!(state.expansion.is_empty());
    }

    #[test]
    fn select_row_toggles() {
        let mut state = DataTable::init(&sample_props());
        assert!(DataTable::update(&mut state, Intent::SelectRow { id: "r1".into() }).is_empty());
        assert!(state.selection.contains("r1"));
        DataTable::update(&mut state, Intent::SelectRow { id: "r1".into() });
        assert!(!state.selection.contains("r1"));
    }

    #[test]
    fn select_all_selects_then_clears() {
        let mut state = DataTable::init(&sample_props());
        let ids = vec!["r0".into(), "r1".into(), "r2".into()];
        DataTable::update(&mut state, Intent::SelectAll { ids: ids.clone() });
        assert_eq!(state.selection.len(), 3);
        DataTable::update(&mut state, Intent::SelectAll { ids });
        assert!(state.selection.is_empty());
    }

    #[test]
    fn select_all_deselect_leaves_unlisted_ids() {
        let mut state = DataTable::init(&sample_props());
        DataTable::update(&mut state, Intent::SelectRow { id: "stale".into() });
        let ids = vec!["r0".into(), "r1".into(), "r2".into()];
        DataTable::update(&mut state, Intent::SelectAll { ids: ids.clone() });
        assert!(state.selection.contains("stale"));
        DataTable::update(&mut state, Intent::SelectAll { ids });
        assert!(state.selection.contains("stale"));
        assert_eq!(state.selection.len(), 1);
    }

    #[test]
    fn select_all_from_partial_selects_the_rest() {
        let mut state = DataTable::init(&sample_props());
        DataTable::update(&mut state, Intent::SelectRow { id: "r0".into() });
        DataTable::update(
            &mut state,
            Intent::SelectAll {
                ids: vec!["r0".into(), "r1".into(), "r2".into()],
            },
        );
        assert_eq!(state.selection.len(), 3);
        assert!(state.selection.contains("r1"));
    }

    #[test]
    fn select_all_with_empty_ids_is_a_noop() {
        let mut state = DataTable::init(&sample_props());
        DataTable::update(&mut state, Intent::SelectRow { id: "r0".into() });
        DataTable::update(&mut state, Intent::SelectAll { ids: vec![] });
        assert!(state.selection.contains("r0"));
        assert_eq!(state.selection.len(), 1);
    }

    #[test]
    fn sort_by_sets_then_flips_then_moves() {
        let mut state = DataTable::init(&sample_props());
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        assert_eq!(
            state.sort,
            Some(Sort {
                column: "name".into(),
                ascending: true,
            })
        );
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        assert_eq!(state.sort.as_ref().map(|s| s.ascending), Some(false));
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "status".into(),
            },
        );
        assert_eq!(
            state.sort,
            Some(Sort {
                column: "status".into(),
                ascending: true,
            })
        );
    }

    #[test]
    fn toggle_expand_toggles() {
        let mut state = DataTable::init(&sample_props());
        DataTable::update(&mut state, Intent::ToggleExpand { id: "r1".into() });
        assert!(state.expansion.contains("r1"));
        DataTable::update(&mut state, Intent::ToggleExpand { id: "r1".into() });
        assert!(!state.expansion.contains("r1"));
    }

    #[test]
    fn view_is_a_table_and_does_not_rewrite_props() {
        let props = sample_props();
        let original = props.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        let state = DataTable::init(&props);
        let node = DataTable::view(&state, &props);
        let table = table_of(&node);
        assert_eq!(table.key.as_str(), TABLE_KEY);
        assert_eq!(table.semantics.role, Some(Role::Table));
        assert_eq!(row_keys(table), vec!["r0", "r1", "r2"]);
        assert_eq!(
            props.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            original.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }

    #[test]
    fn view_sorts_without_rewriting_props() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["r1", "r2", "r0"]);
        assert_eq!(props.rows[0].id, "r0");
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["r0", "r2", "r1"]);
    }

    #[test]
    fn unknown_sort_column_keeps_author_order() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "nope".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["r0", "r1", "r2"]);
    }

    #[test]
    fn select_all_checkbox_tracks_none_some_all() {
        let props = sample_props();
        let mut state = DataTable::init(&props);

        let none = DataTable::view(&state, &props);
        let all = named(&none, "select-all");
        assert_eq!(all.semantics.role, Some(Role::Button));
        assert!(!all.semantics.selected);
        assert_eq!(all.semantics.value, None);

        DataTable::update(&mut state, Intent::SelectRow { id: "r0".into() });
        let some = DataTable::view(&state, &props);
        let all = named(&some, "select-all");
        assert!(!all.semantics.selected);
        assert_eq!(all.semantics.value.as_deref(), Some("mixed"));
        assert!(named(&some, "r0").semantics.selected);
        assert!(!named(&some, "r1").semantics.selected);

        DataTable::update(
            &mut state,
            Intent::SelectAll {
                ids: vec!["r0".into(), "r1".into(), "r2".into()],
            },
        );
        let every = DataTable::view(&state, &props);
        let all = named(&every, "select-all");
        assert!(all.semantics.selected);
        assert_eq!(all.semantics.value, None);
        assert!(named(&every, "r0").semantics.selected);
        assert!(named(&every, "r1").semantics.selected);
        assert!(named(&every, "r2").semantics.selected);
    }

    #[test]
    fn expandable_row_mounts_body_only_when_open() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        let shut = DataTable::view(&state, &props);
        let r1 = named(&shut, "r1");
        assert_eq!(r1.semantics.expanded, Some(false));
        assert!(
            r1.children.iter().all(|c| c.key.as_str() != "body"),
            "collapsed row must not mount a body child"
        );

        DataTable::update(&mut state, Intent::ToggleExpand { id: "r1".into() });
        let open = DataTable::view(&state, &props);
        let r1 = named(&open, "r1");
        assert_eq!(r1.semantics.expanded, Some(true));
        assert_eq!(named(r1, "body-text").props.text.as_deref(), Some("more"));
    }

    #[test]
    fn missing_cells_render_empty_and_extra_cells_are_dropped() {
        let props = Props {
            columns: vec![Column::new("a", "A"), Column::new("b", "B")],
            rows: vec![
                Row::new("short", ["only"]),
                Row::new("long", ["x", "y", "z"]),
            ],
            ..Props::default()
        };
        let state = DataTable::init(&props);
        let node = DataTable::view(&state, &props);
        assert_eq!(
            named(named(&node, "short"), "a").props.text.as_deref(),
            Some("only")
        );
        assert_eq!(
            named(named(&node, "short"), "b").props.text.as_deref(),
            Some("")
        );
        assert_eq!(
            named(named(&node, "long"), "a").props.text.as_deref(),
            Some("x")
        );
        assert_eq!(
            named(named(&node, "long"), "b").props.text.as_deref(),
            Some("y")
        );
    }

    #[test]
    fn sort_keeps_author_order_on_ties() {
        let props = Props {
            columns: vec![Column::new("k", "K")],
            rows: vec![
                Row::new("a", ["same"]),
                Row::new("b", ["same"]),
                Row::new("c", ["same"]),
            ],
            ..Props::default()
        };
        let mut state = DataTable::init(&props);
        DataTable::update(&mut state, Intent::SortBy { column: "k".into() });
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_rows_still_make_a_table() {
        let props = Props {
            columns: vec![Column::new("a", "A")],
            rows: vec![],
            ..Props::default()
        };
        let node = DataTable::view(&DataTable::init(&props), &props);
        let table = table_of(&node);
        assert_eq!(table.semantics.role, Some(Role::Table));
        assert!(row_keys(table).is_empty());
        let all = named(table, "select-all");
        assert!(!all.semantics.selected);
        assert_eq!(all.semantics.value, None);
    }

    #[test]
    fn sort_header_carries_the_active_direction() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(
            named(named(&node, "name"), "sort")
                .semantics
                .value
                .as_deref(),
            Some("ascending")
        );
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(
            named(named(&node, "name"), "sort")
                .semantics
                .value
                .as_deref(),
            Some("descending")
        );
    }

    /// The frame-level property `sort_header_carries_the_active_direction`
    /// does not check: with the active sort on one column, every *other*
    /// sortable column must not also claim `"ascending"` or `"descending"`.
    /// This is the property that would have caught `header_cells`'s earlier
    /// `unwrap_or(true)` — every non-active column claiming ascending too.
    #[test]
    fn at_most_one_column_declares_an_active_sort_direction() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        let header = named(&node, "header");
        let values: Vec<Option<String>> = header
            .children
            .iter()
            .filter_map(|cell| find(cell, "sort"))
            .map(|sort| sort.semantics.value.clone())
            .collect();
        assert_eq!(values.len(), 2, "sample_props has two columns");
        let active = values
            .iter()
            .filter(|v| matches!(v.as_deref(), Some("ascending") | Some("descending")))
            .count();
        assert_eq!(
            active, 1,
            "exactly one column is the active sort: {values:?}"
        );
        let sortable = values
            .iter()
            .filter(|v| v.as_deref() == Some("sortable"))
            .count();
        assert_eq!(
            sortable, 1,
            "the other column is sortable, not silently claiming a \
             direction nothing chose: {values:?}"
        );
    }

    /// `named` panics when the key is missing; this returns `None` instead,
    /// for a cell that is not a sort header at all (a plain `text` column).
    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|c| find(c, key))
    }

    // ===== T034: the toolbar tier =====

    #[test]
    fn search_narrows_which_rows_view_builds_matching_any_column() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        // "busy" only appears in r1's status column, not its name column —
        // proves the search matches every column, not just the first.
        assert!(
            DataTable::update(
                &mut state,
                Intent::Search {
                    query: "busy".into()
                }
            )
            .is_empty()
        );
        assert_eq!(state.query, "busy");
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["r1"]);

        DataTable::update(
            &mut state,
            Intent::Search {
                query: String::new(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(table_of(&node)), vec!["r0", "r1", "r2"]);
    }

    #[test]
    fn search_and_sort_compose() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        // "ready" matches r0 and r2's status; sort by name descending then
        // narrows the visible set and orders it in one pass.
        DataTable::update(
            &mut state,
            Intent::Search {
                query: "ready".into(),
            },
        );
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(
            row_keys(table_of(&node)),
            vec!["r2", "r0"],
            "bravo before charlie"
        );
    }

    #[test]
    fn toggle_column_hides_the_header_and_every_cell_together() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        let shown = DataTable::view(&state, &props);
        assert!(find(table_of(&shown), "status").is_some());

        assert!(
            DataTable::update(
                &mut state,
                Intent::ToggleColumn {
                    id: "status".into()
                }
            )
            .is_empty()
        );
        assert!(state.hidden_columns.contains("status"));
        let hidden = DataTable::view(&state, &props);
        let table = table_of(&hidden);
        assert!(
            find(table, "status").is_none(),
            "the header cell must disappear"
        );
        for row in ["r0", "r1", "r2"] {
            assert!(
                find(named(table, row), "status").is_none(),
                "{row}'s status cell must disappear with the header"
            );
            assert!(
                find(named(table, row), "name").is_some(),
                "{row}'s name cell must survive; only status was hidden"
            );
        }

        DataTable::update(
            &mut state,
            Intent::ToggleColumn {
                id: "status".into(),
            },
        );
        assert!(!state.hidden_columns.contains("status"));
        let restored = DataTable::view(&state, &props);
        assert!(find(table_of(&restored), "status").is_some());
    }

    #[test]
    fn toggle_column_menu_toggles_state() {
        let mut state = DataTable::init(&sample_props());
        assert!(!state.column_menu_open);
        DataTable::update(&mut state, Intent::ToggleColumnMenu);
        assert!(state.column_menu_open);
        DataTable::update(&mut state, Intent::ToggleColumnMenu);
        assert!(!state.column_menu_open);
    }

    /// The batch bar replaces the toolbar as a pure function of the
    /// selection — no second flag to disagree with it.
    #[test]
    fn the_batch_bar_replaces_the_toolbar_exactly_when_selection_is_non_empty() {
        let props = sample_props();
        let mut state = DataTable::init(&props);
        let empty = DataTable::view(&state, &props);
        assert!(find(&empty, "toolbar").is_some());
        assert!(find(&empty, "batch").is_none());

        DataTable::update(&mut state, Intent::SelectRow { id: "r0".into() });
        let one = DataTable::view(&state, &props);
        assert!(find(&one, "batch").is_some());
        assert!(find(&one, "toolbar").is_none());
        assert_eq!(
            named(&one, "count").props.text.as_deref(),
            Some("1 item selected")
        );

        DataTable::update(&mut state, Intent::ClearSelection);
        assert!(state.selection.is_empty());
        let cleared = DataTable::view(&state, &props);
        assert!(find(&cleared, "toolbar").is_some());
        assert!(find(&cleared, "batch").is_none());
    }

    #[test]
    fn batch_action_clears_the_selection() {
        let props = Props {
            batch_actions: vec![ActionItem::new("delete", "Delete")],
            ..sample_props()
        };
        let mut state = DataTable::init(&props);
        DataTable::update(&mut state, Intent::SelectRow { id: "r0".into() });
        DataTable::update(&mut state, Intent::SelectRow { id: "r2".into() });
        assert_eq!(state.selection.len(), 2);
        assert!(
            DataTable::update(
                &mut state,
                Intent::BatchAction {
                    id: "delete".into()
                }
            )
            .is_empty()
        );
        assert!(state.selection.is_empty());
        let node = DataTable::view(&state, &props);
        assert!(
            find(&node, "toolbar").is_some(),
            "back to the toolbar at rest"
        );
    }

    #[test]
    fn toggle_row_menu_shows_one_at_a_time() {
        let mut state = DataTable::init(&sample_props());
        assert_eq!(state.row_menu_open, None);
        DataTable::update(&mut state, Intent::ToggleRowMenu { id: "r0".into() });
        assert_eq!(state.row_menu_open.as_deref(), Some("r0"));
        DataTable::update(&mut state, Intent::ToggleRowMenu { id: "r1".into() });
        assert_eq!(
            state.row_menu_open.as_deref(),
            Some("r1"),
            "opening a different row's menu replaces the open one"
        );
        DataTable::update(&mut state, Intent::ToggleRowMenu { id: "r1".into() });
        assert_eq!(
            state.row_menu_open, None,
            "a second press on the same row closes it"
        );
    }

    #[test]
    fn row_action_closes_the_row_menu() {
        let props = Props {
            row_actions: vec![ActionItem::new("edit", "Edit")],
            ..sample_props()
        };
        let mut state = DataTable::init(&props);
        DataTable::update(&mut state, Intent::ToggleRowMenu { id: "r0".into() });
        assert_eq!(state.row_menu_open.as_deref(), Some("r0"));
        assert!(
            DataTable::update(
                &mut state,
                Intent::RowAction {
                    id: "r0".into(),
                    action: "edit".into(),
                }
            )
            .is_empty()
        );
        assert_eq!(state.row_menu_open, None);
    }

    /// A row with [`Props::row_actions`] grows a trailing menu column; the
    /// open one's items are the author's own actions, keyed by id.
    #[test]
    fn a_row_with_row_actions_grows_a_menu_and_lists_the_authors_actions_when_open() {
        let props = Props {
            row_actions: vec![
                ActionItem::new("edit", "Edit"),
                ActionItem::new("delete", "Delete"),
            ],
            ..sample_props()
        };
        let mut state = DataTable::init(&props);
        let shut = DataTable::view(&state, &props);
        let r0 = named(table_of(&shut), "r0");
        assert!(
            find(r0, "trigger").is_some(),
            "row-menu trigger present even closed"
        );
        assert!(
            find(r0, "row-action-edit").is_none(),
            "menu not mounted while shut"
        );

        DataTable::update(&mut state, Intent::ToggleRowMenu { id: "r0".into() });
        let open = DataTable::view(&state, &props);
        let r0 = named(table_of(&open), "r0");
        assert_eq!(
            named(r0, "row-action-edit").semantics.label.as_deref(),
            Some("Edit")
        );
        assert_eq!(
            named(r0, "row-action-delete").semantics.label.as_deref(),
            Some("Delete")
        );
        // r1 never had its own menu opened; its trigger stays closed.
        let r1 = named(table_of(&open), "r1");
        assert!(find(r1, "row-action-edit").is_none());
    }

    #[test]
    fn loading_replaces_the_whole_view_with_a_skeleton() {
        let props = Props {
            loading: true,
            ..sample_props()
        };
        let state = DataTable::init(&props);
        let node = DataTable::view(&state, &props);
        assert!(find(&node, "toolbar").is_none(), "no toolbar while loading");
        assert!(find(&node, "batch").is_none());
        assert!(
            node.semantics.role.is_none(),
            "the skeleton root carries no Role"
        );
        fn no_focus(node: &ViewNode) -> bool {
            !node
                .interactions
                .contains(&gorgon_petra::tree::Interaction::Focus)
                && node.children.iter().all(|c| no_focus(c))
        }
        assert!(no_focus(&node), "nothing in a loading table is reachable");
    }

    /// A cold load (no cached `rows`) still shows a sensible number of
    /// placeholder rows rather than zero.
    #[test]
    fn loading_with_no_cached_rows_still_shows_placeholder_rows() {
        let props = Props {
            columns: vec![Column::new("name", "Name")],
            rows: Vec::new(),
            loading: true,
            ..Props::default()
        };
        let state = DataTable::init(&props);
        let node = DataTable::view(&state, &props);
        let mut bars = 0;
        fn count(node: &ViewNode, bars: &mut usize) {
            if node.semantics.skeleton {
                *bars += 1;
            }
            for child in &node.children {
                count(child, bars);
            }
        }
        count(&node, &mut bars);
        assert!(bars > 0, "a cold load still draws placeholder bars");
    }
}
