-- Lua source for inventory row "Search" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/search.rs`'s `Search::body()`, at the page's
-- default state (`query = String::new()`, so `""`):
--
--   section("search", "Search", vec![filled_body(
--       "q", sp("spacing.md"),
--       vec![valued(search(Q, "Filter fibers"), self.query.clone())],
--   )])
--
-- `Q` is `"query"`. `ui.valued`'s own key (`"q-val"` here) never reaches the
-- built tree: `registry::expand` calls `valued(node, value)`, which returns
-- `node` with its own key intact (`field.rs::valued`), so the wrapper's key
-- argument is discardable scaffolding, the same as `slider.lua`'s `"vol"`.
return ui.section({
  key = "search",
  label = "Search",
  children = {
    common.filled_body("q", "spacing.md", {
      ui.valued("q-val", {
        node = ui.search({ key = "query", label = "Filter fibers" }),
        value = "",
      }),
    }),
  },
})
