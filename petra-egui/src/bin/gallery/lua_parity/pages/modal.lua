-- Lua source for inventory row "Modal" (spec 013 T009).
--
-- Ported by hand from `page/modal.rs`'s `Modal::body()`, at the page's
-- default state (`open = false`, so the `if self.open { children.push(...) }`
-- branch never runs and the dialog itself never appears in the tree):
--
--   let mut children = vec![button(TRIGGER, "Rebuild fiber")];
--   // self.open is false, so nothing more is pushed.
--   section("dialog", "Modal", vec![body(
--       "md-body", sp("spacing.md"),
--       vec![column("md-col", sp("spacing.md"), children)],
--   )])
return ui.section({
  key = "dialog",
  label = "Modal",
  children = {
    common.body("md-body", "spacing.md", {
      common.column("md-col", "spacing.md", {
        ui.button({ key = "open-modal", label = "Rebuild fiber" }),
      }),
    }),
  },
})
