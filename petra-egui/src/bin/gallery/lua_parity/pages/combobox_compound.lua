-- Lua source for inventory row "Combobox (compound)" (spec 014 A4).
--
-- Ported by hand from `page/combobox_compound.rs`'s
-- `ComboboxCompoundPage::body()`, at the page's default state.
--
-- First of spec 013's five blocked pages. It was blocked on reachability,
-- not on transcription: `combobox_compound` lives in
-- `gorgon-petra-compound`, reaches `gorgon_petra`'s merged table only
-- through `register_external`, and nothing in the stub generator or in this
-- test process called that hook. Spec 014 A1 to A3 opened it.
--
-- The page's `Default` seeds its state by driving real intents —
-- `init`, then `Type { query: "al" }`, then `Highlight { index: 1 }` —
-- and a registry row never calls `init`, so the resting state those three
-- calls land on is named here directly: `{ query = "al", highlighted = 1,
-- open = true }`. That is the compound-row contract, not a shortcut around
-- it (`petra/petra-compound/src/registry.rs`'s module doc).
--
-- Key first, params second: the wire shape is `{ props, state }` with
-- `deny_unknown_fields` and no `key` field. See
-- `gorgon/lua-view/src/ui/compound.lua`.
--
-- Falsified 2026-09-19 by changing `highlighted = 1` to `0`, then restored
-- byte-identical and re-run green. The highlight moves which filtered row
-- carries the selection mark, and the mark is a child:
--
--   .../lua_parity/pages/combobox_compound.lua ("Combobox (compound)") has drifted
--   from its Rust page's body()
--     at children[1].children[1].children[1].children[0].children[0].children
--       lua:  2 element(s)
--       rust: 1 element(s)
return ui.section({
  key = "combobox-compound",
  label = "Combobox (compound)",
  children = {
    common.body("cbc-body", "spacing.md", {
      common.wrapped(
        "cbc-note",
        "The same triple as spec 009 T018, driven through `Compound::update`: typing, highlighting, and choosing are live intents, not a screenshot of `view` called once."
      ),
      ui.combobox_compound("cbc-combobox", {
        props = {
          label = "Theme",
          items = { "Alpha", "Bravo", "Alpine", "Charlie", "Chartreuse" },
        },
        state = { query = "al", highlighted = 1, open = true },
      }),
    }),
  },
})
