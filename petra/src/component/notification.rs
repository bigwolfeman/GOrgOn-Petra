//! Carbon Notification (slice-c). Status colours stay Petra's.
//!
//! Anatomy (`_toast-notification.scss` / `_inline-notification.scss`), as
//! this library draws it:
//! 1. Head — the status glyph, then the required name (and the non-colour
//!    channel), `heading-compact-01`, on one line.
//! 2. Body — the message, `body-compact-01`.
//! 3. Optional action — a [`Role::Button`] with a label.
//!
//! Head and body are centred in the card, on both axes.
//!
//! # No rail, and a glyph instead
//!
//! Carbon draws a `border-inline-start: 3px` in the status colour
//! (`_notification.scss`'s `notification--experimental` mixin). This library
//! drew that rail in the accent, and the operator asked twice for it to go
//! (`.agents/carbon-waves/ROUND2-DEFECTS.md`, "Decisions the operator has
//! now made twice": *"remove the handle, center the text"*). It is gone and
//! it stays gone.
//!
//! That left the card with no status channel at all, which is what the
//! operator's round-3 line — *"give it an icon field"* — is about.
//! [`NotificationKind`] is that field, and its channel is Carbon's own
//! second one: the leading status glyph. `Notification.js` maps
//! `kind -> icon` as `error: ErrorFilled`, `success: CheckmarkFilled`,
//! `warning: WarningFilled`, `info: InformationFilled`, drawn at
//! `size: 20` — MEASURED
//! `ignored/carbon-ref/node_modules/@carbon/react/lib/components/
//! Notification/Notification.js:139-152`.
//!
//! # The glyph is a shape, never a hue
//!
//! Carbon tints that glyph with `$support-error` / `-success` / `-warning`
//! / `-info` and puts it at the card's leading edge. Neither is copied:
//!
//! - **Tone.** [`super::IconTone`] carries no status tone, so all four
//!   glyphs draw in `icon-primary`. That is not a loss. The four marks are
//!   four different pictures — a slash, a bang, an `i`, a tick — so the
//!   kind survives with the colour turned off, which is FR-015 for a
//!   red-green colourblind reader and is stricter than Carbon's own
//!   hue-plus-shape pairing. The word is a third channel:
//!   [`Semantics.value`] carries the kind.
//! - **Placement.** Carbon puts the glyph inline-start of the title, one
//!   `$spacing-05` before it, and left-aligns the pair against the card's
//!   leading edge. The operator asked twice for the text to be centred, so
//!   only the second half of that is dropped: glyph and title are a
//!   horizontal row, the glyph still leading, and the **row** is what the
//!   card centres. Both hold. The earlier reading — glyph stacked above the
//!   title — is gone; it read as a loose symbol rather than a status field.
//!
//! Carbon's alert palette (`$notification-background-error` / success
//! green / warning yellow) is **not** used either. Those names are not in
//! [`super::tokens`], and painting red/green would fail FR-015.
//!
//! [`notification`] / [`notification_toast`] are a [`Role::Toast`]
//! surface. [`notification_inline`] is in-flow [`Role::Status`].

use super::icon::{IconBox, IconMark, IconTone, icon_in};
use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    LAYER_HOVER, SHADOW_RAISED, SHAPE_SM, SPACING_03, SPACING_04, SPACING_05, SURFACE_RAISED,
    TEXT_PRIMARY, TYPOGRAPHY_HEADING_SM, t,
};
use crate::geom::{Align, Axis};
// `crate::tree::Align` is the cross-axis half of an anchor and
// `crate::geom::Align` is a child's alignment inside its parent's cell. Both
// are used in this module, so the anchor one is spelled out at every use.
use crate::tree::{
    Anchor, AxisConstraint, ClampRule, Edge, InputPolicy, Interaction, Key, Layer, NodeKind, Props,
    Role, Semantics, ViewNode,
};

/// Carbon toast `inline-size` below the `max` breakpoint (`18rem`).
const TOAST_INLINE: f32 = 288.0;
/// Carbon inline/actionable `min-block-size` (`3rem`).
const INLINE_MIN_BLOCK: f32 = 48.0;
/// Carbon inline/actionable `min-inline-size` (`288px`) — MEASURED slice-c
/// Notification, "Inline / Actionable: ... `min-inline-size: 288px` floor".
/// Without it every card shrink-wraps its own sentence and a column of them
/// stair-steps, which is what the round-3 walk saw.
const INLINE_MIN_INLINE: f32 = 288.0;
/// Carbon inline/actionable `max-inline-size` at the `md` breakpoint
/// (`608px`) — MEASURED slice-c Notification, "responsive `max-inline-size`
/// ramp `288px -> 608px (md) -> 736px (lg) -> 832px (max)`". Only the `md`
/// step is taken: this library has no breakpoint machinery, and the
/// narrowest cap is the one that never lets a card run the width of a
/// desktop page.
const INLINE_MAX_INLINE: f32 = 608.0;

/// Carbon's toast region inset from the window, `$spacing-05` = 16.
///
/// MEASURED `gorgon/petra/src/token/shipped.rs:994`, `("spacing-05", 16.0)`,
/// which is this library's copy of `@carbon/layout`'s spacing ramp. The
/// region itself is SOURCED: slice-c Notification records the toast as
/// "non-modal, time-based, **top-of-screen**" and its motion as "slides in
/// and out from the **top right**"
/// (`.agents/research/08-25-2026/Carbon-Component-Inventory/slice-c.md:189`,
/// `:217`), and the shipped `padding-right`/`margin` figures on that row are
/// all `$spacing-05`.
const TOAST_DOCK: &str = SPACING_05;

const _: () = assert!(TOAST_INLINE == 288.0);
const _: () = assert!(INLINE_MIN_BLOCK == 48.0);
const _: () = assert!(INLINE_MIN_INLINE == 288.0);
const _: () = assert!(INLINE_MAX_INLINE == 608.0);

const ACTION_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Carbon's notification status field: which of the four things happened.
///
/// The field the operator asked for on 2026-09-05. Each kind names one of
/// Carbon's four status glyphs (`Notification.js`'s `kind -> icon` map) and
/// one word. Both reach the card; the hue does not, for the reason the
/// module doc gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    /// Carbon `kind: 'error'`, `ErrorFilled` — a ring around a slash.
    Error,
    /// Carbon `kind: 'warning'`, `WarningFilled` — a ring around a bang.
    Warning,
    /// Carbon `kind: 'info'`, `InformationFilled` — a ring around an `i`.
    /// Carbon's own `InlineNotification` default is `error`; this library's
    /// is the quiet one, so a caller that names no kind never shouts.
    #[default]
    Info,
    /// Carbon `kind: 'success'`, `CheckmarkFilled` — a ring around a tick.
    Success,
}

impl NotificationKind {
    /// The Carbon glyph for this kind. MEASURED `Notification.js:139-144`.
    #[must_use]
    pub fn glyph(self) -> IconMark {
        match self {
            Self::Error => IconMark::ErrorFilled,
            Self::Warning => IconMark::WarningFilled,
            Self::Info => IconMark::InformationFilled,
            Self::Success => IconMark::CheckmarkFilled,
        }
    }

    /// The kind as a word, for [`Semantics.value`] — the channel that
    /// survives both the hue and the picture.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Success => "success",
        }
    }
}

/// Toast notification at [`NotificationKind::Info`]. `title` is the
/// accessible name; `body` is the message. Neither channel is a hue.
pub fn notification(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    notification_toast(key, title, body)
}

/// Toast form: [`Role::Toast`] on [`Layer::Toast`], at
/// [`NotificationKind::Info`].
pub fn notification_toast(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    notification_toast_kind(key, NotificationKind::default(), title, body)
}

/// [`notification_toast`] with the status field named.
pub fn notification_toast_kind(
    key: impl Into<Key>,
    kind: NotificationKind,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    toast_surface(key, kind, title, body, None)
}

/// In-flow form: [`Role::Status`], not a floating surface, at
/// [`NotificationKind::Info`].
pub fn notification_inline(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    notification_inline_kind(key, NotificationKind::default(), title, body)
}

/// [`notification_inline`] with the status field named.
pub fn notification_inline_kind(
    key: impl Into<Key>,
    kind: NotificationKind,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    let title = title.into();
    let mut node = chrome(key, kind, title.clone(), body, Vec::new());
    node.constraints.vertical.min = Some(INLINE_MIN_BLOCK);
    node.constraints.horizontal.min = Some(INLINE_MIN_INLINE);
    node.constraints.horizontal.max = Some(INLINE_MAX_INLINE);
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(title),
        value: Some(kind.word().to_owned()),
        ..Semantics::default()
    };
    node
}

/// Toast with one labelled action button (Carbon Actionable / Toast
/// pairing), at [`NotificationKind::Info`].
pub fn notification_actionable(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
    action: impl Into<String>,
) -> ViewNode {
    notification_actionable_kind(key, NotificationKind::default(), title, body, action)
}

/// [`notification_actionable`] with the status field named.
pub fn notification_actionable_kind(
    key: impl Into<Key>,
    kind: NotificationKind,
    title: impl Into<String>,
    body: impl Into<String>,
    action: impl Into<String>,
) -> ViewNode {
    toast_surface(key, kind, title, body, Some(action.into()))
}

fn toast_surface(
    key: impl Into<Key>,
    kind: NotificationKind,
    title: impl Into<String>,
    body: impl Into<String>,
    action: Option<String>,
) -> ViewNode {
    let title = title.into();
    let extras = match action {
        Some(label) => vec![action_button("action", label)],
        None => Vec::new(),
    };
    let inner = chrome("panel", kind, title.clone(), body, extras);
    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Toast),
            // Top-trailing, 16 off both edges, which is where Carbon's toast
            // region is and where every product puts one. `Anchor::Viewport`
            // stood here until 2026-09-05 and it has exactly one reading —
            // the centre of the window — so every consumer of this
            // constructor got a toast dead centre of the page, over whatever
            // it was notifying about. See
            // `contracts/anchored-placement.md` §3a.
            anchor: Some(Anchor::ViewportEdge {
                edge: Edge::Top,
                align: crate::tree::Align::End,
                offset: Some(t(TOAST_DOCK)),
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::Passthrough),
            ..Props::default()
        })
        .child(inner);
    node.constraints.horizontal = AxisConstraint {
        min: Some(TOAST_INLINE),
        max: Some(TOAST_INLINE),
        priority: 0,
    };
    node.semantics = Semantics {
        role: Some(Role::Toast),
        label: Some(title),
        value: Some(kind.word().to_owned()),
        ..Semantics::default()
    };
    node
}

/// The card: the glyph beside the title on one line, the body under it,
/// then any extras, every line centred, on the raised surface with its
/// shadow.
fn chrome(
    key: impl Into<Key>,
    kind: NotificationKind,
    title: String,
    body: impl Into<String>,
    extras: Vec<ViewNode>,
) -> ViewNode {
    // Carbon's `NotificationIcon` renders at `size: 20`, which is
    // [`IconBox::Header`]'s extent. `IconTone::Primary` is `$icon-primary`,
    // not the kind's hue — see the module doc.
    let glyph = icon_in("glyph", kind.glyph(), IconBox::Header, IconTone::Primary);
    let mut heading = text("title", title);
    heading.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    // Glyph and title share one line, the glyph leading, separated by
    // Carbon's own `margin-inline-end: 16px` / `$spacing-05` — MEASURED
    // slice-c Notification, "Inline/Actionable ... icon `margin-inline-end`:
    // 16px/`$spacing-05`". The pair is a row, and the row is what the card
    // centres, so both the operator's centring and Carbon's leading glyph
    // hold at once.
    let mut head = stack(
        "head",
        Axis::Horizontal,
        Some(SPACING_05),
        vec![glyph, heading],
    );
    head.props.align = Some(Align::Center);
    let mut copy = vec![head, text("body", body.into())];
    copy.extend(extras);
    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), copy);
    // Cross-axis centre: each line sits in the middle of the card's width.
    // The padding is symmetric, so the block is centred top to bottom too.
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_04));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // No `border` token: neither `_toast-notification.scss` nor
    // `_inline-notification.scss` (per the slice-c ground truth) documents
    // any edge but the `border-left: 3px` rail, and the rail is gone by the
    // operator's decision. The card is a container, not a control, so it
    // takes a tone (`SURFACE_RAISED`) and `SHADOW_RAISED`, not an edge.
    node.props.tokens.insert("radius".into(), t(SHAPE_SM));
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    node
}

fn action_button(key: impl Into<Key>, label: String) -> ViewNode {
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    // Resting background: the panel it sits on (`chrome` binds
    // `SURFACE_RAISED`). Without this, `background@hover` has no resting
    // `background` beneath it and resolves to nothing at rest — the
    // Accordion/Modal/AI-label/data-table/date-picker defect, generalised
    // (see `a_state_decorated_token_always_has_a_resting_binding`).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.interactive(Role::Button, label, ACTION_INTENTS)
}

#[cfg(test)]
mod tests {
    use super::{
        INLINE_MAX_INLINE, INLINE_MIN_BLOCK, INLINE_MIN_INLINE, TOAST_INLINE, notification,
        notification_actionable, notification_inline, notification_toast,
    };
    use crate::component::disabled;
    use crate::component::tokens::{LAYER_HOVER, SURFACE_RAISED, TYPOGRAPHY_HEADING_SM};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{InputPolicy, Interaction, Layer, NodeKind, Props, Registry, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|c| descendant(c, key))
    }

    fn found<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        descendant(node, key).unwrap_or_else(|| panic!("missing descendant {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    #[test]
    fn notification_is_a_labelled_toast_not_a_dialog() {
        let node = notification("restart", "Supervisor restarted", "Worker 3 came back.");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Toast));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_ne!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(
            node.semantics.label.as_deref(),
            Some("Supervisor restarted")
        );
        assert_eq!(node.props.layer, Some(Layer::Toast));
        assert_eq!(node.props.input_policy, Some(InputPolicy::Passthrough));
        assert_eq!(node.constraints.horizontal.max, Some(TOAST_INLINE));
        assert_eq!(TOAST_INLINE, 288.0);
        assert_eq!(
            found(&node, "title").props.text.as_deref(),
            Some("Supervisor restarted")
        );
        assert_eq!(
            found(&node, "body").props.text.as_deref(),
            Some("Worker 3 came back.")
        );
    }

    #[test]
    fn notification_toast_matches_notification() {
        let a = notification("n", "Title", "Body");
        let b = notification_toast("n", "Title", "Body");
        assert_eq!(a.semantics.role, b.semantics.role);
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.props.layer, b.props.layer);
    }

    #[test]
    fn notification_inline_is_status_in_flow() {
        let node = notification_inline("warn", "Disk filling", "Trace volume is at 80%.");
        assert_ne!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Status));
        assert_eq!(node.semantics.label.as_deref(), Some("Disk filling"));
        assert_eq!(node.constraints.vertical.min, Some(INLINE_MIN_BLOCK));
        assert_eq!(INLINE_MIN_BLOCK, 48.0);
        assert_eq!(
            found(&node, "title").props.text.as_deref(),
            Some("Disk filling")
        );
    }

    /// Row 21, the operator's decision made twice: no rail, and the text
    /// centred. The card is one vertical stack of title over body with the
    /// cross axis centred, so there is no column beside the text for a rail
    /// to have lived in.
    #[test]
    fn the_card_has_no_rail_and_centres_its_text() {
        for node in [
            notification("restart", "Supervisor restarted", "Worker 3 came back."),
            notification_inline("warn", "Disk filling", "Trace volume is at 80%."),
            notification_actionable("act", "Update available", "A new build is ready.", "Reload"),
        ] {
            assert!(
                descendant(&node, "rail").is_none(),
                "the accent rail was removed by the operator's decision"
            );
            let card = if node.kind == NodeKind::Surface {
                child(&node, "panel")
            } else {
                &node
            };
            assert_eq!(card.props.axis, Some(Axis::Vertical));
            assert_eq!(
                card.props.align,
                Some(Align::Center),
                "title and body sit in the middle of the card"
            );
            assert_eq!(
                card.children[0].key.as_str(),
                "head",
                "the glyph-and-title row is the card's first line"
            );
            let head = &card.children[0];
            assert_eq!(
                head.props.axis,
                Some(Axis::Horizontal),
                "glyph and title share one line"
            );
            assert_eq!(
                head.children[0].key.as_str(),
                "glyph",
                "the status glyph leads the title, as Carbon places it"
            );
            assert_eq!(head.children[1].key.as_str(), "title");
            assert_eq!(card.children[1].key.as_str(), "body");
            assert_eq!(
                found(&node, "title")
                    .props
                    .style
                    .as_ref()
                    .map(|s| s.as_str()),
                Some(TYPOGRAPHY_HEADING_SM),
                "Carbon's title is heading-compact-01, not the page heading"
            );
            assert!(
                !card.props.tokens.contains_key("border"),
                "no outline: Carbon specs none past the rail"
            );
        }
    }

    /// Carbon floors an inline notification at `min-inline-size: 288px` and
    /// caps it at the `md` step of the ramp, so a stretched column of them is
    /// one column and not a staircase. Round 3 saw the staircase: each card
    /// had shrink-wrapped its own sentence.
    #[test]
    fn every_inline_notification_holds_carbons_width_band() {
        let short = notification_inline("a", "Ok", "Done.");
        let long = notification_inline(
            "b",
            "Rebuild failed",
            "Fiber 7 did not come back and the trace volume is at 80 percent.",
        );
        for node in [&short, &long] {
            assert_eq!(
                node.constraints.horizontal.min,
                Some(INLINE_MIN_INLINE),
                "the floor does not depend on the sentence"
            );
            assert_eq!(
                node.constraints.horizontal.max,
                Some(INLINE_MAX_INLINE),
                "nor does the ceiling"
            );
        }
        let frame = petrify_stretched(vec![short, long]);
        let width = |id: &str| frame.placement(id).map(|p| p.rect.w);
        assert_eq!(
            width("/root/a"),
            width("/root/b"),
            "two inline cards in one stretched column draw the same width"
        );
        assert!(
            width("/root/a").is_some_and(|w| (INLINE_MIN_INLINE..=INLINE_MAX_INLINE).contains(&w)),
            "and that width is inside Carbon's band, got {:?}",
            width("/root/a")
        );
        assert!(
            width("/root/a").is_some_and(|w| w < VIEWPORT.w),
            "the ceiling stops a card running the whole page"
        );
    }

    #[test]
    fn notification_does_not_paint_carbon_alert_red_or_green() {
        let node = notification("restart", "Supervisor restarted", "Worker 3 came back.");
        assert!(
            found(&node, "title").props.text.is_some(),
            "title is the non-colour channel"
        );
        let slots: Vec<&str> = node
            .props
            .tokens
            .values()
            .map(|n| n.as_str())
            .chain(
                child(&node, "panel")
                    .props
                    .tokens
                    .values()
                    .map(|n| n.as_str()),
            )
            .collect();
        for name in &slots {
            assert!(
                !name.contains("error") && !name.contains("success") && !name.contains("warning"),
                "status colours stay Petra's, got {name}"
            );
        }
    }

    #[test]
    fn notification_actionable_carries_a_labelled_button() {
        let node = notification_actionable(
            "restart",
            "Supervisor restarted",
            "Worker 3 came back.",
            "Open",
        );
        assert_eq!(node.semantics.role, Some(Role::Toast));
        let action = found(&node, "action");
        assert_eq!(action.semantics.role, Some(Role::Button));
        assert_eq!(action.semantics.label.as_deref(), Some("Open"));
        assert_eq!(child(action, "label").props.text.as_deref(), Some("Open"));
        assert!(action.interactions.contains(&Interaction::Click));
        assert!(action.interactions.contains(&Interaction::Focus));
        assert_eq!(token(action, "background@hover"), Some(LAYER_HOVER));
        assert_eq!(
            token(action, "background"),
            Some(SURFACE_RAISED),
            "a resting `background` must be bound alongside `background@hover`, \
             or the action button paints nothing when it is not hovered — the \
             paint pass counts that as silent, not empty"
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        petrify_all(vec![node])
    }

    fn petrify_stretched(nodes: Vec<ViewNode>) -> PetrifiedFrame {
        petrify_column(nodes, Some(Align::Stretch))
    }

    fn petrify_all(nodes: Vec<ViewNode>) -> PetrifiedFrame {
        petrify_column(nodes, None)
    }

    fn petrify_column(nodes: Vec<ViewNode>, align: Option<Align>) -> PetrifiedFrame {
        let mut root = ViewNode::new(NodeKind::Stack, "root").with_props(Props {
            axis: Some(Axis::Vertical),
            align,
            ..Props::default()
        });
        for node in nodes {
            root = root.child(node);
        }
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

    fn check_geometry(frame: &PetrifiedFrame) {
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

    /// Check C/D: toast, inline, and actionable forms all place with real
    /// rects, none of their parts outside their parent — and the placed
    /// title's centre is the card's centre, which is the operator's ask
    /// measured on the frame rather than read off a flag.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(notification_toast(
            "restart",
            "Supervisor restarted",
            "Worker 3 came back.",
        ));
        check_geometry(&frame);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending in {suffix}"))
                .rect
        };
        let panel = rect("/panel");
        assert_eq!(panel.w, TOAST_INLINE, "the card fills the toast's 288");
        // The head row — glyph then title — is what centres, not the title
        // on its own: the glyph leads the title inside the row, so the title
        // is deliberately right of centre by half the glyph and its gap.
        for key in ["/head", "/body"] {
            let line = rect(key);
            let off = (line.x + line.w / 2.0) - (panel.x + panel.w / 2.0);
            assert!(
                off.abs() <= 0.5,
                "{key} centre is {off} off the card's centre: {line:?} in {panel:?}"
            );
        }
        let (glyph, title) = (rect("/glyph"), rect("/title"));
        assert!(
            glyph.x + glyph.w <= title.x,
            "the glyph leads the title on the same line, got {glyph:?} then {title:?}"
        );
        let head = rect("/head");
        let share = |r: crate::geom::Rect| (r.y + r.h / 2.0) - (head.y + head.h / 2.0);
        assert!(
            share(glyph).abs() <= 0.5 && share(title).abs() <= 0.5,
            "glyph and title share the row's centre line, got {} and {}",
            share(glyph),
            share(title)
        );
        check_geometry(&petrify_lone(notification_inline(
            "warn",
            "Disk filling",
            "Trace volume is at 80%.",
        )));
        check_geometry(&petrify_lone(notification_actionable(
            "action",
            "Update available",
            "A new build is ready.",
            "Reload",
        )));
    }

    /// Check F: the actionable form's action button is reachable, and a
    /// disabled one is not. Toast and inline forms carry no action, so
    /// there is nothing to reach — the shell itself is not a click target
    /// (`notification_is_a_labelled_toast_not_a_dialog` above).
    #[test]
    fn the_action_button_is_reachable_unless_disabled() {
        // The surface's own key is "toast" here, distinct from the action
        // button's fixed child key "action" (`action_button("action", ..)`
        // inside `chrome`). Reusing "action" for the surface key, as an
        // earlier draft of this test did, makes `ends_with("/action")`
        // match the surface placement (`/root/action`) before the nested
        // button (`/root/action/panel/action`) — both end with `/action` —
        // so the button never gets checked at all.
        let frame = petrify_lone(notification_actionable(
            "toast",
            "Update available",
            "A new build is ready.",
            "Reload",
        ));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let action = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/panel/action"))
            .expect("the action button is placed");
        assert!(
            order.iter().any(|o| o == &action.id),
            "the action button declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(disabled(notification_actionable(
            "toast",
            "Update available",
            "A new build is ready.",
            "Reload",
        )));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_action = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/panel/action"))
            .expect("the disabled action button is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_action.id),
            "a disabled action button must not be reachable"
        );
    }

    /// Check E: title, body and the action label against the card's own
    /// fill (`chrome` binds `SURFACE_RAISED`), in both themes.
    #[test]
    fn card_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = notification_actionable(
                "action",
                "Update available",
                "A new build is ready.",
                "Reload",
            );
            let shell_bg = color(&theme, SURFACE_RAISED);
            fn walk_text(
                node: &ViewNode,
                inherited_bg: ColorValue,
                theme: &Theme,
                min: f32,
                get_color: &impl Fn(&Theme, &str) -> ColorValue,
            ) {
                let bg = match node.props.tokens.get("background") {
                    Some(name) => get_color(theme, name.as_str()),
                    None => inherited_bg,
                };
                if node.props.text.is_some()
                    && let Some(fg_name) = node.props.tokens.get("foreground")
                {
                    let opacity = node.props.opacity.unwrap_or(1.0);
                    let fg = get_color(theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= min,
                        "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                        node.key,
                        fg_name.as_str()
                    );
                }
                for child in &node.children {
                    walk_text(child, bg, theme, min, get_color);
                }
            }
            walk_text(&node, shell_bg, &theme, MIN_TEXT_CONTRAST, &color);
        }
    }
}
