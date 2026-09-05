//! Inventory row 3, Breadcrumb.

use gorgon_petra::component::{
    breadcrumb, breadcrumb_item, breadcrumb_item_current, button, disabled, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, path_has, row, sp, wrapped};

/// The path this page walks. One segment per level, deepest last.
const PATH: [&str; 4] = ["Workspace", "Fibers", "Rebuild", "Log"];

/// The key of the crumb at `level`. Stable across rebuilds, because the
/// chrome routes a press by the id the previous frame placed.
const KEYS: [&str; 4] = ["bc-0", "bc-1", "bc-2", "bc-3"];

/// The page's own control, not part of the component: it walks *down*, so
/// the trail this page demonstrates can be walked back up again.
const DEEPER: &str = "bc-deeper";

/// Live state of the Breadcrumb page: which level of [`PATH`] is the page
/// you are standing on.
///
/// The row was a unit struct whose `handle` returned `false`, with three
/// crumbs in one grey and no current page anywhere in the tree. The
/// operator's report was *"i dont understand this one"*. A trail that
/// cannot move cannot show what a trail is for, so the depth is state.
pub struct Breadcrumb {
    here: usize,
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self {
            here: PATH.len() - 1,
        }
    }
}

impl Page for Breadcrumb {
    fn row(&self) -> &'static str {
        "Breadcrumb"
    }

    fn body(&self) -> ViewNode {
        // Carbon shows the ancestors of the current page and the current
        // page, and nothing below it. So the trail is `PATH[..=here]`, and
        // walking up shortens it.
        let crumbs: Vec<ViewNode> = (0..=self.here)
            .map(|level| {
                if level == self.here {
                    breadcrumb_item_current(KEYS[level], PATH[level])
                } else {
                    breadcrumb_item(KEYS[level], PATH[level])
                }
            })
            .collect();

        let deeper = button(DEEPER, "Open the page below");
        let deeper = if self.here + 1 < PATH.len() {
            deeper
        } else {
            disabled(deeper)
        };

        section(
            "trail",
            "Trail",
            vec![body(
                "crumbs",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "note",
                        "A breadcrumb is the path to the page you are on. Every \
                         crumb before the last one is a link back up the tree: \
                         link ink, underlined under the pointer, and in tab \
                         order. The last crumb is where you are standing, so it \
                         is page ink, it is not a link, and the keyboard walks \
                         past it.",
                    ),
                    breadcrumb("crumbs", crumbs),
                    row("walk", sp("spacing.md"), vec![deeper]),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, DEEPER) {
            if self.here + 1 < PATH.len() {
                self.here += 1;
                return true;
            }
            return false;
        }
        // A crumb only ever walks up: the current one is not a link and
        // the levels below it are not in the trail at all.
        for (level, key) in KEYS.iter().enumerate().take(self.here) {
            if path_has(node, key) {
                self.here = level;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{Breadcrumb, KEYS, PATH};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};
    use gorgon_petra::tree::ViewNode;

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    /// Which keys the trail placed, and which one declares itself current.
    fn trail(page: &Breadcrumb) -> (Vec<String>, Option<String>) {
        let tree = page.body();
        let placed: Vec<String> = KEYS
            .iter()
            .filter(|key| find(&tree, key).is_some())
            .map(|key| (*key).to_owned())
            .collect();
        let current = KEYS
            .iter()
            .find(|key| find(&tree, key).is_some_and(|node: &ViewNode| node.semantics.selected))
            .map(|key| (*key).to_owned());
        (placed, current)
    }

    /// A press on a leading crumb walks up: the trail loses every level
    /// below the one pressed, and the pressed one becomes the current page.
    ///
    /// The old page was a unit struct returning `false` from `handle`, so
    /// this fails on the tree as well as on the picture when the handler
    /// goes away.
    #[test]
    fn a_press_on_a_leading_crumb_makes_it_the_current_page() {
        let mut page = Breadcrumb::default();
        assert_eq!(
            trail(&page),
            (
                KEYS.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>(),
                Some(KEYS[PATH.len() - 1].to_owned())
            )
        );
        assert!(page.handle(&press(), "/page/body/trail/crumbs/crumbs/bc-1"));
        assert_eq!(
            trail(&page),
            (
                vec![KEYS[0].to_owned(), KEYS[1].to_owned()],
                Some(KEYS[1].to_owned())
            )
        );
    }

    /// The current crumb is not a link, so a press on it changes nothing —
    /// and the page below it is reachable again through the page's own
    /// control, so the demonstration is reversible.
    #[test]
    fn the_current_crumb_is_inert_and_the_page_control_walks_back_down() {
        let mut page = Breadcrumb::default();
        assert!(page.handle(&press(), "/page/body/trail/crumbs/crumbs/bc-0"));
        assert_eq!(trail(&page).1.as_deref(), Some(KEYS[0]));
        assert!(
            !page.handle(&press(), "/page/body/trail/crumbs/crumbs/bc-0"),
            "the current crumb is not a link and must consume nothing"
        );
        assert!(page.handle(&press(), "/page/body/trail/walk/bc-deeper"));
        assert_eq!(trail(&page).1.as_deref(), Some(KEYS[1]));
    }
}
