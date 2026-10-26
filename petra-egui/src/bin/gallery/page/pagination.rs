//! Inventory row 23, Pagination.
//!
//! Two bars and the second variant. The table bar is [`pagination_items`],
//! which composes the page size picker, the range caption, the number row
//! and the prev/next pair, plus the two pickers. The compact bar is the
//! number row and the prev/next pair only. The Pagination nav variant
//! (spec 008 T043) is its own cell on its own set: a page-number row, a
//! selected page, and an overflow affordance.
//!
//! The four pieces are not mounted a second time. Their inner keys
//! (`page-size-picker`, `range-text`) are hardcoded, and a duplicate key
//! makes `Camera::click` panic.

use gorgon_petra::component::{
    PaginationNavOverflow, PaginationPicker, heading, pagination_items, pagination_items_open,
    pagination_nav, pagination_nav_open, pagination_numbers, pagination_prev_next, section,
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
/// Ancestor of the Pagination nav **variant** cell (spec 008 T043). Own
/// page set (`NAV_PAGES`), so the overflow affordance has something to
/// collapse and the selected page has somewhere to move.
const NAV: &str = "nav-variant";
/// The variant's own page count. The bars run on `TOTAL_ITEMS /
/// PAGE_SIZES` (2 to 5 pages, never a collapsed run); the variant is the
/// component that documents an overflow affordance, so it needs a set
/// wide enough to produce one.
const NAV_PAGES: u32 = 20;
/// The page sizes the picker offers and the size of the whole set: the
/// numbers the Carbon reference shot was captured with
/// (`ignored/carbon-ref/src/pages.jsx`: `pageSizes={[10, 20, 30]}`,
/// `totalItems={50}`).
const PAGE_SIZES: [u32; 3] = [10, 20, 30];
const TOTAL_ITEMS: u32 = 50;
/// The list either picker opens; `dismissed` matches it by this key.
const MENU: &str = "menu";

/// Live state of the Pagination page: the 1-indexed current page, the
/// page size, and which picker is open — plus the nav variant's own page
/// and which of its overflow runs is open.
pub struct Pagination {
    pager: u32,
    page_size: u32,
    open: Option<PaginationPicker>,
    nav_page: u32,
    nav_overflow: Option<PaginationNavOverflow>,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            pager: 1,
            page_size: PAGE_SIZES[0],
            open: None,
            // Mid-set, so the resting page shows the selected page between
            // both collapsed runs — the whole anatomy in one frame.
            nav_page: 5,
            nav_overflow: None,
        }
    }
}

impl Pagination {
    fn page_count(&self) -> u32 {
        TOTAL_ITEMS.div_ceil(self.page_size.max(1))
    }

    fn toggle(&mut self, picker: PaginationPicker) {
        self.nav_overflow = None;
        self.open = if self.open == Some(picker) {
            None
        } else {
            Some(picker)
        };
    }

    fn toggle_nav(&mut self, which: PaginationNavOverflow) {
        self.open = None;
        self.nav_overflow = if self.nav_overflow == Some(which) {
            None
        } else {
            Some(which)
        };
    }

    /// The nav variant drives its own page over its own set
    /// ([`NAV_PAGES`]). Its controls carry the same inner names as the
    /// bars' (`previous` / `next` / `num-{n}`, and `page-{n}` for the rows
    /// in an overflow list), so the arms are the same shapes keyed to
    /// `nav_page` — a press and a keyboard activation route through the
    /// same arm, which is what makes the keyboard a peer of the pointer
    /// here rather than a second implementation (T046).
    fn handle_nav(&mut self, node: &str) -> bool {
        if path_has(node, "overflow-start") {
            self.toggle_nav(PaginationNavOverflow::Start);
        } else if path_has(node, "overflow-end") {
            self.toggle_nav(PaginationNavOverflow::End);
        } else if let Some(page) = (1..=NAV_PAGES).find(|n| path_has(node, &format!("page-{n}"))) {
            self.nav_page = page;
            self.nav_overflow = None;
        } else if let Some(page) = (1..=NAV_PAGES).find(|n| path_has(node, &format!("num-{n}"))) {
            self.nav_page = page;
            self.nav_overflow = None;
        } else if path_has(node, "previous") {
            self.nav_page = self.nav_page.saturating_sub(1).max(1);
        } else if path_has(node, "next") {
            self.nav_page = (self.nav_page + 1).min(NAV_PAGES);
        } else {
            return false;
        }
        true
    }

    fn ours(node: &str) -> bool {
        path_has(node, PAGER) || path_has(node, COMPACT) || path_has(node, NAV)
    }
}

impl Page for Pagination {
    fn row(&self) -> &'static str {
        "Pagination"
    }

    /// Carbon's pagination skeleton (`_pagination.scss`): `--skeleton__text`
    /// placeholder bars, 16 high × 100 % of their slot, one per item.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{row, sp};
        Some(row(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                skeleton(
                    "skeleton-text",
                    SkeletonAnatomy::Text,
                    SizeStep::Md,
                    160.0,
                    now,
                ),
                skeleton(
                    "skeleton-text-2",
                    SkeletonAnatomy::Text,
                    SizeStep::Md,
                    160.0,
                    now,
                ),
            ],
        ))
    }

    /// Carbon's pagination ramp (slice-d "Sizes": sm 32 / md 40 / lg 48;
    /// "there is no xs/xl pagination size") — the three built steps side by
    /// side, each labeled with its step and the height it lands on
    /// (FR-025). Static bars: the live pager above owns the interaction.
    fn sizes(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::{
            pagination, pagination_items_lg, pagination_items_sm, pagination_lg, pagination_nav,
            pagination_nav_lg, pagination_nav_sm, pagination_numbers_lg, pagination_numbers_sm,
            pagination_prev_next_lg, pagination_prev_next_sm, pagination_sm, text,
        };

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
                        pagination_sm("size-pg-sm", 2, 5),
                    ],
                ),
                row(
                    "size-md",
                    sp("spacing-03"),
                    vec![
                        text("size-md-label", "md · 40"),
                        pagination("size-pg-md", 2, 5),
                    ],
                ),
                row(
                    "size-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-lg-label", "lg · 48"),
                        pagination_lg("size-pg-lg", 2, 5),
                    ],
                ),
                // The sized pieces are built steps of their own (the sm and
                // lg bars above compose them): one cell per row so each wide
                // piece gets the card's cell share (FR-025).
                row(
                    "size-items-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-items-sm-label", "items sm · 32"),
                        pagination_items_sm("size-items-sm-cell", 1, 10, 50),
                    ],
                ),
                row(
                    "size-items-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-items-lg-label", "items lg · 48"),
                        pagination_items_lg("size-items-lg-cell", 1, 10, 50),
                    ],
                ),
                row(
                    "size-numbers-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-numbers-sm-label", "numbers sm · 32"),
                        pagination_numbers_sm("size-numbers-sm-cell", 1, 5),
                    ],
                ),
                row(
                    "size-numbers-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-numbers-lg-label", "numbers lg · 48"),
                        pagination_numbers_lg("size-numbers-lg-cell", 1, 5),
                    ],
                ),
                row(
                    "size-nav-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-nav-sm-label", "prev-next sm · 32"),
                        pagination_prev_next_sm("size-nav-sm-cell", 1, 5),
                    ],
                ),
                row(
                    "size-nav-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-nav-lg-label", "prev-next lg · 48"),
                        pagination_prev_next_lg("size-nav-lg-cell", 1, 5),
                    ],
                ),
                // The second variant's own three steps (spec 008 T045),
                // page 5 of 20 so both collapsed runs show at rest.
                row(
                    "size-navvar-sm",
                    sp("spacing-03"),
                    vec![
                        text("size-navvar-sm-label", "nav-variant sm · 32"),
                        pagination_nav_sm("size-navvar-sm-cell", 5, NAV_PAGES),
                    ],
                ),
                row(
                    "size-navvar-md",
                    sp("spacing-03"),
                    vec![
                        text("size-navvar-md-label", "nav-variant md · 40"),
                        pagination_nav("size-navvar-md-cell", 5, NAV_PAGES),
                    ],
                ),
                row(
                    "size-navvar-lg",
                    sp("spacing-03"),
                    vec![
                        text("size-navvar-lg-label", "nav-variant lg · 48"),
                        pagination_nav_lg("size-navvar-lg-cell", 5, NAV_PAGES),
                    ],
                ),
            ],
        )]
    }

    /// The documented disabled state (spec 008 T025, register row 352): the
    /// nested controls under `disabled()` — the page-size Select beside a
    /// nav whose Previous is unavailable on page 1 and therefore already
    /// carries the production `disabled()` at `pagination.rs`'s
    /// `previous_or_next`. The bar itself recolours nothing; the override
    /// Carbon writes at `_pagination.scss:247-263` is the nested button's
    /// own disabled ink.
    fn disabled(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![
                pagination_page_size("disabled-page-size", 10, None),
                pagination_prev_next("disabled-nav", 1, 3),
            ],
        ))]
    }

    fn body(&self) -> ViewNode {
        // Bar 1 is items + range + numbers + prev/next. `pagination_items`
        // is that composition (`pagination_page_size`, `pagination_range`,
        // `pagination_numbers`, `pagination_prev_next`) plus the two
        // pickers. Mounting the four pieces again would duplicate
        // hardcoded inner keys (`page-size-picker`, `range-text`) and
        // `Camera::click` panics when a tail matches twice.
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
                pagination_prev_next("compact-nav", self.pager, self.page_count()),
            ],
        );
        // The second variant (spec 008 T043): page-number row, selected
        // page, overflow affordance — no pickers, no range, no dividers.
        // Its own set, wide enough to collapse a run.
        let nav_variant = match self.nav_overflow {
            Some(which) => pagination_nav_open(NAV, self.nav_page, NAV_PAGES, which),
            None => pagination_nav(NAV, self.nav_page, NAV_PAGES),
        };
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
                    heading("nav-title", "Pagination nav, the second variant"),
                    nav_variant,
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if !Self::ours(node) {
            return false;
        }
        if path_has(node, NAV) {
            return self.handle_nav(node);
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
            self.nav_overflow = None;
        } else if let Some(page) =
            (1..=self.page_count()).find(|n| path_has(node, &format!("page-{n}")))
        {
            self.pager = page;
            self.open = None;
            self.nav_overflow = None;
        } else if let Some(page) =
            (1..=self.page_count()).find(|n| path_has(node, &format!("num-{n}")))
        {
            self.pager = page;
            self.open = None;
            self.nav_overflow = None;
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
            self.nav_overflow = None;
        }
    }
}
