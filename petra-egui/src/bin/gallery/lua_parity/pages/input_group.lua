-- Lua source for inventory row "Input group" (spec 013 T009).
--
-- Ported by hand from `page/input_group.rs`'s `InputGroup::body()`, at the
-- page's default state (`impl Default`: `host = "kernel.local"`,
-- `port = "9p"`).
--
--   column("group", sp("spacing.lg"), vec![
--       section("spaced", "Spaced addons", vec![filled_body(
--           "ig-body", sp("spacing.md"),
--           vec![
--               labeled("host-item", "Host",
--                   input_group_with_addon("ig-addon", button("ig-prefix", "9p://"),
--                       valued(field("ig-host", "Host"), "kernel.local"))),
--               labeled("pair-item", "Host and port",
--                   input_group("ig-pair", vec![
--                       valued(field("ig-host-pair", "Host"), "kernel.local"),
--                       valued(field("ig-port", "Port"), "9p"),
--                       button("ig-go", "Connect"),
--                   ])),
--           ],
--       )]),
--       section("seamless", "Seamless addons", vec![filled_body(
--           "ig-body-seamless", sp("spacing.md"),
--           vec![
--               labeled("host-item-seamless", "Host",
--                   input_group_with_addon_seamless("ig-addon-seamless",
--                       button("ig-prefix-seamless", "9p://"),
--                       valued(field("ig-host-seamless", "Host"), "kernel.local"))),
--               labeled("pair-item-seamless", "Host and port",
--                   input_group_seamless("ig-pair-seamless", vec![
--                       valued(field("ig-host-pair-seamless", "Host"), "kernel.local"),
--                       valued(field("ig-port-seamless", "Port"), "9p"),
--                       button("ig-go-seamless", "Connect"),
--                   ])),
--           ],
--       )]),
--   ])
return common.column("group", "spacing.lg", {
  ui.section({
    key = "spaced",
    label = "Spaced addons",
    children = {
      common.filled_body("ig-body", "spacing.md", {
        ui.labeled({
          key = "host-item",
          label = "Host",
          control = ui.input_group_with_addon({
            key = "ig-addon",
            addon = ui.button({ key = "ig-prefix", label = "9p://" }),
            field = ui.valued("ig-host", {
              node = ui.field({ key = "ig-host", label = "Host" }),
              value = "kernel.local",
            }),
          }),
        }),
        ui.labeled({
          key = "pair-item",
          label = "Host and port",
          control = ui.input_group({
            key = "ig-pair",
            children = {
              ui.valued("ig-host-pair", {
                node = ui.field({ key = "ig-host-pair", label = "Host" }),
                value = "kernel.local",
              }),
              ui.valued("ig-port", {
                node = ui.field({ key = "ig-port", label = "Port" }),
                value = "9p",
              }),
              ui.button({ key = "ig-go", label = "Connect" }),
            },
          }),
        }),
      }),
    },
  }),
  ui.section({
    key = "seamless",
    label = "Seamless addons",
    children = {
      common.filled_body("ig-body-seamless", "spacing.md", {
        ui.labeled({
          key = "host-item-seamless",
          label = "Host",
          control = ui.input_group_with_addon_seamless({
            key = "ig-addon-seamless",
            addon = ui.button({ key = "ig-prefix-seamless", label = "9p://" }),
            field = ui.valued("ig-host-seamless", {
              node = ui.field({ key = "ig-host-seamless", label = "Host" }),
              value = "kernel.local",
            }),
          }),
        }),
        ui.labeled({
          key = "pair-item-seamless",
          label = "Host and port",
          control = ui.input_group_seamless({
            key = "ig-pair-seamless",
            children = {
              ui.valued("ig-host-pair-seamless", {
                node = ui.field({ key = "ig-host-pair-seamless", label = "Host" }),
                value = "kernel.local",
              }),
              ui.valued("ig-port-seamless", {
                node = ui.field({ key = "ig-port-seamless", label = "Port" }),
                value = "9p",
              }),
              ui.button({ key = "ig-go-seamless", label = "Connect" }),
            },
          }),
        }),
      }),
    },
  }),
})
