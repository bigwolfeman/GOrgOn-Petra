-- Lua source for inventory row "Number input" (spec 013 T009).
--
-- Ported by hand from `page/number_input.rs`'s `NumberInput::body()`, at the
-- page's default state (`value = "12"`, from `impl Default for NumberInput`):
--
--   section("number", "Number input", vec![filled_body(
--       "num", sp("spacing.md"),
--       vec![
--           labeled("count", "Count", number_input(COUNT, "Count", self.value.clone())),
--           labeled("warn-count", "Count (warning)",
--               number_input_warning("n-warn", "Count", "999", "out of range")),
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
      ui.labeled({
        key = "warn-count",
        label = "Count (warning)",
        control = ui.number_input_warning({
          key = "n-warn",
          label = "Count",
          value = "999",
          message = "out of range",
        }),
      }),
    }),
  },
})
