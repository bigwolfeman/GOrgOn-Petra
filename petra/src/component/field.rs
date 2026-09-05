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
    BORDER_STRONG, BORDER_SUBTLE, SIZE_MD, SPACING_02, SPACING_03, SUPPORT_ERROR, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, Interaction, Key, NodeKind, Props, Role, ViewNode,
};

/// What an editable text input declares.
///
/// `Click` is in the list because **a person reaches a field by clicking
/// it**. `hit_test` aims a press at `Interaction::Click`, and the host seats
/// keyboard focus only on a `Route::Pointer`; an input that declared
/// `Focus, Key, TextEdit` and no `Click` was therefore invisible to the
/// pointer. It could be Tab-ed to and driven by `Action::Focus`, which is why
/// three green tests missed it, and it could not be clicked into, which is
/// all an operator ever tries. `Click` does not make the field activate on
/// Enter: it already declares `Key`, so `route_above` was routing Enter to it
/// before this.
///
/// Shared with [`super::search`] and [`super::number_input`], which build the
/// same `NodeKind::Input` leaf and drifted from this list once already.
pub(super) const EDITABLE_TEXT_INTENTS: &[Interaction] = &[
    Interaction::Focus,
    Interaction::Click,
    Interaction::Key,
    Interaction::TextEdit,
];

/// Carbon Default sm. `tokens` only ships [`SIZE_MD`] (md / 40).
const SIZE_SM: f32 = 32.0;
/// Carbon Default lg.
const SIZE_LG: f32 = 48.0;
/// Carbon Fluid `min-block-size`.
const SIZE_FLUID: f32 = 64.0;

/// How one input well is finished.
#[derive(Clone, Copy)]
enum FieldChrome {
    /// Editable Default-style well: a fill and a bottom rule.
    Enabled,
    /// Invalid: same fill, a [`SUPPORT_ERROR`] outline on all four sides.
    Invalid,
    /// Readable, not editable. Keeps Focus; drops Key and TextEdit. No fill,
    /// a subtle bottom rule.
    ReadOnly,
    /// Fluid inner input. The wrapper is the well; this node has no fill.
    Nested,
}

/// Bind Carbon's field chrome onto `props`: the well every text-shaped
/// control in this library shares.
///
/// **A fill and a bottom rule, and nothing on the other three sides.** This
/// is `_text-input.scss`'s `.cds--text-input` — `background-color: $field`,
/// `border-block-end: 1px solid $border-strong` — and `_search.scss` and
/// `_number-input.scss` say the same of their wells (slice-e "Text input",
/// slice-d "Search", "Number input"). Square corners: Carbon's field has no
/// radius. Until 2026-09-04 every one of those wells drew a full
/// [`BORDER_SUBTLE`] box with a 2-unit radius, because a box was the only
/// edge the painter could draw, and the operator called all five rows "not
/// Carbon style" in one breath. `border-bottom` is the slot that exists so
/// this function can say what Carbon says.
///
/// `pub(super)` so [`super::search`] and [`super::number_input`] bind the
/// same three lines rather than three copies of them that drift; the
/// editable-intent list already went that way once
/// ([`EDITABLE_TEXT_INTENTS`]).
pub(super) fn bind_field_chrome(props: &mut Props) {
    props.tokens.insert("background".into(), t(SURFACE_RAISED));
    props
        .tokens
        .insert("border-bottom".into(), t(BORDER_STRONG));
}

/// A muted label above `control`, Carbon's `.cds--label` (slice-e "Text
/// input": `$text-secondary`, `margin-bottom: 8px`).
///
/// The one label shape every form control in the inventory shares, so it is
/// one function and not one per control. `key` names the wrapper; the
/// control keeps the key it was built with, and a route to the control
/// still names the wrapper's key in its path, so a page matching with
/// `path_has` on either works.
///
/// The wrapper has no role: it must not steal the control's.
#[must_use]
pub fn labeled(key: impl Into<Key>, label: impl Into<String>, control: ViewNode) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![muted_label("label", label), control],
    );
    // A vertical stack's cross axis is the width. Stretch passes a caller's
    // width down to the control, so a labelled field fills its column the
    // way a bare one does under `filled_body`.
    node.props.align = Some(Align::Stretch);
    node
}

/// An editable text field — Carbon Default, size md (40).
///
/// `label` fills both the placeholder shown in the empty box and the
/// accessible name announced for it — a `field` has no separate label
/// element, so the one string an author supplies is both, and there is no
/// path that constructs a `field` with one but not the other. Visible
/// label-above anatomy is [`field_labeled`].
///
/// # The well: a fill and a bottom rule
///
/// A field drew a `text.muted` box until 2026-08-25 — the same tone as the
/// placeholder text inside it, which is the specific way an outlined field
/// reads badly: the frame and the content are the same weight, and the frame
/// is longer. It then drew a [`BORDER_SUBTLE`] box with a 2-unit radius
/// until 2026-09-04, because that was the only edge the painter had, and the
/// operator called it "not IBM Carbon style" twice.
///
/// Carbon's field is [`bind_field_chrome`]: `$field` under a
/// `1px solid $border-strong` bottom rule, square, and nothing on the other
/// three sides. The fill is [`SURFACE_RAISED`] — `field-01` is `layer-01` by
/// definition (`FIELD_TOKENS` in `crate::token::shipped`), and `surface.raised`
/// is the name [`super::on_layer`] knows how to reseat — and the rule is
/// [`BORDER_STRONG`] through the `border-bottom` slot. The one-layer step
/// alone is 1.26:1 in dark and 1.12:1 in light, under SC 1.4.11's 3:1, which
/// is why the rule is there: it is the edge that identifies the control.
///
/// Keyboard focus on a field is two vertical bars hugging the left and
/// right, not the underline buttons get. The field declares that
/// ([`FocusFigure::Hug`]); the geometry is `FocusRing::hugs`, and the host
/// paints it. This file does not paint a focus ring. Carbon's is a 2px
/// outline on all four sides; that is the host's figure to change, not
/// this file's.
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
/// `Role::TextInput`; the wrapper does not steal it. Focus shows on the
/// wrapper: the wrapper is the hull ([`FocusFigure::Hug`]) and the input
/// says so ([`FocusFigure::HugWell`]), which is Carbon's 2px focus border
/// on `.cds--text-input--fluid` and not on the `<input>`.
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
    bind_field_chrome(&mut node.props);
    node.semantics.focus_figure = FocusFigure::Hug;
    node.constraints.vertical.min = Some(SIZE_FLUID);
    node
}

/// Carbon Default anatomy: muted label above a md Input.
///
/// [`labeled`] over [`field`]. The wrapper has no role. The `"input"` child
/// is the interactive `Role::TextInput` node, 40 tall.
///
/// The label is also the placeholder. Reasonable when the label is the only
/// thing there is to say; the word then appears twice, once above the well
/// and once inside it. [`hinted`] is what puts something else inside.
pub fn field_labeled(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    labeled(
        key,
        label.clone(),
        input_field("input", label, SIZE_MD, FieldChrome::Enabled),
    )
}

/// Invalid Default input plus a label-adjacent helper.
///
/// Colour is not the only channel: the Input outline is [`SUPPORT_ERROR`]
/// on all four sides (Carbon's invalid field is a 2px `$support-error`
/// outline, slice-e; this painter's edge is one unit) **and** a helper child
/// carries `Invalid: {message}` in [`TEXT_PRIMARY`].
pub fn field_invalid(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    field_validated(key, label, Some(message.into()))
}

/// A Default input that carries its own validity.
///
/// `None` builds the enabled well; `Some(message)` builds
/// [`field_invalid`]'s well plus its helper line.
///
/// **Both builds have the same shape**, so the well's id is `{key}/input`
/// either way. That is the whole reason this constructor exists rather than
/// the caller branching between [`field`] and [`field_invalid`]: those two
/// place the well at different depths, so the keystroke that made a value
/// legal would re-key the node under the operator's cursor and drop his
/// focus mid-edit. A one-child stack costs nothing — a vertical stack's
/// spacing only applies *between* children.
pub fn field_validated(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: Option<String>,
) -> ViewNode {
    let chrome = if message.is_some() {
        FieldChrome::Invalid
    } else {
        FieldChrome::Enabled
    };
    let mut children = vec![input_field("input", label, SIZE_MD, chrome)];
    if let Some(message) = message {
        let mut helper = text("helper", format!("Invalid: {message}"));
        // The word carries the state as well as the hue does, which is the
        // point: the operator is red-green colour blind and a red edge on
        // its own is not a channel he can read. Ink stays primary so the
        // message is legible; tinting it would trade a channel he has for
        // one he does not.
        helper
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_PRIMARY));
        children.push(helper);
    }
    let mut node = stack(key, Axis::Vertical, Some(SPACING_02), children);
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

/// Give the input inside `node` a placeholder of its own.
///
/// Carbon's `TextInput` carries `labelText` and `placeholder` as two props
/// and uses them for two things: the label names the field, the placeholder
/// shows an example of the value. This library builds an input from one
/// string that does both jobs ([`input_field`] binds it to
/// `Props::placeholder` and to the `Role::TextInput` accessible name), so a
/// field under a [`labeled`] wrapper printed its own name twice — once above
/// the well and once inside it. Photographed on the Form row 2026-09-05
/// against `ignored/carbon-ref/shots/13-form.png`, whose well is empty.
///
/// This changes **only** what the empty box draws. The accessible name stays
/// the label, which is the whole point: passing the hint as the label instead
/// would fix the picture and make a screen reader announce "fiber-7" for a
/// field the page calls "Name".
///
/// Shaped like [`valued`] — take a built control, reach the one
/// `NodeKind::Input` in it, change one field — so a page keeps the keys it
/// already routes on and this composes with `valued` in either order. It is
/// a no-op on a tree with no input, same as [`valued`].
#[must_use]
pub fn hinted(mut node: ViewNode, hint: impl Into<String>) -> ViewNode {
    fn fill(node: &mut ViewNode, hint: &str) -> bool {
        if node.kind == NodeKind::Input {
            node.props.placeholder = Some(hint.to_owned());
            return true;
        }
        for child in &mut node.children {
            let mut owned = Arc::unwrap_or_clone(Arc::clone(child));
            if fill(&mut owned, hint) {
                *child = Arc::new(owned);
                return true;
            }
        }
        false
    }
    let hint = hint.into();
    fill(&mut node, &hint);
    node
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
        FieldChrome::Enabled => bind_field_chrome(&mut props),
        FieldChrome::ReadOnly => {
            // Carbon's read-only field has no fill and a `$border-subtle`
            // rule (`_text-input.scss` `--readonly`: `background:
            // transparent`, `border-block-end-color: $border-subtle`). The
            // missing fill is the channel: a field you cannot type into
            // does not look like a well you could.
            props
                .tokens
                .insert("border-bottom".into(), t(BORDER_SUBTLE));
        }
        FieldChrome::Invalid => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            // The error hue, not the accent, and on all four sides: Carbon's
            // invalid field is an outline, the one state where the field is
            // boxed. These were the same token as the accent once, so an
            // invalid field and a focused field drew the identical blue
            // edge. `SUPPORT_ERROR`'s doc says why the name looked missing
            // when it never was.
            props.tokens.insert("border".into(), t(SUPPORT_ERROR));
        }
    }
    let intents: &[Interaction] = match chrome {
        FieldChrome::ReadOnly => &[Interaction::Focus],
        FieldChrome::Enabled | FieldChrome::Invalid | FieldChrome::Nested => EDITABLE_TEXT_INTENTS,
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
    // A bare input is its own well and hugs itself. A nested one sits in a
    // wrapper that is the well, and shows its focus there.
    node.semantics.focus_figure = match chrome {
        FieldChrome::Nested => FocusFigure::HugWell,
        FieldChrome::Enabled | FieldChrome::Invalid | FieldChrome::ReadOnly => FocusFigure::Hug,
    };
    node
}

#[cfg(test)]
mod tests {
    use super::valued;

    use super::{
        BORDER_STRONG, BORDER_SUBTLE, SUPPORT_ERROR, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
    };
    // Not from `super`: no shipped code in this file names the accent any
    // more, and that is the change. The invalid field used to bind it, which
    // made it identical to a focused field. It survives here only so the test
    // can assert the two differ.
    use super::{
        SIZE_FLUID, SIZE_LG, SIZE_MD, SIZE_SM, field, field_fluid, field_invalid, field_labeled,
        field_lg, field_readonly, field_sm, field_validated, hinted, labeled,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, SURFACE_BASE};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{FocusFigure, Interaction, NodeKind, Props, Registry, Role, ViewNode};

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

    /// Every field declares its focus figure; the host does not guess it
    /// from `Role::TextInput`. A bare well hugs itself; the fluid well is
    /// the hull its nested input shows focus on.
    #[test]
    fn every_field_declares_where_its_focus_is_shown() {
        for (label, node) in [
            ("default", field("f", "Name")),
            ("sm", field_sm("f", "Name")),
            ("lg", field_lg("f", "Name")),
            ("readonly", field_readonly("f", "Name")),
        ] {
            assert_eq!(
                node.semantics.focus_figure,
                FocusFigure::Hug,
                "{label}: a bare well is its own hull"
            );
        }
        for (label, node) in [
            ("labeled", field_labeled("f", "Name")),
            ("invalid", field_invalid("f", "Name", "required")),
        ] {
            assert_eq!(
                node.semantics.focus_figure,
                FocusFigure::Underline,
                "{label}: the wrapper is a label seat, not a well"
            );
            assert_eq!(
                child(&node, "input").semantics.focus_figure,
                FocusFigure::Hug,
                "{label}: the input inside a label seat hugs itself"
            );
        }
        let fluid = field_fluid("f", "Name");
        assert_eq!(fluid.semantics.focus_figure, FocusFigure::Hug);
        assert_eq!(
            child(&fluid, "input").semantics.focus_figure,
            FocusFigure::HugWell,
            "the fluid input shows its focus on the 64-tall well around it"
        );
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
        assert_carbon_well(&node, "default");
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Key));
        assert!(node.interactions.contains(&Interaction::TextEdit));
        assert!(!node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert!(node.children.is_empty(), "default field is a leaf Input");
    }

    /// Carbon's field is a fill with a bottom rule: `$field` under
    /// `border-block-end: 1px solid $border-strong`, square, and **no** edge
    /// on the other three sides. Every editable well in this file binds
    /// exactly that, so it is one assertion and not five copies.
    ///
    /// The negative half is the one that catches the defect coming back: a
    /// well that binds `border` or `radius` again is the box the operator
    /// called "not IBM Carbon style" twice.
    fn assert_carbon_well(node: &ViewNode, label: &str) {
        assert_eq!(
            token(node, "background"),
            Some(SURFACE_RAISED),
            "{label}: the well is filled"
        );
        assert_eq!(
            token(node, "border-bottom"),
            Some(BORDER_STRONG),
            "{label}: the well's one edge is a strong bottom rule"
        );
        assert_eq!(
            token(node, "border"),
            None,
            "{label}: a Carbon field is not boxed"
        );
        assert_eq!(
            token(node, "radius"),
            None,
            "{label}: a Carbon field has square corners"
        );
        for side in ["border-top", "border-left", "border-right"] {
            assert_eq!(token(node, side), None, "{label}: no {side}");
        }
    }

    #[test]
    fn field_sm_is_32_tall() {
        let node = field_sm("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_carbon_well(&node, "sm");
    }

    #[test]
    fn field_lg_is_48_tall() {
        let node = field_lg("name", "Fiber name");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_carbon_well(&node, "lg");
    }

    /// `labeled` puts Carbon's muted label over any control and stretches
    /// the control to the column, without taking the control's role.
    #[test]
    fn labeled_puts_a_muted_label_over_any_control_and_keeps_its_role() {
        let node = labeled("port", "Port", field_sm("input", "Port"));
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Vertical));
        assert!(node.semantics.role.is_none(), "the wrapper has no role");
        assert_eq!(
            node.props.align,
            Some(crate::geom::Align::Stretch),
            "the control fills the column the wrapper is given"
        );
        let label = child(&node, "label");
        assert_eq!(label.props.text.as_deref(), Some("Port"));
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let input = child(&node, "input");
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_SM));
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
        assert_carbon_well(&node, "fluid wrapper");
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
            token(input, "border").is_none() && token(input, "border-bottom").is_none(),
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
        assert_carbon_well(input, "labeled input");
    }

    /// `hinted` changes the hint in the empty well and nothing else.
    ///
    /// The trap it exists to avoid: a labelled field printed its own label
    /// twice, once above the well and once inside it, because `input_field`
    /// uses one string for both the placeholder and the accessible name.
    /// The obvious fix — pass the hint as the label — swaps the name too,
    /// so a screen reader would announce "fiber-7" for a field the page
    /// labels "Name". Both halves are asserted, and the second is the one
    /// that would go quietly wrong.
    #[test]
    fn hinted_changes_the_hint_and_never_the_name() {
        let plain = field("name", "Fiber name");
        let one = hinted(field("name", "Fiber name"), "fiber-7");
        assert_eq!(
            one.props.placeholder.as_deref(),
            Some("fiber-7"),
            "the empty well shows the hint"
        );
        assert_eq!(
            one.semantics.label.as_deref(),
            Some("Fiber name"),
            "the accessible name is the label, never the hint"
        );
        assert_eq!(one.kind, plain.kind);
        assert_eq!(one.constraints.vertical.min, plain.constraints.vertical.min);
        assert_eq!(one.interactions, plain.interactions);
        assert_eq!(one.props.tokens, plain.props.tokens, "same Carbon well");
        assert_carbon_well(&one, "hinted input");
    }

    /// It reaches through a wrapper, and it composes with `valued` either
    /// way round: a page writes `hinted(valued(..))` or `valued(hinted(..))`
    /// depending on which reads better and gets the same tree.
    #[test]
    fn hinted_reaches_the_input_inside_a_wrapper_and_composes_with_valued() {
        let wrapped = labeled("item", "Name", field("name", "Name"));
        let one = hinted(wrapped.clone(), "fiber-7");
        assert_eq!(
            child(&one, "name").props.placeholder.as_deref(),
            Some("fiber-7")
        );
        assert_eq!(
            child(&one, "label").props.text.as_deref(),
            Some("Name"),
            "the label above the well is untouched"
        );
        let a = valued(hinted(wrapped.clone(), "fiber-7"), "p9");
        let b = hinted(valued(wrapped, "p9"), "fiber-7");
        let leaf = |n: &ViewNode| {
            let c = child(n, "name");
            (c.props.placeholder.clone(), c.props.text.clone())
        };
        assert_eq!(leaf(&a), leaf(&b), "the two orders disagree");
        assert_eq!(leaf(&a), (Some("fiber-7".into()), Some("p9".into())));
    }

    #[test]
    fn hinted_on_a_tree_with_no_input_is_a_no_op() {
        let plain = super::stack(
            "row",
            Axis::Horizontal,
            None,
            vec![crate::component::text("t", "no well here")],
        );
        assert_eq!(hinted(plain.clone(), "fiber-7"), plain);
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
        assert_eq!(
            token(input, "border-bottom"),
            None,
            "the invalid outline replaces the resting rule rather than \
             stacking on it: Carbon's invalid field is one red box"
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
        // Carbon `--readonly`: transparent, `$border-subtle` rule. No fill
        // is the channel that says "not a well you can type into".
        assert_eq!(token(&node, "border-bottom"), Some(BORDER_SUBTLE));
        assert_eq!(token(&node, "border"), None);
        assert_eq!(
            token(&node, "background"),
            None,
            "a read-only field has no fill"
        );
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
    ///
    /// A read-only field has no fill of its own (Carbon `--readonly` is
    /// transparent), so its placeholder is judged against the base surface
    /// it shows through.
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
                    .map_or(SURFACE_BASE, TokenName::as_str);
                let well_bg = color(&theme, well_bg_name);
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
                    "{label}: at {ratio:.2}:1 against {well_bg_name} fails AA {MIN_TEXT_CONTRAST}:1"
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

    /// A validating field must not move its own well when the value becomes
    /// legal.
    ///
    /// The gallery's Port row branched between `field` and `field_invalid`,
    /// which place the well at `{key}` and at `{key}/input` respectively.
    /// The keystroke that made the value a number therefore re-keyed the
    /// node the operator's cursor was sitting in, and focus went with it.
    /// Both builds are one shape now, and this is the assertion that keeps
    /// them one shape.
    #[test]
    fn a_validating_field_keeps_its_well_at_the_same_key_either_way() {
        let bad = field_validated("port", "Port", Some("must be a number".to_owned()));
        let good = field_validated("port", "Port", None);

        for (node, what) in [(&bad, "invalid"), (&good, "valid")] {
            assert_eq!(node.key.as_str(), "port", "{what}: wrapper key");
            assert_eq!(
                node.children[0].key.as_str(),
                "input",
                "{what}: the well is the first child, at the same key"
            );
            assert_eq!(node.children[0].kind, NodeKind::Input, "{what}");
        }

        assert_eq!(bad.children.len(), 2, "the invalid build carries a helper");
        assert_eq!(good.children.len(), 1, "the valid build carries none");
        assert_eq!(
            token(&bad.children[0], "border"),
            Some(SUPPORT_ERROR),
            "and only the invalid build binds the error edge"
        );
        assert_eq!(good.children[0].props.tokens.get("border"), None);
    }
}
