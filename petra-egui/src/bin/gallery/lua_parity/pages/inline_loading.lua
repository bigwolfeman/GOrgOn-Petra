-- Lua source for inventory row "Inline loading" (spec 013 T009).
--
-- Ported by hand from `page/inline_loading.rs`'s `InlineLoading::body()`,
-- at the page's default state (`now = 0.0`, `#[derive(Default)]` on an f64
-- field):
--
--   section("inline", "Inline loading", vec![body(
--       "il", sp("spacing.md"),
--       vec![
--           inline_loading("il-on", "Saving", self.now),
--           inline_loading_finished("il-off", "Saved"),
--       ],
--   )])
return ui.section({
  key = "inline",
  label = "Inline loading",
  children = {
    common.body("il", "spacing.md", {
      ui.inline_loading({ key = "il-on", label = "Saving", value = 0 }),
      ui.inline_loading_finished({ key = "il-off", label = "Saved" }),
    }),
  },
})
