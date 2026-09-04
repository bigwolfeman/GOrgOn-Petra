//! Inventory row 37, Toggletip.

use gorgon_petra::component::{section, toggletip};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Toggletip page. It holds no live state.
pub struct Toggletip;

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
                vec![toggletip("tt", "Why", false, "Because the spec says so.")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
