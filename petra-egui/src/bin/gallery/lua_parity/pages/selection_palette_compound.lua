-- Lua source for inventory row "Selection palette (compound)" (spec 014 A4).
--
-- Ported by hand from `page/selection_palette_compound.rs`'s
-- `SelectionPaletteCompoundPage::body()`, at the page's default state:
-- `init`, then `Move { point: (320, 420) }`, then `ToggleBold`. A registry
-- row never calls `init`, so the state those calls land on is named
-- directly.
--
-- `props` is an empty table and still has to be written. The compound's
-- `Props` is a unit struct — everything the palette draws from lives in
-- `State` — but the row's contract is "props and state both come from the
-- table", not "props when there happen to be any"
-- (`petra/petra-compound/src/registry.rs`).
--
-- `MOVE_TRIGGER` is the page's own button, outside the compound: a real
-- selection is spec 006, and this stands in for it.
--
-- Falsified 2026-09-19 by changing `bold = true` to `false`, then restored
-- byte-identical and re-run green. A pressed toggle carries an extra child:
--
--   .../lua_parity/pages/selection_palette_compound.lua ("Selection palette
--   (compound)") has drifted from its Rust page's body()
--     at children[1].children[1].children[1].children[0].children[0].children
--       lua:  1 element(s)
--       rust: 2 element(s)
return ui.section({
  key = "palette-compound",
  label = "Selection palette (compound)",
  children = {
    common.body("spc-body", "spacing.md", {
      common.wrapped(
        "spc-note",
        "The same triple as spec 009 T028a. A real selection is spec 006; this trigger stands in for it with `Intent::Move`. Bold and Italic below are the compound's own toggle buttons."
      ),
      common.column("spc-col", "spacing.md", {
        ui.button({ key = "move-selection", label = "Move to another spot" }),
        ui.selection_palette_compound("spc-palette", {
          props = {},
          state = { bold = true, italic = false, point = { x = 320.0, y = 420.0 } },
        }),
      }),
    }),
  },
})
