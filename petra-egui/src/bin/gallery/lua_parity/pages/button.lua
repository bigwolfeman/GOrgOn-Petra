-- Lua source for inventory row "Button" (spec 013 T009).
--
-- Ported by hand from `page/button.rs`'s `Button::body()`. The page holds
-- no state, so there is only one state to port:
--
--   section("variants", "Variants and sizes", vec![body(
--       "buttons", sp("spacing.md"),
--       vec![
--           row("kinds", sp("spacing.md"), vec![
--               primary_button("btn-primary", "Primary"), button("btn-default", "Default"),
--               tertiary_button("btn-tertiary", "Tertiary"), ghost_button("btn-ghost", "Ghost"),
--           ]),
--           row("danger", sp("spacing.md"), vec![
--               danger_button("btn-danger", "Delete"),
--               danger_tertiary_button("btn-danger-tertiary", "Delete"),
--               danger_ghost_button("btn-danger-ghost", "Delete"),
--           ]),
--           row("sizes", sp("spacing.md"), vec![
--               button_xs("btn-xs", "X-small 24"), button_sm("btn-sm", "Small 32"),
--               button("btn-md", "Medium 40"), button_lg("btn-lg", "Large 48"),
--               button_xl("btn-xl", "X-large 64"), button_2xl("btn-2xl", "2X-large 80"),
--           ]),
--       ],
--   )])
return ui.section({
  key = "variants",
  label = "Variants and sizes",
  children = {
    common.body("buttons", "spacing.md", {
      common.row("kinds", "spacing.md", {
        ui.primary_button({ key = "btn-primary", label = "Primary" }),
        ui.button({ key = "btn-default", label = "Default" }),
        ui.tertiary_button({ key = "btn-tertiary", label = "Tertiary" }),
        ui.ghost_button({ key = "btn-ghost", label = "Ghost" }),
      }),
      common.row("danger", "spacing.md", {
        ui.danger_button({ key = "btn-danger", label = "Delete" }),
        ui.danger_tertiary_button({ key = "btn-danger-tertiary", label = "Delete" }),
        ui.danger_ghost_button({ key = "btn-danger-ghost", label = "Delete" }),
      }),
      common.row("sizes", "spacing.md", {
        ui.button_xs({ key = "btn-xs", label = "X-small 24" }),
        ui.button_sm({ key = "btn-sm", label = "Small 32" }),
        ui.button({ key = "btn-md", label = "Medium 40" }),
        ui.button_lg({ key = "btn-lg", label = "Large 48" }),
        ui.button_xl({ key = "btn-xl", label = "X-large 64" }),
        ui.button_2xl({ key = "btn-2xl", label = "2X-large 80" }),
      }),
    }),
  },
})
