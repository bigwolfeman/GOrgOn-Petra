//! Inventory row 29, Select.

use gorgon_petra::component::{dropdown_option, section, select, select_open};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{dismisses, filled_body, path_has, sp};

const THEME: &str = "theme";
/// The option list's key inside `select_open`.
const MENU: &str = "menu";
const OPTIONS: [(&str, &str); 3] = [
    ("opt-dark", "Dark"),
    ("opt-light", "Light"),
    ("opt-system", "System"),
];

/// Live state of the Select page: whether the list is open, and which
/// option is current.
#[derive(Default)]
pub struct Select {
    open: bool,
    selected: usize,
}

impl Page for Select {
    fn row(&self) -> &'static str {
        "Select"
    }

    fn body(&self) -> ViewNode {
        let value = OPTIONS[self.selected].1;
        // `filled_body`, not `body`: a Carbon select is 100% of its
        // container's width (slice-b §29), so the column stretches it.
        let field = if self.open {
            select_open(
                THEME,
                "Theme",
                value,
                OPTIONS
                    .iter()
                    .enumerate()
                    .map(|(i, (key, label))| dropdown_option(*key, *label, i == self.selected))
                    .collect(),
            )
        } else {
            select(THEME, "Theme", value)
        };
        section(
            "select",
            "Select",
            vec![filled_body("sel", sp("spacing.md"), vec![field])],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if let Some(hit) = OPTIONS.iter().position(|(key, _)| path_has(node, key)) {
            self.selected = hit;
            self.open = false;
        } else if path_has(node, THEME) {
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
