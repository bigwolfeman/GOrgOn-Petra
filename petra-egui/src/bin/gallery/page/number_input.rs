//! Inventory row 22, Number input.

use gorgon_petra::component::{number_input, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Number input page. It holds no live state.
pub struct NumberInput;

impl Page for NumberInput {
    fn row(&self) -> &'static str {
        "Number input"
    }

    fn body(&self) -> ViewNode {
        section(
            "number",
            "Number input",
            vec![body(
                "num",
                sp("spacing.md"),
                vec![number_input("n-md", "Count", "12")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
