//! Inventory row 28, Search.

use gorgon_petra::component::{search, section, valued};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const Q: &str = "query";

/// The Search page.
///
/// Holds the query, so the field can contain something. `search` writes only
/// a placeholder, the same gap row 34 had: the operator typed into it, saw
/// nothing appear, and called it broken.
#[derive(Default)]
pub struct Search {
    query: String,
}

impl Page for Search {
    fn row(&self) -> &'static str {
        "Search"
    }

    fn body(&self) -> ViewNode {
        section(
            "search",
            "Search",
            vec![filled_body(
                "q",
                sp("spacing.md"),
                vec![valued(search(Q, "Filter fibers"), self.query.clone())],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if !path_has(node, Q) {
            return false;
        }
        match event {
            InputEvent::Text(typed) => {
                self.query.push_str(typed);
                true
            }
            InputEvent::Key {
                key: KeyCode::Backspace,
                pressed: true,
                ..
            } => {
                self.query.pop();
                true
            }
            _ => false,
        }
    }
}
