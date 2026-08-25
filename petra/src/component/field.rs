//! `field` — the one text-entry component.

use super::tokens::{SHAPE_SM, SURFACE_RAISED, TEXT_MUTED, TYPOGRAPHY_BODY, t};
use crate::tree::{Interaction, Key, NodeKind, Props, Role, ViewNode};

/// An editable text field.
///
/// `label` fills both the placeholder shown in the empty box and the
/// accessible name announced for it — a `field` has no separate label
/// element, so the one string an author supplies is both, and there is no
/// path that constructs a `field` with one but not the other.
///
/// # The box, and why it stopped being an outline
///
/// A field is the control on a page that most needs to say "you can type
/// here", so this was the last border to go. It drew a `text.muted` box —
/// the same tone as the placeholder text inside it, which is the specific
/// way an outlined field reads badly: the frame and the content are the same
/// weight, and the frame is longer.
///
/// What replaces it is the fill. [`super::on_layer`] seats a field one step
/// ahead of whatever it is placed on, so it reads as a well cut into the
/// card rather than a rectangle drawn on top of one. That is Material's
/// *filled* text field minus its underline, and the underline is missing for
/// an honest reason rather than a design one: this painter draws a stroke on
/// all four sides or none, there is no bottom-edge paint slot, and inventing
/// one to carry a focus colour is a change to `gorgon-petra-egui` that this
/// pass did not make. The focus ring already marks the focused field.
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
    props.tokens.insert("radius".into(), t(SHAPE_SM));
    ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(
            Role::TextInput,
            label,
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
        )
}
