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
- The nav list does not scroll, so rows past the window height are unreachable.
