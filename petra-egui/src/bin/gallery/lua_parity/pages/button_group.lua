-- Lua source for inventory row "Button group" (spec 013 T009).
--
-- Ported by hand from `page/button_group.rs`'s `ButtonGroup::body()`. The
-- page returns a bare `column(...)`, not a `section`: two sections live
-- inside it instead of around it. The page holds no live state.
return common.column("group", "spacing.lg", {
  ui.section({
    key = "spaced",
    label = "Spaced group",
    children = {
      common.body("btns", "spacing.md", {
        ui.button_group({
          key = "bg",
          children = {
            ui.primary_button({ key = "bg-primary", label = "Save" }),
            ui.button({ key = "bg-default", label = "Discard" }),
            ui.tertiary_button({ key = "bg-tertiary", label = "More" }),
            ui.ghost_button({ key = "bg-ghost", label = "Cancel" }),
          },
        }),
      }),
    },
  }),
  ui.section({
    key = "flush",
    label = "Flush group",
    children = {
      common.body("flush-btns", "spacing.md", {
        ui.button_group_flush({
          key = "bg-flush",
          height = 40.0,
          children = {
            ui.button({ key = "flush-first", label = "Left" }),
            ui.button({ key = "flush-mid", label = "Middle" }),
            ui.button({ key = "flush-last", label = "Right" }),
          },
        }),
      }),
    },
  }),
})
