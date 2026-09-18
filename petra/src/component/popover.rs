//! Carbon Popover (slice-d). Positioning primitive other overlays compose.
//!
//! Anatomy (`_popover.scss` + usage page), the **caret tip** form:
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Popup`], anchored to a
//!    trigger node. Carbon's popover is not a modal, so this is
//!    [`Role::Overlay`] (not [`Role::Dialog`]) and
//!    [`InputPolicy::DismissOutside`] (not a focus trap).
//! 2. Content box — fill [`SURFACE_RAISED`] (Carbon `$layer`), elevation
//!    [`SHADOW_OVERLAY`] (`drop-shadow(0 4px 4px rgba(0,0,0,.2))`), corner
//!    `shape.corner-md` via [`crate::token::CornerRole::Floating`], padding
//!    [`SPACING_05`]. Carbon's own `$popover-border-radius` is 2px and was
//!    the role's first source; spec 009 T011 reassigned it on the operator's
//!    call of 2026-09-18, because Carbon is a reference here and not a
//!    conformance target, and because a popover is a surface floating free
//!    of the layout in exactly the way a menu and a modal are. All three now
//!    answer 8.
//!    No outline: Carbon draws one only under `--border`, and the shadow is
//!    what lifts the box off the page. Grows to content, capped at 368
//!    (T070). Content is start-aligned, as text in a box is.
//! 3. Caret — **engine-drawn**, from the resolved side
//!    (`contracts/anchored-placement.md` §5, `Tip::Caret`, the default).
//!    12 wide at the base; its depth is the anchor gap, [`SPACING_03`],
//!    which is Carbon's `--caret` offset (10px) at the nearest token. Until
//!    2026-09-04 this file also put the word `"^"` in the content box as a
//!    "second channel", on the belief that the engine's caret was not
//!    painted. It is, in the surface's own fill, and the word sat beside it
//!    as a second, wrong pointer. A caret is not a control and needs no
//!    text twin (FR-026 is about marks that *are* the control's state).
//!
//! [`ClampRule::Flip`] is the `--auto-align` ladder. Interactive contents
//! are the caller's children (FR-058 on those children, not on this node).
//!
//! The **no tip** form is not this file: a dropdown, select or menu opens a
//! `super::list_box`, which is flush, anchor-fitted and unpadded. Building
//! those on this constructor is what gave every list box a beak.

use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{SHADOW_OVERLAY, SPACING_03, SPACING_05, SURFACE_RAISED, t};
use crate::geom::Axis;
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, InputPolicy, Key, Layer, NodeKind,
    Props, Role, Semantics, TextWrap, ViewNode,
};

/// Carbon `.cds--popover-content` `max-inline-size`. T070 prefers SCSS
/// (`_popover.scss:217`) over the style-page 352.
const MAX_INLINE: f32 = 368.0;

const _: () = assert!(MAX_INLINE == 368.0);

/// The popover body has no declared minimum height — it grows to
/// `children` — so this is a floor derived from the one thing that is
/// fixed: `pad(SPACING_05, SPACING_05)` puts 16 units of padding above and
/// below the content, for 32 with zero-height content.
/// [`crate::token::CornerRole::Floating`]'s half-edge clause only fires at
/// or below 16 units, so any real popover clears it comfortably.
const HEIGHT_FLOOR: f32 = 32.0;

/// An anchored popover whose body is one text run.
///
/// `label` is the accessible name (required). `anchor` is the key of the
/// trigger node, which must sit in the same child list as this popover:
/// the anchor is an [`Anchor::Sibling`], resolved against wherever the
/// caller mounts the pair, so this constructor never needs to know its own
/// canonical id. The popover itself is not a click target.
pub fn popover(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor: impl Into<Key>,
    body: impl Into<String>,
) -> ViewNode {
    popover_with(key, label, anchor, vec![bubble_text(body.into())])
}

/// The one body run all four anchored bubbles in this library share.
///
/// Used by [`popover`], [`super::toggletip`], [`super::tooltip`] and
/// [`super::ai_label`]'s explainability panel, because all four are the same
/// bubble with different tones and they have drifted from each other before.
///
/// # The `wrap` line is not what makes a bubble wrap
///
/// It reads like it is, and the note that introduced this function said it
/// was. It is not. [`crate::tree::TextWrap::Wrap`] is `TextWrap`'s
/// `#[default]` and `Props::text()` resolves an unset `wrap` through
/// `unwrap_or_default()`, so [`super::text::text`] already answers a finite
/// offer by wrapping. Writing the value down is worth it for the reader —
/// a bubble body that silently changed policy would be a real regression —
/// but it changes nothing on its own.
///
/// What makes a bubble wrap is the offer it is measured under.
/// `layout::overlay_surface::natural_size` hands the children the surface's
/// declared `constraints.horizontal.max`, less the surface's own padding,
/// instead of an open probe. Before that a run answered with one unbroken
/// line, the box was clamped to the ceiling afterwards, and the tail of the
/// sentence was cut mid-word.
pub(super) fn bubble_text(body: String) -> ViewNode {
    let mut node = text("body", body);
    node.props.wrap = Some(TextWrap::Wrap);
    node
}

/// An anchored popover whose body is caller-supplied children.
///
/// Same chrome as [`popover`]: raised fill, overlay shadow, 2-unit corner,
/// engine caret, [`Role::Overlay`]. The children keep whatever roles and
/// labels they already carry. `anchor` is the trigger's sibling key, as for
/// [`popover`].
///
/// Placement is the engine's default: [`Edge::Bottom`], [`Align::Center`].
/// A caller that needs one of the other eleven of Carbon's twelve placements
/// (`_popover.scss`'s `--top`/`--bottom`/`--left`/`--right`, each crossed
/// with `-start`/(none)/`-end`) wants [`popover_with_placement`], which this
/// calls with that default so every existing caller of this constructor
/// keeps its current bubble unchanged.
pub fn popover_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor: impl Into<Key>,
    children: Vec<ViewNode>,
) -> ViewNode {
    popover_with_placement(key, label, anchor, Edge::Bottom, Align::Center, children)
}

/// [`popover_with`], with the anchor side and cross-axis alignment as
/// parameters instead of the engine's default.
///
/// `edge` and `align` are the same pair `tree::props` resolves an
/// [`Anchor::Sibling`] with (`tree/props.rs:313-320,585-596`): 4 edges × 3
/// aligns is exactly Carbon's twelve named placements, `--<edge>`,
/// `--<edge>-start` and `--<edge>-end`. Carbon's `-left`/`-right` are
/// deprecated aliases for `-start`/`-end` and are not ported
/// (`contracts/anchored-placement.md` §3); this crate has one name per
/// placement, not two.
pub fn popover_with_placement(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor: impl Into<Key>,
    edge: Edge,
    align: Align,
    children: Vec<ViewNode>,
) -> ViewNode {
    let content = stack("content", Axis::Vertical, Some(SPACING_03), children);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: anchor.into(),
                edge,
                align,
                // The caret's depth: the engine draws it as deep as the gap
                // (`overlay_surface::caret_of`), so this is both the air
                // under the trigger and the size of the pointer in it.
                offset: Some(t(SPACING_03)),
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
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    // FR-022: the enum's own doc names "a popover" as a
    // `CornerRole::Floating` example, beside a menu and a modal. It also
    // named it under `BoxedMark` until spec 009 T011; that entry is gone,
    // because one component may not be the example for two roles.
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(CornerRole::Floating, HEIGHT_FLOOR)),
    );
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
    use super::{MAX_INLINE, popover, popover_with, popover_with_placement};
    use crate::component::text::text;
    use crate::component::tokens::{
        SHADOW_OVERLAY, SHAPE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    };
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
            Some(Anchor::Sibling {
                key,
                edge,
                align,
                offset,
            }) => {
                assert_eq!(key.as_str(), "filter-btn");
                assert_eq!(*edge, Edge::Bottom);
                assert_eq!(*align, Align::Center);
                assert_eq!(
                    offset.as_ref().map(|t| t.as_str()),
                    Some(SPACING_03),
                    "the gap under the trigger is the caret's depth"
                );
            }
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "shadow"), Some(SHADOW_OVERLAY));
        assert_eq!(
            token(&node, "radius"),
            Some(SHAPE_MD),
            "spec 009 T011: a popover floats free of the layout the way a \
             menu and a modal do, so all three answer `CornerRole::Floating`"
        );
        assert_eq!(
            token(&node, "border"),
            None,
            "Carbon's popover has no outline unless `--border` asks for one"
        );
        assert_eq!(
            node.props.tip, None,
            "absent is `Tip::Caret`: the engine draws the pointer"
        );
        assert_eq!(padding_token(&node), Some((SPACING_05, SPACING_05)));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MAX_INLINE, 368.0);
    }

    /// The caret is the engine's, and nothing in the content box stands in
    /// for it. Falsify by pushing `text("caret", "^")` back onto the rows.
    #[test]
    fn the_content_box_holds_the_body_and_no_caret_word() {
        let node = popover("help", "Filter help", "filter-btn", "Narrow the list.");
        let content = child(&node, "content");
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "the `^` word is back beside the engine's own caret"
        );
        assert!(
            content
                .children
                .iter()
                .all(|c| c.props.text.as_deref() != Some("^")),
            "no text node in the content spells a caret"
        );
        assert_eq!(
            content.props.align, None,
            "content is start-aligned, as text in a box is"
        );
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
        assert_eq!(
            content.children.len(),
            1,
            "the caller's child and nothing else"
        );
        assert_eq!(child(content, "item").props.text.as_deref(), Some("Rename"));
        assert!(
            child(content, "item").semantics.role.is_none(),
            "FR-058 stays on the caller's children; this node does not wrap them"
        );
    }

    /// `popover` and `popover_with` build an `Anchor::Sibling` naming the
    /// trigger by the bare key the caller passed. Placed beside a control
    /// carrying that key, either is accepted wherever the caller mounts the
    /// pair — here two containers below the root, the gallery catalog's own
    /// depth. This is the root-cause fix for every anchored component in
    /// this crate, since they all build on these two.
    #[test]
    fn popovers_validate_beside_their_trigger_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "popover",
            vec![
                crate::component::button("filter-btn", "Filter"),
                popover("help", "Filter help", "filter-btn", "Narrow the list."),
            ],
        );
        crate::component::tests::assert_mounts_at_catalog_depth(
            "popover_with",
            vec![
                crate::component::button("more-btn", "More"),
                popover_with("menu", "Actions", "more-btn", vec![text("item", "Rename")]),
            ],
        );
    }

    /// The other half of the sibling contract: a key no sibling carries is
    /// still refused, and the refusal names the one canonical id it looked
    /// for — the caller's child list plus the key — so the author can see
    /// which list the trigger was missing from.
    #[test]
    fn a_popover_whose_trigger_is_not_beside_it_is_refused_by_name() {
        use crate::tree::{Registry, Violation, validate};
        let tree = crate::component::tests::mounted_like_the_catalog(vec![popover(
            "help",
            "Filter help",
            "filter-btn",
            "Narrow the list.",
        )]);
        let err = validate(&tree, &Registry::with_vocabulary(standard_vocabulary())).unwrap_err();
        let missing = err
            .as_slice()
            .iter()
            .find_map(|e| match &e.violation {
                Violation::AnchorTargetMissing { id, .. } => Some((e.path.as_str(), id.as_str())),
                _ => None,
            })
            .expect("a sibling key naming nothing is AnchorTargetMissing");
        assert_eq!(
            missing,
            ("/root/page/body/help", "/root/page/body/filter-btn"),
            "the refusal names the surface and the exact sibling id it tried"
        );
    }

    /// T0.3's whole claim: each of the engine's twelve placements resolves
    /// the *surface's rect*, not just an enum a caller can round-trip. A
    /// test that only checked `node.props.anchor == Some(Anchor::Sibling {
    /// edge, align, .. })` would pass even if the layout pass ignored both
    /// fields, because that struct is built by the same line that reads
    /// them. This places a real trigger and a real bubble through
    /// [`crate::layout::place`] and checks the two rects against each
    /// other, the same way `overlay_surface`'s own edge/align tests do for
    /// the engine primitive underneath.
    #[test]
    fn every_placement_lands_on_the_edge_and_align_it_asked_for() {
        use crate::frame::placement::PlacementList;
        use crate::geom::Rect;
        use crate::layout::Slot;
        use crate::tree::KeyPath;

        let control = Size::new(100.0, 40.0);
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        // Centred with room on all four sides, so no placement in the
        // sweep below needs `ClampRule::Flip` to fit — this test is about
        // which side and alignment were asked for, not about the flip
        // ladder (that is `overlay_surface`'s own coverage).
        let anchor_origin = (300.0_f32, 280.0_f32);

        let mut harness = Harness::new();
        let gap = harness
            .theme
            .spacing(&TokenName::new(SPACING_03).unwrap())
            .expect("SPACING_03 is a spacing token in the fixture theme");

        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            for align in [Align::Start, Align::Center, Align::End] {
                let mut pinned = ViewNode::new(NodeKind::Spacer, "box");
                pinned.constraints.horizontal.min = Some(control.w);
                pinned.constraints.horizontal.max = Some(control.w);
                pinned.constraints.vertical.min = Some(control.h);
                pinned.constraints.vertical.max = Some(control.h);
                // A `Surface` pinned by `Anchor::Point` so its placed rect
                // is a known constant, not something this test would have
                // to trust the box layout to reproduce.
                let trigger = ViewNode::new(NodeKind::Surface, "trigger")
                    .with_props(Props {
                        layer: Some(Layer::Popup),
                        anchor: Some(Anchor::Point {
                            x: anchor_origin.0,
                            y: anchor_origin.1,
                        }),
                        clamp: Some(ClampRule::Shrink),
                        input_policy: Some(InputPolicy::Block),
                        ..Props::default()
                    })
                    .child(pinned);
                let bubble = popover_with_placement(
                    "pop",
                    "Filter help",
                    "trigger",
                    edge,
                    align,
                    vec![text("item", "Narrow the list.")],
                );
                let root = ViewNode::new(NodeKind::Overlay, "root")
                    .child(trigger)
                    .child(bubble);

                let mut ctx = harness.ctx();
                let mut path = KeyPath::root();
                let mut sink = PlacementList::new();
                crate::layout::place(&root, &mut ctx, &mut path, Slot::new(viewport), &mut sink);
                let parts = sink.into_parts();
                let find = |id: &str| {
                    parts
                        .placements
                        .iter()
                        .find(|p| p.id == id)
                        .unwrap_or_else(|| panic!("{id} was not placed for {edge:?}/{align:?}"))
                };
                let button = find("/root/trigger");
                let surface = find("/root/pop");

                assert_eq!(
                    button.rect,
                    Rect::new(anchor_origin.0, anchor_origin.1, control.w, control.h),
                    "the trigger's own pinned rect moved — the test fixture is broken, \
                     not the placement under test"
                );

                match edge {
                    Edge::Bottom => assert_eq!(
                        surface.rect.y,
                        button.rect.bottom() + gap,
                        "{align:?}: bottom did not sit under the trigger"
                    ),
                    Edge::Top => assert_eq!(
                        surface.rect.bottom(),
                        button.rect.y - gap,
                        "{align:?}: top did not sit above the trigger"
                    ),
                    Edge::Right => assert_eq!(
                        surface.rect.x,
                        button.rect.right() + gap,
                        "{align:?}: right did not sit beside the trigger"
                    ),
                    Edge::Left => assert_eq!(
                        surface.rect.right(),
                        button.rect.x - gap,
                        "{align:?}: left did not sit beside the trigger"
                    ),
                }

                match edge {
                    Edge::Top | Edge::Bottom => match align {
                        Align::Start => assert_eq!(
                            surface.rect.x, button.rect.x,
                            "{edge:?}/start did not flush the anchor's leading corner"
                        ),
                        Align::Center => assert_eq!(
                            surface.rect.x,
                            button.rect.x + (button.rect.w - surface.rect.w) / 2.0,
                            "{edge:?}/center did not centre on the anchor"
                        ),
                        Align::End => assert_eq!(
                            surface.rect.x + surface.rect.w,
                            button.rect.x + button.rect.w,
                            "{edge:?}/end did not flush the anchor's trailing corner"
                        ),
                    },
                    Edge::Left | Edge::Right => match align {
                        Align::Start => assert_eq!(
                            surface.rect.y, button.rect.y,
                            "{edge:?}/start did not flush the anchor's leading corner"
                        ),
                        Align::Center => assert_eq!(
                            surface.rect.y,
                            button.rect.y + (button.rect.h - surface.rect.h) / 2.0,
                            "{edge:?}/center did not centre on the anchor"
                        ),
                        Align::End => assert_eq!(
                            surface.rect.y + surface.rect.h,
                            button.rect.y + button.rect.h,
                            "{edge:?}/end did not flush the anchor's trailing corner"
                        ),
                    },
                }
            }
        }
    }

    // Every constructor here IS the anchored surface — there is no closed
    // form — so the frame-level checks below audit `content`, the inner
    // `Stack` `popover_with` builds (body/children). It carries no
    // `anchor` of its own — only the outer `Surface` node does — so it
    // petrifies on its own, the same way `date_picker`'s `day_button` and
    // `menu`'s `menu_item` are audited standalone inside an anchored
    // parent's own module.

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

    // Check F does not apply: the body is plain text and declares no
    // `Interaction::Focus`. FR-058 for whatever interactive children a
    // caller supplies is that caller's own component's job
    // (`popover_with_hosts_caller_children` above), not `content`'s.

    /// Check E: the body text against [`SURFACE_RAISED`] — the
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
            let label_key = "body";

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
