-- Lua source for inventory row "Contained list" (spec 013 T009).
--
-- Ported by hand from `page/contained_list.rs`'s `ContainedList::body()`,
-- at the page's default state (`selected = None`, `#[derive(Default)]`, so
-- both rows are unselected):
--
--   const ROWS = [("cl-0", "Trace"), ("cl-1", "Store")]
--   section("on-page", "On-page header", vec![filled_body(
--       "contained", sp("spacing.md"),
--       vec![contained_list("cl", "Recent", vec![
--           list_row("cl-0", "Trace", false),
--           list_row("cl-1", "Store", false),
--       ])],
--   )])
return ui.section({
  key = "on-page",
  label = "On-page header",
  children = {
    common.filled_body("contained", "spacing.md", {
      ui.contained_list({
        key = "cl",
        label = "Recent",
        children = {
          ui.list_row({ key = "cl-0", label = "Trace", selected = false }),
          ui.list_row({ key = "cl-1", label = "Store", selected = false }),
        },
      }),
    }),
  },
})
