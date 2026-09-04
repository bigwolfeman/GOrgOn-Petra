//! Carbon Content switcher (slice-b).
//!
//! Anatomy (docs + `_content-switcher.scss`):
//! 1. [`content_switcher`] — the row (`Role::TabList`), radius 4
//!    ([`SHAPE_SM`]), 1px [`BORDER_SUBTLE`] outline. High-contrast
//!    `$border-inverse` is not in the component vocabulary.
//! 2. [`content_switcher_item`] — one tab button, height 40 ([`SIZE_MD`]).
//!    Selected is never colour alone: [`LAYER_SELECTED`] plus
//!    `Semantics.selected` plus heading type.
//!
//! Icon-only and high-contrast-inverse fills are omitted: no extra icon
//! marks, and `layer-selected-inverse` is not a shipped token.

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SIZE_MD,
    SPACING_03, SPACING_05, SURFACE_BASE, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, Semantics, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// The switcher row. `Role::TabList`, no interactions of its own.
pub fn content_switcher(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, items);
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}

/// One switcher tab. `Role::Button`, selected declared in `Semantics`.
///
/// Height is Carbon md 40. Label padding is `$spacing-05` inline.
pub fn content_switcher_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let mut label_node = text("label", label.clone());
    label_node.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING
    } else {
        TYPOGRAPHY_BODY
    }));
    label_node.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
    );

    let mut node = stack(key, Axis::Horizontal, None, vec![label_node]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            LAYER_SELECTED
        } else {
            SURFACE_BASE
        }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));

    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, ITEM_INTENTS);
    node.semantics.selected = selected;
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
    use super::{
        BORDER_SUBTLE, LAYER_SELECTED, SHAPE_SM, SIZE_MD, content_switcher, content_switcher_item,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

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
    fn content_switcher_sets_role_tablist() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                content_switcher_item("b", "Grid", false),
            ],
        );
        assert_eq!(node.key.as_str(), "sw");
        assert_eq!(node.semantics.role, Some(Role::TabList));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(token(&node, "radius"), Some(SHAPE_SM));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn content_switcher_item_is_a_labelled_button_at_height_40() {
        let node = content_switcher_item("a", "List", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("List"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Hover));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(!node.semantics.selected);
    }

    #[test]
    fn content_switcher_item_declares_selected_not_colour_alone() {
        let on = content_switcher_item("a", "List", true);
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            named(&on, "label").props.style.as_ref().map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_HEADING)
        );

        let off = content_switcher_item("a", "List", false);
        assert!(!off.semantics.selected);
        assert_eq!(
            named(&off, "label")
                .props
                .style
                .as_ref()
                .map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_BODY)
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

    /// Check C/D: the switcher row and every item place with a real rect,
    /// none of them outside the row, whichever item is selected.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                content_switcher_item("b", "Grid", false),
                content_switcher_item("c", "Table", false),
            ],
        );
        let frame = petrify_lone(node);
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
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
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
            }
        }
    }

    /// Check F: an enabled item is reachable in focus order; a disabled one
    /// is not.
    #[test]
    fn enabled_items_are_reachable_and_disabled_ones_are_not() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                crate::component::disabled(content_switcher_item("b", "Grid", false)),
            ],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let a = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/a"))
            .expect("item a is placed");
        let b = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/b"))
            .expect("item b is placed");
        assert!(
            focus.order().iter().any(|o| o == &a.id),
            "enabled item a declares Focus but is not in focus order"
        );
        assert!(
            !focus.order().iter().any(|o| o == &b.id),
            "disabled item b must not be reachable"
        );
    }

    /// Check E: label ink against the item's own resting fill, in both
    /// themes, at both selection states — selected swaps to
    /// [`LAYER_SELECTED`]/heading type, so the ground and the ink both
    /// change together.
    #[test]
    fn item_label_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for selected in [true, false] {
                let item = content_switcher_item("a", "List", selected);
                let bg_name = item
                    .props
                    .tokens
                    .get("background")
                    .expect("item binds a resting background");
                let bg = color(&theme, bg_name.as_str());
                let label = named(&item, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "selected={selected} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    bg_name.as_str()
                );
            }
        }
    }
}
