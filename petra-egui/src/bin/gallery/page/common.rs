//! What several catalog pages share.
//!
//! The token references, the layout stacks a page body is built from, and
//! the path test a handler matches its node ids with. Pages import these
//! rather than copying them, so the drift the per-page split pulls apart
//! cannot come back a different way.

use gorgon_petra::component::text;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    AxisConstraint, Constraints, NodeKind, Props, TextWrap, TrackSize, ViewNode,
};

/// A spacing token reference, for the styling props that take one (FR-053).
pub fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("catalog spacing tokens are well-formed"))
}

/// A colour or type token reference, for `props.tokens` values.
pub fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("catalog style tokens are well-formed")
}

/// Whether any segment of the routed node path `node` is exactly `key`.
///
/// A press on a child (knob, label, state text) still names that child in
/// the route, so a handler matches its own id anywhere in the path rather
/// than only as the last segment.
pub fn path_has(node: &str, key: &str) -> bool {
    node.split('/').any(|part| part == key)
}

/// Whether any of the dismissed surface ids `ids` is the surface keyed
/// `key` — the test a page runs in [`super::Page::dismissed`].
///
/// By segment, as [`path_has`] matches, because the ids are canonical
/// placement paths under the chrome's mount point and the page knows only
/// the key it gave its own surface.
pub fn dismisses(ids: &[String], key: &str) -> bool {
    ids.iter().any(|id| path_has(id, key))
}

/// A vertical run of blocks, each as tall as it needs to be.
///
/// A single-column `Grid`, not a `Stack`. A vertical `Stack` placed at
/// an exact height divides that height among its children by equal
/// share; a `Grid`'s implicit rows are `FitContent`. See
/// `examples/gallery.rs` `Gallery::column`.
pub fn column(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            row_spacing: spacing,
            ..Props::default()
        })
        .with_children(children)
}

/// A horizontal run of blocks, centred on the cross axis.
pub fn row(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            spacing,
            align: Some(Align::Center),
            ..Props::default()
        })
        .with_children(children)
}

/// The body of a `section`: a column that outranks the section's title
/// when the section divides its height.
pub fn body(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    column(key, spacing, children).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: None,
            max: None,
            priority: 1,
        },
        ..Constraints::default()
    })
}

/// A text leaf that wraps at its width instead of overflowing it.
pub fn wrapped(key: &str, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props.wrap = Some(TextWrap::Wrap);
    node
}

/// The first node whose key is `key`, depth first. For page and chrome tests
/// that read a control's semantics back out of a built tree.
#[cfg(test)]
pub fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
    if node.key.as_str() == key {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, key))
}
