-- Lua source for inventory row "Select" (spec 013 T009).
--
-- Ported by hand from `page/select.rs`'s `Select::body()`, at the page's
-- default state (`open = false`, `selected = 0`, `#[derive(Default)]`, so
-- `value = OPTIONS[0].1 = "Dark"` and the closed field is built):
--
--   let field = select("theme", "Theme", "Dark");
--   section("select", "Select", vec![filled_body(
--       "sel", sp("spacing.md"), vec![field],
--   )])
return ui.section({
  key = "select",
  label = "Select",
  children = {
    common.filled_body("sel", "spacing.md", {
      ui.select({ key = "theme", label = "Theme", value = "Dark" }),
    }),
  },
})
