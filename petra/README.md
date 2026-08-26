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

- The engine drives three properties, not four: `DRIVEN_PROPERTIES` is
  `[Position, Size, Opacity]`. Colour is interpolable but nothing drives it, so
  a state change swaps a colour rather than easing it, and
  `color_is_interpolable_but_not_yet_driven_by_the_engine` pins that gap so it
  cannot close or widen silently.
- Settling and ambience are two different questions, deliberately. A non-ambient
  transition settles, and when nothing ambient exists scheduling stops. An
  ambient definition never settles and never blocks a driver's settle - it keeps
  scheduling by design, and `TransitionActivity::is_settled` excludes it. An
  undeclared continuous repaint is what the ambient ledger is for.
- Focus is wired and it shows. `gorgon-petra-egui`'s `Host` owns one
  `FocusTree`, reconciles it with every placed frame, publishes the result to
  `LayoutState::focused`, and `PlacementSemantics::focused` carries it into the
  digest; `paint_focus_ring` draws it. Tab and Shift+Tab traverse, Home and End
  jump to the first and last focusable when the focused node declares no `Key`
  interaction, and Enter or Space works a focused node that declares `Click`.
  What is **not** done: a node that declares `Click` without `Focus` is still
  unreachable by keyboard, and nothing in tree acceptance requires the pair.
- Hover, pressed and pointer capture are declared but not derived. The five
  interaction-state flags reach `PlacementSemantics`, the semantic tree, the
  wire and the digest, and `read_only` and `skeleton` are settable on
  `Semantics` - but no host writes hover or pressed yet, so they read `false` on
  every placement. `InputEvent::PointerLeft` is dropped rather than delivered,
  because nothing tracks which node the pointer is over. The audit rule that
  would refuse a node declaring both `read_only` and `disabled` does not exist
  yet either.
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
- `frame::digest::frame_bytes` and `frame::digest::leaf_hash` are public: when
  two frames that should match do not, the viewport stream and each
  placement's own leaf hash can be diffed directly instead of guessing at the
  digest.
