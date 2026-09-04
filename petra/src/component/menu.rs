//! Carbon Menu (slice-c). Floating action list on Popover.
//!
//! Anatomy (`_menu.scss`):
//! 1. Menu container — [`super::popover::popover_with`], [`Role::Overlay`].
//! 2. Action item — [`menu_item`]: [`Role::Button`], height md 40.
//!
//! Container width is min 160 / max 288 (SCSS `$supported-sizes` map and
//! style-page). Item padding is [`SPACING_05`] inline. Submenus, danger
//! hover, and the `--with-icons` column are omitted.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon menu min-inline (`10rem`).
const MIN_INLINE: f32 = 160.0;
/// Carbon menu max-inline (`18rem`).
const MAX_INLINE: f32 = 288.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MIN_INLINE == 160.0);
const _: () = assert!(MAX_INLINE == 288.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A floating menu of `items`, hosted on a popover.
///
/// Anchored to a sibling keyed `"trigger"` (the key [`super::menu_button`]
/// uses for its button). `label` is the accessible name of the overlay.
pub fn menu(key: impl Into<Key>, label: impl Into<String>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = popover_with(key, label, "trigger", items);
    node.constraints.horizontal = AxisConstraint {
        min: Some(MIN_INLINE),
        max: Some(MAX_INLINE),
        priority: 0,
    };
    node
}

/// One action row. `label` is required (FR-058).
pub fn menu_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, ITEM_INTENTS)
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
    use super::{MAX_INLINE, MIN_INLINE, SIZE_MD, menu, menu_item};
    use crate::component::disabled;
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Anchor, Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn menu_is_an_overlay_popover_not_a_dialog() {
        let node = menu(
            "actions",
            "Actions",
            vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")],
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.interactions.is_empty());
        match &node.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MIN_INLINE, 160.0);
        assert_eq!(MAX_INLINE, 288.0);
        let content = child(&node, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        let rename = child(content, "rename");
        assert_eq!(rename.semantics.role, Some(Role::Button));
        assert_eq!(rename.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(
            child(content, "delete").semantics.label.as_deref(),
            Some("Delete")
        );
    }

    #[test]
    fn menu_item_is_a_labelled_button_at_height_40() {
        let node = menu_item("rename", "Rename");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Rename"));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Rename"));
    }

    /// `menu()` IS the anchored surface, naming a `trigger` sibling by bare
    /// key (`Anchor::Sibling`). Placed beside a control keyed `trigger`, it
    /// is accepted wherever the caller mounts the pair — here two
    /// containers below the root, the gallery catalog's own depth.
    #[test]
    fn menu_validates_beside_its_trigger_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "menu",
            vec![
                crate::component::button("trigger", "Actions"),
                menu(
                    "actions",
                    "Actions",
                    vec![menu_item("rename", "Rename"), menu_item("delete", "Delete")],
                ),
            ],
        );
    }

    // `menu_item` carries no anchor, so — unlike `date_picker`'s
    // `day_button`, which is reachable only inside the anchored calendar —
    // it is audited below with a real `petrify_lone`, the same as any
    // standalone component.

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

    /// Check C/D: two menu items, one of them disabled, place with real
    /// rects, none of them outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let items = ViewNode::new(NodeKind::Stack, "items")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(menu_item("rename", "Rename"))
            .child(disabled(menu_item("delete", "Delete")));
        let frame = petrify_lone(items);
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

    /// Check F: an enabled item is reachable; a disabled one is not.
    #[test]
    fn an_enabled_item_is_reachable_and_a_disabled_one_is_not() {
        let frame = petrify_lone(menu_item("rename", "Rename"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let item = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/rename"))
            .expect("the item is placed");
        assert!(
            order.iter().any(|o| o == &item.id),
            "the item declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(disabled(menu_item("delete", "Delete")));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_item = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/delete"))
            .expect("the disabled item is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_item.id),
            "a disabled item must not be reachable"
        );
    }

    /// Check E: the item's label against the item's own resting fill, in
    /// both themes.
    #[test]
    fn item_label_clears_aa_contrast_against_its_own_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = menu_item("rename", "Rename");
            let bg_name = node
                .props
                .tokens
                .get("background")
                .expect("menu_item binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let caption = child(&node, "label");
            let fg_name = caption
                .props
                .tokens
                .get("foreground")
                .expect("label binds a foreground");
            let opacity = caption.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "menu item label at {ratio:.2}:1 against {} fails AA \
                 {MIN_TEXT_CONTRAST}:1",
                fg_name.as_str()
            );
        }
    }
}
