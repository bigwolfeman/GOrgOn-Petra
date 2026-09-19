-- Lua source for inventory row "Loading" (spec 013 T009).
--
-- Ported by hand from `page/loading.rs`'s `Loading::body()`, at the page's
-- default state (`now = 0.0`, `#[derive(Default)]` on an f64 field):
--
--   section("spinner", "Loading", vec![body(
--       "load", sp("spacing.md"),
--       vec![row(
--           "sizes", sp("spacing-05"),
--           vec![
--               loading("load-lg", "Working", self.now),
--               loading_sm("load-sm", "Working", self.now),
--           ],
--       )],
--   )])
return ui.section({
  key = "spinner",
  label = "Loading",
  children = {
    common.body("load", "spacing.md", {
      common.row("sizes", "spacing-05", {
        ui.loading({ key = "load-lg", label = "Working", value = 0 }),
        ui.loading_sm({ key = "load-sm", label = "Working", value = 0 }),
      }),
    }),
  },
})
