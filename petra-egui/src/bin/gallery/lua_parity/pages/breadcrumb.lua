-- Lua source for inventory row "Breadcrumb" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/breadcrumb.rs`'s `Breadcrumb::body()`, at the
-- page's default state (`here = PATH.len() - 1 = 3`, so the whole trail is
-- shown and the last crumb, "Log", is current; `here + 1 == PATH.len()`, so
-- the page's own "Open the page below" control is disabled):
--
--   let crumbs = (0..=3).map(|level| if level == 3 {
--       breadcrumb_item_current(KEYS[level], PATH[level])
--   } else {
--       breadcrumb_item(KEYS[level], PATH[level])
--   }).collect();
--   let deeper = disabled(button(DEEPER, "Open the page below"));
--   section("trail", "Trail", vec![body(
--       "crumbs", sp("spacing.md"),
--       vec![
--           wrapped("note", "A breadcrumb is the path to the page you are on. ..."),
--           breadcrumb("crumbs", crumbs),
--           row("walk", sp("spacing.md"), vec![deeper]),
--       ],
--   )])
--
-- `PATH` is `["Workspace", "Fibers", "Rebuild", "Log"]`, `KEYS` is
-- `["bc-0", "bc-1", "bc-2", "bc-3"]`. `ui.disabled` is a `(key, t)` modifier
-- (`NodeOnly`, no `key` field of its own).
return ui.section({
  key = "trail",
  label = "Trail",
  children = {
    common.body("crumbs", "spacing.md", {
      common.wrapped(
        "note",
        "A breadcrumb is the path to the page you are on. Every crumb before the last one is a link back up the tree: link ink, underlined under the pointer, and in tab order. The last crumb is where you are standing, so it is page ink, it is not a link, and the keyboard walks past it."
      ),
      ui.breadcrumb({
        key = "crumbs",
        children = {
          ui.breadcrumb_item({ key = "bc-0", label = "Workspace" }),
          ui.breadcrumb_item({ key = "bc-1", label = "Fibers" }),
          ui.breadcrumb_item({ key = "bc-2", label = "Rebuild" }),
          ui.breadcrumb_item_current({ key = "bc-3", label = "Log" }),
        },
      }),
      common.row("walk", "spacing.md", {
        ui.disabled("deeper-disabled", {
          node = ui.button({ key = "bc-deeper", label = "Open the page below" }),
        }),
      }),
    }),
  },
})
