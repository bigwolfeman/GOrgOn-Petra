//! Menubar: a horizontal strip of top-level menu labels, docked to a
//! window edge.
//!
//! Spec 009 T016. Shared row anatomy is [`menu_item`]. What differs from
//! [`super::menu`] and a context menu is the anchor, not the row.
//! A dropdown hangs off a sibling trigger ([`crate::tree::Anchor::Sibling`]).
//! A context menu sits at a point ([`crate::tree::Anchor::Point`]). A
//! menubar docks *inside* a window edge ([`crate::tree::Anchor::ViewportEdge`]):
//! a node anchor puts a surface outside the named edge, a viewport edge
//! puts it inside, because outside the window is nowhere
//! (`tree/props.rs`, `notification.rs`).
//!
//! [`Fit::Anchor`] is the other half of the dock. Without it the bar hugs
//! its labels and the page shows through beside it. That is the defect
//! the demo's status bar hit. With it the bar is as broad as the edge it
//! docks to (width on [`Edge::Top`] / [`Edge::Bottom`], height on
//! [`Edge::Left`] / [`Edge::Right`]). [`crate::tree::Align::Start`] is
//! the origin along that edge. Viewport-edge align is Start, Center, or
//! End. There is no Stretch on that enum. The stretch is the fit.
//!
//! Open menus are not this constructor. `items` are the top-level labels
//! (typically [`menu_item`]), a row of children. The fiber that owns the
//! surface under the bar records which label is open and mounts a
//! [`super::menu`] beside that label on the next `view`
//! (`contracts/view-fiber.md` §4.5).
//!
//! # Anatomy
//!
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Popup`], docked with
//!    [`Anchor::ViewportEdge`] `{ edge, align: Start, offset: None }`.
//!    [`Fit::Anchor`] spans the edge. [`Tip::Flush`]: a window edge has
//!    nothing to point a caret at. [`ClampRule::Shrink`] keeps the docked
//!    edge. [`InputPolicy::Passthrough`]: the bar is not dismissed and
//!    does not trap the page. It does not take focus; open menus are a
//!    different constructor.
//! 2. Fill [`SURFACE_RAISED`]. **No four-sided border.** No radius: a
//!    window-edge strip is square. No overlay shadow: the surface step
//!    is the separation, and a full-bleed dock is not a floating card.
//! 3. Content — one horizontal `Stack` keyed `content`, children
//!    stretched on the cross axis, no gap. Callers pass the labels.

use super::stack;
use super::tokens::{SURFACE_RAISED, t};
use crate::geom::{Align as CrossAlign, Axis};
use crate::tree::{
    Align, Anchor, ClampRule, Edge, Fit, InputPolicy, Key, Layer, NodeKind, Props, Role, Semantics,
    Tip, ViewNode,
};

/// A horizontal bar of top-level menu labels, docked inside `edge`.
///
/// `label` is the accessible name of the bar (FR-058). `items` are the
/// labels themselves, typically [`menu_item`]; this constructor does not
/// inspect them and does not wrap an open [`super::menu`] around them.
///
/// [`Fit::Anchor`] makes the bar as broad as the edge it docks to. The
/// engine resolves that after placement, from the window's own extent
/// along the edge, because a constructor cannot know the window's width.
/// [`Fit::Content`] (the default) is the hugging-width defect: a docked
/// bar sitting at its labels' width with the page showing through beside
/// it. Offset is none: flush with the inside of the window.
#[must_use]
pub fn menubar(
    key: impl Into<Key>,
    label: impl Into<String>,
    edge: Edge,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut content = stack("content", Axis::Horizontal, None, items);
    content.props.align = Some(CrossAlign::Stretch);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::ViewportEdge {
                edge,
                align: Align::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::Passthrough),
            fit: Some(Fit::Anchor),
            tip: Some(Tip::Flush),
            takes_focus: Some(false),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.semantics = Semantics {
        role: Some(Role::List),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

/// [`menubar`] docked to [`Edge::Top`].
#[must_use]
pub fn menubar_top(
    key: impl Into<Key>,
    label: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    menubar(key, label, Edge::Top, items)
}

#[cfg(test)]
mod tests {
    use super::super::list_box::menu_item;
    use super::{menubar, menubar_top};
    use crate::component::tokens::SURFACE_RAISED;
    use crate::tree::{
        Align, Anchor, ClampRule, Edge, Fit, InputPolicy, Layer, NodeKind, Role, Tip, ViewNode,
    };

    fn items() -> Vec<ViewNode> {
        vec![
            menu_item("file", "File"),
            menu_item("edit", "Edit"),
            menu_item("view", "View"),
        ]
    }

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn dock_of(node: &ViewNode) -> (Edge, Align) {
        match &node.props.anchor {
            Some(Anchor::ViewportEdge {
                edge,
                align,
                offset,
            }) => {
                assert_eq!(offset.as_ref(), None, "a menubar is flush with the window");
                (*edge, *align)
            }
            other => panic!("expected Anchor::ViewportEdge, got {other:?}"),
        }
    }

    #[test]
    fn menubar_docks_inside_the_named_viewport_edge_at_fit_anchor() {
        let node = menubar("app", "Application", Edge::Top, items());
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Application"));
        assert!(node.interactions.is_empty());
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(dock_of(&node), (Edge::Top, Align::Start));
        assert_eq!(
            node.props.fit,
            Some(Fit::Anchor),
            "Fit::Anchor spans the docked edge; Fit::Content hugs the labels"
        );
        assert_eq!(node.props.clamp, Some(ClampRule::Shrink));
        assert_eq!(node.props.tip, Some(Tip::Flush));
        assert_eq!(node.props.input_policy, Some(InputPolicy::Passthrough));
        assert_eq!(
            node.props.takes_focus,
            Some(false),
            "open menus are not this constructor"
        );
    }

    #[test]
    fn menubar_top_is_edge_top() {
        let node = menubar_top("app", "Application", items());
        assert_eq!(dock_of(&node), (Edge::Top, Align::Start));
        assert_eq!(node.props.fit, Some(Fit::Anchor));
        assert_eq!(
            node,
            menubar("app", "Application", Edge::Top, items()),
            "menubar_top is menubar at Edge::Top"
        );
    }

    #[test]
    fn chrome_is_raised_with_no_four_sided_border() {
        let node = menubar("app", "Application", Edge::Top, items());
        assert_eq!(
            node.props
                .tokens
                .get("background")
                .map(|name| name.as_str()),
            Some(SURFACE_RAISED)
        );
        assert!(
            !node.props.tokens.contains_key("border"),
            "no four-sided border on a menubar"
        );
        assert!(!node.props.tokens.contains_key("radius"));
        assert!(
            !node.props.tokens.contains_key("shadow"),
            "a full-bleed dock is not a floating card"
        );
        assert!(node.props.padding.is_none(), "labels run edge to edge");
    }

    #[test]
    fn items_are_top_level_labels_not_open_menus() {
        let node = menubar("app", "Application", Edge::Top, items());
        let content = child(&node, "content");
        assert_eq!(content.kind, NodeKind::Stack);
        assert_eq!(content.props.axis, Some(crate::geom::Axis::Horizontal));
        assert!(
            content.children.iter().all(|c| c.kind != NodeKind::Surface),
            "open menus are not this constructor"
        );
        let file = child(content, "file");
        assert_eq!(file.semantics.role, Some(Role::Button));
        assert_eq!(file.semantics.label.as_deref(), Some("File"));
        assert_eq!(
            child(content, "edit").semantics.label.as_deref(),
            Some("Edit")
        );
        assert_eq!(
            child(content, "view").semantics.label.as_deref(),
            Some("View")
        );
    }

    #[test]
    fn every_edge_docks_inside_that_window_edge() {
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let node = menubar("app", "Application", edge, items());
            assert_eq!(dock_of(&node).0, edge, "{edge:?}");
            assert_eq!(node.props.fit, Some(Fit::Anchor), "{edge:?}");
        }
    }
}
