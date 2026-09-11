//! Composition helpers the component library is built from.
//!
//! These are not C13 components. They are the stacks, swatches, pads, pins,
//! carets, description lines, and list-panel anatomy every documented
//! component composes. They are public under this named module so an
//! out-of-crate author uses the same helpers rather than forking them
//! (spec 009 contract §8.1).

use std::sync::Arc;

use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb};
use crate::geom::{Axis, Point};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, NodeKind, Props, ViewNode};

use super::tokens;

pub use super::list_box::{Dividers, ListBoxSize, edge_row, list_box, list_box_field, menu_item};

/// A `Stack` on `axis`, gapped by `spacing`, with no other props set.
///
/// Every component with more than one visual part is built from this — the
/// one place `NodeKind::Stack` is spelled inside the library, so a bug in
/// how a row or column is assembled has one place to be found and fixed
/// rather than a dozen.
pub fn stack(
    key: impl Into<Key>,
    axis: Axis,
    spacing: Option<&str>,
    children: Vec<ViewNode>,
) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(axis),
            spacing: spacing.map(tokens::t),
            ..Props::default()
        })
        .with_children(children)
}

/// A fixed-extent, unlabelled rectangle: the drawn box every swatch-shaped
/// piece of chrome in the library is built from (a checkbox's box, a
/// toggle's knob and pad, a status dot, a progress fill and track).
///
/// A bare coloured box carries no semantics of its own —
/// FR-058 has nothing to say about it because nothing here is interactive
/// or labelled — which is exactly why it is a building block a component
/// composes rather than a component the library ships on its own.
pub fn swatch(
    key: impl Into<Key>,
    w: f32,
    h: f32,
    background: Option<&str>,
    border: Option<&str>,
    radius: Option<&str>,
) -> ViewNode {
    let mut props = Props::default();
    if let Some(name) = background {
        props.tokens.insert("background".into(), tokens::t(name));
    }
    if let Some(name) = border {
        props.tokens.insert("border".into(), tokens::t(name));
    }
    if let Some(name) = radius {
        props.tokens.insert("radius".into(), tokens::t(name));
    }
    ViewNode::new(NodeKind::Spacer, key)
        .with_props(props)
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(w),
                max: Some(w),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(h),
                max: Some(h),
                priority: 0,
            },
        })
}

/// [`InsetRefs::symmetric`] over two of this module's spacing constants,
/// named the way `InsetRefs::symmetric` itself is: horizontal first.
pub fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tokens::t(horizontal), tokens::t(vertical))
}

/// Pin `h` as both the minimum and maximum of the block (vertical) extent.
///
/// `ui_shell.rs`'s own `pin_block` and `button.rs`'s own `pin_height` were
/// byte-for-byte the same four lines: two names for one definition, split
/// across two files by nothing but which component happened to need it
/// first. This is the one copy both call sites now share. (A handful of
/// other components — `pagination`, `code_snippet`, `menu`, `accordion`,
/// `content_switcher`, `contained_list`, `dropdown`, `toggletip`,
/// `date_picker` — still carry their own private `pin_height`/`pin_square`
/// helpers; unifying those is a separate, larger change this one does not
/// make.)
pub fn pin_block(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

/// Logical extent of a [`caret`] on both axes: Carbon's 16px glyph box.
pub const CARET_SIZE: f32 = 16.0;

/// Which way a [`caret`] points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretDirection {
    /// Expanded: the triangle points down.
    Down,
    /// Collapsed: the triangle points right.
    Right,
}

/// A disclosure caret: one filled triangle in [`tokens::ICON_SECONDARY`] on a
/// 16×16 canvas, pointing [`CaretDirection::Down`] when the thing it fronts
/// is open and [`CaretDirection::Right`] when it is shut.
///
/// Not an [`IconMark`]. It predates the icon vocabulary
/// growing past `Check`: [`IconMark::ChevronDown`] and
/// [`IconMark::ChevronUp`] exist now (2026-09-04), and this is the shape a
/// tree branch and an expandable tile still draw instead of spelling the
/// word `expanded` next to their label. The pending swap is
/// `icon_toned(key, IconMark::ChevronDown, IconTone::Secondary)` for the
/// tile and a `ChevronRight` mark for the tree, then delete this; that is
/// the tile and tree rows' owners' change. Like [`swatch`] it carries
/// no semantics of its own: the branch or tile that composes it owns the
/// `Semantics.expanded` fact, and the revealed children are the channel a
/// reader who cannot see the triangle still gets (FR-026).
///
/// Geometry, canvas-local: an 8-wide, 4-tall isoceles triangle centred in
/// the box, which is the CaretDown glyph's own proportion. The points are
/// whole units so nothing in the fill lands on a half-pixel edge and gets
/// snapped off centre at 1x (see
/// `.agents/notes/proposed/bug-fix/2026-09-04-a-half-pixel-inset-snaps-a-small-mark-off-centre.md`).
///
/// # Panics
/// Never in practice: one three-vertex convex path is inside every draw-list
/// bound. A panic here means an edit broke convexity, which is a defect.
pub fn caret(key: impl Into<Key>, direction: CaretDirection) -> ViewNode {
    let (a, b, c) = match direction {
        CaretDirection::Down => (
            Point::new(4.0, 6.0),
            Point::new(12.0, 6.0),
            Point::new(8.0, 10.0),
        ),
        CaretDirection::Right => (
            Point::new(6.0, 4.0),
            Point::new(6.0, 12.0),
            Point::new(10.0, 8.0),
        ),
    };
    let paint = Paint::filled(ColorRef::Token(
        tokens::t(tokens::ICON_SECONDARY).as_str().to_owned(),
    ));
    let list = DrawList::new(vec![Command::Path {
        verbs: vec![
            PathVerb::MoveTo(a),
            PathVerb::LineTo(b),
            PathVerb::LineTo(c),
        ],
        closed: true,
        paint,
    }])
    .unwrap_or_else(|err| panic!("caret draw list refused: {err}"));
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(CARET_SIZE),
                max: Some(CARET_SIZE),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(CARET_SIZE),
                max: Some(CARET_SIZE),
                priority: 0,
            },
        })
}

/// Muted supporting text under a heading.
///
/// Carbon `.cds--label-description`. Body type in [`tokens::TEXT_MUTED`].
/// The heading is a sibling this function does not draw. File uploader
/// already spelled this line; field, checkbox, and radio did not. One
/// helper so those call sites do not fork the tokens.
pub fn description(key: impl Into<Key>, text: &str) -> ViewNode {
    let mut props = Props {
        text: Some(text.to_owned()),
        style: Some(tokens::t(tokens::TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props
        .tokens
        .insert("foreground".into(), tokens::t(tokens::TEXT_MUTED));
    ViewNode::new(NodeKind::Text, key).with_props(props)
}
