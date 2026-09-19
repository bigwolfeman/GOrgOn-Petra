-- Lua source for inventory row "Rating" (spec 013 T009).
--
-- Ported by hand from `page/rating.rs`'s `Rating::body()`, at the page's
-- default state (`value = 3`):
--
--   section("score", "Score", vec![body(
--       "rt-body", sp("spacing.md"),
--       vec![rating("rating", self.value, 5, IconMark::Check)],
--   )])
return ui.section({
  key = "score",
  label = "Score",
  children = {
    common.body("rt-body", "spacing.md", {
      ui.rating({ key = "rating", value = 3, max = 5, mark = "check" }),
    }),
  },
})
