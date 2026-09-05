//! Inventory row 18, Menu.

use gorgon_petra::component::{button, menu, menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp};

/// The key `menu` anchors to: its trigger must be a sibling with this key.
const TRIGGER: &str = "trigger";
const MENU: &str = "menu";
const ITEMS: [(&str, &str); 2] = [("mn-0", "Rename"), ("mn-1", "Delete")];

/// Live state of the Menu page: whether the menu is open.
///
/// Carbon opens a Menu from a context (a right-click) or from a button;
/// the page gives it a button, because a menu with nothing to open it
/// is the row the operator saw.
#[derive(Default)]
pub struct Menu {
    open: bool,
}

impl Page for Menu {
    fn row(&self) -> &'static str {
        "Menu"
    }

    fn body(&self) -> ViewNode {
        let mut pair = vec![button(TRIGGER, "Actions")];
        if self.open {
            pair.push(menu(
                MENU,
                "Actions",
                ITEMS
                    .iter()
                    .map(|(key, label)| menu_item(*key, *label))
                    .collect(),
            ));
        }
        section(
            "menu",
            "Menu",
            vec![body(
                "mn",
                sp("spacing.md"),
                // The trigger and the menu share one child list with no
                // spacing, so the sibling anchor resolves and nothing
                // below the trigger moves when the menu opens.
                vec![column("mn-pair", None, pair)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if ITEMS.iter().any(|(key, _)| path_has(node, key)) {
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
