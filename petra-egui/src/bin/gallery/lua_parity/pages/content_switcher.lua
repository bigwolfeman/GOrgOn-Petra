-- Lua source for inventory row "Content switcher" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/content_switcher.rs`'s `ContentSwitcher::body()`,
-- at the page's default state (`switcher = 0`, so the first item reads
-- selected):
--
--   section("switcher", "Switcher", vec![body(
--       "switch", sp("spacing.md"),
--       vec![
--           content_switcher("sw", vec![
--               content_switcher_item(SW_0, "List", self.switcher == 0),
--               content_switcher_item(SW_1, "Grid", self.switcher == 1),
--           ]),
--           heading("low-title", "Low contrast"),
--           column("low-column", None, vec![content_switcher_low_contrast(
--               "low-contrast", vec![
--                   content_switcher_item_low_contrast("low-0", "List", true),
--                   content_switcher_item_low_contrast("low-1", "Grid", false),
--               ])]),
--           heading("icon-title", "Icon only"),
--           column("icon-column", None, vec![content_switcher_icon_only(
--               "icon-only", vec![
--                   content_switcher_item_icon("icon-0", "List", IconMark::Menu, true),
--                   content_switcher_item_icon("icon-1", "Grid", IconMark::Switcher, false),
--               ])]),
--       ],
--   )])
--
-- The low-contrast and icon-only cells are spec 008 T053's two variants.
-- `IconMark::Menu` / `IconMark::Switcher` are the wire marks `"menu"` /
-- `"switcher"`.
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
      ui.heading({ key = "low-title", label = "Low contrast" }),
      common.column("low-column", nil, {
        ui.content_switcher_low_contrast({
          key = "low-contrast",
          children = {
            ui.content_switcher_item_low_contrast({ key = "low-0", label = "List", selected = true }),
            ui.content_switcher_item_low_contrast({ key = "low-1", label = "Grid", selected = false }),
          },
        }),
      }),
      ui.heading({ key = "icon-title", label = "Icon only" }),
      common.column("icon-column", nil, {
        ui.content_switcher_icon_only({
          key = "icon-only",
          children = {
            ui.content_switcher_item_icon({
              key = "icon-0",
              label = "List",
              mark = "menu",
              selected = true,
            }),
            ui.content_switcher_item_icon({
              key = "icon-1",
              label = "Grid",
              mark = "switcher",
              selected = false,
            }),
          },
        }),
      }),
      -- Register row 113's `--popover-wrapped` modifier.
      ui.heading({ key = "popover-title", label = "Popover-wrapped" }),
      common.column("popover-column", nil, {
        ui.content_switcher_icon_only({
          key = "popover-wrapped",
          children = {
            ui.content_switcher_item_icon_popover({
              key = "wrap-0",
              label = "List",
              mark = "menu",
              selected = true,
            }),
            ui.content_switcher_item_icon({
              key = "wrap-1",
              label = "Grid",
              mark = "switcher",
              selected = false,
            }),
          },
        }),
      }),
    }),
  },
})
