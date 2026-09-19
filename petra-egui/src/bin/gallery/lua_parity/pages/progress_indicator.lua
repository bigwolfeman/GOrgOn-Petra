-- Lua source for inventory row "Progress indicator" (spec 013 T009).
--
-- Ported by hand from `page/progress_indicator.rs`'s
-- `ProgressIndicator::body()`. The page holds no live state.
return ui.section({
  key = "steps",
  label = "Steps",
  children = {
    common.body("pi", "spacing.md", {
      ui.progress_indicator({
        key = "pi",
        children = {
          ui.progress_step({ key = "st-0", label = "Clone", complete = true, current = false }),
          ui.progress_step({ key = "st-1", label = "Build", complete = false, current = true }),
          ui.progress_step({ key = "st-2", label = "Run", complete = false, current = false }),
        },
      }),
    }),
  },
})
