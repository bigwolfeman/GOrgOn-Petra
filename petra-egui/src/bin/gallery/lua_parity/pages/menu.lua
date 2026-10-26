-- Lua source for inventory row "Menu" (spec 013 T009).
--
-- Ported by hand from `page/menu.rs`'s `Menu::body()`, at the page's
-- default state (`open = false`, the `#[derive(Default)]` bool default):
-- the live menu is not mounted, only its trigger button, and the T050
-- variant row under it is static:
--
--   section("menu", "Menu", vec![body(
--       "mn", sp("spacing.md"),
--       vec![
--           column("mn-pair", None, vec![button(TRIGGER, "Actions")]),
--           heading("variants-title", "The documented modifiers and sub-parts"),
--           row("mn-variants", sp("spacing.lg"), vec![
--               column("v-border", sp("spacing.sm"), vec![
--                   text("v-border-title", "--border"),
--                   button(TRIGGER, "Actions"),
--                   menu_border("v-menu-border", "Actions", vec![
--                       menu_item("v-border-0", "Rename"),
--                       menu_item("v-border-1", "Delete")])]),
--               column("v-icons", sp("spacing.sm"), vec![
--                   text("v-icons-title", "--with-icons"),
--                   button(TRIGGER, "Actions"),
--                   menu_with_icons("v-menu-icons", "Actions", vec![
--                       menu_item_with("v-icons-0", "Rename", Some(IconMark::Edit), None, false),
--                       menu_item_with("v-icons-1", "Delete", Some(IconMark::Close), None, false)])]),
--               column("v-selectable", sp("spacing.sm"), vec![
--                   text("v-selectable-title", "--with-selectable-items"),
--                   button(TRIGGER, "View"),
--                   menu("v-menu-selectable", "View", vec![
--                       menu_item_selectable("v-sel-0", "Keep", None, false),
--                       menu_item_selectable("v-sel-1", "Share", Some(IconMark::Check), true)])]),
--               column("v-group", sp("spacing.sm"), vec![
--                   text("v-group-title", "menu-item-group"),
--                   button(TRIGGER, "Actions"),
--                   menu("v-menu-group", "Actions", vec![
--                       menu_item("v-group-before", "Rename"),
--                       menu_item_group("v-group-inner", "View", vec![
--                           menu_item("v-group-0", "Zoom in"),
--                           menu_item("v-group-1", "Zoom out")])])]),
--               column("v-radio", sp("spacing.sm"), vec![
--                   text("v-radio-title", "menu-item-radio-group"),
--                   button(TRIGGER, "Size"),
--                   menu("v-menu-radio", "Size", vec![
--                       menu_item_radio_group("v-radio-inner", "Size", vec![
--                           menu_item_selectable("v-radio-0", "Small", None, false),
--                           menu_item_selectable("v-radio-1", "Large", None, true)])])]),
--           ]),
--       ],
--   )])
--
-- Each variant menu anchors to a `trigger` sibling in its own column, the
-- same pattern the sizes row uses. `IconMark::Edit`/`Close`/`Check` are the
-- wire marks `"edit"`/`"close"`/`"check"`.
return ui.section({
  key = "menu",
  label = "Menu",
  children = {
    common.body("mn", "spacing.md", {
      common.column("mn-pair", nil, {
        ui.button({ key = "trigger", label = "Actions" }),
      }),
      ui.heading({ key = "variants-title", label = "The documented modifiers and sub-parts" }),
      common.row("mn-variants", "spacing.lg", {
        common.column("v-border", "spacing.sm", {
          ui.text({ key = "v-border-title", label = "--border" }),
          ui.button({ key = "trigger", label = "Actions" }),
          ui.menu_border({
            key = "v-menu-border",
            label = "Actions",
            children = {
              ui.menu_item({ key = "v-border-0", label = "Rename" }),
              ui.menu_item({ key = "v-border-1", label = "Delete" }),
            },
          }),
        }),
        common.column("v-icons", "spacing.sm", {
          ui.text({ key = "v-icons-title", label = "--with-icons" }),
          ui.button({ key = "trigger", label = "Actions" }),
          ui.menu_with_icons({
            key = "v-menu-icons",
            label = "Actions",
            children = {
              ui.menu_item_with({ key = "v-icons-0", label = "Rename", icon = "edit", submenu = false }),
              ui.menu_item_with({ key = "v-icons-1", label = "Delete", icon = "close", submenu = false }),
            },
          }),
        }),
        common.column("v-selectable", "spacing.sm", {
          ui.text({ key = "v-selectable-title", label = "--with-selectable-items" }),
          ui.button({ key = "trigger", label = "View" }),
          ui.menu({
            key = "v-menu-selectable",
            label = "View",
            children = {
              ui.menu_item_selectable({ key = "v-sel-0", label = "Keep", selected = false }),
              ui.menu_item_selectable({ key = "v-sel-1", label = "Share", icon = "check", selected = true }),
            },
          }),
        }),
        common.column("v-group", "spacing.sm", {
          ui.text({ key = "v-group-title", label = "menu-item-group" }),
          ui.button({ key = "trigger", label = "Actions" }),
          ui.menu({
            key = "v-menu-group",
            label = "Actions",
            children = {
              ui.menu_item({ key = "v-group-before", label = "Rename" }),
              ui.menu_item_group({
                key = "v-group-inner",
                label = "View",
                children = {
                  ui.menu_item({ key = "v-group-0", label = "Zoom in" }),
                  ui.menu_item({ key = "v-group-1", label = "Zoom out" }),
                },
              }),
            },
          }),
        }),
        common.column("v-radio", "spacing.sm", {
          ui.text({ key = "v-radio-title", label = "menu-item-radio-group" }),
          ui.button({ key = "trigger", label = "Size" }),
          ui.menu({
            key = "v-menu-radio",
            label = "Size",
            children = {
              ui.menu_item_radio_group({
                key = "v-radio-inner",
                label = "Size",
                children = {
                  ui.menu_item_selectable({ key = "v-radio-0", label = "Small", selected = false }),
                  ui.menu_item_selectable({ key = "v-radio-1", label = "Large", selected = true }),
                },
              }),
            },
          }),
        }),
      }),
    }),
  },
})
