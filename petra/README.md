# gorgon-petra

Petra is GOrgOn's retained layout engine. A view tree is plain nested data;
layout is a proposal/response negotiation that runs top-down and answers
bottom-up; the output is a *petrified frame* — one placement per node, plus a
sequence number and a BLAKE3 content digest that make the frame verifiable by a
gate, an agent, or a screenshot consumer.

This crate has no GUI-toolkit dependency and never will. Content measurement
enters through `layout::ContentMeasure` and virtualized rows through
`layout::RowSource`; `gorgon-petra-egui` is the only crate that implements them.
The `petra-boundary` gate reads this crate's manifest and fails if an
egui-family dependency ever appears in it.

The binding documents are `specs/003-petra-layout-engine/contracts/`. Where this
crate and a contract disagree, the contract wins.

## Known Limitations and Deferred Work

- `src/anim/` and `src/semantic/` are module headers with no code (tasks
  T027, T048-T055). Transitions and the semantic projection do not exist yet.
- `src/focus/` is wired: `gorgon-petra-egui`'s `Host` owns one `FocusTree`,
  reconciles it with every placed frame, and publishes the result to
  `LayoutState::focused`. Tab and Shift+Tab traverse, Home and End jump to the
  first and last focusable when the focused node declares no `Key` interaction,
  and Enter or Space works a focused node that declares `Click`. What is
  **not** done: nothing *shows* focus. `PlacementSemantics` carries no
  `focused` flag, no container reads `LayoutState::focused` during
  negotiation, and no painter draws a focus ring — so a keyboard user can
  reach and operate a node without seeing which one it is (tasks T027,
  T048-T055). A node that declares `Click` without `Focus` is still
  unreachable by keyboard; nothing in tree acceptance requires the pair yet.
- `Anchor::Node` resolves the same way `Anchor::Viewport` does. The surface is
  anchored to the viewport rather than to the named node, and
  `overlay_surface::resolve_anchor_kind` says so rather than pretending
  otherwise.
- The digest covers placements and paint state, never pixels. Pixel-level
  agreement between desktop and web is the parity lane's job (SC-006), with a
  declared tolerance, and is not asserted here.
- A spacer answers an `Unbounded` probe with a large finite constant
  (`layout::leaf::SPACER_MAX_EXTENT`) rather than an infinity. That keeps
  responses finite as the contract requires, at the cost of a spacer inside a
  container larger than 65 535 logical units no longer being the most flexible
  child.

## Agent Experience

The engine is the surface an agent authors against, so it is built to fail with
sentences rather than with pictures.

- `tree::validate` refuses a whole tree and returns *every* violation in
  pre-order, each naming the offending node's key path (`/root/list/row-3`) and
  what is wrong with it. One run of the check names every fix.
- Node ids are derived from key paths, not allocated, so an id an agent reads in
  one frame means the same node in the next one and is never recycled.
- `frame::digest::canonical_bytes` is public: when two frames that should match
  do not, the byte stream that produced each digest can be diffed directly
  instead of guessing at the hash.
