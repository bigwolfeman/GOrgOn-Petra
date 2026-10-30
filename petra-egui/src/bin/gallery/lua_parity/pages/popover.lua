-- Lua source for inventory row "Popover" (spec 013 T009).
--
-- Ported by hand from `page/popover.rs`'s `Popover::body()`, at the page's
-- default state (`open = true`, `border_open = true`, `auto_open = true`,
-- from `impl Default for Popover`). At
-- `open = true` the `if !self.open { ... placement_grid() ... }` branch
-- never runs, so this page's ninety-odd lines of raw grid/cell construction
-- (`placement_pair`, `placement_grid`, `lead_in`, `cell_box`, `trigger_seat`)
-- are dead code at this state and are not ported here — the resting panel
-- plus the bordered pair (register row 358) and the auto-aligned pair
-- (row 361):
--
--   let mut pair = vec![button(ANCHOR, "Filters")];
--   // self.open is true:
--   pair.push(popover_with(NOTE, "Filter fibers", ANCHOR, vec![
--       heading("pop-title", "Filter fibers"),
--       wrapped("pop-body", "A popover is the anchored surface every other overlay \
--           in this library is built on. Its contents are the caller's, and they \
--           keep their own roles."),
--       row("pop-actions", sp("spacing.sm"), vec![
--           button("pop-cancel", "Cancel"),
--           primary_button("pop-apply", "Apply"),
--       ]),
--   ]));
--   section("pop", "Popover", vec![body("po", sp("spacing.md"), vec![
--       column("po-pair", None, pair),
--       column("po-shut", None, vec![button(SHUT, "Closed trigger")]),
--       wrapped("po-note", "Press either trigger. The panel is 368 wide on \
--           surface.raised; a toggletip is 288 on the ramp's last rung."),
--       column("po-border", None, vec![
--           button(BORDER_ANCHOR, "Filters"),
--           popover_border(BORDER_NOTE, "Filter fibers", BORDER_ANCHOR, "Narrow the list. ..."),
--       ]),
--       column("po-auto", None, vec![
--           button(AUTO_ANCHOR, "Auto-align"),
--           popover_with_placement(AUTO_BUBBLE, "Auto-align", AUTO_ANCHOR, Edge::Left, Align::Start,
--               vec![wrapped("pop-auto-body", "Declared --left. ...")]),
--       ]),
--       // self.open is true, so the "!self.open" placement-grid branch
--       // (heading + placement_grid()) never runs and is not built.
--   ])])
--
-- where `ANCHOR = "pop-anchor"`, `NOTE = "pop-note"`, `SHUT = "pop-shut"`,
-- `BORDER_ANCHOR = "pop-border-anchor"`, `BORDER_NOTE = "pop-border-note"`,
-- `AUTO_ANCHOR = "pop-auto-anchor"`, `AUTO_BUBBLE = "pop-auto-bubble"`.
return ui.section({
  key = "pop",
  label = "Popover",
  children = {
    common.body("po", "spacing.md", {
      common.column("po-pair", nil, {
        ui.button({ key = "pop-anchor", label = "Filters" }),
        ui.popover_with({
          key = "pop-note",
          label = "Filter fibers",
          anchor = "pop-anchor",
          children = {
            ui.heading({ key = "pop-title", label = "Filter fibers" }),
            common.wrapped(
              "pop-body",
              "A popover is the anchored surface every other overlay in this library is built on. Its contents are the caller's, and they keep their own roles."
            ),
            common.row("pop-actions", "spacing.sm", {
              ui.button({ key = "pop-cancel", label = "Cancel" }),
              ui.primary_button({ key = "pop-apply", label = "Apply" }),
            }),
          },
        }),
      }),
      common.column("po-shut", nil, {
        ui.button({ key = "pop-shut", label = "Closed trigger" }),
      }),
      common.wrapped(
        "po-note",
        "Press either trigger. The panel is 368 wide on surface.raised; a toggletip is 288 on the ramp's last rung."
      ),
      -- Register row 358, `--border`.
      common.column("po-border", nil, {
        ui.button({ key = "pop-border-anchor", label = "Filters" }),
        ui.popover_border({
          key = "pop-border-note",
          label = "Filter fibers",
          anchor = "pop-border-anchor",
          body = "Narrow the list. The border is one token binding, the same channel the --border menu's outline uses.",
        }),
      }),
      -- Register row 361, `--auto-align`: declared `--left` at the page's
      -- left edge, so the flip ladder repositions it.
      common.column("po-auto", nil, {
        ui.button({ key = "pop-auto-anchor", label = "Auto-align" }),
        ui.popover_with_placement({
          key = "pop-auto-bubble",
          label = "Auto-align",
          anchor = "pop-auto-anchor",
          edge = "left",
          align = "start",
          children = {
            common.wrapped(
              "pop-auto-body",
              "Declared --left. There is no room left of a trigger at the page's left edge, so the engine's flip ladder — the --auto-align ladder — moves the bubble to the trigger's right rather than let it paint off-screen."
            ),
          },
        }),
      }),
    }),
  },
})
