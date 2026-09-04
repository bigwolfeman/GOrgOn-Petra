//! Inventory row 13, Form.

use gorgon_petra::component::{checkbox, field, form, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Form page. It holds no live state.
pub struct Form;

impl Page for Form {
    fn row(&self) -> &'static str {
        "Form"
    }

    fn body(&self) -> ViewNode {
        section(
            "form",
            "Form",
            vec![body(
                "form-body",
                sp("spacing.md"),
                vec![form(
                    "demo-form",
                    "Fiber",
                    vec![
                        field("form-name", "Name"),
                        checkbox("form-ok", "Enabled", true),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
