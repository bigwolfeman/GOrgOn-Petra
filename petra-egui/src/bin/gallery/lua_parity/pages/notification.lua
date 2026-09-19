-- Lua source for inventory row "Notification" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/notification.rs`'s `Notification::body()`, at
-- the page's default state (`kind = OPENS_ON = 2`, so `showing()` is
-- `KINDS[2]`, the `Info` row: `"nt-info"`, `"Rebuild finished"`,
-- `"12 fibers reloaded."`):
--
--   let inline = KINDS.iter().map(|(kind, key, title, message)| {
--       notification_inline_kind(*key, *kind, *title, *message)
--   }).collect();
--   section("note", "Notification", vec![body(
--       "nt-inline", sp("spacing.md"),
--       vec![
--           body("kinds", sp("spacing.md"), inline),
--           notification_actionable_kind("nt", Info, "Rebuild finished", "12 fibers reloaded.", "Next kind"),
--       ],
--   )])
--
-- `NotificationKindParams` and `NotificationActionableKindParams` both name
-- their fourth field `body`, not `message` — `page/notification.rs`'s own
-- tuple field is `message`, but the wire shape (`registry/feedback.rs`)
-- calls it `body`, so the Lua key here is `body` for all five nodes.
return ui.section({
  key = "note",
  label = "Notification",
  children = {
    common.body("nt-inline", "spacing.md", {
      common.body("kinds", "spacing.md", {
        ui.notification_inline_kind({
          key = "nt-error",
          kind = "error",
          title = "Rebuild failed",
          body = "Fiber 7 did not come back.",
        }),
        ui.notification_inline_kind({
          key = "nt-warning",
          kind = "warning",
          title = "Disk filling",
          body = "Trace volume is at 80%.",
        }),
        ui.notification_inline_kind({
          key = "nt-info",
          kind = "info",
          title = "Rebuild finished",
          body = "12 fibers reloaded.",
        }),
        ui.notification_inline_kind({
          key = "nt-success",
          kind = "success",
          title = "Supervisor restarted",
          body = "Worker 3 came back.",
        }),
      }),
      ui.notification_actionable_kind({
        key = "nt",
        kind = "info",
        title = "Rebuild finished",
        body = "12 fibers reloaded.",
        action = "Next kind",
      }),
    }),
  },
})
