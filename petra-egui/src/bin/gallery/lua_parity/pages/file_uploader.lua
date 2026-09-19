-- Lua source for inventory row "File uploader" (spec 013 T009).
--
-- Ported by hand from `page/file_uploader.rs`'s `FileUploader::body()`, at
-- the page's default state (`impl Default`: `files = ["trace.ndjson",
-- "fiber-dump.ndjson"]`, so the loop emits `fu-0`/`fu-1`):
--
--   let mut rows = vec![file_uploader_with("fu", "Upload a trace", "NDJSON or YAML, up to 5 MB")];
--   rows.push(file_uploader_item_edit("fu-0", "trace.ndjson"));
--   rows.push(file_uploader_item_edit("fu-1", "fiber-dump.ndjson"));
--   rows.push(file_uploader_item("fu-busy", "kernel.yaml", false));
--   rows.push(file_uploader_item("fu-done", "notes.txt", true));
--   rows.push(file_uploader_item_invalid("fu-bad", "core.dump", "File is over 5 MB"));
--   rows.push(file_uploader_item_warning("fu-warn", "stale.ndjson", "older than the session"));
--   rows.push(wrapped("note", "Pressing a row's remove control drops that row. Pressing the \
--       zone opens the system file dialog, and a file dropped on the \
--       window arrives the same way — both reach the page as paths."));
--   section("files", "Uploader", vec![body("fu-body", sp("spacing.md"), rows)])
return ui.section({
  key = "files",
  label = "Uploader",
  children = {
    common.body("fu-body", "spacing.md", {
      ui.file_uploader_with({
        key = "fu",
        label = "Upload a trace",
        description = "NDJSON or YAML, up to 5 MB",
      }),
      ui.file_uploader_item_edit({ key = "fu-0", label = "trace.ndjson" }),
      ui.file_uploader_item_edit({ key = "fu-1", label = "fiber-dump.ndjson" }),
      ui.file_uploader_item({ key = "fu-busy", label = "kernel.yaml", selected = false }),
      ui.file_uploader_item({ key = "fu-done", label = "notes.txt", selected = true }),
      ui.file_uploader_item_invalid({
        key = "fu-bad",
        label = "core.dump",
        message = "File is over 5 MB",
      }),
      ui.file_uploader_item_warning({
        key = "fu-warn",
        label = "stale.ndjson",
        message = "older than the session",
      }),
      common.wrapped(
        "note",
        "Pressing a row's remove control drops that row. Pressing the zone opens the system file dialog, and a file dropped on the window arrives the same way — both reach the page as paths."
      ),
    }),
  },
})
