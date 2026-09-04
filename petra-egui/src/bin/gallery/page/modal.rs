//! Inventory row 20, Modal.

use gorgon_petra::component::{modal, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Modal page. It holds no live state.
pub struct Modal;

impl Page for Modal {
    fn row(&self) -> &'static str {
        "Modal"
    }

    fn body(&self) -> ViewNode {
        section(
            "dialog",
            "Modal",
            vec![body(
                "md",
                sp("spacing.md"),
                vec![modal(
                    "md",
                    "Confirm rebuild",
                    "This unloads the fiber.",
                    "Rebuild",
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
