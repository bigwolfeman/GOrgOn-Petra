-- Lua source for inventory row "Radio button" (spec 013 T009).
--
-- Ported by hand from `page/radio_button.rs`'s `RadioButton::body()`, at
-- the page's default state (`radio = 0`, `#[derive(Default)]`, so
-- `radio-a` reads selected and `radio-b` does not):
--
--   section("group", "Group", vec![body(
--       "radios", sp("spacing.md"),
--       vec![
--           radio_group("radio-group", "Theme", vec![
--               radio("radio-a", "Dark", true),
--               radio("radio-b", "Light", false),
--               disabled(radio("radio-disabled", "Disabled", false)),
--               disabled(radio("radio-disabled-on", "Disabled, on", true)),
--           ]),
--           radio_warning("radio-warn", "Unusual choice", true, "confirm this"),
--       ],
--   )])
return ui.section({
  key = "group",
  label = "Group",
  children = {
    common.body("radios", "spacing.md", {
      ui.radio_group({
        key = "radio-group",
        label = "Theme",
        children = {
          ui.radio({ key = "radio-a", label = "Dark", selected = true }),
          ui.radio({ key = "radio-b", label = "Light", selected = false }),
          ui.disabled("radio-disabled", {
            node = ui.radio({ key = "radio-disabled", label = "Disabled", selected = false }),
          }),
          ui.disabled("radio-disabled-on", {
            node = ui.radio({ key = "radio-disabled-on", label = "Disabled, on", selected = true }),
          }),
        },
      }),
      ui.radio_warning({
        key = "radio-warn",
        label = "Unusual choice",
        selected = true,
        message = "confirm this",
      }),
    }),
  },
})
