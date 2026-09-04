//! Carbon Popover (slice-d). Positioning primitive other overlays compose.
//!
//! Anatomy (`_popover.scss` + usage page):
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Popup`], anchored to a
//!    trigger node. Carbon's popover is not a modal, so this is
//!    [`Role::Overlay`] (not [`Role::Dialog`]) and
//!    [`InputPolicy::DismissOutside`] (not a focus trap).
//! 2. Content box — fill [`SURFACE_RAISED`], edge [`BORDER_SUBTLE`],
//!    padding [`SPACING_05`]. Grows to content, capped at 368 (T070).
//! 3. Caret — the word `"^"`. A second channel beside the fill, never an
//!    icon-only mark (FR-026). [`IconMark`] has no caret; a triangle canvas
//!    would duplicate the engine caret `overlay_surface` already paints from
//!    the resolved side.
//!
//! [`ClampRule::Flip`] is the `--auto-align` ladder. Interactive contents
//! are the caller's children (FR-058 on those children, not on this node).

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{BORDER_SUBTLE, SPACING_03, SPACING_05, SURFACE_RAISED, t};
use crate::geom::Axis;
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, InputPolicy, Key, Layer, NodeKind,
    Props, Role, Semantics, ViewNode,
};

/// Carbon `.cds--popover-content` `max-inline-size`. T070 prefers SCSS
/// (`_popover.scss:217`) over the style-page 352.
const MAX_INLINE: f32 = 368.0;

const _: () = assert!(MAX_INLINE == 368.0);

/// An anchored popover whose body is one text run.
///
/// `label` is the accessible name (required). `anchor_id` is the semantic
/// id of the trigger node [`Anchor::Node`] names. The popover itself is
/// not a click target.
pub fn popover(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    body: impl Into<String>,
) -> ViewNode {
    popover_with(key, label, anchor_id, vec![text("body", body.into())])
}

/// An anchored popover whose body is caller-supplied children.
///
/// Same chrome as [`popover`]: raised fill, subtle border, `"^"` caret,
/// [`Role::Overlay`]. The children keep whatever roles and labels they
/// already carry.
pub fn popover_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    children: Vec<ViewNode>,
) -> ViewNode {
    let mut rows = Vec::with_capacity(children.len() + 1);
    rows.push(text("caret", "^"));
    rows.extend(children);
    let mut content = stack("content", Axis::Vertical, Some(SPACING_03), rows);
    content.props.align = Some(crate::geom::Align::Center);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Node {
                id: anchor_id.into(),
                edge: Edge::Bottom,
                align: Align::Center,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            padding: Some(pad(SPACING_05, SPACING_05)),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.constraints = Constraints {
        horizontal: AxisConstraint {
            min: None,
            max: Some(MAX_INLINE),
            priority: 0,
        },
        ..Constraints::default()
    };
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::{MAX_INLINE, popover, popover_with};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, SPACING_05, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Align, Anchor, ClampRule, Edge, InputPolicy, Layer, NodeKind, Props, Registry, Role,
        ViewNode,
    };

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

    fn padding_token(node: &ViewNode) -> Option<(&str, &str)> {
        let pad = node.props.padding.as_ref()?;
        Some((pad.left.as_ref()?.as_str(), pad.top.as_ref()?.as_str()))
    }

    #[test]
    fn popover_is_an_anchored_overlay_not_a_dialog() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Filter help"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.props.layer, Some(Layer::Popup));
        assert_eq!(node.props.clamp, Some(ClampRule::Flip));
        assert_eq!(node.props.input_policy, Some(InputPolicy::DismissOutside));
        match &node.props.anchor {
            Some(Anchor::Node {
                id,
                edge,
                align,
                offset,
            }) => {
                assert_eq!(id, "filter-btn");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(*align, Align::Center);
                assert!(offset.is_none());
            }
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert_eq!(padding_token(&node), Some((SPACING_05, SPACING_05)));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 368.0);
    }

    #[test]
    fn popover_caret_is_a_second_channel_not_icon_only() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
        let content = child(&node, "content");
        let caret = child(content, "caret");
        assert_eq!(caret.props.text.as_deref(), Some("^"));
        assert!(
            caret.semantics.role.is_none(),
            "caret is a visual part, not a control"
        );
        assert!(caret.interactions.is_empty());
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Narrow the list.")
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
    }

    #[test]
    fn popover_with_hosts_caller_children() {
        let node = popover_with("menu", "Actions", "more-btn", vec![text("item", "Rename")]);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_eq!(node.semantics.label.as_deref(), Some("Actions"));
        assert!(node.props.anchor.is_some());
        assert!(node.interactions.is_empty());
        let content = child(&node, "content");
        assert_eq!(child(content, "caret").props.text.as_deref(), Some("^"));
        assert_eq!(child(content, "item").props.text.as_deref(), Some("Rename"));
        assert!(
            child(content, "item").semantics.role.is_none(),
            "FR-058 stays on the caller's children; this node does not wrap them"
        );
    }

    // `popover`/`popover_with` themselves build `Anchor::Node { id:
    // anchor_id, .. }`, a bare local key rather than a full canonical
    // key-path. `crate::tree::validate::check_anchors` refuses the WHOLE
    // TREE ("Tree acceptance MUST refuse ... refuse the whole tree, never
    // a silent Anchor::Viewport fallback",
    // `contracts/anchored-placement.md` §2) whenever an `Anchor::Node.id`
    // does not resolve to some node's own canonical path — even a tree
    // holding only the popover under a plain root, since no sibling named
    // "filter-btn"/"more-btn" exists there either. So, unlike Modal and
    // Notification (`Anchor::Viewport`, which resolves with no reference
    // to any sibling), popover cannot be `petrify_lone`d at all in ANY
    // form — not even closed, because it has no closed form; every
    // constructor here IS the anchored surface (a known limit, see
    // `.agents/notes/proposed/architecture/
    // 2026-09-03-anchored-components-cannot-name-their-own-anchor.md`).
    //
    // What CAN be audited: `content`, the inner `Stack` `popover_with`
    // builds (`caret` + body/children). It carries no `anchor` of its own
    // — only the outer `Surface` node does — so it petrifies on its own,
    // the same way `date_picker`'s `day_button` and `menu`'s `menu_item`
    // are audited standalone inside an anchored parent's own module.

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

    /// Check C/D: the caret and the body text place with real rects,
    /// none of them outside `content`'s own rect.
    #[test]
    fn content_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
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

    // Check F does not apply: the caret is a visual part (`caret.semantics
    // .role.is_none()`, `popover_caret_is_a_second_channel_not_icon_only`
    // above) and the body is plain text — neither declares `Interaction
    // ::Focus`. FR-058 for whatever interactive children a caller supplies
    // is that caller's own component's job (`popover_with_hosts_caller_
    // children` above), not `content`'s.

    /// Check E: the caret and body text against [`SURFACE_RAISED`] — the
    /// resting `background` the outer `Surface` node binds, which
    /// `content` (audited standalone, above) would sit on once mounted —
    /// in both themes.
    #[test]
    fn content_text_clears_aa_contrast_against_the_surface_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
            let surface_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the surface binds a resting background");
            let surface_bg = color(&theme, surface_bg_name.as_str());
            let content = child(&node, "content");
            for label_key in ["caret", "body"] {
                let label = child(content, label_key);
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label text binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str())
                    .faded(opacity)
                    .over(surface_bg);
                let ratio = fg.contrast_ratio(surface_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label_key} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
