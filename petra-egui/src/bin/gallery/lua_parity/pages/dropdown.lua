-- Lua source for inventory row "Dropdown" (spec 013 T009).
--
-- Ported by hand from `page/dropdown.rs`'s `Dropdown::body()`, at the
-- page's `#[derive(Default)]` state (`open = false`, `selected = 0`, so
-- `value = OPTIONS[0].1 = "Dark"`). `open = false` takes the `dropdown`
-- (closed) branch rather than `dropdown_open`.
return ui.section({
  key = "drop",
  label = "Dropdown",
  children = {
    common.body("dd-body", "spacing.md", {
      ui.dropdown({ key = "dd", label = "Theme", value = "Dark" }),
    }),
    common.body("dd-sizes", "spacing.md", {
      ui.dropdown_xs({ key = "dd-xs", label = "Xs 24", value = "Dark" }),
      ui.dropdown_sm({ key = "dd-sm", label = "Sm 32", value = "Dark" }),
      ui.dropdown({ key = "dd-md", label = "Md 40", value = "Dark" }),
      ui.dropdown_lg({ key = "dd-lg", label = "Lg 48", value = "Dark" }),
    }),
  },
})
