//! `textarea` — a multiline field. The 43rd atomic (Q8, D-092).
//!
//! Auto-grow from [`SIZE_MD`] (40) up to a max height of
//! [`TEXTAREA_MAX_HEIGHT`] (eight md rows, 320). Wrap is on. The well is
//! [`bind_field_chrome`]: fill + border-bottom, square, not a four-sided box.
//!
//! This is not the Helix editor. It is not a `field()` parameter. A composer
//! that needs a real buffer belongs in Helix; a form that needs more than
//! one line of text belongs here.
//!
//! Focus geometry is host-owned; this file does not paint a ring. A
//! keystroke is text, not an intent.

use super::field::{EDITABLE_TEXT_INTENTS, SUPPORT_WARNING, bind_field_chrome, warning_helper};
use super::stack;
use super::text::text;
use super::tokens::{
    SIZE_MD, SPACING_02, SUPPORT_ERROR, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY,
    t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, FocusShownOn, Key, NodeKind, Props, Role, TextWrap,
    ViewNode,
};

/// Cap on auto-grow: eight Carbon md rows (8 × [`SIZE_MD`], 320).
///
/// Empty, the well is one [`SIZE_MD`] row. Wrapped content grows the well
/// until this cap; past it the well stops growing.
pub const TEXTAREA_MAX_HEIGHT: f32 = 8.0 * SIZE_MD;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(TEXTAREA_MAX_HEIGHT == 320.0);

/// How one textarea well is finished. Same three chromes [`super::field`]
/// spends on an enabled / invalid / warning Default input.
#[derive(Clone, Copy)]
enum Chrome {
    /// Editable well: a fill and a bottom rule.
    Enabled,
    /// Invalid: same fill, a [`SUPPORT_ERROR`] outline on all four sides.
    Invalid,
    /// Warning: same fill, a [`SUPPORT_WARNING`] outline on all four sides.
    Warning,
}

/// A multiline text field — Carbon Default, size md (40), auto-grow.
///
/// `label` fills both the placeholder shown in the empty well and the
/// accessible name announced for it (FR-058). There is no path that
/// constructs a textarea with one but not the other. Visible label-above
/// anatomy is [`super::field::labeled`] over this constructor, the same
/// wrapper every form control shares.
///
/// # The well: a fill and a bottom rule
///
/// Same chrome as [`super::field::field`]: [`bind_field_chrome`], `$field`
/// under a `1px solid $border-strong` bottom rule, square, nothing on the
/// other three sides. Not a four-sided box.
///
/// # Auto-grow, a max height
///
/// Wrap is [`TextWrap::Wrap`]. Vertical min is [`SIZE_MD`]; vertical max is
/// [`TEXTAREA_MAX_HEIGHT`]. That band is the auto-grow: the well starts one
/// md row tall and grows with wrapped content until the cap.
///
/// Intents are [`EDITABLE_TEXT_INTENTS`], the same list [`super::field::field`]
/// binds (Focus, Click, Key, TextEdit).
///
/// # Not the Helix editor
///
/// Q8 closed on D-092. A textarea is a form control. Helix is the editor.
#[must_use]
pub fn textarea(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    input_textarea(key, label, Chrome::Enabled)
}

/// Invalid textarea plus a label-adjacent helper.
///
/// Colour is not the only channel: the Input outline is [`SUPPORT_ERROR`]
/// on all four sides **and** a helper child carries `Invalid: {message}` in
/// [`TEXT_PRIMARY`]. Same two channels as [`super::field::field_invalid`].
#[must_use]
pub fn textarea_invalid(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    textarea_validated(key, label, Some(message.into()))
}

/// Warning textarea plus a label-adjacent helper.
///
/// Colour is not the only channel: the Input outline is [`SUPPORT_WARNING`]
/// on all four sides, a helper child carries `Warning: {message}` in
/// [`TEXT_PRIMARY`], and a [`super::icon::IconMark::WarningFilled`] glyph
/// sits beside that word. The well sits at `{key}/input`, the same depth as
/// [`textarea_invalid`], so warn to legal does not re-key the node under
/// the cursor.
#[must_use]
pub fn textarea_warning(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    let mut children = vec![input_textarea("input", label, Chrome::Warning)];
    children.push(warning_helper(message));
    let mut node = stack(key, Axis::Vertical, Some(SPACING_02), children);
    node.props.align = Some(Align::Stretch);
    node
}

/// A textarea that carries its own validity.
///
/// `None` builds the enabled well; `Some(message)` builds
/// [`textarea_invalid`]'s well plus its helper line.
///
/// **Both builds have the same shape**, so the well's id is `{key}/input`
/// either way. Branching between [`textarea`] and [`textarea_invalid`]
/// would re-key the node under the operator's cursor and drop his focus
/// mid-edit — the same reason [`super::field::field_validated`] exists.
#[must_use]
pub fn textarea_validated(
    key: impl Into<Key>,
    label: impl Into<String>,
    message: Option<String>,
) -> ViewNode {
    let chrome = if message.is_some() {
        Chrome::Invalid
    } else {
        Chrome::Enabled
    };
    let mut children = vec![input_textarea("input", label, chrome)];
    if let Some(message) = message {
        let mut helper = text("helper", format!("Invalid: {message}"));
        // The word carries the state as well as the hue does: the operator
        // is red-green colour blind and a red edge on its own is not a
        // channel he can read. Ink stays primary so the message is
        // legible; tinting it would trade a channel he has for one he
        // does not.
        helper
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_PRIMARY));
        children.push(helper);
    }
    let mut node = stack(key, Axis::Vertical, Some(SPACING_02), children);
    node.props.align = Some(Align::Stretch);
    node
}

fn input_textarea(key: impl Into<Key>, label: impl Into<String>, chrome: Chrome) -> ViewNode {
    let label = label.into();
    let mut props = Props {
        placeholder: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        wrap: Some(TextWrap::Wrap),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    match chrome {
        Chrome::Enabled => bind_field_chrome(&mut props),
        Chrome::Invalid => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            // The error hue, not the accent, and on all four sides: Carbon's
            // invalid field is an outline, the one state where the well is
            // boxed. Same token as [`super::field`]'s invalid well.
            props.tokens.insert("border".into(), t(SUPPORT_ERROR));
        }
        Chrome::Warning => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            // The warning hue, not the error and not the accent. Carbon's
            // warn well is the invalid outline in `$support-warning`.
            props.tokens.insert("border".into(), t(SUPPORT_WARNING));
        }
    }
    let mut node = ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(Role::TextInput, label, EDITABLE_TEXT_INTENTS)
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(SIZE_MD),
                max: Some(TEXTAREA_MAX_HEIGHT),
                priority: 0,
            },
            ..Constraints::default()
        });
    node.semantics.focus_figure = FocusFigure::Sides;
    node.semantics.focus_shown_on = FocusShownOn::Well;
    node
}

#[cfg(test)]
mod tests {
    use super::{
        EDITABLE_TEXT_INTENTS, SIZE_MD, SUPPORT_ERROR, SUPPORT_WARNING, SURFACE_RAISED,
        TEXT_PRIMARY, TEXTAREA_MAX_HEIGHT, textarea, textarea_invalid, textarea_validated,
        textarea_warning,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, BORDER_STRONG};
    use crate::component::{IconMark, IconTone, icon_toned};
    use crate::tree::{FocusFigure, FocusShownOn, Interaction, NodeKind, Role, TextWrap, ViewNode};

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

    /// Carbon's field is a fill with a bottom rule: `$field` under
    /// `border-block-end: 1px solid $border-strong`, square, and **no** edge
    /// on the other three sides.
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
    fn default_textarea_is_a_wrapping_size_md_input_capped_at_eight_rows() {
        let node = textarea("notes", "Notes");
        assert_eq!(node.kind, NodeKind::Input);
        assert_eq!(node.key.as_str(), "notes");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(TEXTAREA_MAX_HEIGHT));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(TEXTAREA_MAX_HEIGHT, 320.0);
        assert_eq!(node.props.wrap, Some(TextWrap::Wrap));
        assert_eq!(node.semantics.role, Some(Role::TextInput));
        assert_eq!(node.semantics.label.as_deref(), Some("Notes"));
        assert_eq!(node.props.placeholder.as_deref(), Some("Notes"));
        assert_carbon_well(&node, "default");
        assert_eq!(node.interactions.as_slice(), EDITABLE_TEXT_INTENTS);
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Key));
        assert!(node.interactions.contains(&Interaction::TextEdit));
        assert!(!node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert!(node.children.is_empty(), "default textarea is a leaf Input");
        assert_eq!(node.semantics.focus_figure, FocusFigure::Sides);
        assert_eq!(node.semantics.focus_shown_on, FocusShownOn::Well);
    }

    /// An invalid textarea's edge is the error hue, never the accent, and
    /// the helper text says so in words as well.
    #[test]
    fn textarea_invalid_draws_the_error_hue_and_says_so_in_words() {
        let node = textarea_invalid("notes", "Notes", "required");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(input.constraints.vertical.max, Some(TEXTAREA_MAX_HEIGHT));
        assert_eq!(input.props.wrap, Some(TextWrap::Wrap));
        assert_eq!(token(input, "border"), Some(SUPPORT_ERROR));
        assert_ne!(
            token(input, "border"),
            Some(ACCENT_PRIMARY),
            "an invalid well must not wear the accent: a focused well wears \
             it too, and the two states became pixel-identical"
        );
        assert_eq!(
            token(input, "border-bottom"),
            None,
            "the invalid outline replaces the resting rule rather than \
             stacking on it: Carbon's invalid field is one red box"
        );
        assert_eq!(token(input, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(input, "radius"),
            None,
            "an invalid well is boxed, but still square"
        );
        assert_eq!(input.interactions.as_slice(), EDITABLE_TEXT_INTENTS);
        let helper = child(&node, "helper");
        assert_eq!(helper.kind, NodeKind::Text);
        assert_eq!(helper.props.text.as_deref(), Some("Invalid: required"));
        assert_eq!(token(helper, "foreground"), Some(TEXT_PRIMARY));
    }

    /// A warning textarea's edge is the warning hue, never the error and
    /// never the accent, and the helper says so in words and in a glyph.
    #[test]
    fn textarea_warning_draws_the_warning_hue_and_says_so_in_words_and_a_glyph() {
        let node = textarea_warning("notes", "Notes", "check the value");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(input.constraints.vertical.max, Some(TEXTAREA_MAX_HEIGHT));
        assert_eq!(input.props.wrap, Some(TextWrap::Wrap));
        assert_eq!(token(input, "border"), Some(SUPPORT_WARNING));
        assert_ne!(
            token(input, "border"),
            Some(SUPPORT_ERROR),
            "a warning well must not wear the error outline"
        );
        assert_ne!(
            token(input, "border"),
            Some(ACCENT_PRIMARY),
            "a warning well must not wear the accent: a focused well wears \
             it too"
        );
        assert_eq!(
            token(input, "border-bottom"),
            None,
            "the warning outline replaces the resting rule rather than \
             stacking on it"
        );
        assert_eq!(token(input, "background"), Some(SURFACE_RAISED));
        assert_eq!(input.interactions.as_slice(), EDITABLE_TEXT_INTENTS);
        let helper = child(&node, "helper");
        let message = child(helper, "message");
        assert_eq!(message.kind, NodeKind::Text);
        assert!(
            message
                .props
                .text
                .as_deref()
                .is_some_and(|t| t.contains("Warning:")),
            "the helper text must contain `Warning:`"
        );
        assert_eq!(
            message.props.text.as_deref(),
            Some("Warning: check the value")
        );
        assert_eq!(token(message, "foreground"), Some(TEXT_PRIMARY));
        let mark = child(helper, "mark");
        let expected = icon_toned("mark", IconMark::WarningFilled, IconTone::Primary);
        assert_eq!(mark.kind, NodeKind::Canvas);
        assert_eq!(
            mark.props.canvas, expected.props.canvas,
            "the helper's glyph is WarningFilled, not ErrorFilled and not a \
             swatch"
        );
    }

    /// Enabled and invalid validated builds place the well at the same id,
    /// so a validity flip does not re-key the node under the cursor.
    #[test]
    fn textarea_validated_keeps_the_well_at_the_same_depth() {
        let ok = textarea_validated("notes", "Notes", None);
        let bad = textarea_validated("notes", "Notes", Some("required".into()));
        assert_eq!(child(&ok, "input").kind, NodeKind::Input);
        assert_eq!(child(&bad, "input").kind, NodeKind::Input);
        assert_eq!(child(&ok, "input").key.as_str(), "input");
        assert_eq!(child(&bad, "input").key.as_str(), "input");
        assert_carbon_well(child(&ok, "input"), "validated enabled");
        assert_eq!(ok.children.len(), 1, "enabled has no helper");
        assert_eq!(bad.children.len(), 2, "invalid adds the helper");
    }
}
