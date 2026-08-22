//! Overlay surfaces: anchoring, clamping, and input policy.
//!
//! A `surface` node floats free of ordinary flow: unlike a stack or a grid, it
//! does not take the rect its layout parent offers. It always sizes itself to
//! its own natural content extent (an "open probe" — every child measured
//! with [`Proposal::Unspecified`], the union taken), positions that box at its
//! declared [`Anchor`], and then clamps the result into the window by its
//! [`ClampRule`] so it never paints outside the viewport (FR-022). This is why
//! [`measure`] ignores the `proposal` it is handed: a floating surface's
//! answer to "how big are you" does not depend on what its parent is willing
//! to offer, only on its own content, and [`place`] repeats the same natural-
//! size probe rather than trusting whatever `slot` it was given.
//!
//! ## What "the window" means here
//!
//! [`place`] treats `slot.rect` intersected with `slot.clip` as the viewport a
//! surface must not render outside of: `slot.rect` is whatever rect the
//! surface's placement parent offered (typically the frame root, generously,
//! since a surface is not meant to be boxed in by an ordinary ancestor's
//! flow), and `slot.clip` is whatever ambient clip is already in force by the
//! time we get here (e.g. a panel's clip chain, if a surface is declared
//! inside a clipped region). Intersecting the two means a surface never
//! escapes a clip its ancestry already imposed, while a permissive root that
//! hands surfaces the whole window as `slot.rect` still lets them use all of
//! it.
//!
//! ## `Anchor::Node` is a documented deferral
//!
//! [`Anchor::Node`] names another node's rect by id. Placements are produced
//! in tree pre-order (`layout/mod.rs`'s `place` walks the tree once,
//! depth-first), so the anchor node's rect may not exist yet when a surface
//! earlier in the walk needs it, and even when it does exist,
//! [`crate::frame::placement::PlacementSink`] exposes only `push` /
//! `current_parent` / `enter` / `leave` — there is no way to read back a
//! placement already pushed. Changing that trait is out of scope for this
//! module (it is shared with every other container). So: every
//! `Anchor::Node` resolves exactly like [`Anchor::Viewport`] today, and that
//! is not left silent — [`resolve_anchor_kind`] is the public, tested seam
//! that says so, and [`place`] calls it rather than quietly special-casing
//! `Anchor::Node` inline. A future crate that gains a two-pass placement
//! walk (or a sink that can be queried) can resolve `Anchor::Node` properly
//! by changing this module alone.
//!
//! ## Input policy reaching the focus scope
//!
//! [`crate::frame::placement::PlacementSemantics`] has no `input_policy`
//! field — it is a `surface`-only property that never belonged on every
//! placement. [`surface_scopes`] is the documented seam instead: it walks a
//! [`ViewNode`] tree (the same tree that produced the frame) and returns
//! every surface's declared [`InputPolicy`] keyed by canonical placement id,
//! for `crate::focus::FocusTree` to fold in alongside the placements
//! themselves.

use std::collections::BTreeMap;

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Rect, Size};
use crate::layout::{LayoutCtx, SizeProposal, Slot};
use crate::tree::{Anchor, ClampRule, InputPolicy, KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
///
/// The incoming `proposal` is deliberately not read: see the module doc for
/// why a surface's size never depends on what its parent offers.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let _ = proposal;
    natural_size(node, ctx, path)
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    // Tree acceptance (`crate::tree::validate`) refuses a `surface` node
    // missing `layer` or `anchor` before layout ever runs, so this is a
    // guarantee this module gets to lean on, not a value it must default.
    let surface = node
        .props
        .surface()
        .expect("tree acceptance guarantees a `surface` node carries `layer` and `anchor`");

    // Same order the dispatcher uses in `crate::layout::measure`: clamp to
    // this node's own declared constraints, then sanitize — so a surface's
    // placed size is bound by the same rule every other node's is.
    let natural = node
        .constraints
        .clamp_size(natural_size(node, ctx, path))
        .sane();

    // See the module doc: the intersection is "the window" a surface must
    // never render outside of.
    let viewport = slot.rect.intersect(slot.clip);

    let anchor_origin = anchor_origin(surface.anchor, viewport, natural);

    let (x, rect_w, content_w, scroll_x) = clamp_axis(
        anchor_origin.0,
        natural.w,
        viewport.x,
        viewport.w,
        surface.clamp,
    );
    let (y, rect_h, content_h, scroll_y) = clamp_axis(
        anchor_origin.1,
        natural.h,
        viewport.y,
        viewport.h,
        surface.clamp,
    );

    let rect = Rect::new(x, y, rect_w, rect_h);
    let content_rect = Rect::new(x, y, content_w, content_h);
    let needs_scroll = scroll_x || scroll_y;

    let z_slot = slot.above(surface.layer.base_z());

    let me = sink.push(Placement {
        id: path.id(),
        kind: node.kind,
        rect,
        z: z_slot.z,
        clip: rect,
        opacity: z_slot.opacity,
        paint: PaintState {
            content_hash: 0,
            // `Scroll` keeps the content's natural extent past what fits: the
            // part beyond `rect` is clipped away below, and that clipping is
            // exactly what "content hidden this frame" means for a surface
            // that has no text run of its own to truncate.
            truncated: needs_scroll,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics: crate::layout::semantics_of(node),
        parent: None,
    });

    // The content slot keeps the surface's natural size (so children never
    // reflow just because the window was small) but is clipped to the
    // placed rect — the scroll affordance the `Scroll` rule promises, and a
    // no-op narrowing for `Flip`/`Shrink`, where `content_rect == rect`.
    let content_slot = z_slot.with_rect(content_rect).clipped_to(rect);

    sink.enter(me);
    // A surface is anchored in viewport coordinates: no ancestor `scroll`
    // has moved `content_slot`, so no ancestor `scroll` may claim to be the
    // scroll context of what is inside it either (`LayoutCtx::outside_scroll`).
    ctx.outside_scroll(|ctx| {
        for child in &node.children {
            crate::layout::place(child, ctx, path, content_slot, sink);
        }
    });
    sink.leave();
}

/// The bounding box of this node's children, each probed at its own natural
/// (`Unspecified`) extent. A surface with no children is a zero-size point at
/// its anchor.
fn natural_size(node: &ViewNode, ctx: &mut LayoutCtx<'_>, path: &mut KeyPath) -> Size {
    // Measured under the same empty scroll context `place` uses, so a
    // surface's natural size and its placement agree about what encloses it.
    ctx.outside_scroll(|ctx| {
        let mut size = Size::ZERO;
        for child in &node.children {
            let child_size = crate::layout::measure(child, ctx, path, SizeProposal::unspecified());
            size = size.max(child_size);
        }
        size
    })
}

/// How [`place`] resolves one [`Anchor`] variant.
///
/// Exposed so a caller (or a test) can see the `Anchor::Node` deferral
/// documented in the module doc rather than infer it from behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorResolution {
    /// Resolved to the declared point.
    Point,
    /// Resolved to the viewport centre, as declared.
    Viewport,
    /// Declared as `Anchor::Node`, but resolved as `Anchor::Viewport` because
    /// this pass cannot read another node's placement (see the module doc).
    NodeFallenBackToViewport,
}

/// How `anchor` will be resolved by [`place`]. See the module doc's
/// `Anchor::Node` section.
#[must_use]
pub fn resolve_anchor_kind(anchor: &Anchor) -> AnchorResolution {
    match anchor {
        Anchor::Point { .. } => AnchorResolution::Point,
        Anchor::Viewport => AnchorResolution::Viewport,
        Anchor::Node { .. } => AnchorResolution::NodeFallenBackToViewport,
    }
}

/// The preferred top-left origin for a surface of `natural` size anchored by
/// `anchor` inside `viewport`, before clamping.
///
/// `Anchor::Point` is read as the box's top-left corner (the simplest
/// deterministic reading available without an `Edge` to grow away from, which
/// only `Anchor::Node` declares). `Anchor::Viewport` centres the box, and
/// `Anchor::Node` — per [`resolve_anchor_kind`] — centres it the same way.
fn anchor_origin(anchor: &Anchor, viewport: Rect, natural: Size) -> (f32, f32) {
    match resolve_anchor_kind(anchor) {
        AnchorResolution::Point => {
            let Anchor::Point { x, y } = anchor else {
                unreachable!("resolve_anchor_kind returned Point for a non-Point anchor")
            };
            (*x, *y)
        }
        AnchorResolution::Viewport | AnchorResolution::NodeFallenBackToViewport => (
            viewport.x + (viewport.w - natural.w) / 2.0,
            viewport.y + (viewport.h - natural.h) / 2.0,
        ),
    }
}

/// Clamp one axis of a preferred `(origin, extent)` box into
/// `(vp_origin, vp_origin + vp_extent)` by `rule`.
///
/// Returns `(rect_origin, rect_extent, content_extent, needs_scroll)`:
/// `rect_extent` is what the placed, visible rect uses (always
/// `<= vp_extent`, so the placement never renders outside the viewport on
/// this axis); `content_extent` is what the child slot uses, which is larger
/// than `rect_extent` only under `ClampRule::Scroll` when the natural extent
/// does not fit.
fn clamp_axis(
    origin: f32,
    extent: f32,
    vp_origin: f32,
    vp_extent: f32,
    rule: ClampRule,
) -> (f32, f32, f32, bool) {
    let vp_extent = vp_extent.max(0.0);
    let fits = extent <= vp_extent;
    match rule {
        ClampRule::Shrink => {
            // "Keep the edge" means the anchored origin itself does not move
            // (unlike `Flip`, which slides the whole box back into bounds);
            // only the far edge gives way. The origin is still pulled into
            // `[vp_origin, vp_end]` first — a preferred origin that is
            // itself off-window has no edge left to keep, so the nearest one
            // is the honest fallback — then the extent shrinks to whatever
            // room remains from there to the viewport's far edge.
            let vp_end = vp_origin + vp_extent;
            let o = origin.clamp(vp_origin, vp_end);
            let room = (vp_end - o).max(0.0);
            let e = extent.min(room);
            (o, e, e, false)
        }
        ClampRule::Flip => {
            if fits {
                let o = clamp_origin(origin, extent, vp_origin, vp_extent);
                (o, extent, extent, false)
            } else {
                // Neither edge has room for the natural extent: flipping to
                // the opposite side cannot help, so this is where Flip's
                // documented fallback to Shrink applies.
                let e = extent.min(vp_extent);
                (vp_origin, e, e, false)
            }
        }
        ClampRule::Scroll => {
            if fits {
                let o = clamp_origin(origin, extent, vp_origin, vp_extent);
                (o, extent, extent, false)
            } else {
                // The extent itself is kept for the content slot; only the
                // visible placement is bounded to the viewport, which is the
                // scroll affordance the module doc describes.
                (vp_origin, vp_extent, extent, true)
            }
        }
    }
}

/// Translate `origin` by the smallest amount that brings a box of `extent`
/// fully inside `[vp_origin, vp_origin + vp_extent]`, assuming `extent <=
/// vp_extent`.
fn clamp_origin(origin: f32, extent: f32, vp_origin: f32, vp_extent: f32) -> f32 {
    let vp_end = vp_origin + vp_extent;
    let mut o = origin;
    if o + extent > vp_end {
        o = vp_end - extent;
    }
    if o < vp_origin {
        o = vp_origin;
    }
    o
}

/// Declared [`InputPolicy`] of every `surface` node in `tree`, keyed by its
/// canonical placement id (`KeyPath::id`).
///
/// See the module doc's "Input policy reaching the focus scope" section:
/// `Placement` carries no `input_policy` field, so `crate::focus::FocusTree`
/// reads it from here instead, walking the same tree that produced the frame
/// the placements came from.
#[must_use]
pub fn surface_scopes(tree: &ViewNode) -> BTreeMap<String, InputPolicy> {
    let mut out = BTreeMap::new();
    collect_surface_scopes(tree, &mut KeyPath::root(), &mut out);
    out
}

fn collect_surface_scopes(
    node: &ViewNode,
    path: &mut KeyPath,
    out: &mut BTreeMap<String, InputPolicy>,
) {
    path.push(node.key.clone());
    if let Some(surface) = node.props.surface() {
        out.insert(path.id(), surface.input_policy);
    }
    for child in &node.children {
        collect_surface_scopes(child, path, out);
    }
    path.pop();
}

#[cfg(test)]
mod tests {
    use super::{AnchorResolution, clamp_axis, resolve_anchor_kind, surface_scopes};
    use crate::frame::placement::PlacementList;
    use crate::geom::{Point, Rect, Size};
    use crate::layout::{SizeProposal, Slot};
    use crate::testing::Harness;
    use crate::tree::{
        Anchor, ClampRule, Edge, InputPolicy, Interaction, KeyPath, Layer, NodeKind, Props, Role,
        ViewNode,
    };

    /// A `surface` node with one `spacer` child of `size`, anchored and
    /// clamped as declared. The spacer is the simplest content whose natural
    /// size is exactly what we ask for under an open probe: a fixed-size
    /// `Exact` proposal echoes back unchanged (see `layout::leaf::measure`),
    /// but an open probe on a bare spacer answers zero — so the child here
    /// carries constraints instead, which `crate::layout::measure` applies
    /// after any kind's own answer, open probe included.
    fn surface(anchor: Anchor, clamp: ClampRule, policy: InputPolicy, size: Size) -> ViewNode {
        let mut content = ViewNode::new(NodeKind::Spacer, "content");
        content.constraints.horizontal.min = Some(size.w);
        content.constraints.horizontal.max = Some(size.w);
        content.constraints.vertical.min = Some(size.h);
        content.constraints.vertical.max = Some(size.h);
        ViewNode::new(NodeKind::Surface, "popup")
            .with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(anchor),
                clamp: Some(clamp),
                input_policy: Some(policy),
                ..Props::default()
            })
            .child(content)
    }

    fn place_surface(node: &ViewNode, viewport: Rect) -> crate::frame::placement::Placement {
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            node,
            &mut h.ctx(),
            &mut path,
            Slot::new(viewport),
            &mut sink,
        );
        sink.into_vec()
            .into_iter()
            .next()
            .expect("surface pushes its own placement first")
    }

    #[test]
    fn a_point_anchor_places_the_box_with_that_point_as_its_top_left_corner() {
        let node = surface(
            Anchor::Point { x: 50.0, y: 60.0 },
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(40.0, 20.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(50.0, 60.0, 40.0, 20.0));
    }

    #[test]
    fn a_viewport_anchor_centres_the_box() {
        let node = surface(
            Anchor::Viewport,
            ClampRule::Shrink,
            InputPolicy::DismissOutside,
            Size::new(100.0, 50.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(350.0, 275.0, 100.0, 50.0));
    }

    #[test]
    fn a_node_anchor_falls_back_to_viewport_centring_and_says_so() {
        let anchor = Anchor::Node {
            id: "/some/other/node".into(),
            edge: Edge::Bottom,
        };
        assert_eq!(
            resolve_anchor_kind(&anchor),
            AnchorResolution::NodeFallenBackToViewport
        );
        let node = surface(
            anchor,
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(100.0, 50.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        // Same rect a bare `Anchor::Viewport` would produce: the fallback is
        // real, not merely claimed.
        assert_eq!(placed.rect, Rect::new(350.0, 275.0, 100.0, 50.0));
    }

    #[test]
    fn point_and_viewport_anchors_resolve_to_themselves() {
        assert_eq!(
            resolve_anchor_kind(&Anchor::Point { x: 1.0, y: 1.0 }),
            AnchorResolution::Point
        );
        assert_eq!(
            resolve_anchor_kind(&Anchor::Viewport),
            AnchorResolution::Viewport
        );
    }

    // -- Each ClampRule at each window edge, hand-computed. --------------

    #[test]
    fn flip_pulls_the_box_back_from_the_right_edge() {
        let node = surface(
            Anchor::Point { x: 780.0, y: 10.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(40.0, 20.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        // 780 + 40 = 820 > 800, so the box is pulled left to end exactly at
        // the viewport's right edge: 800 - 40 = 760.
        assert_eq!(placed.rect, Rect::new(760.0, 10.0, 40.0, 20.0));
    }

    #[test]
    fn flip_pulls_the_box_back_from_the_bottom_edge() {
        let node = surface(
            Anchor::Point { x: 10.0, y: 590.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(20.0, 40.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(10.0, 560.0, 20.0, 40.0));
    }

    #[test]
    fn flip_pulls_the_box_back_from_the_left_and_top_edges() {
        let node = surface(
            Anchor::Point { x: -30.0, y: -30.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(40.0, 40.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(0.0, 0.0, 40.0, 40.0));
    }

    #[test]
    fn flip_falls_back_to_shrink_when_the_natural_extent_cannot_fit_anywhere() {
        let node = surface(
            Anchor::Point { x: 0.0, y: 0.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(1000.0, 50.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(0.0, 0.0, 800.0, 50.0));
    }

    #[test]
    fn shrink_keeps_the_edge_and_reduces_the_extent() {
        let node = surface(
            Anchor::Point { x: 700.0, y: 0.0 },
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(200.0, 50.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        // The left edge (700) is kept; the box shrinks to end at the window.
        assert_eq!(placed.rect, Rect::new(700.0, 0.0, 100.0, 50.0));
    }

    #[test]
    fn scroll_keeps_the_full_extent_for_content_but_clips_the_placed_rect() {
        let node = surface(
            Anchor::Point { x: 0.0, y: 0.0 },
            ClampRule::Scroll,
            InputPolicy::Block,
            Size::new(1000.0, 50.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(0.0, 0.0, 800.0, 50.0));
        assert_eq!(placed.clip, placed.rect);
        assert!(
            placed.paint.truncated,
            "scroll clamp must mark the surface as needing an affordance"
        );
    }

    #[test]
    fn scroll_does_not_clip_when_the_natural_extent_already_fits() {
        let node = surface(
            Anchor::Point { x: 10.0, y: 10.0 },
            ClampRule::Scroll,
            InputPolicy::Block,
            Size::new(40.0, 20.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(10.0, 10.0, 40.0, 20.0));
        assert!(!placed.paint.truncated);
    }

    // -- Re-clamping on resize. -------------------------------------------

    #[test]
    fn the_same_surface_re_clamps_correctly_under_two_viewport_sizes() {
        // `Flip` rather than `Shrink`: this test is about the box moving to
        // stay fully visible, which is exactly the behaviour that
        // distinguishes `Flip` (translate, keep the extent) from `Shrink`
        // (keep the edge, reduce the extent) — see the two rules' own
        // dedicated tests above.
        let node = surface(
            Anchor::Point { x: 700.0, y: 500.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(50.0, 50.0),
        );
        let wide = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(wide.rect, Rect::new(700.0, 500.0, 50.0, 50.0));

        // Shrinking the viewport must move the box, not leave it hanging off
        // the (now smaller) window.
        let narrow = place_surface(&node, Rect::new(0.0, 0.0, 720.0, 520.0));
        assert_eq!(narrow.rect, Rect::new(670.0, 470.0, 50.0, 50.0));
        assert!(narrow.rect.x + narrow.rect.w <= 720.0);
        assert!(narrow.rect.y + narrow.rect.h <= 520.0);
    }

    // -- Layers place above the previous one. ------------------------------

    #[test]
    fn each_layer_places_above_the_previous_one() {
        let base_z = |layer: Layer| {
            let node = ViewNode::new(NodeKind::Surface, "s")
                .with_props(Props {
                    layer: Some(layer),
                    anchor: Some(Anchor::Viewport),
                    ..Props::default()
                })
                .child(ViewNode::new(NodeKind::Spacer, "c"));
            place_surface(&node, Rect::new(0.0, 0.0, 100.0, 100.0)).z
        };
        assert!(base_z(Layer::Popup) < base_z(Layer::Toast));
        assert!(base_z(Layer::Toast) < base_z(Layer::Modal));
        assert!(base_z(Layer::Modal) < base_z(Layer::FrameWide));
    }

    // -- input_policy reaches the focus module. ----------------------------

    #[test]
    fn surface_scopes_collects_every_surfaces_declared_policy_by_placement_id() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "label"))
            .child(surface(
                Anchor::Viewport,
                ClampRule::Shrink,
                InputPolicy::Block,
                Size::new(10.0, 10.0),
            ));
        let scopes = surface_scopes(&tree);
        // Nested under "root": the id is the full path, not just the node's
        // own key.
        assert_eq!(scopes.get("/root/popup").copied(), Some(InputPolicy::Block));
        assert_eq!(
            scopes.len(),
            1,
            "only the surface node itself is a scope, not its content"
        );
    }

    // -- The clamp-axis property: never off-window. -------------------------

    proptest::proptest! {
        #[test]
        fn a_placed_surface_is_always_inside_the_viewport(
            ox in -2000.0f32..2000.0,
            oy in -2000.0f32..2000.0,
            w in 0.0f32..1500.0,
            h in 0.0f32..1500.0,
            vw in 1.0f32..1000.0,
            vh in 1.0f32..1000.0,
            rule in proptest::prop_oneof![
                proptest::strategy::Just(ClampRule::Flip),
                proptest::strategy::Just(ClampRule::Shrink),
                proptest::strategy::Just(ClampRule::Scroll),
            ],
        ) {
            let (x, rw, _cw, _) = clamp_axis(ox, w, 0.0, vw, rule);
            let (y, rh, _ch, _) = clamp_axis(oy, h, 0.0, vh, rule);
            proptest::prop_assert!(x >= 0.0 - 0.001, "x={x} rw={rw}");
            proptest::prop_assert!(x + rw <= vw + 0.001, "x={x} rw={rw} vw={vw}");
            proptest::prop_assert!(y >= 0.0 - 0.001, "y={y} rh={rh}");
            proptest::prop_assert!(y + rh <= vh + 0.001, "y={y} rh={rh} vh={vh}");
        }
    }

    // The same property, driven through the real `place` path rather than the
    // axis helper directly, so the whole pipeline (anchor resolution, z, both
    // axes together) is what gets checked, not just the arithmetic.
    proptest::proptest! {
        #[test]
        fn place_never_produces_a_rect_outside_the_viewport(
            ax in -500.0f32..1300.0,
            ay in -500.0f32..1100.0,
            w in 0.0f32..900.0,
            h in 0.0f32..700.0,
            rule in proptest::prop_oneof![
                proptest::strategy::Just(ClampRule::Flip),
                proptest::strategy::Just(ClampRule::Shrink),
                proptest::strategy::Just(ClampRule::Scroll),
            ],
        ) {
            let node = surface(
                Anchor::Point { x: ax, y: ay },
                rule,
                InputPolicy::Block,
                Size::new(w, h),
            );
            let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
            let placed = place_surface(&node, viewport);
            proptest::prop_assert!(placed.rect.x >= viewport.x - 0.001);
            proptest::prop_assert!(placed.rect.y >= viewport.y - 0.001);
            proptest::prop_assert!(placed.rect.x + placed.rect.w <= viewport.right() + 0.001);
            proptest::prop_assert!(placed.rect.y + placed.rect.h <= viewport.bottom() + 0.001);
        }
    }

    // -- measure() ignores its proposal and answers natural size. ---------

    #[test]
    fn measure_answers_natural_size_regardless_of_the_offered_proposal() {
        let node = surface(
            Anchor::Point { x: 0.0, y: 0.0 },
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(64.0, 64.0),
        );
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let zero = crate::layout::measure(&node, &mut h.ctx(), &mut path, SizeProposal::zero());
        let unbounded =
            crate::layout::measure(&node, &mut h.ctx(), &mut path, SizeProposal::unbounded());
        assert_eq!(zero, unbounded);
        assert_eq!(zero, Size::new(64.0, 64.0));
    }

    // -- A surface with no children is a zero-size point. -------------------

    #[test]
    fn a_childless_surface_places_a_zero_size_box_at_its_anchor() {
        let node = ViewNode::new(NodeKind::Surface, "empty").with_props(Props {
            layer: Some(Layer::Toast),
            anchor: Some(Anchor::Point { x: 12.0, y: 34.0 }),
            ..Props::default()
        });
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(12.0, 34.0, 0.0, 0.0));
    }

    /// Exercises the documented interactive shape (role + label + Focus
    /// action) so a real modal's semantic payload round-trips through
    /// `place`, not just its geometry.
    #[test]
    fn an_interactive_surface_carries_its_declared_semantics_into_the_placement() {
        let mut node = surface(
            Anchor::Viewport,
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(10.0, 10.0),
        );
        node = node.interactive(Role::Dialog, "Confirm delete", &[Interaction::Focus]);
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.semantics.role, Some(Role::Dialog));
        assert_eq!(placed.semantics.label.as_deref(), Some("Confirm delete"));
        assert_eq!(placed.semantics.actions, vec![Interaction::Focus]);
    }

    #[test]
    fn point_anchor_math_matches_hand_computation_at_the_origin() {
        // A sanity check on `anchor_origin` in isolation, independent of the
        // clamp step, using a viewport-anchored box centred exactly.
        let placed_center = {
            let node = surface(
                Anchor::Viewport,
                ClampRule::Shrink,
                InputPolicy::Block,
                Size::new(0.0, 0.0),
            );
            place_surface(&node, Rect::new(0.0, 0.0, 10.0, 10.0)).rect
        };
        assert_eq!(placed_center.origin(), Point::new(5.0, 5.0));
    }
}
