-- Lua source for inventory row "Content switcher" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/content_switcher.rs`'s `ContentSwitcher::body()`,
-- at the page's default state (`switcher = 0`, so the first item reads
-- selected):
--
--   section("switcher", "Switcher", vec![body(
--       "switch", sp("spacing.md"),
--       vec![content_switcher("sw", vec![
--           content_switcher_item(SW_0, "List", self.switcher == 0),
--           content_switcher_item(SW_1, "Grid", self.switcher == 1),
--       ])],
--   )])
return ui.section({
  key = "switcher",
  label = "Switcher",
  children = {
    common.body("switch", "spacing.md", {
      ui.content_switcher({
        key = "sw",
        children = {
          ui.content_switcher_item({ key = "sw-0", label = "List", selected = true }),
          ui.content_switcher_item({ key = "sw-1", label = "Grid", selected = false }),
        },
      }),
    }),
  },
})
