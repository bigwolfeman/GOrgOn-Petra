//! Inventory row 19, Menu buttons.

use gorgon_petra::component::{menu_button, menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

/// `menu_button`'s own keys for its trigger and its menu.
const TRIGGER: &str = "trigger";
const MENU: &str = "menu";
const DUPLICATE: &str = "mb-0";

/// Live state of the Menu buttons page: whether the menu is open.
#[derive(Default)]
pub struct MenuButtons {
    open: bool,
}

impl Page for MenuButtons {
    fn row(&self) -> &'static str {
        "Menu buttons"
    }

    fn body(&self) -> ViewNode {
        section(
            "mb",
            "Menu button",
            vec![body(
                "mb-body",
                sp("spacing.md"),
                vec![menu_button(
                    "mb",
                    "More",
                    self.open,
                    vec![menu_item(DUPLICATE, "Duplicate")],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, DUPLICATE) {
            self.open = false;
        } else if path_has(node, TRIGGER) {
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
