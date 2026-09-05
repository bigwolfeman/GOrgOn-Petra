//! Inventory row 37, Toggletip.

use gorgon_petra::component::{section, toggletip};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

/// `toggletip`'s own keys for its trigger and its popover.
const TRIGGER: &str = "trigger";
const TIP: &str = "tip";

/// Live state of the Toggletip page: whether the tip is showing.
#[derive(Default)]
pub struct Toggletip {
    open: bool,
}

impl Page for Toggletip {
    fn row(&self) -> &'static str {
        "Toggletip"
    }

    fn body(&self) -> ViewNode {
        section(
            "tt",
            "Toggletip",
            vec![body(
                "tt-body",
                sp("spacing.md"),
                vec![toggletip(
                    "tt",
                    "Why",
                    self.open,
                    "Because the spec says so.",
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, TRIGGER) {
            self.open = !self.open;
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, TIP) {
            self.open = false;
        }
    }
}
