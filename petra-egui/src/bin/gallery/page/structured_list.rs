//! Inventory row 31, Structured list.

use gorgon_petra::component::{section, structured_list, structured_list_row, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

/// The rows, as the Carbon reference shot has them (`31-structured-list.png`).
const ROWS: [(&str, &str, &str); 2] = [("sl-0", "kernel", "runtime"), ("sl-1", "petra", "layout")];

/// Live state of the Structured list page: which one row is selected.
/// Carbon's selectable structured list is single-select (a hidden radio
/// per row, slice-e anatomy 4), so a press on a row moves the selection
/// to it.
pub struct StructuredList {
    selected: usize,
}

impl Default for StructuredList {
    fn default() -> Self {
        Self { selected: 1 }
    }
}

impl Page for StructuredList {
    fn row(&self) -> &'static str {
        "Structured list"
    }

    fn body(&self) -> ViewNode {
        let rows = ROWS
            .iter()
            .enumerate()
            .map(|(i, (key, name, role))| {
                structured_list_row(
                    *key,
                    vec![text("c0", *name), text("c1", *role)],
                    i == self.selected,
                )
            })
            .collect();
        section(
            "table",
            "Structured list",
            vec![body(
                "sl",
                sp("spacing.md"),
                vec![structured_list(
                    "sl",
                    vec![text("h0", "Name"), text("h1", "Role")],
                    rows,
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let Some(i) = ROWS.iter().position(|(key, _, _)| path_has(node, key)) else {
            return false;
        };
        self.selected = i;
        true
    }
}
