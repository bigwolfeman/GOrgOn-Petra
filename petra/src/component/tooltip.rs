//! Carbon Tooltip (slice-f). Hover- or focus-triggered, non-interactive
//! bubble.
//!
//! Anatomy (`_tooltip.scss`): caret + content box, on Popover. The trigger
//! is **not** this constructor — it lives on the host control. This node is
//! the bubble only.
//!
//! # A departure from Carbon, on the operator's instruction
//!
//! Carbon's bubble is `$background-inverse` with `$text-inverse` — light on
//! the dark theme, dark on the light one — a step *across* the page's
//! polarity rather than one grey step off it. This shipped that way, and the
//! operator's round-3 note was *"tooltip: white background in dark mode,
//! change it."* Put to him with Carbon's own reference shot beside ours, he
//! chose to change it anyway. So this is a **deliberate departure**, not a
//! defect, and `ignored/carbon-ref/shots/38-tooltip-open.png` is what we gave
//! up.
//!
//! What replaces the polarity is the **top of the layer ramp**,
//! [`SURFACE_LAYER_THREE`] (`#444444` dark). A bubble is dragged over
//! whatever happens to be under the pointer and cannot know what that is: the
//! page is `#121212` and a card is `#222222`, and one grey step off either
//! one is invisible against the other. The ramp's last rung is a step away
//! from both. Ink is [`TEXT_PRIMARY`], which measures about 9:1 there.
//!
//! The caret is engine-drawn in the surface's own fill
//! (`contracts/anchored-placement.md` §5), so it follows the box. Padding is
//! [`SPACING_05`] on every side (`tooltip-padding-block` /
//! `tooltip-padding-inline`); text is `body-01`, [`super::text::text`]'s own
//! step.
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

use super::pad;
use super::popover::{bubble_text, popover_with};
use super::rule;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_STRONG, SPACING_01, SPACING_03, SPACING_05, SURFACE_LAYER_THREE, TEXT_PRIMARY,
    TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::Axis;
use crate::tree::{Key, Role, ViewNode};

/// Carbon default tooltip `max-inline-size`.
const MAX_INLINE: f32 = 288.0;
/// Carbon definition tooltip `max-inline-size` (`definition-tooltip`
/// mixin, slice-f:88).
const DEFINITION_MAX_INLINE: f32 = 176.0;
/// Style-page single-line max-width (design intent, not the SCSS target).
const SINGLE_LINE_INTENT: f32 = 208.0;

const _: () = assert!(MAX_INLINE == 288.0);
const _: () = assert!(DEFINITION_MAX_INLINE == 176.0);
const _: () = assert!(SINGLE_LINE_INTENT == 208.0);

/// The tooltip bubble. `label` is accepted so a caller names the overlay;
/// `Semantics.label` is the `body`, which is the text AT should announce.
/// Anchored to a sibling keyed `"trigger"`.
pub fn tooltip(key: impl Into<Key>, label: impl Into<String>, body: impl Into<String>) -> ViewNode {
    let _label = label.into();
    tooltip_anchored(key, "trigger", body)
}

/// [`tooltip`], anchored to a sibling the caller names.
///
/// The same bubble, for the triggers that are not keyed `"trigger"`. The
/// first of those is [`super::code_snippet`]'s copy control, whose feedback
/// Carbon builds out of these very mixins —
/// `_copy-button.scss:44` is `tooltip--caret` and `:50` is
/// `tooltip--content`, on an element that lives *inside* the button. A
/// second bubble constructor with its own fill and its own padding is how
/// two tooltips in one library stop looking alike, so this is the one
/// bubble with the anchor lifted out rather than a copy of it.
pub fn tooltip_anchored(
    key: impl Into<Key>,
    anchor: impl Into<Key>,
    body: impl Into<String>,
) -> ViewNode {
    let body = body.into();
    let mut run = super::popover::bubble_text(body.clone());
    run.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = popover_with(key, body.clone(), anchor, vec![run]);
    node.constraints.horizontal.max = Some(MAX_INLINE);
    finish_bubble(node, &body)
}

/// The half of a bubble every constructor here shares: the top-of-ramp
/// surface, the overlay role, the body as the AT name, and the refusal to
/// be a control. The variants own only what the mixins change —
/// typography, padding, and the container's max-inline.
fn finish_bubble(mut node: ViewNode, body: &str) -> ViewNode {
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_LAYER_THREE));
    node.semantics.role = Some(Role::Overlay);
    node.semantics.label = Some(body.to_owned());
    debug_assert!(
        node.interactions.is_empty(),
        "a tooltip bubble is not a control"
    );
    node
}

/// Carbon's icon tooltip (`icon-tooltip` mixin, slice-f:78, :89) — the
/// compact bubble that annotates a bare icon trigger.
///
/// What the mixin changes and this constructor carries: the body drops to
/// `body-compact-01` ([`TYPOGRAPHY_BODY_COMPACT`]) and the block padding
/// drops to 2 ([`SPACING_01`], `tooltip-padding-block`) against the
/// default's 16; inline padding stays [`SPACING_05`]. The container's
/// max-inline stays [`MAX_INLINE`]: the mixin overrides no width (slice-f's
/// key numbers name none for this variant), so the SCSS default's 288
/// governs.
///
/// The mixin's `popover-caret-width: 8px` / `popover-caret-height: 4px` and
/// `popover-offset: 8px` are the engine caret's geometry, which this file
/// cannot speak for — the engine draws a caret as deep as the gap
/// (`overlay_surface::caret_of`), and the gap here is the same
/// [`SPACING_03`] every popover places at. The 8x4-vs-6 caret step is
/// therefore **not expressed**; it is the one number of the mixin this
/// constructor does not realize.
pub fn tooltip_icon(
    key: impl Into<Key>,
    label: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    let _label = label.into();
    let body = body.into();
    let mut run = bubble_text(body.clone());
    run.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    run.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = popover_with(key, body.clone(), "trigger", vec![run]);
    node.props.padding = Some(pad(SPACING_05, SPACING_01));
    node.constraints.horizontal.max = Some(MAX_INLINE);
    finish_bubble(node, &body)
}

/// Carbon's definition tooltip (`definition-tooltip` mixin, slice-f:78,
/// :88, :92) — the standalone shape: an underlined inline term and its
/// bubble, one component.
///
/// The trigger is part of this constructor (unlike [`tooltip`], whose
/// trigger lives on the host): the term is text under [`BORDER_STRONG`],
/// the `.cds--definition-term` `border-block-end`. That border is `1px
/// dotted` in Carbon and the draw list has no dash pattern — a rule is one
/// straight stroke — so the underline is the library's flat 1-unit rule in
/// the same material, and the *dots* are the one part of the border this
/// cannot draw. The rule's stack is a sibling the bubble anchors to, so the
/// pair keeps the module's shape (bubble + named anchor) while shipping as
/// one node.
///
/// The bubble is the definition mixin's: block [`SPACING_03`] (8) and
/// inline [`SPACING_05`] (16) padding, [`DEFINITION_MAX_INLINE`] (176)
/// max-inline. Text stays `body-01` — the mixin switches no type step;
/// only [`tooltip_icon`]'s does.
///
/// The States bullet's "hover and focus as independently styled open
/// sub-states (different border treatment each)" (slice-f:80) records that
/// the two open routes differ but not what they differ *in* — the slice
/// captures no token for either border treatment — so the term carries its
/// one resting underline and no per-state pair. That half is a research
/// gap, not a design choice; it is the exclusion rows T057 writes
/// (`VARIANTS.md` 561/562).
pub fn tooltip_definition(
    key: impl Into<Key>,
    term: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    let key = key.into();
    let term = term.into();
    let body = body.into();
    let mut term_text = text("term-text", term);
    term_text
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let term_group = stack(
        "term",
        Axis::Vertical,
        None,
        vec![
            term_text,
            rule("term-underline", Axis::Horizontal, BORDER_STRONG),
        ],
    );
    let mut run = bubble_text(body.clone());
    run.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut bubble = popover_with(format!("{key}-bubble"), body.clone(), "term", vec![run]);
    bubble.props.padding = Some(pad(SPACING_05, SPACING_03));
    bubble.constraints.horizontal.max = Some(DEFINITION_MAX_INLINE);
    let bubble = finish_bubble(bubble, &body);
    stack(key, Axis::Vertical, None, vec![term_group, bubble])
}

#[cfg(test)]
mod tests {
    use super::{
        DEFINITION_MAX_INLINE, MAX_INLINE, SINGLE_LINE_INTENT, tooltip, tooltip_definition,
        tooltip_icon,
    };
    use crate::component::pad;
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{
        BORDER_STRONG, SHADOW_OVERLAY, SPACING_01, SPACING_03, SPACING_05, SURFACE_LAYER_THREE,
        SURFACE_RAISED, TEXT_PRIMARY, TYPOGRAPHY_BODY, TYPOGRAPHY_BODY_COMPACT,
    };

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Anchor, NodeKind, Role, ViewNode};

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

    /// The bubble takes the ramp's last rung, not the popover's own grey.
    ///
    /// It carried Carbon's inverse polarity until 2026-09-05; the operator
    /// asked for a dark bubble in the dark theme and, shown Carbon's own
    /// reference beside ours, chose the departure. What replaces the polarity
    /// still has to be a step away from **both** grounds a bubble can be
    /// dragged over — the page and a card — which `SURFACE_RAISED` is not:
    /// it *is* the card, so a bubble over a card would vanish. Falsify by
    /// leaving `popover_with`'s `SURFACE_RAISED` in place.
    #[test]
    fn the_bubble_takes_the_last_rung_of_the_ramp_not_the_card_it_floats_over() {
        let node = tooltip("copied", "Copied", "Copied to clipboard");
        assert_eq!(token(&node, "background"), Some(SURFACE_LAYER_THREE));
        assert_ne!(
            token(&node, "background"),
            Some(SURFACE_RAISED),
            "a bubble the same tone as a card disappears the moment it is \
             dragged over one"
        );
        assert_eq!(token(&node, "shadow"), Some(SHADOW_OVERLAY));
        assert_eq!(token(&node, "border"), None);
        assert_eq!(
            token(child(child(&node, "content"), "body"), "foreground"),
            Some(TEXT_PRIMARY)
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
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    // Check F does not apply: the bubble has no interactive children
    // (`tooltip_is_a_non_interactive_overlay_bubble` above already asserts
    // this) — a tooltip is never itself a Tab stop, matching slice-f.md's
    // "a tooltip is never itself disabled; it either doesn't render or is
    // suppressed by its host component's disabled state."

    /// Check E: the body text against the resting `background` the outer
    /// `Surface` node binds, in both themes.
    ///
    /// It was the inverse pair until 2026-09-05 — the *other* theme's ink on
    /// the other theme's ground, so the measurement that theme already makes
    /// for its own body text. It is now `text.primary` on
    /// `surface.layer-three`, which is a pairing no theme makes for itself,
    /// so this check earns its keep rather than restating one.
    #[test]
    fn content_text_clears_aa_contrast_against_the_bubble_fill() {
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

    /// The `icon-tooltip` mixin's content-box difference (slice-f:89):
    /// `body-compact-01` type and `tooltip-padding-block: 2px` against the
    /// default's 16, on the *same* bubble — same surface, role, AT name and
    /// 288 max-inline (the mixin overrides no width, slice-f names none).
    #[test]
    fn icon_tooltip_is_the_compact_mixin_on_the_same_bubble() {
        let node = tooltip_icon("tip", "Info", "Copied");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(
            node.semantics.label.as_deref(),
            Some("Copied"),
            "AT name is the body"
        );
        assert!(
            node.interactions.is_empty(),
            "a tooltip bubble is not a control"
        );
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        match &node.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(
            node.props.padding,
            Some(pad(SPACING_05, SPACING_01)),
            "`tooltip-padding-block: 2px`, inline padding unchanged at 16"
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_LAYER_THREE));
        let body = child(child(&node, "content"), "body");
        assert_eq!(
            body.props.style.as_ref().map(|s| s.as_str()),
            Some(TYPOGRAPHY_BODY_COMPACT),
            "the mixin switches the body to `body-compact-01`"
        );
    }

    /// The definition tooltip is one component — term, its
    /// `.cds--definition-term` underline, and the `definition-tooltip`
    /// mixin's bubble anchored to that term (slice-f:78, :88, :92).
    #[test]
    fn definition_tooltip_ships_its_term_and_its_own_mixin_bubble() {
        let node = tooltip_definition("defn", "M-Carbon", "The token layer.");
        assert_eq!(node.kind, NodeKind::Stack);
        assert!(!node.is_interactive(), "a tooltip is not a control");

        let term = child(&node, "term");
        assert_eq!(
            child(term, "term-text").props.text.as_deref(),
            Some("M-Carbon")
        );
        let underline = child(term, "term-underline");
        assert_eq!(underline.kind, NodeKind::Separator);
        assert_eq!(
            token(underline, "background"),
            Some(BORDER_STRONG),
            "the term's `border-block-end` is `border-strong`"
        );

        let bubble = child(&node, "defn-bubble");
        assert_eq!(bubble.kind, NodeKind::Surface);
        assert_eq!(bubble.semantics.label.as_deref(), Some("The token layer."));
        assert!(bubble.interactions.is_empty());
        assert_eq!(
            bubble.constraints.horizontal.max,
            Some(DEFINITION_MAX_INLINE),
            "`max-inline-size: 176px` is the definition mixin's own width"
        );
        assert_eq!(
            bubble.props.padding,
            Some(pad(SPACING_05, SPACING_03)),
            "`padding: 8px 16px` is the definition mixin's own padding"
        );
        match &bubble.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "term"),
            other => panic!("expected the bubble anchored to `term`, got {other:?}"),
        }
        let body = child(child(bubble, "content"), "body");
        assert_eq!(
            body.props.style.as_ref().map(|s| s.as_str()),
            Some(TYPOGRAPHY_BODY),
            "the definition mixin switches no type step; the body keeps `body-01`"
        );
    }
}
