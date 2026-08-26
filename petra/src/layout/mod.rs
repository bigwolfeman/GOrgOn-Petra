//! Layout negotiation: proposals down, responses up, one placement per node.
//!
//! The engine measures with [`measure`] and places with [`place`]. Both
//! dispatch on [`crate::tree::NodeKind`] into the per-container modules in this
//! directory; both are pure functions of the tree, the state snapshot, the
//! viewport, the theme snapshot, and the scale (FR-006). Nothing in this module
//! knows about a toolkit: content measurement enters through
//! [`ContentMeasure`] and row materialization through [`RowSource`], and both
//! are implemented outside this crate.

pub mod constraints;
pub mod grid;
pub mod leaf;
pub mod overlay;
pub mod overlay_surface;
pub mod proposal;
pub mod reuse;
pub mod scroll;
pub mod stack;
pub mod text;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use crate::frame::placement::{
    CaretPaint, PaintContent, PlacementList, PlacementSemantics, PlacementSink, TextPaint,
};
use crate::geom::{Insets, Rect, Scale, Size};
use crate::input::Capture;
use crate::token::{ThemeSnapshot, TokenName};
use crate::tree::props::ScrollProps;
use crate::tree::{Anchor, InsetRefs, KeyPath, NodeKind, Role, TextWrap, ViewNode};

pub use proposal::{ChangeSet, MeasureCache, MeasureKey, Proposal, SizeProposal};

/// What a text node needs measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRequest<'a> {
    /// The content.
    pub text: &'a str,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<&'a str>,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
    /// Width the run may use, or `None` for an open probe.
    pub available_width: Option<f32>,
}

/// What a shaper answers with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMeasurement {
    /// Extent of the shaped run in logical units.
    pub size: Size,
    /// Whether content was dropped to fit.
    pub truncated: bool,
    /// Rendered line count.
    pub lines: usize,
}

/// Measurement of content the engine cannot size on its own.
///
/// This is the U-09 boundary in trait form: `gorgon-petra` declares what it
/// needs measured, `gorgon-petra-egui` answers with shaped galleys, and no
/// egui type crosses.
pub trait ContentMeasure {
    /// Shape a text run.
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement;
    /// Size an image for an offer.
    fn image(&mut self, source: &str, proposal: SizeProposal) -> Size;
    /// Size a host-registered custom node for an offer.
    fn custom(&mut self, name: &str, proposal: SizeProposal) -> Size;
}

/// Materialization of virtualized collection rows.
///
/// A `collection` node declares a row count and a source name; the rows
/// themselves live in the store (D-075), so the engine asks for the window it
/// is about to place and never holds the whole list.
pub trait RowSource {
    /// The rows in `range` of `source`, in order. A source that cannot answer
    /// returns fewer rows; the caller reports the shortfall rather than
    /// fabricating nodes.
    ///
    /// A `collection` re-fetches its rows every frame, so a row that is not
    /// shared can never be pointer-equal to what a previous frame placed and
    /// its subtree can never be reused. Returning the same `Arc` a previous
    /// frame handed back for an unchanged row is what makes a caching
    /// implementation's subtree reusable; returning a fresh `Arc` each time
    /// is correct, only slower.
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<Arc<ViewNode>>;
}

/// The read-only state one frame negotiates against.
///
/// # The last three members are not measure inputs
///
/// `hovered`, `pressed` and `capture` are the host's pointer snapshot
/// (`contracts/interaction-state.md` §2), and they are here for exactly one
/// consumer: [`semantics_of`], which runs *after* a container has decided
/// what size a node takes. **No container may branch on them during
/// measurement.** A layout that got wider on hover would re-lay-out the page
/// under the pointer, which is the reflow every hover-driven design system
/// exists to avoid, and it would make the frame's geometry a function of
/// where a mouse happens to be resting — so an idle window would never be
/// byte-identical twice.
///
/// This cannot be enforced by the type, because `LayoutCtx` hands the whole
/// `LayoutState` to every container and that handoff is what `scroll_offsets`
/// needs. It is enforced by a gate instead: `tests/zero_idle_interaction.rs`
/// places one tree twice with different `hovered` values and requires every
/// measured rect to be byte-identical (§9).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutState {
    /// Scroll offset per scroll-container id, in logical units along its axis.
    pub scroll_offsets: BTreeMap<String, f32>,
    /// The focused node id, if any.
    pub focused: Option<String>,
    /// The one node the pointer is over, if any. Derived by the host from
    /// [`crate::input::hit_test`], never declared by an application — see the
    /// type doc above for why it may not be read during measurement.
    pub hovered: Option<String>,
    /// The node that is pressed: it holds the pointer capture *and* the
    /// pointer is still inside its rect
    /// ([`crate::input::PointerState::pressed`]).
    pub pressed: Option<String>,
    /// The pointer capture in force, if any. Outlives `pressed`: a button
    /// dragged off its own rect is still captured and no longer pressed.
    pub capture: Option<Capture>,
}

impl LayoutState {
    /// The scroll offset for `id`, clamped to zero or more.
    #[must_use]
    pub fn scroll_offset(&self, id: &str) -> f32 {
        self.scroll_offsets
            .get(id)
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .max(0.0)
    }
}

/// One enclosing `scroll` container, as the subtree inside it sees it.
///
/// A `collection` is virtualized against the `scroll` that carries it, and
/// the scrolling parameters belong to that ancestor rather than to the list
/// (`contracts/view-tree.md` §"Virtualized collections": "a `scroll`
/// container with a collection child materializes only the visible window
/// plus declared overscan"). The walk is what knows the ancestry, so the walk
/// is what carries it: [`scroll::place`] and [`scroll::measure`] push a frame
/// around their child, and [`scroll::place_collection`] reads the innermost
/// one back off [`LayoutCtx::enclosing_scroll`].
///
/// The frame deliberately carries no offset and no viewport rect. The offset
/// is already in the geometry — a `scroll` places its child at `-offset`, so
/// the distance from a descendant's own rect to the clip in force *is* the
/// offset that reached it, composed across however many containers and
/// however many nested scrolls sit between them. Copying the number in here
/// as well would be a second source for one fact.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollFrame {
    /// Canonical id of the `scroll` node this frame belongs to.
    pub id: String,
    /// That node's resolved scrolling parameters.
    pub props: ScrollProps,
}

/// The chain of `scroll` ancestors in force at one point in the walk,
/// innermost last.
///
/// Constructed empty and mutated only by [`LayoutCtx::within_scroll`] and
/// [`LayoutCtx::outside_scroll`], which bracket exactly one call each: no
/// container can push without popping.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollStack(Vec<ScrollFrame>);

impl ScrollStack {
    /// An empty stack: the walk starts outside every scroll container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The innermost enclosing `scroll`, or `None` outside every scroll.
    #[must_use]
    pub fn innermost(&self) -> Option<&ScrollFrame> {
        self.0.last()
    }

    /// How many `scroll` containers enclose this point in the walk.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.len()
    }
}

/// Everything a negotiation pass carries.
pub struct LayoutCtx<'a> {
    /// Content measurement boundary.
    pub content: &'a mut dyn ContentMeasure,
    /// Row materialization boundary.
    pub rows: &'a mut dyn RowSource,
    /// Memoized responses.
    pub cache: &'a mut MeasureCache,
    /// The state snapshot this frame reads.
    pub state: &'a LayoutState,
    /// The theme snapshot in force for this frame.
    ///
    /// A container that reads a styling token resolves the name here, at the
    /// read site, rather than against some earlier resolution: the snapshot
    /// is immutable for the whole pass, so every read in one frame answers
    /// from the same theme. [`ThemeSnapshot`] is complete against its
    /// vocabulary by construction ([`crate::token::Theme::build`]), so a name
    /// an accepted tree may carry always resolves and no container needs a
    /// fallback branch.
    ///
    /// This is *not* the measure-cache key: [`LayoutCtx::key`] keys on
    /// `theme_rev`, which is the same fact as one comparable number.
    pub theme: &'a ThemeSnapshot,
    /// Theme snapshot revision in force.
    ///
    /// The revision of [`LayoutCtx::theme`] in a real host. It is a separate
    /// field because it is a separate job: a revision can key a cache and
    /// hash into a digest, and a snapshot can do neither.
    pub theme_rev: u64,
    /// Display scale.
    pub scale: Scale,
    /// The `scroll` ancestors of wherever the walk currently is. Start it
    /// empty; the walk maintains it.
    pub scroll: ScrollStack,
    /// The previous frame, when this pass is allowed to carry subtrees over
    /// from it. `None` is a full negotiation, which is what every caller
    /// outside [`crate::frame::petrify_with_memo`] wants.
    pub reuse: Option<reuse::ReuseState<'a>>,
    /// Where every node named by an [`Anchor::Node`] was placed, harvested
    /// before this pass's real walk.
    ///
    /// Start it empty ([`AnchorRects::new`]); [`place`] fills it at the root
    /// of the walk and clears it again for a tree that anchors nothing. It is
    /// a field on the context rather than a parameter threaded through the
    /// twelve container modules because it is exactly what this struct is
    /// for: everything one negotiation pass carries, available at the read
    /// site (`contracts/anchored-placement.md` §1 step 3).
    pub anchors: AnchorRects,
}

impl LayoutCtx<'_> {
    /// The cache key for `path` under `proposal`.
    ///
    /// The scroll context is deliberately not part of the key: a node's key
    /// path already determines its ancestry, so two entries under the same
    /// `path` cannot have been measured under two different scroll frames.
    /// Content is not part of the key either — a changed node is invalidated
    /// by [`MeasureCache::apply`] once per frame rather than compared here on
    /// every lookup ([`ChangeSet`]).
    #[must_use]
    pub fn key(&self, path: &KeyPath, proposal: SizeProposal) -> MeasureKey {
        MeasureKey {
            node: path.id(),
            proposal,
            theme_rev: self.theme_rev,
            scale: self.scale,
        }
    }

    /// The innermost enclosing `scroll`, or `None` outside every scroll.
    #[must_use]
    pub fn enclosing_scroll(&self) -> Option<&ScrollFrame> {
        self.scroll.innermost()
    }

    /// The gap a styling token reference names, in logical units.
    ///
    /// The read a container performs on `props.spacing`,
    /// `props.column_spacing`, or `props.row_spacing`, resolved against the
    /// snapshot this pass carries. Absence answers
    /// [`crate::tree::props::DEFAULT_SPACING`].
    ///
    /// Here rather than free-standing so that a container never has to reach
    /// for a theme itself: [`LayoutCtx::theme`] is the theme for this frame,
    /// and taking the resolution through the context is what makes that true
    /// at the read site instead of true by convention.
    #[must_use]
    pub fn spacing(&self, reference: &Option<TokenName>) -> f32 {
        crate::tree::props::resolve_spacing(self.theme, reference)
    }

    /// The four content insets a styling token reference names, in logical
    /// units. [`LayoutCtx::spacing`] on four edges; absence answers
    /// [`crate::geom::Insets::NONE`].
    #[must_use]
    pub fn padding(&self, reference: &Option<InsetRefs>) -> Insets {
        crate::tree::props::resolve_insets(self.theme, reference)
    }

    /// Run `f` with `frame` as the innermost enclosing scroll.
    ///
    /// The push and the pop bracket one call with nothing between them, so
    /// there is no early return, no `?`, and no conditional that can leave
    /// the stack unbalanced. (A panic escaping `f` unwinds out of the whole
    /// negotiation pass, which drops the context, so an unbalanced stack is
    /// not observable there either.)
    pub fn within_scroll<R>(&mut self, frame: ScrollFrame, f: impl FnOnce(&mut Self) -> R) -> R {
        self.scroll.0.push(frame);
        let answer = f(self);
        self.scroll.0.pop();
        answer
    }

    /// Run `f` with no enclosing scroll at all.
    ///
    /// A `surface` floats free of its ancestors' flow, and that includes
    /// their scrolling: it is anchored in viewport coordinates and is not
    /// moved by an ancestor `scroll`'s offset (`layout::overlay_surface`).
    /// A list inside a popup is therefore not virtualized against the panel
    /// the popup was declared in. Bracketed the same way as
    /// [`LayoutCtx::within_scroll`].
    pub fn outside_scroll<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = std::mem::take(&mut self.scroll);
        let answer = f(self);
        self.scroll = saved;
        answer
    }
}

/// Where one node an [`Anchor::Node`] names was placed.
///
/// Both halves of what the harvest walk records
/// (`contracts/anchored-placement.md` §1 step 2): the rect the anchor node
/// took, and the clip that was in force over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnchorRect {
    /// The anchor node's own placed rect.
    pub rect: Rect,
    /// The clip in force over it — the chain of every `scroll` and every
    /// clipped container between the anchor and the root.
    pub clip: Rect,
}

impl AnchorRect {
    /// The part of the anchor a surface is placed against: the rect, narrowed
    /// to the clip over it.
    ///
    /// A row scrolled half out of its viewport is anchored to the half that
    /// is on screen, not to the whole row: a tooltip that pointed at the
    /// hidden half would point past the edge of the list at nothing. A row
    /// scrolled entirely out collapses to a zero-extent rect on the clip's
    /// boundary — still a determinate place, and the surface still lands
    /// against the edge of the container the anchor went out of, which is the
    /// nearest honest answer available.
    #[must_use]
    pub fn visible(self) -> Rect {
        self.rect.intersect(self.clip)
    }
}

/// The anchor rects one pass resolves [`Anchor::Node`] against, plus the
/// pruning set in force while they are being harvested.
///
/// Ordered, and it has to be: `contracts/anchored-placement.md` §7 requires
/// the harvest-to-place hand-off be order-independent, and a hash-keyed map
/// here would make iteration order a nondeterminism bug that only some runs
/// would show. `BTreeMap` iterates in key order on every target, which is
/// the same reason `frame::digest` hashes the token map out of one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnchorRects {
    rects: BTreeMap<String, AnchorRect>,
    /// Every id the harvest walk may descend into: the anchor targets and
    /// their ancestors. `None` outside a harvest walk, which is what makes
    /// [`AnchorRects::pruned`] free on the real walk.
    harvest: Option<BTreeSet<String>>,
}

impl AnchorRects {
    /// Empty: nothing harvested, no harvest walk running.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Where the node with canonical id `id` was placed, if the harvest
    /// reached it.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<AnchorRect> {
        self.rects.get(id).copied()
    }

    /// How many anchors were harvested. Zero for every tree that declares no
    /// [`Anchor::Node`] at all.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rects.len()
    }

    /// Whether nothing was harvested.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    /// Whether the walk should stop here: true only inside a harvest walk,
    /// and only for a subtree holding no anchor target
    /// (`contracts/anchored-placement.md` §1 step 2, "MUST prune into a
    /// subtree only when it contains a `T` member").
    ///
    /// The real walk answers `false` without building an id at all, which is
    /// what "a tree with no anchors pays nothing" means in practice.
    fn pruned(&self, path: &KeyPath) -> bool {
        match &self.harvest {
            None => false,
            Some(keep) => !keep.contains(&path.id()),
        }
    }
}

/// The rect, paint order, clip, and opacity a parent gives one child.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Where the child goes, logical units.
    pub rect: Rect,
    /// Paint order.
    pub z: i32,
    /// The clip in force.
    pub clip: Rect,
    /// Cumulative opacity in `[0, 1]`.
    pub opacity: f32,
}

impl Slot {
    /// A slot filling `rect` with no clipping beyond it.
    #[must_use]
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            z: 0,
            clip: rect,
            opacity: 1.0,
        }
    }

    /// This slot moved to `rect`, keeping z, clip, and opacity.
    #[must_use]
    pub fn with_rect(self, rect: Rect) -> Self {
        Self { rect, ..self }
    }

    /// This slot with the clip narrowed to the intersection with `clip`.
    #[must_use]
    pub fn clipped_to(self, clip: Rect) -> Self {
        Self {
            clip: self.clip.intersect(clip),
            ..self
        }
    }

    /// This slot with `z` added to the paint order.
    #[must_use]
    pub fn above(self, z: i32) -> Self {
        Self {
            z: self.z.saturating_add(z),
            ..self
        }
    }

    /// This slot with opacity multiplied by `factor`.
    #[must_use]
    pub fn faded(self, factor: f32) -> Self {
        Self {
            opacity: (self.opacity * factor).clamp(0.0, 1.0),
            ..self
        }
    }
}

/// Measure `node` under `proposal`.
///
/// Applies the node's own constraints to whatever its kind answers, and
/// memoizes the result. `path` is the walk's key path: it is pushed and popped
/// here, so a caller passes the *parent's* path.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    path.push(node.key.clone());
    let key = ctx.key(path, proposal);
    let answer = if let Some(hit) = ctx.cache.peek(&key) {
        hit
    } else {
        let raw = measure_kind(node, ctx, path, proposal);
        let clamped = node.constraints.clamp_size(raw).sane();
        ctx.cache.insert(key, clamped);
        clamped
    };
    path.pop();
    answer
}

/// Place `node` into `slot`, emitting one placement for it and for every node
/// under it. `path` is pushed and popped here, as in [`measure`].
///
/// At the root of a walk — `path` empty, which is what
/// [`crate::frame::petrify`] hands in and what nothing below the root ever
/// can, since this function pushes and pops exactly one key per node — the
/// anchor rects are harvested first ([`harvest_anchor_rects`]). Every
/// recursive call arrives with a non-empty path, so the harvest runs once per
/// frame and the containers below know nothing about it.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    if path.is_empty() {
        harvest_anchor_rects(node, ctx, slot);
    }
    place_node(node, ctx, path, slot, sink);
}

/// Every id an [`Anchor::Node`] in this tree names, and how deeply anchored
/// surfaces nest around those ids.
///
/// The depth is the count of *anchored* surfaces enclosing an anchor target,
/// maximised over the targets: a target inside an `Anchor::Viewport` surface
/// adds nothing, because such a surface is placed without reference to any
/// other node and so is already correct in the first harvest walk. This is
/// the `d` of `contracts/anchored-placement.md` §1 step 4, and it is computed
/// from the tree, never from a previous frame.
///
/// Related to `tree::validate`'s own anchor walk but not the same question:
/// that one asks whether the anchors can be resolved at all, and its refusals
/// are what let this one assume every target exists and no chain loops.
fn anchor_targets(root: &ViewNode) -> (BTreeSet<String>, usize) {
    fn walk(
        node: &ViewNode,
        path: &mut KeyPath,
        surfaces_above: usize,
        targets: &mut BTreeSet<String>,
        enclosure: &mut BTreeMap<String, usize>,
    ) {
        path.push(node.key.clone());
        let anchored = match &node.props.anchor {
            Some(Anchor::Node { id, .. }) => {
                targets.insert(id.clone());
                true
            }
            _ => false,
        };
        // A surface counts its *own* anchor, not only its ancestors': a
        // surface anchored directly to another anchored surface's rect needs
        // that surface placed first, exactly as one anchored to a node inside
        // it does. Counting only ancestors here left that case on one walk,
        // resolving against a rect the first walk had centred in the viewport.
        let own = surfaces_above + usize::from(anchored);
        if own > 0 {
            enclosure.insert(path.id(), own);
        }
        for child in &node.children {
            walk(child, path, own, targets, enclosure);
        }
        path.pop();
    }

    let mut targets = BTreeSet::new();
    let mut enclosure = BTreeMap::new();
    walk(root, &mut KeyPath::root(), 0, &mut targets, &mut enclosure);
    let depth = targets
        .iter()
        .filter_map(|id| enclosure.get(id).copied())
        .max()
        .unwrap_or(0);
    (targets, depth)
}

/// Place the anchor set into `ctx.anchors`, so the real walk below can
/// resolve every [`Anchor::Node`] against a rect that exists.
///
/// This is `contracts/anchored-placement.md` §1 end to end. `1 + d` pruned
/// walks run, each seeded by the one before it, where `d` is the surface-
/// anchor nesting depth [`anchor_targets`] reads off the tree. A tree that
/// names no anchor runs none of them and the pass costs one walk of the tree
/// to find that out.
///
/// Two things it deliberately does not do. It does not read a placement back
/// out of the sink mid-walk — the harvest is a whole extra walk precisely so
/// that every anchor is answered the same way, rather than only those that
/// happen to be placed before the surface asking. And it does not consult the
/// memo: the reuse cursor is set aside for the duration, because a harvested
/// rect that came from the previous frame would be exactly the stale answer
/// §6 exists to prevent. The measure cache *is* shared, which is what keeps
/// this from being a second measurement pass — the harvest warms it and the
/// real walk reads it back.
///
/// One cost this does pay, named rather than hidden: [`RowSource::rows`] is
/// not cached, so a `collection` on the path to an anchor is materialized
/// once per harvest walk as well as once for the real walk. That is safe by
/// that trait's own contract — "a `collection` re-fetches its rows every
/// frame" — and it is bounded by the pruning, which descends into a
/// collection only when the anchor is inside it. A host whose row source is
/// expensive and whose anchors live inside lists pays for it, and the fix
/// there is the caching the trait already invites, not a second cache here.
fn harvest_anchor_rects(root: &ViewNode, ctx: &mut LayoutCtx<'_>, slot: Slot) {
    ctx.anchors = AnchorRects::new();
    let (targets, depth) = anchor_targets(root);
    if targets.is_empty() {
        return;
    }

    // Every id the walk is allowed to descend into: the targets, and the
    // ancestors it has to pass through to reach them.
    let mut keep: BTreeSet<String> = BTreeSet::new();
    for id in &targets {
        keep.extend(KeyPath::ancestor_ids(id));
        keep.insert(id.clone());
    }
    let mut keep = Some(keep);

    let held = ctx.reuse.take();
    for _ in 0..=depth {
        ctx.anchors.harvest = keep.take();
        let mut sink = PlacementList::new();
        place_node(root, ctx, &mut KeyPath::root(), slot, &mut sink);
        keep = ctx.anchors.harvest.take();
        ctx.anchors.rects = sink
            .as_slice()
            .iter()
            .filter(|p| targets.contains(&p.id))
            .map(|p| {
                (
                    p.id.clone(),
                    AnchorRect {
                        rect: p.rect,
                        clip: p.clip,
                    },
                )
            })
            .collect();
    }
    ctx.reuse = held;
}

/// [`place`] below the root: no harvest, and the harvest walk's own pruning.
fn place_node(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    path.push(node.key.clone());
    if ctx.anchors.pruned(path) {
        path.pop();
        return;
    }
    let slot = match node.props.opacity {
        Some(o) => slot.faded(o),
        None => slot,
    };
    let slot = match node.props.z {
        Some(z) => slot.above(z),
        None => slot,
    };

    // Can this whole subtree be carried over from the previous frame? The
    // tests run cheapest-first, and the dirty scan runs last because it is
    // the only one that has to build this node's id, which allocates. A pass
    // with no memo skips all of it.
    let counterpart = ctx
        .reuse
        .as_mut()
        .and_then(|state| state.counterpart(&node.key));
    if let (Some(state), Some((old_node, old_index))) = (ctx.reuse.as_ref(), counterpart)
        && state.reusable(node, old_node, old_index, slot)
        && !state.dirty(&path.id())
    {
        let sub = state.subtree(old_index);
        let len = sub.placements.len();
        sink.reuse_subtree(sub);
        if let Some(state) = ctx.reuse.as_mut() {
            state.note_reuse(len);
        }
        path.pop();
        return;
    }
    // Every container pushes its own placement before any child's, so the
    // index the dispatcher noted before the call is this node's. Attaching
    // here rather than inside each container means no container has to
    // remember to carry paint content, and the twelve kinds cannot drift
    // apart on what "carrying" means.
    let index = sink.len();
    if let Some(state) = ctx.reuse.as_mut() {
        state.note_replaced();
        state.enter(counterpart);
    }
    let caret = place_kind(node, ctx, path, slot, sink);
    if let Some(state) = ctx.reuse.as_mut() {
        state.leave();
    }
    if sink.len() > index {
        debug_assert_eq!(
            sink.placed()[index].id,
            path.id(),
            "a container must push its own placement before its children's"
        );
        let mut content = paint_content_of(node);
        // The resolved side of an anchored surface is not knowable to an
        // author and not derivable from the tree — only the ladder that just
        // ran knows which way the box flipped — so this is the one payload
        // member the placement pass hands the dispatcher rather than the
        // other way round (`contracts/anchored-placement.md` §5).
        content.caret = caret;
        if !content.is_empty() {
            sink.attach(index, content);
        }
        // The offer, not the answer. Noted after the subtree is complete so
        // that a container cannot forget it, and noted here rather than in
        // `place_kind` so the twelve kinds cannot disagree about whether
        // "the slot" means before or after this node's own opacity and z.
        sink.note_slot(index, slot);
    }
    path.pop();
}

/// What this node draws, beyond its rect.
///
/// Derived from the tree and from nothing else, because the placement
/// deliberately holds only what the digest hashes
/// (`contracts/frame-identity.md`).
///
/// Interaction state does **not** enter here. A node that binds `background`
/// and `background@hover` hands both to the painter, and
/// `crate::token::resolve_slot` chooses between them at paint time from the
/// placement's own flags (`contracts/interaction-state.md` §4). Resolving
/// early would make this content differ by hover — an interaction state
/// leaking into the placement stream — and Carbon names combinations rather
/// than layering them, so `background@selected-hover` has to stay reachable
/// as its own key rather than being folded away.
#[must_use]
pub fn paint_content_of(node: &ViewNode) -> PaintContent {
    let props = &node.props;
    let text = match node.kind {
        NodeKind::Text => Some(props.text.clone().unwrap_or_default()),
        // An empty field draws its placeholder, which is why the placeholder
        // is what gets painted rather than the empty string.
        NodeKind::Input => Some(match props.text.as_deref() {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => props.placeholder.clone().unwrap_or_default(),
        }),
        _ => None,
    };
    PaintContent {
        text: text.map(|text| TextPaint {
            text,
            style: props.style.as_ref().map(|t| t.as_str().to_owned()),
            wrap: props.wrap.unwrap_or_default(),
            max_lines: props.max_lines,
        }),
        image: match node.kind {
            NodeKind::Image => props.image.clone(),
            _ => None,
        },
        custom: match node.kind {
            NodeKind::Custom => props.custom_kind.clone(),
            _ => None,
        },
        canvas: match node.kind {
            // `Arc::clone`, not a deep copy: this runs once per canvas per
            // frame, and the whole point of the `Arc` is that a picture the
            // author did not change costs a refcount bump.
            NodeKind::Canvas => props.canvas.clone(),
            _ => None,
        },
        // Verbatim, state-decorated keys and all. `crate::token::resolve_slot`
        // picks the winning key at paint time from the placement's own flags;
        // collapsing here would put an interaction state into the placement
        // stream and throw away the bindings an agent needs to read.
        tokens: props
            .tokens
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().to_owned()))
            .collect(),
        // Written by the placement pass through the dispatcher, never
        // authored: see `crate::layout::place`.
        caret: None,
    }
}

fn measure_kind(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    match node.kind {
        NodeKind::Stack => stack::measure(node, ctx, path, proposal),
        NodeKind::Grid => grid::measure(node, ctx, path, proposal),
        NodeKind::Overlay => overlay::measure(node, ctx, path, proposal),
        NodeKind::Scroll => scroll::measure(node, ctx, path, proposal),
        NodeKind::Collection => scroll::measure_collection(node, ctx, path, proposal),
        NodeKind::Surface => overlay_surface::measure(node, ctx, path, proposal),
        NodeKind::Text => text::measure(node, ctx, proposal),
        NodeKind::Image
        | NodeKind::Input
        | NodeKind::Spacer
        | NodeKind::Separator
        | NodeKind::Custom
        // A canvas is a leaf that never reaches `ctx.content`: it has no
        // intrinsic size to ask for (`contracts/draw-list.md` §7).
        | NodeKind::Canvas => leaf::measure(node, ctx, proposal),
    }
}

/// Dispatch one node's placement, and carry back the one payload only the
/// placement pass can know: an anchored surface's caret.
///
/// Eleven of the twelve kinds return `None` because there is nothing about
/// their placement an author could not have written down; a `surface`
/// anchored to a node returns the caret the ladder resolved for it.
fn place_kind(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) -> Option<CaretPaint> {
    match node.kind {
        NodeKind::Stack => stack::place(node, ctx, path, slot, sink),
        NodeKind::Grid => grid::place(node, ctx, path, slot, sink),
        NodeKind::Overlay => overlay::place(node, ctx, path, slot, sink),
        NodeKind::Scroll => scroll::place(node, ctx, path, slot, sink),
        NodeKind::Collection => scroll::place_collection(node, ctx, path, slot, sink),
        NodeKind::Surface => return overlay_surface::place(node, ctx, path, slot, sink),
        NodeKind::Text => text::place(node, ctx, path, slot, sink),
        NodeKind::Image
        | NodeKind::Input
        | NodeKind::Spacer
        | NodeKind::Separator
        | NodeKind::Custom
        | NodeKind::Canvas => leaf::place(node, ctx, path, slot, sink),
    }
    None
}

/// The semantic payload for one node, built the same way by every container.
///
/// `id` is the node's canonical key path — the same string its
/// [`Placement`](crate::frame::Placement) carries — and `state` is the
/// snapshot this frame is placed from. The two are here for one flag:
/// [`PlacementSemantics::focused`] is the projection of
/// [`LayoutState::focused`] onto the node it names, and this is the single
/// place it is computed so that the twelve node kinds cannot each get it
/// differently. Every container already has both values in hand at the point
/// it builds its placement.
#[must_use]
pub fn semantics_of(node: &ViewNode, id: &str, state: &LayoutState) -> PlacementSemantics {
    PlacementSemantics {
        role: node
            .semantics
            .role
            .clone()
            .or_else(|| default_role(node.kind)),
        label: node.semantics.label.clone(),
        value: node.semantics.value.clone(),
        focused: state.focused.as_deref() == Some(id),
        // Engine-derived, from the pointer snapshot this frame is placed
        // from, exactly the way `focused` reads `state.focused` one line up.
        //
        // The `!disabled` guard is rank 2 of
        // `contracts/interaction-state.md` §4, which says disabled *clears*
        // hover, active and capture rather than merely outranking them.
        // `hit_test` already refuses a disabled placement, so the host cannot
        // put a disabled node in any of these three states in the first
        // place — but the snapshot is a frame old (it was taken against the
        // frame on screen, the way focus is), and a node the application
        // disabled since then would otherwise paint one frame lit before the
        // reconciliation caught it.
        hovered: !node.semantics.disabled && state.hovered.as_deref() == Some(id),
        active: !node.semantics.disabled && state.pressed.as_deref() == Some(id),
        captured: !node.semantics.disabled
            && state
                .capture
                .as_ref()
                .is_some_and(|capture| capture.node == id),
        // App-declared, so these two project straight off the node the way
        // `disabled` does, and are complete as of this change.
        read_only: node.semantics.read_only,
        skeleton: node.semantics.skeleton,
        disabled: node.semantics.disabled,
        selected: node.semantics.selected,
        expanded: node.semantics.expanded,
        stale: node.semantics.stale,
        ambient: node.ambient,
        actions: node.interactions.clone(),
        total_count: node.props.total_count,
    }
}

/// The role a built-in kind carries when the author declares none.
#[must_use]
pub fn default_role(kind: NodeKind) -> Option<Role> {
    Some(match kind {
        NodeKind::Stack | NodeKind::Grid | NodeKind::Overlay => Role::Pane,
        NodeKind::Scroll => Role::Scroll,
        NodeKind::Collection => Role::List,
        NodeKind::Surface => Role::Overlay,
        NodeKind::Text => Role::Label,
        NodeKind::Image => Role::Image,
        NodeKind::Input => Role::TextInput,
        NodeKind::Separator => Role::Separator,
        // A spacer is empty space, a custom node is whatever its host says it
        // is, and a canvas is whatever its author drew. Inventing a role for
        // any of the three would put a node in the accessibility tree that
        // describes nothing — and a canvas is the one most tempting to call an
        // `image`, which would promise a screen reader an alt text the payload
        // does not carry.
        NodeKind::Spacer | NodeKind::Custom | NodeKind::Canvas => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{LayoutCtx, LayoutState, SizeProposal, Slot, default_role, semantics_of};
    use crate::geom::Rect;
    use crate::token::{TokenName, TokenValue};
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

    #[test]
    fn slots_compose_clip_z_and_opacity() {
        let slot = Slot::new(Rect::new(0.0, 0.0, 100.0, 100.0))
            .clipped_to(Rect::new(10.0, 10.0, 50.0, 50.0))
            .above(5)
            .faded(0.5)
            .faded(0.5);
        assert_eq!(slot.clip, Rect::new(10.0, 10.0, 50.0, 50.0));
        assert_eq!(slot.z, 5);
        assert_eq!(slot.opacity, 0.25);
    }

    #[test]
    fn clipping_only_ever_narrows() {
        let slot = Slot::new(Rect::new(0.0, 0.0, 10.0, 10.0))
            .clipped_to(Rect::new(0.0, 0.0, 1000.0, 1000.0));
        assert_eq!(slot.clip, Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn declared_semantics_win_over_the_kind_default() {
        let node = ViewNode::new(NodeKind::Text, "t").interactive(
            Role::Button,
            "Reload",
            &[Interaction::Click],
        );
        let sem = semantics_of(&node, "/t", &LayoutState::default());
        assert_eq!(sem.role, Some(Role::Button));
        assert_eq!(sem.label.as_deref(), Some("Reload"));
        assert_eq!(sem.actions, vec![Interaction::Click]);
    }

    /// The focus flag is a projection of one id, not of "something is
    /// focused": a frame with a focused node must not mark every placement.
    #[test]
    fn only_the_node_the_state_names_is_focused() {
        let node = ViewNode::new(NodeKind::Text, "t");
        let state = LayoutState {
            focused: Some("/panel/t".into()),
            ..LayoutState::default()
        };
        assert!(semantics_of(&node, "/panel/t", &state).focused);
        assert!(!semantics_of(&node, "/panel/other", &state).focused);
        assert!(!semantics_of(&node, "/panel/t", &LayoutState::default()).focused);
        // A prefix of the focused path is a different node.
        assert!(!semantics_of(&node, "/panel", &state).focused);
    }

    #[test]
    fn empty_and_host_defined_kinds_get_no_invented_role() {
        assert_eq!(default_role(NodeKind::Spacer), None);
        assert_eq!(default_role(NodeKind::Custom), None);
        assert_eq!(default_role(NodeKind::Scroll), Some(Role::Scroll));
    }

    /// The dispatcher attaches paint content; containers never have to.
    #[test]
    fn the_dispatcher_pairs_every_placement_with_what_it_draws() {
        use crate::frame::{PlacementList, TransitionActivity, Viewport, petrify};
        use crate::geom::Size;
        use crate::testing::{Harness, validated};
        use crate::token::ThemeMode;
        use crate::tree::Props;

        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                text: Some("Fibers".into()),
                style: Some(TokenName::new("typography.heading").unwrap()),
                ..Props::default()
            }))
            .child(ViewNode::new(NodeKind::Spacer, "gap"));

        let mut h = Harness::new();
        let frame = petrify(
            1,
            validated(&tree),
            &mut h.ctx(),
            Viewport::new(Size::new(200.0, 100.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );

        assert_eq!(frame.placements.len(), frame.content.len());
        let drawn: Vec<(&str, Option<&str>)> = frame
            .drawn()
            .map(|(p, c)| (p.id.as_str(), c.text.as_ref().map(|t| t.text.as_str())))
            .collect();
        assert_eq!(
            drawn,
            [
                ("/root", None),
                ("/root/title", Some("Fibers")),
                ("/root/gap", None),
            ]
        );
        let title = frame.content[1].text.as_ref().unwrap();
        assert_eq!(title.style.as_deref(), Some("typography.heading"));
        assert!(
            frame.content[0].is_empty(),
            "a bare stack draws nothing of its own"
        );

        // The sink is what pairs them, so a hand-driven walk agrees.
        let mut sink = PlacementList::new();
        assert_eq!(sink.len(), 0);
        assert!(sink.is_empty());
        let mut path = crate::tree::KeyPath::root();
        super::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        assert_eq!(sink.content().len(), sink.as_slice().len());
    }

    /// An empty field paints its placeholder; a filled one paints its content.
    #[test]
    fn an_empty_field_draws_its_placeholder() {
        use crate::tree::Props;
        let empty = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some("Filter…".into()),
            ..Props::default()
        });
        assert_eq!(
            super::paint_content_of(&empty).text.unwrap().text,
            "Filter…"
        );
        let filled = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            text: Some("fiber".into()),
            placeholder: Some("Filter…".into()),
            ..Props::default()
        });
        assert_eq!(super::paint_content_of(&filled).text.unwrap().text, "fiber");
    }

    /// Token references reach the renderer through the payload, not through
    /// the placement — but they still reach the digest, hashed into
    /// `PaintState::paint_hash` when the payload is attached.
    ///
    /// This comment used to end "the digest hashes the theme revision, not the
    /// names", and stated a defect as though it were a design: the theme
    /// revision is one global number, so rebinding this node's `background`
    /// from `surface.raised` to `status.down` repainted the panel and moved no
    /// digest. `gorgon/petra/tests/frame_digest_coverage.rs` is where the two
    /// bindings are now proven to differ.
    #[test]
    fn token_references_reach_the_payload() {
        use crate::tree::Props;
        let mut props = Props::default();
        props.tokens.insert(
            "background".into(),
            TokenName::new("surface.raised").unwrap(),
        );
        let node = ViewNode::new(NodeKind::Stack, "panel").with_props(props);
        let content = super::paint_content_of(&node);
        assert_eq!(
            content.tokens.get("background").map(String::as_str),
            Some("surface.raised")
        );
        assert!(!content.is_empty());
        assert_ne!(
            crate::frame::digest::hash_paint_content(&content),
            0,
            "a node that binds a token draws something, so its payload hash \
             must not be the zero a bare container carries"
        );
    }

    /// The scroll context is a stack: the innermost frame answers, the
    /// bracket restores what it found, and a `surface`-style reset restores
    /// too.
    #[test]
    fn the_scroll_context_nests_and_unwinds() {
        use super::{ScrollFrame, ScrollStack};
        use crate::geom::Axis;
        use crate::testing::Harness;
        use crate::tree::props::{DEFAULT_OVERSCAN, ScrollProps};

        let frame = |id: &str, overscan: f32| ScrollFrame {
            id: id.to_owned(),
            props: ScrollProps {
                axis: Axis::Vertical,
                overscan,
            },
        };

        let mut h = Harness::new();
        let mut ctx = h.ctx();
        assert_eq!(ctx.scroll, ScrollStack::new());
        assert!(
            ctx.enclosing_scroll().is_none(),
            "the walk starts outside every scroll"
        );

        ctx.within_scroll(frame("/outer", 200.0), |ctx| {
            assert_eq!(
                ctx.enclosing_scroll().map(|f| f.id.as_str()),
                Some("/outer")
            );
            ctx.within_scroll(frame("/outer/inner", DEFAULT_OVERSCAN), |ctx| {
                let inner = ctx.enclosing_scroll().expect("inside two scrolls");
                assert_eq!(inner.id, "/outer/inner", "the innermost frame answers");
                assert_eq!(inner.props.overscan, DEFAULT_OVERSCAN);
                assert_eq!(ctx.scroll.depth(), 2);

                ctx.outside_scroll(|ctx| {
                    assert!(
                        ctx.enclosing_scroll().is_none(),
                        "a surface clears the chain"
                    );
                    assert_eq!(ctx.scroll.depth(), 0);
                });
                assert_eq!(ctx.scroll.depth(), 2, "the chain is restored");
            });
            assert_eq!(
                ctx.enclosing_scroll().map(|f| f.id.as_str()),
                Some("/outer")
            );
        });
        assert_eq!(ctx.scroll.depth(), 0, "every push is popped");
    }

    /// The walk leaves the stack the way it found it, whatever the tree.
    #[test]
    fn placing_a_tree_of_scrolls_leaves_the_context_empty() {
        use crate::frame::PlacementList;
        use crate::testing::Harness;
        use crate::tree::Props;

        let tree = ViewNode::new(NodeKind::Scroll, "outer").child(
            ViewNode::new(NodeKind::Stack, "body")
                .child(ViewNode::new(NodeKind::Scroll, "inner").child(
                    ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                        total_count: Some(4),
                        source: Some("fibers".into()),
                        ..Props::default()
                    }),
                ))
                .child(ViewNode::new(NodeKind::Text, "footer")),
        );

        let mut h = Harness::new();
        let mut ctx = h.ctx();
        let mut path = crate::tree::KeyPath::root();
        let mut sink = PlacementList::new();
        super::place(
            &tree,
            &mut ctx,
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        assert_eq!(ctx.scroll.depth(), 0);
        assert!(ctx.enclosing_scroll().is_none());
    }

    /// The read every container acquires once `Props`'s styling fields
    /// become token references: a name, resolved against the snapshot the
    /// context carries, at the read site.
    ///
    /// Deliberately shaped like `stack::measure`'s own gap arithmetic —
    /// resolve, then sum the children's measured extents — so the resolved
    /// number has to reach a `Size` for the assertions below to hold. A
    /// lookup whose answer went nowhere would prove nothing.
    fn measure_row_with_token_gap(
        node: &ViewNode,
        ctx: &mut LayoutCtx<'_>,
        token: &TokenName,
    ) -> crate::geom::Size {
        // No fallback arm: `Theme::build` proves every theme complete against
        // its vocabulary, so a declared name resolves at its declared kind or
        // the theme never existed.
        let gap = match ctx.theme.value(token) {
            Some(TokenValue::Spacing(units)) => *units,
            other => panic!("{token} must resolve to a spacing value, found {other:?}"),
        };
        let mut path = crate::tree::KeyPath::root();
        let mut total = gap * node.children.len().saturating_sub(1) as f32;
        let mut tallest = 0.0f32;
        for child in &node.children {
            let got = super::measure(child, ctx, &mut path, SizeProposal::unbounded());
            total += got.w;
            tallest = tallest.max(got.h);
        }
        crate::geom::Size::new(total, tallest)
    }

    /// `light()` with one spacing token reassigned, built through
    /// `Theme::build` like any other theme so the switch under test is a
    /// complete theme, not a patched map.
    fn theme_with_spacing_sm(units: f32) -> crate::token::Theme {
        let mut values = crate::token::light().values().clone();
        values.insert(
            TokenName::new("spacing.sm").expect("well-formed name"),
            TokenValue::Spacing(units),
        );
        crate::token::Theme::build(
            crate::token::ThemeMode::Light,
            &crate::token::standard_vocabulary(),
            values,
        )
        .expect("light()'s own assignments with one spacing value replaced are complete")
    }

    /// `LayoutCtx::theme_rev` can invalidate a cache and nothing else: it
    /// cannot answer "how wide is `spacing.md`". This is the test that the
    /// context reaches a snapshot that can, and that the snapshot it reaches
    /// is the live one — a field wired to some fixed theme would pass the
    /// first assertion and fail the second.
    #[test]
    fn a_container_reads_a_token_during_measure_and_the_value_tracks_a_theme_switch() {
        use crate::testing::Harness;
        use crate::tree::Props;

        let text = |key: &str| {
            ViewNode::new(NodeKind::Text, key).with_props(Props {
                text: Some("ab".to_owned()),
                ..Props::default()
            })
        };
        let row = ViewNode::new(NodeKind::Stack, "row")
            .child(text("a"))
            .child(text("b"))
            .child(text("c"));
        // `spacing.sm`, not `spacing.md`: the eight-step ramp renamed the
        // step that means 8 logical units, and this fixture is about the 8,
        // not about the label.
        let sm = TokenName::new("spacing.sm").expect("well-formed name");

        // `Harness::new` starts on the shipped light theme (plus the fixture
        // gap scale), where `spacing.sm` is 8 logical units.
        let mut h = Harness::new();
        assert_eq!(h.theme.value(&sm), Some(&TokenValue::Spacing(8.0)));
        let narrow = {
            let mut ctx = h.ctx();
            measure_row_with_token_gap(&row, &mut ctx, &sm)
        };

        h.set_theme(crate::token::ThemeSnapshot::new(
            theme_with_spacing_sm(24.0),
            2,
        ));
        let wide = {
            let mut ctx = h.ctx();
            measure_row_with_token_gap(&row, &mut ctx, &sm)
        };

        // Two gaps between three children, each 16 units wider than before.
        assert_eq!(wide.w, narrow.w + 2.0 * (24.0 - 8.0));
        assert_eq!(wide.h, narrow.h, "only the gap moved");
        // The revision the cache keys on moved with the snapshot the
        // containers read, which is what makes the switch invalidate the
        // measurements it changed.
        assert_eq!(h.theme_rev, 2);
        assert_eq!(h.ctx().theme.revision(), 2);
    }

    /// The two helpers a container reads a styling prop through
    /// ([`LayoutCtx::spacing`], [`LayoutCtx::padding`]), against the four
    /// facts their contract states: a named gap resolves, an absent one is
    /// the documented default, a partly named inset insets only the edges it
    /// names, and both answers move with the theme in force.
    #[test]
    fn the_context_helpers_resolve_a_reference_and_read_absence_as_the_default() {
        use crate::testing::Harness;
        use crate::tree::InsetRefs;
        use crate::tree::props::DEFAULT_SPACING;

        let sm = TokenName::new("spacing.sm").expect("well-formed name");
        let lg = TokenName::new("spacing.lg").expect("well-formed name");
        let mut h = Harness::new();

        {
            let ctx = h.ctx();
            assert_eq!(ctx.spacing(&Some(sm.clone())), 8.0);
            assert_eq!(ctx.spacing(&None), DEFAULT_SPACING);
            assert_eq!(
                ctx.padding(&Some(InsetRefs::symmetric(sm.clone(), lg.clone()))),
                crate::geom::Insets::symmetric(8.0, 16.0)
            );
            assert_eq!(
                ctx.padding(&Some(InsetRefs {
                    left: Some(lg.clone()),
                    ..InsetRefs::default()
                })),
                crate::geom::Insets {
                    left: 16.0,
                    ..crate::geom::Insets::NONE
                },
                "an unnamed edge is no inset"
            );
            assert_eq!(ctx.padding(&None), crate::geom::Insets::NONE);
        }

        // The same two declarations under a theme that moved one of them.
        h.set_theme(crate::token::ThemeSnapshot::new(
            theme_with_spacing_sm(40.0),
            2,
        ));
        let ctx = h.ctx();
        assert_eq!(ctx.spacing(&Some(sm.clone())), 40.0);
        assert_eq!(
            ctx.padding(&Some(InsetRefs::symmetric(sm, lg))),
            crate::geom::Insets::symmetric(40.0, 16.0),
            "only the reassigned name moved"
        );
    }

    #[test]
    fn a_missing_or_bad_scroll_offset_reads_as_zero() {
        let mut state = LayoutState::default();
        state.scroll_offsets.insert("/a".into(), -5.0);
        state.scroll_offsets.insert("/b".into(), f32::NAN);
        state.scroll_offsets.insert("/c".into(), 12.0);
        assert_eq!(state.scroll_offset("/a"), 0.0);
        assert_eq!(state.scroll_offset("/b"), 0.0);
        assert_eq!(state.scroll_offset("/c"), 12.0);
        assert_eq!(state.scroll_offset("/missing"), 0.0);
    }

    // -- The harvest walk: what it costs, and what it collects. ------------

    /// A tree that names no anchor runs no harvest walk, and one that names
    /// an anchor harvests exactly the anchors it names — not every node it
    /// passed through on the way (`contracts/anchored-placement.md` §1 steps
    /// 1 and 2).
    ///
    /// Read off the context after the walk rather than inferred from the
    /// placements, because "the anchor map is empty" and "the surface landed
    /// somewhere plausible" are two different claims and only the first one
    /// is about cost.
    #[test]
    fn the_harvest_collects_the_anchor_set_and_nothing_else() {
        use crate::frame::PlacementList;
        use crate::geom::Rect;
        use crate::testing::Harness;
        use crate::tree::{Align, Anchor, Edge, KeyPath, Layer, Props};

        let plain = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "a"))
            .child(ViewNode::new(NodeKind::Text, "b"));
        let anchored = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "a"))
            .child(ViewNode::new(NodeKind::Text, "b"))
            .child(
                ViewNode::new(NodeKind::Surface, "popup")
                    .with_props(Props {
                        layer: Some(Layer::Popup),
                        anchor: Some(Anchor::Node {
                            id: "/root/b".into(),
                            edge: Edge::Bottom,
                            align: Align::Center,
                            offset: None,
                        }),
                        ..Props::default()
                    })
                    .child(ViewNode::new(NodeKind::Text, "body")),
            );

        let harvested = |tree: &ViewNode| {
            let mut h = Harness::new();
            let mut ctx = h.ctx();
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            super::place(
                tree,
                &mut ctx,
                &mut path,
                Slot::new(Rect::new(0.0, 0.0, 400.0, 300.0)),
                &mut sink,
            );
            ctx.anchors.clone()
        };

        let none = harvested(&plain);
        assert!(
            none.is_empty(),
            "a tree with no Anchor::Node harvests nothing at all"
        );

        let some = harvested(&anchored);
        assert_eq!(some.len(), 1, "one anchor named, one rect harvested");
        assert!(
            some.get("/root/b").is_some(),
            "and it is the one the anchor names"
        );
        assert!(
            some.get("/root/a").is_none(),
            "a sibling the walk had to measure past is not an anchor"
        );
        assert!(
            some.get("/root").is_none(),
            "nor is an ancestor it had to descend through"
        );
    }

    // -- Per-state token bindings, collapsed once, by the dispatcher. -------

    /// A state-decorated binding reaches the painter as its own key, and the
    /// resolver picks between the candidates from the placement's own flags.
    ///
    /// Driven through a real frame rather than by calling
    /// [`super::paint_content_of`] directly, because the claim under test
    /// spans two passes: petrify must carry every candidate through, and
    /// `resolve_slot` must choose among them from the state petrify resolved.
    /// A test that supplied the flags itself would pass with the projection
    /// wired to nothing.
    ///
    /// The two assertions that matter are the last two. Carbon *names* the
    /// combination — `$layer-selected-hover` is its own token, not hover
    /// composited over selected — so `background@selected-disabled` must win
    /// over both `background@selected` and `background@disabled`, and a
    /// mechanism that layered states could not express it at all.
    #[test]
    fn a_state_decorated_binding_is_resolved_from_the_placements_own_flags() {
        use crate::frame::{TransitionActivity, Viewport, petrify};
        use crate::geom::Size;
        use crate::testing::{Harness, validated};
        use crate::token::{DerivedState, ThemeMode, TokenName, resolve_slot};
        use crate::tree::Props;

        let name = |n: &str| TokenName::new(n).unwrap();

        let build = |disabled: bool, selected: bool| {
            let mut node = ViewNode::new(NodeKind::Text, "t").with_props(Props {
                text: Some("hi".into()),
                tokens: [
                    ("background".to_owned(), name("surface.base")),
                    ("background@disabled".to_owned(), name("surface.layer-one")),
                    ("background@selected".to_owned(), name("surface.layer-two")),
                    (
                        "background@selected-disabled".to_owned(),
                        name("surface.raised"),
                    ),
                ]
                .into_iter()
                .collect(),
                ..Props::default()
            });
            node.semantics.disabled = disabled;
            node.semantics.selected = selected;
            ViewNode::new(NodeKind::Stack, "root").child(node)
        };

        let painted = |tree: &ViewNode| {
            let mut h = Harness::new();
            let frame = petrify(
                1,
                validated(tree),
                &mut h.ctx(),
                Viewport::new(Size::new(200.0, 100.0), ThemeMode::Dark),
                TransitionActivity::default(),
            );
            let at = frame
                .placements
                .iter()
                .position(|p| p.id == "/root/t")
                .expect("the node is placed");
            let semantics = &frame.placements[at].semantics;
            // Every candidate must survive petrify: resolving early would put
            // an interaction state into the placement stream.
            assert!(
                frame.content[at].tokens.len() >= 4,
                "petrify must carry every candidate, not the winning one: {:?}",
                frame.content[at].tokens
            );
            resolve_slot(
                &frame.content[at].tokens,
                "background",
                DerivedState::of(semantics),
            )
            .map(str::to_owned)
        };

        assert_eq!(
            painted(&build(false, false)).as_deref(),
            Some("surface.base"),
            "no state, so the undecorated binding stands"
        );
        assert_eq!(
            painted(&build(true, false)).as_deref(),
            Some("surface.layer-one"),
            "a disabled node takes its `@disabled` binding"
        );
        assert_eq!(
            painted(&build(false, true)).as_deref(),
            Some("surface.layer-two"),
            "a selected node takes its `@selected` binding"
        );
        assert_eq!(
            painted(&build(true, true)).as_deref(),
            Some("surface.raised"),
            "selected AND disabled takes the named combination, not either \
             half: Carbon names the pair as its own token rather than \
             layering one over the other"
        );
    }
}
