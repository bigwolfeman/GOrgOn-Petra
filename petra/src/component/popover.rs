//! Carbon Popover (slice-d). Positioning primitive other overlays compose.
//!
//! Anatomy (`_popover.scss` + usage page):
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Popup`], anchored to a
//!    trigger node. Carbon's popover is not a modal, so this is
//!    [`Role::Overlay`] (not [`Role::Dialog`]) and
//!    [`InputPolicy::DismissOutside`] (not a focus trap).
//! 2. Content box — fill [`SURFACE_RAISED`], edge [`BORDER_SUBTLE`],
//!    padding [`SPACING_05`]. Grows to content, capped at 368 (T070).
//! 3. Caret — the word `"^"`. A second channel beside the fill, never an
//!    icon-only mark (FR-026). [`IconMark`] has no caret; a triangle canvas
//!    would duplicate the engine caret `overlay_surface` already paints from
//!    the resolved side.
//!
//! [`ClampRule::Flip`] is the `--auto-align` ladder. Interactive contents
//! are the caller's children (FR-058 on those children, not on this node).

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{BORDER_SUBTLE, SPACING_03, SPACING_05, SURFACE_RAISED, t};
use crate::geom::Axis;
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, InputPolicy, Key, Layer, NodeKind,
    Props, Role, Semantics, ViewNode,
};

/// Carbon `.cds--popover-content` `max-inline-size`. T070 prefers SCSS
/// (`_popover.scss:217`) over the style-page 352.
const MAX_INLINE: f32 = 368.0;

const _: () = assert!(MAX_INLINE == 368.0);

/// An anchored popover whose body is one text run.
///
/// `label` is the accessible name (required). `anchor_id` is the semantic
/// id of the trigger node [`Anchor::Node`] names. The popover itself is
/// not a click target.
pub fn popover(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    popover_with(key, label, anchor_id, vec![text("body", body.into())])
}

/// An anchored popover whose body is caller-supplied children.
///
/// Same chrome as [`popover`]: raised fill, subtle border, `"^"` caret,
/// [`Role::Overlay`]. The children keep whatever roles and labels they
/// already carry.
pub fn popover_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    children: Vec<ViewNode>,
) -> ViewNode {
    let mut rows = Vec::with_capacity(children.len() + 1);
    rows.push(text("caret", "^"));
    rows.extend(children);
    let mut content = stack("content", Axis::Vertical, Some(SPACING_03), rows);
    content.props.align = Some(crate::geom::Align::Center);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Node {
                id: anchor_id.into(),
                edge: Edge::Bottom,
                align: Align::Center,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            padding: Some(pad(SPACING_05, SPACING_05)),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.constraints = Constraints {
        horizontal: AxisConstraint {
            min: None,
            max: Some(MAX_INLINE),
            priority: 0,
        },
        ..Constraints::default()
    };
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, popover, popover_with};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, SPACING_05, SURFACE_RAISED};
    use crate::tree::{
        Align, Anchor, ClampRule, Edge, InputPolicy, Layer, NodeKind, Role, ViewNode,
    };

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

    fn padding_token(node: &ViewNode) -> Option<(&str, &str)> {
        let pad = node.props.padding.as_ref()?;
        Some((pad.left.as_ref()?.as_str(), pad.top.as_ref()?.as_str()))
    }

    #[test]
    fn popover_is_an_anchored_overlay_not_a_dialog() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Filter help"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(node.props.clamp, Some(ClampRule::Flip));
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        match &node.props.anchor {
            Some(Anchor::Node {
                id,
                edge,
                align,
                offset,
            }) => {
                assert_eq!(id, "filter-btn");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(*align, Align::Center);
                assert!(offset.is_none());
            }
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(padding_token(&node), Some((SPACING_05, SPACING_05)));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 368.0);
    }

    #[test]
    fn popover_caret_is_a_second_channel_not_icon_only() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
        let content = child(&node, "content");
        let caret = child(content, "caret");
        assert_eq!(caret.props.text.as_deref(), Some("^"));
        assert!(
            caret.semantics.role.is_none(),
            "caret is a visual part, not a control"
        );
        assert!(caret.interactions.is_empty());
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Narrow the list.")
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
    }

    #[test]
    fn popover_with_hosts_caller_children() {
        let node = popover_with("menu", "Actions", "more-btn", vec![text("item", "Rename")]);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.props.anchor.is_some());
        assert!(node.interactions.is_empty());
        let content = child(&node, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(child(content, "item").props.text.as_deref(), Some("Rename"));
        assert!(
            child(content, "item").semantics.role.is_none(),
            "FR-058 stays on the caller's children; this node does not wrap them"
        );
    }
}
