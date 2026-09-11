//! Registry rows: containment. See [`super`] for why this file exists.
//!
//! Each row names a public constructor in `petra/petra/src/component/`,
//! deserializes its parameter table into a shape, and calls the shipped
//! constructor. A row must never re-derive what the constructor does.
//!
//! Components: Accordion, Contained list, Context menu, List, Modal,
//! Popover, Structured list, Tile, Toggletip, Tooltip, Tree view, Code
//! snippet. 41 constructors (see the `coverage` test at the bottom, which
//! counts the source rather than trusting this comment).

use serde::Deserialize;
use serde_json::Value;

use super::Entry;
use crate::component::params::{
    KeyChildren, KeyChildrenSelected, KeyLabel, KeyLabelChildren, KeyLabelSelected, KeyLabelValue,
    ParamError, ParamShape,
};
use crate::component::{
    Bullet, BulletScheme, accordion, accordion_item, accordion_item_lg, accordion_item_sm,
    accordion_item_spaced, accordion_item_spaced_lg, accordion_item_spaced_sm, accordion_item_with,
    accordion_item_with_spaced, accordion_spaced, clickable_tile, code_snippet,
    code_snippet_inline, code_snippet_multi, code_snippet_multi_capped, contained_list,
    contained_list_disclosed, context_menu, expandable_tile, list_item, list_item_with, modal,
    modal_passive, ordered_list, popover, popover_with, popover_with_placement, selectable_tile,
    structured_list, structured_list_row, structured_list_sized, tile, toggletip, toggletip_with,
    tooltip, tooltip_anchored, tree_item, tree_item_xs, tree_view, unordered_list,
    unordered_list_with,
};
use crate::tree::{Align, Edge, Key, ViewNode};

/// Deserialize `params` into `T`, naming the constructor being built.
/// `serde_json`'s own error already names the offending field under
/// `deny_unknown_fields` or a missing key, which is what makes G4 hold
/// without any hand-written field matching here.
fn parse<T: for<'de> Deserialize<'de>>(
    component: &'static str,
    params: &Value,
) -> Result<T, ParamError> {
    serde_json::from_value(params.clone()).map_err(|e| ParamError {
        component: component.to_owned(),
        reason: e.to_string(),
    })
}

// --- one-off shapes, used only by this group -------------------------------

/// Wire form of [`Bullet`]: the real enum has no serde support.
///
/// 1 constructor (`unordered_list_with`, nested in [`BulletSchemeParam`]).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum BulletParam {
    Dash,
    Disc,
    Circle,
    Square,
}

impl From<BulletParam> for Bullet {
    fn from(b: BulletParam) -> Self {
        match b {
            BulletParam::Dash => Bullet::Dash,
            BulletParam::Disc => Bullet::Disc,
            BulletParam::Circle => Bullet::Circle,
            BulletParam::Square => Bullet::Square,
        }
    }
}

/// Wire form of [`BulletScheme`]: the real enum has no serde support.
/// Internally tagged on `type`, matching `M.track`'s convention
/// (`gorgon/kernel-lua/src/ui/builders.lua`), so every variant — including
/// the two unit ones — is a table, never a bare string.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
enum BulletSchemeParam {
    Carbon,
    Rotating,
    Fixed { bullet: BulletParam },
}

impl From<BulletSchemeParam> for BulletScheme {
    fn from(s: BulletSchemeParam) -> Self {
        match s {
            BulletSchemeParam::Carbon => BulletScheme::Carbon,
            BulletSchemeParam::Rotating => BulletScheme::Rotating,
            BulletSchemeParam::Fixed { bullet } => BulletScheme::Fixed(bullet.into()),
        }
    }
}

/// `accordion_item`, `accordion_item_sm`, `accordion_item_lg`,
/// `accordion_item_spaced`, `accordion_item_spaced_sm`,
/// `accordion_item_spaced_lg`, `expandable_tile` — 7 constructors sharing
/// one `(key, label, bool, body)` shape under different field names for the
/// bool (`expanded` here).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelExpandedBody {
    key: Key,
    label: String,
    expanded: bool,
    body: String,
}
impl ParamShape for KeyLabelExpandedBody {
    const LUAU: &'static str = "{ key: string, label: string, expanded: boolean, body: string }";
}

/// `accordion_item_with`, `accordion_item_with_spaced` — 2 constructors.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelExpandedChildren {
    key: Key,
    label: String,
    expanded: bool,
    // An empty Lua table crosses as `{}`, a JSON object, because Lua cannot
    // tell an empty list from an empty map; `#[serde(default)]` supplies the
    // empty `Vec` when the field is missing rather than refusing an omitted
    // (never a present-but-`{}`) `children`.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for KeyLabelExpandedChildren {
    const LUAU: &'static str =
        "{ key: string, label: string, expanded: boolean, children: { ViewNode } }";
}

/// `unordered_list_with` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeySchemeChildren {
    key: Key,
    scheme: BulletSchemeParam,
    // See `KeyLabelExpandedChildren`'s `children` for why: an empty Lua
    // table has no `#[serde(default)]`-free way to reach a `Vec<ViewNode>`.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for KeySchemeChildren {
    const LUAU: &'static str = "{ key: string, scheme: { type: \"carbon\" } | { type: \"rotating\" } | \
         { type: \"fixed\", bullet: \"dash\" | \"disc\" | \"circle\" | \"square\" }, \
         children: { ViewNode } }";
}

/// `list_item_with` — 1 constructor. `nested` is the one optional field in
/// this group; absent or `null` both deserialize to `None`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelNested {
    key: Key,
    label: String,
    #[serde(default)]
    nested: Option<ViewNode>,
}
impl ParamShape for KeyLabelNested {
    const LUAU: &'static str = "{ key: string, label: string, nested: ViewNode? }";
}

/// `modal` — 1 constructor. Three distinct strings; no shared shape has
/// three, so this stays local rather than reusing a shape whose field names
/// (`label`/`value`) would mislabel `title`/`body`/`primary`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModalParams {
    key: Key,
    title: String,
    body: String,
    primary: String,
}
impl ParamShape for ModalParams {
    const LUAU: &'static str = "{ key: string, title: string, body: string, primary: string }";
}

/// `popover` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopoverParams {
    key: Key,
    label: String,
    anchor: Key,
    body: String,
}
impl ParamShape for PopoverParams {
    const LUAU: &'static str = "{ key: string, label: string, anchor: string, body: string }";
}

/// `popover_with` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopoverWithParams {
    key: Key,
    label: String,
    anchor: Key,
    // See `KeyLabelExpandedChildren`'s `children` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for PopoverWithParams {
    const LUAU: &'static str =
        "{ key: string, label: string, anchor: string, children: { ViewNode } }";
}

/// `context_menu(key, label, x, y, items)`. `x`/`y` are logical units, the
/// top-left of [`crate::tree::Anchor::Point`]. The list is `children` on
/// the wire, matching `menu`/`accordion` rather than the constructor's
/// `items` name: Lua cannot send an empty array as `[]`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextMenuParams {
    key: Key,
    label: String,
    x: f32,
    y: f32,
    // See `KeyLabelExpandedChildren`'s `children` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for ContextMenuParams {
    const LUAU: &'static str =
        "{ key: string, label: string, x: number, y: number, children: { ViewNode } }";
}

/// `popover_with_placement` — 1 constructor. `Edge` and `Align` already
/// derive `Deserialize` (`crate::tree::props`), so this shape uses them
/// directly rather than wrapping them.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PopoverWithPlacementParams {
    key: Key,
    label: String,
    anchor: Key,
    edge: Edge,
    align: Align,
    // See `KeyLabelExpandedChildren`'s `children` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for PopoverWithPlacementParams {
    const LUAU: &'static str = "{ key: string, label: string, anchor: string, \
         edge: \"top\" | \"bottom\" | \"left\" | \"right\", \
         align: \"start\" | \"center\" | \"end\", children: { ViewNode } }";
}

/// `structured_list` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredListParams {
    key: Key,
    // See `KeyLabelExpandedChildren`'s `children` for why both fields below
    // need `#[serde(default)]`.
    #[serde(default)]
    header: Vec<ViewNode>,
    #[serde(default)]
    rows: Vec<ViewNode>,
}
impl ParamShape for StructuredListParams {
    const LUAU: &'static str = "{ key: string, header: { ViewNode }, rows: { ViewNode } }";
}

/// `structured_list_sized` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredListSizedParams {
    key: Key,
    // See `KeyLabelExpandedChildren`'s `children` for why both fields below
    // need `#[serde(default)]`.
    #[serde(default)]
    header: Vec<ViewNode>,
    #[serde(default)]
    rows: Vec<ViewNode>,
    // The same empty-Lua-table ambiguity applies to any `Vec<T>`, not only
    // `Vec<ViewNode>`: `weights = {}` (meaning "use the default weighting")
    // crosses as `{}` too and needs the same escape hatch.
    #[serde(default)]
    weights: Vec<f32>,
    dividers: bool,
}
impl ParamShape for StructuredListSizedParams {
    const LUAU: &'static str = "{ key: string, header: { ViewNode }, rows: { ViewNode }, \
         weights: { number }, dividers: boolean }";
}

/// `toggletip` — 1 constructor. Same shape as [`KeyLabelExpandedBody`]
/// structurally, but the bool is named `open` on the real constructor, so a
/// plugin author sees `open` rather than a misleading `expanded`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelOpenBody {
    key: Key,
    label: String,
    open: bool,
    body: String,
}
impl ParamShape for KeyLabelOpenBody {
    const LUAU: &'static str = "{ key: string, label: string, open: boolean, body: string }";
}

/// `toggletip_with` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyLabelOpenChildren {
    key: Key,
    label: String,
    open: bool,
    // See `KeyLabelExpandedChildren`'s `children` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for KeyLabelOpenChildren {
    const LUAU: &'static str =
        "{ key: string, label: string, open: boolean, children: { ViewNode } }";
}

/// `tooltip_anchored` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyAnchorBody {
    key: Key,
    anchor: Key,
    body: String,
}
impl ParamShape for KeyAnchorBody {
    const LUAU: &'static str = "{ key: string, anchor: string, body: string }";
}

/// `tree_item`, `tree_item_xs` — 2 constructors.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TreeItemParams {
    key: Key,
    label: String,
    expanded: bool,
    selected: bool,
    // See `KeyLabelExpandedChildren`'s `children` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for TreeItemParams {
    const LUAU: &'static str = "{ key: string, label: string, expanded: boolean, \
         selected: boolean, children: { ViewNode } }";
}

/// `code_snippet_multi_capped(key, code, filename, language, expanded)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeSnippetCappedParams {
    key: Key,
    code: String,
    filename: String,
    language: String,
    expanded: bool,
}
impl ParamShape for CodeSnippetCappedParams {
    const LUAU: &'static str =
        "{ key: string, code: string, filename: string, language: string, expanded: boolean }";
}

macro_rules! row {
    ($name:literal, $shape:ty, |$p:ident| $body:expr) => {{
        fn ctor(v: &Value) -> Result<ViewNode, ParamError> {
            let $p: $shape = parse($name, v)?;
            Ok($body)
        }
        Entry {
            name: $name,
            ctor,
            luau: <$shape as ParamShape>::LUAU,
        }
    }};
}

/// This group's constructors: one row per public `ViewNode`-returning
/// constructor in the 12 containment components. 41 rows; see the
/// `coverage` test below, which counts the source rather than this list.
pub const ENTRIES: &[Entry] = &[
    row!("accordion", KeyChildren, |p| accordion(p.key, p.children)),
    row!("accordion_spaced", KeyChildren, |p| accordion_spaced(
        p.key, p.children
    )),
    row!("accordion_item", KeyLabelExpandedBody, |p| accordion_item(
        p.key, p.label, p.expanded, p.body
    )),
    row!("accordion_item_sm", KeyLabelExpandedBody, |p| {
        accordion_item_sm(p.key, p.label, p.expanded, p.body)
    }),
    row!("accordion_item_lg", KeyLabelExpandedBody, |p| {
        accordion_item_lg(p.key, p.label, p.expanded, p.body)
    }),
    row!("accordion_item_spaced", KeyLabelExpandedBody, |p| {
        accordion_item_spaced(p.key, p.label, p.expanded, p.body)
    }),
    row!("accordion_item_spaced_sm", KeyLabelExpandedBody, |p| {
        accordion_item_spaced_sm(p.key, p.label, p.expanded, p.body)
    }),
    row!("accordion_item_spaced_lg", KeyLabelExpandedBody, |p| {
        accordion_item_spaced_lg(p.key, p.label, p.expanded, p.body)
    }),
    row!("accordion_item_with", KeyLabelExpandedChildren, |p| {
        accordion_item_with(p.key, p.label, p.expanded, p.children)
    }),
    row!(
        "accordion_item_with_spaced",
        KeyLabelExpandedChildren,
        |p| accordion_item_with_spaced(p.key, p.label, p.expanded, p.children)
    ),
    row!("contained_list", KeyLabelChildren, |p| contained_list(
        p.key, p.label, p.children
    )),
    row!("contained_list_disclosed", KeyLabelChildren, |p| {
        contained_list_disclosed(p.key, p.label, p.children)
    }),
    row!("unordered_list", KeyChildren, |p| unordered_list(
        p.key, p.children
    )),
    row!("unordered_list_with", KeySchemeChildren, |p| {
        unordered_list_with(p.key, p.scheme.into(), p.children)
    }),
    row!("ordered_list", KeyChildren, |p| ordered_list(
        p.key, p.children
    )),
    row!("list_item", KeyLabel, |p| list_item(p.key, p.label)),
    row!("list_item_with", KeyLabelNested, |p| list_item_with(
        p.key, p.label, p.nested
    )),
    row!("modal", ModalParams, |p| modal(
        p.key, p.title, p.body, p.primary
    )),
    row!("modal_passive", KeyLabelValue, |p| modal_passive(
        p.key, p.label, p.value
    )),
    row!("popover", PopoverParams, |p| popover(
        p.key, p.label, p.anchor, p.body
    )),
    row!("popover_with", PopoverWithParams, |p| popover_with(
        p.key, p.label, p.anchor, p.children
    )),
    row!("popover_with_placement", PopoverWithPlacementParams, |p| {
        popover_with_placement(p.key, p.label, p.anchor, p.edge, p.align, p.children)
    }),
    row!("context_menu", ContextMenuParams, |p| context_menu(
        p.key, p.label, p.x, p.y, p.children
    )),
    row!("structured_list", StructuredListParams, |p| {
        structured_list(p.key, p.header, p.rows)
    }),
    row!("structured_list_sized", StructuredListSizedParams, |p| {
        structured_list_sized(p.key, p.header, p.rows, &p.weights, p.dividers)
    }),
    row!("structured_list_row", KeyChildrenSelected, |p| {
        structured_list_row(p.key, p.children, p.selected)
    }),
    row!("tile", KeyLabel, |p| tile(p.key, p.label)),
    row!("clickable_tile", KeyLabelValue, |p| clickable_tile(
        p.key, p.label, p.value
    )),
    row!("selectable_tile", KeyLabelSelected, |p| selectable_tile(
        p.key, p.label, p.selected
    )),
    row!("expandable_tile", KeyLabelExpandedBody, |p| {
        expandable_tile(p.key, p.label, p.expanded, p.body)
    }),
    row!("toggletip", KeyLabelOpenBody, |p| toggletip(
        p.key, p.label, p.open, p.body
    )),
    row!("toggletip_with", KeyLabelOpenChildren, |p| toggletip_with(
        p.key, p.label, p.open, p.children
    )),
    row!("tooltip", KeyLabelValue, |p| tooltip(
        p.key, p.label, p.value
    )),
    row!("tooltip_anchored", KeyAnchorBody, |p| tooltip_anchored(
        p.key, p.anchor, p.body
    )),
    row!("tree_view", KeyChildren, |p| tree_view(p.key, p.children)),
    row!("tree_item", TreeItemParams, |p| tree_item(
        p.key, p.label, p.expanded, p.selected, p.children
    )),
    row!("tree_item_xs", TreeItemParams, |p| tree_item_xs(
        p.key, p.label, p.expanded, p.selected, p.children
    )),
    row!("code_snippet", KeyLabel, |p| code_snippet(p.key, p.label)),
    row!("code_snippet_multi", KeyLabel, |p| code_snippet_multi(
        p.key, p.label
    )),
    row!("code_snippet_inline", KeyLabel, |p| code_snippet_inline(
        p.key, p.label
    )),
    row!("code_snippet_multi_capped", CodeSnippetCappedParams, |p| {
        code_snippet_multi_capped(p.key, p.code, p.filename, p.language, p.expanded)
    }),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use serde_json::json;

    use super::*;

    /// Every `pub fn` in this group's 12 source files whose first parameter
    /// is `key: impl Into<Key>` and whose return type is `ViewNode`. Scanned
    /// from the source text itself (not a hand list), so a constructor added
    /// later without a row fails this test rather than silently shipping
    /// unreachable from Lua.
    fn constructors_in_source() -> BTreeSet<String> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/component");
        let files = [
            "accordion.rs",
            "contained_list.rs",
            "list.rs",
            "modal.rs",
            "popover.rs",
            "structured_list.rs",
            "tile.rs",
            "toggletip.rs",
            "tooltip.rs",
            "tree_view.rs",
            "code_snippet.rs",
            "context_menu.rs",
        ];
        let mut out = BTreeSet::new();
        for file in files {
            let path = format!("{dir}/{file}");
            let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
            // Join the whole file into one line so a signature split across
            // several lines (trailing commas, wrapped args) still matches.
            let joined = src.replace('\n', " ");
            let mut rest = joined.as_str();
            while let Some(at) = rest.find("pub fn ") {
                rest = &rest[at + "pub fn ".len()..];
                let Some(paren) = rest.find('(') else { break };
                let name = rest[..paren].trim().to_owned();
                let Some(close) = matching_paren(rest, paren) else {
                    break;
                };
                let args = &rest[paren + 1..close];
                let after = &rest[close + 1..];
                let Some(brace) = after.find('{') else { break };
                let ret = after[..brace].trim();
                let first_param_is_key = args.trim_start().starts_with("key: impl Into<Key>");
                let returns_view_node = ret == "-> ViewNode";
                if first_param_is_key && returns_view_node {
                    out.insert(name);
                }
                rest = after;
            }
        }
        out
    }

    /// Find the `)` matching the `(` at `open`, honouring nested parens
    /// (generic bounds like `impl Into<Key>` hold no parens, but nested
    /// `Vec<(...)>`-style parameter lists are not used in this group; a
    /// depth counter handles either way).
    fn matching_paren(s: &str, open: usize) -> Option<usize> {
        let bytes = s.as_bytes();
        let mut depth = 0i32;
        for (i, &b) in bytes.iter().enumerate().skip(open) {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// G2: every constructor the source declares has exactly one row.
    #[test]
    fn coverage_every_constructor_has_a_row() {
        let source = constructors_in_source();
        let registered: BTreeSet<String> = ENTRIES.iter().map(|e| e.name.to_owned()).collect();
        let missing: Vec<_> = source.difference(&registered).collect();
        assert!(
            missing.is_empty(),
            "constructors present in source but missing a registry row: {missing:?}"
        );
        let extra: Vec<_> = registered.difference(&source).collect();
        assert!(
            extra.is_empty(),
            "registry rows naming no constructor found in source (renamed or removed?): {extra:?}"
        );
    }

    /// G1: no name appears twice in this file's own list.
    #[test]
    fn no_duplicate_names_within_this_group() {
        let mut seen = BTreeSet::new();
        for e in ENTRIES {
            assert!(
                seen.insert(e.name),
                "`{}` is registered twice in containment",
                e.name
            );
        }
    }

    /// A minimal, valid params table for each row, distinct enough between
    /// rows that two different constructors could never coincidentally
    /// produce the same `ViewNode`. This is what G3 calls each row with.
    fn sample_params(name: &str) -> Value {
        match name {
            "accordion" => json!({ "key": "k", "children": [] }),
            "accordion_spaced" => json!({ "key": "k", "children": [] }),
            "accordion_item" => json!({ "key": "k", "label": "l", "expanded": true, "body": "b" }),
            "accordion_item_sm" => {
                json!({ "key": "k", "label": "l-sm", "expanded": true, "body": "b" })
            }
            "accordion_item_lg" => {
                json!({ "key": "k", "label": "l-lg", "expanded": true, "body": "b" })
            }
            "accordion_item_spaced" => {
                json!({ "key": "k", "label": "l", "expanded": true, "body": "b" })
            }
            "accordion_item_spaced_sm" => {
                json!({ "key": "k", "label": "l-sm", "expanded": true, "body": "b" })
            }
            "accordion_item_spaced_lg" => {
                json!({ "key": "k", "label": "l-lg", "expanded": true, "body": "b" })
            }
            "accordion_item_with" => {
                json!({ "key": "k", "label": "l", "expanded": true, "children": [] })
            }
            "accordion_item_with_spaced" => {
                json!({ "key": "k", "label": "l", "expanded": true, "children": [] })
            }
            "contained_list" => json!({ "key": "k", "label": "title", "children": [] }),
            "contained_list_disclosed" => {
                json!({ "key": "k", "label": "title-disclosed", "children": [] })
            }
            "unordered_list" => json!({ "key": "k", "children": [] }),
            "unordered_list_with" => {
                // `unordered_list` calls this with `BulletScheme::default()`
                // (`list.rs`), and the scheme's effect is stamped onto each
                // child, not the parent — so `children: []` here would make
                // the two rows coincidentally equal and defeat this test's
                // own purpose. One minimal child, plus a non-default scheme,
                // makes the two genuinely diverge.
                json!({
                    "key": "k",
                    "scheme": { "type": "fixed", "bullet": "square" },
                    "children": [{ "kind": "stack", "key": "i1" }]
                })
            }
            "ordered_list" => json!({ "key": "k-ordered", "children": [] }),
            "list_item" => json!({ "key": "k", "label": "l" }),
            // `nested: null` collapses to the same tree `list_item` builds
            // (`list.rs`), so a real nested node is needed to tell the two
            // rows apart.
            "list_item_with" => {
                json!({ "key": "k", "label": "l", "nested": { "kind": "stack", "key": "n1" } })
            }
            "modal" => json!({ "key": "k", "title": "t", "body": "b", "primary": "p" }),
            "modal_passive" => json!({ "key": "k", "label": "t", "value": "b" }),
            "popover" => json!({ "key": "k", "label": "l", "anchor": "a", "body": "b" }),
            "popover_with" => json!({ "key": "k", "label": "l", "anchor": "a", "children": [] }),
            "popover_with_placement" => json!({
                "key": "k", "label": "l", "anchor": "a",
                "edge": "top", "align": "start", "children": []
            }),
            "context_menu" => json!({
                "key": "k", "label": "l", "x": 24.0, "y": 48.0, "children": []
            }),
            // Two header cells: with only one column (the `header: []`
            // default), `lay_out_columns` has nothing to divide and
            // `dividers`/`weights` become unobservable, which is exactly the
            // false collision this sample must avoid.
            "structured_list" => json!({
                "key": "k",
                "header": [
                    { "kind": "stack", "key": "h1" },
                    { "kind": "stack", "key": "h2" }
                ],
                "rows": []
            }),
            // `structured_list` always calls this with uniform weights and
            // `dividers: true` (`structured_list.rs`); non-uniform weights
            // and `dividers: false` on the same two-column header is what
            // tells the two rows apart here.
            "structured_list_sized" => json!({
                "key": "k",
                "header": [
                    { "kind": "stack", "key": "h1" },
                    { "kind": "stack", "key": "h2" }
                ],
                "rows": [],
                "weights": [2.0, 1.0],
                "dividers": false
            }),
            "structured_list_row" => json!({ "key": "k", "children": [], "selected": true }),
            "tile" => json!({ "key": "k", "label": "body-text" }),
            "clickable_tile" => json!({ "key": "k", "label": "l", "value": "b" }),
            "selectable_tile" => json!({ "key": "k", "label": "l", "selected": true }),
            "expandable_tile" => json!({ "key": "k", "label": "l", "expanded": true, "body": "b" }),
            "toggletip" => json!({ "key": "k", "label": "l", "open": true, "body": "b" }),
            "toggletip_with" => json!({ "key": "k", "label": "l", "open": true, "children": [] }),
            "tooltip" => json!({ "key": "k", "label": "l", "value": "b" }),
            "tooltip_anchored" => json!({ "key": "k", "anchor": "a", "body": "b" }),
            "tree_view" => json!({ "key": "k", "children": [] }),
            "tree_item" => {
                json!({ "key": "k", "label": "l", "expanded": true, "selected": true, "children": [] })
            }
            "tree_item_xs" => {
                json!({ "key": "k", "label": "l-xs", "expanded": true, "selected": true, "children": [] })
            }
            "code_snippet" => json!({ "key": "k", "label": "code" }),
            "code_snippet_multi" => json!({ "key": "k", "label": "code-multi" }),
            "code_snippet_inline" => json!({ "key": "k", "label": "code-inline" }),
            "code_snippet_multi_capped" => json!({
                "key": "k",
                "code": "fn main() {}",
                "filename": "main.rs",
                "language": "rust",
                "expanded": false
            }),
            other => panic!("no sample params written for `{other}`; add one"),
        }
    }

    /// Constructor pairs that build the same node ON PURPOSE, because one
    /// delegates to the other with a fixed argument. Each entry names the
    /// delegation so a reader can check it against the source; a pair NOT
    /// listed here that collides in [`no_two_rows_build_the_same_node`] is a
    /// mis-wired row, not an intentional alias.
    ///
    /// Every entry below is read from the delegating constructor's own body,
    /// not assumed. Sample params are still chosen to diverge wherever a
    /// choice is available (a differing size, flag, or non-empty payload);
    /// this table exists for the residual cases where the smaller entry
    /// point calls the larger one with a value this test's own probes also
    /// happen to use, and for the record of intent regardless.
    const ALIASES: &[(&str, &str, &str)] = &[
        (
            "list_item",
            "list_item_with",
            "list.rs: list_item(k, l) = list_item_with(k, l, None)",
        ),
        (
            "unordered_list",
            "unordered_list_with",
            "list.rs: unordered_list(k, items) = \
             unordered_list_with(k, BulletScheme::default(), items)",
        ),
        (
            "structured_list",
            "structured_list_sized",
            "structured_list.rs: structured_list(k, header, rows) = \
             structured_list_sized(k, header, rows, &vec![1.0; ncols], true)",
        ),
    ];

    fn is_declared_alias(a: &str, b: &str) -> bool {
        ALIASES
            .iter()
            .any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
    }

    /// G3: no two rows build the same `ViewNode` from their own sample
    /// params, unless the pair is a declared alias in [`ALIASES`]. Catches a
    /// row wired to the wrong sibling constructor (a `tag_sm` row calling
    /// `tag_lg`), which two identical outputs would reveal immediately.
    #[test]
    fn no_two_rows_build_the_same_node() {
        let mut seen: Vec<(&str, ViewNode)> = Vec::new();
        for e in ENTRIES {
            let params = sample_params(e.name);
            let node = (e.ctor)(&params).unwrap_or_else(|err| {
                panic!("`{}` refused its own sample params: {err:?}", e.name)
            });
            for (other_name, other_node) in &seen {
                if is_declared_alias(e.name, other_name) {
                    continue;
                }
                assert!(
                    &node != other_node,
                    "`{}` and `{other_name}` built identical ViewNodes from distinct sample \
                     params, and neither is a declared alias in ALIASES",
                    e.name
                );
            }
            seen.push((e.name, node));
        }
    }

    /// Every [`ALIASES`] entry actually collides under its own natural
    /// (delegating) argument, proving the table records a real relationship
    /// read from source rather than a guess that happens not to fire.
    #[test]
    fn declared_aliases_are_real() {
        let cases: &[(&str, Value, &str, Value)] = &[
            (
                "list_item",
                json!({ "key": "k", "label": "l" }),
                "list_item_with",
                json!({ "key": "k", "label": "l", "nested": null }),
            ),
            (
                "unordered_list",
                json!({ "key": "k", "children": [] }),
                "unordered_list_with",
                json!({ "key": "k", "scheme": { "type": "carbon" }, "children": [] }),
            ),
            (
                "structured_list",
                json!({ "key": "k", "header": [], "rows": [] }),
                "structured_list_sized",
                json!({
                    "key": "k", "header": [], "rows": [],
                    "weights": [1.0], "dividers": true
                }),
            ),
        ];
        for (a_name, a_params, b_name, b_params) in cases {
            let a = lookup(a_name).unwrap_or_else(|| panic!("`{a_name}` must be registered"));
            let b = lookup(b_name).unwrap_or_else(|| panic!("`{b_name}` must be registered"));
            let a_node = (a.ctor)(a_params).unwrap_or_else(|e| panic!("{a_name}: {e:?}"));
            let b_node = (b.ctor)(b_params).unwrap_or_else(|e| panic!("{b_name}: {e:?}"));
            assert_eq!(
                a_node, b_node,
                "`{a_name}` and `{b_name}` were declared aliases but built different nodes \
                 under the delegating argument; the source no longer supports this ALIASES entry"
            );
        }
    }

    fn lookup(name: &str) -> Option<&'static Entry> {
        ENTRIES.iter().find(|e| e.name == name)
    }

    /// G4: a misspelled field names itself, not just "deserialize failed".
    #[test]
    fn bad_field_name_is_named_in_the_error() {
        let err = parse::<KeyLabel>("list_item", &json!({ "key": "k", "lable": "typo" }))
            .expect_err("misspelled field must be refused");
        assert!(
            err.reason.contains("lable") || err.reason.contains("unknown field"),
            "error must name the bad field, got: {}",
            err.reason
        );
    }

    /// G6: every row's `luau` is the shape's own `ParamShape::LUAU`. The
    /// `row!` macro reads `<$shape as ParamShape>::LUAU` directly rather than
    /// a literal, so this holds by construction; spot-check the two shapes
    /// whose union LUAU strings are the easiest to get wrong by hand.
    #[test]
    fn luau_strings_match_their_shape() {
        let scheme_row = ENTRIES
            .iter()
            .find(|e| e.name == "unordered_list_with")
            .expect("unordered_list_with must be registered");
        assert_eq!(scheme_row.luau, KeySchemeChildren::LUAU);

        let placement_row = ENTRIES
            .iter()
            .find(|e| e.name == "popover_with_placement")
            .expect("popover_with_placement must be registered");
        assert_eq!(placement_row.luau, PopoverWithPlacementParams::LUAU);
    }
}
