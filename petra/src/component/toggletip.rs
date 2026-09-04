//! Carbon Toggletip (slice-f). Click-triggered, interactive disclosure.
//!
//! Anatomy (`_toggletip.scss`):
//! 1. Trigger — a labelled [`Role::Button`]. Carbon often ships an info
//!    icon here; the label is required so the trigger is never icon-only
//!    (FR-026, FR-058).
//! 2. Caret + container — [`super::popover::popover_with`] when `open`.
//!    Interactive children belong in the popover (that is why this is
//!    not [`super::tooltip`]).
//!
//! Container max-inline is 288 (`18rem`, SCSS). Open/closed are the only
//! documented states.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, Role, ViewNode};

/// Carbon toggletip content `max-inline-size` (`18rem`).
const MAX_INLINE: f32 = 288.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MAX_INLINE == 288.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled trigger that, when `open`, hosts a popover of `body`.
///
/// The trigger is keyed `"trigger"`; the popover is keyed `"tip"` and
/// anchored to `"trigger"`.
pub fn toggletip(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger_button("trigger", label.clone());
    let mut children = vec![trigger];
    if open {
        let mut tip = popover_with("tip", label, "trigger", vec![text("body", body.into())]);
        tip.constraints.horizontal.max = Some(MAX_INLINE);
        children.push(tip);
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn trigger_button(key: impl Into<Key>, label: String) -> ViewNode {
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, TRIGGER_INTENTS)
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, toggletip};
    use crate::tree::{Anchor, Interaction, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn toggletip_closed_is_a_labelled_trigger() {
        let node = toggletip("help", "About filters", false, "Narrow the list.");
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1);
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("About filters"));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert!(trigger.interactions.contains(&Interaction::Focus));
        assert_eq!(
            child(trigger, "label").props.text.as_deref(),
            Some("About filters")
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    #[test]
    fn toggletip_open_hosts_an_interactive_popover() {
        let node = toggletip("help", "About filters", true, "Narrow the list.");
        assert_eq!(node.semantics.expanded, Some(true));
        let tip = child(&node, "tip");
        assert_eq!(tip.kind, NodeKind::Surface);
        assert_eq!(tip.semantics.role, Some(Role::Overlay));
        assert_eq!(tip.semantics.label.as_deref(), Some("About filters"));
        assert_eq!(tip.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 288.0);
        match &tip.props.anchor {
            Some(Anchor::Node { id, .. }) => assert_eq!(id, "trigger"),
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        let content = child(tip, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Narrow the list.")
        );
    }
}
