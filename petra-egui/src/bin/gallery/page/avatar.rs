//! Catalog row 43, Avatar.

use gorgon_petra::component::{
    avatar, avatar_group, avatar_lg, avatar_md, avatar_xs, section,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, row, sp};

/// Live state of the Avatar page. Avatars are not interactive; the page
/// holds nothing to flip. The constructors still have to sit on the page
/// so a photograph can show sizes, a status pip, and a group.
pub struct Avatar;

impl Default for Avatar {
    fn default() -> Self {
        Self
    }
}

impl Page for Avatar {
    fn row(&self) -> &'static str {
        "Avatar"
    }

    fn body(&self) -> ViewNode {
        section(
            "faces",
            "Sizes and group",
            vec![body(
                "avs",
                sp("spacing.md"),
                vec![
                    row(
                        "sizes",
                        sp("spacing.md"),
                        vec![
                            avatar_xs("av-xs", "XS"),
                            avatar("av-def", "DF"),
                            avatar_md("av-md", "MD"),
                            avatar_lg("av-lg", "LG"),
                        ],
                    ),
                    avatar_group(
                        "av-group",
                        vec![
                            avatar("g0", "AB"),
                            avatar("g1", "CD"),
                            avatar("g2", "EF"),
                        ],
                        2,
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, _node: &str) -> bool {
        false
    }
}
