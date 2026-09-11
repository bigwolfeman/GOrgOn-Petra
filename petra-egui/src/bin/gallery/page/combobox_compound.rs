//! Spec 009's Combobox compound, hosted directly through the [`Compound`]
//! triple.
//!
//! Distinct from row 11's "Dropdown": that page is a pure, argument-driven
//! atomic whose page owns the open flag and the selected index by hand.
//! This page owns `combobox::State` itself and drives it through
//! `Combobox::update` on a real interaction, so what is on screen is the
//! compound genuinely running, not a picture of `Combobox::view` called
//! once at rest.

use gorgon_petra::component::section;
use gorgon_petra::input::{InputEvent, KeyCode};
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::combobox::{Combobox as ComboboxCompound, Intent, Props, State};
use gorgon_petra_compound::filter_indices;

use super::Page;
use super::common::{body, dismisses, path_has, sp, wrapped};

/// Candidate items, author order. Three share the `al` substring so a
/// query narrows the list rather than only ever showing all or nothing —
/// the same items `combobox.rs`'s own unit tests use, extended by two so
/// the filtered and unfiltered lists visibly differ in length.
const ITEMS: [&str; 5] = ["Alpha", "Bravo", "Alpine", "Charlie", "Chartreuse"];

/// The list-box key `Combobox::view` mounts while open.
const MENU: &str = "menu";
/// The field key `Combobox::view` always mounts.
const FIELD: &str = "field";
/// An open option's key prefix: `opt-<absolute index into Props::items>`.
const OPT: &str = "opt-";

/// Live state of the Combobox (compound) page: the compound's own
/// `Props` and `State`, nothing else.
pub struct ComboboxCompoundPage {
    props: Props,
    state: State,
}

impl Default for ComboboxCompoundPage {
    fn default() -> Self {
        let props = Props {
            label: "Theme".into(),
            items: ITEMS.iter().map(|s| (*s).to_owned()).collect(),
        };
        // Opens on a query that narrows the list, driven through real
        // `update` calls rather than hand-built state, so the resting
        // picture shows the interactive states 42 other rows already cover
        // the closed one for.
        let mut state = ComboboxCompound::init(&props);
        ComboboxCompound::update(
            &mut state,
            Intent::Type {
                query: "al".into(),
            },
        );
        ComboboxCompound::update(&mut state, Intent::Highlight { index: 1 });
        Self { props, state }
    }
}

impl Page for ComboboxCompoundPage {
    fn row(&self) -> &'static str {
        "Combobox (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "combobox-compound",
            "Combobox (compound)",
            vec![body(
                "cbc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "cbc-note",
                        "The same triple as spec 009 T018, driven through \
                         `Compound::update`: typing, highlighting, and \
                         choosing are live intents, not a screenshot of \
                         `view` called once.",
                    ),
                    ComboboxCompound::view(&self.state, &self.props),
                ],
            )],
        )
    }

    fn handle(&mut self, event: &InputEvent, node: &str) -> bool {
        // An option is inside the field's own subtree, so it is matched
        // first, the same ordering `page/dropdown.rs` uses.
        if let Some(idx) = segment_number(node, OPT) {
            let filtered = filter_indices(&self.props.items, &self.state.query, String::as_str);
            if let Some(pos) = filtered.iter().position(|&i| i == idx) {
                ComboboxCompound::update(&mut self.state, Intent::Highlight { index: pos });
                ComboboxCompound::update(&mut self.state, Intent::Choose);
            }
            return true;
        }
        if path_has(node, FIELD) {
            match event {
                InputEvent::Text(typed) => {
                    let mut query = self.state.query.clone();
                    query.push_str(typed);
                    ComboboxCompound::update(&mut self.state, Intent::Type { query });
                }
                InputEvent::Key {
                    key: KeyCode::Backspace,
                    pressed: true,
                    ..
                } => {
                    let mut query = self.state.query.clone();
                    query.pop();
                    ComboboxCompound::update(&mut self.state, Intent::Type { query });
                }
                InputEvent::PointerPressed { .. } => {
                    ComboboxCompound::update(&mut self.state, Intent::Open);
                }
                _ => {}
            }
            return true;
        }
        false
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, MENU) {
            ComboboxCompound::update(&mut self.state, Intent::Close);
        }
    }
}

/// The number in the first path segment of `node` spelled `<prefix><n>`.
/// Copied from `page/date_picker.rs`: whole segments, so `opt-2` is an
/// option and `option-header` is not a malformed one.
fn segment_number(node: &str, prefix: &str) -> Option<usize> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix)?.parse::<usize>().ok())
}
