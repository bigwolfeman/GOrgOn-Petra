//! Carbon Menu buttons (slice-c). A labelled button that opens a Menu.
//!
//! Anatomy (`_menu-button.scss`): trigger button + caret + Menu. Combo
//! button and Overflow menu are omitted (split trigger / icon-only).
//!
//! The caret is the word `"open"` / `"closed"`, never an icon-only mark
//! (FR-026). Carbon rotates a chevron 180°; the word is the second channel
//! that rotation cannot be.

use super::menu::menu;
use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, SHADOW_RAISED, SHAPE_MD, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled menu trigger. When `open` is true the node also carries a
/// [`menu`] popover of `items`. `label` is required (FR-058).
pub fn menu_button(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger("trigger", label.clone(), open);
    let mut children = vec![trigger];
    if open {
        children.push(menu("menu", label, items));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn trigger(key: impl Into<Key>, label: String, open: bool) -> ViewNode {
    let caret = if open { "open" } else { "closed" };
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron = text("caret", caret);
    chevron
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![caption, chevron],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props.tokens.insert("radius".into(), t(SHAPE_MD));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    let mut node = node.with_constraints(pin_height(SIZE_MD)).interactive(
        Role::Button,
        label,
        TRIGGER_INTENTS,
    );
    node.semantics.expanded = Some(open);
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
    use super::{SIZE_MD, menu_button};
    use crate::component::disabled;
    use crate::component::menu::menu_item;
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

    fn items() -> Vec<ViewNode> {
        vec![menu_item("rename", "Rename")]
    }

    #[test]
    fn menu_button_closed_is_a_labelled_trigger_without_a_menu() {
        let node = menu_button("more", "More", false, items());
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1, "closed trigger has no popover");
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("More"));
        assert_eq!(trigger.semantics.expanded, Some(false));
        assert_eq!(trigger.constraints.vertical.min, Some(SIZE_MD));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert_eq!(child(trigger, "label").props.text.as_deref(), Some("More"));
        assert_eq!(
            child(trigger, "caret").props.text.as_deref(),
            Some("closed")
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    #[test]
    fn menu_button_open_includes_a_menu_popover() {
        let node = menu_button("more", "More", true, items());
        assert_eq!(node.semantics.expanded, Some(true));
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.expanded, Some(true));
        assert_eq!(child(trigger, "caret").props.text.as_deref(), Some("open"));

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("More"));
        match &menu.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "trigger"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(menu, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(
            child(content, "rename").semantics.label.as_deref(),
            Some("Rename")
        );
    }

    // Only the CLOSED trigger is audited at the frame level below. The open
    // form's `menu("menu", ...)` child is `Anchor::Node { id: "trigger", .. }`,
    // which names a bare child key, not a full canonical path — a
    // constructor cannot know its own mount point, so the popover cannot
    // be placed correctly under any parent (a known limit; see
    // `.agents/notes/proposed/architecture/
    // 2026-09-03-anchored-components-cannot-name-their-own-anchor.md`).

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

    /// Check C/D: the closed trigger places with a real rect, none of its
    /// parts outside it.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
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

    /// Check F: an enabled trigger is reachable; a disabled one is not.
    #[test]
    fn the_closed_trigger_is_reachable_unless_disabled() {
        let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let trigger = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the trigger is placed");
        assert!(
            order.iter().any(|o| o == &trigger.id),
            "the trigger declares Focus but is not in focus order"
        );

        let disabled_node = disabled(menu_button(
            "more",
            "More",
            false,
            vec![menu_item("rename", "Rename")],
        ));
        let disabled_frame = petrify_lone(disabled_node);
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_trigger = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the disabled trigger is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_trigger.id),
            "a disabled trigger must not be reachable"
        );
    }

    /// Check E: the trigger's label and caret against the trigger's own
    /// resting fill, in both themes.
    #[test]
    fn trigger_text_clears_aa_contrast_against_its_own_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
            let trigger = child(&node, "trigger");
            let bg_name = trigger
                .props
                .tokens
                .get("background")
                .expect("trigger binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            for key in ["label", "caret"] {
                let part = child(trigger, key);
                let fg_name = part
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{key} binds a foreground"));
                let opacity = part.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "trigger/{key} at {ratio:.2}:1 against {} fails AA \
                     {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
