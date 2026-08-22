# gorgon-petra-egui

The only crate in the workspace that knows egui. Petra owns layout; this crate
owns the substrate (D-069). It hosts an `eframe` application, paints a petrified
frame's placements through `egui::Context::layer_painter`, shapes text into
cached galleys behind `gorgon_petra::layout::ContentMeasure`, translates
platform input at the boundary, and pushes AccessKit nodes from the semantic
tree.

egui's `Ui` layout is never used. Nothing in `gorgon-petra`'s public surface
mentions an egui type.

## Known Limitations and Deferred Work

- `host`, `input`, `paint`, and `text` are module headers with no code (task
  T013). This crate compiles and links egui; it does not yet open a window.
- Single viewport only (R2): egui's multi-viewport support is broken upstream on
  Wayland (egui #8142, #8405). Detached panels are a spec 004 concern and will
  need that fixed or worked around.
- Emoji and colour-glyph fallback need the custom-glyph registry (R4); the
  atlas half is merged upstream, the shaping half is still a draft PR. Until the
  overlay fork carries it, an emoji renders monochrome or not at all — never as
  a tofu box, and the gap is recorded rather than hidden.
- The wasm target compiles in gates, but no web capture path exists yet
  (tasks T060-T062).

## Agent Experience

- The crate boundary is the debugging tool: if a layout is wrong, it is wrong in
  `gorgon-petra` and reproducible in a unit test with no window, no GPU, and no
  display server. Only painting, shaping, and input translation live here.
- Every painted frame carries the `(seq, digest)` it was rasterized from, so a
  screenshot an agent captures can be tied back to the exact placements that
  produced it.
