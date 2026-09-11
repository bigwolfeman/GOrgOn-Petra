//! Catalog row 52, Toggle button.

use gorgon_petra::component::{
    section, toggle_button, toggle_button_group, toggle_button_icon,
};
use gorgon_petra::component::IconMark;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const LIST: &str = "tb-list";
const GRID: &str = "tb-grid";
const BOTH: &str = "tb-both";

/// Live state of the Toggle button page: which of the three is pressed.
/// Single-select is a caller convention; the group constructor does not
/// clear siblings.
pub struct ToggleButton {
    pressed: u8,
}

impl Default for ToggleButton {
    fn default() -> Self {
        Self { pressed: 0 }
    }
}

impl Page for ToggleButton {
    fn row(&self) -> &'static str {
        "Toggle button"
    }

    fn body(&self) -> ViewNode {
        section(
            "toggles",
            "Pressed group",
            vec![body(
                "tb-body",
                sp("spacing.md"),
                vec![toggle_button_group(
                    "tb-group",
                    vec![
                        toggle_button(LIST, "List", self.pressed == 0),
                        toggle_button(GRID, "Grid", self.pressed == 1),
                        toggle_button_icon(BOTH, "Both", self.pressed == 2, IconMark::Menu),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, LIST) {
            self.pressed = 0;
        } else if path_has(node, GRID) {
            self.pressed = 1;
        } else if path_has(node, BOTH) {
            self.pressed = 2;
        } else {
            return false;
        }
        true
    }
}
