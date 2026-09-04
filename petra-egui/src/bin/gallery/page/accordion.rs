//! Inventory row 1, Accordion.

use gorgon_petra::component::{accordion, accordion_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const ACC_0: &str = "acc-0";

/// Live state of the Accordion page.
pub struct Accordion {
    acc_open: bool,
}

impl Default for Accordion {
    fn default() -> Self {
        Self { acc_open: true }
    }
}

impl Page for Accordion {
    fn row(&self) -> &'static str {
        "Accordion"
    }

    fn body(&self) -> ViewNode {
        section(
            "items",
            "Items",
            vec![body(
                "accordion",
                sp("spacing.md"),
                vec![accordion(
                    "acc",
                    vec![
                        accordion_item(ACC_0, "First section", self.acc_open, "Hidden body."),
                        accordion_item("acc-1", "Second section", false, "Still closed."),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ACC_0) {
            self.acc_open = !self.acc_open;
        } else {
            return false;
        }
        true
    }
}
