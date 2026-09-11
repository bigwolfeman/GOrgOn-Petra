//! Catalog row 44, Button group.

use gorgon_petra::component::{
    button, button_group, ghost_button, primary_button, section, tertiary_button,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Button group page. The container holds no state; the children are
/// the same buttons row 4 already draws, in a row with a visible gap.
pub struct ButtonGroup;

impl Default for ButtonGroup {
    fn default() -> Self {
        Self
    }
}

impl Page for ButtonGroup {
    fn row(&self) -> &'static str {
        "Button group"
    }

    fn body(&self) -> ViewNode {
        section(
            "group",
            "Spaced group",
            vec![body(
                "btns",
                sp("spacing.md"),
                vec![button_group(
                    "bg",
                    vec![
                        primary_button("bg-primary", "Save"),
                        button("bg-default", "Discard"),
                        tertiary_button("bg-tertiary", "More"),
                        ghost_button("bg-ghost", "Cancel"),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
