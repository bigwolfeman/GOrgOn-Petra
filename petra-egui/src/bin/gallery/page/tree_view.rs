//! Inventory row 39, Tree view.

use std::collections::BTreeSet;

use gorgon_petra::component::{section, tree_item, tree_view};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp, wrapped};

/// The sample hierarchy: `(key, label, parent)`, depth first, a `None`
/// parent for a root.
///
/// A table rather than a nested literal, because three separate questions
/// are asked of this shape — who are my children, am I a branch, and is the
/// selection somewhere below me — and a nested literal answers only the
/// first. Four levels deep with siblings at every level, so Carbon's indent
/// ramp (branch L1 16, leaf L1 40, +16 a level) is visible rather than
/// asserted.
const NODES: [(&str, &str, Option<&str>); 9] = [
    ("tv-gorgon", "gorgon", None),
    ("tv-petra", "petra", Some("tv-gorgon")),
    ("tv-component", "component", Some("tv-petra")),
    ("tv-layout", "layout", Some("tv-petra")),
    ("tv-petra-egui", "petra-egui", Some("tv-gorgon")),
    ("tv-gallery", "gallery", Some("tv-petra-egui")),
    ("tv-inspector", "inspector.rs", Some("tv-gorgon")),
    ("tv-docs", "docs", None),
    ("tv-constitution", "constitution.md", Some("tv-docs")),
];

/// Live state of the Tree view page: which branches stand open, and which
/// node is selected.
///
/// The row was a unit struct with `expanded` and `selected` written into
/// the tree as literals and a `handle` that returned `false`, so every
/// press the chrome routed here was dropped on the floor. The operator's
/// whole report on the row was *"not working"*, and it was exact: the
/// component declares `Interaction::Click` (`tree_view.rs:70`) and applies
/// it (`:211`), so the press arrived and the page threw it away.
pub struct TreeView {
    expanded: BTreeSet<&'static str>,
    selected: &'static str,
}

impl Default for TreeView {
    fn default() -> Self {
        Self {
            expanded: ["tv-gorgon", "tv-petra"].into_iter().collect(),
            selected: "tv-component",
        }
    }
}

impl TreeView {
    /// The children of `key`, in table order. `None` names the roots.
    fn children_of(key: Option<&str>) -> Vec<&'static str> {
        NODES
            .iter()
            .filter(|(_, _, parent)| *parent == key)
            .map(|(child, _, _)| *child)
            .collect()
    }

    fn is_branch(key: &str) -> bool {
        !Self::children_of(Some(key)).is_empty()
    }

    fn build(&self, key: &'static str) -> ViewNode {
        let label = NODES
            .iter()
            .find(|(node, _, _)| *node == key)
            .map(|(_, label, _)| *label)
            .unwrap_or(key);
        // Children are always handed over: `tree_item` reads them to know
        // it is a branch and to pick the caret's direction, and mounts them
        // only while `expanded`.
        let children = Self::children_of(Some(key))
            .into_iter()
            .map(|child| self.build(child))
            .collect();
        tree_item(
            key,
            label,
            self.expanded.contains(key),
            self.selected == key,
            children,
        )
    }

    /// The deepest node key named anywhere in a routed path.
    ///
    /// The route to a nested row names every ancestor as well
    /// (`.../tv-gorgon/children/tv-petra/row/label`), so a first-match scan
    /// would select the root every time. The last matching segment is the
    /// row the pointer was actually over.
    fn hit(node: &str) -> Option<&'static str> {
        node.rsplit('/').find_map(|segment| {
            NODES
                .iter()
                .map(|(key, _, _)| *key)
                .find(|key| *key == segment)
        })
    }
}

impl Page for TreeView {
    fn row(&self) -> &'static str {
        "Tree view"
    }

    fn body(&self) -> ViewNode {
        let roots: Vec<ViewNode> = Self::children_of(None)
            .into_iter()
            .map(|root| self.build(root))
            .collect();
        section(
            "tree",
            "Tree view",
            vec![body(
                "tv",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "note",
                        "Click a row to select it: the accent bar and the fill \
                         move to it. A row with a caret is a branch, and \
                         clicking one opens or shuts it as well.",
                    ),
                    tree_view("tv", roots),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        let Some(key) = Self::hit(node) else {
            return false;
        };
        self.selected = key;
        // Carbon splits the row: the caret is its own 24x24 toggle and the
        // label selects. Ours is one hit target (`tree_view.rs:211` makes
        // the whole grid interactive and the caret inside it declares no
        // interactions of its own), so a press on a branch does both. That
        // split is a component change and round 3's operator question Q11;
        // it is deliberately not answered here.
        if Self::is_branch(key) && !self.expanded.remove(key) {
            self.expanded.insert(key);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::TreeView;
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    fn placed(page: &TreeView, key: &str) -> bool {
        find(&page.body(), key).is_some()
    }

    fn selected(page: &TreeView, key: &str) -> bool {
        find(&page.body(), key).is_some_and(|node| node.semantics.selected)
    }

    /// A press on a branch opens it, and a second press shuts it again.
    ///
    /// Read off the tree the page builds, not off the state field: the
    /// literals were the defect, and a bool flipping in the page proves
    /// nothing about what the next frame is made of.
    #[test]
    fn a_press_on_a_shut_branch_mounts_its_children_and_a_second_press_unmounts_them() {
        let mut page = TreeView::default();
        assert!(!placed(&page, "tv-gallery"), "petra-egui starts shut");
        assert!(page.handle(
            &press(),
            "/page/tree/tv/tv/tv-gorgon/children/tv-petra-egui"
        ));
        assert!(
            placed(&page, "tv-gallery"),
            "the press did not open the branch"
        );
        assert!(page.handle(
            &press(),
            "/page/tree/tv/tv/tv-gorgon/children/tv-petra-egui"
        ));
        assert!(
            !placed(&page, "tv-gallery"),
            "the press did not shut the branch"
        );
    }

    /// The route names every ancestor, so the deepest key in it is the row
    /// that was pressed. Matching the first would toggle the root instead.
    #[test]
    fn a_press_deep_in_the_tree_selects_the_row_it_landed_on_not_its_root() {
        let mut page = TreeView::default();
        assert!(page.handle(
            &press(),
            "/page/tree/tv/tv/tv-gorgon/children/tv-petra/children/tv-layout/row/label",
        ));
        assert!(selected(&page, "tv-layout"));
        assert!(!selected(&page, "tv-gorgon"));
        assert!(
            placed(&page, "tv-petra"),
            "nothing collapsed under the press"
        );
    }

    /// A press on a branch does both jobs Carbon's row does: it shuts the
    /// branch and it takes the selection.
    ///
    /// Carbon splits them — the 24x24 caret toggles and the label selects —
    /// and ours cannot, because `tree_view.rs:211` makes the whole grid one
    /// hit target and the caret inside it declares no interactions. Splitting
    /// them is round 3's operator question Q11. Until it is answered, one
    /// press does both, and this test says so out loud rather than leaving
    /// the behaviour undescribed.
    #[test]
    fn a_press_on_a_branch_both_shuts_it_and_takes_the_selection() {
        let mut page = TreeView::default();
        assert!(selected(&page, "tv-component"));
        assert!(page.handle(&press(), "/page/tree/tv/tv/tv-gorgon/children/tv-petra"));
        assert!(!placed(&page, "tv-component"), "the branch did not shut");
        assert!(selected(&page, "tv-petra"));
    }

    /// A press that names nothing this page built is not consumed, so the
    /// chrome's own Prev/Next still work on this row.
    #[test]
    fn a_press_on_a_stranger_is_left_alone() {
        let mut page = TreeView::default();
        assert!(!page.handle(&press(), "/page/shell/main-scroll/main/nav/next"));
    }
}
