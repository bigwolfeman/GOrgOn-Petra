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
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::command::{
    Command as CommandCompound, Intent, Item, Mode, Props, State, ViewKind as CommandView,
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
                Mode {
                    key: 'f',
                    label: "Files".into(),
                },
                Mode {
                    key: 'c',
                    label: "Calculator".into(),
                },
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

    /// Indices `Command::view` would show for the current query, category,
    /// and favorites, in author order. Used to map a clicked item id onto
    /// [`Intent::Highlight`]'s filtered index.
    fn matching_indices(&self) -> Vec<usize> {
        let q = self.state.query.as_str();
        if q.starts_with(">f") || q.starts_with(">c") {
            return Vec::new();
        }
        if q.starts_with('>') {
            return self
                .props
                .items
                .iter()
                .enumerate()
                .filter(|(_, it)| self.state.favorites.contains(&it.id))
                .map(|(i, _)| i)
                .collect();
        }
        let mut idx = filter_indices(&self.props.items, q, |it| it.label.as_str());
        if let Some(cat) = &self.state.category {
            idx.retain(|&i| self.props.items[i].categories.iter().any(|c| c == cat));
        }
        idx
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
                         filter, `>` for favorite tiles, `>f` / `>c` for a \
                         host-filled slot. Open list and Open tiles bind \
                         the two densities; category chips filter without \
                         clearing the query.",
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
                                    button(OPEN_LIST, "Open list"),
                                    button(OPEN_TILES, "Open tiles"),
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
        if let Some(name) = segment_suffix(node, CAT_PREFIX) {
            if name != "all" {
                CommandCompound::update(&mut self.state, Intent::SetCategory { id: Some(name) });
                return true;
            }
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
            CommandCompound::update(&mut self.state, Intent::OpenList);
            self.hold_open = true;
            return true;
        }
        if path_has(node, OPEN_TILES) {
            CommandCompound::update(&mut self.state, Intent::OpenTiles);
            self.hold_open = true;
            return true;
        }
        if let Some(idx) = self
            .props
            .items
            .iter()
            .position(|it| path_has(node, &it.id))
        {
            if is_enter(event) {
                if self.state.open {
                    CommandCompound::update(&mut self.state, Intent::Choose);
                }
                return true;
            }
            let filtered = self.matching_indices();
            if let Some(pos) = filtered.iter().position(|&i| i == idx) {
                CommandCompound::update(&mut self.state, Intent::Highlight { index: pos });
                CommandCompound::update(&mut self.state, Intent::Choose);
            }
            return true;
        }
        if path_has(node, FIELD) {
            match event {
                InputEvent::Text(typed) => {
                    let mut query = self.state.query.clone();
                    query.push_str(typed);
                    CommandCompound::update(&mut self.state, Intent::Type { query });
                }
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                } => {
                    let mut query = self.state.query.clone();
                    query.pop();
                    CommandCompound::update(&mut self.state, Intent::Type { query });
                }
                InputEvent::Key {
                    key: KeyCode::Enter,
                    pressed: true,
                    ..
                } if self.state.open => {
                    CommandCompound::update(&mut self.state, Intent::Choose);
                }
                _ => {}
            }
            return true;
        }
        if self.state.open && is_enter(event) {
            CommandCompound::update(&mut self.state, Intent::Choose);
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

/// The rest of the first path segment of `node` spelled `<prefix><rest>`.
fn segment_suffix(node: &str, prefix: &str) -> Option<String> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix).map(str::to_owned))
        .filter(|rest| !rest.is_empty())
}
