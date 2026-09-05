# gallery — the 42-row Carbon catalog

The window the operator opens to see what Petra draws. One page per Carbon
inventory row.

## Look at it, do not infer it

`catalog.rs`'s test module holds the only pixel-level check in this workspace:

```sh
PETRA_SHOT_DIR=/tmp/shots cargo test -p gorgon-petra-egui --bin gallery \
    every_built_page_rasterizes_to_more_than_one_colour
```

It rasterizes all 42 pages through a real `wgpu` device (headless, no window)
and writes a PNG each. Read the PNGs. The assertion itself is deliberately weak
— more than one distinct colour, which catches a blank page and nothing else —
because the value here is the capture, not the assert.

**Do not add a frame-record assertion in place of looking.** This catalog
shipped with ragged rows, cells touching with no gap, and a nav indicator stuck
on row 1 while the highlight sat on row 23, under a full green suite. A rect
with zero spacing beside another rect is a legal frame record.

## Layout

- `main.rs` — the eframe entry point.
- `catalog.rs` — `Catalog`, the chrome only: roster, open page, index pane,
  Prev/Next, the page header, `seat_index_focus`. It holds no control state
  and names no component; it reaches the open page through `page::Page`.
  Parent-owned; component groups do not edit it.
- `page/` — one module per inventory row (`page/toggle.rs` is row 36). Each
  module owns its state struct (private fields), its node-id constants, its
  `body()`, its `handle()` and its own tests. A fix to one row edits that
  row's file and nothing else.
  - `page/mod.rs` — the `Page` trait, and `all()`, the roster of pages in
    inventory order. Adding a row is one new file, one line in `all()`, and
    the row in `inventory.rs`. `Page::handle` receives the **full** routed
    node path, not its last segment: a press on a knob names the knob, so
    match with `common::path_has`. The chrome asks the open page before its
    own Prev/Next, because Pagination's `pager/next` shares a segment with
    the chrome's `next`. `Page::gesture` receives the same path plus the
    frame the route was computed against, for the pages with a drag; the
    chrome offers it before the activation filter that gates `handle`. A
    pointer move or pointer-exit that routed **nowhere** also reaches
    `gesture`, with an **empty** path — that is how a hover-revealed
    surface (row 38) learns the pointer left; `path_has("", key)` is false
    for every key, so every other page ignores it. `Page::dismissed(ids)`
    is how a `DismissOutside` surface closes on an outside press; the chrome
    delivers it **after** `handle` has seen the press, so a press on an open
    trigger toggles it shut and the dismissal finds it closed rather than
    closing it and letting the toggle reopen it. Match your surface with
    `common::dismisses`.
  - `page/common.rs` — what several pages share: `sp`, `tok`, `column`,
    `row`, `body`, `wrapped`, `path_has`, `dismisses`, and the test-only
    `find`. Import from here; do not copy.
- `cell.rs` — one `Cell` per inventory row, and `Content` saying what that row
  can hand back. `BUILT` is a test-only hand-written cross-check against
  `xtask`'s `BUILT_COMPONENTS`; keep the two in lockstep.
- `inventory.rs` — the 42 rows, in order. The source of truth for row numbers.
- `shots.rs` — the headless `Camera` driver (test-only); see below.

## Watch for

- **Chrome bugs read as component bugs.** A ragged nav row or a stale indicator
  is `catalog.rs`, not the component on the page.
- The nav list scrolls by wheel and by Tab (`gorgon-petra-egui`'s
  `Host::apply_scroll`/`Host::step_focus`), so all 42 rows are reachable —
  see `catalog.rs`'s `wheeling_over_the_index_pane_...` and
  `tab_walks_all_forty_two_index_rows_...` tests. Prev/Next's
  `seat_index_focus` still does not scroll to an off-fold row; see its doc.

## Driving a page, not just photographing one

`shots.rs` is a headless driver bolted to the camera. It opens any page by its
inventory row name, sends real input through the same translator and routing a
physical device goes through, and photographs the frame that results:

```rust
let mut cam = Camera::on("Dropdown");
cam.click("dropdown-trigger");
cam.shoot("11-dropdown-open");
```

`click`, `hover`, `focus`, `type_into`, `key`, `scroll`, `drag`, and `click_at`
/ `hover_at` for a raw position (pressing *outside* a menu is what dismisses
it). Each of those is **one** pass: `drag` delivers press, both waypoints and
the release in a single `RawInput`, which is right when you only ask where a
gesture finished and blind to anything that needs more than one pass to go
wrong.

`press` / `move_to` / `release` are the same gesture at one pass per event, the
way the window pumps it, and `live_motion()` stops reducing motion so the
160 ms focus caret and the 140 ms toggle slide actually run. Reach for those
four whenever the question is *when* a page paints rather than *what* it paints.
Row 30 is why they exist: a slider drag repainted the stale frame for nine
frames at a time under the caret hop, the handle trailed the pointer by up to
56 units, and a one-pass drag with motion reduced could not see any of it. Every
pass also advances a deterministic 60 Hz clock, so a driven step is a frame
later than the one before it instead of microseconds later. Nodes are named by the tail of their id, so `"btn-ghost"` finds
`/page/.../kinds/btn-ghost`; a tail shared by two placements (a section and a
surface both keyed `menu`) panics as ambiguous, so use a longer one. A tail
that matches nothing panics listing every placed id, which is usually the
fastest way to learn what a page actually built. `rect(tail)` reads a placed
rect back, which is how a drag works out where to let go. `tree()` is the open
page's own `ViewNode` tree, for a leaf's text the frame does not carry; it is
never a substitute for the picture:

```rust
let rail = cam.rect("/row/rail");
cam.drag("/rail/handle", Point::new(rail.x + rail.w * 0.8, rail.y + 1.0));
```

A drag reaches the page through `Page::gesture(event, node, frame)`, a hook
with a default that consumes nothing; `page/slider.rs` is the one override
and `gorgon_petra::component::slider_value_at` is the arithmetic. Verified end
to end 2026-09-04 by `shots::tests::dragging_the_slider_handle_moves_the_fill_the_way_the_pointer_went`.

Set `PETRA_SHOT_DIR` and every `shoot` writes a PNG there. **Read the PNGs.**

Captures are **2 device pixels per point**, so a 1200x900 page is a 2400x1800
file. Two reasons. Half-pixel defects only exist at 1x, so photographing a page
at both scales tells a snapped mark apart from a mis-placed one — that is how
the radio dot was diagnosed. And the IBM Carbon reference app under
`ignored/carbon-ref/` captures at device pixel ratio 2, so a Petra shot and a
Carbon shot of the same component are already the same size and go side by side
without a resample. `Camera::at_scale` picks a different ratio when you want
one.

A resting-state photograph cannot tell a working dropdown from a dead one. That
blindness is how 39 of 42 pages reached the operator broken under a green suite,
and it is the whole reason this driver exists. Any page whose defect is "does
not work" must be proven with a driven shot.

Two smoke tests hold the driver itself honest: a click and a hover each have to
change what the page rasterizes to. If those go red, no picture taken after a
driving step means anything.

**Hover works end to end** — verified 2026-09-04, not inferred. The host derives
the hovered placement and a `background@hover` binding repaints. A primary
button is the exception and is not a counter-example: `button::chrome` gives it
`hover: None` deliberately, because Carbon's `$button-primary-hover` has no name
in Petra's vocabulary and inventing the tone in a component would be a colour
decision in the wrong layer.
