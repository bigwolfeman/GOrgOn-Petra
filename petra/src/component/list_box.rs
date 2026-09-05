//! Carbon's list box: the flush panel a dropdown, a select and a menu open.
//!
//! Not one of the 42 documented components. It is `_list-box.scss`, the
//! primitive `_dropdown.scss` builds on (slice-b), and `_menu.scss`'s
//! container is the same shape (slice-c): a panel on `$layer`, cast with
//! `0 2px 6px 0 rgba(0,0,0,.2)`, **no caret**, as wide as the field that
//! opened it, holding full-width rows. It is here because three components
//! were each building that panel on [`super::popover::popover_with`] and
//! each got the same three things wrong — a beak, a hugging width, and rows
//! centred in a padded box — which the operator read as "menu button does
//! the same thing as popover and isn't IBM style". A shared anatomy has to
//! have one home or it drifts three ways.
//!
//! What it is *not* is a popover. Carbon's popover points at the control
//! that opened it and holds arbitrary content; a list box butts against its
//! field and holds rows. The two differ in [`Tip`], [`Fit`], padding and
//! cross-axis alignment, and the differences are the component.
//!
//! Anatomy:
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Popup`], anchored to the
//!    sibling keyed `anchor` at its bottom-start corner ([`Align::Start`]:
//!    the panel's leading edge is the field's, never centred under it),
//!    [`Tip::Flush`], [`Fit::Anchor`], [`InputPolicy::DismissOutside`],
//!    [`ClampRule::Flip`] (Carbon opens upward when there is no room below,
//!    slice-b "Direction"). Fill [`SURFACE_RAISED`] (Carbon `$layer`),
//!    elevation [`SHADOW_OVERLAY`]. No padding, no border, no radius.
//! 2. Content — one vertical `Stack` keyed `content`, children stretched to
//!    the panel's width, no gap.
//! 3. Rows — the caller's ([`super::dropdown_option`], [`super::menu_item`]).
//!    Between consecutive rows, when `dividers` asks for it, a one-unit
//!    [`BORDER_SUBTLE`] rule inset [`SPACING_05`] at each end, which is
//!    `.cds--list-box__menu-item__option`'s `border-top` at its
//!    `margin: 0 16px`. A menu has no such rules (`_menu.scss` draws a
//!    divider only where an author places `menu-item-divider`).
//!
//! The **field** the panel hangs off is here too, [`list_box_field`], for
//! the same reason: `.cds--list-box__field` (dropdown) and
//! `.cds--select-input` (select) are one anatomy in Carbon — `$field`
//! fill, `border-block-end: 1px solid $border-strong`, value text at
//! `padding-left: 16px`, a 16-unit chevron at 16 from the trailing edge,
//! no box, no radius — and two copies of it had already drifted into two
//! rounded, outlined pills that hugged their own text.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_STRONG, BORDER_SUBTLE, LAYER_HOVER, SHADOW_OVERLAY, SPACING_05, SURFACE_RAISED,
    TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::{Align as CrossAlign, Axis};
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, Fit, FocusFigure, FocusShownOn,
    InputPolicy, InsetRefs, Interaction, Justify, Key, Layer, NodeKind, Props, Role, Semantics,
    TextWrap, Tip, ViewNode,
};

/// Height of the rule between two rows, and of the rule under a field.
/// Carbon `convert.to-rem(1px)`.
const DIVIDER_HEIGHT: f32 = 1.0;

const FIELD_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Whether the panel draws a rule between each pair of rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dividers {
    /// A hairline between rows: the list box of a dropdown or a select.
    Between,
    /// Rows butt against each other: a menu.
    None,
}

/// The flush panel, anchored under the sibling keyed `anchor`, holding
/// `rows` top to bottom. `label` is the accessible name of the overlay.
pub(crate) fn list_box(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor: impl Into<Key>,
    rows: Vec<ViewNode>,
    dividers: Dividers,
) -> ViewNode {
    let mut children = Vec::with_capacity(rows.len() * 2);
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 && dividers == Dividers::Between {
            children.push(divider(format!("div-{index}")));
        }
        children.push(row);
    }
    let mut content = stack("content", Axis::Vertical, None, children);
    content.props.align = Some(CrossAlign::Stretch);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: anchor.into(),
                edge: Edge::Bottom,
                align: Align::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            tip: Some(Tip::Flush),
            fit: Some(Fit::Anchor),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

/// A row with `leading` at its start and `trailing` at its end, whatever
/// the row's width: the shape of a list-box field (value, chevron), a
/// selected option (label, checkmark) and a menu button's trigger (label,
/// chevron).
///
/// A horizontal `Stack` under [`Justify::SpaceBetween`] — Carbon's own
/// `.cds--btn { justify-content: space-between }`. Three shapes were tried
/// and refused first. A priority-1 label in a stack: a text answers its
/// own width to any offer, so the label never grows past its glyphs and
/// the glyph beside it lands wherever the text ends. A two-track `Grid`: a
/// grid's one `align` places a cell's content on both axes, so centring
/// the glyph vertically also centred the text horizontally. A `Spacer`
/// between the two: it answers a natural-size probe with the whole offer
/// (`layout::leaf::spacer_extent`), so a menu button that should hug its
/// 160 grew to the page. `SpaceBetween` spends only the slack the row was
/// actually given, and a row given none hugs its content.
///
/// Inline padding [`SPACING_05`] both sides and the same gap between
/// children: with no slack the glyph sits 16 past the text, which is
/// Carbon's field reserving `padding-right: 48px` for a 16-unit chevron
/// sitting 16 from the edge with 16 clear before it.
pub(crate) fn edge_row(
    key: impl Into<Key>,
    leading: ViewNode,
    trailing: Option<ViewNode>,
) -> ViewNode {
    let mut children = vec![leading];
    children.extend(trailing);
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_05), children);
    node.props.align = Some(CrossAlign::Center);
    node.props.justify = Some(Justify::SpaceBetween);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    node
}

/// The closed field a list box opens from: Carbon's `.cds--list-box__field`
/// and `.cds--select-input`, which are one shape.
///
/// ```text
/// field   Stack ↓, `height` tall, $field fill, hover fill, Role::Button
/// ├ row   edge_row, inline padding 16, the height less the rule
/// │ ├ value    body-compact-01, one line, in the weighted track
/// │ └ chevron  16-unit glyph in $icon-primary, at the trailing edge
/// └ rule  1 unit, $border-strong, the field's only boundary
/// ```
///
/// The value is `TextWrap::Ellipsis` because Carbon's is (`.cds--list-box__label`:
/// `overflow: hidden; text-overflow: ellipsis; white-space: nowrap`).
/// `chevron` points down shut and up open
/// (Carbon turns `.cds--list-box__menu-icon--open` 180°); `expanded` is the
/// declared state beside it (FR-026).
///
/// The field is `Role::Button`: pressing it is what opens the list. Its
/// key is the caller's, and a caller that keeps that key the same in the
/// closed and open forms keeps keyboard focus on the field across the
/// open — the id is what focus is seated on.
///
/// It is a well, so it declares [`FocusFigure::Hug`]: keyboard focus is two
/// bars beside it, the way a text input's is, and never the underline a
/// button gets — which, with the list flush under the field, landed across
/// the first option row (rows 11 and 29, 2026-09-05).
pub(crate) fn list_box_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    height: f32,
    chevron: IconMark,
    expanded: bool,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    value_node.props.wrap = Some(TextWrap::Ellipsis);
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let chevron_node = icon_toned("chevron", chevron, IconTone::Primary);

    let mut row = edge_row("row", value_node, Some(chevron_node));
    // The row is the height the rule leaves. Pinned, not left to priority:
    // a stack answers its content's height to any offer, so a priority-1
    // row would sit 16 tall at the top of the field with the rule right
    // under it and the field's slack trailing below both.
    row.constraints.vertical = pinned(height - DIVIDER_HEIGHT);

    let mut rule = stack("rule", Axis::Horizontal, None, vec![]);
    rule.props
        .tokens
        .insert("background".into(), t(BORDER_STRONG));
    rule.constraints.vertical = pinned(DIVIDER_HEIGHT);

    let mut node = stack(key, Axis::Vertical, None, vec![row, rule]);
    node.props.align = Some(CrossAlign::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(Constraints {
            vertical: pinned(height),
            ..Constraints::default()
        })
        .interactive(Role::Button, label, FIELD_INTENTS);
    node.semantics.expanded = Some(expanded);
    node.semantics.focus_figure = FocusFigure::Sides;
    node.semantics.focus_shown_on = FocusShownOn::Well;
    node
}

/// The rule between two rows: a transparent one-unit strip inset
/// [`SPACING_05`] at each end, carrying the [`BORDER_SUBTLE`] line as its
/// child. Two nodes rather than one because a fill paints a node's whole
/// rect and padding insets only its children — the inset is what puts the
/// line's ends 16 units in from the panel's edges, where Carbon's are.
fn divider(key: impl Into<Key>) -> ViewNode {
    let mut line = stack("rule", Axis::Horizontal, None, vec![]);
    line.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    line.constraints.vertical = pinned(DIVIDER_HEIGHT);
    // The line takes the strip's whole padded interior across the strip.
    // A *vertical* strip, so that width comes from `Align::Stretch` on the
    // cross axis rather than from a main-axis leftover claim: an empty
    // stack measures zero on its main axis, and a priority-1 horizontal
    // claim inside a horizontal strip placed it at width 0 in a panel 80
    // wide — caught by pagination's own degenerate-rect check, which is
    // stricter than a tree assertion and is why it showed there first.
    // Same idiom as `ui_shell::accent_mark`, whose doc states the rule.
    let mut node = stack(key, Axis::Vertical, None, vec![line]);
    node.props.align = Some(CrossAlign::Stretch);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    node.constraints.vertical = pinned(DIVIDER_HEIGHT);
    node
}

fn pinned(h: f32) -> AxisConstraint {
    AxisConstraint {
        min: Some(h),
        max: Some(h),
        priority: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{DIVIDER_HEIGHT, Dividers, list_box, list_box_field};
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::menu::menu_item;
    use crate::component::tokens::{
        BORDER_STRONG, BORDER_SUBTLE, SHADOW_OVERLAY, SURFACE_RAISED, TYPOGRAPHY_BODY_COMPACT,
    };
    use crate::frame::{TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ThemeMode, standard_vocabulary};
    use crate::tree::{
        Align, Anchor, Edge, Fit, InputPolicy, Interaction, Justify, Layer, NodeKind, Props,
        Registry, Role, TextWrap, Tip, ViewNode,
    };

    fn child<'a>(node: &'a crate::tree::ViewNode, key: &str) -> &'a crate::tree::ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn rows() -> Vec<crate::tree::ViewNode> {
        vec![
            menu_item("rename", "Rename"),
            menu_item("delete", "Delete"),
            menu_item("share", "Share"),
        ]
    }

    /// The panel is the list-box shape and not the popover shape: flush,
    /// anchor-fitted, start-aligned, unpadded, unbordered, elevated.
    #[test]
    fn a_list_box_is_a_flush_anchor_fitted_panel_and_not_a_popover() {
        let node = list_box("menu", "Actions", "trigger", rows(), Dividers::None);
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(node.props.tip, Some(Tip::Flush), "a list box has no beak");
        assert_eq!(
            node.props.fit,
            Some(Fit::Anchor),
            "a list box is as wide as the field that opened it"
        );
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        match &node.props.anchor {
            Some(Anchor::Sibling {
                key, edge, align, ..
            }) => {
                assert_eq!(key.as_str(), "trigger");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(
                    *align,
                    Align::Start,
                    "the panel's leading edge is the field's, not centred under it"
                );
            }
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert!(node.props.padding.is_none(), "rows run edge to edge");
        assert!(
            !node.props.tokens.contains_key("border"),
            "Carbon's list box has no outline; the shadow separates it"
        );
        assert!(!node.props.tokens.contains_key("radius"));
        assert_eq!(
            node.props.tokens.get("background").map(|t| t.as_str()),
            Some(SURFACE_RAISED)
        );
        assert_eq!(
            node.props.tokens.get("shadow").map(|t| t.as_str()),
            Some(SHADOW_OVERLAY)
        );
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        let content = child(&node, "content");
        assert_eq!(content.props.align, Some(crate::geom::Align::Stretch));
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "no `^` word and no caret node of any kind"
        );
    }

    /// `Dividers::Between` puts one rule between each pair of rows and none
    /// before the first or after the last; `Dividers::None` puts none.
    #[test]
    fn dividers_sit_between_rows_only() {
        let divided = list_box("menu", "Theme", "field", rows(), Dividers::Between);
        let keys: Vec<&str> = child(&divided, "content")
            .children
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(keys, ["rename", "div-1", "delete", "div-2", "share"]);
        let rule = child(child(child(&divided, "content"), "div-1"), "rule");
        assert_eq!(rule.constraints.vertical.min, Some(DIVIDER_HEIGHT));
        assert_eq!(
            rule.props.tokens.get("background").map(|t| t.as_str()),
            Some(BORDER_SUBTLE)
        );

        let plain = list_box("menu", "Actions", "trigger", rows(), Dividers::None);
        let keys: Vec<&str> = child(&plain, "content")
            .children
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(keys, ["rename", "delete", "share"]);
    }

    /// Accepted beside a control carrying the anchor key, two containers
    /// below the root — the gallery catalog's own depth.
    #[test]
    fn a_list_box_validates_beside_its_anchor_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "list_box",
            vec![
                crate::component::button("trigger", "Actions"),
                list_box("menu", "Actions", "trigger", rows(), Dividers::Between),
            ],
        );
    }

    /// The field is Carbon's list-box field: a `$field` box whose only
    /// boundary is the one-unit `$border-strong` rule under it, square,
    /// value at the leading edge, chevron at the trailing edge.
    #[test]
    fn a_list_box_field_is_a_bottom_ruled_box_not_an_outlined_pill() {
        let node = list_box_field("field", "Theme", "Dark", 40.0, IconMark::ChevronDown, false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Theme"));
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert_eq!(node.constraints.vertical.min, Some(40.0));
        assert_eq!(node.constraints.vertical.max, Some(40.0));
        assert_eq!(
            node.props.tokens.get("background").map(|t| t.as_str()),
            Some(SURFACE_RAISED)
        );
        assert!(
            !node.props.tokens.contains_key("border"),
            "no box: the rule under the field is its whole boundary"
        );
        assert!(!node.props.tokens.contains_key("radius"), "square");
        let rule = child(&node, "rule");
        assert_eq!(
            rule.props.tokens.get("background").map(|t| t.as_str()),
            Some(BORDER_STRONG)
        );
        assert_eq!(rule.constraints.vertical.max, Some(DIVIDER_HEIGHT));
        let row = child(&node, "row");
        let value = child(row, "value");
        assert_eq!(value.props.text.as_deref(), Some("Dark"));
        assert_eq!(
            value.props.style.as_ref().map(|t| t.as_str()),
            Some(TYPOGRAPHY_BODY_COMPACT)
        );
        assert_eq!(value.props.wrap, Some(TextWrap::Ellipsis));
        assert_eq!(
            row.props.justify,
            Some(Justify::SpaceBetween),
            "an `edge_row`: the slack goes between value and chevron"
        );
        let chevron = child(row, "chevron");
        assert_eq!(chevron.kind, NodeKind::Canvas);
        assert_eq!(
            chevron.props.canvas,
            icon_toned("chevron", IconMark::ChevronDown, IconTone::Primary)
                .props
                .canvas
        );
        let open = list_box_field("field", "Theme", "Dark", 40.0, IconMark::ChevronUp, true);
        assert_eq!(open.semantics.expanded, Some(true));
    }

    /// Placed at a real width, the value sits 16 in from the leading edge,
    /// the chevron ends 16 in from the trailing edge, the rule is the
    /// field's bottom unit and spans its full width, and the whole thing
    /// is exactly `height` tall.
    #[test]
    fn a_placed_field_puts_the_chevron_at_the_trailing_edge_over_a_full_width_rule() {
        let field = list_box_field("field", "Theme", "Dark", 40.0, IconMark::ChevronDown, false);
        let mut column = ViewNode::new(NodeKind::Stack, "column")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                align: Some(crate::geom::Align::Stretch),
                ..Props::default()
            })
            .child(field);
        column.constraints.horizontal.min = Some(320.0);
        column.constraints.horizontal.max = Some(320.0);
        // The root is placed at the viewport whatever it declares, so the
        // 320 column sits inside a root that leaves it at its own width.
        let column = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(column);
        let registry = Registry::with_vocabulary(standard_vocabulary());
        let mut harness = Harness::new();
        let viewport = Viewport::new(Size { w: 900.0, h: 700.0 }, ThemeMode::Dark);
        harness.scale = viewport.scale;
        let frame = petrify(
            1,
            validated_with(&column, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        );
        let rect = |id: &str| frame.placement(id).unwrap_or_else(|| panic!("{id}")).rect;
        let field = rect("/root/column/field");
        let row = rect("/root/column/field/row");
        let value = rect("/root/column/field/row/value");
        let chevron = rect("/root/column/field/row/chevron");
        let rule = rect("/root/column/field/rule");
        assert!(
            ((chevron.y + chevron.h / 2.0) - (row.y + row.h / 2.0)).abs() < 0.5,
            "the chevron is centred in the row: chevron {chevron:?}, row {row:?}"
        );
        assert!(
            (field.w - 320.0).abs() < 0.5,
            "the field fills its column: {field:?}"
        );
        assert!((field.h - 40.0).abs() < 0.5, "{field:?}");
        assert!(
            (value.x - (field.x + 16.0)).abs() < 0.5,
            "value {value:?} in field {field:?}"
        );
        assert!(
            ((field.x + field.w - 16.0) - (chevron.x + chevron.w)).abs() < 0.5,
            "chevron {chevron:?} in field {field:?}"
        );
        assert!(
            (chevron.w - 16.0).abs() < 0.5,
            "a 16-unit glyph: {chevron:?}"
        );
        assert!((rule.h - DIVIDER_HEIGHT).abs() < 0.01, "{rule:?}");
        assert!(
            (rule.w - field.w).abs() < 0.5,
            "the rule spans the field: {rule:?}"
        );
        assert!(
            ((rule.y + rule.h) - (field.y + field.h)).abs() < 0.01,
            "the rule is the field's bottom unit: rule {rule:?}, field {field:?}"
        );
    }
}
