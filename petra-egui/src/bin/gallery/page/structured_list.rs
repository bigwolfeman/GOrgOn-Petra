//! Inventory row 31, Structured list.

use gorgon_petra::component::{section, structured_list, structured_list_row, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Structured list page. It holds no live state.
pub struct StructuredList;

impl Page for StructuredList {
    fn row(&self) -> &'static str {
        "Structured list"
    }

    fn body(&self) -> ViewNode {
        section(
            "table",
            "Structured list",
            vec![body(
                "sl",
                sp("spacing.md"),
                vec![structured_list(
                    "sl",
                    vec![text("h0", "Name"), text("h1", "Role")],
                    vec![
                        structured_list_row(
                            "sl-0",
                            vec![text("c0", "kernel"), text("c1", "runtime")],
                            false,
                        ),
                        structured_list_row(
                            "sl-1",
                            vec![text("c0", "petra"), text("c1", "layout")],
                            true,
                        ),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
