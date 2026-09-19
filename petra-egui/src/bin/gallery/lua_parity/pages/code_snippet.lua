-- Lua source for inventory row "Code snippet" (spec 013 T009, leaf G1).
--
-- Ported by hand from `page/code_snippet.rs`'s `CodeSnippet::body()`, at the
-- page's default state (`pending = None`, `now = 0.0`, `copied = None`, so
-- `self.saying_copied(..)` is `false` for all three wells):
--
--   section("snippets", "Single, multi-line, inline", vec![filled_body(
--       "code", sp("spacing.md"),
--       vec![
--           code_snippet_copied(code_runs(code_snippet(SNIP, SINGLE), shell_runs(SINGLE)), false),
--           code_snippet_copied(code_runs(code_snippet_multi(SNIP_MULTI, MULTI), shell_runs(MULTI)), false),
--           code_snippet_copied(code_snippet_inline(SNIP_INLINE, INLINE), false),
--       ],
--   )])
--
-- `code_runs` and `code_snippet_copied` are both `(key, t)` modifiers (their
-- param shapes carry no `key`). `shell_runs` is a page-private classifier
-- (`page/code_snippet.rs`), not a registry constructor, so its output for
-- `SINGLE` and `MULTI` is computed here by hand, running the exact same
-- four rules over the exact same byte strings, and inlined as the `runs`
-- table `code_runs` takes. Cross-checked byte-for-byte against
-- `page/code_snippet.rs`'s `SINGLE`/`MULTI` consts and the `CodeInk` token
-- map (`Plain -> none`, `Comment -> "text.muted"`, `Keyword -> "link-primary"`,
-- `Literal -> "text.primary"`), then run through a standalone reimplementation
-- of `shell_runs`/`words` to produce the merged run list below — not
-- transcribed by eye, because a 662-byte multi-line script is exactly where
-- a by-eye transcription would silently drift.
local SINGLE = "pcargo test -p gorgon-petra --lib"

local MULTI = [[
# The gates, in the order the wave brief lists them.
pcargo test -p gorgon-petra --lib
pcargo test -p gorgon-petra-egui --lib
pcargo test -p gorgon-petra-egui --bin gallery
pcargo test -p gorgon-petra --test anchored_determinism
pcargo test -p gorgon-petra --test incremental_frames
pcargo fmt --all -- --check
pcargo clippy --workspace --all-targets -- -D warnings
pcargo xtask verify-notes

# Then look at the pictures.
PETRA_SHOT_DIR=/tmp/wave pcargo test -p gorgon-petra-egui --bin gallery \
    every_built_page_rasterizes_to_more_than_one_colour
magick /tmp/wave/06-code-snippet.png -crop 900x700+480+240 +repage \
    -filter point -resize 400% /tmp/c.png]]

local INLINE = "cargo xtask gates"

local SINGLE_RUNS = {
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 14 },
  { len = 5, foreground = "text.primary" },
}

local MULTI_RUNS = {
  { len = 52, foreground = "text.muted" },
  { len = 1 },
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 14 },
  { len = 5, foreground = "text.primary" },
  { len = 1 },
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 19 },
  { len = 5, foreground = "text.primary" },
  { len = 1 },
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 19 },
  { len = 5, foreground = "text.primary" },
  { len = 9 },
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 14 },
  { len = 6, foreground = "text.primary" },
  { len = 22 },
  { len = 6, foreground = "link-primary" },
  { len = 6 },
  { len = 2, foreground = "text.primary" },
  { len = 14 },
  { len = 6, foreground = "text.primary" },
  { len = 20 },
  { len = 6, foreground = "link-primary" },
  { len = 5 },
  { len = 5, foreground = "text.primary" },
  { len = 1 },
  { len = 2, foreground = "text.primary" },
  { len = 1 },
  { len = 7, foreground = "text.primary" },
  { len = 1 },
  { len = 6, foreground = "link-primary" },
  { len = 8 },
  { len = 11, foreground = "text.primary" },
  { len = 1 },
  { len = 13, foreground = "text.primary" },
  { len = 1 },
  { len = 2, foreground = "text.primary" },
  { len = 1 },
  { len = 2, foreground = "text.primary" },
  { len = 10 },
  { len = 6, foreground = "link-primary" },
  { len = 21 },
  { len = 28, foreground = "text.muted" },
  { len = 1 },
  { len = 24, foreground = "link-primary" },
  { len = 13 },
  { len = 2, foreground = "text.primary" },
  { len = 19 },
  { len = 5, foreground = "text.primary" },
  { len = 67 },
  { len = 6, foreground = "link-primary" },
  { len = 31 },
  { len = 5, foreground = "text.primary" },
  { len = 31 },
  { len = 7, foreground = "text.primary" },
  { len = 7 },
  { len = 7, foreground = "text.primary" },
  { len = 16 },
}

return ui.section({
  key = "snippets",
  label = "Single, multi-line, inline",
  children = {
    common.filled_body("code", "spacing.md", {
      ui.code_snippet_copied("snip-copied", {
        node = ui.code_runs("snip-runs", {
          node = ui.code_snippet({ key = "snip", label = SINGLE }),
          runs = SINGLE_RUNS,
        }),
        copied = false,
      }),
      ui.code_snippet_copied("snip-multi-copied", {
        node = ui.code_runs("snip-multi-runs", {
          node = ui.code_snippet_multi({ key = "snip-multi", label = MULTI }),
          runs = MULTI_RUNS,
        }),
        copied = false,
      }),
      ui.code_snippet_copied("snip-in-copied", {
        node = ui.code_snippet_inline({ key = "snip-in", label = INLINE }),
        copied = false,
      }),
    }),
  },
})
