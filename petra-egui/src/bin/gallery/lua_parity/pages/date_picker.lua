-- Lua source for inventory row "Date picker" (spec 013 T009).
--
-- Ported by hand from `page/date_picker.rs`'s `DatePicker::body()`. Both
-- panes are `Pane::new()` (`page/date_picker.rs`'s `Default` impl), whose
-- `open` field is `false`, so `Pane::view` (called with `full = false` for
-- the compact pane and `full = true` for the full pane) takes its early
-- return: `date_picker(key, label, self.value())` -- the calendar itself
-- never mounts. `self.value()` at the default `picked = (2026, 8, 30)` is
-- `"2026-08-30"`.
--
-- Under the two live panes sit spec 008 T049's static variant cells:
--
--   heading("variants-title", "The documented forms"),
--   row("dp-variants", sp("spacing.lg"), vec![
--       column("v-input", sp("spacing.sm"), vec![
--           text("v-input-title", "Simple input"),
--           date_picker_input("var-input", "Date", "1989-04-12")]),
--       column("v-fluid", sp("spacing.sm"), vec![
--           text("v-fluid-title", "Fluid"),
--           date_picker_fluid("var-fluid", "Date", "2026-08-30")]),
--       column("v-time", sp("spacing.sm"), vec![
--           text("v-time-title", "Time picker"),
--           time_picker("var-time", "Time", "3:45 PM", Some("PM"), None)])]),
--   row("dp-variants-2", sp("spacing.lg"), vec![
--       column("v-range", sp("spacing.sm"), vec![
--           text("v-range-title", "Calendar range"),
--           date_picker_range("var-range", "Start date", "2026-10-01",
--               "End date", "2026-10-25")]),
--       column("v-range-open", sp("spacing.sm"), vec![
--           text("v-range-open-title", "Calendar range, raised"),
--           date_picker_range_showing("var-range-open", "Start date", "2026-10-01",
--               "End date", "2026-10-25",
--               ["2026-10-05", "2026-10-06", "2026-10-07"],
--               Calendar::Compact { year: 2026, month: 10 })]),
--       column("v-invalid", sp("spacing.sm"), vec![
--           text("v-invalid-title", "Invalid"),
--           date_picker_invalid("var-invalid", "Date", "2026-13-45",
--               "date does not exist")])]),
--
-- The warning form is the page's Warning cell, seeded from `warning()` and
-- not part of `body()`.
return ui.section({
  key = "date",
  label = "Date picker",
  children = {
    -- `spacing.sm` with the Rust body: the raised range calendar's 336
    -- units only drop below its fields inside the 900-unit viewport at
    -- this gap (page/date_picker.rs's `body` call, which says why).
    common.body("dp", "spacing.sm", {
      common.row("dp-forms", "spacing.lg", {
        common.column("compact-form", "spacing.sm", {
          ui.heading({ key = "compact-title", label = "Compact" }),
          ui.date_picker({ key = "compact", label = "Date", value = "2026-08-30" }),
        }),
        -- `forms-gap`: the spacer the Rust body places between the two
        -- live panes so open calendars have a column to drop through
        -- (page/date_picker.rs's `body`, which says why).
        ui.node.spacer({ key = "forms-gap" }),
        common.column("full-form", "spacing.sm", {
          ui.heading({ key = "full-title", label = "Full" }),
          ui.date_picker({ key = "full", label = "Date", value = "2026-08-30" }),
        }),
      }),
      ui.heading({ key = "variants-title", label = "The documented forms" }),
      common.row("dp-variants", "spacing.lg", {
        common.column("v-input", "spacing.sm", {
          ui.text({ key = "v-input-title", label = "Simple input" }),
          ui.date_picker_input({ key = "var-input", label = "Date", value = "1989-04-12" }),
        }),
        common.column("v-fluid", "spacing.sm", {
          ui.text({ key = "v-fluid-title", label = "Fluid" }),
          ui.date_picker_fluid({ key = "var-fluid", label = "Date", value = "2026-08-30" }),
        }),
        common.column("v-time", "spacing.sm", {
          ui.text({ key = "v-time-title", label = "Time picker" }),
          ui.time_picker({
            key = "var-time",
            label = "Time",
            value = "3:45 PM",
            clock = "PM",
          }),
        }),
      }),
      common.row("dp-variants-2", "spacing.lg", {
        common.column("v-range", "spacing.sm", {
          ui.text({ key = "v-range-title", label = "Calendar range" }),
          ui.date_picker_range({
            key = "var-range",
            start_label = "Start date",
            start_date = "2026-10-01",
            end_label = "End date",
            end_date = "2026-10-25",
          }),
        }),
        common.column("v-range-open", "spacing.sm", {
          ui.text({ key = "v-range-open-title", label = "Calendar range, raised" }),
          ui.date_picker_range_showing({
            key = "var-range-open",
            start_label = "Start date",
            start_date = "2026-10-01",
            end_label = "End date",
            end_date = "2026-10-25",
            selected = { "2026-10-05", "2026-10-06", "2026-10-07" },
            calendar = { type = "compact", year = 2026, month = 10 },
          }),
        }),
        common.column("v-invalid", "spacing.sm", {
          ui.text({ key = "v-invalid-title", label = "Invalid" }),
          ui.date_picker_invalid({
            key = "var-invalid",
            label = "Date",
            value = "2026-13-45",
            message = "date does not exist",
          }),
        }),
      }),
    }),
  },
})
