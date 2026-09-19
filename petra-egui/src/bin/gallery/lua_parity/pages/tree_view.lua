-- Lua source for inventory row "Tree view" (spec 013 T009).
--
-- Ported by hand from `page/tree_view.rs`'s `TreeView::body()`. That page
-- builds its tree by recursing `Self::children_of`/`Self::build` over a
-- page-private `NODES` table; there is no registry row for that recursion,
-- so it is expanded here by hand into the literal tree it produces at the
-- default state (`expanded = {"tv-gorgon", "tv-petra"}`, `selected =
-- "tv-component"`):
--
--   tv-gorgon (expanded, not selected)
--     tv-petra (expanded, not selected)
--       tv-component (not expanded -- no children -- selected)
--       tv-layout (not expanded, not selected)
--     tv-petra-egui (not expanded, not selected)
--       tv-gallery (not expanded, not selected)
--     tv-inspector (not expanded, not selected)
--   tv-docs (not expanded, not selected)
--     tv-constitution (not expanded, not selected)
--
-- Every leaf's `children` is simply omitted: `tree_item`'s wire shape
-- default-supplies an absent `children` as empty, and the registry
-- component factory drops an empty Lua table before it ever reaches the
-- wire (`ui/mod.rs`'s `COMPONENT_FACTORY`).
return ui.section({
  key = "tree",
  label = "Tree view",
  children = {
    common.body("tv", "spacing.md", {
      common.wrapped(
        "note",
        "Click a row to select it: the accent bar and the fill move to it. "
          .. "A row with a caret is a branch, and clicking one opens or shuts it as well."
      ),
      ui.tree_view({
        key = "tv",
        children = {
          ui.tree_item({
            key = "tv-gorgon",
            label = "gorgon",
            expanded = true,
            selected = false,
            children = {
              ui.tree_item({
                key = "tv-petra",
                label = "petra",
                expanded = true,
                selected = false,
                children = {
                  ui.tree_item({
                    key = "tv-component",
                    label = "component",
                    expanded = false,
                    selected = true,
                  }),
                  ui.tree_item({
                    key = "tv-layout",
                    label = "layout",
                    expanded = false,
                    selected = false,
                  }),
                },
              }),
              ui.tree_item({
                key = "tv-petra-egui",
                label = "petra-egui",
                expanded = false,
                selected = false,
                children = {
                  ui.tree_item({
                    key = "tv-gallery",
                    label = "gallery",
                    expanded = false,
                    selected = false,
                  }),
                },
              }),
              ui.tree_item({
                key = "tv-inspector",
                label = "inspector.rs",
                expanded = false,
                selected = false,
              }),
            },
          }),
          ui.tree_item({
            key = "tv-docs",
            label = "docs",
            expanded = false,
            selected = false,
            children = {
              ui.tree_item({
                key = "tv-constitution",
                label = "constitution.md",
                expanded = false,
                selected = false,
              }),
            },
          }),
        },
      }),
    }),
  },
})
