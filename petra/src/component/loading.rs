//! `loading` — Carbon Loading (slice-c): a circular in-progress indicator.
//!
//! Carbon's spinner is two concentric SVG strokes rotated by CSS `@keyframes`.
//! The shipped animation registry names only `toggle-knob`; there is no
//! spinner track, and this module does not invent one. The picture is a
//! static [`Command::Ellipse`] ring.
//!
//! Sizes MEASURED `_vars.scss` / `_loading.scss`: default 88 (`$loading-size:
//! 5.5rem`), small 16 (`--small`). Stroke 10 (default) / ~2 (small, the
//! default stroke scaled by 16/88).

use std::sync::Arc;

use super::stack;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SPACING_03, TEXT_MUTED, t};
use crate::draw::{ColorRef, Command, DrawList, Paint, Stroke, Width};
use crate::geom::{Align, Axis, Point, Size};
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, Role, Semantics, ViewNode};

/// Carbon default / large spinner. MEASURED `$loading-size: 5.5rem`.
const SIZE_LG: f32 = 88.0;
/// Carbon `--small`. MEASURED `_loading.scss` `convert.to-rem(16px)`.
const SIZE_SM: f32 = 16.0;
/// Default SVG stroke. MEASURED `_loading.scss` `circle` rule.
const STROKE_LG: f32 = 10.0;
/// Small stroke: default 10 scaled by 16/88, rounded to a 2-unit hairline.
const STROKE_SM: f32 = 2.0;

#[derive(Clone, Copy)]
enum LoadingSize {
    Large,
    Small,
}

impl LoadingSize {
    fn extent(self) -> f32 {
        match self {
            Self::Large => SIZE_LG,
            Self::Small => SIZE_SM,
        }
    }

    fn stroke(self) -> f32 {
        match self {
            Self::Large => STROKE_LG,
            Self::Small => STROKE_SM,
        }
    }
}

/// Circular in-progress indicator, Carbon large (88).
///
/// `label` is required: [`Role::Progress`] refuses an empty label.
pub fn loading(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    loading_sized(key, label, LoadingSize::Large)
}

/// Circular in-progress indicator, Carbon small (16).
pub fn loading_sm(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    loading_sized(key, label, LoadingSize::Small)
}

fn loading_sized(key: impl Into<Key>, label: impl Into<String>, size: LoadingSize) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![ring("spinner", size), caption],
    );
    node.props.align = Some(Align::Center);
    node.semantics = Semantics {
        role: Some(Role::Progress),
        label: Some(label),
        value: Some("loading".into()),
        ..Semantics::default()
    };
    node
}

/// Track + accent ring, no rotation. Two ellipses so the track is not
/// colour-alone: [`BORDER_SUBTLE`] sits under [`ACCENT_PRIMARY`].
fn ring(key: impl Into<Key>, size: LoadingSize) -> ViewNode {
    let extent = size.extent();
    let stroke = size.stroke();
    let half = extent / 2.0;
    let radii = Size::new(half - stroke / 2.0, half - stroke / 2.0);
    let center = Point::new(half, half);
    let list = DrawList::new(vec![
        Command::Ellipse {
            center,
            radii,
            paint: Paint::stroked(Stroke {
                width: Width::Logical(stroke),
                color: ColorRef::Token(t(BORDER_SUBTLE).as_str().to_owned()),
            }),
        },
        Command::Ellipse {
            center,
            radii: Size::new(radii.w * 0.5, radii.h * 0.5),
            paint: Paint::stroked(Stroke {
                width: Width::Logical(stroke),
                color: ColorRef::Token(t(ACCENT_PRIMARY).as_str().to_owned()),
            }),
        },
    ])
    .unwrap_or_else(|err| panic!("loading ring draw list refused: {err}"));
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(extent),
                max: Some(extent),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(extent),
                max: Some(extent),
                priority: 0,
            },
        })
}

#[cfg(test)]
mod tests {
    use super::{SIZE_LG, SIZE_SM, loading, loading_sm};
    use crate::draw::Command;
    use crate::tree::{NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn loading_is_progress_with_an_88_ring() {
        let node = loading("wait", "Loading data");
        assert_eq!(node.semantics.role, Some(Role::Progress));
        assert_eq!(node.semantics.label.as_deref(), Some("Loading data"));
        assert_eq!(node.semantics.value.as_deref(), Some("loading"));
        assert!(!node.is_interactive());
        assert!(node.transition.is_none());
        assert!(!node.ambient);
        let spinner = child(&node, "spinner");
        assert_eq!(spinner.kind, NodeKind::Canvas);
        assert_eq!(spinner.constraints.horizontal.min, Some(SIZE_LG));
        assert_eq!(spinner.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 88.0);
        let list = spinner.props.canvas.as_ref().expect("ring is a canvas");
        let ellipses = list
            .commands()
            .iter()
            .filter(|c| matches!(c, Command::Ellipse { .. }))
            .count();
        assert_eq!(ellipses, 2);
        assert_eq!(
            child(&node, "label").props.text.as_deref(),
            Some("Loading data")
        );
    }

    #[test]
    fn loading_sm_is_16() {
        let node = loading_sm("wait", "Loading");
        assert_eq!(node.semantics.role, Some(Role::Progress));
        assert_eq!(node.semantics.label.as_deref(), Some("Loading"));
        let spinner = child(&node, "spinner");
        assert_eq!(spinner.constraints.horizontal.min, Some(SIZE_SM));
        assert_eq!(spinner.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 16.0);
        assert!(node.transition.is_none());
    }
}
