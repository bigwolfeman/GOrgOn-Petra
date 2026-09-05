//! Inventory row 9, Data table.

use gorgon_petra::component::{data_table, data_table_row, field_sm, section, text, valued};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

/// The rows, as the Carbon reference shot has them (`09-data-table.png`:
/// kernel/runtime and petra/layout) plus two more so the row rules read
/// as a run. The third field is the Kind column's opening value, which the
/// page then owns and the operator can type over.
const ROWS: [(&str, &str, &str); 4] = [
    ("dt-0", "kernel", "runtime"),
    ("dt-1", "petra", "layout"),
    ("dt-2", "luau", "plugin host"),
    ("dt-3", "helix", "editor"),
];

/// The editable cell in each row, one key per row.
///
/// A key per row rather than one key reused: two siblings sharing a key is
/// a tree-acceptance violation, and a route names the field it landed on.
const KIND: [&str; ROWS.len()] = ["dt-kind-0", "dt-kind-1", "dt-kind-2", "dt-kind-3"];

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
    selected: [bool; ROWS.len()],
    /// What each row's Kind well holds.
    kinds: [String; ROWS.len()],
}

impl Default for DataTable {
    fn default() -> Self {
        // The reference shot opens with its second row selected.
        Self {
            selected: [false, true, false, false],
            kinds: ROWS.map(|(_, _, kind)| kind.to_owned()),
        }
    }
}

impl DataTable {
    /// The Kind value the route `node` names, if it names one.
    fn kind_at(&mut self, node: &str) -> Option<&mut String> {
        let i = KIND.iter().position(|key| path_has(node, key))?;
        Some(&mut self.kinds[i])
    }
}

impl Page for DataTable {
    fn row(&self) -> &'static str {
        "Data table"
    }

    fn body(&self) -> ViewNode {
        let rows = ROWS
            .iter()
            .enumerate()
            .map(|(i, (key, name, _))| {
                data_table_row(
                    *key,
                    vec![
                        text("name", *name),
                        valued(field_sm(KIND[i], "Kind"), self.kinds[i].clone()),
                    ],
                    self.selected[i],
                )
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
