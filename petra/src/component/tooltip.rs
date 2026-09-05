//! Carbon Tooltip (slice-f). Hover- or focus-triggered, non-interactive
//! bubble.
//!
//! Anatomy (`_tooltip.scss`): caret + content box, on Popover. The trigger
//! is **not** this constructor — it lives on the host control. This node is
//! the bubble only.
//!
//! What makes it a tooltip and not a popover is the polarity. Carbon's
//! bubble is `$background-inverse` with `$text-inverse` — light on the dark
//! theme, dark on the light one — so it is a step *across* the page's
//! polarity rather than one grey step off it, and it reads as a note laid on
//! the page rather than as another card. The caret is engine-drawn in the
//! surface's own fill (`contracts/anchored-placement.md` §5), so it inverts
//! with the box. Padding is [`SPACING_05`] on every side
//! (`tooltip-padding-block` / `tooltip-padding-inline`); text is `body-01`,
//! [`super::text::text`]'s own step.
//!
//! Distinction from [`super::toggletip`]: a tooltip discloses on hover or
//! focus and MUST NOT contain interactive elements. A toggletip discloses
//! on click and is the place interactive contents go. FR-058 therefore
//! does not apply here: this node has no interactions, so it is outside
//! `ActionableNeedsRoleAndLabel`. [`Role::Overlay`] still gets
//! `Semantics.label` set to the body so AT has a name.
//!
//! Default/multi-line max-inline is 288 (SCSS). Single-line 208 is
//! style-page only and is not bound.

use super::popover::popover_with;
use super::text::text;
use super::tokens::{BACKGROUND_INVERSE, TEXT_INVERSE, t};
use crate::tree::{Key, Role, ViewNode};

/// Carbon default tooltip `max-inline-size`.
const MAX_INLINE: f32 = 288.0;
/// Style-page single-line max-width (design intent, not the SCSS target).
const SINGLE_LINE_INTENT: f32 = 208.0;

const _: () = assert!(MAX_INLINE == 288.0);
const _: () = assert!(SINGLE_LINE_INTENT == 208.0);

/// The tooltip bubble. `label` is accepted so a caller names the overlay;
/// `Semantics.label` is the `body`, which is the text AT should announce.
/// Anchored to a sibling keyed `"trigger"`.
pub fn tooltip(key: impl Into<Key>, label: impl Into<String>, body: impl Into<String>) -> ViewNode {
    let _label = label.into();
    let body = body.into();
    let mut run = text("body", body.clone());
    run.props
        .tokens
        .insert("foreground".into(), t(TEXT_INVERSE));
    let mut node = popover_with(key, body.clone(), "trigger", vec![run]);
    node.props
        .tokens
        .insert("background".into(), t(BACKGROUND_INVERSE));
    node.constraints.horizontal.max = Some(MAX_INLINE);
    node.semantics.role = Some(Role::Overlay);
    node.semantics.label = Some(body);
    debug_assert!(
        node.interactions.is_empty(),
        "a tooltip bubble is not a control"
    );
    node
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, SINGLE_LINE_INTENT, tooltip};
    use crate::component::tokens::{BACKGROUND_INVERSE, SHADOW_OVERLAY, TEXT_INVERSE};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Anchor, NodeKind, Props, Registry, Role, ViewNode};

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

    #[test]
    fn tooltip_is_a_non_interactive_overlay_bubble() {
        let node = tooltip("copied", "Copied", "Copied to clipboard");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(
            node.semantics.label.as_deref(),
            Some("Copied to clipboard"),
            "AT name is the body"
        );
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 288.0);
        assert_eq!(SINGLE_LINE_INTENT, 208.0);
        match &node.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(&node, "content");
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "the caret is the engine's; no `^` word stands in for it"
        );
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Copied to clipboard")
        );
        assert!(
            child(content, "body").interactions.is_empty(),
            "the bubble has no interactive children"
        );
    }

    /// The bubble is the *inverse* polarity — Carbon's `$background-inverse`
    /// under `$text-inverse` — and not another raised grey. Falsify by
    /// leaving `popover_with`'s `SURFACE_RAISED` in place.
    #[test]
    fn the_bubble_is_the_inverse_polarity_not_a_raised_card() {
        let node = tooltip("copied", "Copied", "Copied to clipboard");
        assert_eq!(token(&node, "background"), Some(BACKGROUND_INVERSE));
        assert_eq!(token(&node, "shadow"), Some(SHADOW_OVERLAY));
        assert_eq!(token(&node, "border"), None);
        assert_eq!(
            token(child(child(&node, "content"), "body"), "foreground"),
            Some(TEXT_INVERSE)
        );
        assert_eq!(
            node.props.tip, None,
            "absent is `Tip::Caret`: a tooltip points at its trigger"
        );
    }

    /// `tooltip` IS the anchored bubble, naming a `trigger` sibling by bare
    /// key (`Anchor::Sibling`). Placed beside a control keyed `trigger`, it
    /// is accepted wherever the caller mounts the pair — here two
    /// containers below the root, the gallery catalog's own depth.
    #[test]
    fn tooltip_validates_beside_its_trigger_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "tooltip",
            vec![
                crate::component::button("trigger", "Copy"),
                tooltip("copied", "Copied", "Copied to clipboard"),
            ],
        );
    }

    // `tooltip` builds via `super::popover::popover_with`, so `node` IS the
    // anchored surface with no separate closed form — unlike Toggletip,
    // whose `trigger` stands alone as a plain interactive node. The
    // frame-level checks below therefore audit `content`, the inner `Stack`
    // `popover_with` builds (`body`), which carries no `anchor` of its own
    // and so petrifies standalone — the same technique `popover.rs`'s own
    // `content_frame_geometry_...` test uses.

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

    /// Check C/D: the body text places with a real rect inside `content`'s
    /// own rect.
    #[test]
    fn content_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = tooltip("copied", "Copied", "Copied to clipboard");
        let content = child(&node, "content").clone();
        let frame = petrify_lone(content);
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

    // Check F does not apply: the bubble has no interactive children
    // (`tooltip_is_a_non_interactive_overlay_bubble` above already asserts
    // this) — a tooltip is never itself a Tab stop, matching slice-f.md's
    // "a tooltip is never itself disabled; it either doesn't render or is
    // suppressed by its host component's disabled state."

    /// Check E: the body text against [`BACKGROUND_INVERSE`], the resting
    /// `background` the outer `Surface` node binds, in both themes. The
    /// inverse pair is the *other* theme's ink on the other theme's ground,
    /// so this is the same measurement that theme makes for its own body
    /// text.
    #[test]
    fn content_text_clears_aa_contrast_against_the_inverse_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = tooltip("copied", "Copied", "Copied to clipboard");
            let surface_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the surface binds a resting background");
            let surface_bg = color(&theme, surface_bg_name.as_str());
            let content = child(&node, "content");
            let label = child(content, "body");
            let fg_name = label
                .props
                .tokens
                .get("foreground")
                .expect("body text binds a foreground");
            let opacity = label.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str())
                .faded(opacity)
                .over(surface_bg);
            let ratio = fg.contrast_ratio(surface_bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "body at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                surface_bg_name.as_str()
            );
        }
    }
}
