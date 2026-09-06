//! Inventory row 11, Dropdown.

use gorgon_petra::component::{
    dropdown, dropdown_lg, dropdown_open, dropdown_option, dropdown_sm, dropdown_xs, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

const DD: &str = "dd";
/// The option list's key inside `dropdown_open`.
const MENU: &str = "menu";
const OPTIONS: [(&str, &str); 3] = [
    ("opt-dark", "Dark"),
    ("opt-light", "Light"),
    ("opt-system", "System"),
];

/// Live state of the Dropdown page: whether the list is open, and which
/// option is current.
#[derive(Default)]
pub struct Dropdown {
    open: bool,
    selected: usize,
}

impl Page for Dropdown {
    fn row(&self) -> &'static str {
        "Dropdown"
    }

    fn body(&self) -> ViewNode {
        let value = OPTIONS[self.selected].1;
        let field = if self.open {
            dropdown_open(
                DD,
                "Theme",
                value,
                OPTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, label))| dropdown_option(*key, *label, i == self.selected))
                    .collect(),
            )
        } else {
            dropdown(DD, "Theme", value)
        };
        section(
            "drop",
            "Dropdown",
            vec![
                body("dd-body", sp("spacing.md"), vec![field]),
                // All four of Carbon's list-box sizes, stacked so the
                // xs-to-lg step is one picture to look at rather than a
                // fact only a unit test can see (`button.rs`'s "sizes"
                // row does the same for its own six).
                body(
                    "dd-sizes",
                    sp("spacing.md"),
                    vec![
                        dropdown_xs("dd-xs", "Xs 24", "Dark"),
                        dropdown_sm("dd-sm", "Sm 32", "Dark"),
                        dropdown("dd-md", "Md 40", "Dark"),
                        dropdown_lg("dd-lg", "Lg 48", "Dark"),
                    ],
                ),
            ],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // An option is inside the dropdown's own subtree, so it is matched
        // before the trigger.
        if let Some(hit) = OPTIONS.iter().position(|(key, _)| path_has(node, key)) {
            self.selected = hit;
            self.open = false;
        } else if path_has(node, DD) {
            self.open = !self.open;
        } else {
            return false;
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            self.open = false;
        }
    }
}
