-- Lua source for inventory row "Text input" (spec 013 T009).
--
-- Ported by hand from `page/text_input.rs`'s `TextInput::body()`, at the
-- page's default state (`impl Default`: `md = "kernel-boot"`, `sm = ""`,
-- `lg = ""`, `port = "http"`). `port_is_valid()` is false for "http" (not
-- all-digit), so `port_field()` builds `field_validated` with
-- `message = Some("must be a number")`:
--
--   section("fields", "Default sizes and states", vec![filled_body(
--       "inputs", sp("spacing.md"),
--       vec![
--           labeled("fiber-name", "Fiber name",
--               valued(hinted(field("field-md", "Fiber name"), "fiber-7"), "kernel-boot")),
--           labeled("small", "Small", valued(field_sm("field-sm", "Small input"), "")),
--           labeled("large", "Large", valued(field_lg("field-lg", "Large input"), "")),
--           labeled("port", "Port",
--               valued(field_validated("field-port", "Port", Some("must be a number")), "http")),
--           labeled("read-only", "Read only",
--               valued(field_readonly("field-ro", "Read-only value"), "9p://kernel/0")),
--           field_required("field-req", "Fiber id"),
--           field_warning("field-warn", "Owner", "looks old"),
--       ],
--   )])
return ui.section({
  key = "fields",
  label = "Default sizes and states",
  children = {
    common.filled_body("inputs", "spacing.md", {
      ui.labeled({
        key = "fiber-name",
        label = "Fiber name",
        control = ui.valued("field-md", {
          node = ui.hinted("field-md", {
            node = ui.field({ key = "field-md", label = "Fiber name" }),
            hint = "fiber-7",
          }),
          value = "kernel-boot",
        }),
      }),
      ui.labeled({
        key = "small",
        label = "Small",
        control = ui.valued("field-sm", {
          node = ui.field_sm({ key = "field-sm", label = "Small input" }),
          value = "",
        }),
      }),
      ui.labeled({
        key = "large",
        label = "Large",
        control = ui.valued("field-lg", {
          node = ui.field_lg({ key = "field-lg", label = "Large input" }),
          value = "",
        }),
      }),
      ui.labeled({
        key = "port",
        label = "Port",
        control = ui.valued("field-port", {
          node = ui.field_validated({
            key = "field-port",
            label = "Port",
            message = "must be a number",
          }),
          value = "http",
        }),
      }),
      ui.labeled({
        key = "read-only",
        label = "Read only",
        control = ui.valued("field-ro", {
          node = ui.field_readonly({ key = "field-ro", label = "Read-only value" }),
          value = "9p://kernel/0",
        }),
      }),
      ui.field_required({ key = "field-req", label = "Fiber id" }),
      ui.field_warning({ key = "field-warn", label = "Owner", message = "looks old" }),
    }),
  },
})
