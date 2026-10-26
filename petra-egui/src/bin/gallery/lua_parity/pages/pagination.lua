-- Lua source for inventory row "Pagination" (spec 013 T009).
--
-- Ported by hand from `page/pagination.rs`'s `Pagination::body()`, at the
-- page's default state (`pager = 1`, `page_size = PAGE_SIZES[0] = 10`,
-- `open = None`, `nav_page = 5`, `nav_overflow = None`, so
-- `table = pagination_items(...)`, `page_count() =
-- TOTAL_ITEMS.div_ceil(page_size) = 50.div_ceil(10) = 5`, and the nav
-- variant is its closed form):
--
--   section("pages", "Pagination", vec![body(
--       "bars", sp("spacing.md"),
--       vec![
--           heading("table-title", "Items, range, numbers, and nav"),
--           pagination_items(PAGER, 1, 10, TOTAL_ITEMS),
--           heading("compact-title", "Numbers and nav"),
--           row(COMPACT, sp("spacing.md"), vec![
--               pagination_numbers("compact-nums", 1, 5),
--               pagination_prev_next("compact-nav", 1, 5),
--           ]),
--           heading("nav-title", "Pagination nav, the second variant"),
--           pagination_nav(NAV, 5, NAV_PAGES),
--       ],
--   )])
--
-- where `PAGER = "pager"`, `COMPACT = "compact"`, `NAV = "nav-variant"`,
-- `TOTAL_ITEMS = 50`, `NAV_PAGES = 20`.
return ui.section({
  key = "pages",
  label = "Pagination",
  children = {
    common.body("bars", "spacing.md", {
      ui.heading({ key = "table-title", label = "Items, range, numbers, and nav" }),
      ui.pagination_items({ key = "pager", page = 1, page_size = 10, total_items = 50 }),
      ui.heading({ key = "compact-title", label = "Numbers and nav" }),
      common.row("compact", "spacing.md", {
        ui.pagination_numbers({ key = "compact-nums", page = 1, page_count = 5 }),
        ui.pagination_prev_next({ key = "compact-nav", page = 1, page_count = 5 }),
      }),
      ui.heading({ key = "nav-title", label = "Pagination nav, the second variant" }),
      ui.pagination_nav({ key = "nav-variant", page = 5, page_count = 20 }),
    }),
  },
})
