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
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, ViewNode};

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

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: large and small spinners both place with a real,
    /// non-degenerate rect, none of their parts outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for node in [loading("wait", "Loading data"), loading_sm("wait", "Loading")] {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{} drew content larger than its own rect",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F is vacuous here: slice-c states Loading is "non-interactive
    /// and not focusable" — confirmed rather than assumed.
    #[test]
    fn loading_declares_no_interaction() {
        assert!(!loading("wait", "Loading data").is_interactive());
        assert!(!loading_sm("wait", "Loading").is_interactive());
    }

    /// Check E: the label text against the page ground (`surface.base`,
    /// matching how `text()` itself is styled — `loading.rs` binds no
    /// `background` of its own anywhere), read through `Props.opacity`.
    #[test]
    fn label_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [loading("wait", "Loading data"), loading_sm("wait", "Loading")] {
                let label = node
                    .children
                    .iter()
                    .find(|c| c.key.as_str() == "label")
                    .expect("label child is present");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "loading label at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
