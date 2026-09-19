-- Lua source for inventory row "Menu buttons" (spec 013 T009).
--
-- Ported by hand from `page/menu_buttons.rs`'s `MenuButtons::body()`, at the
-- page's default state (`open = false`):
--
--   section("mb", "Menu button", vec![body(
--       "mb-body", sp("spacing.md"),
--       vec![menu_button("mb", "More", self.open, vec![menu_item(DUPLICATE, "Duplicate")])],
--   )])
--
-- where `DUPLICATE` is the page-private const `"mb-0"`.
return ui.section({
  key = "mb",
  label = "Menu button",
  children = {
    common.body("mb-body", "spacing.md", {
      ui.menu_button({
        key = "mb",
        label = "More",
        open = false,
        children = {
          ui.menu_item({ key = "mb-0", label = "Duplicate" }),
        },
      }),
    }),
  },
})
