-- Lua source for inventory row "Date picker" (spec 013 T009).
--
-- Ported by hand from `page/date_picker.rs`'s `DatePicker::body()`. Both
-- panes are `Pane::new()` (`page/date_picker.rs`'s `Default` impl), whose
-- `open` field is `false`, so `Pane::view` (called with `full = false` for
-- the compact pane and `full = true` for the full pane) takes its early
-- return: `date_picker(key, label, self.value())` -- the calendar itself
-- never mounts. `self.value()` at the default `picked = (2026, 8, 30)` is
-- `"2026-08-30"`.
return ui.section({
  key = "date",
  label = "Date picker",
  children = {
    common.body("dp", "spacing.md", {
      common.row("dp-forms", "spacing.lg", {
        common.column("compact-form", "spacing.sm", {
          ui.heading({ key = "compact-title", label = "Compact" }),
          ui.date_picker({ key = "compact", label = "Date", value = "2026-08-30" }),
        }),
        common.column("full-form", "spacing.sm", {
          ui.heading({ key = "full-title", label = "Full" }),
          ui.date_picker({ key = "full", label = "Date", value = "2026-08-30" }),
        }),
      }),
    }),
  },
})
