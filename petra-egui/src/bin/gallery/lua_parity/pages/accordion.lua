-- Lua source for inventory row "Accordion" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/accordion.rs`'s `Accordion::body()`, at the
-- page's default state (`open = [true, false]`, `nest = true`,
-- `inner = [true, false]`):
--
--   section("items", "Items", vec![filled_body(
--       "accordion", sp("spacing.md"),
--       vec![accordion_spaced("acc", vec![
--           accordion_item_spaced(ACC_0, "First section", true, "The fibers scheduled this pass."),
--           accordion_item_spaced(ACC_1, "Second section", false, "The trace written this session."),
--           accordion_item_with_spaced(ACC_NEST, "Nested section", true, vec![
--               accordion_spaced("inner", vec![
--                   accordion_item_spaced(ACC_NEST_0, "Nested first", true, "One level in. ..."),
--                   accordion_item_spaced(ACC_NEST_1, "Nested second", false, "Each inner header toggles on its own."),
--               ]),
--           ]),
--       ])],
--   )])
--
-- `accordion_item_with_spaced` takes `children` (a subtree), not `body` (a
-- string) — `registry::containment::KeyLabelExpandedChildren` — unlike its
-- five `accordion_item*` siblings, which all take `body`.
return ui.section({
  key = "items",
  label = "Items",
  children = {
    common.filled_body("accordion", "spacing.md", {
      ui.accordion_spaced({
        key = "acc",
        children = {
          ui.accordion_item_spaced({
            key = "acc-0",
            label = "First section",
            expanded = true,
            body = "The fibers scheduled this pass.",
          }),
          ui.accordion_item_spaced({
            key = "acc-1",
            label = "Second section",
            expanded = false,
            body = "The trace written this session.",
          }),
          ui.accordion_item_with_spaced({
            key = "acc-nest",
            label = "Nested section",
            expanded = true,
            children = {
              ui.accordion_spaced({
                key = "inner",
                children = {
                  ui.accordion_item_spaced({
                    key = "acc-nest-0",
                    label = "Nested first",
                    expanded = true,
                    body = "One level in. The panel's own 16 inline padding is the indent.",
                  }),
                  ui.accordion_item_spaced({
                    key = "acc-nest-1",
                    label = "Nested second",
                    expanded = false,
                    body = "Each inner header toggles on its own.",
                  }),
                },
              }),
            },
          }),
        },
      }),
    }),
  },
})
