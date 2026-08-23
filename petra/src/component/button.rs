//! `button` — the one activatable label component.

use super::pad;
use super::tokens::{
    SHAPE_MD, SPACING_MD, SPACING_SM, SURFACE_RAISED, TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, NodeKind, Props, Role, ViewNode};

/// A focusable, clickable control with a label.
///
/// `label` is a required positional parameter, not an `Option<String>` and
/// not a setter a caller could skip calling — there is no `button(key)`
/// that compiles. The role and interactions are set inside this function,
/// not left to [`crate::tree::ViewNode::interactive`] being called
/// separately, so an author cannot construct a `button` node and forget the
/// step that makes it accessible: FR-058 is a fact about this function's
/// signature, not a convention its caller is trusted to follow.
///
/// A `button` is a padded, rounded `Stack` around a `text` label rather
/// than a bare styled `Text` leaf, because `Props.padding` only applies to
/// container kinds (`crate::tree::validate::Violation::PaddingOnLeafKind`)
/// — a `Text` node has no children to inset. The label lives in the child;
/// the role, the interactions, and the chrome live on the wrapper.
pub fn button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut label_props = Props {
        text: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    label_props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let inner =
        ViewNode::new(NodeKind::Text, format!("{}-label", key.as_str())).with_props(label_props);

    let mut props = Props {
        axis: Some(Axis::Horizontal),
        padding: Some(pad(SPACING_MD, SPACING_SM)),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_RAISED));
    props.tokens.insert("radius".into(), t(SHAPE_MD));

    ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .child(inner)
        .interactive(
            Role::Button,
            label,
            &[Interaction::Focus, Interaction::Click],
        )
}
