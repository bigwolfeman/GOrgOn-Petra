//! Inventory row 34, Text input.

use gorgon_petra::component::{field, field_invalid, field_lg, field_readonly, field_sm, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Text input page. It holds no live state.
pub struct TextInput;

impl Page for TextInput {
    fn row(&self) -> &'static str {
        "Text input"
    }

    fn body(&self) -> ViewNode {
        section(
            "fields",
            "Default sizes and states",
            vec![body(
                "inputs",
                sp("spacing.md"),
                vec![
                    field("field-md", "Fiber name"),
                    field_sm("field-sm", "Small"),
                    field_lg("field-lg", "Large"),
                    field_invalid("field-bad", "Port", "must be a number"),
                    field_readonly("field-ro", "Read only"),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
