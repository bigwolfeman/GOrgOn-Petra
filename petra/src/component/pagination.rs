//! Carbon Pagination (slice-d).
//!
//! Anatomy of the bar variant (usage page + `_pagination.scss`):
//! 1. Container — `$layer` fill, 1px `$border-subtle` edge, height md 40.
//! 2. Current-page text plus `Semantics.value` (the page number). The
//!    nested Select that Carbon uses for the page picker is Wave 3
//!    Popover; this constructor does not fake a dropdown.
//! 3. Previous / Next — [`Role::Button`] with labels `"Previous"` /
//!    `"Next"`, never icon-only (FR-026). Page 1 disables Previous
//!    via [`super::disabled`]; the last page disables Next.
//!
//! Items-per-page is a Select. It is not invented here. Pagination nav
//! (page-number buttons) is a second Carbon variant and is omitted.

use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_PRIMARY, t,
};
use super::{disabled, pad};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Interaction, Key, Role, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const NAV_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Pagination bar at Carbon md (40). `page` is 1-indexed.
///
/// `page_count` is the last page number. Previous is unavailable on page
/// 1; Next is unavailable on the last page (and when there are no pages).
pub fn pagination(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    let mut current = text("page", format!("{page} of {page_count}"));
    current
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    current.semantics.value = Some(page.to_string());

    let previous = nav_button("previous", "Previous", page <= 1);
    let next = nav_button("next", "Next", page_count == 0 || page >= page_count);

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![current, previous, next],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.constraints.vertical.min = Some(SIZE_MD);
    node
}

fn nav_button(key: &'static str, label: &'static str, unavailable: bool) -> ViewNode {
    let mut caption = text("label", label);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, NAV_INTENTS);
    if unavailable { disabled(node) } else { node }
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
    use super::{SIZE_MD, pagination};
    use crate::tree::{Interaction, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn child_keys(node: &ViewNode) -> Vec<&str> {
        node.children.iter().map(|c| c.key.as_str()).collect()
    }

    #[test]
    fn pagination_is_size_md_with_labelled_prev_next() {
        let node = pagination("pages", 2, 5);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(child_keys(&node), ["page", "previous", "next"]);

        let previous = child(&node, "previous");
        assert_eq!(previous.semantics.role, Some(Role::Button));
        assert_eq!(previous.semantics.label.as_deref(), Some("Previous"));
        assert!(previous.interactions.contains(&Interaction::Click));
        assert!(!previous.semantics.disabled);

        let next = child(&node, "next");
        assert_eq!(next.semantics.role, Some(Role::Button));
        assert_eq!(next.semantics.label.as_deref(), Some("Next"));
        assert!(next.interactions.contains(&Interaction::Click));
        assert!(!next.semantics.disabled);
    }

    #[test]
    fn pagination_current_page_is_text_plus_value() {
        let node = pagination("pages", 3, 10);
        let page = child(&node, "page");
        assert_eq!(page.props.text.as_deref(), Some("3 of 10"));
        assert_eq!(page.semantics.value.as_deref(), Some("3"));
        assert!(
            page.interactions.is_empty(),
            "current page is text, not a Select"
        );
        assert!(
            !child_keys(&node).iter().any(|k| k.contains("size")
                || *k == "items"
                || *k == "select"
                || *k == "page-size")
        );
    }

    #[test]
    fn pagination_disables_prev_on_page_one() {
        let node = pagination("pages", 1, 4);
        let previous = child(&node, "previous");
        assert!(previous.semantics.disabled);
        assert!(!previous.interactions.contains(&Interaction::Click));
        let next = child(&node, "next");
        assert!(!next.semantics.disabled);
        assert!(next.interactions.contains(&Interaction::Click));
    }

    #[test]
    fn pagination_disables_next_on_the_last_page() {
        let node = pagination("pages", 4, 4);
        let next = child(&node, "next");
        assert!(next.semantics.disabled);
        assert!(!next.interactions.contains(&Interaction::Click));
        let previous = child(&node, "previous");
        assert!(!previous.semantics.disabled);
        assert!(previous.interactions.contains(&Interaction::Click));
    }
}
