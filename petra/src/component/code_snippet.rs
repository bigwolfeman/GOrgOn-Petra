//! Carbon Code snippet (slice-a).
//!
//! Three variants, three constructors. No named sm/md/lg scale; each
//! variant has its own fixed numbers (style page Structure; T070 prefers
//! SCSS where they disagree):
//!
//! - [`code_snippet`] — single line, height 40, fill [`SURFACE_RAISED`].
//! - [`code_snippet_multi`] — multi-line, min-height 288.
//! - [`code_snippet_inline`] — inline, height 16, radius [`SHAPE_SM`]
//!   (SCSS 4px; style-page 2px is stale).
//!
//! Ink is [`super::tokens::TEXT_PRIMARY`]. Do not invent syntax colours.
//! Copy is a labelled [`Role::Button`] (`"Copy"`), never icon-only
//! (FR-058, FR-026).

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{SHAPE_SM, SIZE_MD, SPACING_02, SPACING_03, SPACING_05, SURFACE_RAISED, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Interaction, Key, Role, ViewNode};

/// Carbon `.cds--snippet--multi` `min-block-size`.
const MULTI_MIN: f32 = 288.0;
/// Carbon `.cds--snippet--inline` container height.
const INLINE_HEIGHT: f32 = 16.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MULTI_MIN == 288.0);
const _: () = assert!(INLINE_HEIGHT == 16.0);

const COPY_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click];

/// Single-line snippet. Height 40. Copy button labelled `"Copy"`.
pub fn code_snippet(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![code_text(code.into()), copy_button()],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    paint_well(node).with_constraints(pin_height(SIZE_MD))
}

/// Multi-line snippet. Min-height 288. Copy button labelled `"Copy"`.
pub fn code_snippet_multi(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![copy_button(), code_text(code.into())],
    );
    node.props.padding = Some(pad(SPACING_05, SPACING_05));
    paint_well(node).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(MULTI_MIN),
            max: None,
            priority: 0,
        },
        ..Constraints::default()
    })
}

/// Inline snippet. Height 16, radius sm. Display only — no copy button.
pub fn code_snippet_inline(key: impl Into<Key>, code: impl Into<String>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![code_text(code.into())]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_03)),
        right: Some(t(SPACING_03)),
        ..InsetRefs::default()
    });
    let mut node = paint_well(node);
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.with_constraints(pin_height(INLINE_HEIGHT))
}

fn code_text(code: String) -> ViewNode {
    text("code", code)
}

fn copy_button() -> ViewNode {
    let mut node = stack(
        "copy",
        Axis::Horizontal,
        None,
        vec![text("copy-label", "Copy")],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_03, SPACING_02));
    node.interactive(Role::Button, "Copy", COPY_INTENTS)
}

fn paint_well(mut node: ViewNode) -> ViewNode {
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node
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
    use super::{
        INLINE_HEIGHT, MULTI_MIN, SHAPE_SM, SIZE_MD, SURFACE_RAISED, code_snippet,
        code_snippet_inline, code_snippet_multi,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn code_snippet_exists_at_height_forty() {
        let node = code_snippet("s", "fn main() {}");
        assert_eq!(node.key.as_str(), "s");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert!(!node.is_interactive());
        assert_eq!(
            named(&node, "code").props.text.as_deref(),
            Some("fn main() {}")
        );
    }

    #[test]
    fn code_snippet_copy_is_a_labelled_button() {
        let node = code_snippet("s", "let x = 1;");
        let copy = named(&node, "copy");
        assert_eq!(copy.semantics.role, Some(Role::Button));
        assert_eq!(copy.semantics.label.as_deref(), Some("Copy"));
        assert!(copy.interactions.contains(&Interaction::Focus));
        assert!(copy.interactions.contains(&Interaction::Click));
        assert!(copy.is_interactive());
    }

    #[test]
    fn code_snippet_multi_has_min_height_288() {
        let node = code_snippet_multi("s", "line 1\nline 2");
        assert_eq!(node.constraints.vertical.min, Some(MULTI_MIN));
        assert_eq!(node.constraints.vertical.max, None);
        assert_eq!(MULTI_MIN, 288.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        named(&node, "copy");
        named(&node, "code");
    }

    #[test]
    fn code_snippet_inline_is_sixteen_tall_with_sm_radius() {
        let node = code_snippet_inline("s", "ViewNode");
        assert_eq!(node.constraints.vertical.min, Some(INLINE_HEIGHT));
        assert_eq!(node.constraints.vertical.max, Some(INLINE_HEIGHT));
        assert_eq!(INLINE_HEIGHT, 16.0);
        assert_eq!(token(&node, "radius"), Some(SHAPE_SM));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert!(!node.is_interactive());
        assert!(
            node.children
                .iter()
                .all(|child| child.semantics.role != Some(Role::Button)),
            "inline snippet has no copy button"
        );
    }

    #[test]
    fn code_snippet_does_not_invent_syntax_colours() {
        let node = code_snippet("s", "fn main() {}");
        let code = named(&node, "code");
        assert_eq!(
            token(code, "foreground"),
            Some(super::super::tokens::TEXT_PRIMARY)
        );
        assert!(
            code.props
                .tokens
                .keys()
                .all(|k| k != "syntax" && !k.starts_with("syntax.")),
            "no invented syntax colour slots"
        );
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

    /// Check C/D across all three variants: no degenerate rect, no child
    /// placed outside its parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
            ("inline", code_snippet_inline("s", "ViewNode")),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
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
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: the copy button on `single`/`multi` declares `Focus` and is
    /// reachable; `inline` has no copy button and is not interactive at all.
    #[test]
    fn copy_button_is_reachable_in_focus_order() {
        for (label, node) in [
            ("single", code_snippet("s", "fn main() {}")),
            ("multi", code_snippet_multi("s", "line 1\nline 2")),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let copy = frame
                .placements
                .iter()
                .find(|p| p.semantics.role == Some(Role::Button))
                .unwrap_or_else(|| panic!("{label}: no copy button placed"));
            assert!(
                focus.order().iter().any(|id| id == &copy.id),
                "{label}: copy button declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: code text and the copy label against the snippet's own
    /// well fill, in both themes, read through `Props.opacity`.
    #[test]
    fn snippet_and_copy_text_clear_aa_contrast_against_the_well_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("single", code_snippet("s", "fn main() {}")),
                ("multi", code_snippet_multi("s", "line 1\nline 2")),
                ("inline", code_snippet_inline("s", "ViewNode")),
            ] {
                let bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: snippet has no resting background"));
                let bg = color(&theme, bg_name.as_str());
                fn walk_text(node: &ViewNode, bg: ColorValue, theme: &Theme, min: f32, label: &str) {
                    if node.props.text.is_some() {
                        if let Some(fg_name) = node.props.tokens.get("foreground") {
                            let opacity = node.props.opacity.unwrap_or(1.0);
                            let fg = color(theme, fg_name.as_str()).faded(opacity).over(bg);
                            let ratio = fg.contrast_ratio(bg);
                            assert!(
                                ratio >= min,
                                "{label}: {:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                                node.key,
                                fg_name.as_str()
                            );
                        }
                    }
                    for child in &node.children {
                        walk_text(child, bg, theme, min, label);
                    }
                }
                walk_text(&node, bg, &theme, MIN_TEXT_CONTRAST, label);
            }
        }
    }
}
