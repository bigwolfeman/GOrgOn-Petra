//! Carbon Contained list (slice-a).
//!
//! Not [`super::list`]. This is a header + row list for related content
//! inside a container (card, sidebar, popover).
//!
//! Anatomy (docs + `_contained-list.scss`):
//! 1. [`contained_list`] — outer container, `Role::List`.
//! 2. Header — on-page height 32 (`$spacing-07`); disclosed height 48
//!    ([`contained_list_disclosed`]).
//! 3. Title — `heading` on-page, muted `text` when disclosed.
//! 4. Rows — caller-supplied children, stacked under the header.
//!
//! On-page header has no fill. Disclosed header fills [`SURFACE_RAISED`].

use super::stack;
use super::text::{heading, text};
use super::tokens::{SPACING_05, SURFACE_RAISED, TEXT_MUTED, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, Role, Semantics, ViewNode};

/// Carbon on-page header height (`$spacing-07` = 32).
const HEADER_ON_PAGE: f32 = 32.0;
/// Carbon disclosed header height (style-page Structure: `$spacing-09` = 48).
const HEADER_DISCLOSED: f32 = 48.0;

const _: () = assert!(HEADER_ON_PAGE == 32.0);
const _: () = assert!(HEADER_DISCLOSED == 48.0);

/// On-page contained list: header 32, no header fill, `Role::List`.
pub fn contained_list(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    contained(key, title, items, HEADER_ON_PAGE, false)
}

/// Disclosed contained list: header 48, [`SURFACE_RAISED`] header fill.
pub fn contained_list_disclosed(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    contained(key, title, items, HEADER_DISCLOSED, true)
}

fn contained(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
    header_h: f32,
    disclosed: bool,
) -> ViewNode {
    let title = title.into();
    let title_node = if disclosed {
        let mut node = text("title", title.clone());
        node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
        node
    } else {
        heading("title", title.clone())
    };

    let mut header = stack("header", Axis::Horizontal, None, vec![title_node]);
    header.props.align = Some(Align::Center);
    // Height is pinned; only the documented 16px inline pad is bound.
    header.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    if disclosed {
        header
            .props
            .tokens
            .insert("background".into(), t(SURFACE_RAISED));
    }
    let header = header.with_constraints(pin_height(header_h));

    let mut children = Vec::with_capacity(items.len() + 1);
    children.push(header);
    children.extend(items);

    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics = Semantics {
        role: Some(Role::List),
        label: Some(title),
        ..Semantics::default()
    };
    node
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::text::text;
    use super::{
        HEADER_DISCLOSED, HEADER_ON_PAGE, SURFACE_RAISED, contained_list, contained_list_disclosed,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn contained_list_sets_role_list_and_header_is_32() {
        let node = contained_list(
            "cl",
            "Related",
            vec![text("r0", "Alpha"), text("r1", "Bravo")],
        );
        assert_eq!(node.key.as_str(), "cl");
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_eq!(node.semantics.label.as_deref(), Some("Related"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        let header = named(&node, "header");
        assert_eq!(header.constraints.vertical.min, Some(HEADER_ON_PAGE));
        assert_eq!(header.constraints.vertical.max, Some(HEADER_ON_PAGE));
        assert_eq!(HEADER_ON_PAGE, 32.0);
        assert_eq!(token(header, "background"), None);
        named(&node, "r0");
        named(&node, "r1");
    }

    #[test]
    fn contained_list_disclosed_header_is_48_and_raised() {
        let node = contained_list_disclosed("cl", "Menu", vec![text("r0", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        let header = named(&node, "header");
        assert_eq!(header.constraints.vertical.min, Some(HEADER_DISCLOSED));
        assert_eq!(header.constraints.vertical.max, Some(HEADER_DISCLOSED));
        assert_eq!(HEADER_DISCLOSED, 48.0);
        assert_eq!(token(header, "background"), Some(SURFACE_RAISED));
    }

    #[test]
    fn contained_list_title_is_visible() {
        let node = contained_list("cl", "Inbox", vec![]);
        assert_eq!(named(&node, "title").props.text.as_deref(), Some("Inbox"));
        let disclosed = contained_list_disclosed("cl", "Inbox", vec![]);
        assert_eq!(
            named(&disclosed, "title").props.text.as_deref(),
            Some("Inbox")
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D across both header heights: no degenerate rect, no child
    /// placed outside its parent. Neither variant declares an interaction
    /// of its own (`Role::List` on the container, caller-supplied rows), so
    /// there is no Check F for this file — nothing here declares `Focus`.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            (
                "on-page",
                contained_list("cl", "Related", vec![text("r0", "Alpha"), text("r1", "Bravo")]),
            ),
            (
                "disclosed",
                contained_list_disclosed("cl", "Menu", vec![text("r0", "Charlie")]),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check E: the on-page title (`heading`) and the disclosed title
    /// (muted `text`) against their own header fill -- the on-page header
    /// has none, so it inherits the page ground (`surface.base`); the
    /// disclosed header binds `surface.raised`. Both in both themes, read
    /// through `Props.opacity`.
    #[test]
    fn header_title_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node, page_ground) in [
                ("on-page", contained_list("cl", "Related", vec![]), true),
                (
                    "disclosed",
                    contained_list_disclosed("cl", "Menu", vec![]),
                    false,
                ),
            ] {
                let header = named(&node, "header");
                let bg = if page_ground {
                    color(&theme, SURFACE_BASE)
                } else {
                    let bg_name = header
                        .props
                        .tokens
                        .get("background")
                        .expect("disclosed header binds a background");
                    color(&theme, bg_name.as_str())
                };
                let title = named(header, "title");
                let fg_name = title
                    .props
                    .tokens
                    .get("foreground")
                    .expect("title binds a foreground");
                let opacity = title.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: title at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
