//! `icon` — a digest-visible mark a labeled control can carry.
//!
//! # FR-026: a second channel, never the only one
//!
//! An icon MUST sit beside text or shape. It MUST never be the only channel
//! a part communicates through (Constitution VII, the same rule
//! [`crate::token::StatusToken`] already enforces for status). This
//! constructor is the mark itself: non-interactive, no role, no label of
//! its own. The labeled control that contains it owns those — the toggle
//! has the label. Making [`icon`] a `Role::Button` with no extra label
//! would turn the mark into a control the audit cannot name.
//!
//! A swatch is not a check. [`super::swatch`] is a filled rectangle, and a
//! square pretending to be a tick is the thing this file exists to stop.
//! The picture is a [`crate::draw::DrawList`] on a [`NodeKind::Canvas`], so
//! the path reaches the frame digest as the path.

use std::sync::Arc;

use super::tokens::{TEXT_ON_ACCENT, t};
use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb};
use crate::geom::Point;
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, ViewNode};

/// Logical extent of an icon node on both axes.
///
/// Matches the Carbon small-toggle handle (`convert.to-rem(10px)`, slice-f)
/// and the status marker. The Carbon tick itself is the 6×5
/// `.cds--toggle__check` path, centred in this box.
const SIZE: f32 = 10.0;

/// A named mark [`icon`] can draw.
///
/// The first mark is the one the small toggle needs. Later marks land as
/// the components that carry them land — an empty vocabulary here would
/// make FR-026 a comment with no constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconMark {
    /// Carbon small-toggle on-tick (slice-f `.cds--toggle__check`).
    ///
    /// Source path, viewBox `0 0 6 5`:
    /// `M2.2 2.7L5 0 6 1 2.2 5 0 2.7 1 1.5z`.
    Check,
}

/// A non-interactive visual mark, used inside a labeled control.
///
/// `mark` chooses the picture. The fill is `text.on-accent`, so the tick
/// reads on an accent track (the small toggle's on state). No role, no
/// interactions: those belong to the control that contains this node.
#[must_use]
pub fn icon(key: impl Into<Key>, mark: IconMark) -> ViewNode {
    let list = match mark {
        IconMark::Check => check_mark(),
    };
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(SIZE),
                max: Some(SIZE),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(SIZE),
                max: Some(SIZE),
                priority: 0,
            },
        })
}

/// Carbon `.cds--toggle__check` as two convex filled quads.
///
/// The six-vertex outline is a thick chevron, and [`DrawList::new`] refuses
/// a filled non-convex path (`contracts/draw-list.md` §5). Split on the
/// inner-V to bottom-tip diagonal, the same way the cat's ears are two
/// triangles rather than two notches in the head.
///
/// The 6×5 viewBox is centred in the 10×10 node: offset `(2.0, 2.5)`.
///
/// # Panics
/// Never in practice: two four-verb convex quads are well inside every
/// draw-list bound. A panic here means an edit broke convexity, which is a
/// defect and not a runtime condition to handle.
fn check_mark() -> DrawList {
    let ox = 2.0;
    let oy = 2.5;
    let p = |x: f32, y: f32| Point::new(x + ox, y + oy);
    // Vertex letters follow the Carbon path order:
    // A inner V, B inner top of the long arm, C outer top, D bottom tip,
    // E outer left of the short arm, F inner left.
    let a = p(2.2, 2.7);
    let b = p(5.0, 0.0);
    let c = p(6.0, 1.0);
    let d = p(2.2, 5.0);
    let e = p(0.0, 2.7);
    let f = p(1.0, 1.5);
    let paint = Paint::filled(ColorRef::Token(t(TEXT_ON_ACCENT).as_str().to_owned()));
    DrawList::new(vec![
        filled_quad(a, b, c, d, paint.clone()),
        filled_quad(a, d, e, f, paint),
    ])
    .unwrap_or_else(|err| panic!("Check mark draw list refused: {err}"))
}

fn filled_quad(p0: Point, p1: Point, p2: Point, p3: Point, paint: Paint) -> Command {
    Command::Path {
        verbs: vec![
            PathVerb::MoveTo(p0),
            PathVerb::LineTo(p1),
            PathVerb::LineTo(p2),
            PathVerb::LineTo(p3),
        ],
        closed: true,
        paint,
    }
}

#[cfg(test)]
mod tests {
    use super::{IconMark, SIZE, icon};
    use crate::component::tokens::TEXT_ON_ACCENT;
    use crate::draw::{ColorRef, Command};
    use crate::tree::NodeKind;

    #[test]
    fn icon_produces_a_node_with_the_given_key() {
        let node = icon("tick", IconMark::Check);
        assert_eq!(node.key.as_str(), "tick");
    }

    #[test]
    fn icon_is_not_interactive() {
        let node = icon("tick", IconMark::Check);
        assert!(
            !node.is_interactive(),
            "an icon is a visual part, not a control"
        );
        assert!(node.interactions.is_empty());
        assert!(
            node.semantics.role.is_none(),
            "FR-026: the labeled control owns the role; icon() does not take a label"
        );
        assert!(node.semantics.label.is_none());
    }

    #[test]
    fn check_mark_has_a_non_empty_canvas_draw_list() {
        let node = icon("tick", IconMark::Check);
        assert_eq!(node.kind, NodeKind::Canvas);
        assert_eq!(node.constraints.horizontal.min, Some(SIZE));
        assert_eq!(node.constraints.horizontal.max, Some(SIZE));
        assert_eq!(node.constraints.vertical.min, Some(SIZE));
        assert_eq!(node.constraints.vertical.max, Some(SIZE));
        let list = node
            .props
            .canvas
            .as_ref()
            .expect("Check is a canvas with a draw list");
        assert!(
            !list.is_empty(),
            "a swatch pretending to be a tick is empty of paths"
        );
        assert!(
            list.path_verbs() > 0,
            "Check is a path, not a filled rectangle"
        );
        let mut saw_fill = false;
        for command in list.commands() {
            let Command::Path { paint, verbs, .. } = command else {
                continue;
            };
            assert!(!verbs.is_empty());
            match paint.fill.as_ref() {
                Some(ColorRef::Token(name)) => {
                    assert_eq!(name, TEXT_ON_ACCENT);
                    saw_fill = true;
                }
                other => panic!("Check fill must be token {TEXT_ON_ACCENT}, got {other:?}"),
            }
        }
        assert!(saw_fill, "Check paints with TEXT_ON_ACCENT");
    }
}
