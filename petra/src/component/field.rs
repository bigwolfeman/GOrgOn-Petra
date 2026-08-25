//! `field` — the one text-entry component.

use super::tokens::{BORDER_SUBTLE, SHAPE_SM, SURFACE_RAISED, TEXT_MUTED, TYPOGRAPHY_BODY, t};
use crate::tree::{Interaction, Key, NodeKind, Props, Role, ViewNode};

/// An editable text field.
///
/// `label` fills both the placeholder shown in the empty box and the
/// accessible name announced for it — a `field` has no separate label
/// element, so the one string an author supplies is both, and there is no
/// path that constructs a `field` with one but not the other.
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
/// A focus-coloured underline in the Material style is **not** available and
/// the reason is honest rather than aesthetic: this painter strokes all four
/// sides or none, there is no bottom-edge paint slot, and inventing one is a
/// change to `gorgon-petra-egui` that this pass did not make. The focus ring
/// already marks the focused field.
///
/// `NodeKind::Input` is a leaf kind, so unlike [`super::button`] it carries
/// no padding (`Props.padding` is refused on a leaf,
/// `crate::tree::validate::Violation::PaddingOnLeafKind`) — but a paint
/// slot is not a child, so the corner radius still applies directly to the
/// field's own rect.
pub fn field(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut props = Props {
        placeholder: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_RAISED));
    props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    props.tokens.insert("radius".into(), t(SHAPE_SM));
    ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(
            Role::TextInput,
            label,
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
        )
}
