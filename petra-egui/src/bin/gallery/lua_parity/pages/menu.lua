-- Lua source for inventory row "Menu" (spec 013 T009).
--
-- Ported by hand from `page/menu.rs`'s `Menu::body()`, at the page's
-- default state (`open = false`, the `#[derive(Default)]` bool default):
-- the menu itself is not mounted, only its trigger button.
return ui.section({
  key = "menu",
  label = "Menu",
  children = {
    common.body("mn", "spacing.md", {
      common.column("mn-pair", nil, {
        ui.button({ key = "trigger", label = "Actions" }),
      }),
    }),
  },
})
