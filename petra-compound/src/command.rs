//! Command palette as a [`Compound`]: overlay chrome, density, and prefix parse.
//!
//! Its own fiber later, because a global binding reaches it
//! (`view-fiber.md` §4.5). The triple is still a value. `view` is a
//! [`Layer::FrameWide`] overlay. Filtering is [`crate::filter_indices`],
//! applied in [`Command::view`] because `update` does not see [`Props`].
//!
//! Binding: spec 009 T019.
//!
//! # Petra / app boundary
//!
//! Petra owns the overlay, list/tile density, the category strip, favorites,
//! and the `>` prefix parse ([`PaletteMode`]). The app owns the catalog
//! (`Props.items`, `Props.modes`) and any `>x` body. When the query names a
//! registered mode, [`Command::view`] mounts [`Props::mode_surface`] if the
//! host filled that slot. It never builds a file picker or a calculator.
//!
//! `Intent::Choose` still only closes. Dispatch is spec 010 T046, and this
//! compound does not run the chosen command.
//!
//! # The global chords (T019)
//!
//! [`view`](Command::view) carries three [`Scope::Global`]
//! [`gorgon_petra::keymap::Binding`]s on both the open and the closed root
//! — closed, because that root is the one the operator sees *before* they
//! ever press a chord, and a binding that only existed once the palette
//! was already open could never open it.
//!
//! | Chord | Command |
//! |---|---|
//! | `Ctrl+K` | `command-palette.toggle` |
//! | `Ctrl+Shift+P` | `command-palette.open-list` |
//! | `Ctrl+Shift+T` | `command-palette.open-tiles` |
//!
//! `Binding::new` requires a non-empty trigger, and FR-013a refuses a bare
//! `KeyCode::Char` at global scope (`is_bare_char` in `gorgon-petra`'s
//! `keymap/binding.rs` excludes any chord carrying `ctrl`, `alt`, or
//! `meta`). The open-list / open-tiles chords are placeholders that satisfy
//! that rule; they are not an operator decision about those two densities.
//! [`Owner::Plugin`] rather than [`Owner::Operator`]: this fiber is what
//! declares the binding, not an operator dotfile override.
//!
//! **Declaring the binding is not the same as dispatching it, and the other
//! half does not exist yet.** Verified 2026-09-11 by searching the whole
//! tree: `gorgon_petra::keymap::Resolver` is constructed nowhere outside its
//! own module's tests, nothing reads `ViewNode::bindings` back off a frame,
//! and no host turns a key press into a command. A published binding is
//! inert today. The shell dispatcher is **spec 010 T046** — "wire the
//! interpretation order in the shell dispatcher: reserved, then `route`'s
//! `Raw`, then the binding table, then the `Route` from that same call"
//! — unchecked, along with most of spec 010's binding phase.
//!
//! So this module and its host fiber (`gorgond`'s
//! `gorgon-view-fiber::command` row, `gorgon/gorgond/src/compound.rs`) own
//! declaring the bindings and holding the `ui(global-bindings)` grant
//! FR-012 checks. They do not own dispatch, and nothing else owns it yet
//! either. Do not read this as a pointer to code that lives somewhere else.

use std::collections::BTreeSet;

use gorgon_petra::component::kit::{pad, stack};
use gorgon_petra::component::{
    IconMark, IconTone, icon_toned, list_row_with, search, selectable_tag, text, valued,
};
use gorgon_petra::keymap::{Binding, Chord, CommandName, Owner, Scope};
use gorgon_petra::token::{CornerRole, TokenName, corner_for};
use gorgon_petra::tree::{
    Anchor, AxisConstraint, FocusFigure, InputPolicy, Interaction, Layer, NodeKind,
    Props as NodeProps, Role, Semantics, TextWrap, Tip, TrackSize, ViewNode,
};
use gorgon_petra::{Align, Axis, KeyCode, Modifiers};
use serde::{Deserialize, Serialize};

use crate::Compound;
use crate::filter::filter_indices;

/// The declaring plugin/fiber id [`open_binding`] records on
/// [`Owner::Plugin`]. Matches `gorgond::compound::COMMAND`
/// (`gorgon/gorgond/src/compound.rs`) — kept as a literal here rather than
/// imported, because `gorgon-petra-compound` must not depend on `gorgond`
/// (the dependency edge points the other way).
const OWNER_ID: &str = "gorgon-view-fiber::command";

/// Root key of the node [`Command::view`] returns.
const COMMAND_KEY: &str = "command";

/// Favorite mark on a list row or command tile. Nested under the item id so
/// a page can [`Intent::ToggleFavorite`] without [`Intent::Choose`].
const FAV_KEY: &str = "fav";

const TOGGLE_CMD: &str = "command-palette.toggle";
const OPEN_LIST_CMD: &str = "command-palette.open-list";
const OPEN_TILES_CMD: &str = "command-palette.open-tiles";

fn plugin_binding(chords: Vec<Chord>, command: &'static str) -> Binding {
    Binding::new(
        chords,
        CommandName::builtin(command),
        Scope::Global,
        Owner::Plugin(OWNER_ID.to_owned()),
    )
    .unwrap_or_else(|err| panic!("command palette global binding {command}: {err}"))
}

fn chord(key: char, shift: bool) -> Chord {
    Chord {
        key: KeyCode::Char(key),
        modifiers: Modifiers {
            ctrl: true,
            shift,
            ..Modifiers::NONE
        },
    }
}

/// `Ctrl+K`, global scope, spec 010 FR-012/FR-013a.
fn open_binding() -> Binding {
    plugin_binding(vec![chord('k', false)], TOGGLE_CMD)
}

/// `Ctrl+Shift+P` placeholder for [`OPEN_LIST_CMD`]. `Binding::new` needs a
/// chord; FR-013a needs ctrl/alt/meta. See the module doc.
fn open_list_binding() -> Binding {
    plugin_binding(vec![chord('p', true)], OPEN_LIST_CMD)
}

/// `Ctrl+Shift+T` placeholder for [`OPEN_TILES_CMD`]. Same rule as
/// [`open_list_binding`].
fn open_tiles_binding() -> Binding {
    plugin_binding(vec![chord('t', true)], OPEN_TILES_CMD)
}

fn with_palette_bindings(node: ViewNode) -> ViewNode {
    node.with_binding(open_binding())
        .with_binding(open_list_binding())
        .with_binding(open_tiles_binding())
}

/// Namespace over the command-palette triple. Never constructed as a value.
pub struct Command;

/// List vs tiles. Last density lives on [`State::view`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewKind {
    /// One row per command, [`list_row_with`].
    #[default]
    List,
    /// Compact command tiles in a grid.
    Tiles,
}

/// Wire and gallery name for [`ViewKind`].
pub type View = ViewKind;

/// One command the author supplies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Stable identity. Node key of the row or tile.
    pub id: String,
    /// Visible caption, and the haystack [`crate::filter_indices`] reads.
    pub label: String,
    /// Leading mark. `None` leaves the label as the first child.
    pub icon: Option<IconMark>,
    /// Trailing hotkey caption on a list row. `None` omits it.
    pub shortcut: Option<String>,
    /// One item may belong to many categories. Empty still registers.
    pub categories: Vec<String>,
}

impl Item {
    /// A command identified by `id` and labelled `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            shortcut: None,
            categories: Vec::new(),
        }
    }

    /// Place `icon` at the leading edge of the row or tile.
    #[must_use]
    pub fn with_icon(self, icon: IconMark) -> Self {
        Self {
            icon: Some(icon),
            ..self
        }
    }

    /// Place a muted `shortcut` caption at the trailing edge of a list row.
    #[must_use]
    pub fn with_shortcut(self, shortcut: impl Into<String>) -> Self {
        Self {
            shortcut: Some(shortcut.into()),
            ..self
        }
    }

    /// Categories this item belongs to. One item, many names.
    #[must_use]
    pub fn with_categories<I, S>(self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            categories: categories.into_iter().map(Into::into).collect(),
            ..self
        }
    }
}

/// A `>` prefix the host may fill with [`Props::mode_surface`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    /// Single char after `>`. `'f'`, `'c'`.
    pub key: char,
    /// Caption on the favorites-mode tile (`mode-{key}`).
    pub label: String,
}

impl Mode {
    /// A registered prefix key and the tile caption for it.
    #[must_use]
    pub fn new(key: char, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }
}

/// The palette's catalog and density. The author owns these.
///
/// No `Eq`: [`Self::mode_surface`] is a [`ViewNode`], which carries `f32`
/// extents. Same drop `data_table::State` took for column weights.
#[derive(Clone, Debug, PartialEq)]
pub struct Props {
    /// Commands in author order. [`view`](Command::view) presents a filtered
    /// slice; `items` itself is not rewritten.
    pub items: Vec<Item>,
    /// List-density columns. Default 1, clamped `1..=3` at view time.
    pub list_columns: u8,
    /// Tile-density columns. Default 4, clamped `3..=8` at view time.
    pub tile_columns: u8,
    /// Density [`Command::init`] copies onto [`State::view`].
    pub default_view: ViewKind,
    /// Registered `>x` prefixes. Petra never builds the body behind them.
    pub modes: Vec<Mode>,
    /// Host-filled `>x` body. `None` mounts nothing extra; Petra does not
    /// invent a picker.
    pub mode_surface: Option<ViewNode>,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            list_columns: 1,
            tile_columns: 4,
            default_view: ViewKind::List,
            modes: Vec::new(),
            mode_surface: None,
        }
    }
}

/// Query, highlight, open-or-shut, density, category, and favorites.
/// Round-trips through reload as a whole.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Text in the search field. Also the filter needle, or a `>` prefix.
    pub query: String,
    /// Index into the *filtered* list, not into [`Props::items`].
    pub highlighted: usize,
    /// Whether the overlay is mounted.
    pub open: bool,
    /// Last density. [`Intent::OpenList`] / [`Intent::OpenTiles`] write this.
    pub view: ViewKind,
    /// Strip selection. `None` is all categories.
    pub category: Option<String>,
    /// Favorited item ids. Not author-supplied.
    pub favorites: BTreeSet<String>,
}

/// Closed set of things that can happen to the palette.
///
/// `Deserialize` so [`gorgon_view_fiber::ViewFiber`] can decode it off the
/// `ui:intent` bus (`ViewFiber`'s `C::Intent: DeserializeOwned` bound).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    /// Replace the query, reset the highlight to the first match, and open.
    Type {
        /// New search text.
        query: String,
    },
    /// Point the highlight at a row of the current filtered list.
    Highlight {
        /// Filtered-list index.
        index: usize,
    },
    /// Mount the overlay. Density stays whatever [`State::view`] already is.
    Open,
    /// Unmount the overlay.
    Close,
    /// Accept the highlighted row. Closes. The host reads [`State::highlighted`]
    /// against the current query; `update` does not see [`Props`]. Spec 010
    /// T046 still does not dispatch the chosen command.
    Choose,
    /// Open in list density.
    OpenList,
    /// Open in tile density.
    OpenTiles,
    /// Select a category chip. `None` is "all".
    SetCategory {
        /// Category name, or `None` for every item.
        id: Option<String>,
    },
    /// Insert or remove `id` from [`State::favorites`].
    ToggleFavorite {
        /// [`Item::id`].
        id: String,
    },
}

/// What the leading `>` of [`State::query`] means, given [`Props::modes`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteMode {
    /// No leading `>`. Command list or tiles.
    Commands,
    /// `>` alone, or `>` plus a first char that is not a registered mode key.
    Favorites,
    /// `>` plus a registered mode key. `rest` is the substring after the key,
    /// with one leading space stripped if present.
    Extension {
        /// The registered key.
        key: char,
        /// Remainder after the key, for the host.
        rest: String,
    },
}

impl PaletteMode {
    /// Prefix parse. Petra's half of the `>` slot; the host fills
    /// [`Props::mode_surface`] from [`Self::Extension`].
    #[must_use]
    pub fn parse(query: &str, modes: &[Mode]) -> Self {
        let Some(after) = query.strip_prefix('>') else {
            return Self::Commands;
        };
        let mut chars = after.chars();
        let Some(first) = chars.next() else {
            return Self::Favorites;
        };
        if modes.iter().any(|mode| mode.key == first) {
            let rest = chars.as_str();
            let rest = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
            Self::Extension { key: first, rest }
        } else {
            Self::Favorites
        }
    }
}

impl Compound for Command {
    type Props = Props;
    type State = State;
    type Intent = Intent;
    type Request = ();

    fn init(props: &Self::Props) -> Self::State {
        State {
            view: props.default_view,
            ..State::default()
        }
    }

    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request> {
        match intent {
            Intent::Type { query } => {
                state.query = query;
                state.highlighted = 0;
                state.open = true;
            }
            Intent::Highlight { index } => {
                state.highlighted = index;
            }
            Intent::Open => {
                state.open = true;
            }
            Intent::Close | Intent::Choose => {
                state.open = false;
            }
            Intent::OpenList => {
                state.open = true;
                state.view = ViewKind::List;
            }
            Intent::OpenTiles => {
                state.open = true;
                state.view = ViewKind::Tiles;
            }
            Intent::SetCategory { id } => {
                state.category = id;
            }
            Intent::ToggleFavorite { id } => {
                if !state.favorites.remove(&id) {
                    state.favorites.insert(id);
                }
            }
        }
        Vec::new()
    }

    fn view(state: &Self::State, props: &Self::Props) -> ViewNode {
        if !state.open {
            let mut node =
                with_palette_bindings(stack(COMMAND_KEY, Axis::Vertical, None, Vec::new()));
            node.semantics.expanded = Some(false);
            return node;
        }
        let mode = PaletteMode::parse(&state.query, &props.modes);
        let mut children = vec![search_field(state)];
        match &mode {
            PaletteMode::Commands => {
                if let Some(strip) = category_strip(state, props) {
                    children.push(strip);
                }
                children.push(command_results(state, props));
            }
            PaletteMode::Favorites => {
                children.push(favorites_grid(state, props));
            }
            PaletteMode::Extension { .. } => {
                if let Some(strip) = category_strip(state, props) {
                    children.push(strip);
                }
                if let Some(surface) = &props.mode_surface {
                    children.push(surface.clone());
                }
            }
        }
        open_overlay(children, overlay_max(&mode, state, props))
    }
}

fn search_field(state: &State) -> ViewNode {
    // `search` seats `FocusFigure::Sides`, which draws its two bars
    // seven units *outside* the control. That is right for a field with
    // a card's padding around it and wrong here: the palette is
    // full-bleed, so the field's own edges are the overlay's edges and
    // the two bars hang in the page behind it, touching nothing.
    //
    // `Border` and not `BarInside`, which is the other figure that stays
    // inside its control: a bar figure marks the *content run*, and
    // `marked_rect` excludes `NodeKind::Input` from that run because an
    // input is its own focus target. A search field's only other content
    // is its magnifier, so `BarInside` here draws a 13-unit stub under
    // the icon and nothing under the text. A ring on the field's own
    // rect is what a command palette shows and what Carbon specifies.
    //
    // The well and the holder must agree: focus is *shown on* the field
    // and *held by* its `input` child, and
    // `a_control_and_the_node_it_shows_focus_on_agree_about_the_figure`
    // refuses a holder that declares a shape nothing draws.
    let mut field = valued(search("field", "Command"), state.query.as_str());
    field.semantics.focus_figure = FocusFigure::Border;
    for child in &mut field.children {
        std::sync::Arc::make_mut(child).semantics.focus_figure = FocusFigure::Border;
    }
    field
}

fn open_overlay(children: Vec<ViewNode>, max_width: f32) -> ViewNode {
    let mut content = stack("content", Axis::Vertical, None, children);
    content.props.align = Some(Align::Stretch);

    let mut node = with_palette_bindings(
        ViewNode::new(NodeKind::Surface, COMMAND_KEY)
            .with_props(NodeProps {
                layer: Some(Layer::FrameWide),
                anchor: Some(Anchor::Viewport),
                input_policy: Some(InputPolicy::DismissOutside),
                tip: Some(Tip::Flush),
                takes_focus: Some(true),
                ..NodeProps::default()
            })
            .child(content),
    );
    node.props
        .tokens
        .insert("background".into(), token("surface.raised"));
    node.props
        .tokens
        .insert("shadow".into(), token("shadow.overlay"));
    node.constraints.horizontal = AxisConstraint {
        min: Some(320.0),
        max: Some(max_width),
        priority: 0,
    };
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some("Command palette".into()),
        expanded: Some(true),
        ..Semantics::default()
    };
    node
}

fn overlay_max(mode: &PaletteMode, state: &State, props: &Props) -> f32 {
    let list_max = {
        let n = props.list_columns.clamp(1, 3);
        (320.0 * f32::from(n)).max(480.0).min(960.0)
    };
    let tile_max = {
        let n = props.tile_columns.clamp(3, 8);
        (160.0 * f32::from(n)).min(1280.0)
    };
    match (mode, state.view) {
        (PaletteMode::Commands, ViewKind::List)
        | (PaletteMode::Extension { .. }, ViewKind::List) => list_max,
        (PaletteMode::Commands, ViewKind::Tiles)
        | (PaletteMode::Extension { .. }, ViewKind::Tiles)
        | (PaletteMode::Favorites, _) => tile_max,
    }
}

fn command_indices(state: &State, props: &Props) -> Vec<usize> {
    filter_indices(&props.items, &state.query, |item| item.label.as_str())
        .into_iter()
        .filter(|&i| match &state.category {
            None => true,
            Some(cat) => props.items[i].categories.iter().any(|c| c == cat),
        })
        .collect()
}

fn command_results(state: &State, props: &Props) -> ViewNode {
    let hits = command_indices(state, props);
    match state.view {
        ViewKind::List => {
            let rows = hits
                .into_iter()
                .enumerate()
                .map(|(pos, i)| {
                    let item = &props.items[i];
                    command_row(
                        item,
                        pos == state.highlighted,
                        state.favorites.contains(&item.id),
                    )
                })
                .collect();
            weight_grid("results", props.list_columns.clamp(1, 3), rows)
        }
        ViewKind::Tiles => {
            let tiles = hits
                .into_iter()
                .enumerate()
                .map(|(pos, i)| {
                    let item = &props.items[i];
                    command_tile(
                        item,
                        pos == state.highlighted,
                        state.favorites.contains(&item.id),
                    )
                })
                .collect();
            weight_grid("results", props.tile_columns.clamp(3, 8), tiles)
        }
    }
}

fn favorites_grid(state: &State, props: &Props) -> ViewNode {
    let remainder = state.query.strip_prefix('>').unwrap_or("");
    let favorited: Vec<&Item> = props
        .items
        .iter()
        .filter(|item| state.favorites.contains(&item.id))
        .collect();
    let item_hits = filter_indices(&favorited, remainder, |item| item.label.as_str());
    let mode_hits = filter_indices(&props.modes, remainder, |mode| mode.label.as_str());
    let mut tiles = Vec::new();
    let mut pos = 0usize;
    for i in item_hits {
        tiles.push(command_tile(favorited[i], pos == state.highlighted, true));
        pos += 1;
    }
    for i in mode_hits {
        tiles.push(mode_tile(&props.modes[i], pos == state.highlighted));
        pos += 1;
    }
    weight_grid("results", props.tile_columns.clamp(3, 8), tiles)
}

fn unique_categories(items: &[Item]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        for cat in &item.categories {
            if cat.is_empty() {
                continue;
            }
            if !out.iter().any(|seen| seen == cat) {
                out.push(cat.clone());
            }
        }
    }
    out
}

fn category_strip(state: &State, props: &Props) -> Option<ViewNode> {
    let cats = unique_categories(&props.items);
    if cats.is_empty() {
        return None;
    }
    let mut chips = vec![selectable_tag("cat-all", "all", state.category.is_none())];
    for name in cats {
        let selected = state.category.as_deref() == Some(name.as_str());
        chips.push(selectable_tag(format!("cat-{name}"), name, selected));
    }
    Some(stack(
        "categories",
        Axis::Horizontal,
        Some("spacing-03"),
        chips,
    ))
}

fn command_row(item: &Item, selected: bool, favorited: bool) -> ViewNode {
    list_row_with(
        item.id.as_str(),
        item.label.as_str(),
        selected,
        item.icon,
        item.shortcut.as_deref(),
    )
    .child(favorite_mark(favorited))
}

fn command_tile(item: &Item, selected: bool, favorited: bool) -> ViewNode {
    compact_tile(item.id.as_str(), item.label.as_str(), item.icon, selected)
        .child(favorite_mark(favorited))
}

fn mode_tile(mode: &Mode, selected: bool) -> ViewNode {
    compact_tile(
        &format!("mode-{}", mode.key),
        mode.label.as_str(),
        None,
        selected,
    )
}

/// Compact command tile. Not [`gorgon_petra::component::selectable_tile`]
/// (checkbox mark) and not [`gorgon_petra::component::clickable_tile`]
/// (Carbon text tile). Kickoff-shaped: icon plus label, selected fill,
/// [`Role::Button`].
fn compact_tile(key: &str, label: &str, icon: Option<IconMark>, selected: bool) -> ViewNode {
    let mut parts = Vec::new();
    if let Some(mark) = icon {
        parts.push(icon_toned("icon", mark, IconTone::Primary));
    }
    let mut caption = text("label", label);
    caption.props.wrap = Some(TextWrap::Ellipsis);
    parts.push(caption);
    let mut node = stack(key, Axis::Vertical, Some("spacing-03"), parts);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad("spacing-05", "spacing-04"));
    bind_selected_fills(&mut node);
    node.props
        .tokens
        .insert("radius".into(), token(corner_for(CornerRole::Tiled, 64.0)));
    let mut node = node.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click, Interaction::Hover],
    );
    node.semantics.selected = selected;
    node.semantics.focus_figure = FocusFigure::BarInside;
    node
}

fn favorite_mark(favorited: bool) -> ViewNode {
    let mark = if favorited {
        IconMark::CheckmarkFilled
    } else {
        IconMark::CheckmarkOutline
    };
    let glyph = icon_toned("glyph", mark, IconTone::Primary);
    let mut node = stack(FAV_KEY, Axis::Horizontal, None, vec![glyph]);
    node.props.align = Some(Align::Center);
    let label = if favorited {
        "Remove from favorites"
    } else {
        "Add to favorites"
    };
    let mut node = node.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click, Interaction::Hover],
    );
    node.semantics.selected = favorited;
    node.semantics.focus_figure = FocusFigure::BarInside;
    if favorited {
        node.props
            .tokens
            .insert("background".into(), token("layer-selected"));
    }
    node
}

fn bind_selected_fills(node: &mut ViewNode) {
    for (slot, name) in [
        ("background", "surface.base"),
        ("background@hover", "layer-hover"),
        ("background@selected", "layer-selected"),
        ("background@selected-hover", "layer-selected-hover"),
    ] {
        node.props.tokens.insert(slot.into(), token(name));
    }
}

fn weight_grid(key: &'static str, columns: u8, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(NodeProps {
            columns: vec![TrackSize::Weight { weight: 1.0 }; columns as usize],
            column_spacing: Some(token("spacing-03")),
            row_spacing: Some(token("spacing-03")),
            ..NodeProps::default()
        })
        .with_children(children)
}

fn token(name: &'static str) -> TokenName {
    TokenName::new(name).unwrap_or_else(|err| panic!("shipped token {name:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use gorgon_petra::component::{IconMark, text};
    use gorgon_petra::keymap::Scope;
    use gorgon_petra::tree::{Anchor, Layer, NodeKind, Role, ViewNode};
    use gorgon_petra::{KeyCode, Modifiers};

    use crate::Compound;

    use super::{
        COMMAND_KEY, Command, FAV_KEY, Intent, Item, Mode, OPEN_LIST_CMD, OPEN_TILES_CMD,
        PaletteMode, Props, TOGGLE_CMD, ViewKind,
    };

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn walks_to(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| walks_to(child, key))
    }

    fn has_list_item(node: &ViewNode) -> bool {
        node.semantics.role == Some(Role::ListItem)
            || node.children.iter().any(|child| has_list_item(child))
    }

    fn tree_has_text(node: &ViewNode, needle: &str) -> bool {
        node.props.text.as_deref() == Some(needle)
            || node
                .children
                .iter()
                .any(|child| tree_has_text(child, needle))
    }

    fn command_names(node: &ViewNode) -> Vec<&str> {
        node.bindings
            .iter()
            .map(|binding| binding.command.name.as_str())
            .collect()
    }

    fn binding_named<'a>(node: &'a ViewNode, name: &str) -> &'a gorgon_petra::keymap::Binding {
        node.bindings
            .iter()
            .find(|binding| binding.command.name == name)
            .unwrap_or_else(|| panic!("no binding named {name}"))
    }

    fn sample_props() -> Props {
        Props {
            items: vec![
                Item::new("open", "Open file"),
                Item::new("save", "Save"),
                Item::new("close", "Close window"),
            ],
            ..Props::default()
        }
    }

    #[test]
    fn init_is_closed() {
        let state = Command::init(&sample_props());
        assert!(state.query.is_empty());
        assert!(!state.open);
        assert_eq!(state.view, ViewKind::List);
    }

    #[test]
    fn init_copies_default_view() {
        let props = Props {
            default_view: ViewKind::Tiles,
            ..sample_props()
        };
        assert_eq!(Command::init(&props).view, ViewKind::Tiles);
    }

    #[test]
    fn closed_view_is_not_an_overlay() {
        let props = sample_props();
        let node = Command::view(&Command::init(&props), &props);
        assert_eq!(node.key.as_str(), COMMAND_KEY);
        assert_ne!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.expanded, Some(false));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
    }

    /// T019: the closed root — the one the operator sees *before* pressing
    /// the chord — must carry the global binding, or the chord could never
    /// open the palette in the first place.
    #[test]
    fn the_closed_root_carries_the_global_open_binding() {
        let props = sample_props();
        let node = Command::view(&Command::init(&props), &props);
        assert_eq!(
            command_names(&node),
            [TOGGLE_CMD, OPEN_LIST_CMD, OPEN_TILES_CMD],
            "{:?}",
            node.bindings
        );
        let binding = binding_named(&node, TOGGLE_CMD);
        assert_eq!(binding.scope, Scope::Global);
        assert_eq!(
            binding.trigger(),
            &[gorgon_petra::keymap::Chord {
                key: KeyCode::Char('k'),
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::NONE
                },
            }],
            "Ctrl+K, not a bare char (FR-013a)"
        );
        assert!(
            binding.validate().is_ok(),
            "Binding::new's own construction-time check must still accept it: {binding}"
        );
        for name in [OPEN_LIST_CMD, OPEN_TILES_CMD] {
            let extra = binding_named(&node, name);
            assert_eq!(extra.scope, Scope::Global);
            assert!(
                extra.validate().is_ok(),
                "placeholder chord for {name} must still pass FR-013a: {extra}"
            );
        }
    }

    /// The open root carries the same bindings — the palette must not lose
    /// its own toggle chord the moment it opens.
    #[test]
    fn the_open_root_also_carries_the_global_open_binding() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        assert_eq!(
            command_names(&node),
            [TOGGLE_CMD, OPEN_LIST_CMD, OPEN_TILES_CMD],
            "{:?}",
            node.bindings
        );
        assert_eq!(node.bindings[0].scope, Scope::Global);
    }

    #[test]
    fn open_view_is_a_frame_wide_overlay() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.props.layer, Some(Layer::FrameWide));
        assert_eq!(node.props.anchor, Some(Anchor::Viewport));
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.expanded, Some(true));
        assert!(named(&node, "open").semantics.selected);
        assert!(!named(&node, "save").semantics.selected);
        assert_eq!(named(&node, "open").semantics.role, Some(Role::ListItem));
    }

    #[test]
    fn type_filters_to_the_matching_labels() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(
            &mut state,
            Intent::Type {
                query: "clo".into(),
            },
        );
        let node = Command::view(&state, &props);
        named(&node, "close");
        assert!(
            !walks_to(&node, "open") && !walks_to(&node, "save"),
            "non-matching commands must not mount"
        );
        assert!(named(&node, "close").semantics.selected);
    }

    #[test]
    fn choose_closes() {
        let mut state = Command::init(&sample_props());
        Command::update(&mut state, Intent::Open);
        assert!(Command::update(&mut state, Intent::Choose).is_empty());
        assert!(!state.open);
    }

    #[test]
    fn an_item_with_icon_and_shortcut_renders_those_on_the_row() {
        let props = Props {
            items: vec![
                Item::new("rebuild", "Rebuild fiber")
                    .with_icon(IconMark::Menu)
                    .with_shortcut("Ctrl+R"),
            ],
            ..Props::default()
        };
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        let row = named(&node, "rebuild");
        let keys: Vec<&str> = row.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["run", "shortcut", FAV_KEY]);
        assert_eq!(
            named(&row, "label").props.text.as_deref(),
            Some("Rebuild fiber")
        );
        let shortcut = named(&row, "shortcut");
        assert_eq!(shortcut.props.text.as_deref(), Some("Ctrl+R"));
        assert_eq!(
            shortcut.props.tokens.get("foreground").map(|t| t.as_str()),
            Some("text.muted")
        );
        assert!(row.semantics.selected);
        named(&row, FAV_KEY);
    }

    #[test]
    fn an_item_with_two_categories_matches_either_set_category() {
        let props = Props {
            items: vec![
                Item::new("copy", "Copy path").with_categories(["Edit", "Files"]),
                Item::new("rebuild", "Rebuild").with_categories(["Edit"]),
            ],
            ..Props::default()
        };
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        Command::update(
            &mut state,
            Intent::SetCategory {
                id: Some("Files".into()),
            },
        );
        let node = Command::view(&state, &props);
        named(&node, "cat-all");
        named(&node, "cat-Edit");
        named(&node, "cat-Files");
        named(&node, "copy");
        assert!(
            !walks_to(&node, "rebuild"),
            "Edit-only item must drop when Files is selected"
        );
        Command::update(
            &mut state,
            Intent::SetCategory {
                id: Some("Edit".into()),
            },
        );
        let node = Command::view(&state, &props);
        named(&node, "copy");
        named(&node, "rebuild");
    }

    #[test]
    fn no_categories_on_any_item_means_no_cat_all_node() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        assert!(
            !walks_to(&node, "cat-all"),
            "an empty categories list on every item must not mount the strip"
        );
    }

    #[test]
    fn toggle_favorite_then_query_gt_mounts_that_item_as_a_tile() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::ToggleFavorite { id: "close".into() });
        assert!(state.favorites.contains("close"));
        Command::update(&mut state, Intent::Type { query: ">".into() });
        let node = Command::view(&state, &props);
        let tile = named(&node, "close");
        assert_eq!(tile.semantics.role, Some(Role::Button));
        named(&tile, FAV_KEY);
        assert!(
            !walks_to(&node, "open") && !walks_to(&node, "save"),
            "unfavorited commands must not mount on `>`"
        );
        assert!(!has_list_item(&node));
    }

    #[test]
    fn extension_mode_mounts_the_host_surface_and_does_not_invent_a_files_body() {
        let modes = vec![Mode::new('f', "Files")];
        let mut with_slot = sample_props();
        with_slot.modes = modes.clone();
        with_slot.mode_surface = Some(text("host-slot", "the host surface"));
        let mut state = Command::init(&with_slot);
        Command::update(&mut state, Intent::Type { query: ">f".into() });
        let node = Command::view(&state, &with_slot);
        named(&node, "host-slot");
        assert_eq!(
            named(&node, "host-slot").props.text.as_deref(),
            Some("the host surface")
        );

        let mut empty_slot = sample_props();
        empty_slot.modes = modes;
        empty_slot.mode_surface = None;
        let node = Command::view(&state, &empty_slot);
        assert!(
            !walks_to(&node, "host-slot"),
            "an empty slot must not keep a previous host surface"
        );
        assert!(
            !walks_to(&node, "Files") && !tree_has_text(&node, "Files"),
            "Petra must not invent a Files picker when mode_surface is None"
        );
        named(&node, "field");
    }

    #[test]
    fn open_tiles_sets_view_and_mounts_tiles_not_list_rows() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::OpenTiles);
        assert!(state.open);
        assert_eq!(state.view, ViewKind::Tiles);
        let node = Command::view(&state, &props);
        let tile = named(&node, "open");
        assert_eq!(tile.semantics.role, Some(Role::Button));
        named(&tile, FAV_KEY);
        assert!(
            !has_list_item(&node),
            "tile density must not mount list_row"
        );
    }

    #[test]
    fn prefix_parse_splits_commands_favorites_and_extension() {
        let modes = [Mode::new('f', "Files")];
        assert_eq!(PaletteMode::parse("clo", &modes), PaletteMode::Commands);
        assert_eq!(PaletteMode::parse(">", &modes), PaletteMode::Favorites);
        assert_eq!(PaletteMode::parse(">save", &modes), PaletteMode::Favorites);
        assert_eq!(
            PaletteMode::parse(">f", &modes),
            PaletteMode::Extension {
                key: 'f',
                rest: String::new(),
            }
        );
        assert_eq!(
            PaletteMode::parse(">f files", &modes),
            PaletteMode::Extension {
                key: 'f',
                rest: "files".into(),
            }
        );
        assert_eq!(
            PaletteMode::parse(">ffoo", &modes),
            PaletteMode::Extension {
                key: 'f',
                rest: "foo".into(),
            }
        );
    }
}
