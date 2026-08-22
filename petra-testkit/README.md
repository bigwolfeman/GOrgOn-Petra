# gorgon-petra-testkit

The Petra UI driver: a semantic-tree query server, settle-blocking synthetic
input, and identity-verified screenshots, over an owner-only Unix socket
speaking NDJSON. One protocol, used by gates, by tests, and by agents alike
(`specs/003-petra-layout-engine/contracts/driver-protocol.md`).

The server exists only when the `testkit` cargo feature is on. Production
binaries do not contain it, and there is no runtime flag that turns it on.

## Known Limitations and Deferred Work

- Nothing is implemented yet (tasks T029-T037). The crate exists so the
  workspace member, its gates, and its feature flag are in place; it currently
  exports no server, no client, and no finders.
- The snapshot lane will mirror `egui_kittest`'s wgpu readback machinery rather
  than depend on it (R3), so a `egui_kittest` change does not silently change
  what our gates capture — at the cost of tracking that machinery by hand.
- Screenshots are identity-verified against a frame's `(seq, digest)`, which
  covers placements, not pixels. A driver-level bug that produces the right
  geometry and the wrong colours is the parity lane's to catch, not this
  crate's.

## Agent Experience

This crate is how an agent sees its own work.

- `tree` and `screenshot` are the capture mechanism for UI changes (A-07/P-16):
  the same path a gate uses, so an agent's self-check and CI's check cannot
  disagree.
- Actions go through the real input router — hit-testing, focus, event
  ordering — so an action that a human could not perform is one an agent cannot
  perform either.
- A stale node id fails with `stale-node` naming the id. It never falls through
  to coordinates, which is the failure mode that makes a passing test meaningless.
