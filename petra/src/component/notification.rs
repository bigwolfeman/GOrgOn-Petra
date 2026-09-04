//! Carbon Notification (slice-c). Status colours stay Petra's.
//!
//! Anatomy (`_toast-notification.scss` / `_inline-notification.scss`):
//! 1. Accent rail — 3px [`ACCENT_PRIMARY`] bar. Chrome, not meaning.
//! 2. Title — the required name (and the non-colour channel).
//! 3. Body — the message.
//! 4. Optional action — a [`Role::Button`] with a label.
//!
//! Carbon's alert palette (`$notification-background-error` / success
//! green / warning yellow) is **not** used. Those names are not in
//! [`super::tokens`], and painting red/green would fail FR-015 for a
//! red-green colourblind operator. Meaning lives in the title and body;
//! the rail spends Petra's accent, the same hue [`super::primary_button`]
//! already spends. [`crate::token::StatusToken`] is not assembled here:
//! this constructor has no kind to pair with a shape.
//!
//! [`notification`] / [`notification_toast`] are a [`Role::Toast`]
//! surface. [`notification_inline`] is in-flow [`Role::Status`].

use super::pad;
use super::stack;
use super::text::{heading, text};
use super::tokens::{
    ACCENT_PRIMARY, LAYER_HOVER, SHADOW_RAISED, SHAPE_SM, SPACING_03, SPACING_04, SPACING_05,
    SURFACE_RAISED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    Anchor, AxisConstraint, ClampRule, InputPolicy, Interaction, Key, Layer, NodeKind, Props, Role,
    Semantics, ViewNode,
};

/// Carbon toast `inline-size` below the `max` breakpoint (`18rem`).
const TOAST_INLINE: f32 = 288.0;
/// Carbon inline/actionable `min-block-size` (`3rem`).
const INLINE_MIN_BLOCK: f32 = 48.0;
/// Carbon `border-left` on every form.
const RAIL: f32 = 3.0;

const _: () = assert!(TOAST_INLINE == 288.0);
const _: () = assert!(INLINE_MIN_BLOCK == 48.0);
const _: () = assert!(RAIL == 3.0);

const ACTION_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Toast notification. `title` is the accessible name; `body` is the
/// message. Neither channel is a hue.
pub fn notification(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    notification_toast(key, title, body)
}

/// Toast form: [`Role::Toast`] on [`Layer::Toast`].
pub fn notification_toast(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    toast_surface(key, title, body, None)
}

/// In-flow form: [`Role::Status`], not a floating surface.
pub fn notification_inline(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    let title = title.into();
    let mut node = chrome(key, title.clone(), body, Vec::new());
    node.constraints.vertical.min = Some(INLINE_MIN_BLOCK);
    node.semantics = Semantics {
        role: Some(Role::Status),
        label: Some(title),
        ..Semantics::default()
    };
    node
}

/// Toast with one labelled action button (Carbon Actionable / Toast pairing).
pub fn notification_actionable(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
    action: impl Into<String>,
) -> ViewNode {
    toast_surface(key, title, body, Some(action.into()))
}

fn toast_surface(
    key: impl Into<Key>,
    title: impl Into<String>,
    body: impl Into<String>,
    action: Option<String>,
) -> ViewNode {
    let title = title.into();
    let extras = match action {
        Some(label) => vec![action_button("action", label)],
        None => Vec::new(),
    };
    let inner = chrome("panel", title.clone(), body, extras);
    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Toast),
            anchor: Some(Anchor::Viewport),
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
        ..Semantics::default()
    };
    node
}

fn chrome(
    key: impl Into<Key>,
    title: String,
    body: impl Into<String>,
    extras: Vec<ViewNode>,
) -> ViewNode {
    let mut rail = stack("rail", Axis::Vertical, None, vec![]);
    rail.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    rail.constraints.horizontal = AxisConstraint {
        min: Some(RAIL),
        max: Some(RAIL),
        priority: 0,
    };

    let mut copy = vec![heading("title", title), text("body", body.into())];
    copy.extend(extras);
    let mut details = stack("details", Axis::Vertical, Some(SPACING_03), copy);
    details.props.padding = Some(pad(SPACING_05, SPACING_04));

    let mut node = stack(key, Axis::Horizontal, None, vec![rail, details]);
    // Cross-axis stretch, not `Align::Start`: `rail` is a childless swatch
    // with no intrinsic content of its own, the same shape as the Accordion
    // item divider (`accordion.rs`'s own module doc). Under `Align::Start`
    // it petrified at a **0px height** — declared paint content
    // (`background: accent.primary`) covering zero pixels, invisible even
    // though every `ViewNode`-level check (which never looks at a placed
    // rect) passed. `Align::Stretch` matches it to `details`'s full height,
    // the same fix `accordion_item_sized` applies to its own divider.
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // No `border` token: neither `_toast-notification.scss` nor
    // `_inline-notification.scss` (per the slice-c ground truth) documents
    // anything but the `border-left: 3px` accent rail, already drawn above
    // by `rail`'s own coloured bar. A full `border.subtle` outline around
    // the card is decoration Carbon does not spec, the same class as
    // `file_uploader_item`'s spurious border (T070: SCSS wins). The card is
    // a container, not a control, so it takes a tone (`SURFACE_RAISED`) and
    // `SHADOW_RAISED` below, not an edge.
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
        ACCENT_PRIMARY, INLINE_MIN_BLOCK, RAIL, TOAST_INLINE, notification,
        notification_actionable, notification_inline, notification_toast,
    };
    use crate::component::disabled;
    use crate::component::tokens::{LAYER_HOVER, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        InputPolicy, Interaction, Layer, NodeKind, Props, Registry, Role, ViewNode,
    };

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|c| walk(c, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("missing descendant {key}"))
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
            descendant(&node, "title").props.text.as_deref(),
            Some("Supervisor restarted")
        );
        assert_eq!(
            descendant(&node, "body").props.text.as_deref(),
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
            descendant(&node, "title").props.text.as_deref(),
            Some("Disk filling")
        );
    }

    #[test]
    fn notification_does_not_paint_carbon_alert_red_or_green() {
        let node = notification("restart", "Supervisor restarted", "Worker 3 came back.");
        let rail = descendant(&node, "rail");
        assert_eq!(token(rail, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(rail.constraints.horizontal.max, Some(RAIL));
        assert_eq!(RAIL, 3.0);
        assert!(
            descendant(&node, "title").props.text.is_some(),
            "title is the non-colour channel; the rail is chrome"
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
        let action = descendant(&node, "action");
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
    /// rects, none of their parts outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        check_geometry(&petrify_lone(notification_toast(
            "restart",
            "Supervisor restarted",
            "Worker 3 came back.",
        )));
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
        // button (`/root/action/panel/details/action`) — both end with
        // `/action` — so the button never gets checked at all.
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
            .find(|p| p.id.ends_with("/details/action"))
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
            .find(|p| p.id.ends_with("/details/action"))
            .expect("the disabled action button is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_action.id),
            "a disabled action button must not be reachable"
        );
    }

    /// Check E: title, body and the action label against the card's own
    /// fill (`chrome` binds `SURFACE_RAISED`; `rail` and `details` bind no
    /// background of their own, so text under them inherits it), in both
    /// themes.
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
                if node.props.text.is_some() {
                    if let Some(fg_name) = node.props.tokens.get("foreground") {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        let fg = get_color(theme, fg_name.as_str())
                            .faded(opacity)
                            .over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= min,
                            "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                            node.key,
                            fg_name.as_str()
                        );
                    }
                }
                for child in &node.children {
                    walk_text(child, bg, theme, min, get_color);
                }
            }
            walk_text(&node, shell_bg, &theme, MIN_TEXT_CONTRAST, &color);
        }
    }
}
