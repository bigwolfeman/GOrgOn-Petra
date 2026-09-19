-- Lua source for inventory row "Tile" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/tile.rs`'s `Tile::body()`, at the page's
-- default state (`tile_sel = false`, `tile_exp = false`):
--
--   section("kinds", "Base, clickable, selectable, expandable", vec![body(
--       "tiles", sp("spacing.md"),
--       vec![
--           tile("tile-base", "A static tile holds related content."),
--           clickable_tile("tile-click", "Open workspace", "Clickable tile — one target."),
--           selectable_tile(TILE_SEL, "Select this option", self.tile_sel),
--           expandable_tile(TILE_EXP, "More detail", self.tile_exp, "Below-the-fold body."),
--       ],
--   )])
return ui.section({
  key = "kinds",
  label = "Base, clickable, selectable, expandable",
  children = {
    common.body("tiles", "spacing.md", {
      ui.tile({ key = "tile-base", label = "A static tile holds related content." }),
      ui.clickable_tile({
        key = "tile-click",
        label = "Open workspace",
        value = "Clickable tile — one target.",
      }),
      ui.selectable_tile({ key = "tile-sel", label = "Select this option", selected = false }),
      ui.expandable_tile({
        key = "tile-exp",
        label = "More detail",
        expanded = false,
        body = "Below-the-fold body.",
      }),
    }),
  },
})
