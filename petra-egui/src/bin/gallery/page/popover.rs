//! Inventory row 24, Popover.

use gorgon_petra::component::{button, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp, wrapped};

/// The Popover page. It holds no live state.
pub struct Popover;

impl Page for Popover {
    fn row(&self) -> &'static str {
        "Popover"
    }

    fn body(&self) -> ViewNode {
        section(
            "pop",
            "Popover chrome",
            vec![body(
                "po",
                sp("spacing.md"),
                vec![
                    button("pop-anchor", "Anchor"),
                    wrapped("pop-body", "Anchored note (constructor sets Anchor::Node)."),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
