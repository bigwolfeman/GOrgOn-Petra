//! Inventory row 18, Menu.

use gorgon_petra::component::{menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Menu page. It holds no live state.
pub struct Menu;

impl Page for Menu {
    fn row(&self) -> &'static str {
        "Menu"
    }

    fn body(&self) -> ViewNode {
        section(
            "menu",
            "Menu items",
            vec![body(
                "mn",
                sp("spacing.md"),
                vec![menu_item("mn-0", "Rename"), menu_item("mn-1", "Delete")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
