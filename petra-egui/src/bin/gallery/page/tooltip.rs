//! Inventory row 38, Tooltip.

use gorgon_petra::component::section;
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp, wrapped};

/// The Tooltip page. It holds no live state.
pub struct Tooltip;

impl Page for Tooltip {
    fn row(&self) -> &'static str {
        "Tooltip"
    }

    fn body(&self) -> ViewNode {
        section(
            "tip",
            "Tooltip",
            vec![body(
                "tip-body",
                sp("spacing.md"),
                vec![wrapped("tip-text", "Save writes the composition.")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
