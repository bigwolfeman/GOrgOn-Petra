//! Inventory row 28, Search.

use gorgon_petra::component::{search, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Search page. It holds no live state.
pub struct Search;

impl Page for Search {
    fn row(&self) -> &'static str {
        "Search"
    }

    fn body(&self) -> ViewNode {
        section(
            "search",
            "Search",
            vec![body(
                "q",
                sp("spacing.md"),
                vec![search("q", "Filter fibers")],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
