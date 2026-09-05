//! Inventory row 23, Pagination.

use gorgon_petra::component::{pagination_items, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, sp};

const PAGER: &str = "pager";
/// Items per page and the size of the whole set: five pages of ten, the
/// numbers the Carbon reference shot was captured with.
const PAGE_SIZE: u32 = 10;
const TOTAL_ITEMS: u32 = 50;

/// Live state of the Pagination page: the 1-indexed current page.
pub struct Pagination {
    pager: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { pager: 1 }
    }
}

impl Pagination {
    fn page_count() -> u32 {
        TOTAL_ITEMS.div_ceil(PAGE_SIZE)
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
                vec![pagination_items(PAGER, self.pager, PAGE_SIZE, TOTAL_ITEMS)],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, PAGER) && path_has(node, "previous") {
            if self.pager > 1 {
                self.pager -= 1;
            }
        } else if path_has(node, PAGER) && path_has(node, "next") {
            if self.pager < Self::page_count() {
                self.pager += 1;
            }
        } else {
            return false;
        }
        true
    }
}
