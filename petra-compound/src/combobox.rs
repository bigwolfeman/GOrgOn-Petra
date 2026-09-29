//! Combobox as a [`Compound`]: query, highlight, and open-or-shut live here.
//!
//! Not its own fiber: a field on the form that hosts it
//! (`view-fiber.md` §4.5). `view` composes [`field`] plus
//! [`gorgon_petra::component::kit::list_box`]. Filtering is
//! [`crate::filter_indices`], applied in [`Combobox::view`] because
//! `update` does not see [`Props`].
//!
//! Binding: spec 009 T018.

use gorgon_petra::component::kit::{Dividers, list_box, stack};
use gorgon_petra::component::{dropdown_option, field, valued};
use gorgon_petra::tree::ViewNode;
use gorgon_petra::{Align, Axis};
use serde::{Deserialize, Serialize};

use crate::Compound;
use crate::filter::filter_indices;

/// Namespace over the combobox triple. Never constructed as a value.
pub struct Combobox;

/// Root key of the node [`Combobox::view`] returns.
const COMBOBOX_KEY: &str = "combobox";

/// Items and the accessible name of the field. The author owns these.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Props {
    /// Accessible name of the field, and of the open list.
    pub label: String,
    /// Candidate strings, author order. [`view`](Combobox::view) presents a
    /// filtered slice; `items` itself is not rewritten.
    pub items: Vec<String>,
}

/// Query, highlight, and open-or-shut. Round-trips through reload as a whole.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    /// Text in the field. Also the filter needle.
    pub query: String,
    /// Index into the *filtered* list, not into [`Props::items`]. `0` is
    /// the first match. Out of range means no row is marked.
    pub highlighted: usize,
    /// Whether the list is mounted.
    pub open: bool,
}

/// Closed set of things that can happen to the combobox.
///
/// `Deserialize` so [`gorgon_view_fiber::ViewFiber`] can decode it off the
/// `ui:intent` bus (`ViewFiber`'s `C::Intent: DeserializeOwned` bound).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    /// Replace the query, reset the highlight to the first match, and open.
    Type {
        /// New field text.
        query: String,
    },
    /// Point the highlight at a row of the current filtered list.
    Highlight {
        /// Filtered-list index.
        index: usize,
    },
    /// Mount the list.
    Open,
    /// Unmount the list.
    Close,
    /// Accept the highlighted row. Closes. The host reads [`State::highlighted`]
    /// against the current query; `update` does not see [`Props`].
    Choose,
}

impl Compound for Combobox {
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
        let field_node = valued(field("field", props.label.as_str()), state.query.as_str());
        let mut children = vec![field_node];
        if state.open {
            let rows = matching(state, props)
                .into_iter()
                .enumerate()
                .map(|(pos, i)| {
                    dropdown_option(
                        format!("opt-{i}"),
                        props.items[i].as_str(),
                        pos == state.highlighted,
                    )
                })
                .collect();
            children.push(list_box(
                "menu",
                props.label.as_str(),
                "field",
                rows,
                Dividers::Between,
            ));
        }
        let mut node = stack(COMBOBOX_KEY, Axis::Vertical, None, children);
        node.props.align = Some(Align::Stretch);
        node.semantics.expanded = Some(state.open);
        node
    }
}

fn matching(state: &State, props: &Props) -> Vec<usize> {
    filter_indices(&props.items, &state.query, String::as_str)
}

#[cfg(test)]
mod tests {
    use gorgon_petra::tree::{Role, ViewNode};

    use crate::Compound;

    use super::{COMBOBOX_KEY, Combobox, Intent, Props};

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
            label: "Theme".into(),
            items: vec!["alpha".into(), "bravo".into(), "alpine".into()],
        }
    }

    #[test]
    fn init_is_empty_query_closed() {
        let state = Combobox::init(&sample_props());
        assert!(state.query.is_empty());
        assert_eq!(state.highlighted, 0);
        assert!(!state.open);
    }

    #[test]
    fn type_opens_and_resets_highlight() {
        let mut state = Combobox::init(&sample_props());
        Combobox::update(&mut state, Intent::Highlight { index: 2 });
        assert!(Combobox::update(&mut state, Intent::Type { query: "al".into() }).is_empty());
        assert_eq!(state.query, "al");
        assert_eq!(state.highlighted, 0);
        assert!(state.open);
    }

    #[test]
    fn open_close_choose_toggle_the_list() {
        let mut state = Combobox::init(&sample_props());
        Combobox::update(&mut state, Intent::Open);
        assert!(state.open);
        Combobox::update(&mut state, Intent::Close);
        assert!(!state.open);
        Combobox::update(&mut state, Intent::Open);
        Combobox::update(&mut state, Intent::Choose);
        assert!(!state.open);
    }

    #[test]
    fn closed_view_is_the_field_alone() {
        let props = sample_props();
        let node = Combobox::view(&Combobox::init(&props), &props);
        assert_eq!(node.key.as_str(), COMBOBOX_KEY);
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(
            node.children.iter().all(|c| c.key.as_str() != "menu"),
            "closed combobox must not mount a list"
        );
        let field = named(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::TextInput));
    }

    #[test]
    fn open_view_filters_and_marks_the_highlight() {
        let props = sample_props();
        let mut state = Combobox::init(&props);
        Combobox::update(&mut state, Intent::Type { query: "al".into() });
        let node = Combobox::view(&state, &props);
        assert_eq!(node.semantics.expanded, Some(true));
        assert_eq!(named(&node, "menu").semantics.role, Some(Role::Overlay));
        assert!(named(&node, "opt-0").semantics.selected);
        assert!(!named(&node, "opt-2").semantics.selected);
        assert!(
            !walks_to(&node, "opt-1"),
            "bravo must drop out of the filtered list"
        );
    }

    fn walks_to(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| walks_to(child, key))
    }

    #[test]
    fn highlight_moves_the_selected_row() {
        let props = sample_props();
        let mut state = Combobox::init(&props);
        Combobox::update(&mut state, Intent::Open);
        Combobox::update(&mut state, Intent::Highlight { index: 1 });
        let node = Combobox::view(&state, &props);
        assert!(!named(&node, "opt-0").semantics.selected);
        assert!(named(&node, "opt-1").semantics.selected);
    }
}
