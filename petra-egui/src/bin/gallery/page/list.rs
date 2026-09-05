//! Inventory row 16, List.

use gorgon_petra::component::{
    Bullet, BulletScheme, list_item, list_item_with, ordered_list, section, unordered_list,
    unordered_list_with,
};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, sp};

/// The List page. It holds no live state.
pub struct List;

/// One four-level chain under `scheme`, keyed from `prefix`.
///
/// Four levels because [`BulletScheme::Rotating`]'s cycle is four marks
/// long, and a picture of three of them does not show that the fourth wraps
/// back to the first. The operator asked for the Word / org-mode set on
/// 2026-09-05 and a capture that shows every mark is the evidence.
fn chain(prefix: &str, scheme: BulletScheme, labels: [&str; 4]) -> ViewNode {
    unordered_list_with(
        prefix.to_owned(),
        scheme,
        vec![list_item_with(
            format!("{prefix}-1"),
            labels[0],
            Some(unordered_list(
                format!("{prefix}-l2"),
                vec![list_item_with(
                    format!("{prefix}-2"),
                    labels[1],
                    Some(unordered_list(
                        format!("{prefix}-l3"),
                        vec![list_item_with(
                            format!("{prefix}-3"),
                            labels[2],
                            Some(unordered_list(
                                format!("{prefix}-l4"),
                                vec![list_item(format!("{prefix}-4"), labels[3])],
                            )),
                        )],
                    )),
                )],
            )),
        )],
    )
}

impl Page for List {
    fn row(&self) -> &'static str {
        "List"
    }

    fn body(&self) -> ViewNode {
        section(
            "kinds",
            "Ordered, unordered, nested",
            vec![body(
                "lists",
                sp("spacing.md"),
                vec![
                    // Carbon's own two marks, unchanged: an en dash, then
                    // the small square at every level under it.
                    chain(
                        "ul",
                        BulletScheme::Carbon,
                        ["Inbox", "Archive", "2026", "March"],
                    ),
                    // The Word / org-mode rotation. Four levels, so all four
                    // marks are in one capture: disc, ring, square, dash.
                    chain(
                        "rot",
                        BulletScheme::Rotating,
                        ["Disc", "Ring", "Square", "Dash"],
                    ),
                    // One named mark, at every level.
                    unordered_list_with(
                        "fix",
                        BulletScheme::Fixed(Bullet::Square),
                        vec![
                            list_item("fix-0", "Fixed square"),
                            list_item("fix-1", "at every level"),
                        ],
                    ),
                    // Ordered nests too, and past level 2: decimal, then
                    // lower-latin, then lower-roman.
                    ordered_list(
                        "ol",
                        vec![
                            list_item("ol-0", "Clone"),
                            list_item_with(
                                "ol-1",
                                "Build",
                                Some(ordered_list(
                                    "ol-l2",
                                    vec![
                                        list_item("ol-1-0", "Compile"),
                                        list_item_with(
                                            "ol-1-1",
                                            "Link",
                                            Some(ordered_list(
                                                "ol-l3",
                                                vec![
                                                    list_item("ol-1-1-0", "Static"),
                                                    list_item("ol-1-1-1", "Dynamic"),
                                                ],
                                            )),
                                        ),
                                    ],
                                )),
                            ),
                            list_item("ol-2", "Run"),
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

#[cfg(test)]
mod tests {
    use super::List;
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::component::{Bullet, BulletScheme};
    use gorgon_petra::component::{IconTone, icon_toned};

    /// Every bullet the library draws is on this page at once, so one
    /// capture answers the operator's ask. Read off each marker's own draw
    /// list: a bullet carries no text and this operator cannot use hue, so
    /// the picture is the only channel there is.
    #[test]
    fn the_page_draws_every_bullet_the_library_has() {
        let tree = List.body();
        let mut drawn = Vec::new();
        fn collect(node: &gorgon_petra::tree::ViewNode, out: &mut Vec<Vec<u8>>) {
            if node.key.as_str() == "marker"
                && let Some(canvas) = &node.props.canvas
            {
                out.push(format!("{:?}", canvas.commands()).into_bytes());
            }
            for child in &node.children {
                collect(child, out);
            }
        }
        collect(&tree, &mut drawn);
        for bullet in [Bullet::Dash, Bullet::Disc, Bullet::Circle, Bullet::Square] {
            let want: Vec<u8> = format!(
                "{:?}",
                icon_toned("m", bullet.mark(), IconTone::Primary)
                    .props
                    .canvas
                    .expect("a bullet is a canvas")
                    .commands()
            )
            .into_bytes();
            assert!(
                drawn.contains(&want),
                "{bullet:?} is not on the page, so the capture cannot show it"
            );
        }
    }

    /// The rotating chain nests four deep and the ordered chain three, both
    /// in the built tree. A page that only claims to nest is a page whose
    /// photograph shows one level.
    #[test]
    fn the_page_nests_four_levels_of_bullets_and_three_of_counters() {
        let tree = List.body();
        for key in ["rot-1", "rot-2", "rot-3", "rot-4"] {
            assert!(find(&tree, key).is_some(), "{key} is not on the page");
        }
        for key in ["ol-0", "ol-1-0", "ol-1-1-0"] {
            assert!(find(&tree, key).is_some(), "{key} is not on the page");
        }
        assert_eq!(BulletScheme::default(), BulletScheme::Carbon);
    }
}
