//! Inventory row 1, Accordion.

use gorgon_petra::component::{accordion, accordion_item, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const ACC_0: &str = "acc-0";
const ACC_1: &str = "acc-1";

/// Live state of the Accordion page: whether each of the two sections is
/// expanded. Carbon's accordion lets any number be open at once, so two
/// independent facts rather than one index.
pub struct Accordion {
    open: [bool; 2],
}

impl Default for Accordion {
    fn default() -> Self {
        Self {
            open: [true, false],
        }
    }
}

impl Page for Accordion {
    fn row(&self) -> &'static str {
        "Accordion"
    }

    fn body(&self) -> ViewNode {
        section(
            "items",
            "Items",
            vec![filled_body(
                "accordion",
                sp("spacing.md"),
                vec![accordion(
                    "acc",
                    vec![
                        accordion_item(
                            ACC_0,
                            "First section",
                            self.open[0],
                            "The fibers scheduled this pass.",
                        ),
                        accordion_item(
                            ACC_1,
                            "Second section",
                            self.open[1],
                            "The trace written this session.",
                        ),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, ACC_0) {
            self.open[0] = !self.open[0];
        } else if path_has(node, ACC_1) {
            self.open[1] = !self.open[1];
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{ACC_1, Accordion};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    /// The second section was built with a literal `false` and no handler
    /// arm, so a press on its header changed nothing. Now it expands.
    #[test]
    fn a_press_on_the_second_header_expands_the_second_section() {
        let mut page = Accordion::default();
        // `expanded` is declared on the item's header button, and the body
        // is in the tree only while expanded; both are read.
        let expanded = |page: &Accordion| {
            let tree = page.body();
            let item = find(&tree, ACC_1).unwrap();
            let header = find(item, "header").unwrap();
            (header.semantics.expanded, find(item, "body").is_some())
        };
        assert_eq!(expanded(&page), (Some(false), false));
        let press = InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        assert!(page.handle(&press, "/page/body/acc/acc-1/header"));
        assert_eq!(expanded(&page), (Some(true), true));
        assert!(page.handle(&press, "/page/body/acc/acc-1/header"));
        assert_eq!(expanded(&page), (Some(false), false));
    }
}
