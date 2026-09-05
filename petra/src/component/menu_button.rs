//! Carbon Menu buttons (slice-c). A labelled button that opens a Menu.
//!
//! Anatomy (`_menu-button.scss`, a thin composition over `button` +
//! `menu`): trigger button + caret + Menu. Combo button and Overflow menu
//! are omitted (split trigger / icon-only).
//!
//! The trigger is Carbon's **primary** button (`kind="primary"` is the
//! default): [`ACCENT_PRIMARY`] fill, [`TEXT_ON_ACCENT`] label at the
//! leading edge, the chevron at the trailing edge, `min-inline-size` 160 —
//! the same 160 as the menu's own minimum, which is why Carbon's reference
//! shot has the two exactly one width. Square corners, as every Carbon
//! button is. Height md 40. No hover fill, for the reason
//! [`super::primary_button`] has none: the vocabulary has no
//! `accent.primary-hover`, and inventing the tone here would put a colour
//! decision in a component.
//!
//! The caret is [`IconMark::ChevronDown`] shut and [`IconMark::ChevronUp`]
//! open (Carbon turns `.cds--menu-button__trigger--open svg` 180°), in
//! [`IconTone::OnAccent`]. Never the only channel: `Semantics.expanded` is
//! declared and the menu is mounted only while open (FR-026).
//!
//! The menu hangs flush under the trigger at its leading edge and is at
//! least the trigger's width ([`super::list_box`]'s `Fit::Anchor`). Until
//! 2026-09-04 it was a centred, padded popover with a beak, and the
//! operator could not tell the page from the Popover page.

use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::edge_row;
use super::menu::{MIN_INLINE, menu};
use super::stack;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, ICON_ON_COLOR_DISABLED, SIZE_MD, TEXT_ON_ACCENT, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::Axis;
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, Interaction, Key, Role, TextWrap, ViewNode,
};

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MIN_INLINE == 160.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled menu trigger. When `open` is true the node also carries a
/// [`menu`] list box of `items`. `label` is required (FR-058).
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
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    caption.props.wrap = Some(TextWrap::Clip);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_ON_ACCENT));
    caption
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_ON_COLOR_DISABLED));
    let chevron = icon_toned(
        "caret",
        if open {
            IconMark::ChevronUp
        } else {
            IconMark::ChevronDown
        },
        IconTone::OnAccent,
    );
    // Label at the leading edge, chevron at the trailing one, whatever the
    // trigger's width (`edge_row`).
    let mut node = edge_row(key, caption, Some(chevron));
    node.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    let mut node = node
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(MIN_INLINE),
                max: None,
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(SIZE_MD),
                max: Some(SIZE_MD),
                priority: 0,
            },
        })
        .interactive(Role::Button, label, TRIGGER_INTENTS)
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.expanded = Some(open);
    node
}

#[cfg(test)]
mod tests {
    use super::{IconMark, IconTone, MIN_INLINE, SIZE_MD, icon_toned, menu_button};
    use crate::component::disabled;
    use crate::component::menu::menu_item;
    use crate::component::tokens::{ACCENT_PRIMARY, TEXT_ON_ACCENT};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, inks, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, Interaction, Justify, NodeKind, Props, Registry, Role, Tip, ViewNode,
    };

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
        assert_eq!(node.children.len(), 1, "closed trigger has no list box");
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("More"));
        assert_eq!(trigger.semantics.expanded, Some(false));
        assert_eq!(trigger.constraints.vertical.min, Some(SIZE_MD));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert_eq!(child(trigger, "label").props.text.as_deref(), Some("More"));
        let caret = child(trigger, "caret");
        assert_eq!(caret.kind, NodeKind::Canvas);
        assert_eq!(caret.props.text, None, "the caret is a glyph, not a word");
        assert_eq!(
            caret.props.canvas,
            icon_toned("caret", IconMark::ChevronDown, IconTone::OnAccent)
                .props
                .canvas,
            "a closed trigger points its caret down"
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    /// The trigger is Carbon's primary button: accent fill, on-accent
    /// label and caret, at least 160 wide, square. Falsify by handing the
    /// trigger `SURFACE_RAISED` and a radius again.
    #[test]
    fn the_trigger_is_a_primary_button_at_least_160_wide() {
        let node = menu_button("more", "More", false, items());
        let trigger = child(&node, "trigger");
        assert_eq!(
            trigger.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(
            child(trigger, "label")
                .props
                .tokens
                .get("foreground")
                .map(|t| t.as_str()),
            Some(TEXT_ON_ACCENT)
        );
        assert_eq!(trigger.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(MIN_INLINE, 160.0);
        assert!(
            !trigger.props.tokens.contains_key("radius"),
            "Carbon buttons are square"
        );
        assert_eq!(
            trigger.props.justify,
            Some(Justify::SpaceBetween),
            "an `edge_row`: the slack goes between label and caret, so the \
             caret sits at the trailing edge"
        );
    }

    #[test]
    fn menu_button_open_includes_a_flush_menu() {
        let node = menu_button("more", "More", true, items());
        assert_eq!(node.semantics.expanded, Some(true));
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.expanded, Some(true));
        assert_eq!(
            child(trigger, "caret").props.canvas,
            icon_toned("caret", IconMark::ChevronUp, IconTone::OnAccent)
                .props
                .canvas,
            "an open trigger points its caret up"
        );

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("More"));
        assert_eq!(menu.props.tip, Some(Tip::Flush), "a menu has no beak");
        match &menu.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(menu, "content");
        assert!(content.children.iter().all(|c| c.key.as_str() != "caret"));
        assert_eq!(
            child(content, "rename").semantics.label.as_deref(),
            Some("Rename")
        );
    }

    /// The open form's `menu` names its `trigger` sibling by bare key
    /// (`Anchor::Sibling`), so it is accepted wherever a caller mounts it —
    /// here two containers below the root, the gallery catalog's own depth.
    #[test]
    fn menu_button_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "menu_button open",
            vec![menu_button("more", "More", true, items())],
        );
    }

    // The frame-level checks below audit the CLOSED trigger.

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
    /// parts outside it, and the caret ends where the trigger's inline
    /// padding begins.
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
        let trigger = frame.placement("/root/more/trigger").expect("trigger");
        let caret = frame.placement("/root/more/trigger/caret").expect("caret");
        assert!(
            (trigger.rect.w - MIN_INLINE).abs() < 0.5,
            "a short label leaves the trigger at its 160 minimum: {:?}",
            trigger.rect
        );
        assert!(
            ((trigger.rect.x + trigger.rect.w - 16.0) - (caret.rect.x + caret.rect.w)).abs() < 0.5,
            "the caret's trailing edge is 16 in from the trigger's: trigger {:?}, caret {:?}",
            trigger.rect,
            caret.rect
        );
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
    /// resting fill — the accent — in both themes.
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
                let inks = inks(part);
                assert!(!inks.is_empty(), "trigger/{key} binds an ink");
                let opacity = part.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
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
}
