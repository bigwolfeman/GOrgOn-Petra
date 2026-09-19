-- Lua source for inventory row "List" (spec 013 T009).
--
-- Ported by hand from `page/list.rs`'s `List::body()`. The page holds no
-- live state. `chain` is `page/list.rs`'s own private helper (a four-level
-- nested `unordered_list_with`/`unordered_list`/`list_item_with`/
-- `list_item` chain), inlined here as a local Lua function rather than a
-- `common.lua` addition -- it is page-private in Rust too.
--
-- Wire form of `BulletScheme` (`registry/containment.rs`'s
-- `BulletSchemeParam`, internally tagged on `type`): `{ type = "carbon" }`,
-- `{ type = "rotating" }`, `{ type = "fixed", bullet = "square" }`.
local function chain(prefix, scheme, labels)
  return ui.unordered_list_with({
    key = prefix,
    scheme = scheme,
    children = {
      ui.list_item_with({
        key = prefix .. "-1",
        label = labels[1],
        nested = ui.unordered_list({
          key = prefix .. "-l2",
          children = {
            ui.list_item_with({
              key = prefix .. "-2",
              label = labels[2],
              nested = ui.unordered_list({
                key = prefix .. "-l3",
                children = {
                  ui.list_item_with({
                    key = prefix .. "-3",
                    label = labels[3],
                    nested = ui.unordered_list({
                      key = prefix .. "-l4",
                      children = {
                        ui.list_item({ key = prefix .. "-4", label = labels[4] }),
                      },
                    }),
                  }),
                },
              }),
            }),
          },
        }),
      }),
    },
  })
end

return ui.section({
  key = "kinds",
  label = "Ordered, unordered, nested",
  children = {
    common.body("lists", "spacing.md", {
      chain("ul", { type = "carbon" }, { "Inbox", "Archive", "2026", "March" }),
      chain("rot", { type = "rotating" }, { "Disc", "Ring", "Square", "Dash" }),
      ui.unordered_list_with({
        key = "fix",
        scheme = { type = "fixed", bullet = "square" },
        children = {
          ui.list_item({ key = "fix-0", label = "Fixed square" }),
          ui.list_item({ key = "fix-1", label = "at every level" }),
        },
      }),
      ui.ordered_list({
        key = "ol",
        children = {
          ui.list_item({ key = "ol-0", label = "Clone" }),
          ui.list_item_with({
            key = "ol-1",
            label = "Build",
            nested = ui.ordered_list({
              key = "ol-l2",
              children = {
                ui.list_item({ key = "ol-1-0", label = "Compile" }),
                ui.list_item_with({
                  key = "ol-1-1",
                  label = "Link",
                  nested = ui.ordered_list({
                    key = "ol-l3",
                    children = {
                      ui.list_item({ key = "ol-1-1-0", label = "Static" }),
                      ui.list_item({ key = "ol-1-1-1", label = "Dynamic" }),
                    },
                  }),
                }),
              },
            }),
          }),
          ui.list_item({ key = "ol-2", label = "Run" }),
        },
      }),
    }),
  },
})
