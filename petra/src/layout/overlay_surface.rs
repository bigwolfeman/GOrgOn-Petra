//! Overlay surfaces: anchoring, clamping, and input policy.
//!
//! A `surface` node floats free of ordinary flow: unlike a stack or a grid, it
//! does not take the rect its layout parent offers, and it takes no room in
//! that parent's flow either — [`crate::layout::measure`] answers zero for
//! it, so the trigger under an open menu keeps the neighbours it had when
//! the menu was closed. It always sizes itself to its own natural content
//! extent (an "open probe" — every child measured with
//! [`Proposal::Unspecified`], the union taken), positions that box at its
//! declared [`Anchor`], and then clamps the result into the window by its
//! [`ClampRule`] so it never paints outside the viewport (FR-022). This is
//! why [`place`] repeats the natural-size probe rather than trusting whatever
//! `slot.rect` it was given: a floating surface's size does not depend on
//! what its parent is willing to offer, only on its own content.
//!
//! ## What "the window" means here
//!
//! [`place`] treats [`Slot::window`] — the rect the root of this walk was
//! placed into, the frame's viewport in any frame `petrify` makes — as the
//! viewport a surface must not render outside of, at every layer and at
//! every depth. Not `slot.rect`, and not `slot.clip`: a stack or a grid
//! offers a child a cell, and a cell is the wrong window for a box that is
//! supposed to float over the page. Before 2026-09-04 this read
//! `slot.rect ∩ slot.clip`, on the argument that a surface declared inside a
//! clipped panel should stay inside it. Two things were wrong with that.
//! Every surface declared anywhere but the frame root was boxed to a cell
//! exactly its own natural size, so a viewport-centred modal sat inside the
//! card that declared it and an anchored menu was dragged back into its
//! trigger's column, whatever its anchor said. And the surface's own
//! placement already broke the clip the rule claimed to respect: its
//! `clip` is its own rect, not the ancestor chain's, so the one thing the
//! intersection did was decide *where* the box landed, never *whether* it
//! could paint past the panel. The harvested anchor rects are in frame
//! coordinates and `ctx.outside_scroll` already places a surface's content
//! outside every ancestor scroll; the window is the coordinate space the
//! rest of this module was already working in.
//!
//! A surface that asks for more than the window gets the window. The
//! [`ClampRule::Shrink`] ladder keeps the anchored edge and gives the extent
//! away until the box fits, so a viewport-centred surface whose declared
//! minimum exceeds any window is placed exactly on the window rect. That is
//! how a scrim is spelled (`crate::component::modal`): a node covering the
//! viewport, painted through `background` like any other fill, with no
//! second sizing vocabulary for "the whole screen".
//!
//! ## `Anchor::Node` resolves against a harvested rect
//!
//! [`Anchor::Node`] names another node's rect by id, and a single pre-order
//! walk does not have that rect: placements are produced depth-first, so a
//! surface early in the walk would be asking for a rect its anchor has not
//! been given yet. Reading the sink back would answer for the anchors that
//! happen to be placed first and answer nothing for the rest, which is a
//! worse contract than answering the same way for all of them.
//!
//! What closes it is the pruned harvest walk in `layout/mod.rs`
//! ([`crate::layout::AnchorRects`], `contracts/anchored-placement.md` §1):
//! before the real walk, the anchor set alone is placed, and the resulting
//! rects arrive here on [`crate::layout::LayoutCtx`]. So the declared
//! [`Edge`] is the side of the harvested rect the surface is placed against,
//! the declared [`Align`] is where along that side it starts, and the ladder
//! below flips, shifts and shrinks from there. [`resolve_anchor_kind`] is
//! still the public seam that says which of those happened, and it now has a
//! fourth answer for the one case left: a walk begun below the anchor's own
//! subtree harvests no rect for it, and such a surface still centres in the
//! viewport rather than guessing.
//!
//! [`Anchor::Sibling`] is the same anchor spelled from where a component
//! constructor stands: a bare key, resolved against the surface's *own*
//! parent path ([`crate::tree::Anchor::target_id`]) into the canonical id
//! the harvest map is keyed by. The walk's `path` is that context — it is
//! the surface's full path by the time [`place`] runs — so the lookup costs
//! one id build and no search, and from the map onwards the two spellings
//! are one thing: the ladder below never learns which one named the rect.
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

use crate::frame::placement::{CaretPaint, PaintState, Placement, PlacementSink};
use crate::geom::{Axis, Rect, Size};
use crate::layout::{AnchorRects, LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::{Align, Anchor, ClampRule, Edge, Fit, InputPolicy, KeyPath, Tip, ViewNode};

/// Place this container and everything under it against `slot.window`, and
/// report the caret it draws back at its anchor, if it has one.
///
/// There is no `measure` counterpart in this module: a surface's flow size
/// is zero by the dispatcher's own rule (`crate::layout::measure`), and its
/// floating size is probed here, from its content, at placement.
///
/// The caret is returned rather than pushed because it is paint payload, and
/// the dispatcher — not any container — is what attaches paint payload
/// (`crate::layout::place`). It is the one member of that payload no author
/// could have written down: which side the box ended up on is decided here,
/// against the window, by the ladder in [`clamp_axis`].
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) -> Option<CaretPaint> {
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

    // See the module doc: the walk's window is "the window" a surface must
    // never render outside of — never the cell its flow parent offered.
    let viewport = slot.window;

    let mut plan = anchor_placement(surface.anchor, path, viewport, natural, ctx);
    // `Fit::Anchor`: a list box is as broad as the field that opened it
    // (`contracts/anchored-placement.md` §4, "Fit"). The anchor's rect is
    // only known once the plan has harvested it, and the cross-axis origin
    // the plan chose depends on the surface's extent, so the plan is redone
    // against the broadened size rather than patched. After `clamp_size`,
    // deliberately: a surface's own `max` describes its content, and a
    // menu narrower than its trigger is exactly the picture this exists to
    // rule out.
    if let Some(broadened) = fit_to_anchor(surface.fit, &plan, natural)
        && broadened != natural
    {
        natural = broadened;
        plan = anchor_placement(surface.anchor, path, viewport, natural, ctx);
    }

    let x = clamp_axis(plan.x, natural.w, viewport.x, viewport.w, surface.clamp);
    let y = clamp_axis(plan.y, natural.h, viewport.y, viewport.h, surface.clamp);
    let (scroll_x, scroll_y) = (x.scrolls, y.scrolls);

    let rect = Rect::new(x.origin, y.origin, x.extent, y.extent);
    // One record, read by the rect above and by the caret below, so the two
    // cannot disagree about which side the box landed on
    // (`contracts/anchored-placement.md` §4, "Resolution record").
    //
    // `Tip::Flush` is the author saying there is no pointer to draw: the
    // resolution record is still produced — the rect read it — and only the
    // caret payload is withheld (`contracts/anchored-placement.md` §5).
    let caret = match surface.tip {
        Tip::Caret => plan
            .anchored
            .map(|anchored| anchored.resolve(&plan, x, y, natural))
            .and_then(|resolved| caret_of(&resolved, rect, corner_radius(node, ctx))),
        Tip::Flush => None,
    };
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
    // clipped to that same placed rect. `with_clip`, not `clipped_to`: the
    // clip the ancestry handed down is the card's, and the surface is not
    // in the card — it is placed against the window, so its content starts
    // a fresh clip chain at the surface's own rect, exactly as the
    // surface's own placement above does. (Intersecting here clipped a
    // modal's footer to a zero-height sliver of the card that declared it,
    // which made the buttons invisible to focus and unreachable by hit
    // test.) `content_rect` is always `rect` minus the padding, at every
    // `ClampRule`, including `Scroll`: an overflowing surface's content
    // never gets the wider unclamped natural extent to lay out into before
    // clipping; it is offered exactly the clamped, padded interior, the
    // same "children are offered the box minus padding, never more" rule
    // this module's padding support follows throughout.
    let content_slot = z_slot.with_rect(content_rect).with_clip(rect);

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
    caret
}

/// The bounding box of this node's children, each probed at its own natural
/// extent — under the surface's own horizontal ceiling when it declares one.
/// A surface with no children is a zero-size point at its anchor.
///
/// # Why the ceiling is offered and not applied afterwards
///
/// A surface that declares `constraints.horizontal.max` has to **offer** it
/// to its children. Every child was probed at `Unspecified` until
/// 2026-09-05, so a wrapping text run answered with one unwrapped line, the
/// placement clamped the box to the ceiling after the fact, and the run's
/// tail was cut mid-word. `popover_with` sets a 368-unit ceiling, so that
/// hit the popover, the toggletip, the tooltip and the AI label panel; only
/// bodies short enough to fit one line escaped, which is why every one of
/// those rows shipped looking fine.
///
/// The vertical axis stays `Unspecified`: a bubble grows downward as far as
/// its content needs, and there is no ceiling to offer.
fn natural_size(node: &ViewNode, ctx: &mut LayoutCtx<'_>, path: &mut KeyPath) -> Size {
    let offer = SizeProposal {
        horizontal: node
            .constraints
            .horizontal
            .max
            .map_or(Proposal::Unspecified, Proposal::Exact),
        vertical: Proposal::Unspecified,
    };
    // Measured under the same empty scroll context `place` uses, so a
    // surface's natural size and its placement agree about what encloses it.
    ctx.outside_scroll(|ctx| {
        let mut size = Size::ZERO;
        for child in &node.children {
            let child_size = crate::layout::measure(child, ctx, path, offer);
            size = size.max(child_size);
        }
        size
    })
}

/// How [`place`] resolves one [`Anchor`] variant.
///
/// Exposed so a caller (or a test) can see which of the four readings a
/// surface got rather than infer it from behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnchorResolution {
    /// Resolved to the declared point.
    Point,
    /// Resolved to the viewport centre, as declared.
    Viewport,
    /// Resolved against the harvested rect of the node it names: the
    /// declared [`Edge`] picks the side, the declared [`Align`] picks where
    /// along it (`contracts/anchored-placement.md` §4 step 1).
    Node,
    /// Declared as [`Anchor::Node`] or [`Anchor::Sibling`], but this walk
    /// harvested no rect for the id it resolves to, so the surface centres
    /// in the viewport instead.
    ///
    /// Tree acceptance refuses an anchor naming no node
    /// ([`crate::tree::Violation::AnchorTargetMissing`]) and the harvest walk
    /// reaches every id it does name, so a frame from
    /// [`crate::frame::petrify`] never lands here. What can is a walk begun
    /// part-way down a tree — `crate::layout::place` called directly on a
    /// subtree whose anchor lives above it — which is a real thing this
    /// crate's own tests do, and centring is the honest answer for it.
    NodeUnharvested,
}

/// How `anchor` will be resolved by [`place`], given what `anchors` holds.
///
/// `surface` is the full key path of the surface carrying `anchor`: what an
/// [`Anchor::Sibling`] resolves against, and what an [`Anchor::Node`]
/// ignores.
#[must_use]
pub fn resolve_anchor_kind(
    anchor: &Anchor,
    surface: &KeyPath,
    anchors: &AnchorRects,
) -> AnchorResolution {
    match anchor {
        Anchor::Point { .. } => AnchorResolution::Point,
        Anchor::Viewport => AnchorResolution::Viewport,
        Anchor::Node { .. } | Anchor::Sibling { .. } => {
            let harvested = anchor
                .target_id(surface)
                .is_some_and(|id| anchors.get(&id).is_some());
            if harvested {
                AnchorResolution::Node
            } else {
                AnchorResolution::NodeUnharvested
            }
        }
    }
}

/// The size a surface declaring `fit` takes against the anchor `plan`
/// harvested, or `None` when the plan harvested no anchor and there is
/// nothing to fit to.
///
/// [`Fit::Anchor`] widens the cross axis — the axis that runs along the
/// anchor's edge — to at least the anchor's own extent on it. The main axis
/// is never touched: how far a menu hangs down is its content's business,
/// not its trigger's. [`Fit::Content`] returns the size unchanged.
fn fit_to_anchor(fit: Fit, plan: &AnchorPlan, natural: Size) -> Option<Size> {
    let anchored = plan.anchored?;
    match fit {
        Fit::Content => Some(natural),
        Fit::Anchor => Some(match anchored.edge.axis() {
            Axis::Vertical => Size::new(natural.w.max(anchored.anchor_rect.w), natural.h),
            Axis::Horizontal => Size::new(natural.w, natural.h.max(anchored.anchor_rect.h)),
        }),
    }
}

/// Base width of a caret, along the near edge of the surface it belongs to.
///
/// Engine geometry rather than a theme token, for the reason
/// `gorgon_petra_egui::paint`'s `SHADOW_GEOMETRY` is: the proportions of a
/// pointer that has to read as a pointer do not change between a light theme
/// and a dark one, and a theme is the wrong place to keep a fact that is the
/// same in both. What the theme does decide is the caret's *colour*, which it
/// already does — the caret paints in the surface's own `background` binding.
pub const CARET_BASE: f32 = 12.0;

/// How far a caret reaches out past the surface's near edge, towards the
/// anchor. See [`CARET_BASE`].
pub const CARET_DEPTH: f32 = 6.0;

/// What the fallback ladder decided, in the form both the surface's rect and
/// its caret read (`contracts/anchored-placement.md` §4, "Resolution
/// record").
///
/// One record rather than two derivations: a caret computed from the declared
/// edge while the box was placed on the resolved one is a pointer aimed at
/// nothing, and it is exactly the kind of disagreement that survives review
/// because both halves look right on their own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved {
    /// The side of the anchor the surface was finally placed on — after the
    /// flip, if there was one.
    pub edge: Edge,
    /// The declared cross-axis alignment. Unchanged by the ladder: a flip
    /// reverses the main axis and never re-aligns the cross one.
    ///
    /// Recorded and asserted, not yet consumed: nothing in the shipped paint
    /// path reads it, because the rect already encodes where the box landed.
    /// It is here because the contract binds this record's field list and
    /// because an explanation of *why* a surface is where it is — the
    /// inspector's question — needs the declaration beside the outcome.
    pub align: Align,
    /// How far the cross axis had to slide to bring the box into the window
    /// (§4 step 4), signed. Zero when the aligned position already fit.
    ///
    /// Recorded on the same terms as [`Resolved::align`]. The caret does not
    /// need it: it is computed from the *final* rect, so a shift is already
    /// accounted for in the projection rather than corrected for afterwards.
    pub shift: f32,
    /// The resolved gap between the anchor's edge and the surface.
    pub offset: f32,
    /// The rect the surface was placed against: the anchor's own rect,
    /// narrowed to the clip that was over it
    /// ([`crate::layout::AnchorRect::visible`]).
    pub anchor_rect: Rect,
}

/// An [`Anchor::Node`] as the ladder needs it, before any clamping.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AnchoredPlan {
    edge: Edge,
    align: Align,
    offset: f32,
    anchor_rect: Rect,
}

impl AnchoredPlan {
    /// This plan plus what the two axes actually did, as one [`Resolved`].
    ///
    /// The flip is read off the *chosen* [`AxisPlacement`] on the main axis
    /// rather than recomputed: whichever side [`flip_axis`] settled on is the
    /// side the box is on, by construction.
    fn resolve(self, plan: &AnchorPlan, x: AxisResult, y: AxisResult, natural: Size) -> Resolved {
        let (main, cross, cross_extent) = match self.edge.axis() {
            Axis::Vertical => (y, x, natural.w),
            Axis::Horizontal => (x, y, natural.h),
        };
        let declared = match self.edge.axis() {
            Axis::Vertical => plan.y,
            Axis::Horizontal => plan.x,
        };
        let edge = if main.chosen == declared {
            self.edge
        } else {
            self.edge.opposite()
        };
        Resolved {
            edge,
            align: self.align,
            shift: cross.origin - cross.chosen.origin(cross_extent),
            offset: self.offset,
            anchor_rect: self.anchor_rect,
        }
    }
}

/// A surface's preferred position, one [`AxisPlacement`] per axis, plus the
/// anchored reading that produced it when there was one.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AnchorPlan {
    x: AxisPlacement,
    y: AxisPlacement,
    /// `Some` only for an [`Anchor::Node`] whose rect was harvested — the
    /// only case with a side to flip about and an anchor to point a caret at.
    anchored: Option<AnchoredPlan>,
}

/// The preferred placement of a surface of `natural` size anchored by
/// `anchor` inside `viewport`, before clamping. `path` is the surface's own
/// full key path, which an [`Anchor::Sibling`] resolves against.
///
/// A node anchor (`Anchor::Node` or `Anchor::Sibling`, one thing once
/// resolved) grows away from the declared [`Edge`] of the harvested anchor
/// rect, gapped by the declared `offset` token, and starts where the
/// declared [`Align`] puts it on the cross axis. The main axis is therefore a
/// *side* — which is what gives [`ClampRule::Flip`] an opposite to try — and
/// the cross axis is a bare origin, because sliding is the only thing the
/// ladder does to a cross axis (§4 step 4).
///
/// `Anchor::Point` is read as the box's top-left corner (the simplest
/// deterministic reading available without an `Edge`). That reading is a side
/// on both axes: the box grows right and down from the point, so the point's
/// other side — box ending at the point — is a real opposite for
/// [`ClampRule::Flip`] to try. `Anchor::Viewport` centres the box and declares
/// no side, and so does a node anchor this walk harvested no rect for
/// (see [`AnchorResolution::NodeUnharvested`]).
fn anchor_placement(
    anchor: &Anchor,
    path: &KeyPath,
    viewport: Rect,
    natural: Size,
    ctx: &LayoutCtx<'_>,
) -> AnchorPlan {
    let centred = AnchorPlan {
        x: AxisPlacement::Centred(viewport.x + (viewport.w - natural.w) / 2.0),
        y: AxisPlacement::Centred(viewport.y + (viewport.h - natural.h) / 2.0),
        anchored: None,
    };
    match resolve_anchor_kind(anchor, path, &ctx.anchors) {
        AnchorResolution::Point => {
            let Anchor::Point { x, y } = anchor else {
                unreachable!("resolve_anchor_kind returned Point for a non-Point anchor")
            };
            AnchorPlan {
                x: AxisPlacement::point(*x, Grow::Forward),
                y: AxisPlacement::point(*y, Grow::Forward),
                anchored: None,
            }
        }
        AnchorResolution::Viewport | AnchorResolution::NodeUnharvested => centred,
        AnchorResolution::Node => {
            let (Some(id), Some(terms)) = (anchor.target_id(path), anchor.node_terms()) else {
                unreachable!("resolve_anchor_kind returned Node for an anchor naming no node")
            };
            let Some(harvested) = ctx.anchors.get(&id) else {
                unreachable!("resolve_anchor_kind returned Node only for a harvested id")
            };
            let anchor_rect = harvested.visible();
            let offset = ctx.spacing(&terms.offset.cloned());
            let main = main_axis_placement(terms.edge, anchor_rect, offset);
            let cross = cross_axis_placement(terms.edge, terms.align, anchor_rect, natural);
            let (x, y) = match terms.edge.axis() {
                Axis::Vertical => (cross, main),
                Axis::Horizontal => (main, cross),
            };
            AnchorPlan {
                x,
                y,
                anchored: Some(AnchoredPlan {
                    edge: terms.edge,
                    align: terms.align,
                    offset,
                    anchor_rect,
                }),
            }
        }
    }
}

/// The main axis of an anchored surface: the declared side of `anchor_rect`,
/// pushed `offset` further away from it, carrying the anchor's *other* side
/// so a flip lands past that edge rather than back across the anchor.
fn main_axis_placement(edge: Edge, anchor_rect: Rect, offset: f32) -> AxisPlacement {
    let (near, far) = match edge.axis() {
        Axis::Vertical => (anchor_rect.y - offset, anchor_rect.bottom() + offset),
        Axis::Horizontal => (anchor_rect.x - offset, anchor_rect.right() + offset),
    };
    match edge {
        // Above (or left of) the anchor: the box's *far* edge sits on the
        // anchor's leading side, so it grows backwards from there, and the
        // flip puts it against the anchor's trailing side.
        Edge::Top | Edge::Left => AxisPlacement::Sided {
            at: near,
            across: far,
            grow: Grow::Backward,
        },
        Edge::Bottom | Edge::Right => AxisPlacement::Sided {
            at: far,
            across: near,
            grow: Grow::Forward,
        },
    }
}

/// The cross axis of an anchored surface: where along the anchor's edge the
/// declared [`Align`] starts the box.
///
/// [`AxisPlacement::Centred`] rather than `Sided` because there is no side
/// here to flip about: `Start` and `End` are the two ends of one edge, not
/// two opposite sides of the anchor, and turning a left-aligned menu into a
/// right-aligned one because the window is narrow would move it out from
/// under the control that opened it. The ladder slides the cross axis instead
/// (§4 step 4), and [`Resolved::shift`] records by how much.
fn cross_axis_placement(
    edge: Edge,
    align: Align,
    anchor_rect: Rect,
    natural: Size,
) -> AxisPlacement {
    let (at, anchor_extent, surface_extent) = match edge.axis() {
        Axis::Vertical => (anchor_rect.x, anchor_rect.w, natural.w),
        Axis::Horizontal => (anchor_rect.y, anchor_rect.h, natural.h),
    };
    AxisPlacement::Centred(align.leading(at, anchor_extent, surface_extent))
}

/// The corner radius this surface paints with, in logical units.
///
/// Read here, in the engine, because the caret's clamp range depends on it
/// (`contracts/anchored-placement.md` §5): a caret placed inside the arc of a
/// rounded corner is drawn hanging off the corner rather than growing out of
/// the edge. `"radius"` is a declared paint slot
/// (`crate::token::standard_slots`), so the name is not invented here, and a
/// surface that binds none paints square corners and needs no inset.
fn corner_radius(node: &ViewNode, ctx: &LayoutCtx<'_>) -> f32 {
    node.props
        .tokens
        .get("radius")
        .and_then(|name| ctx.theme.corner(name))
        .unwrap_or(0.0)
}

/// The caret a resolved surface draws back at its anchor, or `None` when it
/// would not land on the surface's near edge.
///
/// `contracts/anchored-placement.md` §5, in order: project the anchor rect's
/// centre onto the surface's near edge; keep it inside that edge less a
/// corner radius and half the caret's own base at each end; and suppress the
/// caret outright — never slide it to the nearest legal spot — when the
/// projection is off the near edge altogether, or when the edge is too short
/// to hold a caret at all. A pointer clamped onto a surface it is not
/// actually beside points at the wrong thing, which is worse than no pointer.
///
/// The depth is [`CARET_DEPTH`] or the declared gap, whichever is larger: a
/// caret shorter than the gap its surface was pushed away by would float,
/// tip in mid-air, with a band of background between it and the control it
/// points at. [`Resolved::offset`] is the gap, resolved from the anchor's
/// spacing token.
fn caret_of(resolved: &Resolved, rect: Rect, radius: f32) -> Option<CaretPaint> {
    let anchor = resolved.anchor_rect;
    let (projection, near_start, near_end) = match resolved.edge.axis() {
        Axis::Vertical => (anchor.x + anchor.w / 2.0, rect.x, rect.right()),
        Axis::Horizontal => (anchor.y + anchor.h / 2.0, rect.y, rect.bottom()),
    };
    if !projection.is_finite() || projection < near_start || projection > near_end {
        return None;
    }
    let low = near_start + radius + CARET_BASE / 2.0;
    let high = near_end - radius - CARET_BASE / 2.0;
    if low > high {
        return None;
    }
    let along = projection.clamp(low, high);
    let depth = CARET_DEPTH.max(resolved.offset);
    let (tip_x, tip_y) = match resolved.edge {
        // The surface is below the anchor, so its near edge is its top and
        // the caret reaches up out of it.
        Edge::Bottom => (along, rect.y - depth),
        Edge::Top => (along, rect.bottom() + depth),
        Edge::Right => (rect.x - depth, along),
        Edge::Left => (rect.right() + depth, along),
    };
    Some(CaretPaint {
        side: resolved.edge,
        tip_x,
        tip_y,
        w: CARET_BASE,
        h: depth,
    })
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
    /// against `across` instead, growing the other way.
    Sided {
        /// The anchor coordinate the box is placed against on this axis.
        at: f32,
        /// The anchor's *other* side on this axis — where the flip places
        /// the box instead.
        ///
        /// Equal to `at` for an [`Anchor::Point`], which has one coordinate
        /// per axis and so is its own other side; different for an
        /// [`Anchor::Node`], whose anchor is a rect with two edges. Getting
        /// this wrong is not subtle: a menu declared above a 40-high button
        /// and flipped below it would land on the button's *top* edge,
        /// covering the control that opened it.
        across: f32,
        /// Which way the box extends from `at`.
        grow: Grow,
    },
}

impl AxisPlacement {
    /// A box placed against one coordinate, growing away from it, whose
    /// other side is the same coordinate: what an [`Anchor::Point`] declares.
    fn point(at: f32, grow: Grow) -> Self {
        Self::Sided {
            at,
            across: at,
            grow,
        }
    }

    /// The preferred origin for a box of `extent`, before any clamping.
    fn origin(self, extent: f32) -> f32 {
        match self {
            Self::Centred(origin) => origin,
            Self::Sided {
                at,
                grow: Grow::Forward,
                ..
            } => at,
            Self::Sided {
                at,
                grow: Grow::Backward,
                ..
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
            Self::Sided { at, across, grow } => Self::Sided {
                at: across,
                across: at,
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
                ..
            } => (vp_origin + vp_extent - at).max(0.0),
            Self::Sided {
                at,
                grow: Grow::Backward,
                ..
            } => (at - vp_origin).max(0.0),
        }
    }
}

/// What one axis of the ladder settled on.
///
/// A struct rather than a tuple because the ladder now answers three things,
/// and the third — which side the box ended up on — is what the caret reads.
/// A caller that positionally unpacked two floats and a placement would be
/// one reordered field away from pointing every caret the wrong way.
///
/// The wider unclamped extent `ClampRule::Scroll` keeps for its content is
/// deliberately not here. `place` insets the child slot out of the *clamped*
/// rect at every rule, `Scroll` included (see this module's `place`), so the
/// only thing left of the scroll affordance is the flag below.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AxisResult {
    /// Leading coordinate of the placed, visible rect on this axis.
    origin: f32,
    /// Extent of that rect. Always `<= vp_extent`, so a placement never
    /// renders outside the viewport on this axis.
    extent: f32,
    /// Whether the surface needs a scroll affordance on this axis.
    scrolls: bool,
    /// The side the box was finally placed on. Equal to the incoming `plan`
    /// unless [`ClampRule::Flip`] moved it to the anchor's other side.
    chosen: AxisPlacement,
}

/// Clamp one axis of a `plan`ned box of `extent` into
/// `(vp_origin, vp_origin + vp_extent)` by `rule`.
fn clamp_axis(
    plan: AxisPlacement,
    extent: f32,
    vp_origin: f32,
    vp_extent: f32,
    rule: ClampRule,
) -> AxisResult {
    let vp_extent = vp_extent.max(0.0);
    match rule {
        // Neither `Shrink` nor `Scroll` has an opposite-side step, so the
        // side they place on is the side that was declared.
        ClampRule::Shrink => {
            let (origin, extent) = shrink_at(plan, extent, vp_origin, vp_extent);
            AxisResult {
                origin,
                extent,
                scrolls: false,
                chosen: plan,
            }
        }
        ClampRule::Flip => {
            let (origin, extent, chosen) = flip_axis(plan, extent, vp_origin, vp_extent);
            AxisResult {
                origin,
                extent,
                scrolls: false,
                chosen,
            }
        }
        ClampRule::Scroll => {
            if extent <= vp_extent {
                AxisResult {
                    origin: clamp_origin(plan.origin(extent), extent, vp_origin, vp_extent),
                    extent,
                    scrolls: false,
                    chosen: plan,
                }
            } else {
                // The extent itself is kept for the content slot; only the
                // visible placement is bounded to the viewport, which is the
                // scroll affordance the module doc describes.
                AxisResult {
                    origin: vp_origin,
                    extent: vp_extent,
                    scrolls: true,
                    chosen: plan,
                }
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
fn flip_axis(
    plan: AxisPlacement,
    extent: f32,
    vp_origin: f32,
    vp_extent: f32,
) -> (f32, f32, AxisPlacement) {
    let preferred = plan.origin(extent);
    if fits_at(preferred, extent, vp_origin, vp_extent) {
        return (preferred, extent, plan);
    }

    let other = plan.flipped();
    let flipped = other.origin(extent);
    if fits_at(flipped, extent, vp_origin, vp_extent) {
        return (flipped, extent, other);
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
            chosen,
        );
    }
    // The extent is over budget on this axis whatever side it is on, so this
    // is where `Flip`'s documented fallback to `Shrink` applies — at the
    // chosen side, not at the window's near edge.
    let (origin, extent) = shrink_at(chosen, extent, vp_origin, vp_extent);
    (origin, extent, chosen)
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
            ..
        } => {
            let o = near.clamp(vp_origin, vp_end);
            (o, extent.min((vp_end - o).max(0.0)))
        }
        AxisPlacement::Sided {
            at,
            grow: Grow::Backward,
            ..
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
        AnchorPlan, AnchorResolution, AnchoredPlan, AxisPlacement, CARET_BASE, CARET_DEPTH, Grow,
        clamp_axis, cross_axis_placement, main_axis_placement, resolve_anchor_kind, surface_scopes,
    };
    use crate::frame::placement::PlacementList;
    use crate::geom::{Axis, Insets, Point, Rect, Size};
    use crate::layout::{AnchorRects, SizeProposal, Slot};
    use crate::testing::Harness;
    use crate::tree::{
        Align, Anchor, ClampRule, Edge, InputPolicy, Interaction, KeyPath, Layer, NodeKind, Props,
        Role, TextWrap, ViewNode,
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
    /// A surface offers its own horizontal ceiling to its children.
    ///
    /// Every child was probed at `Unspecified` until 2026-09-05, so a
    /// wrapping run answered with one unwrapped line, the placement clamped
    /// the box to the ceiling afterwards, and the run's tail was cut
    /// mid-word. `popover_with` sets a 368-unit ceiling, so it hit the
    /// popover, the toggletip, the tooltip and the AI label panel alike; only
    /// bodies short enough for one line escaped, which is why all four rows
    /// shipped looking fine.
    ///
    /// `MonoContent` is 8 units a character and 16 a line, so twenty
    /// characters under an 80-unit ceiling is ten a line and two lines: 32
    /// tall. Under the old open probe the run answered 160x16 and the box
    /// came out 80 **by 16** — one line's worth of room for two lines of
    /// text. Falsify by putting `SizeProposal::unspecified()` back.
    #[test]
    fn a_surface_offers_its_own_width_ceiling_to_a_wrapping_child() {
        let mut body = ViewNode::new(NodeKind::Text, "body").with_props(Props {
            text: Some("abcdefghijklmnopqrst".to_owned()),
            wrap: Some(TextWrap::Wrap),
            ..Props::default()
        });
        body.constraints.horizontal.max = Some(80.0);

        let mut node = ViewNode::new(NodeKind::Surface, "popup")
            .with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Viewport),
                clamp: Some(ClampRule::Shrink),
                ..Props::default()
            })
            .child(body);
        node.constraints.horizontal.max = Some(80.0);

        let placed = place_surface(&node, Rect::new(0.0, 0.0, 600.0, 400.0));
        assert_eq!(
            placed.rect.w, 80.0,
            "the ceiling still caps the box: {:?}",
            placed.rect
        );
        assert_eq!(
            placed.rect.h, 32.0,
            "twenty characters at ten a line is two lines, and the box has to \
             be tall enough to hold both: {:?}",
            placed.rect
        );
    }

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
        let wrong = clamp_axis(
            AxisPlacement::point(anchor_x, Grow::Forward),
            content_size.w,
            viewport.x,
            viewport.w,
            ClampRule::Shrink,
        );
        let (wrong_x, wrong_w) = (wrong.origin, wrong.extent + padding.along(Axis::Horizontal));
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

    /// The one case left where a `Node` anchor centres in the viewport: a
    /// walk that never saw the node the anchor names, so nothing was
    /// harvested for it.
    ///
    /// Unreachable from `petrify` — tree acceptance refuses an anchor naming
    /// no node — but reachable here, because this test places a bare subtree
    /// directly, and it is the honest answer for that case rather than a
    /// guess at where the missing node would have been.
    #[test]
    fn an_anchor_node_with_no_harvested_rect_centres_in_the_viewport_and_says_so() {
        let anchor = Anchor::Node {
            id: "/some/other/node".into(),
            edge: Edge::Bottom,
            align: Align::Center,
            offset: None,
        };
        assert_eq!(
            resolve_anchor_kind(&anchor, &KeyPath::root(), &AnchorRects::new()),
            AnchorResolution::NodeUnharvested
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
        let none = AnchorRects::new();
        let root = KeyPath::root();
        assert_eq!(
            resolve_anchor_kind(&Anchor::Point { x: 1.0, y: 1.0 }, &root, &none),
            AnchorResolution::Point
        );
        assert_eq!(
            resolve_anchor_kind(&Anchor::Viewport, &root, &none),
            AnchorResolution::Viewport
        );
    }

    // -- Anchor::Sibling: the same rect, named from where a constructor stands. --

    /// A tree whose control and surface are siblings, the shape every
    /// component constructor builds: `root > [button, popup]`. Both sit
    /// under an `overlay`, which hands every child the whole window, and
    /// that is why the control here is itself a surface anchored to a
    /// point: it is the one kind of `overlay` child that takes its natural
    /// size, so it has a real rect to be anchored to. Anchoring a surface to
    /// another anchored surface is a case the harvest walk already covers
    /// (`an_anchor_node_naming_an_anchored_surface_itself_still_waits_for_it`).
    /// `anchor` is whatever spelling the test wants the popup to name the
    /// control by. (This fixture predates `Slot::window`; a stack would do
    /// now, and `a_viewport_anchored_surface_deep_in_a_grid_cell_centres_in_the_window`
    /// covers that shape.)
    fn sibling_tree(anchor: Anchor) -> ViewNode {
        let mut pinned = ViewNode::new(NodeKind::Spacer, "box");
        pinned.constraints.horizontal.min = Some(80.0);
        pinned.constraints.horizontal.max = Some(80.0);
        pinned.constraints.vertical.min = Some(32.0);
        pinned.constraints.vertical.max = Some(32.0);
        let button = ViewNode::new(NodeKind::Surface, "button")
            .with_props(Props {
                layer: Some(Layer::Popup),
                anchor: Some(Anchor::Point { x: 100.0, y: 100.0 }),
                clamp: Some(ClampRule::Shrink),
                input_policy: Some(InputPolicy::Block),
                ..Props::default()
            })
            .child(pinned);
        ViewNode::new(NodeKind::Overlay, "root")
            .child(button)
            .child(surface(
                anchor,
                ClampRule::Flip,
                InputPolicy::DismissOutside,
                Size::new(120.0, 60.0),
            ))
    }

    /// Place `tree` from a fresh harness: everything the walk produced, and
    /// the anchor map it harvested.
    fn placed_with_anchors(tree: &ViewNode) -> (crate::frame::placement::PlacedTree, AnchorRects) {
        let mut h = Harness::new();
        let mut ctx = h.ctx();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            tree,
            &mut ctx,
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 800.0, 600.0)),
            &mut sink,
        );
        let anchors = ctx.anchors.clone();
        (sink.into_parts(), anchors)
    }

    /// The whole claim of `Anchor::Sibling`: a bare key resolved against the
    /// surface's own parent path is *the same anchor* as the canonical id
    /// written out by hand. Not "lands somewhere plausible" — the placement
    /// and paint lists are equal, so the harvest, the ladder and the caret
    /// all saw one rect and one set of terms.
    #[test]
    fn a_sibling_anchor_places_exactly_as_the_canonical_node_anchor_does() {
        let by_key = Anchor::Sibling {
            key: "button".into(),
            edge: Edge::Bottom,
            align: Align::Center,
            offset: None,
        };
        let (a, _) = placed_with_anchors(&sibling_tree(Anchor::Node {
            id: "/root/button".into(),
            edge: Edge::Bottom,
            align: Align::Center,
            offset: None,
        }));
        let (b, anchors) = placed_with_anchors(&sibling_tree(by_key.clone()));
        assert_eq!(
            a.placements, b.placements,
            "the two spellings of one anchor placed differently"
        );
        assert_eq!(
            a.content, b.content,
            "the two spellings of one anchor painted differently"
        );

        // And it really was anchored: the popup hangs off the button's
        // bottom edge, centred on it — a sibling key the harvest could not
        // resolve would have centred the surface in the window at
        // (340, 270) instead — and it carries a caret, which `place`
        // attaches only for an anchor that resolved to a harvested rect
        // (`a_sibling_anchor_does_not_search_outward_for_its_key` is the
        // other half).
        let button = b
            .placements
            .iter()
            .find(|p| p.id == "/root/button")
            .unwrap();
        let popup = b
            .placements
            .iter()
            .position(|p| p.id == "/root/popup")
            .unwrap();
        assert_eq!(button.rect, Rect::new(100.0, 100.0, 80.0, 32.0));
        assert_eq!(
            b.placements[popup].rect,
            Rect::new(80.0, 132.0, 120.0, 60.0)
        );
        assert!(
            b.content[popup].caret.is_some(),
            "an anchored surface points at its anchor"
        );
        assert_eq!(
            anchors.get("/root/button").map(|r| r.rect),
            Some(button.rect),
            "the harvest keyed the sibling's rect by its canonical id"
        );
        let surface_path = KeyPath::root().child(&"root".into()).child(&"popup".into());
        assert_eq!(
            resolve_anchor_kind(&by_key, &surface_path, &anchors),
            AnchorResolution::Node
        );
    }

    /// A sibling key is resolved against the surface's own child list and
    /// nowhere else. The button here is a *cousin* — one level up — and the
    /// popup's key names nothing beside it, so the harvest has no rect for
    /// it and the surface centres. (Acceptance refuses this tree outright;
    /// this test places the bare subtree to show the walk agrees with the
    /// refusal rather than quietly reaching outward.)
    #[test]
    fn a_sibling_anchor_does_not_search_outward_for_its_key() {
        let mut button = ViewNode::new(NodeKind::Spacer, "button");
        button.constraints.horizontal.min = Some(80.0);
        button.constraints.horizontal.max = Some(80.0);
        button.constraints.vertical.min = Some(32.0);
        button.constraints.vertical.max = Some(32.0);
        let tree = ViewNode::new(NodeKind::Overlay, "root")
            .child(button)
            .child(ViewNode::new(NodeKind::Overlay, "inner").child(surface(
                Anchor::Sibling {
                    key: "button".into(),
                    edge: Edge::Bottom,
                    align: Align::Center,
                    offset: None,
                },
                ClampRule::Flip,
                InputPolicy::DismissOutside,
                Size::new(100.0, 50.0),
            )));
        let (placed, anchors) = placed_with_anchors(&tree);
        let popup = placed
            .placements
            .iter()
            .position(|p| p.id == "/root/inner/popup")
            .unwrap();
        assert_eq!(
            placed.placements[popup].rect,
            Rect::new(350.0, 275.0, 100.0, 50.0),
            "an unresolvable sibling key centres, exactly as an unharvested node id does"
        );
        assert!(
            placed.content[popup].caret.is_none(),
            "and a centred surface has no anchor to point a caret at"
        );
        assert!(
            anchors.get("/root/button").is_none() && anchors.get("/root/inner/button").is_none(),
            "nothing was harvested: the key resolved to `/root/inner/button`, which no node has"
        );
    }

    // -- Anchor::Node: the harvested rect, the edge, the align, the caret. --

    /// A tree shaped the way a real popover is: an `overlay` root holding the
    /// control and the surface as siblings, so both are offered the whole
    /// window and the surface anchors against the control's own rect rather
    /// than against a box some ancestor's flow put it in.
    ///
    /// The control is inside a `stack` so that it takes its natural size and
    /// not the whole window — an anchor rect equal to the viewport would make
    /// every `Align` and every `Edge` land in the same place, and the tests
    /// below could not tell them apart.
    fn anchored_tree(
        edge: Edge,
        align: Align,
        offset: Option<crate::token::TokenName>,
        clamp: ClampRule,
        control: Size,
        popup: Size,
        pad: f32,
    ) -> ViewNode {
        let mut button = ViewNode::new(NodeKind::Spacer, "button");
        button.constraints.horizontal.min = Some(control.w);
        button.constraints.horizontal.max = Some(control.w);
        button.constraints.vertical.min = Some(control.h);
        button.constraints.vertical.max = Some(control.h);
        ViewNode::new(NodeKind::Overlay, "root")
            .child(
                ViewNode::new(NodeKind::Stack, "bar")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        align: Some(crate::geom::Align::Start),
                        // `pad` moves the control off the window's corner, so
                        // a test that wants all four sides to have room can
                        // ask for it. At zero the control sits at the origin
                        // and `Top` and `Left` have none, which is what the
                        // flip cases want instead.
                        padding: (pad > 0.0)
                            .then(|| crate::tree::InsetRefs::all(crate::testing::gap_token(pad))),
                        ..Props::default()
                    })
                    .child(button),
            )
            .child(surface(
                Anchor::Node {
                    id: "/root/bar/button".into(),
                    edge,
                    align,
                    offset,
                },
                clamp,
                InputPolicy::DismissOutside,
                popup,
            ))
    }

    /// Place `tree` and hand back the anchor's placement and the surface's,
    /// by id, so a test asserts one against the other rather than against a
    /// number that would still pass if the surface had centred in the window.
    fn place_anchored(
        tree: &ViewNode,
        viewport: Rect,
    ) -> (
        crate::frame::placement::Placement,
        crate::frame::placement::Placement,
        crate::frame::placement::PaintContent,
    ) {
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(viewport),
            &mut sink,
        );
        let parts = sink.into_parts();
        let at = |id: &str| {
            parts
                .placements
                .iter()
                .position(|p| p.id == id)
                .unwrap_or_else(|| panic!("{id} was placed"))
        };
        let button = at("/root/bar/button");
        let popup = at("/root/popup");
        (
            parts.placements[button].clone(),
            parts.placements[popup].clone(),
            parts.content[popup].clone(),
        )
    }

    /// T009's whole claim: `Anchor::Node::edge` decides which side of the
    /// harvested rect the surface is placed against.
    ///
    /// All four edges, each against the anchor's own placement rather than
    /// against a hand-copied number — a surface that fell back to viewport
    /// centring would fail every one of them, and so would one that read the
    /// edge but harvested the wrong rect.
    #[test]
    fn an_anchor_node_edge_picks_the_side_of_the_harvested_rect() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        // Small enough that all four sides of a control inset by 60 have
        // room: this test is about which side is chosen, not about the flip.
        let popup = Size::new(40.0, 30.0);
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let tree = anchored_tree(
                edge,
                Align::Center,
                None,
                ClampRule::Flip,
                Size::new(100.0, 40.0),
                popup,
                60.0,
            );
            let (button, surface, _) = place_anchored(&tree, viewport);
            match edge {
                Edge::Bottom => assert_eq!(
                    surface.rect.y,
                    button.rect.bottom(),
                    "a bottom anchor puts the surface's top on the anchor's bottom"
                ),
                Edge::Top => assert_eq!(
                    surface.rect.bottom(),
                    button.rect.y,
                    "a top anchor puts the surface's bottom on the anchor's top"
                ),
                Edge::Right => assert_eq!(surface.rect.x, button.rect.right()),
                Edge::Left => assert_eq!(surface.rect.right(), button.rect.x),
            }
        }
    }

    /// `align` is the cross-axis half, and the three readings are three
    /// different numbers on an anchor narrower than the surface.
    #[test]
    fn an_anchor_node_align_picks_where_along_that_side_the_surface_starts() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let control = Size::new(100.0, 40.0);
        let popup = Size::new(60.0, 30.0);
        let x_of = |align| {
            let tree = anchored_tree(
                Edge::Bottom,
                align,
                None,
                ClampRule::Flip,
                control,
                popup,
                0.0,
            );
            let (button, surface, _) = place_anchored(&tree, viewport);
            (surface.rect.x, button.rect.x, button.rect.w)
        };
        let (start, bx, bw) = x_of(Align::Start);
        assert_eq!(start, bx);
        let (centre, _, _) = x_of(Align::Center);
        assert_eq!(centre, bx + (bw - popup.w) / 2.0);
        let (end, _, _) = x_of(Align::End);
        assert_eq!(end, bx + bw - popup.w);
        assert!(start < centre && centre < end, "the three must differ");
    }

    /// `offset` is a spacing token, resolved through the same path
    /// `props.spacing` takes, and it pushes the surface away from the anchor
    /// rather than moving the anchor.
    #[test]
    fn an_anchor_node_offset_gaps_the_surface_away_from_its_anchor() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let control = Size::new(100.0, 40.0);
        let popup = Size::new(60.0, 30.0);
        let gap = crate::testing::gap_token(8.0);
        let tree = anchored_tree(
            Edge::Bottom,
            Align::Center,
            Some(gap),
            ClampRule::Flip,
            control,
            popup,
            0.0,
        );
        let (button, surface, _) = place_anchored(&tree, viewport);
        assert_eq!(
            surface.rect.y,
            button.rect.bottom() + 8.0,
            "the declared spacing token is the gap, and the anchor did not move"
        );
    }

    /// The caret rides the resolved side, and the resolved side is the one
    /// the ladder ended on — not the one that was declared.
    ///
    /// The control sits at the top of the window, so a `Top` anchor cannot
    /// hold a 200-high surface above it and the ladder flips to `Bottom`.
    /// A caret computed from the declared edge would point up, off the top of
    /// its own box, at nothing.
    #[test]
    fn an_anchor_node_caret_follows_the_flip_rather_than_the_declaration() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let tree = anchored_tree(
            Edge::Top,
            Align::Center,
            None,
            ClampRule::Flip,
            Size::new(100.0, 40.0),
            Size::new(120.0, 200.0),
            0.0,
        );
        let (button, surface, content) = place_anchored(&tree, viewport);
        assert_eq!(
            surface.rect.y,
            button.rect.bottom(),
            "there is no room above, so the ladder must take the other side"
        );
        let caret = content.caret.expect("an anchored surface carries a caret");
        assert_eq!(
            caret.side,
            Edge::Bottom,
            "the caret rides the resolved side"
        );
        assert_eq!(
            (caret.tip_x, caret.tip_y),
            (
                button.rect.x + button.rect.w / 2.0,
                surface.rect.y - CARET_DEPTH
            ),
            "the tip is the anchor's centre projected onto the surface's near edge"
        );
        assert_eq!((caret.w, caret.h), (CARET_BASE, CARET_DEPTH));
    }

    /// A caret whose projection falls off the surface's near edge is
    /// suppressed rather than slid to the nearest legal spot: a pointer that
    /// is not beside its anchor points at the wrong thing.
    ///
    /// `Align::End` on an anchor much wider than the surface puts the
    /// surface's whole near edge past the anchor's centre, so the projection
    /// lands left of `rect.x`.
    #[test]
    fn an_anchor_node_caret_is_suppressed_when_it_would_miss_the_near_edge() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let tree = anchored_tree(
            Edge::Bottom,
            Align::End,
            None,
            ClampRule::Flip,
            Size::new(400.0, 40.0),
            Size::new(40.0, 30.0),
            0.0,
        );
        let (button, surface, content) = place_anchored(&tree, viewport);
        assert!(
            button.rect.x + button.rect.w / 2.0 < surface.rect.x,
            "the fixture must actually put the anchor's centre off the near edge"
        );
        assert_eq!(
            content.caret, None,
            "a caret that cannot land on the near edge is not drawn at all"
        );
    }

    /// The record `place` builds is what both the rect and the caret read,
    /// and it reports what actually happened rather than what was declared.
    ///
    /// Hand-computed against a 40x20 anchor at (100, 100) with an 8-unit gap
    /// and a 60x30 surface: a bottom anchor puts the box at y = 128, so a
    /// 130-high window cannot hold it and the ladder takes the anchor's other
    /// side (`edge` becomes `Top`); a 120-wide window cannot hold the centred
    /// cross axis at x = 90 either, so it slides to 60 and `shift` records
    /// the -30 that took.
    #[test]
    fn the_resolution_record_reports_the_flip_the_shift_and_the_declaration() {
        let anchor_rect = Rect::new(100.0, 100.0, 40.0, 20.0);
        let natural = Size::new(60.0, 30.0);
        let declared = AnchoredPlan {
            edge: Edge::Bottom,
            align: Align::Center,
            offset: 8.0,
            anchor_rect,
        };
        let plan = AnchorPlan {
            x: cross_axis_placement(Edge::Bottom, Align::Center, anchor_rect, natural),
            y: main_axis_placement(Edge::Bottom, anchor_rect, 8.0),
            anchored: Some(declared),
        };
        assert_eq!(
            plan.x.origin(natural.w),
            90.0,
            "centred on a 40-wide anchor at 100 with a 60-wide box"
        );

        let roomy_x = clamp_axis(plan.x, natural.w, 0.0, 800.0, ClampRule::Flip);
        let roomy_y = clamp_axis(plan.y, natural.h, 0.0, 600.0, ClampRule::Flip);
        let kept = declared.resolve(&plan, roomy_x, roomy_y, natural);
        assert_eq!(kept.edge, Edge::Bottom, "a window with room keeps the side");
        assert_eq!(kept.align, Align::Center, "the ladder never re-aligns");
        assert_eq!(kept.offset, 8.0, "the resolved gap, not the token name");
        assert_eq!(kept.shift, 0.0, "nothing had to slide");
        assert_eq!(kept.anchor_rect, anchor_rect);

        let short_y = clamp_axis(plan.y, natural.h, 0.0, 130.0, ClampRule::Flip);
        let narrow_x = clamp_axis(plan.x, natural.w, 0.0, 120.0, ClampRule::Flip);
        let moved = declared.resolve(&plan, narrow_x, short_y, natural);
        assert_eq!(
            moved.edge,
            Edge::Top,
            "128 + 30 does not fit in 130, so the box takes the anchor's top"
        );
        assert_eq!(
            short_y.origin, 62.0,
            "92 - 30: against the anchor's top edge"
        );
        assert_eq!(moved.shift, -30.0, "90 slid back to 60 to fit a 120 window");
        assert_eq!(
            moved.align,
            Align::Center,
            "a cross-axis slide is not a change of alignment"
        );
    }

    /// A declared gap wider than the caret's own depth stretches the caret to
    /// bridge it, rather than leaving it floating short of the control.
    #[test]
    fn an_anchor_node_caret_spans_a_gap_wider_than_its_own_depth() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let tree = anchored_tree(
            Edge::Bottom,
            Align::Center,
            Some(crate::testing::gap_token(14.0)),
            ClampRule::Flip,
            Size::new(100.0, 40.0),
            Size::new(60.0, 30.0),
            0.0,
        );
        let (button, surface, content) = place_anchored(&tree, viewport);
        let caret = content.caret.expect("an anchored surface carries a caret");
        assert_eq!(surface.rect.y, button.rect.bottom() + 14.0);
        assert_eq!(caret.h, 14.0, "the caret reaches the whole way across");
        assert_eq!(
            caret.tip_y,
            button.rect.bottom(),
            "so its tip lands on the anchor's own edge"
        );
        assert_eq!(caret.w, CARET_BASE, "the base is not stretched with it");
    }

    /// A surface anchored to a node *inside* another anchored surface
    /// resolves against where that surface actually landed
    /// (`contracts/anchored-placement.md` §1.4).
    ///
    /// This is the case one harvest walk cannot answer. On the first walk the
    /// outer surface has no anchor rect yet, so it centres in the viewport
    /// and its child lands there; a scheme that stopped after one walk would
    /// put the inner surface under *that* rect, hundreds of units from the
    /// control the outer one is actually beside. The assertion is against the
    /// outer surface's own placed child, so it fails for exactly that
    /// mistake and for no other.
    #[test]
    fn an_anchor_node_inside_an_anchored_surface_resolves_against_where_it_landed() {
        let mut item = ViewNode::new(NodeKind::Spacer, "item");
        item.constraints.horizontal.min = Some(90.0);
        item.constraints.horizontal.max = Some(90.0);
        item.constraints.vertical.min = Some(24.0);
        item.constraints.vertical.max = Some(24.0);
        let mut button = ViewNode::new(NodeKind::Spacer, "button");
        button.constraints.horizontal.min = Some(100.0);
        button.constraints.horizontal.max = Some(100.0);
        button.constraints.vertical.min = Some(40.0);
        button.constraints.vertical.max = Some(40.0);
        let node_anchor = |id: &str| Anchor::Node {
            id: id.to_owned(),
            edge: Edge::Bottom,
            align: Align::Center,
            offset: None,
        };
        let surface_at = |key: &str, anchor: Anchor, child: ViewNode| {
            ViewNode::new(NodeKind::Surface, key)
                .with_props(Props {
                    layer: Some(Layer::Popup),
                    anchor: Some(anchor),
                    clamp: Some(ClampRule::Flip),
                    input_policy: Some(InputPolicy::DismissOutside),
                    ..Props::default()
                })
                .child(child)
        };
        let mut body = ViewNode::new(NodeKind::Spacer, "body");
        body.constraints.horizontal.min = Some(50.0);
        body.constraints.horizontal.max = Some(50.0);
        body.constraints.vertical.min = Some(20.0);
        body.constraints.vertical.max = Some(20.0);

        let tree = ViewNode::new(NodeKind::Overlay, "root")
            .child(
                ViewNode::new(NodeKind::Stack, "bar")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        align: Some(crate::geom::Align::Start),
                        ..Props::default()
                    })
                    .child(button),
            )
            .child(surface_at("outer", node_anchor("/root/bar/button"), item))
            .child(surface_at("inner", node_anchor("/root/outer/item"), body));

        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 800.0, 600.0)),
            &mut sink,
        );
        let placed = sink.into_vec();
        let rect = |id: &str| {
            placed
                .iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("{id} was placed"))
                .rect
        };

        let button_rect = rect("/root/bar/button");
        let item_rect = rect("/root/outer/item");
        assert_eq!(
            rect("/root/outer").y,
            button_rect.bottom(),
            "the outer surface hangs off the control"
        );
        assert_eq!(
            rect("/root/inner").y,
            item_rect.bottom(),
            "and the inner one hangs off the outer surface's own child"
        );
        // The number a single-walk scheme would have produced, named so the
        // assertion above cannot be satisfied by accident: a 90x24 item
        // inside a surface centred in an 800x600 window sits far below this.
        assert!(
            item_rect.y < 200.0,
            "the item must follow the control, not the window centre: {item_rect:?}"
        );
    }

    /// The other shape of the same nesting: a surface anchored to another
    /// anchored surface's **own** rect, rather than to a node inside it.
    ///
    /// A separate case because the depth rule can get one right and the other
    /// wrong. Counting only the anchored surfaces *above* a target leaves
    /// this one at depth zero — the target is not inside any surface, it *is*
    /// one — so the whole thing runs on a single walk and the inner surface
    /// resolves against where the outer one had not been placed yet.
    #[test]
    fn an_anchor_node_naming_an_anchored_surface_itself_still_waits_for_it() {
        let mut button = ViewNode::new(NodeKind::Spacer, "button");
        button.constraints.horizontal.min = Some(100.0);
        button.constraints.horizontal.max = Some(100.0);
        button.constraints.vertical.min = Some(40.0);
        button.constraints.vertical.max = Some(40.0);
        let node_anchor = |id: &str| Anchor::Node {
            id: id.to_owned(),
            edge: Edge::Bottom,
            align: Align::Center,
            offset: None,
        };
        let surface_at = |key: &str, anchor: Anchor, size: Size| {
            let mut body = ViewNode::new(NodeKind::Spacer, "body");
            body.constraints.horizontal.min = Some(size.w);
            body.constraints.horizontal.max = Some(size.w);
            body.constraints.vertical.min = Some(size.h);
            body.constraints.vertical.max = Some(size.h);
            ViewNode::new(NodeKind::Surface, key)
                .with_props(Props {
                    layer: Some(Layer::Popup),
                    anchor: Some(anchor),
                    clamp: Some(ClampRule::Flip),
                    input_policy: Some(InputPolicy::DismissOutside),
                    ..Props::default()
                })
                .child(body)
        };

        let tree = ViewNode::new(NodeKind::Overlay, "root")
            .child(
                ViewNode::new(NodeKind::Stack, "bar")
                    .with_props(Props {
                        axis: Some(Axis::Vertical),
                        align: Some(crate::geom::Align::Start),
                        ..Props::default()
                    })
                    .child(button),
            )
            .child(surface_at(
                "outer",
                node_anchor("/root/bar/button"),
                Size::new(90.0, 24.0),
            ))
            .child(surface_at(
                "inner",
                node_anchor("/root/outer"),
                Size::new(50.0, 20.0),
            ));

        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 800.0, 600.0)),
            &mut sink,
        );
        let placed = sink.into_vec();
        let rect = |id: &str| {
            placed
                .iter()
                .find(|p| p.id == id)
                .unwrap_or_else(|| panic!("{id} was placed"))
                .rect
        };
        assert_eq!(
            rect("/root/outer").y,
            rect("/root/bar/button").bottom(),
            "the outer surface hangs off the control"
        );
        assert_eq!(
            rect("/root/inner").y,
            rect("/root/outer").bottom(),
            "and the inner one hangs off the outer surface's own rect, not \
             off where an unresolved first walk left it"
        );
    }

    /// The harvest is not a second measurement pass and not a second frame:
    /// placing the same tree twice, from two independent harnesses, produces
    /// the identical anchored rect.
    #[test]
    fn an_anchor_node_resolves_identically_on_two_independent_passes() {
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let tree = anchored_tree(
            Edge::Bottom,
            Align::Center,
            None,
            ClampRule::Flip,
            Size::new(100.0, 40.0),
            Size::new(120.0, 60.0),
            0.0,
        );
        let first = place_anchored(&tree, viewport);
        let second = place_anchored(&tree, viewport);
        assert_eq!(first.1.rect, second.1.rect);
        assert_eq!(first.2.caret, second.2.caret);
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
                0 => AxisPlacement::point(at, Grow::Forward),
                1 => AxisPlacement::point(at, Grow::Backward),
                _ => AxisPlacement::Centred(at),
            };
            let ax = clamp_axis(axis(ox), w, 0.0, vw, rule);
            let ay = clamp_axis(axis(oy), h, 0.0, vh, rule);
            let (x, rw, y, rh) = (ax.origin, ax.extent, ay.origin, ay.extent);
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
            let placed = clamp_axis(
                AxisPlacement::point(at, Grow::Forward),
                w,
                0.0,
                vw,
                ClampRule::Flip,
            );
            let (x, rw) = (placed.origin, placed.extent);
            proptest::prop_assert!(!placed.scrolls);
            proptest::prop_assert_eq!(
                placed.chosen,
                AxisPlacement::point(at, Grow::Backward),
                "the ladder must report the side it actually placed on, \
                 because that is what the caret is drawn from"
            );
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

    // -- A surface takes no room in its parent's flow. ---------------------

    /// The dispatcher answers zero for a surface under every proposal, and
    /// its own constraints do not change that: a scrim declaring a minimum
    /// wider than any window must not report that minimum to the card that
    /// declares it.
    #[test]
    fn measure_answers_zero_to_the_flow_parent_whatever_the_constraints_say() {
        let mut node = surface(
            Anchor::Point { x: 0.0, y: 0.0 },
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(64.0, 64.0),
        );
        node.constraints.horizontal.min = Some(f32::MAX);
        node.constraints.vertical.min = Some(f32::MAX);
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let zero = crate::layout::measure(&node, &mut h.ctx(), &mut path, SizeProposal::zero());
        let unbounded =
            crate::layout::measure(&node, &mut h.ctx(), &mut path, SizeProposal::unbounded());
        assert_eq!(zero, Size::ZERO);
        assert_eq!(unbounded, Size::ZERO);
        assert!(
            path.is_empty(),
            "the dispatcher leaves the path as it found it"
        );
    }

    /// A vertical stack of two 30-high rows and a surface between them: the
    /// rows abut exactly as they would with no surface at all, and the
    /// surface still places its own natural box.
    #[test]
    fn a_surface_between_two_rows_does_not_push_the_second_row_down() {
        fn row(key: &str) -> ViewNode {
            let mut node = ViewNode::new(NodeKind::Spacer, key);
            node.constraints.vertical.min = Some(30.0);
            node.constraints.vertical.max = Some(30.0);
            node
        }
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(row("a"))
            .child(surface(
                Anchor::Point { x: 10.0, y: 10.0 },
                ClampRule::Shrink,
                InputPolicy::Passthrough,
                Size::new(64.0, 64.0),
            ))
            .child(row("b"));
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 400.0, 300.0)),
            &mut sink,
        );
        let placed = sink.into_vec();
        let rect_of = |id: &str| placed.iter().find(|p| p.id == id).map(|p| p.rect);
        assert_eq!(rect_of("/root/a"), Some(Rect::new(0.0, 0.0, 400.0, 30.0)));
        assert_eq!(
            rect_of("/root/b"),
            Some(Rect::new(0.0, 30.0, 400.0, 30.0)),
            "row b sits directly under row a: the surface took no flow room"
        );
        assert_eq!(
            rect_of("/root/popup"),
            Some(Rect::new(10.0, 10.0, 64.0, 64.0)),
            "and the surface still has its own natural box at its anchor"
        );
    }

    // -- The window is the walk's window, not the cell a parent offered. ----

    /// A viewport-anchored surface declared three containers deep, inside a
    /// grid cell that clips it, centres in the *window* — the exact defect
    /// that put a modal inside the page card that declared it.
    #[test]
    fn a_viewport_anchored_surface_deep_in_a_grid_cell_centres_in_the_window() {
        let window = Rect::new(0.0, 0.0, 800.0, 600.0);
        let popup = surface(
            Anchor::Viewport,
            ClampRule::Shrink,
            InputPolicy::Block,
            Size::new(100.0, 50.0),
        );
        let card = ViewNode::new(NodeKind::Stack, "card")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "filler"))
            .child(popup);
        // Two columns; the card sits in the narrow right-hand one, so its
        // cell is nowhere near the window's centre.
        let tree = ViewNode::new(NodeKind::Grid, "root")
            .with_props(Props {
                columns: vec![
                    crate::tree::TrackSize::Weight { weight: 3.0 },
                    crate::tree::TrackSize::Weight { weight: 1.0 },
                ],
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "left"))
            .child(card);
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(window), &mut sink);
        let placed = sink.into_vec();
        let popup = placed
            .iter()
            .find(|p| p.id == "/root/card/popup")
            .expect("the surface is placed");
        assert_eq!(
            popup.rect,
            Rect::new(350.0, 275.0, 100.0, 50.0),
            "centred in the 800x600 window, not in the 200-wide cell at x=600"
        );
    }

    /// The scrim idiom: a viewport-centred `Shrink` surface whose declared
    /// minimum exceeds the window is placed on the window rect exactly, from
    /// any depth.
    #[test]
    fn a_shrink_surface_asking_for_more_than_the_window_gets_the_window() {
        let window = Rect::new(0.0, 0.0, 800.0, 600.0);
        let mut scrim = ViewNode::new(NodeKind::Surface, "scrim").with_props(Props {
            layer: Some(Layer::Modal),
            anchor: Some(Anchor::Viewport),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::Block),
            ..Props::default()
        });
        scrim.constraints.horizontal.min = Some(f32::MAX);
        scrim.constraints.vertical.min = Some(f32::MAX);
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "filler"))
            .child(scrim);
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(window), &mut sink);
        let placed = sink.into_vec();
        let scrim = placed
            .iter()
            .find(|p| p.id == "/root/scrim")
            .expect("the scrim is placed");
        assert_eq!(scrim.rect, window);
        assert_eq!(scrim.clip, window);
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
