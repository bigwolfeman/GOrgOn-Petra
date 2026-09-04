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
- `catalog.rs` — `Catalog`, the application: roster, page state, chrome (nav
  list, Prev/Next, the page header). Parent-owned; component groups do not
  edit it.
- `cell.rs` — one `Cell` per inventory row, and `Content` saying what that row
  can hand back. `BUILT` is a test-only hand-written cross-check against
  `xtask`'s `BUILT_COMPONENTS`; keep the two in lockstep.
- `inventory.rs` — the 42 rows, in order. The source of truth for row numbers.

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

`click`, `hover`, `focus`, `type_into`, `key`, `scroll`. Nodes are named by the
tail of their id, so `"btn-ghost"` finds `/page/.../kinds/btn-ghost`. A tail
that matches nothing panics listing every placed id, which is usually the
fastest way to learn what a page actually built.

Set `PETRA_SHOT_DIR` and every `shoot` writes a PNG there. **Read the PNGs.**

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
