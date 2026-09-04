//! Inventory row 7, Contained list.

use gorgon_petra::component::{contained_list, list_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The Contained list page. It holds no live state.
pub struct ContainedList;

impl Page for ContainedList {
    fn row(&self) -> &'static str {
        "Contained list"
    }

    fn body(&self) -> ViewNode {
        section(
            "on-page",
            "On-page header",
            vec![body(
                "contained",
                sp("spacing.md"),
                vec![contained_list(
                    "cl",
                    "Recent",
                    vec![list_item("cl-0", "Trace"), list_item("cl-1", "Store")],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
