//! Inventory row 39, Tree view.

use gorgon_petra::component::{section, tree_item, tree_view};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Tree view page. It holds no live state.
pub struct TreeView;

impl Page for TreeView {
    fn row(&self) -> &'static str {
        "Tree view"
    }

    fn body(&self) -> ViewNode {
        section(
            "tree",
            "Tree view",
            vec![body(
                "tv",
                sp("spacing.md"),
                vec![tree_view(
                    "tv",
                    vec![tree_item(
                        "tv-0",
                        "gorgon",
                        true,
                        false,
                        vec![tree_item("tv-0-0", "petra", false, true, vec![])],
                    )],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
