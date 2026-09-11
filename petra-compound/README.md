# gorgon-petra-compound

The `Compound` trait: the pure triple a view-fiber hosts. No kernel, no
egui, no tokio — this crate depends on `gorgon-petra` and `serde` only. A
compound that needs those is not a compound.

Binding contract: `specs/009-petra-design-language/contracts/view-fiber.md`
§4.1. Tier 3 in that contract's three-tier split: tier 1 is a pure
`fn(key, …) -> ViewNode` in `gorgon-petra` itself, tier 2 is engine-derived
interaction state (`Host`, `LayoutState`), and tier 3 is state an operator
would be surprised to lose on reopening the shell — a sort column, an open
overlay, a filter query. `Compound` is the shape that state takes:

```rust
pub trait Compound {
    type Props;   // The author supplies it. The compound never writes it.
    type State;   // The compound writes it. The author never supplies it.
    type Intent;  // The closed set of things that can happen to it.
    type Request; // What it asks its host to do. `()` when it asks nothing.

    fn init(props: &Self::Props) -> Self::State;
    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request>;
    fn view(state: &Self::State, props: &Self::Props) -> gorgon_petra::tree::ViewNode;
}
```

`update` is pure and total: it never performs an effect itself, only
returns requests for the host fiber (`gorgon-view-fiber`) to carry out
through `Ctx`. A pure function cannot own an inverse (Constitution
Principle II), so a pure function may not mutate the world.

## The five shipped compounds

| Compound | `Props` | What `State` owns |
|---|---|---|
| `calendar::Calendar` | label, selection `Mode` | month on show, selected days, range anchor |
| `combobox::Combobox` | label, candidate items | query, highlight, open-or-shut |
| `command::Command` | commands (`Item`) | query, highlight, open-or-shut |
| `data_table::DataTable` | columns, rows | sort, selection, expansion |
| `selection_palette::SelectionPalette` | none | bold/italic marks, overlay point |

Each builds its `view` by composing `gorgon_petra::component` kit
constructors under hand-written node keys — a row id doubling as a node
key, a column id doubling as a header key, a filtered-list index doubling
as `opt-{n}`. `filter::filter_indices` is the one piece of shared,
non-compound logic: a total `(items, query) -> Vec<usize>` substring match
that `Combobox::view` and `Command::view` both call to narrow `Props`
before building the tree.

## Known Limitations and Deferred Work

- No shipped compound is hosted by a real `gorgon-view-fiber::ViewFiber<C>`
  anywhere in this tree. `gorgond/tests/view_fiber_reattach.rs` drives
  `DataTable::init` / `update` / `view` by hand and documents that the
  actual `ViewFiber::<DataTable>::new` call only appears inside a
  `compile_fail` doc block, because `gorgond` does not depend on
  `gorgon-view-fiber`. Every compound in this crate is exercised only
  through its own unit tests, never through the loop that will actually
  run it.
- `filter.rs`'s own module doc says `Combobox` and `Command` call
  `filter_indices` "from their own `Compound::update`." Both actually call
  it from `view`: `combobox.rs` and `command.rs` each say so explicitly
  ("applied in `Combobox::view` because `update` does not see `Props`"),
  and the call sites confirm it. The module doc is stale and should be
  fixed with the next edit to `filter.rs`.
- `calendar.rs` duplicates `days_in_month` from `component::date_picker`
  because that helper is crate-private there. Two Gregorian tables now
  exist; a leap-year rule change would need both edited in step and
  nothing catches a drift between them.
- `SelectionPalette::Props` is a unit struct: the point the overlay
  anchors to is a hardcoded gallery value in `State`, not yet a real text
  selection. `selection_palette.rs`'s own doc names spec 006 as the
  prerequisite for a real anchor.
- `Command`'s own doc says it needs its own fiber later, because a global
  keybinding reaches it rather than a form field. Nothing in this crate or
  `gorgon-view-fiber` wires that binding yet; the triple is complete, the
  host is not.
- The invariant companion (`src/invariant.rs`) only asserts violations
  `gorgon_petra::tree::Violation::judgeable_standalone` reports — a
  duplicate sibling key among them, but not an unregistered custom kind,
  transition or design token. Those depend on a host `Registry` this crate
  does not own, so a compound could still fail `gorgond`'s stage-1
  acceptance for a reason this check cannot see.

## Agent Experience

- Every compound's `update` is pure and total, so it is testable with no
  daemon, no kernel, and no async runtime — call `init`, then `update`
  with an `Intent`, and read `State` back directly.
- `src/invariant.rs` builds each shipped compound's real `init` → `view`
  output and runs it through `gorgon_petra::tree::validate`, so a
  hand-written duplicate key introduced while composing kit constructors
  fails `cargo test` with the offending key path, instead of surfacing
  only when a driver clicks the tree.
