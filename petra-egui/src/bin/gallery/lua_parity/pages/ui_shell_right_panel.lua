-- Lua source for inventory row "UI shell right panel" (spec 013 T009).
--
-- Ported by hand from `page/ui_shell_right_panel.rs`'s
-- `UiShellRightPanel::body()`, at the page's default state (`impl Default`:
-- `open = true`, `current = 0`).
--
--   header = ui_shell_header("shell-header", "GOrgOn", None, vec![],
--       vec![ui_shell_header_action("shell-switcher-trigger", "App switcher", true)]);
--   switcher = ui_shell_switcher("shell-switcher", "App switcher", true, vec![
--       ui_shell_switcher_item("shell-switcher-petra", "Petra", true),
--       ui_shell_right_panel_divider("shell-switcher-div"),
--       ui_shell_switcher_item("shell-switcher-inspector", "Inspector", false),
--   ]);
--   stub = Grid("shell-page") { columns: [Weight(1.0)], rows: [Weight(1.0)],
--       tokens: { background: "surface.base" } };
--   content = Grid("shell-content") {
--       columns: [Weight(1.0), FitContent], rows: [Weight(1.0)], align: Stretch,
--       children: [stub, switcher],
--   };
--   frame = Grid("shell-frame") {
--       columns: [Weight(1.0)], rows: [FitContent, Weight(1.0)], align: Stretch,
--       children: [header, content],
--       constraints: { vertical: { min: 320.0, max: 320.0 } },
--       tokens: { background: "surface.base" },
--   };
--   section("shell-right-section", "Header with the switcher", vec![filled_body(
--       "shell-right-body", sp("spacing.md"), vec![frame],
--   )])
local header = ui.ui_shell_header({
  key = "shell-header",
  product_name = "GOrgOn",
  actions = {
    ui.ui_shell_header_action({ key = "shell-switcher-trigger", label = "App switcher", selected = true }),
  },
})

local switcher = ui.ui_shell_switcher({
  key = "shell-switcher",
  label = "App switcher",
  open = true,
  children = {
    ui.ui_shell_switcher_item({ key = "shell-switcher-petra", label = "Petra", selected = true }),
    ui.ui_shell_right_panel_divider({ key = "shell-switcher-div" }),
    ui.ui_shell_switcher_item({ key = "shell-switcher-inspector", label = "Inspector", selected = false }),
  },
})

local stub = ui.node.grid({
  key = "shell-page",
  columns = { ui.track.weight(1.0) },
  rows = { ui.track.weight(1.0) },
  tokens = { background = "surface.base" },
})

local content = ui.node.grid({
  key = "shell-content",
  columns = { ui.track.weight(1.0), ui.track.fit_content() },
  rows = { ui.track.weight(1.0) },
  align = "stretch",
  children = { stub, switcher },
})

local frame = ui.node.grid({
  key = "shell-frame",
  columns = { ui.track.weight(1.0) },
  rows = { ui.track.fit_content(), ui.track.weight(1.0) },
  align = "stretch",
  children = { header, content },
  constraints = { vertical = { min = 320.0, max = 320.0 } },
  tokens = { background = "surface.base" },
})

return ui.section({
  key = "shell-right-section",
  label = "Header with the switcher",
  children = {
    common.filled_body("shell-right-body", "spacing.md", { frame }),
  },
})
