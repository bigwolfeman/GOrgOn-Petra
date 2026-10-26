-- Lua source for inventory row "Tooltip" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/tooltip.rs`'s `Tooltip::body()`, at the page's
-- default state (`hovered = false`, `focused = false`, so `open()` is
-- false and the bubble is not in the tree):
--
--   let pair = vec![ghost_button(TRIGGER, "Save")];
--   section("tip", "Tooltip", vec![body(
--       "tip-body", sp("spacing.md"),
--       vec![
--           column("tip-pair", None, pair),
--           heading("icon-title", "Icon tooltip"),
--           column("tip-icon", None, vec![
--               ghost_button("trigger", "Information"),
--               tooltip_icon("icon-bubble", "Information",
--                   "Built from the tree, not a window."),
--           ]),
--           heading("definition-title", "Definition tooltip"),
--           tooltip_definition("term-cell", "Fiber",
--               "A unit of concurrent work in the kernel."),
--       ],
--   )])
--
-- `column("tip-pair", nil, pair)` is `page/common.rs`'s `column`, mirrored
-- by `common.lua`'s `M.column`, called with no spacing token (`nil`) — the
-- same helper `drawer.lua` calls, but here with `nil` where `drawer.lua`
-- passes a real token, exercising that branch of `M.column`'s `row_gap`.
-- The icon and definition cells are spec 008 T052's two variant shapes.
return ui.section({
  key = "tip",
  label = "Tooltip",
  children = {
    common.body("tip-body", "spacing.md", {
      common.column("tip-pair", nil, {
        ui.ghost_button({ key = "trigger", label = "Save" }),
      }),
      ui.heading({ key = "icon-title", label = "Icon tooltip" }),
      common.column("tip-icon", nil, {
        ui.ghost_button({ key = "trigger", label = "Information" }),
        ui.tooltip_icon({
          key = "icon-bubble",
          label = "Information",
          value = "Built from the tree, not a window.",
        }),
      }),
      ui.heading({ key = "definition-title", label = "Definition tooltip" }),
      ui.tooltip_definition({
        key = "term-cell",
        term = "Fiber",
        body = "A unit of concurrent work in the kernel.",
      }),
    }),
  },
})
