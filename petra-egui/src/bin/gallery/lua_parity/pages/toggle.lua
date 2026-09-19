-- Lua source for inventory row "Toggle" (spec 013 T009).
--
-- Ported by hand from `page/toggle.rs`'s `Toggle::body()`, at the page's
-- default state:
--
--   default_off = false, default_on = true, small_off = false, small_on = true
--
-- The two disabled rows wrap an already-built `toggle` node in the
-- `disabled` modifier (`ui.disabled(key, t)`, T007's two-argument
-- exception — its param shape carries no `key` of its own, so the outer
-- key here names only the wire component reference, not anything
-- `registry::expand` reads back out).
return ui.section({
  key = "states",
  label = "States",
  children = {
    common.body("toggles", "spacing.md", {
      ui.toggle({ key = "toggle-default-off", label = "Default off", selected = false }),
      ui.toggle({ key = "toggle-default-on", label = "Default on", selected = true }),
      ui.toggle_sm({ key = "toggle-sm-off", label = "Small off", selected = false }),
      ui.toggle_sm({ key = "toggle-sm-on", label = "Small on", selected = true }),
      ui.disabled("toggle-disabled-off", {
        node = ui.toggle({ key = "toggle-disabled-off", label = "Disabled off", selected = false }),
      }),
      ui.disabled("toggle-disabled-on", {
        node = ui.toggle({ key = "toggle-disabled-on", label = "Disabled on", selected = true }),
      }),
    }),
  },
})
