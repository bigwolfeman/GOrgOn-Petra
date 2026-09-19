-- Lua source for inventory row "UI shell header" (spec 013 T009).
--
-- Ported by hand from `page/ui_shell_header.rs`'s `UiShellHeader::body()`, at
-- the page's default state (`nav = 0`, `menu_open = false`, `action = None`):
--
--   section("shell-header-section", "Header", vec![body(
--       "shell-header-body", sp("spacing.md"),
--       vec![ui_shell_header(
--           "shell-header", "GOrgOn",
--           Some(ui_shell_header_menu_trigger(MENU, self.menu_open)),
--           NAV.map(|(key,label), i| ui_shell_header_nav_item(key, label, i == self.nav)),
--           ACTIONS.map(|(key,label), i| ui_shell_header_action(key, label, self.action == Some(i))),
--       )],
--   )])
--
-- where `MENU = "shell-menu"`, `NAV = [("shell-nav-overview","Overview"),
-- ("shell-nav-fibers","Fibers")]`, `ACTIONS = [("shell-action-notify",
-- "Notifications"), ("shell-action-switcher","App switcher")]`. At the
-- default state `nav == 0` selects only the first nav item, and
-- `action == None` selects neither action.
return ui.section({
  key = "shell-header-section",
  label = "Header",
  children = {
    common.body("shell-header-body", "spacing.md", {
      ui.ui_shell_header({
        key = "shell-header",
        product_name = "GOrgOn",
        menu_trigger = ui.ui_shell_header_menu_trigger({ key = "shell-menu", open = false }),
        nav = {
          ui.ui_shell_header_nav_item({ key = "shell-nav-overview", label = "Overview", selected = true }),
          ui.ui_shell_header_nav_item({ key = "shell-nav-fibers", label = "Fibers", selected = false }),
        },
        actions = {
          ui.ui_shell_header_action({ key = "shell-action-notify", label = "Notifications", selected = false }),
          ui.ui_shell_header_action({ key = "shell-action-switcher", label = "App switcher", selected = false }),
        },
      }),
    }),
  },
})
