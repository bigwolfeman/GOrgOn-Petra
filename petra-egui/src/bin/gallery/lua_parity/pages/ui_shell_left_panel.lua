-- Lua source for inventory row "UI shell left panel" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/ui_shell_left_panel.rs`'s
-- `UiShellLeftPanel::body()`, at the page's default state (`mode = 1`, so
-- `MODES[1]` = `Fixed`; `nav_open = true`; `expanded = true`;
-- `current = Current::Petra`). At `Fixed`, `mode()` returns `Fixed`
-- unchanged (the `Expandable` substitution branch never runs), and
-- `expanded = self.expanded && !matches!(mode, Rail)` is `true && true =
-- true`. `expandable = matches!(mode, Expandable)` is `false`, so the
-- trigger is built `open = false` (`expandable && self.nav_open` short-
-- circuits) and then wrapped in `disabled`.
--
-- The three raw `ViewNode::new(NodeKind::Grid, ..)` builds
-- (`PAGE`/`CONTENT`/`FRAME`) have no registry row — they are the page
-- building primitives directly, not calling a component constructor — so
-- they are ported here as `ui.node.grid` calls, the primitive vocabulary,
-- not `ui.<component>`. `FRAME`'s `Constraints { vertical: { min:
-- Some(300.0), max: Some(300.0), priority: 0 }, .. }` renders on the wire
-- as `{ vertical = { min = 300.0, max = 300.0 } }`: `priority` is `0`,
-- which `AxisConstraint`'s own `is_zero` skip omits, and `horizontal` is
-- `AxisConstraint::default()`, which `Constraints`'s `is_default_axis` skip
-- omits entirely.
--
--   let panel = ui_shell_left_panel_in("shell-left", Fixed, vec![
--       ui_shell_left_panel_icon_item(KERNEL, "Kernel", Switcher, true, false, vec![
--           ui_shell_left_panel_icon_subitem(FIBERS, "Fibers", false),
--       ]),
--       ui_shell_left_panel_icon_item(PETRA, "Petra", Edit, false, true, vec![]),
--       ui_shell_left_panel_icon_item(TRACE, "Trace", Search, false, false, vec![]),
--   ]);
--   let trigger = disabled(ui_shell_header_menu_trigger(MENU, false));
--   let stub = Grid(PAGE) { columns: [Weight(1)], rows: [Weight(1)] }, tokens.background = surface.base;
--   let content = Grid(CONTENT) { columns: [FitContent, Weight(1)], rows: [Weight(1)], align: Stretch }
--       .with_children([panel, stub]);
--   let frame = Grid(FRAME) { columns: [Weight(1)], rows: [FitContent, Weight(1)], align: Stretch }
--       .with_children([row("shell-left-bar", None, [trigger]), content])
--       .with_constraints({ vertical: { min: 300.0, max: 300.0 } });
--   frame.props.tokens.background = surface.base;
--   section("shell-left-section", "Width modes", vec![filled_body(
--       "shell-left-body", sp("spacing.md"),
--       vec![
--           content_switcher("shell-left-modes", MODES.map(|(key, label, _), i| {
--               content_switcher_item(key, label, i == 1)
--           })),
--           frame,
--       ],
--   )])
--
-- `MODES` in order: `("shell-left-rail", "Rail", Rail)`,
-- `("shell-left-fixed", "Fixed", Fixed)`,
-- `("shell-left-expandable", "Expandable", Expandable { expanded: false })`,
-- `("shell-left-hidden", "Hidden", Hidden)` — index 1 (`Fixed`) is selected.
return ui.section({
  key = "shell-left-section",
  label = "Width modes",
  children = {
    common.filled_body("shell-left-body", "spacing.md", {
      ui.content_switcher({
        key = "shell-left-modes",
        children = {
          ui.content_switcher_item({ key = "shell-left-rail", label = "Rail", selected = false }),
          ui.content_switcher_item({ key = "shell-left-fixed", label = "Fixed", selected = true }),
          ui.content_switcher_item({
            key = "shell-left-expandable",
            label = "Expandable",
            selected = false,
          }),
          ui.content_switcher_item({ key = "shell-left-hidden", label = "Hidden", selected = false }),
        },
      }),
      ui.node.grid({
        key = "shell-left-frame",
        columns = { ui.track.weight(1.0) },
        rows = { ui.track.fit_content(), ui.track.weight(1.0) },
        align = "stretch",
        constraints = {
          vertical = { min = 300.0, max = 300.0 },
        },
        tokens = { background = "surface.base" },
        children = {
          common.row("shell-left-bar", nil, {
            ui.disabled("shell-left-menu-disabled", {
              node = ui.ui_shell_header_menu_trigger({ key = "shell-left-menu", open = false }),
            }),
          }),
          ui.node.grid({
            key = "shell-left-content",
            columns = { ui.track.fit_content(), ui.track.weight(1.0) },
            rows = { ui.track.weight(1.0) },
            align = "stretch",
            children = {
              ui.ui_shell_left_panel_in({
                key = "shell-left",
                mode = { type = "fixed" },
                items = {
                  ui.ui_shell_left_panel_icon_item({
                    key = "shell-left-kernel",
                    label = "Kernel",
                    mark = "switcher",
                    expanded = true,
                    selected = false,
                    children = {
                      ui.ui_shell_left_panel_icon_subitem({
                        key = "shell-left-fibers",
                        label = "Fibers",
                        selected = false,
                      }),
                    },
                  }),
                  ui.ui_shell_left_panel_icon_item({
                    key = "shell-left-petra",
                    label = "Petra",
                    mark = "edit",
                    expanded = false,
                    selected = true,
                    children = {},
                  }),
                  ui.ui_shell_left_panel_icon_item({
                    key = "shell-left-trace",
                    label = "Trace",
                    mark = "search",
                    expanded = false,
                    selected = false,
                    children = {},
                  }),
                },
              }),
              ui.node.grid({
                key = "shell-left-page",
                columns = { ui.track.weight(1.0) },
                rows = { ui.track.weight(1.0) },
                tokens = { background = "surface.base" },
              }),
            },
          }),
        },
      }),
    }),
  },
})
