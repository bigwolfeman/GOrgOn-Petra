//! Inventory row 7, Contained list.

use gorgon_petra::component::{contained_list, list_row, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const ROWS: [(&str, &str); 2] = [("cl-0", "Trace"), ("cl-1", "Store")];

/// Live state of the Contained list page: which row, if any, is selected.
///
/// `list_row` is a selectable `Role::ListItem` that declares `Click`, so a
/// page that mounts it and holds no selection has a row that lights on
/// hover and does nothing on press — the pattern this catalog is fixing.
#[derive(Default)]
pub struct ContainedList {
    selected: Option<usize>,
}

impl Page for ContainedList {
    fn row(&self) -> &'static str {
        "Contained list"
    }

    fn body(&self) -> ViewNode {
        section(
            "on-page",
            "On-page header",
            vec![body(
                "contained",
                sp("spacing.md"),
                vec![contained_list(
                    "cl",
                    "Recent",
                    ROWS.iter()
                        .enumerate()
                        .map(|(i, (key, label))| list_row(*key, *label, self.selected == Some(i)))
                        .collect(),
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let Some(hit) = ROWS.iter().position(|(key, _)| path_has(node, key)) else {
            return false;
        };
        // A second press on the selected row clears the selection, so
        // "nothing selected" is reachable again.
        self.selected = (self.selected != Some(hit)).then_some(hit);
        true
    }
}
