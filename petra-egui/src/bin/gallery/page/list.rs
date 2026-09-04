//! Inventory row 16, List.

use gorgon_petra::component::{list_item, list_item_with, ordered_list, section, unordered_list};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The List page. It holds no live state.
pub struct List;

impl Page for List {
    fn row(&self) -> &'static str {
        "List"
    }

    fn body(&self) -> ViewNode {
        section(
            "kinds",
            "Ordered, unordered, nested",
            vec![body(
                "lists",
                sp("spacing.md"),
                vec![
                    unordered_list(
                        "ul",
                        vec![
                            list_item("ul-0", "Inbox"),
                            list_item_with(
                                "ul-1",
                                "Archive",
                                Some(unordered_list(
                                    "ul-nested",
                                    vec![list_item("ul-1-0", "2025"), list_item("ul-1-1", "2026")],
                                )),
                            ),
                        ],
                    ),
                    ordered_list(
                        "ol",
                        vec![
                            list_item("ol-0", "Clone"),
                            list_item("ol-1", "Build"),
                            list_item("ol-2", "Run"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
