-- Lua source for inventory row "Toggletip" (spec 013 T009).
--
-- Ported by hand from `page/toggletip.rs`'s `Toggletip::body()`, at the
-- page's default state (`open = true`, from `impl Default for Toggletip`):
--
--   section("tt", "Toggletip", vec![body(
--       "tt-body", sp("spacing.md"),
--       vec![
--           heading("tt-title", "Why is this fiber parked?"),
--           toggletip_with("tt", "Why", self.open, vec![
--               wrapped("tt-note", "It is waiting on a channel no other fiber writes to."),
--               row("tt-actions", sp("spacing.sm"), vec![link("tt-more", "Open the trace")]),
--           ]),
--           text("tt-hint", "Press the mark to open and again to close."),
--       ],
--   )])
return ui.section({
  key = "tt",
  label = "Toggletip",
  children = {
    common.body("tt-body", "spacing.md", {
      ui.heading({ key = "tt-title", label = "Why is this fiber parked?" }),
      ui.toggletip_with({
        key = "tt",
        label = "Why",
        open = true,
        children = {
          common.wrapped(
            "tt-note",
            "It is waiting on a channel no other fiber writes to."
          ),
          common.row("tt-actions", "spacing.sm", {
            ui.link({ key = "tt-more", label = "Open the trace" }),
          }),
        },
      }),
      ui.text({ key = "tt-hint", label = "Press the mark to open and again to close." }),
    }),
  },
})
