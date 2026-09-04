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
    // Resting background: the bar it sits on (`pagination` binds
    // `SURFACE_RAISED`). Without this, `background@hover` has no resting
    // `background` beneath it and resolves to nothing at rest — the
    // Accordion/Modal/AI-label/data-table/date-picker/notification defect,
    // generalised (see
    // `a_state_decorated_token_always_has_a_resting_binding`).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
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
    use crate::component::tokens::{LAYER_HOVER, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

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

    #[test]
    fn nav_buttons_carry_a_resting_background_under_their_hover_state() {
        let node = pagination("pages", 2, 5);
        for key in ["previous", "next"] {
            let button = child(&node, key);
            assert_eq!(
                button.props.tokens.get("background@hover").map(|t| t.as_str()),
                Some(LAYER_HOVER)
            );
            assert_eq!(
                button.props.tokens.get("background").map(|t| t.as_str()),
                Some(SURFACE_RAISED),
                "{key}: a resting `background` must be bound alongside \
                 `background@hover`, or the nav button paints nothing when \
                 it is not hovered — the paint pass counts that as silent, \
                 not empty"
            );
        }
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

    /// Check C/D: page 1 (previous disabled), the last page (next
    /// disabled), and a mid-run page (both enabled) all place with real
    /// rects, none of them outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for (label, node) in [
            ("first", pagination("pages", 1, 4)),
            ("last", pagination("pages", 4, 4)),
            ("mid", pagination("pages", 2, 5)),
        ] {
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

    /// Check F: an enabled nav button is reachable; a disabled one is not.
    #[test]
    fn a_disabled_nav_button_is_not_reachable() {
        let frame = petrify_lone(pagination("pages", 1, 4));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let previous = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/previous"))
            .expect("previous is placed");
        let next = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/next"))
            .expect("next is placed");
        assert!(
            !order.iter().any(|o| o == &previous.id),
            "a disabled previous button must not be reachable"
        );
        assert!(
            order.iter().any(|o| o == &next.id),
            "an enabled next button declares Focus but is not in focus order"
        );
    }

    /// Check E: the current-page text and both nav labels against the
    /// bar's own resting fill, in both themes.
    #[test]
    fn bar_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = pagination("pages", 2, 5);
            let bar_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("pagination binds a resting background");
            let bar_bg = color(&theme, bar_bg_name.as_str());
            fn walk_text(
                node: &ViewNode,
                inherited_bg: ColorValue,
                theme: &Theme,
                min: f32,
                get_color: &impl Fn(&Theme, &str) -> ColorValue,
            ) {
                let bg = match node.props.tokens.get("background") {
                    Some(name) => get_color(theme, name.as_str()),
                    None => inherited_bg,
                };
                if node.props.text.is_some() {
                    if let Some(fg_name) = node.props.tokens.get("foreground") {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        let fg = get_color(theme, fg_name.as_str())
                            .faded(opacity)
                            .over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= min,
                            "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                            node.key,
                            fg_name.as_str()
                        );
                    }
                }
                for child in &node.children {
                    walk_text(child, bg, theme, min, get_color);
                }
            }
            walk_text(&node, bar_bg, &theme, MIN_TEXT_CONTRAST, &color);
        }
    }
}
