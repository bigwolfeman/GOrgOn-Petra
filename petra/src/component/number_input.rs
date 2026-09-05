//! `number_input` — Carbon Number input (slice-d).
//!
//! Anatomy: label-adjacent well + numeric value + two labelled stepper
//! buttons drawing Carbon's `Subtract` / `Add` ([`IconMark::Subtract`],
//! [`IconMark::Add`], `fill: $icon-primary`, slice-d). Steppers are
//! discrete Click targets; there is no pointer-drag and no hold-to-repeat.
//! Carbon overlay-positioned controls become a trailing pair of
//! [`Role::Button`] children because `NodeKind::Input` is a leaf. Each
//! stepper's accessible name is the word (`"Decrement"` / `"Increment"`),
//! so the glyph is never the only channel (FR-026).
//!
//! Sizes MEASURED `_number-input.scss`: sm 32, md 40 (default), lg 48.
//!
//! # The well, as Carbon draws it (`22-number-input.png`, 2026-09-04)
//!
//! One filled well ([`super::field::bind_field_chrome`]: `$field` under a
//! `$border-strong` bottom rule, square) holding, left to right: the value,
//! then at the trailing edge the Subtract button, a one-unit
//! [`BORDER_SUBTLE`] rule divider 16 tall (`_number-input.scss:285-286`),
//! and the Add button. Each stepper is a `height × height` square resting
//! in the well's own tone — flush with the well, no box — and a
//! [`LAYER_HOVER`] fill under the pointer (`.cds--number__control-btn:hover
//! { background-color: $field-hover }`). Nothing separates the value from
//! the controls but the value cell's own width: Carbon reserves
//! `padding-inline-end` under an overlay, and here the steppers are inline
//! siblings that take exactly their square and leave the value the rest.
//!
//! Before this the well was a `border.subtle` box with a radius, the
//! steppers were `surface.base` boxes with a 16-unit gap between every
//! cell, and the whole well hugged the value's width so the steppers hung
//! outside it. The operator's word was "visually broken", and it was.

use super::field::bind_field_chrome;
use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SIZE_MD, SUPPORT_ERROR, SURFACE_RAISED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, Interaction, Justify, Key, NodeKind, Props, Role, ViewNode,
};

/// Carbon Default sm. `tokens` only ships [`SIZE_MD`] (md / 40).
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;
/// MEASURED `_number-input.scss:285-286`: the rule divider between the two
/// steppers is 1 wide and 16 tall.
const DIVIDER_WIDTH: f32 = 1.0;
/// See [`DIVIDER_WIDTH`].
const DIVIDER_HEIGHT: f32 = 16.0;

#[derive(Clone, Copy)]
enum Chrome {
    Enabled,
    Invalid,
}

/// Numeric field, Carbon md (40), with Increment and Decrement buttons.
///
/// `label` names the input (FR-058). `value` is the current numeric text.
pub fn number_input(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    number_sized(key, label, value, SIZE_MD, Chrome::Enabled)
}

/// Carbon sm (32).
pub fn number_input_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    number_sized(key, label, value, SIZE_SM, Chrome::Enabled)
}

/// Carbon lg (48).
pub fn number_input_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    number_sized(key, label, value, SIZE_LG, Chrome::Enabled)
}

/// Invalid md well: a [`SUPPORT_ERROR`] outline plus helper text, the same
/// two channels as [`super::field::field_invalid`].
pub fn number_input_invalid(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    let message = message.into();
    let mut helper = text("helper", format!("Invalid: {message}"));
    helper
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    stack(
        key,
        Axis::Vertical,
        Some(super::tokens::SPACING_02),
        vec![
            number_sized("input", label, value, SIZE_MD, Chrome::Invalid),
            helper,
        ],
    )
}

fn number_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    height: f32,
    chrome: Chrome,
) -> ViewNode {
    let label = label.into();
    let value = value.into();
    // No spacing: the steppers are glyph squares that butt against each
    // other with the rule divider between them, and the value cell takes
    // whatever width the well has left. The stack distributes an exact
    // budget least-flexible first, so the two pinned squares and the
    // one-unit divider land before the flexible value cell is offered the
    // remainder (`layout::stack`).
    let mut well = stack(
        key,
        Axis::Horizontal,
        None,
        vec![
            value_field("value", label.clone(), value, height),
            stepper("decrement", "Decrement", IconMark::Subtract, height),
            divider(),
            stepper("increment", "Increment", IconMark::Add, height),
        ],
    );
    well.props.align = Some(Align::Center);
    match chrome {
        Chrome::Enabled => bind_field_chrome(&mut well.props),
        Chrome::Invalid => {
            well.props
                .tokens
                .insert("background".into(), t(SURFACE_RAISED));
            // The error hue on all four sides, as `field_invalid` does, and
            // never the accent: the accent is the focus ring, and an invalid
            // well in the accent was indistinguishable from a focused one.
            well.props.tokens.insert("border".into(), t(SUPPORT_ERROR));
        }
    }
    well.constraints.vertical.min = Some(height);
    well
}

/// The one-unit rule between the two steppers: `.cds--number__rule-divider`,
/// `$border-subtle`, 1 × 16 (`_number-input.scss:285-286`).
fn divider() -> ViewNode {
    swatch(
        "divider",
        DIVIDER_WIDTH,
        DIVIDER_HEIGHT,
        Some(BORDER_SUBTLE),
        None,
        None,
    )
}

fn value_field(key: &'static str, label: String, value: String, height: f32) -> ViewNode {
    let mut props = Props {
        text: Some(value),
        placeholder: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(Role::TextInput, label, super::field::EDITABLE_TEXT_INTENTS)
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(height),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        })
}

/// One stepper: a `height × height` square holding a centred glyph, resting
/// in the well's own tone so it is flush with the well, and lifting to
/// [`LAYER_HOVER`] under the pointer.
///
/// The resting fill is [`SURFACE_RAISED`] and not absent: a state-decorated
/// binding needs a resting one under it
/// (`tests::a_state_decorated_token_always_has_a_resting_binding`), and the
/// well's tone is the one that draws nothing a reader can see.
///
/// Pinned on both axes. Carbon's stepper width (md 40,
/// `_number-input.scss:153` controls-width / 2; lg 48 `:403`; sm 32 `:416`)
/// is the hit box for an icon-only Add/Subtract glyph, and the glyph fits
/// it. Before the glyph the square held the word "Increment", which did not
/// fit and forced `max` open; the square is closed again.
fn stepper(key: &'static str, label: &'static str, mark: IconMark, height: f32) -> ViewNode {
    let glyph = icon_toned("glyph", mark, IconTone::Primary);
    let mut node = stack(key, Axis::Horizontal, None, vec![glyph]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    // The square is as tall as the well and paints after it, so its fill
    // would cover the well's bottom rule; it carries the same rule itself,
    // and the rule runs unbroken under the value and both controls.
    bind_field_chrome(&mut node.props);
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
    })
    .interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click, Interaction::Hover],
    )
}

#[cfg(test)]
mod tests {
    use super::{
        IconMark, IconTone, SIZE_LG, SIZE_MD, SIZE_SM, icon_toned, number_input,
        number_input_invalid, number_input_lg, number_input_sm,
    };
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_STRONG, BORDER_SUBTLE, LAYER_HOVER, SUPPORT_ERROR, SURFACE_RAISED,
        TEXT_PRIMARY,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, inks, validated_with};
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

    #[test]
    fn number_input_is_size_md_with_labelled_steppers() {
        let node = number_input("count", "Replicas", "3");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        // Carbon's `.cds--number` well: a fill under a `$border-strong`
        // bottom rule and nothing on the other sides. A `border` here is
        // the box that read as "visually broken".
        assert_eq!(token(&node, "border-bottom"), Some(BORDER_STRONG));
        assert_eq!(token(&node, "border"), None, "the well is not boxed");
        assert_eq!(token(&node, "radius"), None, "square corners");
        assert_eq!(
            node.props.spacing, None,
            "the steppers butt against the divider and the value cell takes \
             the rest; a gap between the cells is not Carbon's anatomy"
        );
        assert!(!node.interactions.contains(&Interaction::Drag));

        let input = child(&node, "value");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.semantics.label.as_deref(), Some("Replicas"));
        assert_eq!(input.props.text.as_deref(), Some("3"));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert!(input.interactions.contains(&Interaction::TextEdit));
        assert!(!input.interactions.contains(&Interaction::Drag));

        let dec = child(&node, "decrement");
        assert_eq!(dec.semantics.role, Some(Role::Button));
        assert_eq!(dec.semantics.label.as_deref(), Some("Decrement"));
        let glyph = child(dec, "glyph");
        assert_eq!(glyph.kind, NodeKind::Canvas);
        assert_eq!(
            glyph.props.text, None,
            "the stepper is a glyph, not the word"
        );
        assert_eq!(
            glyph.props.canvas,
            icon_toned("glyph", IconMark::Subtract, IconTone::Primary)
                .props
                .canvas,
            "decrement draws Carbon's Subtract"
        );
        assert!(dec.interactions.contains(&Interaction::Click));
        assert!(!dec.interactions.contains(&Interaction::Drag));

        let inc = child(&node, "increment");
        assert_eq!(inc.semantics.role, Some(Role::Button));
        assert_eq!(inc.semantics.label.as_deref(), Some("Increment"));
        assert_eq!(
            child(inc, "glyph").props.canvas,
            icon_toned("glyph", IconMark::Add, IconTone::Primary)
                .props
                .canvas,
            "increment draws Carbon's Add"
        );
        assert!(inc.interactions.contains(&Interaction::Click));
        assert!(!inc.interactions.contains(&Interaction::Drag));
    }

    /// The controls are Carbon's: two `height × height` squares flush with
    /// the well, a hover fill, and a one-unit `$border-subtle` rule 16 tall
    /// between them (`_number-input.scss:153, 285-286`).
    ///
    /// "Flush with the well" is the assertion that catches the old picture:
    /// each stepper used to be a `surface.base` box, which painted two dark
    /// squares over the well's fill, and the glyphs sat outside them.
    #[test]
    fn steppers_are_unfilled_squares_either_side_of_a_rule_divider() {
        for (node, size) in [
            (number_input_sm("n", "Replicas", "3"), SIZE_SM),
            (number_input("n", "Replicas", "3"), SIZE_MD),
            (number_input_lg("n", "Replicas", "3"), SIZE_LG),
        ] {
            let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
            assert_eq!(
                keys,
                ["value", "decrement", "divider", "increment"],
                "value, then the controls at the trailing edge with the rule \
                 between them"
            );
            for key in ["decrement", "increment"] {
                let stepper = child(&node, key);
                assert_eq!(stepper.constraints.horizontal.min, Some(size));
                assert_eq!(stepper.constraints.horizontal.max, Some(size));
                assert_eq!(stepper.constraints.vertical.min, Some(size));
                assert_eq!(stepper.constraints.vertical.max, Some(size));
                assert_eq!(
                    token(stepper, "background"),
                    token(&node, "background"),
                    "{key}: a resting stepper is flush with the well"
                );
                assert_eq!(token(stepper, "background"), Some(SURFACE_RAISED));
                assert_eq!(
                    token(stepper, "border-bottom"),
                    Some(BORDER_STRONG),
                    "{key}: the well's rule runs under the control, which \
                     paints over the well"
                );
                assert_eq!(token(stepper, "background@hover"), Some(LAYER_HOVER));
                assert!(stepper.interactions.contains(&Interaction::Hover));
                assert_eq!(
                    stepper.props.justify,
                    Some(crate::tree::Justify::Center),
                    "{key}: the glyph is centred in its square"
                );
            }
            let divider = child(&node, "divider");
            assert_eq!(divider.constraints.horizontal.min, Some(1.0));
            assert_eq!(divider.constraints.horizontal.max, Some(1.0));
            assert_eq!(divider.constraints.vertical.min, Some(16.0));
            assert_eq!(divider.constraints.vertical.max, Some(16.0));
            assert_eq!(token(divider, "background"), Some(BORDER_SUBTLE));
            assert!(!divider.is_interactive());
        }
    }

    #[test]
    fn number_input_sm_is_32_and_lg_is_48() {
        let sm = number_input_sm("count", "Replicas", "3");
        assert_eq!(sm.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(child(&sm, "value").semantics.role, Some(Role::TextInput));
        let lg = number_input_lg("count", "Replicas", "3");
        assert_eq!(lg.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(
            child(&lg, "increment").semantics.label.as_deref(),
            Some("Increment")
        );
    }

    /// An invalid well is outlined in the error hue on all four sides and
    /// says so in words. Never the accent: that is the focus ring's colour,
    /// and an invalid well in it was indistinguishable from a focused one.
    #[test]
    fn number_input_invalid_pairs_an_error_outline_with_helper_text() {
        let node = number_input_invalid("count", "Replicas", "x", "must be a number");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(node.semantics.role.is_none());
        let well = child(&node, "input");
        assert_eq!(token(well, "border"), Some(SUPPORT_ERROR));
        assert_ne!(token(well, "border"), Some(ACCENT_PRIMARY));
        assert_eq!(
            token(well, "border-bottom"),
            None,
            "the outline replaces the resting rule"
        );
        assert_eq!(token(well, "background"), Some(SURFACE_RAISED));
        assert_eq!(well.constraints.vertical.min, Some(SIZE_MD));
        let input = child(well, "value");
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        let helper = child(&node, "helper");
        assert_eq!(helper.kind, NodeKind::Text);
        assert_eq!(
            helper.props.text.as_deref(),
            Some("Invalid: must be a number")
        );
        assert_eq!(token(helper, "foreground"), Some(TEXT_PRIMARY));
        let _ = child(well, "increment");
        let _ = child(well, "decrement");
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

    /// Check C/D across sizes, plus the invalid form. Class 4: the
    /// steppers take `Constraints.horizontal.min == height` as a floor
    /// (`stepper`'s own doc), the Carbon icon-only hit-box number
    /// (`_number-input.scss:153,403,416`, controls width / 2), and `max:
    /// None` — lifted while the stepper held a word instead of a glyph.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        check_geometry(&petrify_lone(number_input("count", "Replicas", "3")), "md");
        check_geometry(
            &petrify_lone(number_input_sm("count", "Replicas", "3")),
            "sm",
        );
        check_geometry(
            &petrify_lone(number_input_lg("count", "Replicas", "3")),
            "lg",
        );
        check_geometry(
            &petrify_lone(number_input_invalid(
                "count",
                "Replicas",
                "x",
                "must be a number",
            )),
            "invalid",
        );
    }

    /// Check F: the value field and both steppers declare `Focus` and are
    /// reachable.
    #[test]
    fn value_field_and_steppers_are_reachable() {
        let frame = petrify_lone(number_input("count", "Replicas", "3"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        for suffix in ["/value", "/decrement", "/increment"] {
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"));
            assert!(
                order.iter().any(|o| o == &placement.id),
                "{suffix} declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: the value text against the well's own resting fill and both
    /// stepper glyphs against the stepper's, in both themes.
    #[test]
    fn well_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = number_input("count", "Replicas", "3");
            let well_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the well binds a resting background");
            let well_bg = color(&theme, well_bg_name.as_str());
            let value = child(&node, "value");
            {
                let fg_name = value
                    .props
                    .tokens
                    .get("foreground")
                    .expect("value text binds a foreground");
                let opacity = value.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(well_bg);
                let ratio = fg.contrast_ratio(well_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "value at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
            for stepper_key in ["decrement", "increment"] {
                let stepper = child(&node, stepper_key);
                let stepper_bg_name = stepper
                    .props
                    .tokens
                    .get("background")
                    .expect("stepper binds a resting background");
                let stepper_bg = color(&theme, stepper_bg_name.as_str());
                let glyph = child(stepper, "glyph");
                let inks = inks(glyph);
                assert!(!inks.is_empty(), "{stepper_key} glyph binds an ink");
                let opacity = glyph.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
                    let fg = color(&theme, fg_name.as_str())
                        .faded(opacity)
                        .over(stepper_bg);
                    let ratio = fg.contrast_ratio(stepper_bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{stepper_key} glyph at {ratio:.2}:1 against {} fails AA \
                         {MIN_TEXT_CONTRAST}:1",
                        fg_name.as_str()
                    );
                }
            }
        }
    }
}
