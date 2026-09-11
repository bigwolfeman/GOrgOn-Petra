//! Data table as a [`Compound`]: sort, selection, and expansion live here.
//!
//! The constructors in `gorgon_petra::component` stay put. They take every
//! piece of state as an argument (`data_table_row`, `data_table_row_expandable`,
//! `data_table_sort_header`) and derive the tri-state select-all from the
//! rows the caller passed. This module is the caller that holds that state.
//!
//! Binding: spec 009 T006, `view-fiber.md` §4.1. `update` does not rewrite
//! the row list; sort is a presentation of [`Props`] applied in [`DataTable::view`].

use std::collections::BTreeSet;

use gorgon_petra::component::{
    SortDirection, data_table, data_table_row, data_table_row_expandable, data_table_sort_header,
    text,
};
use gorgon_petra::tree::ViewNode;
use serde::{Deserialize, Serialize};

use crate::Compound;

/// Namespace over the data table's triple. Never constructed as a value.
pub struct DataTable;

/// Root key of the node [`DataTable::view`] returns.
const TABLE_KEY: &str = "table";

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

/// Rows and columns. The author owns these; the compound never writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    /// Column order is the header order and the cell index order.
    pub columns: Vec<Column>,
    /// Body rows in author order. [`view`](DataTable::view) may present them
    /// in a different order when a sort is active; `rows` itself is not
    /// rewritten.
    pub rows: Vec<Row>,
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

/// Sort, selection, and expansion. Round-trips through reload as a whole.
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
        }
        Vec::new()
    }

    fn view(state: &Self::State, props: &Self::Props) -> ViewNode {
        let header = header_cells(state, props);
        let rows = sorted_indices(state, props)
            .into_iter()
            .map(|i| body_row(state, props, &props.rows[i]))
            .collect();
        data_table(TABLE_KEY, header, rows)
    }
}

fn cell_at(row: &Row, idx: usize) -> &str {
    row.cells.get(idx).map(String::as_str).unwrap_or("")
}

fn sorted_indices(state: &State, props: &Props) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..props.rows.len()).collect();
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

/// Every column is a sort header so a click can become [`Intent::SortBy`].
/// A column that is not the active sort shows [`SortDirection::Sortable`],
/// never a forced `Ascending` — [`data_table_sort_header`] has a real
/// unsorted spelling now, so this stops claiming a direction nothing chose.
fn header_cells(state: &State, props: &Props) -> Vec<ViewNode> {
    props
        .columns
        .iter()
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

fn body_row(state: &State, props: &Props, row: &Row) -> ViewNode {
    let selected = state.selection.contains(&row.id);
    let cells: Vec<ViewNode> = props
        .columns
        .iter()
        .enumerate()
        .map(|(i, col)| text(col.id.as_str(), cell_at(row, i)))
        .collect();
    match row.body.as_deref() {
        Some(body) => {
            let expanded = state.expansion.contains(&row.id);
            data_table_row_expandable(row.id.as_str(), cells, selected, expanded, body)
        }
        None => data_table_row(row.id.as_str(), cells, selected),
    }
}

#[cfg(test)]
mod tests {
    use gorgon_petra::tree::{Role, ViewNode};

    use crate::Compound;

    use super::{Column, DataTable, Intent, Props, Row, Sort, TABLE_KEY};

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
        }
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
        assert_eq!(node.key.as_str(), TABLE_KEY);
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert_eq!(row_keys(&node), vec!["r0", "r1", "r2"]);
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
        assert_eq!(row_keys(&node), vec!["r1", "r2", "r0"]);
        assert_eq!(props.rows[0].id, "r0");
        DataTable::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(&node), vec!["r0", "r2", "r1"]);
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
        assert_eq!(row_keys(&node), vec!["r0", "r1", "r2"]);
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
        };
        let mut state = DataTable::init(&props);
        DataTable::update(&mut state, Intent::SortBy { column: "k".into() });
        let node = DataTable::view(&state, &props);
        assert_eq!(row_keys(&node), vec!["a", "b", "c"]);
    }

    #[test]
    fn empty_rows_still_make_a_table() {
        let props = Props {
            columns: vec![Column::new("a", "A")],
            rows: vec![],
        };
        let node = DataTable::view(&DataTable::init(&props), &props);
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert!(row_keys(&node).is_empty());
        let all = named(&node, "select-all");
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
}
