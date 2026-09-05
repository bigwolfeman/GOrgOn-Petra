//! Inventory row 23, Pagination.

use gorgon_petra::component::{PaginationPicker, pagination_items, pagination_items_open, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

const PAGER: &str = "pager";
/// The page sizes the picker offers and the size of the whole set: the
/// numbers the Carbon reference shot was captured with
/// (`ignored/carbon-ref/src/pages.jsx`: `pageSizes={[10, 20, 30]}`,
/// `totalItems={50}`).
const PAGE_SIZES: [u32; 3] = [10, 20, 30];
const TOTAL_ITEMS: u32 = 50;
/// The list either picker opens; `dismissed` matches it by this key.
const MENU: &str = "menu";

/// Live state of the Pagination page: the 1-indexed current page, the
/// page size, and which picker is open.
pub struct Pagination {
    pager: u32,
    page_size: u32,
    open: Option<PaginationPicker>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            pager: 1,
            page_size: PAGE_SIZES[0],
            open: None,
        }
    }
}

impl Pagination {
    fn page_count(&self) -> u32 {
        TOTAL_ITEMS.div_ceil(self.page_size.max(1))
    }

    fn toggle(&mut self, picker: PaginationPicker) {
        self.open = if self.open == Some(picker) {
            None
        } else {
            Some(picker)
        };
    }
}

impl Page for Pagination {
    fn row(&self) -> &'static str {
        "Pagination"
    }

    fn body(&self) -> ViewNode {
        let bar = match self.open {
            Some(picker) => pagination_items_open(
                PAGER,
                self.pager,
                self.page_size,
                &PAGE_SIZES,
                TOTAL_ITEMS,
                picker,
            ),
            None => pagination_items(PAGER, self.pager, self.page_size, TOTAL_ITEMS),
        };
        section(
            "pager",
            "Pagination",
            vec![body("pages", sp("spacing.md"), vec![bar])],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if !path_has(node, PAGER) {
            return false;
        }
        if path_has(node, "page-size-picker") {
            self.toggle(PaginationPicker::PageSize);
        } else if path_has(node, "page-picker") {
            self.toggle(PaginationPicker::Page);
        } else if let Some(size) = PAGE_SIZES
            .iter()
            .find(|n| path_has(node, &format!("size-{n}")))
        {
            self.page_size = *size;
            self.pager = self.pager.min(self.page_count()).max(1);
            self.open = None;
        } else if let Some(page) =
            (1..=self.page_count()).find(|n| path_has(node, &format!("page-{n}")))
        {
            self.pager = page;
            self.open = None;
        } else if path_has(node, "previous") {
            if self.pager > 1 {
                self.pager -= 1;
            }
        } else if path_has(node, "next") {
            if self.pager < self.page_count() {
                self.pager += 1;
            }
        } else {
            return false;
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            self.open = None;
        }
    }
}
