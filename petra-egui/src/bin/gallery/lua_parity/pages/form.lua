-- Lua source for inventory row "Form" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/form.rs`'s `Form::body()`, at the page's
-- default state (`name = String::new()`, `enabled = true`):
--
--   section("form", "Form", vec![filled_body(
--       "form-body", sp("spacing.md"),
--       vec![form("demo-form", "Fiber", vec![
--           labeled("name-item", "Name",
--               valued(hinted(field(NAME, "Name"), PLACEHOLDER), self.name.clone())),
--           checkbox(ENABLED, "Enabled", self.enabled),
--       ])],
--   )])
--
-- `NAME` is `"form-name"`, `PLACEHOLDER` is `"fiber-7"`, `ENABLED` is
-- `"form-ok"`. `hinted` and `valued` are both `(key, t)` modifiers over an
-- already-built node (their param shapes carry no `key`), nested here the
-- same order the Rust call nests them: `hinted` wraps `field` first, then
-- `valued` wraps that.
return ui.section({
  key = "form",
  label = "Form",
  children = {
    common.filled_body("form-body", "spacing.md", {
      ui.form({
        key = "demo-form",
        label = "Fiber",
        children = {
          ui.labeled({
            key = "name-item",
            label = "Name",
            control = ui.valued("name-val", {
              node = ui.hinted("name-hint", {
                node = ui.field({ key = "form-name", label = "Name" }),
                hint = "fiber-7",
              }),
              value = "",
            }),
          }),
          ui.checkbox({ key = "form-ok", label = "Enabled", selected = true }),
        },
      }),
    }),
  },
})
