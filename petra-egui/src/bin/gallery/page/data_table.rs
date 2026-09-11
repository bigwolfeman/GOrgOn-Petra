//! Inventory row 9, Data table.

use gorgon_petra::component::{
    SortDirection, data_table_row_expandable, data_table_row_lg, data_table_row_md,
    data_table_row_sm, data_table_row_xl, data_table_row_xs, data_table_sort_header,
    data_table_zebra, disabled, field_sm, section, text, valued,
};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp, wrapped};

/// The four rows the catalog tests drive (`dt-0`..`dt-3`). Same names as
/// `09-data-table.png` (kernel/runtime, petra/layout) plus two more so the
/// row rules read as a run. The third field is the Kind column's opening
/// value, which the page then owns and the operator can type over.
///
/// Sizes: dt-0 is lg, the height the catalog's cursor and drag tests
/// photograph. xs sits on the disabled row, then sm, md, xl, so all five
/// constructors are on the page without shrinking the driven row.
const ROWS: [(&str, &str, &str); 4] = [
    ("dt-0", "kernel", "runtime"),
    ("dt-1", "petra", "layout"),
    ("dt-2", "luau", "plugin host"),
    ("dt-3", "helix", "editor"),
];

/// The editable cell in each of [`ROWS`], one key per row.
///
/// A key per row rather than one key reused: two siblings sharing a key is
/// a tree-acceptance violation, and a route names the field it landed on.
const KIND: [&str; ROWS.len()] = ["dt-kind-0", "dt-kind-1", "dt-kind-2", "dt-kind-3"];

/// Index of the xs row in [`ROWS`]. Wrapped in [`disabled`] so the operator
/// can see an unavailable row next to the live ones. Catalog tests still
/// select it through select-all; they never click it. Not dt-0: that row
/// is lg because a cursor pass and a drag-to-select photograph it.
const DISABLED_ROW: usize = 3;

/// Fifth size (xl). Not in [`ROWS`]: those four keep the Kind wells the
/// catalog types into.
const XL: (&str, &str, &str) = ("dt-4", "gorgond", "daemon");

/// Expandable row, lg. A press toggles the body; select-all still selects it.
const EXP: (&str, &str) = ("dt-exp", "supervisor");
const EXP_BODY: &str = "Retry policy lives here. Mounted only while open.";

/// [`ROWS`] plus [`XL`] plus [`EXP`].
const N_SELECTED: usize = ROWS.len() + 2;
const XL_INDEX: usize = ROWS.len();
const EXP_INDEX: usize = ROWS.len() + 1;

/// Live state of the Data table page.
///
/// # The editable column
///
/// Round 3, the operator: *"this makes me notice that neither it or the
/// other table like structure is editable."*
///
/// **Carbon v11 core has no inline-edit anatomy to copy.** slice-b
/// enumerates every Data table variant and there is no editable one; what
/// it does say, at slice-b:69, is that *"individual form controls placed
/// inside cells — e.g. a text input column — carry their own validation
/// states"*. So Carbon's sanctioned way to make a table cell editable is to
/// put a form control in the cell, and that is exactly what the Kind column
/// is: a `field_sm` well per row, filled from this page's state.
///
/// A click seats focus on the well the way it does on any other field
/// (`field.rs`'s `EDITABLE_TEXT_INTENTS` includes `Click` for precisely
/// this reason), and the keystrokes after it land here.
pub struct DataTable {
    /// One flag per body row: [`ROWS`], then [`XL`], then [`EXP`].
    selected: [bool; N_SELECTED],
    /// What each of [`ROWS`]' Kind wells holds.
    kinds: [String; ROWS.len()],
    /// Sort direction on the Name header. `true` is ascending.
    name_ascending: bool,
    /// Whether [`EXP`]'s body is mounted.
    expanded: bool,
}

impl Default for DataTable {
    fn default() -> Self {
        // The reference shot opens with its second row selected, so
        // select-all starts mixed: that is the tri-state the header
        // derives, on screen at rest.
        Self {
            selected: {
                let mut selected = [false; N_SELECTED];
                selected[1] = true;
                selected
            },
            kinds: ROWS.map(|(_, _, kind)| kind.to_owned()),
            name_ascending: true,
            expanded: true,
        }
    }
}

impl DataTable {
    /// The Kind value the route `node` names, if it names one.
    fn kind_at(&mut self, node: &str) -> Option<&mut String> {
        let i = KIND.iter().position(|key| path_has(node, key))?;
        Some(&mut self.kinds[i])
    }

    fn cells(&self, i: usize, name: &str) -> Vec<ViewNode> {
        vec![
            text("name", name),
            valued(field_sm(KIND[i], "Kind"), self.kinds[i].clone()),
        ]
    }
}

impl Page for DataTable {
    fn row(&self) -> &'static str {
        "Data table"
    }

    fn body(&self) -> ViewNode {
        let mut rows: Vec<ViewNode> = ROWS
            .iter()
            .enumerate()
            .map(|(i, (key, name, _))| {
                let cells = self.cells(i, name);
                let row = match i {
                    0 => data_table_row_lg(*key, cells, self.selected[i]),
                    1 => data_table_row_sm(*key, cells, self.selected[i]),
                    2 => data_table_row_md(*key, cells, self.selected[i]),
                    3 => data_table_row_xs(*key, cells, self.selected[i]),
                    _ => unreachable!("ROWS is four entries"),
                };
                if i == DISABLED_ROW {
                    disabled(row)
                } else {
                    row
                }
            })
            .collect();
        rows.push(data_table_row_xl(
            XL.0,
            vec![text("name", XL.1), text("kind", XL.2)],
            self.selected[XL_INDEX],
        ));
        rows.push(data_table_row_expandable(
            EXP.0,
            vec![text("name", EXP.1), text("kind", "retry")],
            self.selected[EXP_INDEX],
            self.expanded,
            EXP_BODY,
        ));
        section(
            "table",
            "Data table",
            vec![body(
                "dt",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "note",
                        "Five row sizes (xs 24 through xl 64), zebra on odd \
                         rows, a sortable Name header, a disabled lg row, an \
                         expandable row, and mixed select-all at rest.",
                    ),
                    data_table_zebra(
                        "dt",
                        vec![
                            data_table_sort_header(
                                "h0",
                                "Name",
                                if self.name_ascending {
                                    SortDirection::Ascending
                                } else {
                                    SortDirection::Descending
                                },
                            ),
                            text("h1", "Kind"),
                        ],
                        rows,
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        // The editable cell is checked **first**, and it swallows whatever
        // it does not use. Every route into a cell also names the row it is
        // in, so without this a press meant to put the caret in a well
        // would toggle the row's selection underneath it.
        if KIND.iter().any(|key| path_has(node, key)) {
            match event {
                InputEvent::Text(typed) => {
                    let typed = typed.clone();
                    if let Some(value) = self.kind_at(node) {
                        value.push_str(&typed);
                    }
                }
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                } => {
                    if let Some(value) = self.kind_at(node) {
                        value.pop();
                    }
                }
                _ => {}
            }
            return true;
        }
        if path_has(node, "sort") {
            self.name_ascending = !self.name_ascending;
            return true;
        }
        if path_has(node, "select-all") {
            let every = self.selected.iter().all(|s| *s);
            self.selected = [!every; N_SELECTED];
            return true;
        }
        if path_has(node, XL.0) {
            self.selected[XL_INDEX] = !self.selected[XL_INDEX];
            return true;
        }
        if path_has(node, EXP.0) {
            self.expanded = !self.expanded;
            return true;
        }
        if let Some(i) = ROWS.iter().position(|(key, _, _)| path_has(node, key)) {
            if i == DISABLED_ROW {
                return true;
            }
            self.selected[i] = !self.selected[i];
            return true;
        }
        false
    }
}
