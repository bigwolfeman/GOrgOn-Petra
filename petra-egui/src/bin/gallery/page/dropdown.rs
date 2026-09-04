//! Inventory row 11, Dropdown.

use gorgon_petra::component::{dropdown, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Dropdown page. It holds no live state.
pub struct Dropdown;

impl Page for Dropdown {
    fn row(&self) -> &'static str {
        "Dropdown"
    }

    fn body(&self) -> ViewNode {
        section(
            "drop",
            "Closed dropdown",
            vec![body(
                "dd",
                sp("spacing.md"),
                vec![dropdown("dd", "Theme", "Dark")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
