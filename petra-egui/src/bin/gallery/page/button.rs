//! Inventory row 4, Button.

use gorgon_petra::component::{
    button, button_lg, button_sm, danger_button, ghost_button, primary_button, section,
    tertiary_button,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// The Button page. It holds no live state.
pub struct Button;

impl Page for Button {
    fn row(&self) -> &'static str {
        "Button"
    }

    fn body(&self) -> ViewNode {
        section(
            "variants",
            "Variants and sizes",
            vec![body(
                "buttons",
                sp("spacing.md"),
                vec![
                    row(
                        "kinds",
                        sp("spacing.md"),
                        vec![
                            primary_button("btn-primary", "Primary"),
                            button("btn-default", "Default"),
                            tertiary_button("btn-tertiary", "Tertiary"),
                            ghost_button("btn-ghost", "Ghost"),
                            danger_button("btn-danger", "Danger"),
                        ],
                    ),
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            button_sm("btn-sm", "Small 32"),
                            button("btn-md", "Medium 40"),
                            button_lg("btn-lg", "Large 48"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
