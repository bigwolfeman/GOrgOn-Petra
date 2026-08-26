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
//! [`Anchor::Node`] names another node's rect by id, and this pass does not
//! have that rect. The reason is the walk order, not the sink. Placements are
//! produced in tree pre-order (`layout/mod.rs`'s `place` walks the tree once,
//! depth-first), so a surface earlier in the walk asks for a rect its anchor
//! has not been given yet. The sink itself can be read back —
//! [`crate::frame::placement::PlacementSink::placed`] exists and
//! `layout/mod.rs` already calls it — but reading it from here would answer
//! for the anchors that happen to be placed first and answer nothing for the
//! rest, which is a worse contract than answering the same way for all of
//! them.
//!
//! So every `Anchor::Node` resolves exactly like [`Anchor::Viewport`] today,
//! and that is not left silent — [`resolve_anchor_kind`] is the public,
//! tested seam that says so rather than leaving it to be inferred from
//! behaviour, and [`place`] calls it rather than quietly special-casing
//! `Anchor::Node` inline. What closes it is a pruned harvest walk that places
//! the anchor set before the real walk and hands this module the resulting
//! rects (`contracts/anchored-placement.md` §1). That walk belongs in
//! `layout/mod.rs`. Once it exists, the declared
//! [`Edge`](crate::tree::Edge) becomes the side of the harvested rect the
//! surface is placed against, and the clamp ladder below is already written
//! to take a side: `AxisPlacement::Sided` is the shape it will arrive in.
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
use crate::geom::{Axis, Rect, Size};
use crate::layout::{LayoutCtx, SizeProposal, Slot, semantics_of};
use crate::tree::{Anchor, ClampRule, InputPolicy, KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
///
/// The incoming `proposal` is deliberately not read: see the module doc for
/// why a surface's size never depends on what its parent offers.
///
/// Padding grows this container's own natural size, exactly as it grows any
/// other container's reported size — a surface's own placed rect is what
/// carries the padding, never its children's (`layout-insets.md` §8 step 6).
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let _ = proposal;
    let natural = natural_size(node, ctx, path);
    let padding = ctx.padding(&node.props.padding);
    Size::new(
        natural.w + padding.along(Axis::Horizontal),
        natural.h + padding.along(Axis::Vertical),
    )
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    // `crate::frame::petrify` and `petrify_with_memo` — the only entry
    // points that reach this walk — accept a `crate::tree::ValidatedTree`,
    // which only `crate::tree::validate` can mint, and `validate` refuses a
    // `surface` node missing `layer` or `anchor`. So this is enforced by the
    // type system, not merely by a caller remembering to check first: a
    // tree that reaches this line already passed acceptance, and this is a
    // guarantee this module gets to lean on, not a value it must default.
    let surface = node
        .props
        .surface()
        .expect("tree acceptance guarantees a `surface` node carries `layer` and `anchor`");

    // Same order the dispatcher uses in `crate::layout::measure`: clamp to
    // this node's own declared constraints, then sanitize — so a surface's
    // placed size is bound by the same rule every other node's is.
    let mut natural = node
        .constraints
        .clamp_size(natural_size(node, ctx, path))
        .sane();

    // "Add before clamp, inset content_rect after" (`layout-insets.md` §8
    // step 6): padding grows the surface's own natural size here, before the
    // `ClampRule`/anchor math below, so padding participates in whether the
    // surface needs to shrink, flip, or scroll against the window edge —
    // exactly like the rest of its natural size. `content_rect` is inset
    // back out of the *clamped* `rect` afterwards, not built from this
    // padded `natural` directly.
    let padding = ctx.padding(&node.props.padding);
    natural.w += padding.along(Axis::Horizontal);
    natural.h += padding.along(Axis::Vertical);

    // See the module doc: the intersection is "the window" a surface must
    // never render outside of.
    let viewport = slot.rect.intersect(slot.clip);

    let (plan_x, plan_y) = anchor_placement(surface.anchor, viewport, natural);

    let (x, rect_w, _content_w, scroll_x) =
        clamp_axis(plan_x, natural.w, viewport.x, viewport.w, surface.clamp);
    let (y, rect_h, _content_h, scroll_y) =
        clamp_axis(plan_y, natural.h, viewport.y, viewport.h, surface.clamp);

    let rect = Rect::new(x, y, rect_w, rect_h);
    // Padding is inside the box: children are offered the placed, clamped
    // rect minus the surface's own padding — never the wider unclamped
    // extent `clamp_axis` tracked for the scroll affordance (`_content_w`/
    // `_content_h`, now unused for this) — the same "container's own placed
    // rect never shrinks for its own padding; only what it offers its
    // children does" rule every other container in this change follows.
    let content_rect = rect.inset_edges(padding);
    let needs_scroll = scroll_x || scroll_y;

    let z_slot = slot.above(surface.layer.base_z());

    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    let me = sink.push(Placement {
        id,
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
            // See `PaintState::overflowed`: written by the leaf that owns the
            // content, never by a container.
            overflowed: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });

    // The content slot is the placed, clamped rect's own padded interior,
    // clipped to that same placed rect — a no-op narrowing except that
    // `content_rect` is always `rect` minus the padding, at every
    // `ClampRule`, including `Scroll`: an overflowing surface's content no
    // longer gets the wider unclamped natural extent to lay out into before
    // clipping (what this slot did pre-padding); it is offered exactly the
    // clamped, padded interior, the same "children are offered the box minus
    // padding, never more" rule this module's padding support follows
    // throughout.
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
    /// Declared as `Anchor::Node`, but resolved as `Anchor::Viewport`
    /// because this single pre-order pass reaches a surface before the node
    /// it is anchored to, so the anchor rect does not exist yet (see the
    /// module doc).
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

/// Which way a surface grows away from its anchor on one axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grow {
    /// Towards increasing coordinates: the anchor is the box's near edge.
    Forward,
    /// Towards decreasing coordinates: the anchor is the box's far edge.
    Backward,
}

impl Grow {
    fn opposite(self) -> Self {
        match self {
            Self::Forward => Self::Backward,
            Self::Backward => Self::Forward,
        }
    }
}

/// One axis of a surface's preferred position, in the form the clamp rules
/// need it: either a declared side to grow away from — which is what gives
/// [`ClampRule::Flip`] an opposite side to try — or a bare origin with no
/// side at all.
#[derive(Clone, Copy, Debug, PartialEq)]
enum AxisPlacement {
    /// The box is centred on this axis and declares no side, so there is
    /// nothing to flip to. This is what an `Anchor::Viewport` surface (and,
    /// today, an `Anchor::Node` one) gets on both axes.
    Centred(f32),
    /// The box's near-in-`grow` edge sits at `at`, so `Flip` may put the box
    /// on the other side of `at` instead.
    Sided {
        /// The anchor coordinate on this axis.
        at: f32,
        /// Which way the box extends from `at`.
        grow: Grow,
    },
}

impl AxisPlacement {
    /// The preferred origin for a box of `extent`, before any clamping.
    fn origin(self, extent: f32) -> f32 {
        match self {
            Self::Centred(origin) => origin,
            Self::Sided {
                at,
                grow: Grow::Forward,
            } => at,
            Self::Sided {
                at,
                grow: Grow::Backward,
            } => at - extent,
        }
    }

    /// The same anchor read from its other side.
    ///
    /// `Centred` has no side, so it is its own opposite: the flip attempt
    /// then simply repeats the preferred attempt that already failed, and
    /// the ladder falls through to the shift with no special case for it.
    fn flipped(self) -> Self {
        match self {
            Self::Centred(origin) => Self::Centred(origin),
            Self::Sided { at, grow } => Self::Sided {
                at,
                grow: grow.opposite(),
            },
        }
    }

    /// How much window there is between the anchor and the far edge on this
    /// side — the room a box placed here has before it runs out. Used only to
    /// choose between the two sides when neither fits.
    fn room(self, vp_origin: f32, vp_extent: f32) -> f32 {
        match self {
            Self::Centred(_) => vp_extent.max(0.0),
            Self::Sided {
                at,
                grow: Grow::Forward,
            } => (vp_origin + vp_extent - at).max(0.0),
            Self::Sided {
                at,
                grow: Grow::Backward,
            } => (at - vp_origin).max(0.0),
        }
    }
}

/// The preferred placement of a surface of `natural` size anchored by
/// `anchor` inside `viewport`, one [`AxisPlacement`] per axis, before
/// clamping.
///
/// `Anchor::Point` is read as the box's top-left corner (the simplest
/// deterministic reading available without an `Edge` to grow away from, which
/// only `Anchor::Node` declares). That reading is a *side* on both axes: the
/// box grows right and down from the point, so the point's other side — box
/// ending at the point — is a real opposite for [`ClampRule::Flip`] to try.
/// `Anchor::Viewport` centres the box and declares no side, and
/// `Anchor::Node` — per [`resolve_anchor_kind`] — centres it the same way
/// until the harvest walk lands (see the module doc).
fn anchor_placement(
    anchor: &Anchor,
    viewport: Rect,
    natural: Size,
) -> (AxisPlacement, AxisPlacement) {
    match resolve_anchor_kind(anchor) {
        AnchorResolution::Point => {
            let Anchor::Point { x, y } = anchor else {
                unreachable!("resolve_anchor_kind returned Point for a non-Point anchor")
            };
            (
                AxisPlacement::Sided {
                    at: *x,
                    grow: Grow::Forward,
                },
                AxisPlacement::Sided {
                    at: *y,
                    grow: Grow::Forward,
                },
            )
        }
        AnchorResolution::Viewport | AnchorResolution::NodeFallenBackToViewport => (
            AxisPlacement::Centred(viewport.x + (viewport.w - natural.w) / 2.0),
            AxisPlacement::Centred(viewport.y + (viewport.h - natural.h) / 2.0),
        ),
    }
}

/// Clamp one axis of a `plan`ned box of `extent` into
/// `(vp_origin, vp_origin + vp_extent)` by `rule`.
///
/// Returns `(rect_origin, rect_extent, content_extent, needs_scroll)`:
/// `rect_extent` is what the placed, visible rect uses (always
/// `<= vp_extent`, so the placement never renders outside the viewport on
/// this axis); `content_extent` is what the child slot uses, which is larger
/// than `rect_extent` only under `ClampRule::Scroll` when the natural extent
/// does not fit.
fn clamp_axis(
    plan: AxisPlacement,
    extent: f32,
    vp_origin: f32,
    vp_extent: f32,
    rule: ClampRule,
) -> (f32, f32, f32, bool) {
    let vp_extent = vp_extent.max(0.0);
    match rule {
        ClampRule::Shrink => {
            let (o, e) = shrink_at(plan, extent, vp_origin, vp_extent);
            (o, e, e, false)
        }
        ClampRule::Flip => {
            let (o, e) = flip_axis(plan, extent, vp_origin, vp_extent);
            (o, e, e, false)
        }
        ClampRule::Scroll => {
            if extent <= vp_extent {
                let o = clamp_origin(plan.origin(extent), extent, vp_origin, vp_extent);
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

/// [`ClampRule::Flip`] on one axis, returning `(origin, extent)`.
///
/// The ladder, in order, first fit wins (`anchored-placement.md` §4): the
/// declared side; then the anchor's *opposite* side, whole; then — neither
/// having room — the side with strictly more of it, with an exact tie kept on
/// the declared side so the choice is settled by declaration order and never
/// by an unspecified float comparison. Only once a side is chosen does
/// anything move or give way: the box slides into the window if the extent
/// fits it at all, and shrinks against the chosen side if it does not.
///
/// The opposite-side attempt is the step this rule is named for, and it is
/// what distinguishes `Flip` from `Shrink` and from a bare shift: a box that
/// overhangs the right edge by 20 is placed on the anchor's left side, not
/// slid 20 to the left.
fn flip_axis(plan: AxisPlacement, extent: f32, vp_origin: f32, vp_extent: f32) -> (f32, f32) {
    let preferred = plan.origin(extent);
    if fits_at(preferred, extent, vp_origin, vp_extent) {
        return (preferred, extent);
    }

    let other = plan.flipped();
    let flipped = other.origin(extent);
    if fits_at(flipped, extent, vp_origin, vp_extent) {
        return (flipped, extent);
    }

    // Neither side holds the box as declared, so take the side with strictly
    // more room. An exact tie keeps the declared side: the ladder's one tie,
    // settled by declaration order rather than by which way a float
    // comparison happens to fall.
    let chosen = if other.room(vp_origin, vp_extent) > plan.room(vp_origin, vp_extent) {
        other
    } else {
        plan
    };
    if extent <= vp_extent {
        // The window does hold the box somewhere, even though neither side
        // does: slide it in from the side just chosen.
        return (
            clamp_origin(chosen.origin(extent), extent, vp_origin, vp_extent),
            extent,
        );
    }
    // The extent is over budget on this axis whatever side it is on, so this
    // is where `Flip`'s documented fallback to `Shrink` applies — at the
    // chosen side, not at the window's near edge.
    shrink_at(chosen, extent, vp_origin, vp_extent)
}

/// Whether a box of `extent` placed at `origin` lies wholly inside
/// `[vp_origin, vp_origin + vp_extent]`.
fn fits_at(origin: f32, extent: f32, vp_origin: f32, vp_extent: f32) -> bool {
    origin >= vp_origin && origin + extent <= vp_origin + vp_extent
}

/// Keep the anchored edge where it is and let the extent give way, returning
/// `(origin, extent)`.
///
/// This is what [`ClampRule::Shrink`] means, and what [`flip_axis`] falls
/// back to once no side can hold the natural extent. The anchor is still
/// pulled into `[vp_origin, vp_end]` first — an anchor that is itself
/// off-window has no edge left to keep, so the nearest one is the honest
/// fallback — and then the extent shrinks to whatever room remains from there
/// to the window edge on the growing side.
fn shrink_at(plan: AxisPlacement, extent: f32, vp_origin: f32, vp_extent: f32) -> (f32, f32) {
    let vp_end = vp_origin + vp_extent;
    match plan {
        // A forward-growing box's anchor is already its near edge, and a
        // centred box has no anchored edge at all, so its own preferred
        // origin stands in as one. Both keep a near edge and let the far one
        // give way, which is the same arithmetic.
        AxisPlacement::Centred(near)
        | AxisPlacement::Sided {
            at: near,
            grow: Grow::Forward,
        } => {
            let o = near.clamp(vp_origin, vp_end);
            (o, extent.min((vp_end - o).max(0.0)))
        }
        AxisPlacement::Sided {
            at,
            grow: Grow::Backward,
        } => {
            // The anchor is the box's *far* edge here, so it is the far edge
            // that stays put and the near edge that gives way.
            let far = at.clamp(vp_origin, vp_end);
            let e = extent.min((far - vp_origin).max(0.0));
            (far - e, e)
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
    use super::{
        AnchorResolution, AxisPlacement, Grow, clamp_axis, resolve_anchor_kind, surface_scopes,
    };
    use crate::frame::placement::PlacementList;
    use crate::geom::{Axis, Insets, Point, Rect, Size};
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

    /// Same as [`place_surface`], but also returns the placed content
    /// child's own placement — needed to see `content_rect`, which never
    /// reaches the surface's own `Placement.rect`.
    fn place_surface_and_content(
        node: &ViewNode,
        viewport: Rect,
    ) -> (
        crate::frame::placement::Placement,
        crate::frame::placement::Placement,
    ) {
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
        let mut all = sink.into_vec().into_iter();
        let surface = all.next().expect("surface pushes its own placement first");
        let content = all.next().expect("the surface's content is placed too");
        (surface, content)
    }

    /// Same shape as [`surface`], with declared `padding`.
    fn padded_surface(anchor: Anchor, clamp: ClampRule, size: Size, padding: Insets) -> ViewNode {
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
                input_policy: Some(InputPolicy::Block),
                padding: Some(crate::testing::gap_insets(padding)),
                ..Props::default()
            })
            .child(content)
    }

    // -- Padding: inside the clamped rect, added before the clamp runs. ----

    /// The behaviour this leaf exists to add, pinned by name (gate G4).
    ///
    /// The surface's own placed rect grows by the padding (content 40x20 plus
    /// 8.0 on every edge is 56x36); the content it hands its one child is the
    /// *clamped* rect's interior, inset back down by that same padding —
    /// landing exactly on the child's own 40x20 natural size, round-tripped
    /// through "add before clamp, inset content_rect after"
    /// (`layout-insets.md` §8 step 6).
    #[test]
    fn a_padded_surface_pads_inside_its_clamped_rect() {
        let node = padded_surface(
            Anchor::Point { x: 100.0, y: 100.0 },
            ClampRule::Shrink,
            Size::new(40.0, 20.0),
            Insets::all(8.0),
        );
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let (surface, content) = place_surface_and_content(&node, viewport);

        assert_eq!(
            surface.rect,
            Rect::new(100.0, 100.0, 56.0, 36.0),
            "the surface's own placed rect includes its padding (40+16, 20+16), \
             with the anchor point unmoved: padding is inside the box"
        );
        assert_eq!(
            content.rect,
            Rect::new(108.0, 108.0, 40.0, 20.0),
            "the child is offered the surface's clamped rect minus its padding \
             — exactly its own declared 40x20, from the padded (108, 108) origin"
        );
    }

    /// The clamp-order decision, proven to matter, not merely asserted.
    ///
    /// `place` adds padding to the natural size *before* `clamp_axis` runs,
    /// so a padded box that does not fit is shrunk (or flipped) accounting
    /// for the padding, staying inside the viewport. The other order —
    /// clamp the *un-padded* content first, then add the padding to
    /// whatever came back — would let the padding push the box back out
    /// past the edge `clamp_axis` had just pulled it inside of, since
    /// nothing re-checks the viewport after the addition. This test computes
    /// both orders (the real one through `place`, the wrong one by calling
    /// the same `clamp_axis` this module uses, by hand, in the other
    /// sequence) and shows only one of them keeps the surface on-screen.
    #[test]
    fn add_before_clamp_keeps_the_padded_surface_on_screen_the_other_order_would_not() {
        let viewport = Rect::new(0.0, 0.0, 100.0, 100.0);
        let content_size = Size::new(40.0, 20.0);
        let padding = Insets::all(10.0);
        // Anchored close enough to the right edge that the un-padded 40.0
        // width fits (70 + 40 = 110 > 100, so even the un-padded box needs
        // *some* shrink — chosen so both orders engage `ClampRule::Shrink`,
        // and the only variable left is where the padding addition happens).
        let anchor_x = 70.0_f32;

        // The real pipeline: padding is added to `natural` before
        // `clamp_axis` runs (this module's own rule, exercised end-to-end).
        let node = padded_surface(
            Anchor::Point {
                x: anchor_x,
                y: 0.0,
            },
            ClampRule::Shrink,
            content_size,
            padding,
        );
        let placed = place_surface(&node, viewport);
        assert!(
            placed.rect.x + placed.rect.w <= viewport.right() + 0.001,
            "add-before-clamp: the padded box stays inside the viewport, rect={:?}",
            placed.rect
        );

        // The other order: clamp the un-padded 40.0-wide content first (the
        // same `clamp_axis` call `place` makes, just fed the un-padded
        // extent), then add the padding to the result afterwards, the way a
        // "pad after clamp" implementation would.
        let (wrong_x, clamped_w, _content_w, _scrolls) = clamp_axis(
            AxisPlacement::Sided {
                at: anchor_x,
                grow: Grow::Forward,
            },
            content_size.w,
            viewport.x,
            viewport.w,
            ClampRule::Shrink,
        );
        let wrong_w = clamped_w + padding.along(Axis::Horizontal);
        assert!(
            wrong_x + wrong_w > viewport.right() + 0.001,
            "clamp-then-add: adding padding after the clamp pushes the box \
             back past the edge clamp_axis just pulled it inside of \
             (x={wrong_x}, w={wrong_w}, viewport right={})",
            viewport.right()
        );
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

    /// The behaviour that makes `Flip` a flip rather than a shift, at the
    /// right edge.
    ///
    /// A point anchor grows the box right and down, so the opposite side is
    /// the box's right edge sitting on the anchor: 780 - 40 = 740. A shift
    /// would instead slide the box back until it ended at the window edge,
    /// 800 - 40 = 760. The two answers differ by exactly the overhang, which
    /// is what makes this test able to tell them apart
    /// (`anchored-placement.md` §4a).
    #[test]
    fn flip_places_the_box_on_the_opposite_side_of_the_anchor_at_the_right_edge() {
        let node = surface(
            Anchor::Point { x: 780.0, y: 10.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(40.0, 20.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(
            placed.rect,
            Rect::new(740.0, 10.0, 40.0, 20.0),
            "the box belongs on the anchor's other side (740), not slid back \
             against the window edge (760)"
        );
    }

    /// The same rule on the other axis: 590 - 40 = 550 is the opposite side,
    /// 600 - 40 = 560 is the shift this test used to assert.
    #[test]
    fn flip_places_the_box_on_the_opposite_side_of_the_anchor_at_the_bottom_edge() {
        let node = surface(
            Anchor::Point { x: 10.0, y: 590.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(20.0, 40.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(10.0, 550.0, 20.0, 40.0));
    }

    /// A side that fits is kept. The flip is a fallback, not a preference:
    /// nothing moves while the declared side has room.
    #[test]
    fn flip_leaves_the_box_on_its_declared_side_while_that_side_fits() {
        let node = surface(
            Anchor::Point { x: 100.0, y: 100.0 },
            ClampRule::Flip,
            InputPolicy::Block,
            Size::new(40.0, 20.0),
        );
        let placed = place_surface(&node, Rect::new(0.0, 0.0, 800.0, 600.0));
        assert_eq!(placed.rect, Rect::new(100.0, 100.0, 40.0, 20.0));
    }

    /// Both sides fail, so the ladder falls through to the shift — but only
    /// after the opposite side was tried and rejected. Anchored at -30 with
    /// a 40-wide box, the declared side reaches from -30 to 10 (off-window
    /// on the left) and the opposite side from -70 to -30 (entirely
    /// off-window), so the side with more room is the declared one and the
    /// box slides in to 0.
    #[test]
    fn flip_shifts_the_box_in_only_after_the_opposite_side_also_fails_at_the_left_and_top_edges() {
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
        // distinguishes `Flip` (put it on the anchor's other side, keep the
        // extent) from `Shrink` (keep the edge, reduce the extent) — see the
        // two rules' own dedicated tests above.
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
        // 700 + 50 = 750 > 720 and 500 + 50 = 550 > 520, so both axes take
        // the opposite side of the anchor: 700 - 50 = 650, 500 - 50 = 450.
        let narrow = place_surface(&node, Rect::new(0.0, 0.0, 720.0, 520.0));
        assert_eq!(narrow.rect, Rect::new(650.0, 450.0, 50.0, 50.0));
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
            // Every shape of plan the ladder can be handed, including the
            // backward-growing side only `Flip` can produce and the sideless
            // centred plan a viewport anchor produces.
            plan in proptest::prop_oneof![
                proptest::strategy::Just(0u8),
                proptest::strategy::Just(1u8),
                proptest::strategy::Just(2u8),
            ],
        ) {
            let axis = |at: f32| match plan {
                0 => AxisPlacement::Sided { at, grow: Grow::Forward },
                1 => AxisPlacement::Sided { at, grow: Grow::Backward },
                _ => AxisPlacement::Centred(at),
            };
            let (x, rw, _cw, _) = clamp_axis(axis(ox), w, 0.0, vw, rule);
            let (y, rh, _ch, _) = clamp_axis(axis(oy), h, 0.0, vh, rule);
            proptest::prop_assert!(x >= 0.0 - 0.001, "x={x} rw={rw}");
            proptest::prop_assert!(x + rw <= vw + 0.001, "x={x} rw={rw} vw={vw}");
            proptest::prop_assert!(y >= 0.0 - 0.001, "y={y} rh={rh}");
            proptest::prop_assert!(y + rh <= vh + 0.001, "y={y} rh={rh} vh={vh}");
        }
    }

    // The ladder's own ordering, as a property rather than as three worked
    // examples. The inputs are generated so the declared side never fits and
    // the opposite side always does — a box at most half the window wide,
    // anchored past the point where it would still fit growing forward.
    // `Flip` must then put it on the anchor's other side at full extent. A
    // shift would put it against the window edge instead, which is a
    // different number for every case here.
    proptest::proptest! {
        #[test]
        fn flip_puts_the_box_on_the_anchors_other_side_whenever_that_side_holds_it(
            vw in 10.0f32..800.0,
            width_ratio in 0.05f32..0.5,
            past_ratio in 0.1f32..1.0,
        ) {
            let w = vw * width_ratio;
            // Strictly past the last origin the declared side could hold, and
            // never past the window's far edge.
            let at = vw - w + past_ratio * w;
            let (x, rw, _cw, scrolls) = clamp_axis(
                AxisPlacement::Sided { at, grow: Grow::Forward },
                w,
                0.0,
                vw,
                ClampRule::Flip,
            );
            proptest::prop_assert!(!scrolls);
            proptest::prop_assert!(
                (rw - w).abs() < 0.001,
                "the other side holds the box whole, so nothing may shrink: rw={rw} w={w}"
            );
            proptest::prop_assert!(
                (x - (at - w)).abs() < 0.001,
                "flip must land on the anchor's other side ({}), not against the \
                 window edge ({}): x={x} at={at} w={w} vw={vw}",
                at - w,
                vw - w
            );
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
