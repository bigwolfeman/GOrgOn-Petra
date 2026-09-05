//! Inventory row 9, Data table.

use gorgon_petra::component::{data_table, data_table_row, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

/// The rows, as the Carbon reference shot has them (`09-data-table.png`:
/// kernel/runtime and petra/layout) plus two more so the row rules read
/// as a run.
const ROWS: [(&str, &str, &str); 4] = [
    ("dt-0", "kernel", "runtime"),
    ("dt-1", "petra", "layout"),
    ("dt-2", "luau", "plugin host"),
    ("dt-3", "helix", "editor"),
];

/// Live state of the Data table page: which rows are selected. A press on
/// a row toggles it; a press on the header's select-all checkbox selects
/// every row, or clears every row when all are already selected.
pub struct DataTable {
    selected: [bool; ROWS.len()],
}

impl Default for DataTable {
    fn default() -> Self {
        // The reference shot opens with its second row selected.
        Self {
            selected: [false, true, false, false],
        }
    }
}

impl Page for DataTable {
    fn row(&self) -> &'static str {
        "Data table"
    }

    fn body(&self) -> ViewNode {
        let rows = ROWS
            .iter()
            .zip(self.selected)
            .map(|((key, name, kind), selected)| {
                data_table_row(*key, vec![text("c0", *name), text("c1", *kind)], selected)
            })
            .collect();
        section(
            "table",
            "Data table",
            vec![body(
                "dt",
                sp("spacing.md"),
                vec![data_table(
                    "dt",
                    vec![text("h0", "Name"), text("h1", "Kind")],
                    rows,
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, "select-all") {
            let every = self.selected.iter().all(|s| *s);
            self.selected = [!every; ROWS.len()];
            return true;
        }
        if let Some(i) = ROWS.iter().position(|(key, _, _)| path_has(node, key)) {
            self.selected[i] = !self.selected[i];
            return true;
        }
        false
    }
}
