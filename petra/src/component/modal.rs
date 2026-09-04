//! Carbon Modal (slice-c). Focus-trapping dialog, not a popover.
//!
//! Anatomy (`_modal.scss`):
//! 1. Dialog surface — [`Role::Dialog`], [`Layer::Modal`],
//!    [`InputPolicy::Block`] (the focus trap). Not [`Role::Overlay`].
//! 2. Header — the required `label` as visible title.
//! 3. Body — caller-supplied text.
//! 4. Close — a [`Role::Button`] labelled `"Close"`, never an icon-only
//!    mark (FR-026). Hit box 48 (Carbon `3rem`). The close icon itself is
//!    **20×20** (SCSS, T070); the style page's 16×16 is design intent.
//!
//! Fill is [`SURFACE_RAISED`]. There is no dimmed-page scrim as the only
//! channel: the dialog chrome (title, body, labelled Close) is the dialog.
//! A viewport-centred surface is not a popover; this module does not call
//! [`super::popover`].

use super::pad;
use super::stack;
use super::text::{heading, text};
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SHADOW_RAISED, SHAPE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, InputPolicy, Interaction, Key, Layer, NodeKind,
    Props, Role, Semantics, ViewNode,
};

/// Carbon close-button hit box (`3rem`).
const CLOSE_HIT: f32 = 48.0;
/// Carbon close icon, SCSS `convert.to-rem(20px)`. Style page says 16; T070
/// prefers SCSS. Recorded, not painted: the visible channel is the word
/// `"Close"`.
const CLOSE_ICON: f32 = 20.0;

const _: () = assert!(CLOSE_HIT == 48.0);
const _: () = assert!(CLOSE_ICON == 20.0);

const CLOSE_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled dialog. `label` is the accessible name and the visible title.
/// `body` is the message. Close is a labelled button, not an icon.
pub fn modal(key: impl Into<Key>, label: impl Into<String>, body: impl Into<String>) -> ViewNode {
    let label = label.into();
    let header = stack(
        "header",
        Axis::Horizontal,
        Some(SPACING_05),
        vec![heading("title", label.clone()), close_button()],
    );
    let mut content = stack(
        "content",
        Axis::Vertical,
        Some(SPACING_03),
        vec![header, text("body", body.into())],
    );
    content.props.align = Some(Align::Start);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Modal),
            anchor: Some(Anchor::Viewport),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::Block),
            padding: Some(pad(SPACING_05, SPACING_05)),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_MD));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    node.semantics = Semantics {
        role: Some(Role::Dialog),
        label: Some(label),
        ..Semantics::default()
    };
    node
}

fn close_button() -> ViewNode {
    let mut caption = text("label", "Close");
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack("close", Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    // Resting fill matches the dialog surface it sits on ([`SURFACE_RAISED`])
    // rather than binding no `background` at all: an unbound slot with only
    // `background@hover` beside it declares content the paint pass cannot
    // resolve at rest, which the accounting counts as silent
    // (`gorgon_petra_egui::paint::PaintReport::silent`).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(CLOSE_HIT),
            max: Some(CLOSE_HIT),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(CLOSE_HIT),
            max: Some(CLOSE_HIT),
            priority: 0,
        },
    })
    .interactive(Role::Button, "Close", CLOSE_INTENTS)
}

#[cfg(test)]
mod tests {
    use super::{CLOSE_HIT, CLOSE_ICON, modal};
    use crate::component::tokens::{LAYER_HOVER, SURFACE_RAISED};
    use crate::tree::{Anchor, InputPolicy, Interaction, Layer, NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|c| walk(c, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("missing descendant {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn modal_is_a_labelled_dialog_not_an_overlay() {
        let node = modal(
            "retire",
            "Retire fiber",
            "Its children are retired with it.",
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Dialog));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Retire fiber"));
        assert_eq!(node.props.layer, Some(Layer::Modal));
        assert_eq!(node.props.input_policy, Some(InputPolicy::Block));
        assert_eq!(node.props.anchor, Some(Anchor::Viewport));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert!(
            node.interactions.is_empty(),
            "the dialog is not a click target"
        );
    }

    #[test]
    fn modal_chrome_is_title_body_and_labelled_close() {
        let node = modal(
            "retire",
            "Retire fiber",
            "Its children are retired with it.",
        );
        let content = child(&node, "content");
        assert_eq!(
            descendant(content, "title").props.text.as_deref(),
            Some("Retire fiber")
        );
        assert_eq!(
            descendant(content, "body").props.text.as_deref(),
            Some("Its children are retired with it.")
        );
        let close = descendant(content, "close");
        assert_eq!(close.semantics.role, Some(Role::Button));
        assert_eq!(close.semantics.label.as_deref(), Some("Close"));
        assert_eq!(child(close, "label").props.text.as_deref(), Some("Close"));
        assert!(close.interactions.contains(&Interaction::Click));
        assert!(close.interactions.contains(&Interaction::Focus));
        assert_eq!(close.constraints.horizontal.min, Some(CLOSE_HIT));
        assert_eq!(close.constraints.vertical.min, Some(CLOSE_HIT));
        assert_eq!(CLOSE_HIT, 48.0);
        assert_eq!(CLOSE_ICON, 20.0, "T070: SCSS 20, not the style-page 16");
        assert!(
            close.props.text.is_none(),
            "Close is a labelled control, not an icon-only leaf"
        );
        assert_eq!(token(close, "background@hover"), Some(LAYER_HOVER));
        assert_eq!(
            token(close, "background"),
            Some(SURFACE_RAISED),
            "a resting `background` must be bound alongside `background@hover`, \
             or the close button paints nothing when it is not hovered — the \
             paint pass counts that as silent, not empty"
        );
    }
}
