-- Lua source for inventory row "Menubar" (spec 013 T009).
--
-- Ported by hand from `page/menubar.rs`'s `Menubar::body()`, at the page's
-- default state (`open = Some(Open::File)`, `flyout = false`):
--
--   * File is the open slot: it re-keys its label to "trigger" and mounts
--     `menu("mb-file-menu", "File", file_items(false))` beside it, both
--     inside `hugging("mb-file-open", ...)`.
--   * `file_items(false)` (flyout shut) folds "Open Recent" into one
--     `menu_item_with` row rather than a nested flyout stack.
--   * Edit and View are closed slots: each is a bare `menu_item` keyed by
--     its own constant (`mb-edit`, `mb-view`), returned directly by
--     `Menubar::slot` with no menu mounted beside it.
--
-- `hugging` is `page/menubar.rs`'s own private helper (a vertical `Stack`,
-- stretched on the cross axis, holding a trigger and its overlay as
-- siblings with no flow size of their own) -- there is no registry row for
-- a raw `NodeKind::Stack`, so it is rebuilt here from `ui.node.stack`
-- rather than invented as a sixth `common.lua` helper: it is page-private
-- in Rust too.
local function hugging(key, children)
  return ui.node.stack({
    key = key,
    axis = "vertical",
    align = "stretch",
    children = children,
  })
end

local file_items = {
  ui.menu_item_with({ key = "mb-file-new", label = "New", icon = "add", shortcut = "Ctrl+N", submenu = false }),
  ui.menu_item_with({ key = "mb-file-open", label = "Open", icon = "search", shortcut = "Ctrl+O", submenu = false }),
  ui.menu_item_with({ key = "mb-file-recent-item", label = "Open Recent", icon = "menu", submenu = true }),
  ui.menu_item_with({ key = "mb-file-quit", label = "Quit", icon = "close", shortcut = "Ctrl+Q", submenu = false }),
}

return ui.section({
  key = "bar",
  label = "Top edge",
  children = {
    common.body("mb-body", "spacing.md", {
      common.wrapped(
        "mb-note",
        "File, Edit and View dock to the top of the window. Menus list icons and "
          .. "Ctrl+ shortcuts. Hover File \xe2\x86\x92 Open Recent to fold it out to the right."
      ),
      ui.menubar_top({
        key = "menubar",
        label = "Application",
        children = {
          hugging("mb-file-open", {
            ui.menu_item({ key = "trigger", label = "File" }),
            ui.menu({ key = "mb-file-menu", label = "File", children = file_items }),
          }),
          ui.menu_item({ key = "mb-edit", label = "Edit" }),
          ui.menu_item({ key = "mb-view", label = "View" }),
        },
      }),
    }),
  },
})
