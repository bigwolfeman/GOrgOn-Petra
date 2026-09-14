//! Spec 009's Data table compound, hosted directly through the [`Compound`]
//! triple.
//!
//! Row 9, "Data table", is five pure, argument-driven `data_table_row_*`
//! atomics whose page owns selection and Kind text by hand. This page
//! instead owns `data_table::State` itself and drives it through
//! `DataTable::update`: sort, select, select-all, expand, and — T034 —
//! search, column visibility, batch actions, the row menu, are all live
//! intents, not flags the page flips directly.

use gorgon_petra::component::{data_table_weights_at, section};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::data_table::{
    ActionItem, Column, DataTable as DataTableCompound, Intent, Props, Row, State,
};

use super::Page;
use super::common::{body, path_has, path_is_column_chrome, sp, wrapped};

/// The column-visibility trigger's own key, and every checkbox inside its
/// menu is prefixed `col-`.
const COLUMNS: &str = "columns";
/// Prefix of a column-visibility checkbox: `col-<Column::id>`.
const COLUMN_CHECK_PREFIX: &str = "col-";
/// The batch bar's own cancel-selection control.
const CANCEL: &str = "cancel";
/// Prefix of a batch-bar action button: `batch-action-<ActionItem::id>`.
const BATCH_ACTION_PREFIX: &str = "batch-action-";
/// Prefix of a row-menu item: `row-action-<ActionItem::id>`.
const ROW_ACTION_PREFIX: &str = "row-action-";
/// Key every menu/row-menu trigger shares. Disambiguated by which owner's
/// subtree the press routed through — see [`Page::handle`]'s own ordering.
const TRIGGER: &str = "trigger";
/// The toolbar's search field.
const SEARCH: &str = "search";

/// Sample fiber-scheduling rows, distinct from row 9's kernel/petra/luau
/// set so the two pages are never mistaken for the same screenshot.
fn sample_rows() -> Vec<Row> {
    vec![
        Row::new("f0", ["scheduler", "running"]),
        Row::new("f1", ["layout", "idle"]).with_body("Retry policy: exponential backoff, 3 tries."),
        Row::new("f2", ["trace", "idle"]),
        Row::new("f3", ["snapshot", "running"]),
    ]
}

/// Live state of the Data table (compound) page.
pub struct DataTableCompoundPage {
    props: Props,
    state: State,
    /// A press on a divider opened a resize that has not ended.
    dragging: bool,
}

impl Default for DataTableCompoundPage {
    fn default() -> Self {
        let props = Props {
            columns: vec![Column::new("name", "Name"), Column::new("status", "Status")],
            rows: sample_rows(),
            batch_actions: vec![ActionItem::new("archive", "Archive")],
            row_actions: vec![
                ActionItem::new("rename", "Rename"),
                ActionItem::new("delete", "Delete"),
            ],
            loading: false,
        };
        let mut state = DataTableCompound::init(&props);
        // Seeded through real intents, not hand-built state, so the resting
        // picture is what driving the compound actually produces: sorted by
        // name, with one row's body open.
        //
        // **Nothing is selected at rest, and that is the point.** The batch
        // bar is a pure function of the selection and replaces the toolbar
        // while a selection exists, so seeding one row selected — which this
        // page did until 2026-09-11 — made the catalog's own picture of a
        // data table a blue bar reading "1 item selected", with the whole
        // T034 toolbar tier invisible until a reader thought to deselect.
        // That is not what a data table looks like. Row 9 already
        // photographs a mixed select-all at rest, so nothing is lost, and
        // `the_batch_bar_replaces_the_toolbar_while_a_row_is_selected`
        // photographs the other state by driving a real press.
        DataTableCompound::update(
            &mut state,
            Intent::SortBy {
                column: "name".into(),
            },
        );
        DataTableCompound::update(&mut state, Intent::ToggleExpand { id: "f1".into() });
        Self {
            props,
            state,
            dragging: false,
        }
    }
}

impl Page for DataTableCompoundPage {
    fn row(&self) -> &'static str {
        "Data table (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "table-compound",
            "Data table (compound)",
            vec![body(
                "dtc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "dtc-note",
                        "The same triple as spec 009 T006, driven through \
                         `Compound::update`: sort, select, select-all, \
                         expand, search, column visibility, the batch bar \
                         and the row menu are all live intents rather than \
                         flags the page flips directly.",
                    ),
                    DataTableCompound::view(&self.state, &self.props),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if path_is_column_chrome(node) {
            return false;
        }
        // The column-visibility trigger and its checkboxes: checked ahead
        // of the row block below because both a row's own row-menu trigger
        // and this one share the literal key `trigger`, and the column
        // menu is never nested inside a row's own subtree.
        if path_has(node, COLUMNS) {
            if path_has(node, TRIGGER) {
                DataTableCompound::update(&mut self.state, Intent::ToggleColumnMenu);
                return true;
            }
            if let Some(id) = segment_suffix(node, COLUMN_CHECK_PREFIX) {
                DataTableCompound::update(&mut self.state, Intent::ToggleColumn { id });
                return true;
            }
        }

        if path_has(node, CANCEL) {
            DataTableCompound::update(&mut self.state, Intent::ClearSelection);
            return true;
        }
        if let Some(id) = segment_suffix(node, BATCH_ACTION_PREFIX) {
            DataTableCompound::update(&mut self.state, Intent::BatchAction { id });
            return true;
        }

        if path_has(node, SEARCH) {
            match event {
                InputEvent::Text(typed) => {
                    let mut query = self.state.query.clone();
                    query.push_str(typed);
                    DataTableCompound::update(&mut self.state, Intent::Search { query });
                }
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                } => {
                    let mut query = self.state.query.clone();
                    query.pop();
                    DataTableCompound::update(&mut self.state, Intent::Search { query });
                }
                _ => {}
            }
            return true;
        }

        // Select-all is matched first among the table's own controls: its
        // own key is unambiguous and does not appear inside any row or
        // header cell's own path.
        if path_has(node, "select-all") {
            let ids: Vec<String> = self.props.rows.iter().map(|r| r.id.clone()).collect();
            DataTableCompound::update(&mut self.state, Intent::SelectAll { ids });
            return true;
        }
        // A sort header's own root key is the column id it sorts, so the
        // column is found the same way a row is found below: by which
        // column's id is a segment of the route.
        if path_has(node, "sort")
            && let Some(col) = self.props.columns.iter().find(|c| path_has(node, &c.id))
        {
            DataTableCompound::update(
                &mut self.state,
                Intent::SortBy {
                    column: col.id.clone(),
                },
            );
            return true;
        }

        // Row-scoped controls. Checked ahead of the plain row-click
        // fallback: a row-menu trigger and a row-action item both sit
        // inside their owning row's own subtree, so `path_has(node,
        // &row.id)` matches them too, and without this ordering they would
        // be treated as a select/expand press on the row instead.
        if let Some(row_id) = self
            .props
            .rows
            .iter()
            .find(|r| path_has(node, &r.id))
            .map(|r| r.id.clone())
        {
            if let Some(action) = segment_suffix(node, ROW_ACTION_PREFIX) {
                DataTableCompound::update(
                    &mut self.state,
                    Intent::RowAction { id: row_id, action },
                );
                return true;
            }
            if path_has(node, TRIGGER) {
                DataTableCompound::update(&mut self.state, Intent::ToggleRowMenu { id: row_id });
                return true;
            }
            // A row's own select checkbox and expand chevron are painted,
            // not independently interactive — `data_table_row`/`data_table_
            // row_expandable` declare `Interaction::Click` on the row as a
            // whole (`ROW_INTENTS`) and nowhere inside it, so hit-testing a
            // press anywhere in a row resolves to the row's own id, never a
            // child's (other than the two row-scoped controls matched
            // above, which are the T034 addition to that same row).
            let expandable = self
                .props
                .rows
                .iter()
                .any(|r| r.id == row_id && r.body.is_some());
            if expandable {
                DataTableCompound::update(&mut self.state, Intent::ToggleExpand { id: row_id });
            } else {
                DataTableCompound::update(&mut self.state, Intent::SelectRow { id: row_id });
            }
            return true;
        }
        false
    }

    fn gesture(&mut self, event: &InputEvent, node: &str, frame: &PetrifiedFrame) -> bool {
        let n = self
            .props
            .columns
            .iter()
            .filter(|c| !self.state.hidden_columns.contains(&c.id))
            .count()
            .max(1);
        let weights = if self.state.weights.len() == n {
            self.state.weights.clone()
        } else {
            vec![1.0; n]
        };
        match event {
            InputEvent::GestureEnded { .. } => {
                let acted = self.dragging;
                self.dragging = false;
                acted
            }
            InputEvent::PointerPressed { pos, .. } => {
                if let Some(next) = data_table_weights_at(frame, node, *pos, &weights) {
                    DataTableCompound::update(&mut self.state, Intent::Resize { weights: next });
                    self.dragging = true;
                    return true;
                }
                false
            }
            InputEvent::PointerMoved { pos } | InputEvent::PointerReleased { pos, .. } => {
                if !self.dragging {
                    return false;
                }
                if let Some(next) = data_table_weights_at(frame, node, *pos, &weights) {
                    DataTableCompound::update(&mut self.state, Intent::Resize { weights: next });
                }
                true
            }
            _ => false,
        }
    }

    fn dismissed(&mut self, ids: &[String]) {
        // The ids named here are exactly the currently-open, `DismissOutside`
        // surfaces a press landed outside of (`Page::dismissed`'s own doc),
        // so a toggle intent closes rather than reopens each one.
        if dismissed_under(ids, COLUMNS, "menu") {
            DataTableCompound::update(&mut self.state, Intent::ToggleColumnMenu);
        }
        for row in &self.props.rows {
            if dismissed_under(ids, &row.id, "menu") {
                DataTableCompound::update(
                    &mut self.state,
                    Intent::ToggleRowMenu { id: row.id.clone() },
                );
            }
        }
    }
}

/// The suffix after `prefix` in whichever path segment of `node` carries
/// it — `None` if no segment starts with `prefix`.
fn segment_suffix(node: &str, prefix: &str) -> Option<String> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix).map(str::to_owned))
}

/// Whether any dismissed id names the surface keyed `key` inside the
/// control keyed `owner` — the same two-part match `page/date_picker.rs`'s
/// own `dismissed_under` uses, for the same reason: this page mounts more
/// than one overlay keyed `menu` (the column-visibility menu and every
/// open row's own menu), so `key` alone is ambiguous and `owner` picks
/// which one.
fn dismissed_under(ids: &[String], owner: &str, key: &str) -> bool {
    ids.iter()
        .any(|id| path_has(id, owner) && id.rsplit('/').next() == Some(key))
}
