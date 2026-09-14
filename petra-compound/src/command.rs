//! Command palette as a [`Compound`]: the global filtered list.
//!
//! Its own fiber later, because a global binding reaches it
//! (`view-fiber.md` §4.5). The triple is still a value. `view` is a
//! [`Layer::FrameWide`] overlay list. Filtering is [`crate::filter_indices`],
//! applied in [`Command::view`] because `update` does not see [`Props`].
//!
//! Binding: spec 009 T019.
//!
//! # The global chord (T019)
//!
//! [`view`](Command::view) carries `Ctrl+K` as a [`Scope::Global`]
//! [`gorgon_petra::keymap::Binding`], on both the open and the closed root
//! — closed, because that root is the one the operator sees *before* they
//! ever press the chord, and a binding that only existed once the palette
//! was already open could never open it. `Ctrl+K` is not a bare
//! `KeyCode::Char` (FR-013a: `Binding::new` refuses one at global scope,
//! `is_bare_char` in `gorgon-petra`'s `keymap/binding.rs` excludes any
//! chord carrying `ctrl`), so it stays eligible while the operator types
//! into an ordinary field. [`Owner::Plugin`] rather than
//! [`Owner::Operator`]: this fiber is what declares the binding, not an
//! operator dotfile override — `Owner::Operator` is FR-011's *other* case,
//! the one a binding like this one can be outranked by.
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
//! exactly two things: declaring the binding, and holding the
//! `ui(global-bindings)` grant FR-012 checks. They do not own dispatch, and
//! nothing else owns it yet either. Do not read this as a pointer to code
//! that lives somewhere else.

use gorgon_petra::component::kit::stack;
use gorgon_petra::component::{IconMark, list_row_with, search, valued};
use gorgon_petra::keymap::{Binding, Chord, CommandName, Owner, Scope};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, FocusFigure, InputPolicy, Layer, NodeKind, Props as NodeProps, Role,
    Semantics, Tip, ViewNode,
};
use gorgon_petra::{Align, Axis, KeyCode, Modifiers};
use serde::{Deserialize, Serialize};

use crate::Compound;

/// The declaring plugin/fiber id [`open_binding`] records on
/// [`Owner::Plugin`]. Matches `gorgond::compound::COMMAND`
/// (`gorgon/gorgond/src/compound.rs`) — kept as a literal here rather than
/// imported, because `gorgon-petra-compound` must not depend on `gorgond`
/// (the dependency edge points the other way).
const OWNER_ID: &str = "gorgon-view-fiber::command";

/// `Ctrl+K`, global scope, spec 010 FR-012/FR-013a. See the module doc's
/// "The global chord" section.
///
/// # Panics
/// Only if `Ctrl+K` at global scope stopped validating — which would mean
/// `Binding::new`'s own FR-013a check started refusing a chord that carries
/// `ctrl`, a change to `gorgon-petra`, not to this module.
fn open_binding() -> Binding {
    Binding::new(
        vec![Chord {
            key: KeyCode::Char('k'),
            modifiers: Modifiers {
                ctrl: true,
                ..Modifiers::NONE
            },
        }],
        CommandName::builtin("command-palette.toggle"),
        Scope::Global,
        Owner::Plugin(OWNER_ID.to_owned()),
    )
    .unwrap_or_else(|err| panic!("command palette's own Ctrl+K global binding: {err}"))
}
use crate::filter::filter_indices;

/// Namespace over the command-palette triple. Never constructed as a value.
pub struct Command;

/// Root key of the node [`Command::view`] returns.
const COMMAND_KEY: &str = "command";

/// One command the author supplies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Stable identity. Node key of the row.
    pub id: String,
    /// Visible caption, and the haystack [`crate::filter_indices`] reads.
    pub label: String,
    /// Leading mark on the row. `None` leaves the label as the first child.
    pub icon: Option<IconMark>,
    /// Trailing hotkey caption. `None` omits the muted shortcut child.
    pub shortcut: Option<String>,
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
        }
    }

    /// Place `icon` at the leading edge of the row.
    #[must_use]
    pub fn with_icon(self, icon: IconMark) -> Self {
        Self {
            icon: Some(icon),
            ..self
        }
    }

    /// Place a muted `shortcut` caption at the trailing edge of the row.
    #[must_use]
    pub fn with_shortcut(self, shortcut: impl Into<String>) -> Self {
        Self {
            shortcut: Some(shortcut.into()),
            ..self
        }
    }
}

/// The palette's commands. The author owns these.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    /// Commands in author order. [`view`](Command::view) presents a filtered
    /// slice; `items` itself is not rewritten.
    pub items: Vec<Item>,
}

/// Query, highlight, and open-or-shut. Round-trips through reload as a whole.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Text in the search field. Also the filter needle.
    pub query: String,
    /// Index into the *filtered* list, not into [`Props::items`].
    pub highlighted: usize,
    /// Whether the overlay is mounted.
    pub open: bool,
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
    /// Mount the overlay.
    Open,
    /// Unmount the overlay.
    Close,
    /// Accept the highlighted row. Closes. The host reads [`State::highlighted`]
    /// against the current query; `update` does not see [`Props`].
    Choose,
}

impl Compound for Command {
    type Props = Props;
    type State = State;
    type Intent = Intent;
    type Request = ();

    fn init(_props: &Self::Props) -> Self::State {
        State::default()
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
        }
        Vec::new()
    }

    fn view(state: &Self::State, props: &Self::Props) -> ViewNode {
        if !state.open {
            let mut node =
                stack(COMMAND_KEY, Axis::Vertical, None, Vec::new()).with_binding(open_binding());
            node.semantics.expanded = Some(false);
            return node;
        }
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
        let mut children = vec![field];
        children.extend(
            matching(state, props)
                .into_iter()
                .enumerate()
                .map(|(pos, i)| {
                    let item = &props.items[i];
                    list_row_with(
                        item.id.as_str(),
                        item.label.as_str(),
                        pos == state.highlighted,
                        item.icon,
                        item.shortcut.as_deref(),
                    )
                }),
        );
        let mut content = stack("content", Axis::Vertical, None, children);
        content.props.align = Some(Align::Stretch);

        let mut node = ViewNode::new(NodeKind::Surface, COMMAND_KEY)
            .with_props(NodeProps {
                layer: Some(Layer::FrameWide),
                anchor: Some(Anchor::Viewport),
                input_policy: Some(InputPolicy::DismissOutside),
                tip: Some(Tip::Flush),
                takes_focus: Some(true),
                ..NodeProps::default()
            })
            .child(content)
            .with_binding(open_binding());
        node.props
            .tokens
            .insert("background".into(), token("surface.raised"));
        node.props
            .tokens
            .insert("shadow".into(), token("shadow.overlay"));
        node.constraints.horizontal = AxisConstraint {
            min: Some(320.0),
            max: Some(480.0),
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
}

fn matching(state: &State, props: &Props) -> Vec<usize> {
    filter_indices(&props.items, &state.query, |item| item.label.as_str())
}

fn token(name: &'static str) -> TokenName {
    TokenName::new(name).unwrap_or_else(|err| panic!("shipped token {name:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use gorgon_petra::component::IconMark;
    use gorgon_petra::tree::{Anchor, Layer, NodeKind, Role, ViewNode};

    use crate::Compound;

    use super::{COMMAND_KEY, Command, Intent, Item, Props};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn sample_props() -> Props {
        Props {
            items: vec![
                Item::new("open", "Open file"),
                Item::new("save", "Save"),
                Item::new("close", "Close window"),
            ],
        }
    }

    #[test]
    fn init_is_closed() {
        let state = Command::init(&sample_props());
        assert!(state.query.is_empty());
        assert!(!state.open);
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
        assert_eq!(node.bindings.len(), 1, "{:?}", node.bindings);
        let binding = &node.bindings[0];
        assert_eq!(binding.scope, gorgon_petra::keymap::Scope::Global);
        assert_eq!(
            binding.trigger(),
            &[gorgon_petra::keymap::Chord {
                key: gorgon_petra::KeyCode::Char('k'),
                modifiers: gorgon_petra::Modifiers {
                    ctrl: true,
                    ..gorgon_petra::Modifiers::NONE
                },
            }],
            "Ctrl+K, not a bare char (FR-013a)"
        );
        assert!(
            binding.validate().is_ok(),
            "Binding::new's own construction-time check must still accept it: {binding}"
        );
    }

    /// The open root carries the same binding — the palette must not lose
    /// its own toggle chord the moment it opens.
    #[test]
    fn the_open_root_also_carries_the_global_open_binding() {
        let props = sample_props();
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        assert_eq!(node.bindings.len(), 1, "{:?}", node.bindings);
        assert_eq!(node.bindings[0].scope, gorgon_petra::keymap::Scope::Global);
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
        };
        let mut state = Command::init(&props);
        Command::update(&mut state, Intent::Open);
        let node = Command::view(&state, &props);
        let row = named(&node, "rebuild");
        let keys: Vec<&str> = row.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["run", "shortcut"]);
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
    }

    fn walks_to(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| walks_to(child, key))
    }
}
