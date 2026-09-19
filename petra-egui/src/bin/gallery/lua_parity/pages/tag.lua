-- Lua source for inventory row "Tag" (spec 013 T009).
--
-- Ported by hand from `page/tag.rs`'s `Tag::body()`, at the page's default
-- state (`tag_sel = false`):
--
--   section("tags", "Tags", vec![body(
--       "tag-row", sp("spacing.md"),
--       vec![
--           tag("tag-ro", "Read only"),
--           dismissible_tag("tag-x", "Filter"),
--           selectable_tag("tag-sel-off", "Unselected", false),
--           selectable_tag("tag-sel-on", "Selected", true),
--           selectable_tag(TAG_SEL, "Selectable", self.tag_sel),
--       ],
--   )])
--
-- where `TAG_SEL = "tag-sel"`.
return ui.section({
  key = "tags",
  label = "Tags",
  children = {
    common.body("tag-row", "spacing.md", {
      ui.tag({ key = "tag-ro", label = "Read only" }),
      ui.dismissible_tag({ key = "tag-x", label = "Filter" }),
      ui.selectable_tag({ key = "tag-sel-off", label = "Unselected", selected = false }),
      ui.selectable_tag({ key = "tag-sel-on", label = "Selected", selected = true }),
      ui.selectable_tag({ key = "tag-sel", label = "Selectable", selected = false }),
    }),
  },
})
