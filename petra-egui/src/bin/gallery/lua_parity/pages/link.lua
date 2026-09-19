-- Lua source for inventory row "Link" (spec 013 T009).
--
-- Ported by hand from `page/link.rs`'s `Link::body()`. This page holds no
-- state.
--
--   section("links", "Link", vec![body(
--       "link-row", sp("spacing.md"),
--       vec![
--           link("docs", "Open the spec"),
--           row("prose", None, vec![
--               text("before", "Read "),
--               link_inline("docs-inline", "the inline form"),
--               text("after", " when a link sits in running text."),
--           ]),
--       ],
--   )])
return ui.section({
  key = "links",
  label = "Link",
  children = {
    common.body("link-row", "spacing.md", {
      ui.link({ key = "docs", label = "Open the spec" }),
      common.row("prose", nil, {
        ui.text({ key = "before", label = "Read " }),
        ui.link_inline({ key = "docs-inline", label = "the inline form" }),
        ui.text({ key = "after", label = " when a link sits in running text." }),
      }),
    }),
  },
})
