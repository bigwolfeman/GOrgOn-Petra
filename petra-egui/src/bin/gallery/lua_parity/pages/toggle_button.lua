-- Lua source for inventory row "Toggle button" (spec 013 T009).
--
-- Ported by hand from `page/toggle_button.rs`'s `ToggleButton::body()`, at
-- the page's default state (`pressed = 0`, so only `LIST` reads pressed):
--
--   section("toggles", "Pressed group", vec![body(
--       "tb-body", sp("spacing.md"),
--       vec![toggle_button_group("tb-group", vec![
--           toggle_button(LIST, "List", self.pressed == 0),
--           toggle_button(GRID, "Grid", self.pressed == 1),
--           toggle_button_icon(BOTH, "Both", self.pressed == 2, IconMark::Menu),
--       ])],
--   )])
--
-- where `LIST = "tb-list"`, `GRID = "tb-grid"`, `BOTH = "tb-both"`, and
-- `IconMark::Menu` is the wire string `"menu"`.
return ui.section({
  key = "toggles",
  label = "Pressed group",
  children = {
    common.body("tb-body", "spacing.md", {
      ui.toggle_button_group({
        key = "tb-group",
        children = {
          ui.toggle_button({ key = "tb-list", label = "List", pressed = true }),
          ui.toggle_button({ key = "tb-grid", label = "Grid", pressed = false }),
          ui.toggle_button_icon({ key = "tb-both", label = "Both", pressed = false, mark = "menu" }),
        },
      }),
    }),
  },
})
