//! Inventory row 15, Link.

use gorgon_petra::component::{link, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Link page. It holds no live state.
pub struct Link;

impl Page for Link {
    fn row(&self) -> &'static str {
        "Link"
    }

    fn body(&self) -> ViewNode {
        section(
            "links",
            "Link",
            vec![body(
                "link-row",
                sp("spacing.md"),
                vec![link("docs", "Open the spec")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
