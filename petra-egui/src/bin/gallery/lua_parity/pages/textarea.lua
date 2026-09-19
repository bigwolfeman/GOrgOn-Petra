-- Lua source for inventory row "Textarea" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/textarea.rs`'s `Textarea::body()`, at the page's
-- default state (`notes = "The fiber scheduled this pass."`):
--
--   section("areas", "Default, invalid, warning", vec![filled_body(
--       "ta-body", sp("spacing.md"),
--       vec![
--           labeled("notes-item", "Notes", valued(textarea(NOTES, "Notes"), self.notes.clone())),
--           labeled("bad-item", "Invalid", textarea_invalid(BAD, "Invalid", "must not be empty")),
--           labeled("warn-item", "Warning", textarea_warning(WARN, "Warning", "looks old")),
--       ],
--   )])
--
-- `textarea_invalid`/`textarea_warning` take `message`, not `body`
-- (`registry::new_atomics::KeyLabelMessage`), so the Lua key differs from
-- the Rust argument name for both.
return ui.section({
  key = "areas",
  label = "Default, invalid, warning",
  children = {
    common.filled_body("ta-body", "spacing.md", {
      ui.labeled({
        key = "notes-item",
        label = "Notes",
        control = ui.valued("notes-val", {
          node = ui.textarea({ key = "ta-notes", label = "Notes" }),
          value = "The fiber scheduled this pass.",
        }),
      }),
      ui.labeled({
        key = "bad-item",
        label = "Invalid",
        control = ui.textarea_invalid({ key = "ta-bad", label = "Invalid", message = "must not be empty" }),
      }),
      ui.labeled({
        key = "warn-item",
        label = "Warning",
        control = ui.textarea_warning({ key = "ta-warn", label = "Warning", message = "looks old" }),
      }),
    }),
  },
})
