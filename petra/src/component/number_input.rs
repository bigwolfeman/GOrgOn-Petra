//! `number_input` — Carbon Number input (slice-d).
//!
//! Anatomy: label-adjacent well + numeric value + two labelled stepper
//! buttons. Steppers are discrete Click targets; there is no pointer-drag
//! and no hold-to-repeat. Carbon overlay-positioned controls become a
//! trailing pair of [`Role::Button`] children because `NodeKind::Input` is
//! a leaf.
//!
//! Sizes MEASURED `_number-input.scss`: sm 32, md 40 (default), lg 48.
//! Fill/edge is Petra's field pair: [`SURFACE_RAISED`] + [`BORDER_SUBTLE`].

use super::stack;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_SM, SIZE_MD, SPACING_05, SURFACE_BASE, SURFACE_RAISED,
    TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, ViewNode};

/// Carbon Default sm. `tokens` only ships [`SIZE_MD`] (md / 40).
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;

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

/// Invalid md well: accent border plus helper text, same pattern as
/// [`super::field::field_invalid`].
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
    // `None` spacing drew the value and both stepper captions flush against
    // each other — the value "12" ran straight into the caption
    // "Decrement" with no gap, and "Decrement" ran straight into
    // "Increment". Carbon's own field clears `padding-inline-end:
    // $spacing-05` (16px, MEASURED `_number-input.scss:59-66`) for its
    // absolutely-positioned controls; FR-026 makes Petra's steppers inline
    // siblings instead of an overlay (this module's own doc), so that same
    // 16px becomes a real gap between each of the three cells rather than
    // reserved padding under an overlay.
    let mut well = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_05),
        vec![
            value_field("value", label.clone(), value, height),
            stepper("decrement", "Decrement", height),
            stepper("increment", "Increment", height),
        ],
    );
    well.props.align = Some(Align::Center);
    well.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    well.props.tokens.insert(
        "border".into(),
        t(match chrome {
            Chrome::Enabled => BORDER_SUBTLE,
            Chrome::Invalid => ACCENT_PRIMARY,
        }),
    );
    well.props.tokens.insert("radius".into(), t(SHAPE_SM));
    well.constraints.vertical.min = Some(height);
    well
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
        .interactive(
            Role::TextInput,
            label,
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
        )
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(height),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        })
}

fn stepper(key: &'static str, label: &'static str, height: f32) -> ViewNode {
    let mut caption = text("label", label);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.with_constraints(Constraints {
        // Horizontal takes a floor, not a fixed width. Carbon's stepper
        // width (md 40, `_number-input.scss:153` controls-width / 2; lg 48
        // `:403`; sm 32 `:416`) is the hit box for an icon-only
        // Add/Subtract glyph; FR-026 replaces the icon with the word
        // "Increment"/"Decrement" (this module's own doc), which does not
        // fit inside a box pinned to the icon's own width — the same
        // Class-4 shape as Modal's `close_button` defect. `min` stays the
        // floor so the tap target never shrinks below Carbon's number; the
        // label decides how much wider it needs
        // (`frame_geometry_has_no_degenerate_or_overflowing_placements`
        // caught it once Step 0 put Number input in `full_gallery()`).
        horizontal: AxisConstraint {
            min: Some(height),
            max: None,
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
        &[Interaction::Focus, Interaction::Click],
    )
}

#[cfg(test)]
mod tests {
    use super::{
        SIZE_LG, SIZE_MD, SIZE_SM, number_input, number_input_invalid, number_input_lg,
        number_input_sm,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SURFACE_RAISED, TEXT_PRIMARY};
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

    #[test]
    fn number_input_is_size_md_with_labelled_steppers() {
        let node = number_input("count", "Replicas", "3");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
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
        assert!(dec.interactions.contains(&Interaction::Click));
        assert!(!dec.interactions.contains(&Interaction::Drag));

        let inc = child(&node, "increment");
        assert_eq!(inc.semantics.role, Some(Role::Button));
        assert_eq!(inc.semantics.label.as_deref(), Some("Increment"));
        assert!(inc.interactions.contains(&Interaction::Click));
        assert!(!inc.interactions.contains(&Interaction::Drag));
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

    #[test]
    fn number_input_invalid_pairs_accent_border_with_helper_text() {
        let node = number_input_invalid("count", "Replicas", "x", "must be a number");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(node.semantics.role.is_none());
        let well = child(&node, "input");
        assert_eq!(token(well, "border"), Some(ACCENT_PRIMARY));
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
    /// steppers pin `Constraints.horizontal.min == max == height`
    /// (`stepper`'s own doc), the Carbon icon-only hit-box number
    /// (`_number-input.scss:153,403,416`, controls width / 2). Petra draws
    /// the word "Increment"/"Decrement" instead of an icon (FR-026), which
    /// does not fit inside a box pinned to the icon's own width — the same
    /// shape as Modal's `close_button` defect. `min` stays the floor so the
    /// tap target never shrinks below Carbon's number; `max: None` lets the
    /// label decide the width.
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

    /// Check E: the value text and both stepper labels against the well's
    /// own resting fill, in both themes.
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
                let label = child(stepper, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("stepper label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str())
                    .faded(opacity)
                    .over(stepper_bg);
                let ratio = fg.contrast_ratio(stepper_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{stepper_key} label at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
