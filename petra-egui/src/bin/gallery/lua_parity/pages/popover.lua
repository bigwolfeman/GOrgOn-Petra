-- Lua source for inventory row "Popover" (spec 013 T009).
--
-- Ported by hand from `page/popover.rs`'s `Popover::body()`, at the page's
-- default state (`open = true`, from `impl Default for Popover`). At
-- `open = true` the `if !self.open { ... placement_grid() ... }` branch
-- never runs, so this page's ninety-odd lines of raw grid/cell construction
-- (`placement_pair`, `placement_grid`, `lead_in`, `cell_box`, `trigger_seat`)
-- are dead code at this state and are not ported here — only the resting
-- panel is:
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
--       // self.open is true, so the "!self.open" placement-grid branch
--       // (heading + placement_grid()) never runs and is not built.
--   ])])
--
-- where `ANCHOR = "pop-anchor"`, `NOTE = "pop-note"`, `SHUT = "pop-shut"`.
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
    }),
  },
})
