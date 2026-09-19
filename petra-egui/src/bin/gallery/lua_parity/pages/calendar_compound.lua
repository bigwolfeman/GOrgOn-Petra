-- Lua source for inventory row "Calendar (compound)" (spec 014 A4).
--
-- Ported by hand from `page/calendar_compound.rs`'s
-- `CalendarCompoundPage::body()`, at the page's default state: three panes,
-- one per `calendar::Mode`, each seeded by replaying `Intent::Pick` calls.
--
-- A registry row never calls `Compound::init`, so each pane's resting state
-- is named here directly rather than replayed. The Range pane is the one
-- worth reading twice: picking the 3rd and then the 9th does not store two
-- dates and an anchor, it stores the whole inclusive run 3..=9 and clears
-- the anchor, which is what `Calendar::update` does and what this file has
-- to spell out.
--
-- Falsified 2026-09-19 by changing the Single pane's `day = 15` to `16`,
-- then restored byte-identical and re-run green:
--
--   .../lua_parity/pages/calendar_compound.lua ("Calendar (compound)") has drifted
--   from its Rust page's body()
--     at children[1].children[1].children[0].children[1].children[0].children[0].props.text
--       lua:  "2026-08-16"
--       rust: "2026-08-15"
return ui.section({
  key = "calendar-compound",
  label = "Calendar (compound)",
  children = {
    common.body("calc-body", "spacing.md", {
      common.wrapped(
        "calc-note",
        "The same triple as spec 009 T032, one pane per `Mode`: Single replaces the pick, Range fills inclusive between two picks, Multi toggles each day on its own."
      ),
      common.row("calc-row", "spacing.lg", {
        common.column("single", "spacing.sm", {
          ui.heading({ key = "single-title", label = "Single" }),
          ui.calendar_compound("single-cal", {
            props = { label = "Single", mode = "Single" },
            state = {
              year = 2026,
              month = 8,
              selected = { { year = 2026, month = 8, day = 15 } },
              mode = "Single",
            },
          }),
        }),
        common.column("range", "spacing.sm", {
          ui.heading({ key = "range-title", label = "Range" }),
          ui.calendar_compound("range-cal", {
            props = { label = "Range", mode = "Range" },
            state = {
              year = 2026,
              month = 8,
              selected = {
                { year = 2026, month = 8, day = 3 },
                { year = 2026, month = 8, day = 4 },
                { year = 2026, month = 8, day = 5 },
                { year = 2026, month = 8, day = 6 },
                { year = 2026, month = 8, day = 7 },
                { year = 2026, month = 8, day = 8 },
                { year = 2026, month = 8, day = 9 },
              },
              mode = "Range",
            },
          }),
        }),
        common.column("multi", "spacing.sm", {
          ui.heading({ key = "multi-title", label = "Multi" }),
          ui.calendar_compound("multi-cal", {
            props = { label = "Multi", mode = "Multi" },
            state = {
              year = 2026,
              month = 8,
              selected = {
                { year = 2026, month = 8, day = 4 },
                { year = 2026, month = 8, day = 18 },
                { year = 2026, month = 8, day = 27 },
              },
              mode = "Multi",
            },
          }),
        }),
      }),
    }),
  },
})
