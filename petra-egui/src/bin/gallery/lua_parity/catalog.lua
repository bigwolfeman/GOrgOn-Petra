-- Lua source for the gallery chrome (spec 013 T010).
--
-- Hand-written mirror of what `catalog.rs` builds: `Catalog::view()` plus
-- `index_pane`, `index_row`, `page_body` and `page_body_raw`, over
-- `inventory.rs`'s `ROWS` as the roster they read. Same rules as
-- `pages/*.lua` and `common.lua`: written from the Rust source read alongside
-- this file, not derived from it — read `catalog.rs` and `inventory.rs`
-- again before editing anything here.
--
-- Proven by `lua_parity.rs`'s
-- `lua_catalog_chrome_builds_the_same_tree_as_the_rust_one`, which compares
-- this chunk's tree against `Catalog::view()` node for node. As with the
-- pages, what that proves is reachability, param fidelity and
-- transcription, plus real drift where this file types raw nodes by hand
-- (below) — the registry-backed constructors still compare `f(x)` to
-- `f(x)`, and `lua_parity.rs`'s module doc is the place that argument is
-- made in full.
--
-- # What crosses as a name, and what is typed by hand here
--
-- `heading`/`text`/`button`/`content_switcher`/`content_switcher_item`/
-- `list_row` are registry rows (`ui.<name>`), so those compare one Rust
-- constructor to itself. The chrome's own raw nodes are not: the two
-- `Scroll`s (`index`, `main-scroll`), `page`'s and `shell`'s `Grid`s, plus
-- `common.column`/`common.row`'s scaffolding, are transcribed field by
-- field — `axis`, `overscan`, `column_gap`, `row_gap`, `padding`, `tokens`,
-- `align`, `constraints`, `semantics`/`interactions` on the two scrolls —
-- and that transcription is where real drift can appear.
--
-- Two values need a registry modifier rather than a plain constructor or a
-- field set beside one, because `registry::expand_node` returns `build(...)`
-- whole and discards every node field set on a component reference
-- (`common.lua`'s `wrapped` doc says the same for `props.wrap`):
--
-- * `ui.padded` — `catalog.rs::index_row` puts the pane's own density over
--   `list_row`'s Carbon inset (`component::padded`'s doc).
-- * `ui.seat_card` — the deep re-seat `catalog.rs` used to walk privately
--   (`component::seat_card`'s doc). Rust and Lua call the same function, so
--   the walk is not a second implementation.
--
-- # The `state` global
--
-- Unlike `pages/*.lua`, this chunk reads one extra global, set by the
-- harness before it is evaluated: `state = { open = <inventory row name>,
-- theme = "dark" | "light", body = <the open page's body tree> }`. `body` is
-- `lua_parity/pages/<slug>.lua`'s own return value — the page body is the
-- page's mirror (T009), not this file's, and a chrome that rebuilt it here
-- would be a stand-in with two homes. `open` is the row NAME, matching
-- `Catalog::on_page`.
--
-- # What is deliberately NOT here
--
-- `seat_index_focus`, scrolling and event routing are host-side (`Host`,
-- `route`, the focus tree) and are contributed by no Lua tree — out of the
-- Lua half by construction, not forgotten.

local IDX = "idx-"
local INDEX_WIDTH = 240.0

-- `inventory.rs`'s `ROWS`, row for row (57): number, component, slice
-- letter, and what `cell.rs`'s `content_for`/`Cell::is_built` answers for it
-- today. Every row is built while `content_for` maps its name; the column is
-- where a future row says otherwise, and the equality test is what notices
-- the roster moved.
local ROSTER = {
  { number = 1, component = "Accordion", slice = "A", built = true },
  { number = 2, component = "AI label", slice = "A", built = true },
  { number = 3, component = "Breadcrumb", slice = "A", built = true },
  { number = 4, component = "Button", slice = "A", built = true },
  { number = 5, component = "Checkbox", slice = "A", built = true },
  { number = 6, component = "Code snippet", slice = "A", built = true },
  { number = 7, component = "Contained list", slice = "A", built = true },
  { number = 8, component = "Content switcher", slice = "B", built = true },
  { number = 9, component = "Data table", slice = "B", built = true },
  { number = 10, component = "Date picker", slice = "B", built = true },
  { number = 11, component = "Dropdown", slice = "B", built = true },
  { number = 12, component = "File uploader", slice = "B", built = true },
  { number = 13, component = "Form", slice = "B", built = true },
  { number = 14, component = "Inline loading", slice = "B", built = true },
  { number = 15, component = "Link", slice = "C", built = true },
  { number = 16, component = "List", slice = "C", built = true },
  { number = 17, component = "Loading", slice = "C", built = true },
  { number = 18, component = "Menu", slice = "C", built = true },
  { number = 19, component = "Menu buttons", slice = "C", built = true },
  { number = 20, component = "Modal", slice = "C", built = true },
  { number = 21, component = "Notification", slice = "C", built = true },
  { number = 22, component = "Number input", slice = "D", built = true },
  { number = 23, component = "Pagination", slice = "D", built = true },
  { number = 24, component = "Popover", slice = "D", built = true },
  { number = 25, component = "Progress bar", slice = "D", built = true },
  { number = 26, component = "Progress indicator", slice = "D", built = true },
  { number = 27, component = "Radio button", slice = "D", built = true },
  { number = 28, component = "Search", slice = "D", built = true },
  { number = 29, component = "Select", slice = "E", built = true },
  { number = 30, component = "Slider", slice = "E", built = true },
  { number = 31, component = "Structured list", slice = "E", built = true },
  { number = 32, component = "Tabs", slice = "E", built = true },
  { number = 33, component = "Tag", slice = "E", built = true },
  { number = 34, component = "Text input", slice = "E", built = true },
  { number = 35, component = "Tile", slice = "E", built = true },
  { number = 36, component = "Toggle", slice = "F", built = true },
  { number = 37, component = "Toggletip", slice = "F", built = true },
  { number = 38, component = "Tooltip", slice = "F", built = true },
  { number = 39, component = "Tree view", slice = "F", built = true },
  { number = 40, component = "UI shell header", slice = "F", built = true },
  { number = 41, component = "UI shell left panel", slice = "F", built = true },
  { number = 42, component = "UI shell right panel", slice = "F", built = true },
  { number = 43, component = "Avatar", slice = "G", built = true },
  { number = 44, component = "Button group", slice = "G", built = true },
  { number = 45, component = "Context menu", slice = "G", built = true },
  { number = 46, component = "Drawer", slice = "G", built = true },
  { number = 47, component = "Input group", slice = "G", built = true },
  { number = 48, component = "Menubar", slice = "G", built = true },
  { number = 49, component = "OTP", slice = "G", built = true },
  { number = 50, component = "Rating", slice = "G", built = true },
  { number = 51, component = "Textarea", slice = "G", built = true },
  { number = 52, component = "Toggle button", slice = "G", built = true },
  { number = 53, component = "Combobox (compound)", slice = "G", built = true },
  { number = 54, component = "Command (compound)", slice = "G", built = true },
  { number = 55, component = "Calendar (compound)", slice = "G", built = true },
  { number = 56, component = "Data table (compound)", slice = "G", built = true },
  { number = 57, component = "Selection palette (compound)", slice = "G", built = true },
}

-- `catalog.rs::Catalog::on_page` panics by name; this names the mistake too,
-- because a mirror that silently drew row 1 would hide the error it is here
-- to catch.
local function row_for(name)
  for _, row in ipairs(ROSTER) do
    if row.component == name then
      return row
    end
  end
  error("catalog.lua: no inventory row is named " .. tostring(name), 2)
end

-- `catalog.rs::index_row`: `list_row` with the pane's own density put back
-- over it. The padding is a `padded` modifier, not a field beside the call:
-- the component reference discards node fields (see the module doc). The
-- wrapper's key names only the wire reference — `registry::expand` reads
-- nothing back out of it, the same documented shape as `ui.disabled`.
local function index_row(row, selected)
  return ui.padded(IDX .. row.number, {
    node = ui.list_row({
      key = IDX .. row.number,
      label = string.format("%2d  %s", row.number, row.component),
      selected = selected,
    }),
    padding = {
      top = "spacing-02",
      right = "spacing-03",
      bottom = "spacing-02",
      left = "spacing-03",
    },
  })
end

-- `catalog.rs::index_pane`: the scrolling `Scroll` and its one-column
-- `Grid` of 57 rows.
local function index_pane(open_number)
  local rows = {}
  for _, row in ipairs(ROSTER) do
    rows[#rows + 1] = index_row(row, row.number == open_number)
  end
  -- `list.props.align = Some(Align::Stretch)`: without it each row is
  -- measured at its own text width and the sidebar stair-steps
  -- (`catalog.rs::index_pane`'s own comment).
  local list = common.column("rows", "spacing.2xs", rows)
  list.props.align = "stretch"
  return ui.node.scroll({
    key = "index",
    axis = "vertical",
    overscan = 64.0,
    semantics = { role = "scroll", label = "Component index" },
    interactions = { "scroll" },
    tokens = { background = "surface.raised" },
    constraints = { horizontal = { min = INDEX_WIDTH, max = INDEX_WIDTH } },
    children = { list },
  })
end

-- `catalog.rs::page_body_raw`, then its re-seat: `component::seat_card`, the
-- same walk the Rust side calls.
local function page_body(state, row)
  local raw
  if row.built then
    assert(
      state.body ~= nil,
      "catalog.lua: " .. row.component
        .. " is built, so state.body must carry its page's body tree (lua_parity/pages/)"
    )
    raw = state.body
  else
    -- `page_body_raw`'s unbuilt arm, verbatim. Unreachable while every
    -- roster row is built; kept because the Rust arm exists and a mirror
    -- that dropped it would disagree the day one row goes unbuilt.
    raw = common.wrapped("unbuilt", string.format(
      "This inventory row is unbuilt. No Petra component exists for %s. Anatomy, variants, sizes, and states live in slice-%s.md. This page does not draw a stand-in.",
      row.component,
      string.lower(row.slice)
    ))
  end
  return ui.seat_card("page-body", { node = raw })
end

assert(
  type(state) == "table",
  "catalog.lua: the harness must set the `state` global before evaluating this chunk: "
    .. "{ open = <inventory row name>, theme = \"dark\" | \"light\", body = <the page's body tree> }"
)
assert(
  type(state.open) == "string",
  "catalog.lua: state.open is the open inventory row's NAME (Catalog::on_page's spelling)"
)
assert(
  state.theme == "dark" or state.theme == "light",
  "catalog.lua: state.theme is \"dark\" or \"light\" (got " .. tostring(state.theme) .. ")"
)
local row = row_for(state.open)

-- `Catalog::view()`'s three derived strings.
local title = string.format("%d  %s", row.number, row.component)
local slice = row.slice == "G" and "spec 009" or "slice " .. row.slice
local status = row.built and "BUILT" or "UNBUILT"

local main = common.column("main", "spacing.xl", {
  ui.heading({ key = "title", label = title }),
  common.column("meta", "spacing.sm", {
    ui.text({ key = "slice", label = slice }),
    ui.text({ key = "status", label = status }),
  }),
  common.row("nav", "spacing.md", {
    ui.button({ key = "prev", label = "Prev" }),
    ui.button({ key = "next", label = "Next" }),
    ui.content_switcher({
      key = "theme",
      children = {
        ui.content_switcher_item({
          key = "theme-dark",
          label = "Dark",
          selected = state.theme == "dark",
        }),
        ui.content_switcher_item({
          key = "theme-light",
          label = "Light",
          selected = state.theme == "light",
        }),
      },
    }),
  }),
  page_body(state, row),
})
-- `pad("spacing.xl", "spacing.lg")` is `InsetRefs::symmetric` over token
-- names, horizontal first: top/bottom the vertical step, left/right the
-- horizontal one (`component::kit::pad`'s own spelling).
main.props.padding = {
  top = "spacing.lg",
  right = "spacing.xl",
  bottom = "spacing.lg",
  left = "spacing.xl",
}

local main_scroll = ui.node.scroll({
  key = "main-scroll",
  axis = "vertical",
  overscan = 64.0,
  semantics = { role = "scroll", label = "Page content" },
  interactions = { "scroll" },
  children = { main },
})

-- Both `shell` rows are `Weight`, so the index cell gets a bounded viewport
-- to scroll within (`catalog.rs`'s own comment on the shape).
local shell = ui.node.grid({
  key = "shell",
  columns = { ui.track.fixed(INDEX_WIDTH), ui.track.weight(1.0) },
  rows = { ui.track.weight(1.0) },
  column_gap = "spacing.md",
  tokens = { background = "surface.base" },
  children = { index_pane(row.number), main_scroll },
})

-- `page` is a plain 1x1 `Grid`, not a `Scroll`: a `Scroll` offers its child
-- `Unbounded` on the scrolling axis and would defeat `shell`'s `Weight` row
-- (`catalog.rs`'s own comment).
return ui.node.grid({
  key = "page",
  columns = { ui.track.weight(1.0) },
  rows = { ui.track.weight(1.0) },
  tokens = { background = "surface.base" },
  children = { shell },
})
