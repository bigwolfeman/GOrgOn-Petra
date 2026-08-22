# gorgon-petra-egui

The only crate in the workspace that knows egui. Petra owns layout; this crate
owns the substrate (D-069). It hosts an `eframe` application, paints a petrified
frame's placements through `egui::Context::layer_painter`, shapes text into
cached galleys behind `gorgon_petra::layout::ContentMeasure`, and translates
platform input at the boundary.

egui's `Ui` layout is never used. Nothing in `gorgon-petra`'s public surface
mentions an egui type.

## Known Limitations and Deferred Work

- **No window has ever been opened.** `Host` implements `eframe::App` and every
  test in this crate drives it headless, through an `egui::Context` with no
  event loop, no GPU, and no compositor. Layout, painting, shaping, and input
  translation are exercised; `eframe`'s native run path is not.
- **AccessKit nodes are not pushed.** The crate has no `accesskit` dependency
  and the semantic tree does not reach the platform; incoming
  `AccessKitActionRequest` events are counted in
  `EventTranslator::untranslated()` and discarded. Screen-reader support is
  task T028.
- Images and custom painters have no painter here. A node declaring either
  measures to zero and lands in `PaintReport::undrawn` by name, and the pass
  reports itself incomplete rather than green.
- Token slots other than `background`, `border`, and `foreground` are recorded
  in `PaintReport::unknown_slots` and not drawn. The shipped vocabulary
  declares `shape.*` corner radii that nothing here consumes, so a node asking
  for rounded corners gets square ones — visibly, in the report.
- Single viewport only (R2): egui's multi-viewport support is broken upstream on
  Wayland (egui #8142, #8405). Detached panels are a spec 004 concern and will
  need that fixed or worked around.
- Emoji and colour-glyph fallback need the custom-glyph registry (R4); the
  atlas half is merged upstream, the shaping half is still a draft PR. Until the
  overlay fork carries it, an emoji renders monochrome or not at all — never as
  a tofu box, and the gap is recorded rather than hidden.
- The wasm target is **not** built by any gate. Nothing in
  `gorgon/xtask/src/` passes `--target`, so desktop/web parity (SC-006) is
  unverified in both directions (tasks T060-T062).

## Agent Experience

- The crate boundary is the debugging tool: if a layout is wrong, it is wrong in
  `gorgon-petra` and reproducible in a unit test with no window, no GPU, and no
  display server. Only painting, shaping, and input translation live here.
- Every painted frame carries the `(seq, digest)` it was rasterized from, so a
  screenshot an agent captures can be tied back to the exact placements that
  produced it.
