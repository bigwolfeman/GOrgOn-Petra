//! Command palette as a [`Compound`]: the global filtered list.
//!
//! Its own fiber later, because a global binding reaches it
//! (`view-fiber.md` §4.5). The triple is still a value. `view` is a
//! [`Layer::FrameWide`] overlay list. Filtering is [`crate::filter_indices`],
//! applied in [`Command::view`] because `update` does not see [`Props`].
//!
//! Binding: spec 009 T019.

use gorgon_petra::component::kit::stack;
use gorgon_petra::component::{list_row, search, valued};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, InputPolicy, Layer, NodeKind, Props as NodeProps, Role, Semantics, Tip,
    ViewNode,
};
use gorgon_petra::{Align, Axis};
use serde::{Deserialize, Serialize};

use crate::Compound;
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
}

impl Item {
    /// A command identified by `id` and labelled `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
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
#[derive(Clone, Debug, PartialEq, Eq)]
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
            let mut node = stack(COMMAND_KEY, Axis::Vertical, None, Vec::new());
            node.semantics.expanded = Some(false);
            return node;
        }
        let field = valued(search("field", "Command"), state.query.as_str());
        let mut children = vec![field];
        children.extend(
            matching(state, props)
                .into_iter()
                .enumerate()
                .map(|(pos, i)| {
                    let item = &props.items[i];
                    list_row(
                        item.id.as_str(),
                        item.label.as_str(),
                        pos == state.highlighted,
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
            .child(content);
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

    fn walks_to(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| walks_to(child, key))
    }
}
