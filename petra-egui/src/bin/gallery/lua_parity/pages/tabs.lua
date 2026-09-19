-- Lua source for inventory row "Tabs" (spec 013 T009).
--
-- Ported by hand from `page/tabs.rs`'s `Tabs::body()`, at the page's
-- `#[derive(Default)]` state (`line_tab = 0`, `contained_tab = 0`,
-- `vertical_tab = 0`). `tab_panel` is `page/tabs.rs`'s own private helper
-- (a `common.column`, padded and filled, showing the selected vertical
-- tab's content) -- inlined here as a local Lua function, not a
-- `common.lua` addition, since it is page-private in Rust too. It builds
-- from `common.column` and then mutates the returned node's own `props`,
-- the same pattern `common.lua`'s own `filled_body` uses.
--
-- The strip-beside-panel layout (`vertical_with_panel` in Rust) is a raw
-- `NodeKind::Grid` the page builds directly, not a registry component, so
-- it is rebuilt here from `ui.node.grid`.
local function tab_panel(content)
  local panel = common.column("panel", nil, { common.wrapped("panel-text", content) })
  panel.props.padding = ui.padding_symmetric("spacing.md", "spacing.md")
  panel.props.tokens = { background = "surface.raised" }
  return panel
end

local vertical = ui.vertical_tab_bar({
  key = "vert-strip",
  children = {
    ui.vertical_tab({ key = "tab-vert-0", label = "North", selected = true }),
    ui.vertical_tab({ key = "tab-vert-1", label = "South", selected = false }),
  },
})

local vertical_with_panel = ui.node.grid({
  key = "vert",
  columns = { ui.track.fit_content(), ui.track.weight(1.0) },
  align = "stretch",
  children = { vertical, tab_panel("North panel") },
})

return ui.section({
  key = "strips",
  label = "Line, contained, vertical",
  children = {
    common.body("tabs", "spacing.md", {
      ui.tab_bar({
        key = "line-strip",
        children = {
          ui.tab({ key = "tab-line-0", label = "Fibers", selected = true }),
          ui.tab({ key = "tab-line-1", label = "Trace", selected = false }),
        },
      }),
      ui.contained_tab_bar({
        key = "cont-strip",
        children = {
          ui.contained_tab({ key = "tab-cont-0", label = "One", selected = true }),
          ui.contained_tab({ key = "tab-cont-1", label = "Two", selected = false }),
        },
      }),
      vertical_with_panel,
    }),
  },
})
