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
    use crate::tree::{Role, ViewNode};

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
}
