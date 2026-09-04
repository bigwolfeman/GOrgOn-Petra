//! Inventory row 23, Pagination.

use gorgon_petra::component::{pagination, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

/// Live state of the Pagination page.
pub struct Pagination {
    pager: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { pager: 1 }
    }
}

impl Page for Pagination {
    fn row(&self) -> &'static str {
        "Pagination"
    }

    fn body(&self) -> ViewNode {
        section(
            "pager",
            "Pagination",
            vec![body(
                "pages",
                sp("spacing.md"),
                vec![pagination("pager", self.pager, 5)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, "pager") && path_has(node, "previous") {
            if self.pager > 1 {
                self.pager -= 1;
            }
        } else if path_has(node, "pager") && path_has(node, "next") {
            if self.pager < 5 {
                self.pager += 1;
            }
        } else {
            return false;
        }
        true
    }
}
