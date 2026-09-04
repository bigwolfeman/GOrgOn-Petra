//! Carbon Dropdown (slice-b). Closed field plus an open list on Popover.
//!
//! Anatomy (`_dropdown.scss` + `_list-box.scss`):
//! 1. Field — [`SURFACE_RAISED`] + [`BORDER_SUBTLE`], height md 40.
//! 2. Current value (visible text).
//! 3. Chevron as the word `"closed"` / `"open"` — never an icon-only mark
//!    (FR-026).
//! 4. Open menu — [`super::popover::popover_with`] listing option rows.
//! 5. Option — [`Role::Button`] + `Semantics.selected`. Selected is also
//!    the word `"selected"` and [`LAYER_SELECTED`], never a hue alone.
//!
//! [`dropdown`] is the closed field (like [`super::select`]). [`dropdown_open`]
//! wraps that field and a popover of caller-supplied option nodes. Combo box
//! and Multiselect are omitted (clear icon + tags).

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_SM, SIZE_MD,
    SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const FIELD_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed dropdown at Carbon md (40). `label` is the accessible name;
/// `value` is the visible current option.
pub fn dropdown(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    closed_field(key, label, value, "closed", None)
}

/// Open dropdown: the closed field plus a popover listing `options`.
///
/// The field child is keyed `"field"`; the popover is keyed `"menu"` and
/// anchored to `"field"`. Callers that place this node under a parent must
/// keep that child key so [`Anchor::Node`](crate::tree::Anchor) can name it.
pub fn dropdown_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    options: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field("field", label.clone(), value, "open", Some(true));
    let menu = popover_with("menu", label, "field", options);
    let mut node = stack(key, Axis::Vertical, None, vec![field, menu]);
    node.semantics.expanded = Some(true);
    node
}

/// One option row. `selected` is a declared fact plus the word `"selected"`
/// and [`LAYER_SELECTED`] — never colour alone.
pub fn dropdown_option(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut parts = vec![caption];
    if selected {
        let mut mark = text("mark", "selected");
        mark.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
        parts.push(mark);
    }
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), parts);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            LAYER_SELECTED
        } else {
            SURFACE_RAISED
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
    let mut node = node.interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.selected = selected;
    node
}

fn closed_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    chevron: &'static str,
    expanded: Option<bool>,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut chevron_node = text("chevron", chevron);
    chevron_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, chevron_node],
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
    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.expanded = expanded;
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
    use super::{SIZE_MD, dropdown, dropdown_open, dropdown_option};
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED, SURFACE_RAISED};
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

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn dropdown_is_a_closed_button_at_height_40() {
        let node = dropdown("theme", "Theme", "Dark");
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
        assert!(child(&node, "chevron").semantics.role.is_none());
        assert_eq!(node.semantics.expanded, None);
        assert_ne!(node.semantics.role, Some(Role::Overlay));
    }

    #[test]
    fn dropdown_open_hosts_options_in_a_popover() {
        let node = dropdown_open(
            "theme",
            "Theme",
            "Dark",
            vec![
                dropdown_option("dark", "Dark", true),
                dropdown_option("light", "Light", false),
            ],
        );
        assert_eq!(node.semantics.expanded, Some(true));
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.semantics.expanded, Some(true));
        assert_eq!(child(field, "chevron").props.text.as_deref(), Some("open"));

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("Theme"));
        match &menu.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "field"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(menu, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        let dark = child(content, "dark");
        assert_eq!(dark.semantics.role, Some(Role::Button));
        assert!(dark.semantics.selected);
        assert_eq!(child(dark, "mark").props.text.as_deref(), Some("selected"));
        assert_eq!(token(dark, "background"), Some(LAYER_SELECTED));
        let light = child(content, "light");
        assert_eq!(light.semantics.role, Some(Role::Button));
        assert!(!light.semantics.selected);
        assert!(light.children.iter().all(|c| c.key.as_str() != "mark"));
    }

    #[test]
    fn dropdown_option_sets_role_label_and_selected() {
        let on = dropdown_option("dark", "Dark", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("Dark"));
        assert!(on.semantics.selected);
        assert!(on.interactions.contains(&Interaction::Click));
        let off = dropdown_option("light", "Light", false);
        assert!(!off.semantics.selected);
        assert_eq!(off.semantics.label.as_deref(), Some("Light"));
    }

    /// `dropdown_open`'s `menu` names its `field` sibling by bare key
    /// (`Anchor::Sibling`), so the open form is accepted wherever a caller
    /// mounts it — here two containers below the root, the gallery
    /// catalog's own depth. (`component/tests.rs` holds the same case as
    /// the acceptance test for `Anchor::Sibling` itself.)
    #[test]
    fn dropdown_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "dropdown_open",
            vec![dropdown_open(
                "theme",
                "Theme",
                "Dark",
                vec![
                    dropdown_option("dark", "Dark", true),
                    dropdown_option("light", "Light", false),
                ],
            )],
        );
    }

    // The frame-level checks below audit the CLOSED field and standalone
    // `dropdown_option` rows. `dropdown_option` is not anchored — it is a
    // plain interactive row a caller places inside the popover — so it is
    // audited on its own.

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

    /// Check C/D: the closed field and both option states place with a
    /// real rect, none of them outside their own row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("closed", dropdown("theme", "Theme", "Dark")),
            ("option-selected", dropdown_option("dark", "Dark", true)),
            ("option-plain", dropdown_option("light", "Light", false)),
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

    /// Check F: the closed field and an enabled option are reachable; a
    /// disabled option is not.
    #[test]
    fn field_and_enabled_options_are_reachable_and_disabled_ones_are_not() {
        let field_frame = petrify_lone(dropdown("theme", "Theme", "Dark"));
        let focus = crate::focus::FocusTree::from_placements(
            &field_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let field = field_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/theme"))
            .expect("the field is placed");
        assert!(
            focus.order().iter().any(|o| o == &field.id),
            "the closed field declares Focus but is not in focus order"
        );

        let opt_frame = petrify_lone(crate::component::disabled(dropdown_option(
            "dark", "Dark", true,
        )));
        let opt_focus = crate::focus::FocusTree::from_placements(
            &opt_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let option = opt_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/dark"))
            .expect("the option is placed");
        assert!(
            !opt_focus.order().iter().any(|o| o == &option.id),
            "a disabled option must not be reachable"
        );
    }

    /// Check E: the field's value/chevron and both option states' label
    /// (and "selected" mark) against their own resting fill, in both
    /// themes.
    #[test]
    fn text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let field = dropdown("theme", "Theme", "Dark");
            let field_bg = color(
                &theme,
                field
                    .props
                    .tokens
                    .get("background")
                    .expect("the field binds a resting background")
                    .as_str(),
            );
            for label_key in ["value", "chevron"] {
                let label = child(&field, label_key);
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label text binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str())
                    .faded(opacity)
                    .over(field_bg);
                let ratio = fg.contrast_ratio(field_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "field {label_key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    field.props.tokens.get("background").unwrap().as_str()
                );
            }

            for selected in [true, false] {
                let option = dropdown_option("opt", "Dark", selected);
                let bg_name = option
                    .props
                    .tokens
                    .get("background")
                    .expect("option binds a resting background");
                let bg = color(&theme, bg_name.as_str());
                fn walk_text(node: &ViewNode, bg: ColorValue, theme: &Theme, min: f32) {
                    if node.props.text.is_some()
                        && let Some(fg_name) = node.props.tokens.get("foreground")
                    {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        let fg = color(theme, fg_name.as_str()).faded(opacity).over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= min,
                            "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                            node.key,
                            fg_name.as_str()
                        );
                    }
                    for child in &node.children {
                        walk_text(child, bg, theme, min);
                    }
                }
                walk_text(&option, bg, &theme, MIN_TEXT_CONTRAST);
            }
        }
    }
}
