//! Drawer and sheet — one overlay, parameterised by [`InputPolicy`].
//!
//! Spec 009 T017. MynaUI's two pages describe the same edge-docked panel
//! and the researcher could not separate them. What differs is the press
//! past the panel, and that parameter already ships:
//!
//! * [`drawer`] — [`InputPolicy::Block`]. Outside is swallowed and the hit
//!   test stops at this surface (`contracts/interaction-state.md` §10),
//!   the same two rules [`super::modal`] spends on a dialog. The page
//!   behind is inert.
//! * [`sheet`] — [`InputPolicy::DismissOutside`]. Same panel, outside
//!   press closes, the way [`super::popover`] does.
//!
//! [`docked`] is the one constructor. The two names pick a policy.
//!
//! Open-or-shut is a `bool` the owning fiber passes, not a fiber of its
//! own (`contracts/view-fiber.md` §4.5). A closed panel is a zero-size
//! spacer under the same key, never a full-width ghost Surface.
//!
//! # Anatomy (open)
//!
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Modal`], docked with
//!    [`Anchor::ViewportEdge`] (inside the named window edge, flush).
//!    [`Fit::Anchor`] spans that edge. [`Tip::Flush`]: a window edge has
//!    nothing to point a caret at. [`ClampRule::Shrink`].
//! 2. Fill [`SURFACE_RAISED`], elevation [`SHADOW_OVERLAY`]. **No
//!    border.** Design language: no four-sided border on a sheet.
//! 3. Radius [`CornerRole::Floating`] via [`corner_for`]. All four
//!    corners share that token. A docked edge wants radius 0 and the free
//!    edge wants Floating; per-corner radius is T001 and is blocked on
//!    the painter. Documented, not papered over.
//! 4. Body — the caller-supplied child, padded [`SPACING_05`].
//!
//! # Why there is no covering scrim
//!
//! [`super::modal`]'s scrim *is* the Surface: [`Anchor::Viewport`], a
//! minimum no window can satisfy, painted `overlay.scrim`. That box
//! covers the window. A [`Anchor::ViewportEdge`] surface is the panel
//! itself, sized along one edge. It cannot also be the window. Block
//! still swallows a press past the panel (rule 1 of §10) without a dim
//! on the page. A dimming scrim on a docked panel is a second Surface
//! and is not this constructor.
//!
//! # Closed
//!
//! A [`NodeKind::Spacer`] keyed the same, pinned to 0×0. Not a Surface,
//! so layout harvests no overlay and the spacer takes no flow room.

use super::tokens::{SHADOW_OVERLAY, SPACING_05, SURFACE_RAISED, t};
use super::{pad, stack};
use crate::geom::{Align as CrossAlign, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, Fit, InputPolicy, Key, Layer,
    NodeKind, Props, Role, Semantics, Tip, ViewNode,
};

/// Carbon `mini-units(32)`: the side-nav / header-panel width, and the
/// shorter-edge hint [`corner_for`] gets so Floating does not promote to
/// a pill. A left or right panel is at least this wide; a top or bottom
/// panel is as tall as its body.
const PANEL_CROSS: f32 = 256.0;

const _: () = assert!(PANEL_CROSS == 256.0);

/// A blocking drawer: the page behind is inert. `edge` is the window
/// edge it docks inside. `open` is a field on the fiber that opened it.
#[must_use]
pub fn drawer(key: impl Into<Key>, edge: Edge, open: bool, body: ViewNode) -> ViewNode {
    docked(key, edge, open, body, InputPolicy::Block)
}

/// The same panel as [`drawer`], dismissed by a press outside it.
#[must_use]
pub fn sheet(key: impl Into<Key>, edge: Edge, open: bool, body: ViewNode) -> ViewNode {
    docked(key, edge, open, body, InputPolicy::DismissOutside)
}

/// Drawer and sheet as one overlay. [`drawer`] is [`InputPolicy::Block`];
/// [`sheet`] is [`InputPolicy::DismissOutside`]. [`InputPolicy::Passthrough`]
/// is legal (the panel floats and lets the page through) and is the
/// caller's to pick; it is not a third named constructor.
#[must_use]
pub fn docked(
    key: impl Into<Key>,
    edge: Edge,
    open: bool,
    body: ViewNode,
    policy: InputPolicy,
) -> ViewNode {
    let key = key.into();
    if !open {
        return closed(key);
    }

    let mut content = stack("content", Axis::Vertical, None, vec![body]);
    content.props.align = Some(CrossAlign::Stretch);

    let (role, label, takes_focus) = match policy {
        InputPolicy::Block => (Role::Dialog, "Drawer", true),
        InputPolicy::DismissOutside => (Role::Overlay, "Sheet", false),
        InputPolicy::Passthrough => (Role::Overlay, "Panel", false),
    };
    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Modal),
            anchor: Some(Anchor::ViewportEdge {
                edge,
                align: Align::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(policy),
            fit: Some(Fit::Anchor),
            tip: Some(Tip::Flush),
            takes_focus: Some(takes_focus),
            padding: Some(pad(SPACING_05, SPACING_05)),
            ..Props::default()
        })
        .with_constraints(cross_axis(edge))
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    // All four corners share Floating. The docked-edge zero is T001: the
    // painter has one radius slot, and `corner_for` returns one name.
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(CornerRole::Floating, PANEL_CROSS)),
    );
    node.semantics = Semantics {
        role: Some(role),
        label: Some(label.into()),
        expanded: Some(true),
        ..Semantics::default()
    };
    node
}

/// A closed panel: same key, no overlay, no flow room.
fn closed(key: Key) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Spacer, key).with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(0.0),
            max: Some(0.0),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(0.0),
            max: Some(0.0),
            priority: 0,
        },
    });
    node.semantics.expanded = Some(false);
    node
}

/// Min extent *away* from the docked edge. Left/right pin width;
/// top/bottom let the body decide height. The long edge is [`Fit::Anchor`].
fn cross_axis(edge: Edge) -> Constraints {
    match edge {
        Edge::Left | Edge::Right => Constraints {
            horizontal: AxisConstraint {
                min: Some(PANEL_CROSS),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        },
        Edge::Top | Edge::Bottom => Constraints::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{PANEL_CROSS, docked, drawer, sheet};
    use crate::component::text::text;
    use crate::component::tokens::{SHADOW_OVERLAY, SHAPE_MD, SPACING_05, SURFACE_RAISED};
    use crate::token::{CornerRole, corner_for};
    use crate::tree::{
        Align, Anchor, ClampRule, Edge, Fit, InputPolicy, Layer, NodeKind, Role, Tip, ViewNode,
    };

    fn body() -> ViewNode {
        text("copy", "Filters")
    }

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

    fn dock_of(node: &ViewNode) -> (Edge, Align) {
        match &node.props.anchor {
            Some(Anchor::ViewportEdge {
                edge,
                align,
                offset,
            }) => {
                assert_eq!(offset.as_ref(), None, "a drawer is flush with the window");
                (*edge, *align)
            }
            other => panic!("expected Anchor::ViewportEdge, got {other:?}"),
        }
    }

    #[test]
    fn drawer_is_a_blocking_viewport_edge_dialog() {
        let node = drawer("nav", Edge::Left, true, body());
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Dialog));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Drawer"));
        assert_eq!(node.semantics.expanded, Some(true));
        assert!(node.interactions.is_empty());
        assert_eq!(node.props.layer, Some(Layer::Modal));
        assert_eq!(node.props.input_policy, Some(InputPolicy::Block));
        assert_eq!(node.props.takes_focus, Some(true));
        assert_eq!(node.props.clamp, Some(ClampRule::Shrink));
        assert_eq!(node.props.fit, Some(Fit::Anchor));
        assert_eq!(node.props.tip, Some(Tip::Flush));
        assert_eq!(dock_of(&node), (Edge::Left, Align::Start));
        assert_eq!(node.constraints.horizontal.min, Some(PANEL_CROSS));
        assert_eq!(PANEL_CROSS, 256.0);
    }

    #[test]
    fn sheet_is_the_same_panel_with_dismiss_outside() {
        let node = sheet("help", Edge::Bottom, true, body());
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Sheet"));
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        assert_eq!(node.props.takes_focus, Some(false));
        assert_eq!(node.props.layer, Some(Layer::Modal));
        assert_eq!(dock_of(&node), (Edge::Bottom, Align::Start));
        assert_eq!(
            node.constraints.horizontal.min, None,
            "a bottom sheet is as wide as the window (Fit::Anchor), not a pinned width"
        );
    }

    #[test]
    fn drawer_and_sheet_are_docked_with_a_policy() {
        assert_eq!(
            drawer("nav", Edge::Left, true, body()),
            docked("nav", Edge::Left, true, body(), InputPolicy::Block)
        );
        assert_eq!(
            sheet("help", Edge::Bottom, true, body()),
            docked(
                "help",
                Edge::Bottom,
                true,
                body(),
                InputPolicy::DismissOutside
            )
        );
        let pass = docked("c", Edge::Right, true, body(), InputPolicy::Passthrough);
        assert_eq!(pass.props.input_policy, Some(InputPolicy::Passthrough));
        assert_eq!(pass.semantics.role, Some(Role::Overlay));
        assert_eq!(pass.semantics.label.as_deref(), Some("Panel"));
        assert_eq!(pass.props.takes_focus, Some(false));
    }

    #[test]
    fn chrome_is_raised_overlay_with_no_border_and_floating_radius() {
        let node = drawer("nav", Edge::Left, true, body());
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "shadow"), Some(SHADOW_OVERLAY));
        assert_eq!(
            token(&node, "border"),
            None,
            "no four-sided border on a sheet"
        );
        assert_eq!(
            token(&node, "radius"),
            Some(SHAPE_MD),
            "Floating at PANEL_CROSS is shape.corner-md; all four corners share it"
        );
        assert_eq!(
            token(&node, "radius"),
            Some(corner_for(CornerRole::Floating, PANEL_CROSS)),
        );
        assert_eq!(padding_token(&node), Some((SPACING_05, SPACING_05)));
        assert_eq!(
            child(&child(&node, "content"), "copy")
                .props
                .text
                .as_deref(),
            Some("Filters")
        );
    }

    #[test]
    fn closed_is_a_zero_size_spacer_not_a_ghost_surface() {
        let node = drawer("nav", Edge::Left, false, body());
        assert_eq!(node.kind, NodeKind::Spacer);
        assert_ne!(node.kind, NodeKind::Surface);
        assert_eq!(node.key.as_str(), "nav");
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(node.props.anchor.is_none());
        assert!(node.props.input_policy.is_none());
        assert!(node.children.is_empty(), "a closed drawer mounts no body");
        assert_eq!(node.constraints.horizontal.min, Some(0.0));
        assert_eq!(node.constraints.horizontal.max, Some(0.0));
        assert_eq!(node.constraints.vertical.min, Some(0.0));
        assert_eq!(node.constraints.vertical.max, Some(0.0));
        // Closed drops edge and policy: the spacer is the key, nothing else.
        assert_eq!(
            node,
            sheet("nav", Edge::Bottom, false, body()),
            "closed form does not depend on edge or policy"
        );
        assert_eq!(
            node,
            docked("nav", Edge::Top, false, body(), InputPolicy::Passthrough)
        );
    }

    #[test]
    fn every_edge_docks_inside_that_window_edge() {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let node = drawer("d", edge, true, body());
            assert_eq!(dock_of(&node).0, edge, "{edge:?}");
        }
    }
}
