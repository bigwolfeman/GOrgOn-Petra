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
    ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_SM, SIZE_MD, SURFACE_BASE, SURFACE_RAISED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, t,
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
    let mut well = stack(
        key,
        Axis::Horizontal,
        None,
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
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

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
}
