-- Lua source for inventory row "Data table" (spec 013 T009).
--
-- Ported by hand from `page/data_table.rs`'s `DataTable::body()`, at the
-- page's default state (`impl Default`):
--   selected = [false, true, false, false, false, false]  -- N_SELECTED = 6
--   kinds = ["runtime", "layout", "plugin host", "editor"]
--   name_ascending = true
--   expanded = true
--   weights = [1.0, 1.0]
--   order = [0, 1, 2, 3, 4, 5]  -- identity, so `build_row` runs in order
--
-- `build_row(i)` for i in order:
--   0: data_table_row_lg("dt-0", [text("name","kernel"), valued(field_sm("dt-kind-0","Kind"), "runtime")], false)
--   1: data_table_row_sm("dt-1", [text("name","petra"), valued(field_sm("dt-kind-1","Kind"), "layout")], true)
--   2: data_table_row_md("dt-2", [text("name","luau"), valued(field_sm("dt-kind-2","Kind"), "plugin host")], false)
--   3: disabled(data_table_row_xs("dt-3", [text("name","helix"), valued(field_sm("dt-kind-3","Kind"), "editor")], false))
--   4 (XL_INDEX): data_table_row_xl("dt-4", [text("name","gorgond"), text("kind","daemon")], false)
--   5 (EXP_INDEX): data_table_row_expandable("dt-exp", [text("name","supervisor"), text("kind","retry")], false, true, EXP_BODY)
--
-- Then:
--   section("table", "Data table", vec![body("dt", sp("spacing.md"), vec![
--       wrapped("note", <drag note>),
--       data_table_zebra_sized("dt",
--           [data_table_sort_header("h0", "Name", Ascending), text("h1", "Kind")],
--           rows, &weights, true, true),
--   ])])
return ui.section({
  key = "table",
  label = "Data table",
  children = {
    common.body("dt", "spacing.md", {
      common.wrapped(
        "note",
        "Drag a column separator to resize. Drag the grip at the leading edge of a row to reorder. Five row sizes, zebra, a sortable Name header, a disabled row, an expandable row, mixed select-all at rest."
      ),
      ui.data_table_zebra_sized({
        key = "dt",
        header = {
          -- `data_table_sort_header`'s wire bool is `selected`; the ctor
          -- reads it as ascending-when-true (`registry/data.rs`).
          ui.data_table_sort_header({ key = "h0", label = "Name", selected = true }),
          ui.text({ key = "h1", label = "Kind" }),
        },
        rows = {
          ui.data_table_row_lg({
            key = "dt-0",
            selected = false,
            children = {
              ui.text({ key = "name", label = "kernel" }),
              ui.valued("dt-kind-0", {
                node = ui.field_sm({ key = "dt-kind-0", label = "Kind" }),
                value = "runtime",
              }),
            },
          }),
          ui.data_table_row_sm({
            key = "dt-1",
            selected = true,
            children = {
              ui.text({ key = "name", label = "petra" }),
              ui.valued("dt-kind-1", {
                node = ui.field_sm({ key = "dt-kind-1", label = "Kind" }),
                value = "layout",
              }),
            },
          }),
          ui.data_table_row_md({
            key = "dt-2",
            selected = false,
            children = {
              ui.text({ key = "name", label = "luau" }),
              ui.valued("dt-kind-2", {
                node = ui.field_sm({ key = "dt-kind-2", label = "Kind" }),
                value = "plugin host",
              }),
            },
          }),
          ui.disabled("dt-3", {
            node = ui.data_table_row_xs({
              key = "dt-3",
              selected = false,
              children = {
                ui.text({ key = "name", label = "helix" }),
                ui.valued("dt-kind-3", {
                  node = ui.field_sm({ key = "dt-kind-3", label = "Kind" }),
                  value = "editor",
                }),
              },
            }),
          }),
          ui.data_table_row_xl({
            key = "dt-4",
            selected = false,
            children = {
              ui.text({ key = "name", label = "gorgond" }),
              ui.text({ key = "kind", label = "daemon" }),
            },
          }),
          ui.data_table_row_expandable({
            key = "dt-exp",
            selected = false,
            expanded = true,
            body = "Retry policy lives here. Mounted only while open.",
            children = {
              ui.text({ key = "name", label = "supervisor" }),
              ui.text({ key = "kind", label = "retry" }),
            },
          }),
        },
        weights = { 1.0, 1.0 },
        dividers = true,
        reorderable = true,
      }),
    }),
  },
})
