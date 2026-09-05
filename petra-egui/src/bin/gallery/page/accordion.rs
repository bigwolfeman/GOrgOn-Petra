//! Inventory row 1, Accordion.

use gorgon_petra::component::{accordion, accordion_item, accordion_item_with, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{filled_body, path_has, sp};

const ACC_0: &str = "acc-0";
const ACC_1: &str = "acc-1";
/// The outer item that carries the nested list.
const ACC_NEST: &str = "acc-nest";
/// The two items of the nested list, one level in.
const ACC_NEST_0: &str = "acc-nest-0";
const ACC_NEST_1: &str = "acc-nest-1";

/// Live state of the Accordion page: whether each section is expanded.
/// Carbon's accordion lets any number be open at once, so one independent
/// fact per header rather than one index.
///
/// The last three are the nested demonstration the operator asked for on
/// 2026-09-05. It opens expanded, and its first inner section opens
/// expanded too, so the nesting is visible in the resting photograph
/// instead of only after a click.
pub struct Accordion {
    open: [bool; 2],
    nest: bool,
    inner: [bool; 2],
}

impl Default for Accordion {
    fn default() -> Self {
        Self {
            open: [true, false],
            nest: true,
            inner: [true, false],
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
                        // Nested: an accordion inside an accordion item's
                        // panel. Carbon writes no rule for this either way
                        // — see `accordion_item_with`'s doc — so it is
                        // shown with that caveat rather than as conformance.
                        accordion_item_with(
                            ACC_NEST,
                            "Nested section",
                            self.nest,
                            vec![accordion(
                                "inner",
                                vec![
                                    accordion_item(
                                        ACC_NEST_0,
                                        "Nested first",
                                        self.inner[0],
                                        "One level in. The panel's own 16 \
                                         inline padding is the indent.",
                                    ),
                                    accordion_item(
                                        ACC_NEST_1,
                                        "Nested second",
                                        self.inner[1],
                                        "Each inner header toggles on its own.",
                                    ),
                                ],
                            )],
                        ),
                    ],
                )],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // The inner keys are tested first: a press inside the nested list
        // routes through the outer item, so its path carries both
        // `acc-nest` and `acc-nest-0`, and the outer arm would otherwise
        // swallow every inner header and shut the whole branch.
        if path_has(node, ACC_NEST_0) {
            self.inner[0] = !self.inner[0];
        } else if path_has(node, ACC_NEST_1) {
            self.inner[1] = !self.inner[1];
        } else if path_has(node, ACC_NEST) {
            self.nest = !self.nest;
        } else if path_has(node, ACC_0) {
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
    use super::{ACC_1, ACC_NEST, ACC_NEST_0, ACC_NEST_1, Accordion};
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

    /// The nested accordion the operator asked to see is a real accordion,
    /// not a picture of one: an inner header toggles only itself, and the
    /// outer header it is routed through keeps its own state.
    ///
    /// The routed path is what makes this worth asserting. A press on
    /// `Nested first` names `acc-nest` *and* `acc-nest-0` in the same path,
    /// so a handler that tests the outer key first shuts the whole branch on
    /// every inner press and the inner sections can never be opened at all.
    #[test]
    fn a_press_on_an_inner_header_toggles_only_that_inner_section() {
        let mut page = Accordion::default();
        let open = |page: &Accordion, key: &str| {
            let tree = page.body();
            let item = find(&tree, key).unwrap_or_else(|| panic!("{key} is not on the page"));
            let expanded = find(item, "header").unwrap().semantics.expanded;
            assert_eq!(
                expanded.is_some_and(|e| e),
                find(item, "body").is_some(),
                "{key}: the panel must be mounted exactly while expanded"
            );
            expanded
        };
        let press = InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        };
        // The nested list is inside the outer item's panel, so its own path
        // runs through it.
        let inner_1 = "/page/body/acc/acc-nest/body/inner/acc-nest-1/header";

        assert_eq!(open(&page, ACC_NEST), Some(true));
        assert_eq!(open(&page, ACC_NEST_0), Some(true));
        assert_eq!(open(&page, ACC_NEST_1), Some(false));

        assert!(page.handle(&press, inner_1));
        assert_eq!(
            open(&page, ACC_NEST_1),
            Some(true),
            "the inner header did not open its own section"
        );
        assert_eq!(
            open(&page, ACC_NEST),
            Some(true),
            "the press was swallowed by the outer item and shut the branch"
        );
        assert_eq!(
            open(&page, ACC_NEST_0),
            Some(true),
            "opening one inner section closed its sibling"
        );

        // And the outer header still works on its own: shutting it takes the
        // whole nested list out of the tree.
        assert!(page.handle(&press, "/page/body/acc/acc-nest/header"));
        assert_eq!(open(&page, ACC_NEST), Some(false));
        assert!(
            find(&page.body(), ACC_NEST_0).is_none(),
            "a shut outer panel must not leave its nested list mounted"
        );
    }
}
