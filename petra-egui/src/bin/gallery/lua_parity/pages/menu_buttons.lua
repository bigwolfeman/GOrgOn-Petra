-- Lua source for inventory row "Menu buttons" (spec 013 T009).
--
-- Ported by hand from `page/menu_buttons.rs`'s `MenuButtons::body()`, at the
-- page's default state (`open = false`):
--
--   section("mb", "Menu button", vec![body(
--       "mb-body", sp("spacing.md"),
--       vec![
--           column("mb-live", None, vec![
--               menu_button("mb", "More", self.open, vec![menu_item(DUPLICATE, "Duplicate")])]),
--           heading("variants-title", "The documented trigger shapes"),
--           row("mb-variants", sp("spacing.lg"), vec![
--               column("v-combo", sp("spacing.sm"), vec![
--                   text("v-combo-title", "Combo button"),
--                   combo_button("v-combo-button", "Save", true, vec![
--                       menu_item("v-combo-0", "Save and continue"),
--                       menu_item("v-combo-1", "Save as draft")])]),
--               column("v-overflow", sp("spacing.sm"), vec![
--                   text("v-overflow-title", "Overflow menu"),
--                   overflow_menu("v-overflow-button", "More", true, vec![
--                       menu_item("v-overflow-0", "Duplicate"),
--                       menu_item("v-overflow-1", "Archive")])]),
--           ]),
--       ],
--   )])
--
-- where `DUPLICATE` is the page-private const `"mb-0"`. The combo and
-- overflow cells are spec 008 T051's two trigger shapes, both shown open.
return ui.section({
  key = "mb",
  label = "Menu button",
  children = {
    common.body("mb-body", "spacing.md", {
      common.column("mb-live", nil, {
        ui.menu_button({
          key = "mb",
          label = "More",
          open = false,
          children = {
            ui.menu_item({ key = "mb-0", label = "Duplicate" }),
          },
        }),
      }),
      ui.heading({ key = "variants-title", label = "The documented trigger shapes" }),
      common.row("mb-variants", "spacing.lg", {
        common.column("v-combo", "spacing.sm", {
          ui.text({ key = "v-combo-title", label = "Combo button" }),
          ui.combo_button({
            key = "v-combo-button",
            action = "Save",
            open = true,
            children = {
              ui.menu_item({ key = "v-combo-0", label = "Save and continue" }),
              ui.menu_item({ key = "v-combo-1", label = "Save as draft" }),
            },
          }),
        }),
        common.column("v-overflow", "spacing.sm", {
          ui.text({ key = "v-overflow-title", label = "Overflow menu" }),
          ui.overflow_menu({
            key = "v-overflow-button",
            label = "More",
            open = true,
            children = {
              ui.menu_item({ key = "v-overflow-0", label = "Duplicate" }),
              ui.menu_item({ key = "v-overflow-1", label = "Archive" }),
            },
          }),
        }),
      }),
    }),
  },
})
