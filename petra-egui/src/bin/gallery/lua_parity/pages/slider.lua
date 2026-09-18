-- Lua source for inventory row "Slider" (spec 013 T008/T008a proof).
--
-- Ported by hand from `page/slider.rs`'s `Slider::body()`, at the page's
-- default state (`volume = 0.4`, `draft = slider_input_text(0.4) = "40"`):
--
--   section("slide", "Slider", vec![body(
--       "slid", sp("spacing.md"),
--       vec![valued(slider(VOLUME, "Volume", self.volume), self.draft.clone())],
--   )])
--
-- Chosen as the first page on purpose: it never calls bare `text(`, bare
-- `button(`, or `common::wrapped`, so it proves the harness's two real load-
-- bearing paths — `common.body` (T008a's one genuine second implementation)
-- and a component reference nested inside a modifier's own params
-- (`ui.valued`'s `node` field, expanded by `registry::expand`'s recursive
-- walk before `valued` itself runs) — without also depending on a third.
--
-- `lua_parity.rs` finds this file by walking `lua_parity/pages/`, and maps
-- it to `page::slider::Slider`'s `body()` by slugging that page's
-- `Page::row()` ("Slider" -> "slider") the same way its own module file is
-- named. Adding the next page's proof is one new file here plus the
-- matching page's own `page/*.rs` if it does not exist yet — never an edit
-- to this file or to `lua_parity.rs` itself, which is what lets T009's 56
-- remaining pages be split across parallel leaves with nothing shared to
-- collide on.
return ui.section({
  key = "slide",
  label = "Slider",
  children = {
    common.body("slid", "spacing.md", {
      ui.valued("vol", {
        node = ui.slider({ key = "vol", label = "Volume", value = 0.4 }),
        value = "40",
      }),
    }),
  },
})
