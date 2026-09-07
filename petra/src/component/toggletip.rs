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
//! Container max-inline is 288 (`18rem`, SCSS), against Popover's 368.
//! Open/closed are the only documented states.
//!
//! # The surface, and a departure the operator chose
//!
//! Carbon separates Toggletip from Popover by **surface**, not by shape:
//! `.cds--popover-content` is `$layer` and `.cds--toggletip-content` is
//! `$background-inverse` with `$text-inverse` — a near-white chip on the
//! g100 page (`ignored/carbon-ref/shots/37-toggletip-open.png`, read
//! 2026-09-05). Ours called `popover_with` and changed only the width cap,
//! so the two rows rendered the **same** `#333333` bubble (measured on
//! `24-popover-open.png` and `37-toggletip-open.png` at device x 700), and
//! the operator's note on row 24 was *"this is the same as toggle tip as
//! far as I can tell"*. He was right, and the pixels say so.
//!
//! This takes [`SURFACE_LAYER_THREE`] rather than `background-inverse`, and
//! that is a **departure recorded on purpose**. Hours earlier the operator
//! read row 38 as *"tooltip: white background in dark mode, change it"* and,
//! shown Carbon's own reference beside ours, chose to change it anyway;
//! `tooltip.rs` carries that decision. A toggletip is the same inverse
//! family, so shipping a light chip here would put the thing he just
//! refused back on the next row. The ramp's last rung is the tone
//! `tooltip.rs` moved to, it is a step away from both the page (`#121212`)
//! and a card (`#222222`), and it leaves this bubble 17 sRGB levels off
//! Popover's. **Restoring Carbon is one line**: bind `background-inverse`
//! and a descendant `text-inverse` here, and re-add the `BACKGROUND_INVERSE`
//! re-export that `tokens.rs` dropped on 2026-09-05.
//!
//! # The trigger carries an information mark
//!
//! Carbon's `.cds--toggletip-button` is a reset button — no fill, no edge —
//! and the reference renders it as the bare word "Why" on the page ground.
//! Ours did the same and the operator read it as a text label, because a
//! `SURFACE_BASE` fill inside a catalog card **is** the card
//! (`catalog.rs::seat_card` reseats it, and the trigger's whole row band
//! measured a uniform `#222222`). Carbon's own convention for this trigger
//! is an information icon, so the trigger now leads with
//! [`IconMark::InformationFilled`] beside its label: a ringed glyph reads as
//! a control on any ground without inventing chrome Carbon does not draw,
//! and it is a second channel to the label rather than a replacement for
//! it (FR-026).
//!
//! The trigger declares [`FocusFigure::Hug`]: keyboard focus is two bars
//! beside it, as on a text input. An underline under a trigger whose
//! popover opens flush beneath it landed on the popover's beak (row 37,
//! 2026-09-05), and the operator asked for the sides.

use super::icon::{IconBox, IconMark, IconTone, icon_in};
use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, SURFACE_LAYER_THREE, TEXT_PRIMARY,
    t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, FocusFigure, FocusShownOn, Interaction, Key, Role, ViewNode,
};

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
    toggletip_with(
        key,
        label,
        open,
        vec![super::popover::bubble_text(body.into())],
    )
}

/// [`toggletip`] whose bubble holds caller-supplied children — Carbon's
/// `.cds--toggletip-actions` row is one of these, and interactive contents
/// are the whole reason this component is not [`super::tooltip`].
pub fn toggletip_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger_button("trigger", label.clone());
    let mut nodes = vec![trigger];
    if open {
        nodes.push(bubble(label, children));
    }
    let mut node = stack(key, Axis::Vertical, None, nodes);
    node.semantics.expanded = Some(open);
    node
}

/// The open surface: [`super::popover::popover_with`]'s chrome at
/// Toggletip's own width cap and on Toggletip's own tone.
fn bubble(label: String, children: Vec<ViewNode>) -> ViewNode {
    let mut tip = popover_with("tip", label, "trigger", children);
    tip.constraints.horizontal.max = Some(MAX_INLINE);
    // The one binding that makes this row tell itself apart from row 24.
    // See the module doc for why it is the ramp's last rung and not
    // `background-inverse`.
    tip.props
        .tokens
        .insert("background".into(), t(SURFACE_LAYER_THREE));
    tip
}

fn trigger_button(key: impl Into<Key>, label: String) -> ViewNode {
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let glyph = icon_in(
        "glyph",
        IconMark::InformationFilled,
        IconBox::Glyph,
        IconTone::Primary,
    );
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![glyph, caption],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, TRIGGER_INTENTS)
        .owning_its_text();
    node.semantics.focus_figure = FocusFigure::Sides;
    node.semantics.focus_shown_on = FocusShownOn::Well;
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
    use super::{MAX_INLINE, toggletip, toggletip_with};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, FocusFigure, FocusShownOn, Interaction, NodeKind, Props, Registry, Role, ViewNode,
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
            FocusFigure::Sides,
            "focus brackets the trigger's sides, as on a text input"
        );
        assert_eq!(trigger.semantics.focus_shown_on, FocusShownOn::Well);
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

    /// **Row 24 against row 37.** The whole operator complaint was that the
    /// two rows draw the same picture, and they did: `toggletip` called
    /// `popover_with` and changed only the width cap, so both bubbles
    /// resolved `surface.raised` and rasterized `#333333`. The two facts
    /// that separate them are asserted here as an inequality, not only as
    /// an equality, so a later edit that puts them back on one tone fails.
    ///
    /// Falsify by deleting the `background` insert in `bubble`.
    #[test]
    fn the_toggletip_bubble_is_not_the_popover_s_own_tone() {
        use crate::component::tokens::{SURFACE_LAYER_THREE, SURFACE_RAISED};
        let open = toggletip("help", "About filters", true, "Narrow the list.");
        let tip = child(&open, "tip");
        let popover = crate::component::popover("p", "Note", "trigger", "Narrow the list.");
        let tone = |node: &ViewNode| {
            node.props
                .tokens
                .get("background")
                .map(|t| t.as_str().to_owned())
                .expect("an anchored bubble binds a resting background")
        };
        assert_eq!(tone(&popover), SURFACE_RAISED, "popover is Carbon's $layer");
        assert_eq!(
            tone(tip),
            SURFACE_LAYER_THREE,
            "a toggletip is a foreign object and takes the ramp's last rung; \
             see this module's doc for why that stands in for Carbon's \
             `background-inverse`"
        );
        assert_ne!(
            tone(tip),
            tone(&popover),
            "row 24 and row 37 are back on one tone, which is the defect"
        );
        assert_eq!(tip.constraints.horizontal.max, Some(MAX_INLINE));
        assert_ne!(
            tip.constraints.horizontal.max, popover.constraints.horizontal.max,
            "288 against 368 is the other half of the split"
        );
    }

    /// Carbon's trigger is a reset button and the reference draws it as a
    /// bare word. Inside a catalog card a `surface.base` fill *is* the
    /// card, so a bare word is all the operator saw. The information mark
    /// is what makes it read as a control, beside the label rather than
    /// instead of it.
    #[test]
    fn the_trigger_leads_with_an_information_mark_beside_its_label() {
        let node = toggletip("help", "About filters", false, "Narrow the list.");
        let trigger = child(&node, "trigger");
        let keys: Vec<&str> = trigger.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["glyph", "label"],
            "the mark leads and the label follows it"
        );
        assert_eq!(child(trigger, "glyph").kind, NodeKind::Canvas);
        assert_eq!(
            child(trigger, "label").props.text.as_deref(),
            Some("About filters"),
            "the mark is a second channel, never the only one"
        );
    }

    /// `toggletip_with` is the interactive-contents form — the reason this
    /// component exists rather than `tooltip` — and its children keep their
    /// own roles.
    #[test]
    fn toggletip_with_hosts_interactive_children() {
        let node = toggletip_with(
            "help",
            "About filters",
            true,
            vec![crate::component::button("learn", "Learn more")],
        );
        let content = child(child(&node, "tip"), "content");
        let action = child(content, "learn");
        assert_eq!(action.semantics.role, Some(Role::Button));
        assert_eq!(action.semantics.label.as_deref(), Some("Learn more"));
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
