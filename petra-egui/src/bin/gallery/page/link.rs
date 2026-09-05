//! Inventory row 15, Link.

use gorgon_petra::component::{link, link_inline, section, text};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// The Link page. It holds no live state.
///
/// Both of Carbon's forms: the standalone link, which underlines under the
/// pointer, and the inline link set in a sentence, which is underlined at
/// rest. The second is on the page because the underline is the channel a
/// red-green colour-blind reader has, and a catalog that only showed the
/// form whose underline needs a pointer would not show it at all.
pub struct Link;

impl Page for Link {
    fn row(&self) -> &'static str {
        "Link"
    }

    fn body(&self) -> ViewNode {
        section(
            "links",
            "Link",
            vec![body(
                "link-row",
                sp("spacing.md"),
                vec![
                    link("docs", "Open the spec"),
                    row(
                        "prose",
                        None,
                        vec![
                            text("before", "Read "),
                            link_inline("docs-inline", "the inline form"),
                            text("after", " when a link sits in running text."),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
