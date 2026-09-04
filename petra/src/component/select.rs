//! Carbon Select (slice-e). Closed state only.
//!
//! Anatomy of the closed field (`_select.scss`):
//! 1. Field — [`SURFACE_RAISED`] + [`BORDER_SUBTLE`], height md 40.
//! 2. Current value (visible text).
//! 3. Chevron as the word `"closed"` — never an icon-only mark (FR-026).
//!
//! The open menu is Wave 3 Popover. This module does not mount an overlay,
//! a listbox, or option rows. `Role::Button` is the closed field: it is
//! what would open the menu. `select_sm` 32 / `select_lg` 48 follow the
//! shared layout scale.
//!
//! The field label is the accessible name. Carbon's label-above anatomy
//! would make the control taller than 40; it is not stacked here.

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SHAPE_SM, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon Default sm.
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);

const CLOSED_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed select at Carbon md (40). `label` is the accessible name;
/// `value` is the visible current option.
pub fn select(key: impl Into<Key>, label: impl Into<String>, value: impl Into<String>) -> ViewNode {
    select_sized(key, label, value, SIZE_MD)
}

/// Carbon sm (32).
pub fn select_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    select_sized(key, label, value, SIZE_SM)
}

/// Carbon lg (48).
pub fn select_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    select_sized(key, label, value, SIZE_LG)
}

fn select_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    height: f32,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron = text("chevron", "closed");
    chevron
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, chevron],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(height))
        .interactive(Role::Button, label, CLOSED_INTENTS)
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
    use super::{SIZE_LG, SIZE_MD, SIZE_SM, select, select_lg, select_sm};
    use crate::component::tokens::{BORDER_SUBTLE, SURFACE_RAISED};
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

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn descendant_roles(node: &ViewNode) -> Vec<Role> {
        let mut roles = Vec::new();
        if let Some(role) = node.semantics.role.clone() {
            roles.push(role);
        }
        for child in &node.children {
            roles.extend(descendant_roles(child));
        }
        roles
    }

    #[test]
    fn select_is_a_closed_button_at_height_40() {
        let node = select("theme", "Theme", "Dark");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(child(&node, "value").props.text.as_deref(), Some("Dark"));
        assert_eq!(
            child(&node, "chevron").props.text.as_deref(),
            Some("closed")
        );
        assert_eq!(
            child(&node, "chevron").semantics.role,
            None,
            "chevron is a second channel, not its own control"
        );
    }

    #[test]
    fn select_does_not_fake_an_open_menu() {
        let node = select("theme", "Theme", "Dark");
        assert_eq!(node.semantics.expanded, None);
        assert!(
            !descendant_roles(&node)
                .iter()
                .any(|role| { matches!(role, Role::List | Role::Overlay | Role::Dialog) })
        );
        assert_eq!(
            node.children.len(),
            2,
            "closed field is value + chevron only"
        );
    }

    #[test]
    fn select_sm_is_32_and_lg_is_48() {
        let sm = select_sm("theme", "Theme", "Dark");
        assert_eq!(sm.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(sm.constraints.vertical.max, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(sm.semantics.role, Some(Role::Button));
        let lg = select_lg("theme", "Theme", "Dark");
        assert_eq!(lg.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(lg.constraints.vertical.max, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(lg.semantics.role, Some(Role::Button));
        assert_eq!(child(&lg, "chevron").props.text.as_deref(), Some("closed"));
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

    fn check_geometry(frame: &PetrifiedFrame, label: &str) {
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

    /// Check C/D: sm, md, lg and the disabled form all place with real
    /// rects, none of them outside their parent, and the chevron word
    /// never overflows its own rect (Class 4's shape: had the chevron
    /// been an icon-only hit box pinned to a glyph's width, this is what
    /// would have caught it — `select`'s own module doc records that the
    /// chevron is deliberately the word "closed" rather than an icon for
    /// exactly this reason, FR-026).
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        check_geometry(&petrify_lone(select("theme", "Theme", "Dark")), "md");
        check_geometry(&petrify_lone(select_sm("theme", "Theme", "Dark")), "sm");
        check_geometry(&petrify_lone(select_lg("theme", "Theme", "Dark")), "lg");
        check_geometry(
            &petrify_lone(crate::component::disabled(select("theme", "Theme", "Dark"))),
            "disabled",
        );
    }

    /// Check F: the closed field declares `Focus` and is reachable; the
    /// disabled form is not.
    #[test]
    fn field_focus_reachability_matches_disabled_state() {
        for (label, node, should_be_focusable) in [
            ("enabled", select("theme", "Theme", "Dark"), true),
            (
                "disabled",
                crate::component::disabled(select("theme", "Theme", "Dark")),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let root_placement = frame
                .placements
                .iter()
                .find(|p| p.id == "/root/theme")
                .expect("the field is placed");
            let reachable = focus.order().iter().any(|id| id == &root_placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the value text and the chevron word against the field's
    /// own resting fill, in both themes.
    #[test]
    fn field_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = select("theme", "Theme", "Dark");
            let field_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the closed field binds a resting background");
            let field_bg = color(&theme, field_bg_name.as_str());
            for key in ["value", "chevron"] {
                let child_node = child(&node, key);
                let fg_name = child_node
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{key} binds a foreground"));
                let opacity = child_node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str())
                    .faded(opacity)
                    .over(field_bg);
                let ratio = fg.contrast_ratio(field_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
