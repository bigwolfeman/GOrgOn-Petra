-- Lua source for inventory row "Tooltip" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/tooltip.rs`'s `Tooltip::body()`, at the page's
-- default state (`hovered = false`, `focused = false`, so `open()` is
-- false and the bubble is not in the tree):
--
--   let pair = vec![ghost_button(TRIGGER, "Save")];
--   section("tip", "Tooltip", vec![body(
--       "tip-body", sp("spacing.md"),
--       vec![column("tip-pair", None, pair)],
--   )])
--
-- `column("tip-pair", None, pair)` is `page/common.rs`'s `column`, mirrored
-- by `common.lua`'s `M.column`, called with no spacing token (`nil`) — the
-- same helper `drawer.lua` calls, but here with `nil` where `drawer.lua`
-- passes a real token, exercising that branch of `M.column`'s `row_gap`.
return ui.section({
  key = "tip",
  label = "Tooltip",
  children = {
    common.body("tip-body", "spacing.md", {
      common.column("tip-pair", nil, {
        ui.ghost_button({ key = "trigger", label = "Save" }),
      }),
    }),
  },
})
