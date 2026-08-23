//! `text` and `heading` — the two non-interactive type components.

use super::tokens::{TEXT_PRIMARY, TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING, t};
use crate::tree::{Key, NodeKind, Props, ViewNode};

/// A run of body text.
///
/// Carries no role and no interactions: `crate::layout::default_role` gives
/// a plain text node none, and a node with no actions is outside
/// `ActionableNeedsRoleAndLabel`'s reach, so FR-058 has nothing to say about
/// it — the obligation is about what an *interactive* component cannot
/// omit, and `text` is not one.
pub fn text(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    let mut props = Props {
        text: Some(content.into()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    ViewNode::new(NodeKind::Text, key).with_props(props)
}

/// A section or page title.
///
/// `text`'s sibling in every way but one: the typography step it binds is
/// [`TYPOGRAPHY_HEADING`] instead of [`TYPOGRAPHY_BODY`]. Kept as two
/// functions rather than one with a `size` argument, so an author reaching
/// for "the big text" and "the normal text" finds two names rather than a
/// style value to pick — which is the whole point of a component library
/// over the primitives (FR-052).
pub fn heading(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    let mut props = Props {
        text: Some(content.into()),
        style: Some(t(TYPOGRAPHY_HEADING)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    ViewNode::new(NodeKind::Text, key).with_props(props)
}
