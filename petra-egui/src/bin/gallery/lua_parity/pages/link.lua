-- Lua source for inventory row "Link" (spec 013 T009).
--
-- Ported by hand from `page/link.rs`'s `Link::body()`. This page holds no
-- state.
--
--   section("links", "Link", vec![body(
--       "link-row", sp("spacing.md"),
--       vec![
--           row("forms", sp("spacing.md"), vec![
--               link("docs", "Open the spec"),
--               link_with_icon("docs-icon", "Open the spec", IconMark::ChevronRight),
--           ]),
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
      common.row("forms", "spacing.md", {
        ui.link({ key = "docs", label = "Open the spec" }),
        -- Register row #231, Carbon's `--icon` modifier.
        ui.link_with_icon({ key = "docs-icon", label = "Open the spec", mark = "chevron-right" }),
      }),
      common.row("prose", nil, {
        ui.text({ key = "before", label = "Read " }),
        ui.link_inline({ key = "docs-inline", label = "the inline form" }),
        ui.text({ key = "after", label = " when a link sits in running text." }),
      }),
    }),
  },
})
