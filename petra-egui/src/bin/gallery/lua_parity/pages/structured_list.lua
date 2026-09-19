-- Lua source for inventory row "Structured list" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/structured_list.rs`'s `StructuredList::body()`,
-- at the page's default state (`selected = 1`, `weights = [1.0, 1.0]`):
--
--   let rows = ROWS.iter().enumerate().map(|(i, (key, name, role))| {
--       structured_list_row(*key, vec![text("name", *name), text("role", *role)], i == self.selected)
--   }).collect();
--   section("table", "Structured list", vec![body(
--       "sl", sp("spacing.md"),
--       vec![
--           wrapped("note", "This is also the Table row. ..."),
--           structured_list_sized("sl", vec![text("h0", "Name"), text("h1", "Role")], rows, &self.weights, true),
--       ],
--   )])
--
-- `ROWS` is `[("sl-0", "kernel", "runtime"), ("sl-1", "petra", "layout")]`,
-- so row 0 (`i == 0`) reads unselected against `selected == 1` and row 1
-- reads selected.
return ui.section({
  key = "table",
  label = "Structured list",
  children = {
    common.body("sl", "spacing.md", {
      common.wrapped(
        "note",
        "This is also the Table row. Carbon Table is this structured list; there is no second component."
      ),
      ui.structured_list_sized({
        key = "sl",
        header = {
          ui.text({ key = "h0", label = "Name" }),
          ui.text({ key = "h1", label = "Role" }),
        },
        rows = {
          ui.structured_list_row({
            key = "sl-0",
            children = {
              ui.text({ key = "name", label = "kernel" }),
              ui.text({ key = "role", label = "runtime" }),
            },
            selected = false,
          }),
          ui.structured_list_row({
            key = "sl-1",
            children = {
              ui.text({ key = "name", label = "petra" }),
              ui.text({ key = "role", label = "layout" }),
            },
            selected = true,
          }),
        },
        weights = { 1.0, 1.0 },
        dividers = true,
      }),
    }),
  },
})
