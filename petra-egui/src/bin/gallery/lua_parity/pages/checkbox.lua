-- Lua source for inventory row "Checkbox" (spec 013 T009).
--
-- Ported by hand from `page/checkbox.rs`'s `Checkbox::body()`, at the
-- page's default state (`impl Default`: `check_a = true`, `check_b = false`,
-- `mixed = CheckState::Mixed`):
--
--   section("states", "States", vec![body(
--       "checks", sp("spacing.md"),
--       vec![checkbox_group("check-group", "Notifications", vec![
--           checkbox("check-a", "Email", true),
--           checkbox("check-b", "Push", false),
--           checkbox_tristate("check-mixed", "Mixed", CheckState::Mixed),
--           checkbox_readonly("check-ro", "Read only", true),
--           disabled(checkbox("check-disabled", "Disabled", false)),
--           disabled(checkbox("check-disabled-on", "Disabled, on", true)),
--           checkbox_warning("check-warn", "Required consent", false, "must be checked"),
--       ])],
--   )])
return ui.section({
  key = "states",
  label = "States",
  children = {
    common.body("checks", "spacing.md", {
      ui.checkbox_group({
        key = "check-group",
        label = "Notifications",
        children = {
          ui.checkbox({ key = "check-a", label = "Email", selected = true }),
          ui.checkbox({ key = "check-b", label = "Push", selected = false }),
          ui.checkbox_tristate({ key = "check-mixed", label = "Mixed", state = "mixed" }),
          ui.checkbox_readonly({ key = "check-ro", label = "Read only", selected = true }),
          ui.disabled("check-disabled", {
            node = ui.checkbox({ key = "check-disabled", label = "Disabled", selected = false }),
          }),
          ui.disabled("check-disabled-on", {
            node = ui.checkbox({ key = "check-disabled-on", label = "Disabled, on", selected = true }),
          }),
          ui.checkbox_warning({
            key = "check-warn",
            label = "Required consent",
            selected = false,
            message = "must be checked",
          }),
        },
      }),
    }),
  },
})
