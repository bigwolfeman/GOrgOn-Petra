//! Carbon Select (slice-e). A labelled field plus an open list box.
//!
//! Anatomy (`_select.scss`, Default style):
//! 1. Label — `.cds--label`, `label-01` in `$text-secondary`, 8 above the
//!    field (`margin-bottom: $spacing-03`).
//! 2. Field — `.cds--select-input`: [`super::list_box::list_box_field`],
//!    which is `$field` under a one-unit `$border-strong` rule, square,
//!    value at `padding-left: 16px`, 16-unit chevron 16 in from the
//!    trailing edge, height md 40 (`select_sm` 32 / `select_lg` 48). It is
//!    the same node Dropdown's field is, because in Carbon they are the
//!    same field. Until 2026-09-04 this was a rounded, outlined pill that
//!    hugged the word `closed` — the row the operator called "not carbon
//!    style".
//! 3. Open list — [`super::list_box::list_box`] anchored flush under the
//!    field, the field's width, option rows built with
//!    [`super::dropdown::dropdown_option`] and a hairline between rows.
//!
//! Carbon's Select is the browser's native `<select>`, so Carbon draws no
//! open list of its own and the reference has no open shot; the open form
//! here is [`super::dropdown::dropdown_open`]'s, which is the nearest
//! ground truth Carbon offers (`ignored/carbon-ref/shots/11-dropdown-open.png`).
//!
//! The closed and open forms are one shape — a column keyed `key` holding
//! `label` and `field`, plus `menu` while open — so the field's id does not
//! change when the list opens and keyboard focus stays on it, which is
//! Carbon's behaviour and was not this component's.

use super::icon::IconMark;
use super::list_box::{Dividers, list_box, list_box_field};
use super::stack;
use super::text::text;
use super::tokens::{SIZE_MD, SPACING_03, TEXT_MUTED, TYPOGRAPHY_LABEL, t};
use crate::geom::{Align, Axis};
use crate::tree::{Key, ViewNode};

/// Carbon Default sm.
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);

/// Closed select at Carbon md (40). `label` is the visible label above the
/// field and the field's accessible name; `value` is the visible current
/// option.
pub fn select(key: impl Into<Key>, label: impl Into<String>, value: impl Into<String>) -> ViewNode {
    labelled(key, label, value, SIZE_MD, None)
}

/// Carbon sm (32).
pub fn select_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    labelled(key, label, value, SIZE_SM, None)
}

/// Carbon lg (48).
pub fn select_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    labelled(key, label, value, SIZE_LG, None)
}

/// Open select: the labelled md field plus a list box of `options`.
///
/// The field child is keyed `"field"` in both forms; the list box is keyed
/// `"menu"` and anchored to `"field"` by sibling key, so the pair is
/// accepted wherever a caller mounts it. Build `options` with
/// [`super::dropdown::dropdown_option`].
pub fn select_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    options: Vec<ViewNode>,
) -> ViewNode {
    labelled(key, label, value, SIZE_MD, Some(options))
}

/// The column: label above field, plus the list box when `options` is
/// `Some`. One builder for both forms so their ids cannot drift apart.
fn labelled(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    height: f32,
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
    let field = list_box_field("field", label.clone(), value, height, chevron, open);
    let mut children = vec![caption, field];
    if let Some(options) = options {
        children.push(list_box("menu", label, "field", options, Dividers::Between));
    }
    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), children);
    node.props.align = Some(Align::Stretch);
    node.semantics.expanded = Some(open);
    node
}

#[cfg(test)]
mod tests {
    use super::{SIZE_LG, SIZE_MD, SIZE_SM, select, select_lg, select_open, select_sm};
    use crate::component::dropdown::{dropdown_option, open_field_of as field_of};
    use crate::component::tokens::{BORDER_STRONG, SURFACE_RAISED, TEXT_MUTED, TYPOGRAPHY_LABEL};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, FocusFigure, FocusShownOn, Interaction, NodeKind, Props, Registry, Role, Tip,
        ViewNode,
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

    /// The closed form is a label over a bottom-ruled field, and the field
    /// is the button.
    #[test]
    fn select_is_a_label_over_a_closed_field_at_height_40() {
        let node = select("theme", "Theme", "Dark");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.semantics.role, None, "the column is not the control");
        assert_eq!(node.semantics.expanded, Some(false));
        let label = child(&node, "label");
        assert_eq!(label.props.text.as_deref(), Some("Theme"));
        assert_eq!(
            label.props.style.as_ref().map(|t| t.as_str()),
            Some(TYPOGRAPHY_LABEL)
        );
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let field = field_of(&node);
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(
            field.semantics.focus_figure,
            FocusFigure::Sides,
            "the field is a well: focus brackets its sides, as a text input's does"
        );
        assert_eq!(field.semantics.focus_shown_on, FocusShownOn::Well);
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.semantics.expanded, Some(false));
        assert_eq!(field.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(field.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(field.interactions.contains(&Interaction::Click));
        assert!(field.interactions.contains(&Interaction::Focus));
        assert_eq!(token(field, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(field, "border"),
            None,
            "no box around the field: Carbon's boundary is the rule under it"
        );
        assert_eq!(
            token(field, "radius"),
            None,
            "and no rounding on it either: `.cds--select-input` is a flat \
             fill, and a corner here would read as a box even without one"
        );
        assert_eq!(
            token(child(field, "rule"), "background"),
            Some(BORDER_STRONG)
        );
        let row = child(field, "row");
        assert_eq!(child(row, "value").props.text.as_deref(), Some("Dark"));
        let chevron = child(row, "chevron");
        assert_eq!(
            chevron.kind,
            NodeKind::Canvas,
            "a glyph, not the word `closed`"
        );
        assert_eq!(chevron.props.text, None);
        assert!(chevron.semantics.role.is_none());
    }

    #[test]
    fn select_does_not_fake_an_open_menu() {
        let node = select("theme", "Theme", "Dark");
        assert!(
            !descendant_roles(&node)
                .iter()
                .any(|role| { matches!(role, Role::List | Role::Overlay | Role::Dialog) })
        );
        assert_eq!(
            node.children.len(),
            2,
            "the closed form is label + field only"
        );
    }

    #[test]
    fn select_sm_is_32_and_lg_is_48() {
        let sm = select_sm("theme", "Theme", "Dark");
        assert_eq!(field_of(&sm).constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(field_of(&sm).constraints.vertical.max, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(field_of(&sm).semantics.role, Some(Role::Button));
        let lg = select_lg("theme", "Theme", "Dark");
        assert_eq!(field_of(&lg).constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(field_of(&lg).constraints.vertical.max, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(field_of(&lg).semantics.role, Some(Role::Button));
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
    /// rects, none of them outside their parent.
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

    /// Check F: the field declares `Focus` and is reachable; the disabled
    /// form is not. The column above it is never a stop.
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
            let reachable = focus.order().iter().any(|id| id == "/root/theme/field");
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
            assert!(
                !focus.order().iter().any(|id| id == "/root/theme"),
                "{label}: the column is not a stop"
            );
        }
    }

    /// Check E: the value text against the field's own resting fill, and
    /// the label against the page, in both themes.
    #[test]
    fn field_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = select("theme", "Theme", "Dark");
            let field = field_of(&node);
            let field_bg = color(&theme, token(field, "background").expect("field fill"));
            let value = child(child(field, "row"), "value");
            let fg_name = token(value, "foreground").expect("value binds a foreground");
            let opacity = value.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name).faded(opacity).over(field_bg);
            let ratio = fg.contrast_ratio(field_bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "value at {ratio:.2}:1 against {fg_name} fails AA {MIN_TEXT_CONTRAST}:1"
            );
            let page = color(&theme, crate::component::tokens::SURFACE_BASE);
            let label = child(&node, "label");
            let fg = color(&theme, token(label, "foreground").expect("label ink")).over(page);
            let ratio = fg.contrast_ratio(page);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "label at {ratio:.2}:1 against the page fails AA {MIN_TEXT_CONTRAST}:1"
            );
        }
    }

    fn open_theme() -> ViewNode {
        select_open(
            "theme",
            "Theme",
            "Dark",
            vec![
                dropdown_option("dark", "Dark", true),
                dropdown_option("light", "Light", false),
            ],
        )
    }

    /// The open form is the same column, its field expanded and pointing
    /// up, beside a flush list box of the caller's rows anchored to that
    /// field by its bare sibling key.
    #[test]
    fn select_open_hosts_options_in_a_flush_list_box() {
        let node = open_theme();
        assert_eq!(node.semantics.expanded, Some(true));
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(
            field.semantics.focus_figure,
            FocusFigure::Sides,
            "open, the field keeps its sides: a bar under would cross the list"
        );
        assert_eq!(field.semantics.focus_shown_on, FocusShownOn::Well);
        assert_eq!(field.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(field.semantics.expanded, Some(true));
        assert_eq!(field.constraints.vertical.min, Some(SIZE_MD));

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
        assert!(child(content, "dark").semantics.selected);
        assert!(!child(content, "light").semantics.selected);
    }

    /// The field's key is the same in both forms, so the id focus sits on
    /// survives the open. Falsify by keying the open field `"field-open"`.
    #[test]
    fn the_field_keeps_its_key_across_open_and_closed() {
        let closed = select("theme", "Theme", "Dark");
        let open = open_theme();
        assert_eq!(field_of(&closed).key, field_of(&open).key);
        assert_eq!(closed.key, open.key);
    }

    /// Accepted two containers below the root with the shipped `Registry`,
    /// the gallery catalog's own depth — the same acceptance
    /// `dropdown_open` has.
    #[test]
    fn select_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth("select_open", vec![open_theme()]);
    }
}
