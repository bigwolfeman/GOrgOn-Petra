//! Pressable toggle button and group (spec 009 T028, D-092).
//!
//! This is **not** Carbon's switch. The switch is [`super::toggle`] /
//! [`super::toggle_sm`] in `controls.rs` and this file does not touch it.
//! MynaUI's Toggle is a pressable button; the version they show is a
//! floating format palette (bold / italic) over the pointer. That palette
//! is a later compound (T028a, selection palette). These constructors are
//! the atomics it will host.
//!
//! # Anatomy
//!
//! 1. [`toggle_button`] — one labelled button. Pressed-or-not is
//!    app-declared, a `bool`, the same line [`super::selectable_tag`]
//!    draws. [`Role::Button`] plus `Semantics.selected`. Resting chrome
//!    is a filled secondary button ([`SURFACE_RAISED`], [`SHADOW_RAISED`],
//!    no outline). Pressed fills [`LAYER_SELECTED`], never a four-sided
//!    border — the painter cannot round a border, and a container edge is
//!    the thing spec 009's design language withdrew.
//! 2. [`toggle_button_group`] — a horizontal stack of those buttons.
//!    Single- vs multiple-select is a **caller convention**: this
//!    constructor does not inspect `pressed`, does not clear siblings, and
//!    does not grow a `single`/`multiple` argument. The caller passes
//!    `pressed` on each child, the same way [`super::radio_group`] leaves
//!    exclusivity to the `selected` bools it is given.
//!
//! # Why pressed also draws [`IconMark::Check`]
//!
//! [`LAYER_SELECTED`] is the named fill and it is not enough on its own.
//! A captured render of a selectable tag measured `layer-selected` only 2
//! of 255 sRGB steps off the resting `layer.raised` fill in the dark
//! theme — not merely hard for a colour-blind reader, indistinguishable
//! for any reader (`selectable_tag`'s own doc, 2026-09-04). FR-015 says
//! colour is never the only channel. [`IconMark::Check`] is the
//! vocabulary's existing "this is on" glyph (selectable tag, selectable
//! tile, checkbox), reused rather than invented. Unpressed draws no mark.
//!
//! # Icon + label
//!
//! [`toggle_button_icon`] takes a leading [`IconMark`] as identity (a
//! Bold button that always shows its B). The label is still required
//! (FR-026 / FR-058): there is no icon-only constructor that compiles.
//! Pressed still leads with Check; the identity icon does not replace it.
//!
//! Squared inner edges of a flush group wait on T002 (per-corner radius).
//! Until then the group gaps its children by [`SPACING_02`] so two
//! Grouping radii do not collide.

use super::icon::{IconMark, IconTone, icon_toned};
use super::text::text;
use super::tokens::{
    ICON_DISABLED, LAYER_ACTIVE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHADOW_RAISED,
    SIZE_MD, SPACING_02, SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY_COMPACT, t,
};
use super::{pin_block, stack};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Behaviour, FocusFigure, InsetRefs, Intent, Interaction, Key, Phase, Role, ViewNode,
};

const _: () = assert!(SIZE_MD == 40.0);

const INTERACTIVE: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// One boolean, flipped when the press completes. Same gesture as the
/// switch and the checkbox; a different picture.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

/// Inline pad is [`SPACING_05`], the same as a labelled button. Block is
/// none: height is the pinned constraint.
fn pad_inline() -> InsetRefs {
    InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    }
}

/// A pressable toggle button. `pressed` is app-declared; this function
/// does not keep it. `label` is required (FR-058). Role and interactions
/// are set inside the body — there is no `toggle_button(key)` that compiles.
#[must_use]
pub fn toggle_button(key: impl Into<Key>, label: impl Into<String>, pressed: bool) -> ViewNode {
    built(key, label, pressed, None)
}

/// [`toggle_button`] with a leading identity icon. The label is still
/// required: an icon-only toggle is unrepresentable (FR-026 / FR-058).
#[must_use]
pub fn toggle_button_icon(
    key: impl Into<Key>,
    label: impl Into<String>,
    pressed: bool,
    mark: IconMark,
) -> ViewNode {
    built(key, label, pressed, Some(mark))
}

/// A horizontal stack of toggle buttons.
///
/// Single-select (one child pressed) and multiple-select (any number
/// pressed) are caller conventions. This constructor does not read
/// `Semantics.selected` on its children, does not clear siblings, and
/// does not take a mode argument. Pass `pressed` on each
/// [`toggle_button`] the way [`super::radio_group`] passes `selected` on
/// each radio.
///
/// No role of its own: `Role` has no group entry, and a container with
/// no actions is outside `ActionableNeedsRoleAndLabel`. The buttons
/// carry chrome, role, and the pressed fill; this node is the row.
/// Each child's focus figure is re-seated from [`FocusFigure::Sides`]
/// to [`FocusFigure::BarUnder`] so the caret sits under the row, not
/// in the gap. A caller that already overrode the figure (the selection
/// palette's `BarInside`) is left alone.
#[must_use]
pub fn toggle_button_group(key: impl Into<Key>, mut children: Vec<ViewNode>) -> ViewNode {
    seat_bar_under(&mut children);
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_02), children);
    node.props.align = Some(Align::Center);
    // No radius token: a radius with no fill is a silent placement
    // (`petra-egui` paint). Squared inner edges wait on T002. Until
    // then the children keep Grouping on their own chrome and the
    // row only spaces them.
    node
}

fn seat_bar_under(children: &mut [ViewNode]) {
    for child in children {
        if child.semantics.focus_figure == FocusFigure::Sides {
            child.semantics.focus_figure = FocusFigure::BarUnder;
        }
    }
}

fn built(
    key: impl Into<Key>,
    label: impl Into<String>,
    pressed: bool,
    identity: Option<IconMark>,
) -> ViewNode {
    let label = label.into();
    let mut parts = Vec::new();
    if pressed {
        parts.push(icon_toned("mark", IconMark::Check, IconTone::Primary));
    }
    if let Some(mark) = identity {
        parts.push(icon_toned("icon", mark, IconTone::Primary));
    }
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    caption
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_DISABLED));
    parts.push(caption);

    let spacing = if parts.len() > 1 {
        Some(SPACING_03)
    } else {
        None
    };
    let mut node = stack(key, Axis::Horizontal, spacing, parts);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad_inline());
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    // Selected is app-declared (`Semantics.selected`); the painter
    // resolves `background@selected` from that flag. Resting fill stays
    // `SURFACE_RAISED` so a tree diff of pressed vs not is the selected
    // slots and the Check, not a forked chrome function.
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(CornerRole::Grouping, SIZE_MD)),
    );

    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label, INTERACTIVE)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text()
        .with_focus_figure(FocusFigure::Sides)
        .with_transition(crate::anim::BUTTON_PRESS);
    node.semantics.selected = pressed;
    node
}

#[cfg(test)]
mod tests {
    use super::{
        IconMark, IconTone, SIZE_MD, icon_toned, toggle_button, toggle_button_group,
        toggle_button_icon,
    };
    use crate::component::disabled;
    use crate::component::tokens::{
        LAYER_SELECTED, LAYER_SELECTED_HOVER, SHADOW_RAISED, SURFACE_RAISED,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, inks, validated_with};
    use crate::token::{
        ColorValue, CornerRole, Theme, ThemeMode, TokenName, TokenValue, corner_for,
        standard_vocabulary,
    };
    use crate::tree::{
        Behaviour, FocusFigure, Intent, Interaction, NodeKind, Phase, Props, Registry, Role,
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

    fn has_canvas(node: &ViewNode) -> bool {
        node.kind == crate::tree::NodeKind::Canvas
            || node.children.iter().any(|child| has_canvas(child))
    }

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    #[test]
    fn toggle_button_is_a_labelled_button_at_height_40() {
        let node = toggle_button("bold", "Bold", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Bold"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Hover));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(!node.semantics.selected);
        assert_eq!(
            node.behaviour,
            Some(Behaviour {
                intent: Intent::Toggle,
                phase: Phase::OnRelease,
            })
        );
        assert!(node.semantics.owns_its_text);
    }

    #[test]
    fn pressed_is_a_declared_bool_and_fills_layer_selected_not_a_border() {
        let on = toggle_button("bold", "Bold", true);
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            token(&on, "background@selected-hover"),
            Some(LAYER_SELECTED_HOVER)
        );
        assert_eq!(token(&on, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&on, "shadow"), Some(SHADOW_RAISED));
        assert_eq!(
            token(&on, "border"),
            None,
            "pressed is a fill step, not a four-sided border"
        );
        assert_eq!(
            token(&on, "radius"),
            Some(corner_for(CornerRole::Grouping, SIZE_MD))
        );

        let off = toggle_button("bold", "Bold", false);
        assert!(!off.semantics.selected);
        assert_eq!(token(&off, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(token(&off, "border"), None);
        assert!(off.is_interactive());
    }

    /// A5 (2026-09-04): `layer-selected` measured 2 of 255 sRGB steps off
    /// the resting `layer.raised` fill in the dark theme. Pressed must
    /// carry a canvas mark; unpressed must not.
    #[test]
    fn pressed_carries_a_check_and_unpressed_does_not() {
        let on = toggle_button("bold", "Bold", true);
        assert!(
            has_canvas(&on),
            "layer-selected measured 2 of 255 sRGB steps off the resting \
             fill; pressed must not be fill-tone alone"
        );
        let mark = child(&on, "mark");
        assert_eq!(mark.kind, crate::tree::NodeKind::Canvas);
        assert_eq!(
            mark.props.canvas,
            icon_toned("mark", IconMark::Check, IconTone::Primary)
                .props
                .canvas,
            "pressed draws the vocabulary's existing on-glyph"
        );

        let off = toggle_button("bold", "Bold", false);
        assert!(
            !has_canvas(&off),
            "the mark is the on-state glyph, not a permanent decoration"
        );
        assert_eq!(child(&off, "label").props.text.as_deref(), Some("Bold"));
    }

    #[test]
    fn icon_constructor_keeps_the_label_and_still_marks_pressed_with_check() {
        let off = toggle_button_icon("bold", "Bold", false, IconMark::WarningFilled);
        assert_eq!(off.semantics.role, Some(Role::Button));
        assert_eq!(off.semantics.label.as_deref(), Some("Bold"));
        assert!(!off.semantics.selected);
        assert_eq!(
            child(&off, "icon").props.canvas,
            icon_toned("icon", IconMark::WarningFilled, IconTone::Primary)
                .props
                .canvas
        );
        assert_eq!(child(&off, "label").props.text.as_deref(), Some("Bold"));

        let on = toggle_button_icon("bold", "Bold", true, IconMark::WarningFilled);
        assert!(on.semantics.selected);
        let keys: Vec<&str> = on.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["mark", "icon", "label"]);
        assert_eq!(
            child(&on, "mark").props.canvas,
            icon_toned("mark", IconMark::Check, IconTone::Primary)
                .props
                .canvas
        );
    }

    #[test]
    fn group_is_a_horizontal_stack_and_does_not_invent_exclusive_state() {
        let node = toggle_button_group(
            "format",
            vec![
                toggle_button("bold", "Bold", true),
                toggle_button("italic", "Italic", true),
                toggle_button("under", "Underline", false),
            ],
        );
        assert_eq!(node.key.as_str(), "format");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert!(node.semantics.role.is_none());
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(!node.semantics.selected);
        assert_eq!(
            token(&node, "radius"),
            None,
            "a radius with no fill is a silent placement; the row is a gap"
        );
        assert_eq!(
            token(&node, "border"),
            None,
            "the group is a row, not a bordered well"
        );

        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["bold", "italic", "under"]);
        assert!(named(&node, "bold").semantics.selected);
        assert!(named(&node, "italic").semantics.selected);
        assert!(!named(&node, "under").semantics.selected);
        assert_eq!(
            named(&node, "bold").semantics.focus_figure,
            FocusFigure::BarUnder,
            "a grouped toggle wears the under bar, not Sides in the gap"
        );
        let mut kept = toggle_button("keep", "Keep", false);
        kept.semantics.focus_figure = FocusFigure::BarInside;
        let grouped = toggle_button_group("keep-group", vec![kept]);
        assert_eq!(
            grouped.children[0].semantics.focus_figure,
            FocusFigure::BarInside,
            "a caller that already overrode the figure is left alone"
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        crate::anim::shipped_registry().declare_into(&mut registry);
        registry
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

    fn three() -> ViewNode {
        toggle_button_group(
            "format",
            vec![
                toggle_button("bold", "Bold", true),
                toggle_button("italic", "Italic", false),
                toggle_button_icon("warn", "Warn", false, IconMark::WarningFilled),
            ],
        )
    }

    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("off", toggle_button("bold", "Bold", false)),
            ("on", toggle_button("bold", "Bold", true)),
            (
                "icon-off",
                toggle_button_icon("bold", "Bold", false, IconMark::WarningFilled),
            ),
            (
                "icon-on",
                toggle_button_icon("bold", "Bold", true, IconMark::WarningFilled),
            ),
            ("group", three()),
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

    #[test]
    fn enabled_buttons_are_reachable_and_disabled_ones_are_not() {
        let node = toggle_button_group(
            "format",
            vec![
                toggle_button("bold", "Bold", true),
                disabled(toggle_button("italic", "Italic", false)),
            ],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let bold = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/bold"))
            .expect("bold is placed");
        let italic = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/italic"))
            .expect("italic is placed");
        assert!(
            focus.order().iter().any(|o| o == &bold.id),
            "enabled bold declares Focus but is not in focus order"
        );
        assert!(
            !focus.order().iter().any(|o| o == &italic.id),
            "disabled italic must not be reachable"
        );
    }

    #[test]
    fn label_clears_aa_contrast_against_the_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for pressed in [true, false] {
                let item = toggle_button("bold", "Bold", pressed);
                let slot = if pressed {
                    "background@selected"
                } else {
                    "background"
                };
                let bg_name = item
                    .props
                    .tokens
                    .get(slot)
                    .unwrap_or_else(|| panic!("button binds {slot}"));
                let bg = color(&theme, bg_name.as_str());
                let label = named(&item, "label");
                let inks = inks(label);
                assert!(!inks.is_empty(), "label binds an ink");
                let opacity = label.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "pressed={pressed} at {ratio:.2}:1 against {} fails AA \
                         {MIN_TEXT_CONTRAST}:1",
                        bg_name.as_str()
                    );
                }
            }
        }
    }
}
