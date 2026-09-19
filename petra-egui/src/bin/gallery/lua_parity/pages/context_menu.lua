-- Lua source for inventory row "Context menu" (spec 013 T009).
--
-- Ported by hand from `page/context_menu.rs`'s `ContextMenu::body()`, at the
-- page's default state: `open = true`, `at = Point::new(340.0, 380.0)`,
-- `flyout = false`. So the panel IS mounted beside the trigger, and
-- `add_row()` takes its `!self.flyout` early return -- one plain
-- `menu_item_with` keyed `ctx-add` with `submenu = true`, not the hugging
-- Stack plus `menu_flyout` pair the open state builds.
--
-- **This page was the last of the 57 to land, and the only one that needed a
-- change in `gorgon_petra` to become portable at all.** Its trigger used to
-- be built by mutating a finished node:
--
--   let mut trigger = button(TRIGGER, "Show menu");
--   trigger.interactions.push(Interaction::SecondaryClick);
--
-- `ui.button` emits a component REFERENCE, and `registry::expand_node`
-- returns `build(..)` whole, so anything set on the node carrying the
-- reference is discarded. The measured failure was exactly one line, which
-- is what makes it a wire limit rather than a transcription mistake:
--
--   at children[1].children[1].children[0].interactions
--     lua:  3 element(s)
--     rust: 4 element(s)
--
-- `Interaction::SecondaryClick` had no constructor anywhere in
-- `gorgon_petra`, so this page was reaching past the library for something
-- the library never offered. `component::also_secondary_click` is that
-- constructor (operator ruling 2026-09-19: a modifier, not a
-- `secondary_click_button`, because the capability is not a button's), and
-- `ui.also_secondary_click` is its wrapper. It takes `(key, t)` like the
-- other `NodeOnly` modifiers: its shape declares no `key`, so the injected
-- `component` factory cannot read one off the table.
--
-- # Falsified twice, 2026-09-19
--
-- First by accident, which is the more useful of the two. This file was
-- written at `open = false` because that is what a page whose trigger opens
-- a menu ought to rest at. It rests at `open = true`:
--
--   at children[1].children[1].children
--     lua:  1 element(s)
--     rust: 2 element(s)
--
-- Then deliberately, `ctx-undo`'s `icon = "edit"` changed to `"copy"`, and
-- the interesting part is where the panic lands:
--
--   at children[1].children[1].children[1].children[0].children[0].children[0].children[0].props.canvas.commands
--     lua:  6 element(s)
--     rust: 3 element(s)
--
-- Eight levels down, in the draw list of the glyph the icon expands into.
-- A wire enum this page never sees rendered still reaches a vector path
-- with a different number of commands in it, which is the registry doing
-- its job: one implementation, and the proof runs all the way to the ink.
-- Restored byte-identical afterwards (`diff` against a saved copy, empty).
return ui.section({
  key = "ctx-sec",
  label = "Point-anchored menu",
  children = {
    common.body("ctx-body", "spacing.md", {
      common.wrapped(
        "ctx-note",
        "Right-click Show menu to open it at the pointer; a plain click or Enter opens it at the button too. Rows carry a leading icon and a Ctrl+ shortcut. Hover Add to folder to fold it out to the right. Top-left of the panel is the press point. A dropdown from a button is row 18."
      ),
      common.column("ctx-pair", nil, {
        ui.also_secondary_click("show-ctx", {
          node = ui.button({ key = "show-ctx", label = "Show menu" }),
        }),
        ui.context_menu({
          key = "ctx",
          label = "Actions",
          x = 340.0,
          y = 380.0,
          children = {
            ui.menu_item_with({
              key = "ctx-undo",
              label = "Undo",
              icon = "edit",
              shortcut = "Ctrl+Z",
              submenu = false,
            }),
            ui.menu_item_with({
              key = "ctx-copy",
              label = "Copy",
              icon = "copy",
              shortcut = "Ctrl+C",
              submenu = false,
            }),
            -- `add_row()` at `flyout = false`.
            ui.menu_item_with({
              key = "ctx-add",
              label = "Add to folder",
              icon = "add",
              submenu = true,
            }),
            ui.menu_item_with({
              key = "ctx-delete",
              label = "Delete",
              icon = "close",
              shortcut = "Ctrl+D",
              submenu = false,
            }),
          },
        }),
      }),
    }),
  },
})
