//! `field` — Carbon Text input (slice-e).
//!
//! Two styles: **Default** (this module's default constructors) and **Fluid**.
//! Default `field()` is a single `Input` of height [`SIZE_MD`] (40). Carbon's
//! label-above anatomy lives on [`field_labeled`], not on `field()`, because
//! `tests.rs` (`a_field_is_as_tall_as_size_md`) places `/root/name` and
//! asserts that rect is 40 tall. A label+input wrapper would be taller.
//!
//! Password is skipped (needs View/ViewOff marks and host text secrecy).
//! Focus geometry is host-owned; this file does not paint a ring.

use std::sync::Arc;

use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, SHAPE_SM, SIZE_MD, SPACING_02, SPACING_03, SUPPORT_ERROR, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, ViewNode};

/// Carbon Default sm. `tokens` only ships [`SIZE_MD`] (md / 40).
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;
/// Carbon Fluid `min-block-size`.
const SIZE_FLUID: f32 = 64.0;

/// How one input well is finished.
#[derive(Clone, Copy)]
enum FieldChrome {
    /// Editable Default-style well: Petra's field/border pair.
    Enabled,
    /// Invalid: same well, [`ACCENT_PRIMARY`] border as the error stand-in.
    Invalid,
    /// Readable, not editable. Keeps Focus; drops Key and TextEdit.
    ReadOnly,
    /// Fluid inner input. The wrapper is the well; this node has no fill.
    Nested,
}

/// An editable text field — Carbon Default, size md (40).
///
/// `label` fills both the placeholder shown in the empty box and the
/// accessible name announced for it — a `field` has no separate label
/// element, so the one string an author supplies is both, and there is no
/// path that constructs a `field` with one but not the other. Visible
/// label-above anatomy is [`field_labeled`].
///
/// # The box: a tone *and* an edge, and why it needs both
///
/// A field drew a `text.muted` box until 2026-08-25 — the same tone as the
/// placeholder text inside it, which is the specific way an outlined field
/// reads badly: the frame and the content are the same weight, and the frame
/// is longer.
///
/// The first attempt at fixing that deleted the border outright and leaned on
/// the fill, since [`super::on_layer`] seats a field one step ahead of
/// whatever it is placed on. A capture and a measurement both said that was
/// not enough. **A one-layer step is 1.26:1 in dark and 1.12:1 in light**,
/// against WCAG 2.1 SC 1.4.11's 3:1 floor for the visual information that
/// identifies a control — see [`super::button`]'s `Chrome::Edged` for the
/// full table. Light is the worse case because its layer set alternates
/// rather than ramps, so the step is `#f2f2f2` to `#ffffff` and there is
/// nowhere further to go.
///
/// So the tone carries the depth and [`BORDER_SUBTLE`] carries the boundary,
/// which is M-Carbon's own rule for exactly this pairing — `LAYER_TOKENS`'
/// doc in `crate::token::shipped` records it as *"Borders pair with their
/// same number."* The edge is now 3.34:1 in light instead of 8.70:1: it is
/// still an outlined field, and it is no longer as loud as its own contents.
///
/// Petra's field/border pair is [`SURFACE_RAISED`] + [`BORDER_SUBTLE`]. Carbon
/// wants `$field` + `$border-strong`. `FIELD_TOKENS` exist on `crate::token`
/// but `tokens.rs` does not export a field fill, so this library keeps the
/// pairing it can name.
///
/// Keyboard focus on a field is two vertical bars hugging the left and
/// right, not the underline buttons get. Geometry is `FocusRing::hugs`.
/// This file does not paint a focus ring.
///
/// `NodeKind::Input` is a leaf kind, so unlike [`super::button`] it carries
/// no padding (`Props.padding` is refused on a leaf,
/// `crate::tree::validate::Violation::PaddingOnLeafKind`) — but a paint
/// slot is not a child, so the corner radius still applies directly to the
/// field's own rect. The text inset lives in the painter (`spacing-04`
/// horizontal, vertically centred) because that is the only place a leaf
/// has a chrome rect and a content origin as two different things.
pub fn field(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    input_field(key, label, SIZE_MD, FieldChrome::Enabled)
}

/// Carbon Default, size sm (32).
pub fn field_sm(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    input_field(key, label, SIZE_SM, FieldChrome::Enabled)
}

/// Carbon Default, size lg (48).
pub fn field_lg(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    input_field(key, label, SIZE_LG, FieldChrome::Enabled)
}

/// Carbon Fluid: 64 tall, label stacked inside the well.
///
/// The wrapper is the 64-unit well (fill + edge). The inner `Input` holds
/// `Role::TextInput`; the wrapper does not steal it.
pub fn field_fluid(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_02),
        vec![
            muted_label("label", label.clone()),
            input_field("input", label, 0.0, FieldChrome::Nested),
        ],
    );
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.constraints.vertical.min = Some(SIZE_FLUID);
    node
}

/// Carbon Default anatomy: muted label above a md Input.
///
/// The wrapper has no role. The `"input"` child is the interactive
/// `Role::TextInput` node, 40 tall.
pub fn field_labeled(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![
            muted_label("label", label.clone()),
            input_field("input", label, SIZE_MD, FieldChrome::Enabled),
        ],
    )
}

/// Invalid Default input plus a label-adjacent helper.
///
/// Colour is not the only channel: the Input border is [`ACCENT_PRIMARY`]
/// (Petra's stand-in for Carbon's invalid edge — there is no `$text-error`
/// / `$support-error` in this library) **and** a helper child carries
/// `Invalid: {message}` in [`TEXT_PRIMARY`].
pub fn field_invalid(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    let message = message.into();
    let mut helper = text("helper", format!("Invalid: {message}"));
    // The word carries the state as well as the hue does, which is the point:
    // the operator is red-green colour blind and a red edge on its own is not
    // a channel he can read. Ink stays primary so the message is legible;
    // tinting it would trade a channel he has for one he does not.
    helper
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_02),
        vec![
            input_field("input", label, SIZE_MD, FieldChrome::Invalid),
            helper,
        ],
    );
    // A vertical stack's cross axis is the width, so this is what passes a
    // caller's width down to the well. Without it the field hugged its own
    // placeholder while its plain siblings filled the column, and row 34
    // showed five fields at five widths.
    node.props.align = Some(Align::Stretch);
    node
}

/// Read-only md input: still focusable, not editable, not [`super::disabled`].
///
/// Drops `TextEdit` and `Key`. Keeps `Focus` and the placeholder. Sets
/// `Semantics.read_only` and does not set `disabled`.
pub fn field_readonly(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    input_field(key, label, SIZE_MD, FieldChrome::ReadOnly)
}

/// Put a value in a field.
///
/// Every field constructor in this library takes `(key, label)` and nothing
/// else, so until now no field could contain anything: the engine paints
/// `props.text` when it is non-empty and falls back to `props.placeholder`
/// otherwise (`layout::paint_content_of`), and no constructor ever wrote
/// `text`. Four of the operator's rows on 2026-09-04 — Form, Number input,
/// Search and Text input — were all the same empty well seen four times.
///
/// A modifier rather than seven more constructors, and rather than a third
/// parameter on the seven that exist. [`super::disabled`] and
/// [`super::on_layer`] already establish the shape: a fact the *caller* knows
/// and the constructor cannot is stamped on afterwards. A value is exactly
/// that — the same `field("email", "you@example.com")` is empty on a fresh
/// form and full on a loaded one, and the constructor cannot tell which.
///
/// **It also swaps the ink.** A placeholder is a hint and takes
/// [`TEXT_MUTED`]; a value is content and takes [`TEXT_PRIMARY`]. Leaving the
/// muted tone on real content would make a filled field read as an empty one,
/// which is the defect this function exists to fix, wearing a hat.
///
/// Finds the first [`NodeKind::Input`] in the subtree, itself included, so it
/// works on the flat forms ([`field`], [`field_sm`]) and on the wrapped ones
/// ([`field_labeled`], [`field_fluid`]) without the caller knowing which shape
/// it holds. Passing a node with no `Input` in it returns the node unchanged:
/// there is nothing to fill, and panicking would make the modifier harder to
/// compose than the problem it solves.
///
/// Children are cloned out of their `Arc`s for [`super::disabled`]'s reason —
/// a shared subtree filled in one place and empty in another has to become two
/// subtrees.
#[must_use]
pub fn valued(mut node: ViewNode, value: impl Into<String>) -> ViewNode {
    fn fill(node: &mut ViewNode, value: &str) -> bool {
        if node.kind == NodeKind::Input {
            node.props.text = Some(value.to_owned());
            if !value.is_empty() {
                node.props
                    .tokens
                    .insert("foreground".into(), t(TEXT_PRIMARY));
            }
            return true;
        }
        for child in &mut node.children {
            let mut owned = Arc::unwrap_or_clone(Arc::clone(child));
            if fill(&mut owned, value) {
                *child = Arc::new(owned);
                return true;
            }
        }
        false
    }
    let value = value.into();
    fill(&mut node, &value);
    node
}

fn muted_label(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    node
}

fn input_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    height: f32,
    chrome: FieldChrome,
) -> ViewNode {
    let label = label.into();
    let mut props = Props {
        placeholder: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    match chrome {
        FieldChrome::Nested => {}
        FieldChrome::Enabled | FieldChrome::ReadOnly => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            props.tokens.insert("border".into(), t(BORDER_SUBTLE));
            props.tokens.insert("radius".into(), t(SHAPE_SM));
        }
        FieldChrome::Invalid => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            // The error hue, not the accent. These were the same token, so
            // an invalid field and a focused field drew the identical blue
            // edge and the error state had no visual channel at all beyond
            // its helper text. `SUPPORT_ERROR`'s doc says why the name looked
            // missing when it never was.
            props.tokens.insert("border".into(), t(SUPPORT_ERROR));
            props.tokens.insert("radius".into(), t(SHAPE_SM));
        }
    }
    let intents: &[Interaction] = match chrome {
        FieldChrome::ReadOnly => &[Interaction::Focus],
        FieldChrome::Enabled | FieldChrome::Invalid | FieldChrome::Nested => {
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit]
        }
    };
    let mut node = ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(Role::TextInput, label, intents);
    if height > 0.0 {
        node = node.with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(height),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        });
    }
    if matches!(chrome, FieldChrome::ReadOnly) {
        node.semantics.read_only = true;
    }
    node
}

#[cfg(test)]
mod tests {
    use super::valued;

    use super::{BORDER_SUBTLE, SUPPORT_ERROR, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY};
    // Not from `super`: no shipped code in this file names the accent any
    // more, and that is the change. The invalid field used to bind it, which
    // made it identical to a focused field. It survives here only so the test
    // can assert the two differ.
    use super::{
        SIZE_FLUID, SIZE_LG, SIZE_MD, SIZE_SM, field, field_fluid, field_invalid, field_labeled,
        field_lg, field_readonly, field_sm,
    };
    use crate::component::tokens::ACCENT_PRIMARY;
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
    fn default_field_is_a_single_size_md_input() {
        let node = field("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.key.as_str(), "name");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_eq!(node.semantics.label.as_deref(), Some("Fiber name"));
        assert_eq!(node.props.placeholder.as_deref(), Some("Fiber name"));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Key));
        assert!(node.interactions.contains(&Interaction::TextEdit));
        assert!(!node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert!(node.children.is_empty(), "default field is a leaf Input");
    }

    #[test]
    fn field_sm_is_32_tall() {
        let node = field_sm("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn field_lg_is_48_tall() {
        let node = field_lg("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn field_fluid_is_64_tall_with_the_label_inside() {
        let node = field_fluid("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_FLUID));
        assert_eq!(SIZE_FLUID, 64.0);
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        let label = child(&node, "label");
        assert_eq!(label.kind, NodeKind::Text);
        assert_eq!(label.props.text.as_deref(), Some("Fiber name"));
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.semantics.label.as_deref(), Some("Fiber name"));
        assert!(input.interactions.contains(&Interaction::TextEdit));
        assert!(
            token(input, "border").is_none(),
            "fluid chrome lives on the 64-tall wrapper"
        );
    }

    #[test]
    fn field_labeled_puts_a_muted_label_above_a_size_md_input() {
        let node = field_labeled("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );
        assert!(node.constraints.vertical.min.is_none());
        let label = child(&node, "label");
        assert_eq!(label.kind, NodeKind::Text);
        assert_eq!(label.props.text.as_deref(), Some("Fiber name"));
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.semantics.label.as_deref(), Some("Fiber name"));
        assert_eq!(token(input, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(input, "border"), Some(BORDER_SUBTLE));
    }

    /// An invalid field's edge is the error hue, never the accent, and the
    /// helper text says so in words as well.
    ///
    /// Both halves matter and the first is the one that shipped broken.
    /// `field_invalid` bound `ACCENT_PRIMARY` because `tokens.rs` had no
    /// error name re-exported, so an invalid field and a focused field drew
    /// the **identical** blue edge: the state had no visual channel at all
    /// beyond its message. Asserting the border merely "is not the default"
    /// would have passed the whole time.
    ///
    /// The inequality is asserted explicitly rather than left implied by the
    /// two token names, because the defect was precisely that two names which
    /// should differ resolved to one.
    #[test]
    fn field_invalid_draws_the_error_hue_and_says_so_in_words() {
        let node = field_invalid("name", "Fiber name", "required");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(token(input, "border"), Some(SUPPORT_ERROR));
        assert_ne!(
            token(input, "border"),
            Some(ACCENT_PRIMARY),
            "an invalid field must not wear the accent: a focused field wears \
             it too, and the two states became pixel-identical"
        );
        assert_eq!(token(input, "background"), Some(SURFACE_RAISED));
        assert!(input.interactions.contains(&Interaction::TextEdit));
        let helper = child(&node, "helper");
        assert_eq!(helper.kind, NodeKind::Text);
        assert!(
            helper
                .props
                .text
                .as_deref()
                .is_some_and(|t| t.contains("Invalid")),
            "the error must be carried by a word as well as a hue: the \
             operator is red-green colour blind and cannot rely on the edge"
        );
        assert_eq!(helper.props.text.as_deref(), Some("Invalid: required"));
        assert_eq!(token(helper, "foreground"), Some(TEXT_PRIMARY));
    }

    #[test]
    fn field_readonly_keeps_focus_and_drops_edit() {
        let node = field_readonly("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert!(node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert_eq!(node.interactions, vec![Interaction::Focus]);
        assert!(!node.interactions.contains(&Interaction::TextEdit));
        assert!(!node.interactions.contains(&Interaction::Key));
        assert_eq!(node.props.placeholder.as_deref(), Some("Fiber name"));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
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

    /// Check C/D across every Default and Fluid form, including
    /// `field_invalid` (which is absent from the crate-wide
    /// `full_gallery()` tree — see `tests.rs`'s own comment above
    /// `carbon5` — so this is its only frame-level coverage).
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("default", field("name", "Fiber name")),
            ("sm", field_sm("name", "Fiber name")),
            ("lg", field_lg("name", "Fiber name")),
            ("fluid", field_fluid("name", "Fiber name")),
            ("labeled", field_labeled("name", "Fiber name")),
            ("readonly", field_readonly("name", "Fiber name")),
            ("invalid", field_invalid("name", "Fiber name", "required")),
            (
                "disabled",
                crate::component::disabled(field("name", "Fiber name")),
            ),
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

    /// Check F: an enabled field and a read-only field (which keeps
    /// `Focus`, this module's own doc) are both reachable; a disabled one
    /// is not.
    #[test]
    fn field_focus_reachability_matches_disabled_state() {
        for (label, node, suffix, should_be_focusable) in [
            ("enabled", field("name", "Fiber name"), "/name", true),
            (
                "readonly",
                field_readonly("name", "Fiber name"),
                "/name",
                true,
            ),
            (
                "disabled",
                crate::component::disabled(field("name", "Fiber name")),
                "/name",
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{label}: no placement ending {suffix}"));
            let reachable = focus.order().iter().any(|o| o == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the placeholder text against the well's own resting fill,
    /// across Default, sm, lg and read-only, in both themes.
    #[test]
    fn placeholder_clears_aa_contrast_against_its_own_well_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("default", field("name", "Fiber name")),
                ("sm", field_sm("name", "Fiber name")),
                ("lg", field_lg("name", "Fiber name")),
                ("readonly", field_readonly("name", "Fiber name")),
            ] {
                let well_bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: well binds a resting background"));
                let well_bg = color(&theme, well_bg_name.as_str());
                let fg_name = node
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{label}: well binds a foreground"));
                let opacity = node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(well_bg);
                let ratio = fg.contrast_ratio(well_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    well_bg_name.as_str()
                );
            }
        }
    }

    /// A field with a value paints the value, not the placeholder, and paints
    /// it in content ink.
    ///
    /// This is the whole of what was missing. `layout::paint_content_of`
    /// already prefers a non-empty `props.text` over `props.placeholder` for
    /// every `NodeKind::Input`; no constructor had ever written `text`, so
    /// four of the operator's rows were the same empty well seen four times.
    ///
    /// Asserts the ink as well as the string. A value left in `TEXT_MUTED`
    /// still reads as a placeholder, so painting the right characters in the
    /// wrong tone would satisfy a text-only assertion and keep the defect.
    #[test]
    fn a_valued_field_paints_its_value_in_content_ink() {
        let empty = field("f", "you@example.com");
        assert_eq!(empty.props.text, None);
        assert_eq!(
            empty.props.tokens.get("foreground").map(TokenName::as_str),
            Some(TEXT_MUTED),
            "an empty field's placeholder is a hint and takes the muted tone"
        );

        let full = valued(field("f", "you@example.com"), "wolfe@gorgon.dev");
        assert_eq!(full.props.text.as_deref(), Some("wolfe@gorgon.dev"));
        assert_eq!(
            full.props.tokens.get("foreground").map(TokenName::as_str),
            Some(TEXT_PRIMARY),
            "a value is content, not a hint: leaving it muted makes a filled \
             field read as an empty one, which is the defect this fixes"
        );
        assert_eq!(
            full.props.placeholder.as_deref(),
            Some("you@example.com"),
            "the placeholder survives, so clearing the value restores the hint"
        );
    }

    /// `valued` reaches the `Input` inside a wrapper, so a caller does not
    /// have to know which shape a constructor returned.
    ///
    /// `field` is an `Input` at its root; `field_labeled` and `field_fluid`
    /// wrap one in a stack. A modifier that only handled the flat case would
    /// silently do nothing for two of the seven constructors — silently, which
    /// is why this asserts rather than trusting the walk.
    #[test]
    fn valued_reaches_the_input_inside_a_wrapper() {
        for node in [
            valued(field_labeled("f", "Name"), "Wolfe"),
            valued(field_fluid("f", "Name"), "Wolfe"),
        ] {
            let input = child(&node, "input");
            assert_eq!(input.kind, NodeKind::Input);
            assert_eq!(
                input.props.text.as_deref(),
                Some("Wolfe"),
                "the wrapper's Input child never received the value"
            );
        }
    }

    /// A node with no `Input` comes back untouched rather than panicking.
    ///
    /// Composing modifiers is the point of this shape, and a modifier that
    /// panics on a tree it does not recognise is harder to compose than
    /// writing the field out by hand.
    #[test]
    fn valued_on_a_tree_with_no_input_is_a_no_op() {
        let plain = super::text("t", "not a field");
        let after = valued(plain.clone(), "ignored");
        assert_eq!(after.props.text, plain.props.text);
    }
}
