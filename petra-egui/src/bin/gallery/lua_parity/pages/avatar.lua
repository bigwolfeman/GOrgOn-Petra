-- Lua source for inventory row "Avatar" (spec 013 T009).
--
-- Ported by hand from `page/avatar.rs`'s `Avatar::body()`. The page holds no
-- state (avatars are not interactive), so there is only one state to port:
--
--   section("faces", "Sizes and group", vec![body(
--       "avs", sp("spacing.md"),
--       vec![
--           row("sizes", sp("spacing.md"), vec![
--               avatar_xs("av-xs", "XS"), avatar("av-def", "DF"),
--               avatar_md("av-md", "MD"), avatar_lg("av-lg", "LG"),
--           ]),
--           avatar_group("av-group", vec![avatar("g0","AB"), avatar("g1","CD"), avatar("g2","EF")], 2),
--       ],
--   )])
return ui.section({
  key = "faces",
  label = "Sizes and group",
  children = {
    common.body("avs", "spacing.md", {
      common.row("sizes", "spacing.md", {
        ui.avatar_xs({ key = "av-xs", initials = "XS" }),
        ui.avatar({ key = "av-def", initials = "DF" }),
        ui.avatar_md({ key = "av-md", initials = "MD" }),
        ui.avatar_lg({ key = "av-lg", initials = "LG" }),
      }),
      ui.avatar_group({
        key = "av-group",
        avatars = {
          ui.avatar({ key = "g0", initials = "AB" }),
          ui.avatar({ key = "g1", initials = "CD" }),
          ui.avatar({ key = "g2", initials = "EF" }),
        },
        overflow = 2,
      }),
    }),
  },
})
