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

    /// Carbon's content-switcher sizing (`_content-switcher.scss`,
    /// 32/40/48): the three built steps side by side, each labeled with its
    /// step and the height it lands on (FR-025).
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{content_switcher_lg, content_switcher_sm, text};

        use super::common::{column, row, sp};
        vec![column(
            "size-steps",
            sp("spacing-05"),
            vec![
                row(
                    "size-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-sm-label", "sm · 32"),
                        content_switcher_sm(
                            "size-sw-sm",
                            vec![
                                content_switcher_item("size-sw-sm-a", "List", true),
                                content_switcher_item("size-sw-sm-b", "Grid", false),
                            ],
                        ),
                    ],
                ),
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        content_switcher(
                            "size-sw-md",
                            vec![
                                content_switcher_item("size-sw-md-a", "List", true),
                                content_switcher_item("size-sw-md-b", "Grid", false),
                            ],
                        ),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        content_switcher_lg(
                            "size-sw-lg",
                            vec![
                                content_switcher_item("size-sw-lg-a", "List", true),
                                content_switcher_item("size-sw-lg-b", "Grid", false),
                            ],
                        ),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 122): a
    /// disabled switcher — item labels in `text.muted`, the inverse ground
    /// structural and dropped with the painter's elevation.
    fn disabled(&self) -> Vec<ViewNode> {
        use super::common::row;
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![content_switcher(
                "disabled-switcher",
                vec![
                    content_switcher_item("disabled-a", "Graph", true),
                    content_switcher_item("disabled-b", "Grid", false),
                ],
            )],
        ))]
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
