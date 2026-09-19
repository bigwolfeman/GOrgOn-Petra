-- Lua source for inventory row "AI label" (spec 013 T009).
--
-- Ported by hand from `page/ai_label.rs`'s `AiLabel::body()`, at the
-- page's `#[derive(Default)]` state (`open = false`): every `ai_label*`
-- call on the page passes its own `selected`/`open` argument, and the one
-- that reads live page state (`self.open`) is `false` here, so the
-- explainability panel never mounts and every row is a plain trigger.
local EXPLANATION = "This score came from a model, not a person. It reads 4,318 resolved "
  .. "tickets and weighs the twelve most recent the heaviest."

return ui.section({
  key = "ai",
  label = "AI label",
  children = {
    common.body("ai-body", "spacing.md", {
      common.wrapped(
        "note",
        "An AI label marks a value a model produced rather than a person. It is also a "
          .. "button: press it and it opens an explainability popover that says where the "
          .. "value came from. Press the mark below, and press it again to close it."
      ),
      common.row("live-row", "spacing.md", {
        ui.ai_label({ key = "ai-live", label = "Confidence score", selected = false, value = EXPLANATION }),
      }),
      common.wrapped(
        "flat-note",
        "Carbon tints the open panel with a gradient aura. Petra's draw list has no "
          .. "gradient and no blur primitive, on purpose (draw/mod.rs), so the panel here "
          .. "is a flat surface with the same shape."
      ),
      common.wrapped(
        "sizes-note",
        "The default variant ships seven sizes: 16, 20, 24, 32, 40, 48 and 64."
      ),
      common.row("sizes", "spacing.md", {
        ui.ai_label_mini({ key = "ai-mini", label = "Confidence, mini", selected = false, value = EXPLANATION }),
        ui.ai_label_2xs({ key = "ai-2xs", label = "Confidence, 2xs", selected = false, value = EXPLANATION }),
        ui.ai_label_xs({ key = "ai-xs", label = "Confidence, xs", selected = false, value = EXPLANATION }),
        ui.ai_label_sm({ key = "ai-sm", label = "Confidence, sm", selected = false, value = EXPLANATION }),
        ui.ai_label({ key = "ai-md", label = "Confidence, md", selected = false, value = EXPLANATION }),
        ui.ai_label_lg({ key = "ai-lg", label = "Confidence, lg", selected = false, value = EXPLANATION }),
        ui.ai_label_xl({ key = "ai-xl", label = "Confidence, xl", selected = false, value = EXPLANATION }),
      }),
      common.wrapped(
        "inline-note",
        "The inline variant sits in running text. It has a leading bullet instead of a "
          .. "border, and its own three sizes: 16, 18 and 22."
      ),
      common.row("inline", "spacing.md", {
        ui.ai_label_inline_sm({ key = "ai-inline-sm", label = "Ask AI, sm", selected = false, value = EXPLANATION }),
        ui.ai_label_inline({ key = "ai-inline-md", label = "Ask AI, md", selected = false, value = EXPLANATION }),
        ui.ai_label_inline_lg({ key = "ai-inline-lg", label = "Ask AI, lg", selected = false, value = EXPLANATION }),
      }),
      common.wrapped(
        "revert-note",
        "Revert is the one state unique to this component: the whole trigger is swapped "
          .. "for a control that puts the value back. Carbon draws a bare undo arrow; "
          .. "Petra has no Undo mark and does not ship icon-only controls, so it is the word."
      ),
      common.row("revert-row", "spacing.md", {
        ui.ai_label_revert({ key = "ai-revert", label = "Revert to the AI suggestion" }),
      }),
    }),
  },
})
