-- Lua mirror of `page/common.rs`'s `body`/`row`/`column`/`filled_body`
-- (spec 013 T008a).
--
-- These four are raw `Grid`/`Stack` builders private to the gallery
-- **binary**, not `gorgon_petra::component` constructors: no registry row
-- names them and phases 1 and 2 of this spec could not have given them a
-- `ctx.ui` name no matter how many more atoms they registered. So every one
-- of the 57 pages that calls one of these needs this file's version too, and
-- this file is hand-written from `page/common.rs`'s own source, not derived
-- from it.
--
-- **This is the one genuine second implementation in the whole project.**
-- Every `ctx.ui.<component>` wrapper elsewhere in this codebase
-- (`atoms.lua`, `form.lua`, ...) names a registry row and the row calls the
-- exact same Rust constructor a page calls directly — `lua_parity.rs`'s own
-- module doc explains why that makes most of the T008 equality test compare
-- `f(x)` to `f(x)` and unable to fail on drift. These four functions are the
-- one place that is not true: `column`'s `Grid` shape, `row`'s `Stack`
-- shape and `body`'s constraint are typed out here by hand, in a second
-- language, and a token or a field renamed on one side and not the other is
-- exactly the kind of drift the equality test exists to catch. Keep it
-- honest: read `page/common.rs` again before editing this file, don't
-- infer its shape from what "looks right" in Lua.
--
-- Not part of `ctx.ui`: it does not go in `builders.lua` or any of the six
-- `MODULES` group files `gorgon_kernel_lua::ui::install` loads, because
-- shipping the gallery's own private layout choices as public plugin
-- surface would be a second, unrelated mistake riding along with this one.
-- `lua_parity.rs` loads this file directly, by path, the same way it loads
-- each page source under `lua_parity/pages/`.
--
-- Takes the already-installed `ui` table (`gorgon_kernel_lua::ui::install`'s
-- return value) as its one argument rather than reading a global, so a
-- caller controls exactly which `ui` these four builders compose from —
-- the same reason every `MODULES` group file in `builders.lua`'s own
-- `install` takes `(base, factory)` as parameters instead of reaching for a
-- global.
return function(ui)
  local M = {}

  -- `page/common.rs::column`: a single-column `Grid` whose implicit rows are
  -- `FitContent`, not a `Stack` dividing an exact height by equal share —
  -- see that function's own doc comment for why a `Grid` is required here.
  function M.column(key, spacing, children)
    return ui.node.grid({
      key = key,
      columns = { ui.track.weight(1.0) },
      row_gap = spacing,
      children = children,
    })
  end

  -- `page/common.rs::row`: a horizontal `Stack`, centred on the cross axis.
  function M.row(key, spacing, children)
    return ui.node.stack({
      key = key,
      axis = "horizontal",
      gap = spacing,
      align = "center",
      children = children,
    })
  end

  -- `page/common.rs::body`: `column`, with its vertical constraint given
  -- priority 1 so it outranks a `section`'s title when the section divides
  -- its height.
  function M.body(key, spacing, children)
    local node = M.column(key, spacing, children)
    node.constraints = { vertical = { priority = 1 } }
    return node
  end

  -- `page/common.rs::filled_body`: `body`, stretched to the column's width.
  -- `M.column` always attaches a non-empty `props` (its `columns` entry is
  -- never empty), so `node.props` exists here to set `align` on.
  function M.filled_body(key, spacing, children)
    local node = M.body(key, spacing, children)
    node.props.align = "stretch"
    return node
  end

  return M
end
