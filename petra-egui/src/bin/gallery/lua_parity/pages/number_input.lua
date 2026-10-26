-- Lua source for inventory row "Number input" (spec 013 T009).
--
-- Ported by hand from `page/number_input.rs`'s `NumberInput::body()`, at the
-- page's default state (`value = "12"`, from `impl Default for NumberInput`):
--
--   section("number", "Number input", vec![filled_body(
--       "num", sp("spacing.md"),
--       vec![
--           labeled("count", "Count", number_input(COUNT, "Count", self.value.clone())),
--       ],
--   )])
--
-- where `COUNT = "n-md"`.
return ui.section({
  key = "number",
  label = "Number input",
  children = {
    common.filled_body("num", "spacing.md", {
      ui.labeled({
        key = "count",
        label = "Count",
        control = ui.number_input({ key = "n-md", label = "Count", value = "12" }),
      }),
    }),
  },
})
