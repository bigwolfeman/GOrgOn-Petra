//! Registry rows: navigation. See [`super`] for why this file exists.
//!
//! Each row names a public constructor in `petra/petra/src/component/`,
//! deserializes its parameter table into a shape, and calls the shipped
//! constructor. A row must never re-derive what the constructor does.
//!
//! Components: Breadcrumb, Content switcher, Link, Menu, Menu buttons,
//! Menubar, Pagination, Tabs, UI shell header, UI shell left panel, UI
//! shell right panel. 47 constructors (see the `coverage` test at the
//! bottom, which counts the source rather than trusting this comment).

use serde::Deserialize;

use super::Entry;
use crate::component::params::{
    KeyChildren, KeyLabel, KeyLabelChildren, KeyLabelOpenChildren, KeyLabelSelected, KeyOnly,
    ParamShape,
};
use crate::component::registry::IconMarkParam;
use crate::component::{
    LeftPanelMode, PaginationPicker, breadcrumb, breadcrumb_item, breadcrumb_item_current,
    breadcrumb_item_icon, breadcrumb_overflow, breadcrumb_with_separator, contained_tab,
    contained_tab_bar, content_switcher, content_switcher_item, link, link_inline, menu,
    menu_button, menu_flyout, menu_item, menu_item_with, menubar, menubar_top, pagination,
    pagination_items, pagination_items_open, pagination_nav, pagination_numbers,
    pagination_page_size, pagination_range, tab, tab_bar, ui_shell_header, ui_shell_header_action,
    ui_shell_header_action_icon, ui_shell_header_menu_trigger, ui_shell_header_nav_item,
    ui_shell_left_panel, ui_shell_left_panel_divider, ui_shell_left_panel_icon_item,
    ui_shell_left_panel_icon_subitem, ui_shell_left_panel_in, ui_shell_left_panel_item,
    ui_shell_left_panel_rail, ui_shell_left_panel_subitem, ui_shell_right_panel,
    ui_shell_right_panel_divider, ui_shell_switcher, ui_shell_switcher_item, vertical_tab,
    vertical_tab_bar,
};
use crate::tree::{Edge, Key, ViewNode};

// --- one-off shapes, used only by this group -------------------------------

/// Wire form of [`PaginationPicker`]: the real enum has no serde support.
///
/// 1 constructor (`pagination_items_open`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PaginationPickerParam {
    PageSize,
    Page,
}

impl From<PaginationPickerParam> for PaginationPicker {
    fn from(p: PaginationPickerParam) -> Self {
        match p {
            PaginationPickerParam::PageSize => PaginationPicker::PageSize,
            PaginationPickerParam::Page => PaginationPicker::Page,
        }
    }
}

/// Wire form of [`LeftPanelMode`]: the real enum has no serde support.
/// Internally tagged on `type`, matching `M.track`'s convention
/// (`gorgon/lua-view/src/ui/builders.lua`), so every variant — including
/// the three unit ones — is a table, never a bare string.
///
/// 1 constructor (`ui_shell_left_panel_in`).
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
enum LeftPanelModeParam {
    Rail,
    Fixed,
    Expandable { expanded: bool },
    Hidden,
}

impl From<LeftPanelModeParam> for LeftPanelMode {
    fn from(m: LeftPanelModeParam) -> Self {
        match m {
            LeftPanelModeParam::Rail => LeftPanelMode::Rail,
            LeftPanelModeParam::Fixed => LeftPanelMode::Fixed,
            LeftPanelModeParam::Expandable { expanded } => LeftPanelMode::Expandable { expanded },
            LeftPanelModeParam::Hidden => LeftPanelMode::Hidden,
        }
    }
}

/// `breadcrumb_with_separator(key, sep, crumbs)`. The list is `children` on
/// the wire, matching [`KeyChildren`] on `breadcrumb`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreadcrumbWithSeparatorParams {
    key: Key,
    sep: String,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for BreadcrumbWithSeparatorParams {
    const LUAU: &'static str = "{ key: string, sep: string, children: { ViewNode } }";
}

/// `breadcrumb_overflow(key, max_visible, crumbs)`. `max_visible` is `u32`
/// on the wire; the constructor takes `usize`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreadcrumbOverflowParams {
    key: Key,
    max_visible: u32,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for BreadcrumbOverflowParams {
    const LUAU: &'static str = "{ key: string, max_visible: number, children: { ViewNode } }";
}

/// `breadcrumb_item_icon(key, mark, label)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreadcrumbItemIconParams {
    key: Key,
    mark: IconMarkParam,
    label: String,
}
impl ParamShape for BreadcrumbItemIconParams {
    const LUAU: &'static str = concat!(
        "{ key: string, mark: ",
        crate::icon_mark_luau!(),
        ", label: string }"
    );
}

/// `menu_item_with(key, label, icon?, shortcut?, submenu)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MenuItemWithParams {
    key: Key,
    label: String,
    #[serde(default)]
    icon: Option<IconMarkParam>,
    #[serde(default)]
    shortcut: Option<String>,
    #[serde(default)]
    submenu: bool,
}
impl ParamShape for MenuItemWithParams {
    const LUAU: &'static str = concat!(
        "{ key: string, label: string, icon: ",
        crate::icon_mark_luau!(),
        "?, shortcut: string?, submenu: boolean }"
    );
}

/// `menubar(key, label, edge, items)`. `Edge` already derives `Deserialize`
/// (`crate::tree::props`), same as containment's `popover_with_placement`.
/// The list is `children` on the wire so `menubar_top` (`KeyLabelChildren`)
/// and `menubar` share one Lua field name.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct MenubarParams {
    key: Key,
    label: String,
    edge: Edge,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for MenubarParams {
    const LUAU: &'static str = "{ key: string, label: string, \
         edge: \"top\" | \"bottom\" | \"left\" | \"right\", \
         children: { ViewNode } }";
}

/// `pagination`, `pagination_numbers`, `pagination_nav` — 3 constructors.
/// Two bare numbers; no shared shape has two numbers and no string, so
/// this stays local.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaginationParams {
    key: Key,
    page: u32,
    page_count: u32,
}
impl ParamShape for PaginationParams {
    const LUAU: &'static str = "{ key: string, page: number, page_count: number }";
}

/// `pagination_items`, `pagination_range` — 2 constructors.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaginationItemsParams {
    key: Key,
    page: u32,
    page_size: u32,
    total_items: u32,
}
impl ParamShape for PaginationItemsParams {
    const LUAU: &'static str =
        "{ key: string, page: number, page_size: number, total_items: number }";
}

/// `pagination_items_open` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaginationItemsOpenParams {
    key: Key,
    page: u32,
    page_size: u32,
    // An empty Lua table has no way to say "empty array" versus "empty
    // map" (`convert.rs`'s `sequence` gives up at `raw_len() == 0`), so any
    // `Vec<T>` field — not only `Vec<ViewNode>` — needs `#[serde(default)]`
    // to accept `page_sizes = {}`.
    #[serde(default)]
    page_sizes: Vec<u32>,
    total_items: u32,
    picker: PaginationPickerParam,
}
impl ParamShape for PaginationItemsOpenParams {
    const LUAU: &'static str = "{ key: string, page: number, page_size: number, \
         page_sizes: { number }, total_items: number, picker: \"page-size\" | \"page\" }";
}

/// `pagination_page_size(key, page_size, open_sizes?)`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct PaginationPageSizeParams {
    key: Key,
    page_size: u32,
    // Missing means the picker is closed (`None`). A present empty Lua
    // table still needs `#[serde(default)]` on the inner Vec; wrapping in
    // `Option` lets the field vanish entirely.
    #[serde(default)]
    open_sizes: Option<Vec<u32>>,
}
impl ParamShape for PaginationPageSizeParams {
    const LUAU: &'static str = "{ key: string, page_size: number, open_sizes: { number }? }";
}

/// `ui_shell_header` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiShellHeaderParams {
    key: Key,
    product_name: String,
    #[serde(default)]
    menu_trigger: Option<ViewNode>,
    // An empty Lua table crosses as `{}`, a JSON object, not `[]`: Lua has
    // no way to say "empty array" versus "empty map". `#[serde(default)]`
    // is what lets `nav = {}`/`actions = {}` still mean the empty vector.
    #[serde(default)]
    nav: Vec<ViewNode>,
    #[serde(default)]
    actions: Vec<ViewNode>,
}
impl ParamShape for UiShellHeaderParams {
    const LUAU: &'static str = "{ key: string, product_name: string, menu_trigger: ViewNode?, \
         nav: { ViewNode }, actions: { ViewNode } }";
}

/// `ui_shell_header_menu_trigger` — 1 constructor. No shared shape is a bare
/// `(key, bool)`, so this stays local.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyOpenOnly {
    key: Key,
    open: bool,
}
impl ParamShape for KeyOpenOnly {
    const LUAU: &'static str = "{ key: string, open: boolean }";
}

/// `ui_shell_header_action_icon` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderActionIconParams {
    key: Key,
    label: String,
    mark: IconMarkParam,
    active: bool,
}
impl ParamShape for HeaderActionIconParams {
    const LUAU: &'static str = concat!(
        "{ key: string, label: string, mark: ",
        crate::icon_mark_luau!(),
        ", active: boolean }"
    );
}

/// `ui_shell_left_panel_in` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LeftPanelInParams {
    key: Key,
    mode: LeftPanelModeParam,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    items: Vec<ViewNode>,
}
impl ParamShape for LeftPanelInParams {
    const LUAU: &'static str = "{ key: string, mode: { type: \"rail\" } | { type: \"fixed\" } | \
         { type: \"expandable\", expanded: boolean } | { type: \"hidden\" }, \
         items: { ViewNode } }";
}

/// `ui_shell_left_panel_item` — 1 constructor. Same shape as containment's
/// `tree_item`/`tree_item_xs`, kept as a fresh local copy rather than a
/// cross-file import: registry groups do not share private items with each
/// other any more than they share `params.rs`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LeftPanelItemParams {
    key: Key,
    label: String,
    expanded: bool,
    selected: bool,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for LeftPanelItemParams {
    const LUAU: &'static str = "{ key: string, label: string, expanded: boolean, \
         selected: boolean, children: { ViewNode } }";
}

/// `ui_shell_left_panel_icon_item` — 1 constructor.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LeftPanelIconItemParams {
    key: Key,
    label: String,
    mark: IconMarkParam,
    expanded: bool,
    selected: bool,
    // See `UiShellHeaderParams`'s `nav`/`actions` for why.
    #[serde(default)]
    children: Vec<ViewNode>,
}
impl ParamShape for LeftPanelIconItemParams {
    const LUAU: &'static str = concat!(
        "{ key: string, label: string, mark: ",
        crate::icon_mark_luau!(),
        ", expanded: boolean, selected: boolean, children: { ViewNode } }"
    );
}

/// This group's constructors: one row per public `ViewNode`-returning
/// constructor in the 11 navigation components. 47 rows; see the
/// `coverage` test below, which counts the source rather than this list.
pub const ENTRIES: &[Entry] = &[
    row!("breadcrumb", KeyChildren, |p| breadcrumb(p.key, p.children)),
    row!(
        "breadcrumb_with_separator",
        BreadcrumbWithSeparatorParams,
        |p| breadcrumb_with_separator(p.key, &p.sep, p.children)
    ),
    row!("breadcrumb_overflow", BreadcrumbOverflowParams, |p| {
        breadcrumb_overflow(p.key, p.max_visible as usize, p.children)
    }),
    row!("breadcrumb_item", KeyLabel, |p| breadcrumb_item(
        p.key, p.label
    )),
    row!("breadcrumb_item_current", KeyLabel, |p| {
        breadcrumb_item_current(p.key, p.label)
    }),
    row!("breadcrumb_item_icon", BreadcrumbItemIconParams, |p| {
        breadcrumb_item_icon(p.key, p.mark.into(), p.label)
    }),
    row!("content_switcher", KeyChildren, |p| content_switcher(
        p.key, p.children
    )),
    row!("content_switcher_item", KeyLabelSelected, |p| {
        content_switcher_item(p.key, p.label, p.selected)
    }),
    row!("link", KeyLabel, |p| link(p.key, p.label)),
    row!("link_inline", KeyLabel, |p| link_inline(p.key, p.label)),
    row!("menu", KeyLabelChildren, |p| menu(
        p.key, p.label, p.children
    )),
    row!("menu_item", KeyLabel, |p| menu_item(p.key, p.label)),
    row!("menu_item_with", MenuItemWithParams, |p| menu_item_with(
        p.key,
        p.label,
        p.icon.map(Into::into),
        p.shortcut.as_deref(),
        p.submenu
    )),
    row!("menu_flyout", KeyLabelChildren, |p| menu_flyout(
        p.key, p.label, p.children
    )),
    row!("menu_button", KeyLabelOpenChildren, |p| menu_button(
        p.key, p.label, p.open, p.children
    )),
    row!("menubar", MenubarParams, |p| menubar(
        p.key, p.label, p.edge, p.children
    )),
    row!("menubar_top", KeyLabelChildren, |p| menubar_top(
        p.key, p.label, p.children
    )),
    row!("pagination", PaginationParams, |p| pagination(
        p.key,
        p.page,
        p.page_count
    )),
    row!("pagination_items", PaginationItemsParams, |p| {
        pagination_items(p.key, p.page, p.page_size, p.total_items)
    }),
    row!("pagination_items_open", PaginationItemsOpenParams, |p| {
        pagination_items_open(
            p.key,
            p.page,
            p.page_size,
            &p.page_sizes,
            p.total_items,
            p.picker.into(),
        )
    }),
    row!("pagination_page_size", PaginationPageSizeParams, |p| {
        pagination_page_size(p.key, p.page_size, p.open_sizes.as_deref())
    }),
    row!("pagination_range", PaginationItemsParams, |p| {
        pagination_range(p.key, p.page, p.page_size, p.total_items)
    }),
    row!("pagination_numbers", PaginationParams, |p| {
        pagination_numbers(p.key, p.page, p.page_count)
    }),
    row!("pagination_nav", PaginationParams, |p| pagination_nav(
        p.key,
        p.page,
        p.page_count
    )),
    row!("tab", KeyLabelSelected, |p| tab(p.key, p.label, p.selected)),
    row!("contained_tab", KeyLabelSelected, |p| contained_tab(
        p.key, p.label, p.selected
    )),
    row!("vertical_tab", KeyLabelSelected, |p| vertical_tab(
        p.key, p.label, p.selected
    )),
    row!("tab_bar", KeyChildren, |p| tab_bar(p.key, p.children)),
    row!("contained_tab_bar", KeyChildren, |p| contained_tab_bar(
        p.key, p.children
    )),
    row!("vertical_tab_bar", KeyChildren, |p| vertical_tab_bar(
        p.key, p.children
    )),
    row!("ui_shell_header", UiShellHeaderParams, |p| {
        ui_shell_header(p.key, p.product_name, p.menu_trigger, p.nav, p.actions)
    }),
    row!("ui_shell_header_menu_trigger", KeyOpenOnly, |p| {
        ui_shell_header_menu_trigger(p.key, p.open)
    }),
    row!("ui_shell_header_nav_item", KeyLabelSelected, |p| {
        ui_shell_header_nav_item(p.key, p.label, p.selected)
    }),
    row!("ui_shell_header_action", KeyLabelSelected, |p| {
        ui_shell_header_action(p.key, p.label, p.selected)
    }),
    row!("ui_shell_header_action_icon", HeaderActionIconParams, |p| {
        ui_shell_header_action_icon(p.key, p.label, p.mark.into(), p.active)
    }),
    row!("ui_shell_left_panel_in", LeftPanelInParams, |p| {
        ui_shell_left_panel_in(p.key, p.mode.into(), p.items)
    }),
    row!("ui_shell_left_panel", KeyChildren, |p| ui_shell_left_panel(
        p.key, p.children
    )),
    row!("ui_shell_left_panel_rail", KeyChildren, |p| {
        ui_shell_left_panel_rail(p.key, p.children)
    }),
    row!("ui_shell_left_panel_item", LeftPanelItemParams, |p| {
        ui_shell_left_panel_item(p.key, p.label, p.expanded, p.selected, p.children)
    }),
    row!(
        "ui_shell_left_panel_icon_item",
        LeftPanelIconItemParams,
        |p| ui_shell_left_panel_icon_item(
            p.key,
            p.label,
            p.mark.into(),
            p.expanded,
            p.selected,
            p.children
        )
    ),
    row!("ui_shell_left_panel_subitem", KeyLabelSelected, |p| {
        ui_shell_left_panel_subitem(p.key, p.label, p.selected)
    }),
    row!("ui_shell_left_panel_icon_subitem", KeyLabelSelected, |p| {
        ui_shell_left_panel_icon_subitem(p.key, p.label, p.selected)
    }),
    row!("ui_shell_left_panel_divider", KeyOnly, |p| {
        ui_shell_left_panel_divider(p.key)
    }),
    row!("ui_shell_right_panel", KeyLabelOpenChildren, |p| {
        ui_shell_right_panel(p.key, p.label, p.open, p.children)
    }),
    row!("ui_shell_switcher", KeyLabelOpenChildren, |p| {
        ui_shell_switcher(p.key, p.label, p.open, p.children)
    }),
    row!("ui_shell_switcher_item", KeyLabelSelected, |p| {
        ui_shell_switcher_item(p.key, p.label, p.selected)
    }),
    row!("ui_shell_right_panel_divider", KeyOnly, |p| {
        ui_shell_right_panel_divider(p.key)
    }),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use serde_json::{Value, json};

    use super::*;

    /// Every `pub fn` in this group's 9 source files whose first parameter
    /// is `key: impl Into<Key>` and whose return type is `ViewNode`. Scanned
    /// from the source text itself (not a hand list), so a constructor added
    /// later without a row fails this test rather than silently shipping
    /// unreachable from Lua.
    fn constructors_in_source() -> BTreeSet<String> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/component");
        let files = [
            "breadcrumb.rs",
            "content_switcher.rs",
            "link.rs",
            "menu.rs",
            "menu_button.rs",
            "menubar.rs",
            "pagination.rs",
            "tabs.rs",
            "ui_shell.rs",
        ];
        let mut out = BTreeSet::new();
        for file in files {
            let path = format!("{dir}/{file}");
            let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
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

    /// Find the `)` matching the `(` at `open`, honouring nested parens.
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
        // `menu_item` and `menu_item_with` are `pub use`d from
        // `list_box.rs` (`menu.rs`); the scanner only sees `pub fn` text,
        // so a re-export is invisible. Lua still names them. `menu_flyout`
        // is the same filter for the same reason. Same rule as a
        // constructor whose first param is not `key: impl Into<Key>`.
        let extra: Vec<_> = registered
            .difference(&source)
            .filter(|name| {
                *name != "menu_item" && *name != "menu_item_with" && *name != "menu_flyout"
            })
            .collect();
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
                "`{}` is registered twice in navigation",
                e.name
            );
        }
    }

    /// Constructor pairs that build the same node ON PURPOSE, because one
    /// delegates to the other with a fixed argument. Each entry names the
    /// delegation so a reader can check it against the source; a pair NOT
    /// listed here that collides in [`no_two_rows_build_the_same_node`] is a
    /// mis-wired row, not an intentional alias.
    const ALIASES: &[(&str, &str, &str)] = &[];

    fn is_declared_alias(a: &str, b: &str) -> bool {
        ALIASES
            .iter()
            .any(|(x, y, _)| (*x == a && *y == b) || (*x == b && *y == a))
    }

    /// A minimal, valid params table for each row, distinct enough between
    /// rows that two different constructors could never coincidentally
    /// produce the same `ViewNode`. This is what G3 calls each row with.
    fn sample_params(name: &str) -> Value {
        match name {
            "breadcrumb" => json!({ "key": "k", "children": [] }),
            "breadcrumb_with_separator" => json!({
                "key": "k",
                "sep": ">",
                "children": [
                    { "kind": "stack", "key": "c1" },
                    { "kind": "stack", "key": "c2" }
                ]
            }),
            "breadcrumb_overflow" => json!({
                "key": "k",
                "max_visible": 2,
                "children": [
                    { "kind": "stack", "key": "c1" },
                    { "kind": "stack", "key": "c2" },
                    { "kind": "stack", "key": "c3" }
                ]
            }),
            "breadcrumb_item" => json!({ "key": "k", "label": "l" }),
            "breadcrumb_item_current" => json!({ "key": "k", "label": "l-current" }),
            "breadcrumb_item_icon" => {
                json!({ "key": "k", "mark": "check", "label": "l" })
            }
            "content_switcher" => json!({ "key": "k-cs", "children": [] }),
            "content_switcher_item" => json!({ "key": "k", "label": "l", "selected": true }),
            "link" => json!({ "key": "k", "label": "l" }),
            "link_inline" => json!({ "key": "k", "label": "l-inline" }),
            "menu" => json!({ "key": "k", "label": "l", "children": [] }),
            "menu_item" => json!({ "key": "k", "label": "l" }),
            "menu_item_with" => json!({
                "key": "k", "label": "l-with", "icon": "copy",
                "shortcut": "⌘S", "submenu": true
            }),
            "menu_flyout" => json!({ "key": "k-flyout", "label": "l-flyout", "children": [] }),
            "menu_button" => {
                json!({ "key": "k", "label": "l", "open": true, "children": [] })
            }
            "menubar" => json!({
                "key": "k", "label": "l", "edge": "bottom", "children": []
            }),
            "menubar_top" => json!({ "key": "k", "label": "l-menubar", "children": [] }),
            "pagination" => json!({ "key": "k", "page": 1, "page_count": 4 }),
            "pagination_items" => {
                json!({ "key": "k", "page": 1, "page_size": 10, "total_items": 42 })
            }
            "pagination_items_open" => json!({
                "key": "k", "page": 1, "page_size": 10,
                "page_sizes": [10, 20, 30], "total_items": 42, "picker": "page"
            }),
            "pagination_page_size" => json!({
                "key": "k", "page_size": 10, "open_sizes": [10, 20, 30]
            }),
            "pagination_range" => {
                json!({ "key": "k", "page": 1, "page_size": 10, "total_items": 42 })
            }
            "pagination_numbers" => json!({ "key": "k", "page": 1, "page_count": 4 }),
            "pagination_nav" => json!({ "key": "k", "page": 1, "page_count": 4 }),
            "tab" => json!({ "key": "k", "label": "l", "selected": true }),
            "contained_tab" => json!({ "key": "k", "label": "l-contained", "selected": true }),
            "vertical_tab" => json!({ "key": "k", "label": "l-vertical", "selected": true }),
            "tab_bar" => json!({ "key": "k", "children": [] }),
            "contained_tab_bar" => json!({ "key": "k-contained-bar", "children": [] }),
            "vertical_tab_bar" => json!({ "key": "k-vertical-bar", "children": [] }),
            "ui_shell_header" => json!({
                "key": "k", "product_name": "p", "menu_trigger": null, "nav": [], "actions": []
            }),
            "ui_shell_header_menu_trigger" => json!({ "key": "k", "open": true }),
            "ui_shell_header_nav_item" => json!({ "key": "k", "label": "l", "selected": true }),
            "ui_shell_header_action" => {
                json!({ "key": "k", "label": "l-action", "selected": true })
            }
            "ui_shell_header_action_icon" => {
                json!({ "key": "k", "label": "l", "mark": "close", "active": true })
            }
            "ui_shell_left_panel_in" => json!({
                "key": "k", "mode": { "type": "rail" }, "items": []
            }),
            "ui_shell_left_panel" => json!({ "key": "k", "children": [] }),
            "ui_shell_left_panel_rail" => json!({ "key": "k-rail", "children": [] }),
            "ui_shell_left_panel_item" => json!({
                "key": "k", "label": "l", "expanded": true, "selected": true, "children": []
            }),
            "ui_shell_left_panel_icon_item" => json!({
                "key": "k", "label": "l", "mark": "close",
                "expanded": true, "selected": true, "children": []
            }),
            "ui_shell_left_panel_subitem" => json!({ "key": "k", "label": "l", "selected": true }),
            "ui_shell_left_panel_icon_subitem" => {
                json!({ "key": "k", "label": "l-icon-sub", "selected": true })
            }
            "ui_shell_left_panel_divider" => json!({ "key": "k-left-divider" }),
            "ui_shell_right_panel" => {
                json!({ "key": "k", "label": "l", "open": true, "children": [] })
            }
            "ui_shell_switcher" => {
                json!({ "key": "k-switcher", "label": "l", "open": true, "children": [] })
            }
            "ui_shell_switcher_item" => {
                json!({ "key": "k", "label": "l-switcher-item", "selected": true })
            }
            "ui_shell_right_panel_divider" => json!({ "key": "k-right-divider" }),
            other => panic!("no sample params written for `{other}`; add one"),
        }
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

    /// G4: a misspelled field names itself, not just "deserialize failed".
    /// Calls [`crate::component::registry::parse`], the one owner of the
    /// registry's error wrapping.
    #[test]
    fn bad_field_name_is_named_in_the_error() {
        let err = crate::component::registry::parse::<KeyLabel>(
            "menu_item",
            &json!({ "key": "k", "lable": "typo" }),
        )
        .expect_err("misspelled field must be refused");
        assert!(
            err.reason.contains("lable") || err.reason.contains("unknown field"),
            "error must name the bad field, got: {}",
            err.reason
        );
    }

    /// G6: every row's `luau` is the shape's own `ParamShape::LUAU`. The
    /// `row!` macro reads `<$shape as ParamShape>::LUAU` directly rather
    /// than a literal, so this holds by construction; spot-check the two
    /// shapes whose union LUAU strings are the easiest to get wrong by hand.
    #[test]
    fn luau_strings_match_their_shape() {
        let icon_row = ENTRIES
            .iter()
            .find(|e| e.name == "ui_shell_header_action_icon")
            .expect("ui_shell_header_action_icon must be registered");
        assert_eq!(icon_row.luau, HeaderActionIconParams::LUAU);

        let panel_row = ENTRIES
            .iter()
            .find(|e| e.name == "ui_shell_left_panel_in")
            .expect("ui_shell_left_panel_in must be registered");
        assert_eq!(panel_row.luau, LeftPanelInParams::LUAU);
    }
}
