//! Inventory row 8, Content switcher.

use gorgon_petra::component::{content_switcher, content_switcher_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const SW_0: &str = "sw-0";
const SW_1: &str = "sw-1";

/// Live state of the Content switcher page.
#[derive(Default)]
pub struct ContentSwitcher {
    switcher: u8,
}

impl Page for ContentSwitcher {
    fn row(&self) -> &'static str {
        "Content switcher"
    }

    fn body(&self) -> ViewNode {
        section(
            "switcher",
            "Switcher",
            vec![body(
                "switch",
                sp("spacing.md"),
                vec![content_switcher(
                    "sw",
                    vec![
                        content_switcher_item(SW_0, "List", self.switcher == 0),
                        content_switcher_item(SW_1, "Grid", self.switcher == 1),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, SW_0) {
            self.switcher = 0;
        } else if path_has(node, SW_1) {
            self.switcher = 1;
        } else {
            return false;
        }
        true
    }
}
