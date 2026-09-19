-- Lua source for inventory row "OTP" (spec 013 T009).
--
-- Ported by hand from `page/otp.rs`'s `Otp::body()`, at the page's default
-- state (`value = "204"`, `GROUP = "otp"`, `LENGTH = 6`):
--
--   section("code", "One-time code", vec![filled_body(
--       "otp-body", sp("spacing.md"),
--       vec![otp(GROUP, LENGTH, &self.value)],
--   )])
return ui.section({
  key = "code",
  label = "One-time code",
  children = {
    common.filled_body("otp-body", "spacing.md", {
      ui.otp({ key = "otp", length = 6, value = "204" }),
    }),
  },
})
