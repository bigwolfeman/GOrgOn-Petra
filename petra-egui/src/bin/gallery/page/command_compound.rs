//! Spec 009's Command compound, hosted directly through the [`Compound`]
//! triple.
//!
//! Row 17 (Menu) is the sibling-anchored dropdown; `Command::view` is a
//! [`gorgon_petra::tree::Layer::FrameWide`] overlay, closed to an empty,
//! invisible stack. A page that only called `view` once would show
//! nothing at all, so this page owns a trigger of its own and drives
//! `Command::update` from real presses and keystrokes.

use gorgon_petra::component::{button, section};
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::command::{Command as CommandCompound, Intent, Item, Props, State};
use gorgon_petra_compound::filter_indices;

use super::Page;
use super::common::{body, column, dismisses, path_has, sp, wrapped};

/// The page's own trigger, outside the compound: `Command::view` closed is
/// an empty stack, so nothing in the compound itself can open it.
const TRIGGER: &str = "open-command";
/// [`Command::view`]'s own root key, open or shut.
const COMMAND: &str = "command";
/// The search field [`Command::view`] mounts while open.
const FIELD: &str = "field";

/// Live state of the Command (compound) page: the compound's own `Props`
/// and `State`, plus nothing else — the trigger reads `state.open` rather
/// than a second flag.
pub struct CommandCompoundPage {
    props: Props,
    state: State,
}

impl Default for CommandCompoundPage {
    fn default() -> Self {
        let props = Props {
            items: vec![
                Item::new("rebuild", "Rebuild fiber"),
                Item::new("open-file", "Open file"),
                Item::new("close-window", "Close window"),
            ],
        };
        let mut state = CommandCompound::init(&props);
        // Open at rest, through the real intent, so the resting picture is
        // the overlay rather than the closed stack every other frame-wide
        // overlay page already photographs from rest.
        CommandCompound::update(&mut state, Intent::Open);
        Self { props, state }
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
                         overlay driven through `Compound::update`. Open \
                         command toggles it; typing filters the list.",
                    ),
                    column(
                        "cmc-col",
                        sp("spacing.md"),
                        vec![
                            button(TRIGGER, "Open command"),
                            CommandCompound::view(&self.state, &self.props),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        if let Some(idx) = self.props.items.iter().position(|it| path_has(node, &it.id)) {
            let filtered =
                filter_indices(&self.props.items, &self.state.query, |it| it.label.as_str());
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
                _ => {}
            }
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
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, COMMAND) {
            CommandCompound::update(&mut self.state, Intent::Close);
        }
    }
}
