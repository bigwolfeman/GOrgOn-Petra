//! Inventory row 19, Menu buttons.

use gorgon_petra::component::{menu_button, menu_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Menu buttons page. It holds no live state.
pub struct MenuButtons;

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
                    false,
                    vec![menu_item("mb-0", "Duplicate")],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
