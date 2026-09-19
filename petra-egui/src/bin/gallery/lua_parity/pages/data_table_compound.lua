-- Lua source for inventory row "Data table (compound)" (spec 014 A4).
--
-- Ported by hand from `page/data_table_compound.rs`'s
-- `DataTableCompoundPage::body()`, at the page's default state: `init`,
-- then `SortBy { column: "name" }`, then `ToggleExpand { id: "f1" }`.
--
-- A registry row never calls `Compound::init`, so the state those two
-- intents land on is named directly: sorted ascending by name, with `f1`
-- expanded. Nothing is selected at rest — the Rust page says why in its own
-- `Default`, and it is a deliberate choice about what the catalog
-- photographs, not an omission to copy carelessly.
--
-- Every state field this omits is `#[serde(default)]` on
-- `data_table::State`. They are omitted rather than written empty because
-- Lua cannot tell an empty list from an empty map, so `{}` on the wire is a
-- map and will not deserialize into a `BTreeSet` or a `Vec`.
--
-- Falsified 2026-09-19 by changing `ascending = true` to `false`, then
-- restored byte-identical and re-run green:
--
--   .../lua_parity/pages/data_table_compound.lua ("Data table (compound)") has
--   drifted from its Rust page's body()
--     at children[1].children[1].children[1].children[0].children[1].children[0]
--        .children[1].props.text
--       lua:  "descending"
--       rust: "ascending"
return ui.section({
  key = "table-compound",
  label = "Data table (compound)",
  children = {
    common.body("dtc-body", "spacing.md", {
      common.wrapped(
        "dtc-note",
        "The same triple as spec 009 T006, driven through `Compound::update`: sort, select, select-all, expand, search, column visibility, the batch bar and the row menu are all live intents rather than flags the page flips directly."
      ),
      ui.data_table_compound("dtc-table", {
        props = {
          columns = {
            { id = "name", label = "Name" },
            { id = "status", label = "Status" },
          },
          rows = {
            { id = "f0", cells = { "scheduler", "running" } },
            {
              id = "f1",
              cells = { "layout", "idle" },
              body = "Retry policy: exponential backoff, 3 tries.",
            },
            { id = "f2", cells = { "trace", "idle" } },
            { id = "f3", cells = { "snapshot", "running" } },
          },
          batch_actions = { { id = "archive", label = "Archive" } },
          row_actions = {
            { id = "rename", label = "Rename" },
            { id = "delete", label = "Delete" },
          },
          loading = false,
        },
        state = {
          sort = { column = "name", ascending = true },
          expansion = { "f1" },
        },
      }),
    }),
  },
})
