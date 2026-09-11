//! Inventory row 23, Pagination.
//!
//! Two bars. The table bar is [`pagination_items`]: that constructor is
//! [`pagination_page_size`] + [`pagination_range`] + [`pagination_numbers`]
//! + [`pagination_nav`], plus the two pickers. The compact bar is
//! numbers + nav only. The four pieces are not mounted a second time;
//! their inner keys (`page-size-picker`, `range-text`) are hardcoded and
//! a duplicate makes `Camera::click` panic.

use gorgon_petra::component::{
    PaginationPicker, heading, pagination_items, pagination_items_open, pagination_nav,
    pagination_numbers, section,
};
// The other two pieces of bar 1. `pagination_items` already mounts them.
// A second copy collides on hardcoded inner keys (`page-size-picker`,
// `range-text`) and `Camera::click` panics when a tail matches twice.
#[allow(
    unused_imports,
    reason = "kit names; pagination_items already mounts them"
)]
use gorgon_petra::component::{pagination_page_size, pagination_range};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, row, sp};

const PAGER: &str = "pager";
/// Ancestor of the numbers+nav bar. A chrome Next is keyed `next` with no
/// such segment, so a press there still falls through to the catalog.
const COMPACT: &str = "compact";
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

    fn ours(node: &str) -> bool {
        path_has(node, PAGER) || path_has(node, COMPACT)
    }
}

impl Page for Pagination {
    fn row(&self) -> &'static str {
        "Pagination"
    }

    fn body(&self) -> ViewNode {
        // Bar 1 is items + range + numbers + nav. `pagination_items` is
        // that composition (`pagination_page_size`, `pagination_range`,
        // `pagination_numbers`, `pagination_nav`) plus the two pickers.
        // Mounting the four pieces again would duplicate hardcoded inner
        // keys (`page-size-picker`, `range-text`) and `Camera::click`
        // panics when a tail matches twice.
        let table = match self.open {
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
        // Bar 2 is numbers + nav only. Outer keys are unique; inner
        // `previous` / `next` / `num-{n}` share names with bar 1, so
        // handle accepts either ancestor.
        let compact = row(
            COMPACT,
            sp("spacing.md"),
            vec![
                pagination_numbers("compact-nums", self.pager, self.page_count()),
                pagination_nav("compact-nav", self.pager, self.page_count()),
            ],
        );
        section(
            "pages",
            "Pagination",
            vec![body(
                "bars",
                sp("spacing.md"),
                vec![
                    heading("table-title", "Items, range, numbers, and nav"),
                    table,
                    heading("compact-title", "Numbers and nav"),
                    compact,
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if !Self::ours(node) {
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
        } else if let Some(page) =
            (1..=self.page_count()).find(|n| path_has(node, &format!("num-{n}")))
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
