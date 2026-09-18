//! Spec 009's Command compound, hosted directly through the [`Compound`]
//! triple.
//!
//! Row 17 (Menu) is the sibling-anchored dropdown; `Command::view` is a
//! [`gorgon_petra::tree::Layer::FrameWide`] overlay, closed to an empty,
//! invisible stack. A page that only called `view` once would show
//! nothing at all, so this page owns a trigger of its own and drives
//! `Command::update` from real presses and keystrokes. Categories, list
//! and tile density, favorites, and the `>` prefix slot are live intents
//! too; the page fills `Props.mode_surface` when the query names a mode.

use gorgon_petra::component::{IconMark, button, section, text};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::command::{
    Command as CommandCompound, Intent, Item, Mode, PaletteMode, Props, State,
    ViewKind as CommandView, step_highlight,
};
use gorgon_petra_compound::filter_indices;

use super::Page;
use super::common::{body, column, dismisses, path_has, row, sp, wrapped};

/// The page's own trigger, outside the compound: `Command::view` closed is
/// an empty stack, so nothing in the compound itself can open it.
const TRIGGER: &str = "open-command";
/// Open the overlay in list density.
const OPEN_LIST: &str = "open-list";
/// Open the overlay in tile density.
const OPEN_TILES: &str = "open-tiles";
/// Open the overlay as a two-column list.
const OPEN_COLS_2: &str = "open-cols-2";
/// Open the overlay as a three-column list.
const OPEN_COLS_3: &str = "open-cols-3";
/// [`Command::view`]'s own root key, open or shut.
const COMMAND: &str = "command";
/// The search field [`Command::view`] mounts while open.
const FIELD: &str = "field";
/// Favorite mark on a command row or tile. Nested under the item's own id.
const FAV: &str = "fav";
/// The unfiltered category chip.
const CAT_ALL: &str = "cat-all";
/// Prefix of a named category chip: `cat-{name}`.
const CAT_PREFIX: &str = "cat-";

/// Live state of the Command (compound) page: the compound's own `Props`
/// and `State`, plus nothing else — the trigger reads `state.open` rather
/// than a second flag. `mode_surface` is filled at view time from the
/// query, never stored.
pub struct CommandCompoundPage {
    props: Props,
    state: State,
    /// `Page::dismissed` runs after `handle`. Open list / Open tiles sit
    /// outside the overlay, so a press on them is an outside press. Without
    /// this latch, dismissal would close the overlay `OpenList`/`OpenTiles`
    /// just opened. The toggle-trigger case the trait doc describes.
    hold_open: bool,
}

impl Default for CommandCompoundPage {
    fn default() -> Self {
        let props = Props {
            items: vec![
                Item::new("rebuild", "Rebuild fiber")
                    .with_icon(IconMark::Menu)
                    .with_shortcut("Ctrl+R")
                    .with_categories(["Edit"]),
                Item::new("open-file", "Open file")
                    .with_icon(IconMark::Search)
                    .with_shortcut("Ctrl+O")
                    .with_categories(["Files"]),
                Item::new("edit", "Edit buffer")
                    .with_icon(IconMark::Edit)
                    .with_shortcut("Ctrl+E")
                    .with_categories(["Edit"]),
                Item::new("copy-path", "Copy path")
                    .with_icon(IconMark::Copy)
                    .with_shortcut("Ctrl+C")
                    .with_categories(["Edit", "Files"]),
                Item::new("close-window", "Close window")
                    .with_icon(IconMark::Close)
                    .with_shortcut("Ctrl+W")
                    .with_categories(["Files"]),
            ],
            list_columns: 2,
            tile_columns: 4,
            default_view: CommandView::List,
            modes: vec![
                Mode::new('f', "Files").with_icon(IconMark::Search),
                Mode::new('c', "Calculator").with_icon(IconMark::Add),
            ],
            mode_surface: None,
        };
        let mut state = CommandCompound::init(&props);
        // Open at rest, through the real intent, so the resting picture is
        // the overlay rather than the closed stack every other frame-wide
        // overlay page already photographs from rest.
        CommandCompound::update(&mut state, Intent::Open);
        CommandCompound::update(
            &mut state,
            Intent::ToggleFavorite {
                id: "rebuild".into(),
            },
        );
        Self {
            props,
            state,
            hold_open: false,
        }
    }
}

impl CommandCompoundPage {
    /// Props handed to [`CommandCompound::view`]: the stored catalog plus
    /// the host-filled slot when the query names a registered mode. Petra
    /// does not invent a files picker or a calculator; this page does.
    fn view_props(&self) -> Props {
        let mut props = self.props.clone();
        props.mode_surface = if self.state.query.starts_with(">f") {
            Some(text("files-slot", "Files slot"))
        } else if self.state.query.starts_with(">c") {
            Some(text("calc-slot", "Calculator slot"))
        } else {
            None
        };
        props
    }

    /// Indices `Command::view` would show in [`PaletteMode::Commands`].
    fn matching_indices(&self) -> Vec<usize> {
        let mut idx = filter_indices(&self.props.items, &self.state.query, |it| it.label.as_str());
        if let Some(cat) = &self.state.category {
            idx.retain(|&i| self.props.items[i].categories.iter().any(|c| c == cat));
        }
        idx
    }

    /// Keys in the order [`CommandCompound::view`] paints them, which is
    /// also [`State::highlighted`]'s index space.
    fn cursor_keys(&self) -> Vec<String> {
        match PaletteMode::parse(&self.state.query, &self.props.modes) {
            PaletteMode::Extension { .. } => Vec::new(),
            PaletteMode::Favorites => {
                let remainder = self.state.query.strip_prefix('>').unwrap_or("");
                let favorited: Vec<&Item> = self
                    .props
                    .items
                    .iter()
                    .filter(|it| self.state.favorites.contains(&it.id))
                    .collect();
                let mut keys = Vec::new();
                for i in filter_indices(&favorited, remainder, |it| it.label.as_str()) {
                    keys.push(favorited[i].id.clone());
                }
                for i in filter_indices(&self.props.modes, remainder, |mode| mode.label.as_str()) {
                    keys.push(format!("mode-{}", self.props.modes[i].key));
                }
                keys
            }
            PaletteMode::Commands => self
                .matching_indices()
                .into_iter()
                .map(|i| self.props.items[i].id.clone())
                .collect(),
        }
    }

    fn cursor_columns(&self) -> u8 {
        match PaletteMode::parse(&self.state.query, &self.props.modes) {
            PaletteMode::Favorites => self.props.tile_columns.clamp(3, 8),
            PaletteMode::Commands | PaletteMode::Extension { .. } => match self.state.view {
                CommandView::Tiles => self.props.tile_columns.clamp(3, 8),
                CommandView::List => self.props.list_columns.clamp(1, 3),
            },
        }
    }

    fn cursor_index_for(&self, node: &str) -> Option<usize> {
        self.cursor_keys()
            .iter()
            .position(|key| path_has(node, key))
    }

    fn move_cursor(&mut self, key: KeyCode) {
        let keys = self.cursor_keys();
        let next = step_highlight(
            self.state.highlighted,
            keys.len(),
            self.cursor_columns(),
            key,
        );
        CommandCompound::update(&mut self.state, Intent::Highlight { index: next });
    }
}

impl Page for CommandCompoundPage {
    fn row(&self) -> &'static str {
        "Command (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "command-compound",
            "Command (compound)",
            vec![body(
                "cmc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "cmc-note",
                        "The same triple as spec 009 T019: a frame-wide \
                         overlay driven through `Compound::update`. Type to \
                         filter, arrows move the cursor, Enter chooses. \
                         `>` for favorite tiles, `>f` / `>c` for a \
                         host-filled slot. 1 / 2 / 3 columns and Tiles set \
                         density; category icons filter without clearing \
                         the query.",
                    ),
                    column(
                        "cmc-col",
                        sp("spacing.md"),
                        vec![
                            row(
                                "cmc-actions",
                                sp("spacing.sm"),
                                vec![
                                    button(TRIGGER, "Open command"),
                                    button(OPEN_LIST, "1 column"),
                                    button(OPEN_COLS_2, "2 columns"),
                                    button(OPEN_COLS_3, "3 columns"),
                                    button(OPEN_TILES, "Tiles"),
                                ],
                            ),
                            CommandCompound::view(&self.state, &self.view_props()),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if self.state.open {
            if let Some(key) = arrow_key(event) {
                self.move_cursor(key);
                return true;
            }
            match event {
                InputEvent::Text(typed) => {
                    let mut query = self.state.query.clone();
                    query.push_str(typed);
                    CommandCompound::update(&mut self.state, Intent::Type { query });
                    return true;
                }
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                } => {
                    let mut query = self.state.query.clone();
                    query.pop();
                    CommandCompound::update(&mut self.state, Intent::Type { query });
                    return true;
                }
                InputEvent::Key {
                    key: KeyCode::Enter,
                    pressed: true,
                    ..
                } => {
                    CommandCompound::update(&mut self.state, Intent::Choose);
                    return true;
                }
                _ => {}
            }
        }
        if path_has(node, FAV) && !is_enter(event) {
            if let Some(item) = self.props.items.iter().find(|it| path_has(node, &it.id)) {
                CommandCompound::update(
                    &mut self.state,
                    Intent::ToggleFavorite {
                        id: item.id.clone(),
                    },
                );
            }
            return true;
        }
        if path_has(node, CAT_ALL) {
            CommandCompound::update(&mut self.state, Intent::SetCategory { id: None });
            return true;
        }
        if let Some(name) = segment_suffix(node, CAT_PREFIX)
            && name != "all"
        {
            CommandCompound::update(&mut self.state, Intent::SetCategory { id: Some(name) });
            return true;
        }
        if path_has(node, TRIGGER) {
            if self.state.open {
                CommandCompound::update(&mut self.state, Intent::Close);
            } else {
                CommandCompound::update(&mut self.state, Intent::Open);
            }
            return true;
        }
        if path_has(node, OPEN_LIST) {
            self.props.list_columns = 1;
            CommandCompound::update(&mut self.state, Intent::OpenList);
            self.hold_open = true;
            return true;
        }
        if path_has(node, OPEN_COLS_2) {
            self.props.list_columns = 2;
            CommandCompound::update(&mut self.state, Intent::OpenList);
            self.hold_open = true;
            return true;
        }
        if path_has(node, OPEN_COLS_3) {
            self.props.list_columns = 3;
            CommandCompound::update(&mut self.state, Intent::OpenList);
            self.hold_open = true;
            return true;
        }
        if path_has(node, OPEN_TILES) {
            CommandCompound::update(&mut self.state, Intent::OpenTiles);
            self.hold_open = true;
            return true;
        }
        if let Some(pos) = self.cursor_index_for(node) {
            CommandCompound::update(&mut self.state, Intent::Highlight { index: pos });
            return true;
        }
        if path_has(node, FIELD) {
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        let hold = self.hold_open;
        self.hold_open = false;
        if hold {
            return;
        }
        if dismisses(ids, COMMAND) {
            CommandCompound::update(&mut self.state, Intent::Close);
        }
    }

    fn focused(&mut self, node: Option<&str>) {
        let Some(node) = node else {
            return;
        };
        if let Some(pos) = self.cursor_index_for(node)
            && self.state.highlighted != pos
        {
            CommandCompound::update(&mut self.state, Intent::Highlight { index: pos });
        }
    }

    fn gesture(&mut self, event: &InputEvent, node: &str, _frame: &PetrifiedFrame) -> bool {
        if !self.state.open {
            return false;
        }
        if !matches!(event, InputEvent::PointerMoved { .. }) {
            return false;
        }
        if let Some(pos) = self.cursor_index_for(node)
            && self.state.highlighted != pos
        {
            CommandCompound::update(&mut self.state, Intent::Highlight { index: pos });
        }
        false
    }
}

fn is_enter(event: &InputEvent) -> bool {
    matches!(
        event,
        InputEvent::Key {
            key: KeyCode::Enter,
            pressed: true,
            ..
        }
    )
}

fn arrow_key(event: &InputEvent) -> Option<KeyCode> {
    match event {
        InputEvent::Key {
            key: key @ (KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right),
            pressed: true,
            ..
        } => Some(*key),
        _ => None,
    }
}

/// The rest of the first path segment of `node` spelled `<prefix><rest>`.
fn segment_suffix(node: &str, prefix: &str) -> Option<String> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix).map(str::to_owned))
        .filter(|rest| !rest.is_empty())
}
