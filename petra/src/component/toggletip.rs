//! Carbon Toggletip (slice-f). Click-triggered, interactive disclosure.
//!
//! Anatomy (`_toggletip.scss`):
//! 1. Trigger — a labelled [`Role::Button`]. Carbon often ships an info
//!    icon here; the label is required so the trigger is never icon-only
//!    (FR-026, FR-058).
//! 2. Caret + container — [`super::popover::popover_with`] when `open`.
//!    Interactive children belong in the popover (that is why this is
//!    not [`super::tooltip`]).
//!
//! Container max-inline is 288 (`18rem`, SCSS). Open/closed are the only
//! documented states.
//!
//! The trigger declares [`FocusFigure::Hug`]: keyboard focus is two bars
//! beside it, as on a text input. An underline under a trigger whose
//! popover opens flush beneath it landed on the popover's beak (row 37,
//! 2026-09-05), and the operator asked for the sides.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, FocusFigure, Interaction, Key, Role, ViewNode};

/// Carbon toggletip content `max-inline-size` (`18rem`).
const MAX_INLINE: f32 = 288.0;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MAX_INLINE == 288.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A labelled trigger that, when `open`, hosts a popover of `body`.
///
/// The trigger is keyed `"trigger"`; the popover is keyed `"tip"` and
/// anchored to `"trigger"`.
pub fn toggletip(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger_button("trigger", label.clone());
    let mut children = vec![trigger];
    if open {
        let mut tip = popover_with("tip", label, "trigger", vec![super::popover::bubble_text(body.into())]);
        tip.constraints.horizontal.max = Some(MAX_INLINE);
        children.push(tip);
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn trigger_button(key: impl Into<Key>, label: String) -> ViewNode {
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node.with_constraints(pin_height(SIZE_MD)).interactive(
        Role::Button,
        label,
        TRIGGER_INTENTS,
    );
    node.semantics.focus_figure = FocusFigure::Hug;
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
    use super::{MAX_INLINE, toggletip};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, FocusFigure, Interaction, NodeKind, Props, Registry, Role, ViewNode,
    };

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    #[test]
    fn toggletip_closed_is_a_labelled_trigger() {
        let node = toggletip("help", "About filters", false, "Narrow the list.");
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1);
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(
            trigger.semantics.focus_figure,
            FocusFigure::Hug,
            "focus brackets the trigger's sides, as on a text input"
        );
        assert_eq!(trigger.semantics.label.as_deref(), Some("About filters"));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert!(trigger.interactions.contains(&Interaction::Focus));
        assert_eq!(
            child(trigger, "label").props.text.as_deref(),
            Some("About filters")
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    #[test]
    fn toggletip_open_hosts_an_interactive_popover() {
        let node = toggletip("help", "About filters", true, "Narrow the list.");
        assert_eq!(node.semantics.expanded, Some(true));
        let tip = child(&node, "tip");
        assert_eq!(tip.kind, NodeKind::Surface);
        assert_eq!(tip.semantics.role, Some(Role::Overlay));
        assert_eq!(tip.semantics.label.as_deref(), Some("About filters"));
        assert_eq!(tip.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 288.0);
        match &tip.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(tip, "content");
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Narrow the list.")
        );
    }

    /// The open form's `tip` names its `trigger` sibling by bare key
    /// (`Anchor::Sibling`), so it is accepted wherever a caller mounts the
    /// pair — including two containers below the root, which is where the
    /// gallery catalog puts every page.
    #[test]
    fn toggletip_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "toggletip open",
            vec![toggletip("help", "About filters", true, "Narrow the list.")],
        );
    }

    // The frame-level checks below audit the CLOSED form: toggletip's
    // `trigger` stands on its own as a plain interactive `Stack`, unlike
    // Tooltip and the UI shell right panel, whose only constructor IS the
    // anchored surface.

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

    /// Check C/D: the closed trigger places with a real rect and draws no
    /// content larger than it.
    #[test]
    fn closed_trigger_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = toggletip("help", "About filters", false, "Narrow the list.");
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

    /// Check F: the closed trigger declares `Focus` and is reachable.
    #[test]
    fn closed_trigger_is_focus_reachable() {
        let node = toggletip("help", "About filters", false, "Narrow the list.");
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let placement = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the trigger is placed");
        let reachable = focus.order().iter().any(|o| o == &placement.id);
        assert!(
            reachable,
            "closed toggletip trigger must be focus reachable"
        );
    }

    /// Check E: the trigger label against its own resting fill
    /// ([`SURFACE_BASE`], the page's own ground), in both themes.
    #[test]
    fn closed_trigger_label_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = toggletip("help", "About filters", false, "Narrow the list.");
            let trigger = child(&node, "trigger");
            let bg_name = trigger
                .props
                .tokens
                .get("background")
                .expect("trigger binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let label = child(trigger, "label");
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
                "trigger label at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                bg_name.as_str()
            );
        }
    }
}
