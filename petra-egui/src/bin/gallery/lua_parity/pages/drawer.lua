-- Lua source for inventory row "Drawer" (spec 013 T009).
--
-- Ported by hand from `page/drawer.rs`'s `Drawer::body()`, at the page's
-- default state (`open = false`, so the panel starts shut and the first
-- frame shows only the trigger).
--
-- Second page proven, and chosen for what it adds over `slider.lua`: it is
-- the first page through `common.wrapped`, and `wrapped` is the one mirror
-- in `common.lua` that had to collapse two Rust steps into one Lua
-- primitive. See that function's comment for why. It also nests a whole
-- subtree inside a component's own params (`docked`'s `body` field), which
-- `registry::expand_params` has to walk before `docked` itself runs.
return ui.section({
  key = "panel",
  label = "Drawer",
  children = {
    common.body("dr-body", "spacing.md", {
      common.column("dr-col", "spacing.md", {
        ui.button({ key = "open-drawer", label = "Open drawer" }),
        common.wrapped(
          "drawer-note",
          "The panel docks to the right edge. The catalog still takes clicks: this page uses Passthrough, not Block."
        ),
        ui.docked({
          key = "drawer",
          edge = "right",
          open = false,
          policy = "passthrough",
          body = common.column("drawer-body", "spacing.md", {
            ui.text({ key = "drawer-title", label = "Session" }),
            ui.text({ key = "drawer-copy", label = "The panel docks inside the window edge." }),
            ui.button({ key = "close-drawer", label = "Close" }),
          }),
        }),
      }),
    }),
  },
})
