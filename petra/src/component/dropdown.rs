//! Carbon Dropdown (slice-b). A labelled field plus an open list box.
//!
//! Anatomy (`_dropdown.scss` + `_list-box.scss`):
//! 1. Label — `.cds--label`, `label-01` in `$text-secondary`, 8 above the
//!    field.
//! 2. Field — `.cds--list-box__field`: [`super::list_box::list_box_field`],
//!    `$field` under a one-unit `$border-strong` rule, square, value at
//!    `padding-left: 16px`, height md 40. The same node Select's field is.
//! 3. Chevron — [`IconMark::ChevronDown`] shut, [`IconMark::ChevronUp`]
//!    open (Carbon turns `.cds--list-box__menu-icon--open` 180°), in
//!    [`IconTone::Primary`] (`fill: $icon-primary`), 16 in from the
//!    trailing edge. Never the only channel: `Semantics.expanded` is
//!    declared on the field and the list is mounted only while open
//!    (FR-026).
//! 4. Open menu — [`super::list_box::list_box`], flush under the field and
//!    the field's width, a hairline between rows, **no caret**.
//! 5. Option — [`dropdown_option`]: [`Role::Button`] + `Semantics.selected`,
//!    height md 40, label at the leading edge, and when selected Carbon's
//!    `Checkmark` glyph at the trailing edge plus [`LAYER_SELECTED`] — a
//!    glyph and a fill, never a hue alone. Until 2026-09-04 the glyph was
//!    the literal word `selected` printed beside the label.
//!
//! [`dropdown`] is the closed column, [`dropdown_open`] the same column with
//! the list. The field is keyed `"field"` in both, so keyboard focus seated
//! on it survives the open. Combo box and Multiselect are omitted (clear
//! icon + tags).

use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::{Dividers, edge_row, list_box, list_box_field};
use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SIZE_MD, SPACING_03, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, TYPOGRAPHY_LABEL, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, TextWrap, ViewNode};

const _: () = assert!(SIZE_MD == 40.0);

const OPTION_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed dropdown at Carbon md (40). `label` is the visible label above
/// the field and the field's accessible name; `value` is the visible
/// current option.
pub fn dropdown(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    labelled(key, label, value, None)
}

/// Open dropdown: the labelled field plus a list box of `options`.
///
/// The field child is keyed `"field"` in both forms; the list box is keyed
/// `"menu"` and anchored to `"field"` by sibling key, so the pair is
/// accepted wherever a caller mounts it.
pub fn dropdown_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    options: Vec<ViewNode>,
) -> ViewNode {
    labelled(key, label, value, Some(options))
}

/// The column: label above field, plus the list box when `options` is
/// `Some`. One builder for both forms so their ids cannot drift apart.
fn labelled(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    options: Option<Vec<ViewNode>>,
) -> ViewNode {
    let label = label.into();
    let open = options.is_some();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_LABEL));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let chevron = if open {
        IconMark::ChevronUp
    } else {
        IconMark::ChevronDown
    };
    let field = list_box_field("field", label.clone(), value, SIZE_MD, chevron, open);
    let mut children = vec![caption, field];
    if let Some(options) = options {
        children.push(list_box("menu", label, "field", options, Dividers::Between));
    }
    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), children);
    node.props.align = Some(Align::Stretch);
    node.semantics.expanded = Some(open);
    node
}

/// The `field` child of a column built by [`labelled`] — this module's and
/// [`super::select`]'s alike — for tests that audit the field on its own.
///
/// # Panics
/// If `node` has no child keyed `field`, which no column here lacks.
#[cfg(test)]
pub(crate) fn open_field_of(node: &ViewNode) -> &ViewNode {
    node.children
        .iter()
        .find(|c| c.key.as_str() == "field")
        .map(|c| c.as_ref())
        .expect("a labelled column carries its field")
}

/// One option row. `selected` is a declared fact plus Carbon's `Checkmark`
/// glyph at the trailing edge and [`LAYER_SELECTED`] — never colour alone.
///
/// An [`edge_row`]: the text sits at the leading edge and the glyph at the
/// trailing one; one line, ellipsised, as
/// `.cds--list-box__menu-item__option` is.
pub fn dropdown_option(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    caption.props.wrap = Some(TextWrap::Ellipsis);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mark = selected.then(|| icon_toned("mark", IconMark::Checkmark, IconTone::Primary));
    let mut node = edge_row(key, caption, mark);
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
    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, OPTION_INTENTS);
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
        IconMark, IconTone, SIZE_MD, dropdown, dropdown_open, dropdown_option, icon_toned,
        open_field_of,
    };
    use crate::component::tokens::{
        BORDER_STRONG, LAYER_SELECTED, SURFACE_RAISED, TEXT_MUTED, TYPOGRAPHY_LABEL,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, inks, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, FocusFigure, Interaction, NodeKind, Props, Registry, Role, Tip, ViewNode,
    };

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
    fn dropdown_is_a_label_over_a_closed_field_at_height_40() {
        let node = dropdown("theme", "Theme", "Dark");
        assert_eq!(node.semantics.role, None, "the column is not the control");
        let label = child(&node, "label");
        assert_eq!(label.props.text.as_deref(), Some("Theme"));
        assert_eq!(
            label.props.style.as_ref().map(|t| t.as_str()),
            Some(TYPOGRAPHY_LABEL)
        );
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let field = open_field_of(&node);
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(
            field.semantics.focus_figure,
            FocusFigure::Hug,
            "the field is a well: focus hugs its sides, as a text input's does"
        );
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(field.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(field.interactions.contains(&Interaction::Click));
        assert!(field.interactions.contains(&Interaction::Focus));
        assert_eq!(token(field, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(field, "border"),
            None,
            "no box: the rule under the field is its whole boundary"
        );
        assert_eq!(
            token(child(field, "rule"), "background"),
            Some(BORDER_STRONG)
        );
        let row = child(field, "row");
        assert_eq!(child(row, "value").props.text.as_deref(), Some("Dark"));
        let chevron = child(row, "chevron");
        assert_eq!(chevron.kind, NodeKind::Canvas);
        assert_eq!(
            chevron.props.text, None,
            "the chevron is a glyph, not a word"
        );
        assert_eq!(
            chevron.props.canvas,
            icon_toned("chevron", IconMark::ChevronDown, IconTone::Primary)
                .props
                .canvas,
            "a closed field points its chevron down"
        );
        assert!(chevron.semantics.role.is_none());
        assert_eq!(field.semantics.expanded, Some(false));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
    }

    #[test]
    fn dropdown_open_hosts_options_in_a_flush_list_box() {
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
        assert_eq!(
            field.semantics.focus_figure,
            FocusFigure::Hug,
            "open, the field still hugs: an underline would cross the list"
        );
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.semantics.expanded, Some(true));
        assert_eq!(
            child(child(field, "row"), "chevron").props.canvas,
            icon_toned("chevron", IconMark::ChevronUp, IconTone::Primary)
                .props
                .canvas,
            "an open field points its chevron up"
        );

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(menu.props.tip, Some(Tip::Flush), "a list box has no beak");
        match &menu.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "field"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(menu, "content");
        assert!(content.children.iter().all(|c| c.key.as_str() != "caret"));
        let dark = child(content, "dark");
        assert_eq!(dark.semantics.role, Some(Role::Button));
        assert!(dark.semantics.selected);
        let mark = child(dark, "mark");
        assert_eq!(
            mark.kind,
            NodeKind::Canvas,
            "a glyph, not the word `selected`"
        );
        assert_eq!(mark.props.text, None);
        assert_eq!(
            mark.props.canvas,
            icon_toned("mark", IconMark::Checkmark, IconTone::Primary)
                .props
                .canvas
        );
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
        assert_eq!(
            on.semantics.focus_figure,
            FocusFigure::Underline,
            "an option row is a button, not a well: it keeps the underline"
        );
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

    /// Check C/D: the closed column and both option states place with a
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
            .find(|p| p.id.ends_with("/theme/field"))
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
    /// (and check mark) against their own resting fill, in both themes.
    #[test]
    fn text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let column = dropdown("theme", "Theme", "Dark");
            let field = open_field_of(&column);
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
                let label = child(child(field, "row"), label_key);
                let inks = inks(label);
                assert!(!inks.is_empty(), "field {label_key} binds an ink");
                let opacity = label.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
                    let fg = color(&theme, fg_name.as_str())
                        .faded(opacity)
                        .over(field_bg);
                    let ratio = fg.contrast_ratio(field_bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "field {label_key} at {ratio:.2}:1 against {} fails AA \
                         {MIN_TEXT_CONTRAST}:1",
                        field.props.tokens.get("background").unwrap().as_str()
                    );
                }
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
