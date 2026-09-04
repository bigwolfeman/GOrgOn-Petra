//! Carbon Tooltip (slice-f). Hover-triggered, non-interactive bubble.
//!
//! Anatomy (`_tooltip.scss`): caret + content box. The trigger is **not**
//! this constructor — it lives on the host control. This node is the
//! bubble only.
//!
//! Distinction from [`super::toggletip`]: a tooltip discloses on hover or
//! focus and MUST NOT contain interactive elements. A toggletip discloses
//! on click and is the place interactive contents go. FR-058 therefore
//! does not apply here: this node has no interactions, so it is outside
//! `ActionableNeedsRoleAndLabel`. [`Role::Overlay`] still gets
//! `Semantics.label` set to the body so AT has a name.
//!
//! Default/multi-line max-inline is 288 (SCSS). Single-line 208 is
//! style-page only and is not bound.

use super::popover::popover;
use crate::tree::{Key, Role, ViewNode};

/// Carbon default tooltip `max-inline-size`.
const MAX_INLINE: f32 = 288.0;
/// Style-page single-line max-width (design intent, not the SCSS target).
const SINGLE_LINE_INTENT: f32 = 208.0;

const _: () = assert!(MAX_INLINE == 288.0);
const _: () = assert!(SINGLE_LINE_INTENT == 208.0);

/// The tooltip bubble. `label` is accepted so a caller names the overlay;
/// `Semantics.label` is the `body`, which is the text AT should announce.
/// Anchored to a sibling keyed `"trigger"`.
pub fn tooltip(key: impl Into<Key>, label: impl Into<String>, body: impl Into<String>) -> ViewNode {
    let _label = label.into();
    let body = body.into();
    let mut node = popover(key, body.clone(), "trigger", body.clone());
    node.constraints.horizontal.max = Some(MAX_INLINE);
    node.semantics.role = Some(Role::Overlay);
    node.semantics.label = Some(body);
    debug_assert!(
        node.interactions.is_empty(),
        "a tooltip bubble is not a control"
    );
    node
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, SINGLE_LINE_INTENT, tooltip};
    use crate::tree::{Anchor, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn tooltip_is_a_non_interactive_overlay_bubble() {
        let node = tooltip("copied", "Copied", "Copied to clipboard");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(
            node.semantics.label.as_deref(),
            Some("Copied to clipboard"),
            "AT name is the body"
        );
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 288.0);
        assert_eq!(SINGLE_LINE_INTENT, 208.0);
        match &node.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "trigger"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(&node, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Copied to clipboard")
        );
        assert!(
            child(content, "body").interactions.is_empty(),
            "the bubble has no interactive children"
        );
    }
}
