-- Lua source for inventory row "Progress bar" (spec 013 T009).
--
-- Ported by hand from `page/progress_bar.rs`'s `ProgressBar::body()`. The
-- page holds no live state, so there is only one shape to match.
return ui.section({
  key = "bars",
  label = "Determinate",
  children = {
    common.body("progress", "spacing.md", {
      ui.progress({ key = "prog-big", label = "Rebuild", value = 0.62 }),
      ui.progress_sm({ key = "prog-sm", label = "Upload", value = 0.25 }),
      ui.progress_with_helper({ key = "prog-help", label = "Index", value = 1.0, helper = "Complete" }),
    }),
  },
})
