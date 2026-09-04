//! `inline_loading` — Carbon Inline loading (slice-b).
//!
//! Status of an in-flight action. Carbon's active state is a 16×16 spinning
//! ring; the shipped animation registry has no spinner track, so the active
//! mark is a static outlined disc. Finished adds [`IconMark::Check`].
//!
//! Spinner size 16 is MEASURED `_loading.scss` `--small`. Container floor
//! 32 (`min-block-size: 2rem`, `_inline-loading.scss:30`). Gap spinner-to-
//! label is [`SPACING_03`] (8).

use super::icon::{IconMark, icon};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{ACCENT_PRIMARY, BORDER_SUBTLE, SHAPE_FULL, SPACING_03, TEXT_MUTED, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Key, Role, Semantics, ViewNode};

/// Carbon small loading spinner, the only size Inline loading uses.
const SPINNER: f32 = 16.0;
/// Container `min-block-size: 2rem`. MEASURED `_inline-loading.scss:30`.
const MIN_BLOCK: f32 = 32.0;

/// In-place status of an action: spinning (static disc) or finished (check).
///
/// `label` is required ([`Role::Status`] needs a label). `active` selects
/// the mark and [`Semantics.value`]: `"loading"` vs `"finished"`.
pub fn inline_loading(key: impl Into<Key>, label: impl Into<String>, active: bool) -> ViewNode {
    let label = label.into();
    let mark = if active {
        swatch(
            "mark",
            SPINNER,
            SPINNER,
            None,
            Some(BORDER_SUBTLE),
            Some(SHAPE_FULL),
        )
    } else {
        let mut badge = stack(
            "mark",
            Axis::Horizontal,
            None,
            vec![icon("tick", IconMark::Check)],
        );
        badge.props.align = Some(Align::Center);
        badge
            .props
            .tokens
            .insert("background".into(), t(ACCENT_PRIMARY));
        badge.props.tokens.insert("radius".into(), t(SHAPE_FULL));
        badge.with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(SPINNER),
                max: Some(SPINNER),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(SPINNER),
                max: Some(SPINNER),
                priority: 0,
            },
        })
    };

    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![mark, caption]);
    node.props.align = Some(Align::Center);
    node.constraints.vertical.min = Some(MIN_BLOCK);
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(label),
        value: Some(if active {
            "loading".into()
        } else {
            "finished".into()
        }),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{MIN_BLOCK, SPINNER, inline_loading};
    use crate::component::icon::IconMark;
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
    fn active_is_status_loading_with_the_label() {
        let node = inline_loading("save", "Saving", true);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saving"));
        assert_eq!(node.semantics.value.as_deref(), Some("loading"));
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.vertical.min, Some(MIN_BLOCK));
        assert_eq!(MIN_BLOCK, 32.0);
        let mark = child(&node, "mark");
        assert_eq!(mark.constraints.horizontal.min, Some(SPINNER));
        assert_eq!(mark.constraints.vertical.min, Some(SPINNER));
        assert_eq!(SPINNER, 16.0);
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("Saving"));
        assert!(
            node.transition.is_none(),
            "no spinner track is registered; motion is omitted"
        );
    }

    #[test]
    fn finished_adds_a_check_mark() {
        let node = inline_loading("save", "Saved", false);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Saved"));
        assert_eq!(node.semantics.value.as_deref(), Some("finished"));
        let mark = child(&node, "mark");
        let tick = child(mark, "tick");
        assert_eq!(tick.kind, NodeKind::Canvas);
        let list = tick.props.canvas.as_ref().expect("Check draws a path");
        assert!(list.path_verbs() > 0);
        assert!(
            list.commands()
                .iter()
                .any(|c| matches!(c, Command::Path { .. }))
        );
        let _ = IconMark::Check;
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

    /// Check C/D: both the active and the finished mark place with a real
    /// rect, none of them outside the row.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for (label, node) in [
            ("active", inline_loading("save", "Saving", true)),
            ("finished", inline_loading("save", "Saved", false)),
        ] {
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

    /// Check F: `inline_loading` declares no interaction — it is a status
    /// readout, never a control — so neither state contributes anything to
    /// the focus order.
    #[test]
    fn declares_no_interaction_so_nothing_enters_focus_order() {
        for node in [
            inline_loading("save", "Saving", true),
            inline_loading("save", "Saved", false),
        ] {
            assert!(!node.is_interactive());
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            assert!(
                focus.order().is_empty(),
                "a non-interactive status readout must contribute nothing \
                 to the focus order, found {:?}",
                focus.order()
            );
        }
    }

    /// Check E: the label against the page ground it is read on
    /// (`surface.base` — `inline_loading` sets no fill of its own), read
    /// through `Props.opacity`, at both states.
    #[test]
    fn label_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [
                inline_loading("save", "Saving", true),
                inline_loading("save", "Saved", false),
            ] {
                let label = child(&node, "label");
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
                    "label at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
