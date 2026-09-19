-- Lua source for inventory row "Command (compound)" (spec 014 A4).
--
-- Ported by hand from `page/command_compound.rs`'s
-- `CommandCompoundPage::body()`, at the page's default state: `init`, then
-- `Open`, then `ToggleFavorite { id: "rebuild" }`. A registry row never
-- calls `Compound::init`, so that resting state is named directly.
--
-- This page is why spec 014 put `icon` on the command wire. All five items
-- and both modes carry an `IconMark`, `Command::view` mounts a glyph child
-- for each one, and the wire used to drop the field — so this page had no
-- expressible Lua form at all, whatever the registration path did. See
-- `petra/petra-compound/src/registry.rs`'s `CommandItemWire`.
--
-- `mode_surface` is deliberately absent and is not a state this file can
-- reach: the host fills it in Rust when the query names a mode, the query
-- is empty at rest, and the wire refuses the field by name
-- (`deny_unknown_fields`).
--
-- The five buttons above the palette are the page's own, outside the
-- compound: `Command::view` closed is an empty stack, so nothing inside it
-- can open it.
--
-- Falsified 2026-09-19 by changing the first item's `icon = "menu"` to
-- `"close"`, then restored byte-identical and re-run green. The icon really
-- is on this page's critical path — the difference lands in the glyph's own
-- canvas commands, so nothing about this page passes if the wire drops it:
--
--   .../lua_parity/pages/command_compound.lua ("Command (compound)") has drifted
--   from its Rust page's body()
--     at children[1].children[1].children[1].children[0].children[2].children[0]
--        .children[0].children[0].props.canvas.commands
--       lua:  2 element(s)
--       rust: 4 element(s)
--
-- `tile_columns = 4` changed to `5` was tried first and did NOT fire: the
-- resting view is List, so tile density is not on screen. Worth knowing —
-- a page that rests in one mode proves nothing about the others.
return ui.section({
  key = "command-compound",
  label = "Command (compound)",
  children = {
    common.body("cmc-body", "spacing.md", {
      common.wrapped(
        "cmc-note",
        "The same triple as spec 009 T019: a frame-wide overlay driven through `Compound::update`. Type to filter, arrows move the cursor, Enter chooses. `>` for favorite tiles, `>f` / `>c` for a host-filled slot. 1 / 2 / 3 columns and Tiles set density; category icons filter without clearing the query."
      ),
      common.column("cmc-col", "spacing.md", {
        common.row("cmc-actions", "spacing.sm", {
          ui.button({ key = "open-command", label = "Open command" }),
          ui.button({ key = "open-list", label = "1 column" }),
          ui.button({ key = "open-cols-2", label = "2 columns" }),
          ui.button({ key = "open-cols-3", label = "3 columns" }),
          ui.button({ key = "open-tiles", label = "Tiles" }),
        }),
        ui.command_compound("cmc-palette", {
          props = {
            items = {
              {
                id = "rebuild",
                label = "Rebuild fiber",
                shortcut = "Ctrl+R",
                categories = { "Edit" },
                icon = "menu",
              },
              {
                id = "open-file",
                label = "Open file",
                shortcut = "Ctrl+O",
                categories = { "Files" },
                icon = "search",
              },
              {
                id = "edit",
                label = "Edit buffer",
                shortcut = "Ctrl+E",
                categories = { "Edit" },
                icon = "edit",
              },
              {
                id = "copy-path",
                label = "Copy path",
                shortcut = "Ctrl+C",
                categories = { "Edit", "Files" },
                icon = "copy",
              },
              {
                id = "close-window",
                label = "Close window",
                shortcut = "Ctrl+W",
                categories = { "Files" },
                icon = "close",
              },
            },
            list_columns = 2,
            tile_columns = 4,
            default_view = "list",
            modes = {
              { key = "f", label = "Files", icon = "search" },
              { key = "c", label = "Calculator", icon = "add" },
            },
          },
          state = { open = true, favorites = { "rebuild" } },
        }),
      }),
    }),
  },
})
