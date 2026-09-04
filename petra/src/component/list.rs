//! Carbon List: ordered, unordered, and nested static markers (slice-c).
//!
//! The list container is not interactive (FR-058). List items that are only
//! text are not interactive either. Markers are generated here because Petra
//! has no CSS counters and no `::before` content; Carbon's hanging indent is
//! a horizontal stack of marker + label.

use std::sync::Arc;

use super::stack;
use super::text::text;
use super::tokens::{SPACING_02, SPACING_03, SPACING_05, SPACING_07, t};
use crate::geom::Axis;
use crate::tree::{InsetRefs, Key, Role, Semantics, ViewNode};

/// Unordered level-1 marker: en dash U+2013.
const MARKER_UNORDERED_L1: &str = "\u{2013}";
/// Unordered level-2 marker: black small square U+25AA plus VS15 so it
/// stays a text glyph, matching Carbon's `\0025AA\00FE0E`.
const MARKER_UNORDERED_L2: &str = "\u{25AA}\u{FE0E}";

#[derive(Clone, Copy)]
enum Kind {
    Unordered,
    Ordered,
}

#[derive(Clone, Copy)]
enum Level {
    One,
    Two,
}

/// An unordered list. Level-1 markers are en dashes. `Role::List`, no
/// interactions, container `margin-inline-start` [`SPACING_05`].
pub fn unordered_list(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        None,
        stamp_items(items, Kind::Unordered, Level::One),
    );
    pad_inline_start(&mut node, SPACING_05);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    node
}

/// An ordered list. Level-1 markers are `1.` `2.` … generated here.
/// `Role::List`, no interactions.
pub fn ordered_list(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        None,
        stamp_items(items, Kind::Ordered, Level::One),
    );
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    node
}

/// One list row: marker text + label text, hanging indent via a horizontal
/// stack. Default marker is the unordered level-1 en dash; an
/// [`ordered_list`] parent restamps it. `Role::ListItem`, no interactions.
pub fn list_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    list_item_with(key, label, None)
}

/// [`list_item`] plus an optional nested [`unordered_list`] / [`ordered_list`].
///
/// Nested indent is Carbon's 32px (`spacing-07`) on the nested container and
/// [`SPACING_02`] on each nested item (T070: SCSS wins over the style-page
/// `$spacing-05`). Nested unordered markers become the small square; nested
/// ordered markers become `a.` `b.` …
pub fn list_item_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    nested: Option<ViewNode>,
) -> ViewNode {
    let label = label.into();
    match nested {
        None => with_list_item_role(item_row(key, MARKER_UNORDERED_L1, &label), label),
        Some(nested) => {
            let row = item_row("row", MARKER_UNORDERED_L1, &label);
            with_list_item_role(
                stack(key, Axis::Vertical, None, vec![row, nest(nested)]),
                label,
            )
        }
    }
}

fn with_list_item_role(mut node: ViewNode, label: String) -> ViewNode {
    node.semantics = Semantics {
        role: Some(Role::ListItem),
        label: Some(label),
        ..Semantics::default()
    };
    node
}

fn item_row(key: impl Into<Key>, marker: &str, label: &str) -> ViewNode {
    stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![text("marker", marker), text("label", label)],
    )
}

fn stamp_items(items: Vec<ViewNode>, kind: Kind, level: Level) -> Vec<ViewNode> {
    items
        .into_iter()
        .enumerate()
        .map(|(i, mut item)| {
            set_marker(&mut item, &marker_for(kind, level, i));
            if matches!(level, Level::Two) {
                pad_inline_start(&mut item, SPACING_02);
            }
            item
        })
        .collect()
}

fn nest(mut list: ViewNode) -> ViewNode {
    if list.semantics.role != Some(Role::List) {
        return list;
    }
    let ordered = list
        .children
        .first()
        .is_some_and(|item| marker_is_ordered(item));
    let kind = if ordered {
        Kind::Ordered
    } else {
        Kind::Unordered
    };
    let items: Vec<ViewNode> = list.children.drain(..).map(Arc::unwrap_or_clone).collect();
    list.children = stamp_items(items, kind, Level::Two)
        .into_iter()
        .map(Arc::new)
        .collect();
    pad_inline_start(&mut list, SPACING_07);
    list
}

fn marker_for(kind: Kind, level: Level, index: usize) -> String {
    match (kind, level) {
        (Kind::Unordered, Level::One) => MARKER_UNORDERED_L1.to_string(),
        (Kind::Unordered, Level::Two) => MARKER_UNORDERED_L2.to_string(),
        (Kind::Ordered, Level::One) => format!("{}.", index + 1),
        (Kind::Ordered, Level::Two) => latin_marker(index),
    }
}

fn latin_marker(index: usize) -> String {
    let mut chars = Vec::new();
    let mut n = index;
    loop {
        chars.push(char::from(b'a' + (n % 26) as u8));
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    chars.reverse();
    let mut out: String = chars.into_iter().collect();
    out.push('.');
    out
}

fn marker_is_ordered(item: &ViewNode) -> bool {
    match marker_text(item) {
        Some(marker) if marker.ends_with('.') => {
            let body = &marker[..marker.len() - 1];
            !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric())
        }
        _ => false,
    }
}

fn marker_text(node: &ViewNode) -> Option<&str> {
    for child in &node.children {
        match child.key.as_str() {
            "marker" => return child.props.text.as_deref(),
            "row" => return marker_text(child),
            _ => {}
        }
    }
    None
}

fn set_marker(item: &mut ViewNode, marker: &str) {
    for child in &mut item.children {
        let child = Arc::make_mut(child);
        match child.key.as_str() {
            "marker" => {
                child.props.text = Some(marker.to_string());
                return;
            }
            "row" => {
                set_marker(child, marker);
                return;
            }
            _ => {}
        }
    }
}

fn pad_inline_start(node: &mut ViewNode, token: &str) {
    node.props.padding = Some(InsetRefs {
        left: Some(t(token)),
        ..InsetRefs::default()
    });
}

#[cfg(test)]
mod tests {
    use super::{
        MARKER_UNORDERED_L1, MARKER_UNORDERED_L2, SPACING_02, SPACING_07, list_item,
        list_item_with, marker_text, ordered_list, unordered_list,
    };
    use crate::tree::{Role, ViewNode};

    fn padding_left(node: &ViewNode) -> Option<&str> {
        node.props
            .padding
            .as_ref()
            .and_then(|pad| pad.left.as_ref())
            .map(|name| name.as_str())
    }

    fn uses_padding_token(node: &ViewNode, token: &str) -> bool {
        padding_left(node) == Some(token)
            || node
                .children
                .iter()
                .any(|child| uses_padding_token(child, token))
    }

    fn nested_list(item: &ViewNode) -> &ViewNode {
        item.children
            .iter()
            .find(|child| child.semantics.role == Some(Role::List))
            .expect("list_item_with nests a Role::List child")
    }

    #[test]
    fn unordered_list_sets_role_list() {
        let node = super::unordered_list("u", vec![super::list_item("a", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }

    #[test]
    fn ordered_list_sets_role_list() {
        let node = super::ordered_list("o", vec![super::list_item("a", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }

    #[test]
    fn list_item_sets_role_list_item() {
        let node = super::list_item("a", "Alpha");
        assert_eq!(node.semantics.role, Some(Role::ListItem));
        assert_eq!(node.semantics.label.as_deref(), Some("Alpha"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(marker_text(&node), Some(MARKER_UNORDERED_L1));
    }

    #[test]
    fn ordered_list_generates_numbered_markers() {
        let node = super::ordered_list(
            "o",
            vec![
                super::list_item("a", "Alpha"),
                super::list_item("b", "Bravo"),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_eq!(marker_text(&node.children[0]), Some("1."));
        assert_eq!(marker_text(&node.children[1]), Some("2."));
        assert_eq!(node.children[0].semantics.role, Some(Role::ListItem));
        assert_eq!(node.children[1].semantics.role, Some(Role::ListItem));
    }

    #[test]
    fn nested_indent_uses_spacing_02() {
        let nested = super::unordered_list("inner", vec![super::list_item("n", "Nested")]);
        let item = super::list_item_with("outer-item", "Outer", Some(nested));
        assert_eq!(item.semantics.role, Some(Role::ListItem));
        let nested = nested_list(&item);
        assert_eq!(nested.semantics.role, Some(Role::List));
        assert_eq!(padding_left(nested), Some(SPACING_07));
        assert_eq!(padding_left(&nested.children[0]), Some(SPACING_02));
        assert!(
            uses_padding_token(&item, SPACING_02),
            "nested indent must bind SPACING_02"
        );
    }

    #[test]
    fn nested_unordered_uses_square_marker() {
        let nested = unordered_list("inner", vec![list_item("n", "Nested")]);
        let item = list_item_with("outer-item", "Outer", Some(nested));
        let nested = nested_list(&item);
        assert_eq!(marker_text(&nested.children[0]), Some(MARKER_UNORDERED_L2));
    }

    #[test]
    fn nested_ordered_uses_latin_markers() {
        let nested = ordered_list(
            "inner",
            vec![list_item("a", "Alpha"), list_item("b", "Bravo")],
        );
        let item = list_item_with("outer-item", "Outer", Some(nested));
        let nested = nested_list(&item);
        assert_eq!(marker_text(&nested.children[0]), Some("a."));
        assert_eq!(marker_text(&nested.children[1]), Some("b."));
        assert_eq!(padding_left(&nested.children[0]), Some(SPACING_02));
    }
}
