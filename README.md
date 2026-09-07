# GOrgOn-Petra

Petra is a retained view-tree layout engine. It computes petrified frames
(immutable, hashed layout snapshots), the transitions between them, and a
semantic tree used for accessibility and driven testing.

## Crates

- `gorgon-petra`: the layout engine itself. Depends only on `blake3` and
  `serde`. It knows nothing about any GUI toolkit.
- `gorgon-petra-egui`: the only crate in this workspace that knows egui. It
  hosts an `eframe` application, paints layers, shapes text, and translates
  input into Petra's event model.
- `gorgon-petra-testkit`: a driver for Petra applications. A semantic-tree
  query server, settle-blocking actions, and identity-verified screenshots,
  for end-to-end tests that act like a real user rather than calling
  handler functions directly.

## Provenance

This repository is filtered out of the private GOrgOn monorepo, where Petra
is developed alongside the rest of GOrgOn (an agent operating system built
on an immutable Rust kernel). Commit history for these three crates is
preserved; everything else in the monorepo, including `gorgon-kernel` and
`gorgon-inspector`, is excluded.

`gorgon-petra-testkit` originally carried a dev-dependency on
`gorgon-inspector` for three integration tests
(`inspector_journey`, `inspector_reconnect`, `semantic_audit`) that drove a
real `gorgon-inspector` shell against a real `gorgond` daemon. Both of those
crates live only in the monorepo, so that dependency and those three tests
were removed here rather than shipped broken or silently skipped. That
coverage still exists in the monorepo.

One further test was removed for the same reason. The gallery's
`inventory.rs` carries a 42-row Carbon component scaffold, and the
monorepo checks it row for row against a research document outside these
three crates. That document is monorepo-only, so
`the_rows_match_the_checked_in_inventory` does not run here. The three
tests that check the scaffold against itself do still run, and the
removal is documented at the point it was removed.

## License

Licensed under either of Apache License, Version 2.0
(`LICENSE-APACHE`) or MIT license (`LICENSE-MIT`) at your option.
