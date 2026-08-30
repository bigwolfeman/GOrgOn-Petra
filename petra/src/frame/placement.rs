//! Placements: exactly one final rect per node per frame.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::draw::DrawList;
use crate::geom::Rect;
use crate::layout::Slot;
use crate::tree::{Edge, Interaction, NodeKind, Role, TextWrap};

/// Paint-relevant state that is not geometry but does change the picture.
///
/// Every field here is a digest input (`contracts/frame-identity.md` §2).
/// Adding one is a serialization change: it needs a line in the leaf-hash
/// stream built by [`crate::frame::digest::leaf_hash`], which destructures
/// this struct with no rest pattern so the compiler asks, and a bump of
/// [`crate::frame::digest::DOMAIN`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PaintState {
    /// Hash of the node's rendered text, or zero when it renders none.
    pub content_hash: u64,
    /// Whether content was hidden by a truncation *rule* this frame: an
    /// ellipsis policy, a `max_lines` cap, or a container conceding under
    /// FR-005 because the fit was impossible.
    ///
    /// A rule that fired is a rule working. A title elided to one line with a
    /// trailing `…` is this flag and is not a defect;
    /// [`PaintState::overflowed`] is the one that is.
    pub truncated: bool,
    /// Whether the content is larger than the rect this node was given, so
    /// the part that did not fit is clipped away rather than elided.
    ///
    /// Separate from [`PaintState::truncated`] because the two have opposite
    /// meanings for a reader of a frame. `truncated` says a policy chose to
    /// hide something and marked where it stopped; `overflowed` says nobody
    /// chose anything — the box was too small, and `layout::text::place`
    /// clips the run to its own rect so the remainder does not paint over the
    /// neighbouring row. One flag carried both until 2026-08-24, which made
    /// an audit rule that reads it unable to tell a working ellipsis from a
    /// corrupted panel.
    ///
    /// Written by [`crate::layout::text::place`], which is the only place
    /// that knows both the run's real extent and the rect it was handed.
    pub overflowed: bool,
    /// Revision of the theme snapshot this node's tokens resolved against.
    ///
    /// This is the *global* snapshot revision, never which token this node
    /// asked for. Rebinding one node's `background` from `surface.raised` to
    /// `status.down` moves no revision anywhere; [`PaintState::paint_hash`] is
    /// what sees that.
    pub token_revision: u64,
    /// Hash of this node's [`PaintContent`] — its token bindings, its text
    /// run's typography and truncation policy, its image source, and its
    /// custom painter name. Zero when the node draws nothing of its own.
    ///
    /// The payload itself lives in a parallel array on
    /// [`crate::frame::PetrifiedFrame`], not in the placement, but the digest
    /// must be recomputable from `(viewport, placements)` alone: the driver's
    /// `frame` response carries exactly those two and no payload array
    /// (`contracts/driver-protocol.md`). So the hash rides here, written by
    /// [`PlacementSink::attach`] rather than by any container — one place, so
    /// the twelve node kinds cannot each forget it differently.
    pub paint_hash: u64,
}

/// The semantic payload a placement carries into the projection.
///
/// Kept beside the geometry rather than recomputed later: the semantic tree,
/// the AccessKit tree, and the driver's finders must describe the frame that
/// was actually placed, and the cheapest way to guarantee that is to project
/// them from the placements themselves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacementSemantics {
    /// What this node is.
    pub role: Option<Role>,
    /// Human-readable name.
    pub label: Option<String>,
    /// Current value for inputs and status readouts.
    pub value: Option<String>,
    /// Whether this node holds keyboard focus in the state snapshot this
    /// frame was placed from.
    ///
    /// Unlike every other flag here this one **is** a digest input: the
    /// painter draws a focus ring from it, so two frames differing only in
    /// which node is focused are two different pictures
    /// (`contracts/frame-identity.md`, §2). Projected from
    /// [`crate::layout::LayoutState::focused`] by
    /// [`crate::layout::semantics_of`]; the focus tree owns focus and this is
    /// its per-placement projection, never a place to write.
    pub focused: bool,
    /// Whether the pointer is inside this node's hit region this frame.
    ///
    /// Engine-derived, exactly like [`PlacementSemantics::focused`], and from
    /// the same hit test the router uses for clicks — an app deriving its own
    /// hover from a pointer route would be a second hit test by a second
    /// owner (`contracts/interaction-state.md` §1). The engine refuses this as
    /// a [`crate::tree::Semantics`] field for that reason.
    ///
    /// A digest input under `gorgon-petra-frame-v6`, the same way `focused`
    /// has been since `v3`: a hovered node is to paint from its own token
    /// family, so two frames differing only in hover are two different
    /// pictures and `Action::Hover` is a mutating action by design
    /// (`contracts/interaction-state.md` §3). The flag is hashed and projected
    /// as of this change; the per-state token binding that makes it visible
    /// lands with the token work.
    pub hovered: bool,
    /// Whether this node is pressed: it holds pointer capture **and** the
    /// pointer is still inside its rect.
    ///
    /// Distinct from [`PlacementSemantics::captured`] rather than a synonym
    /// for it. A button pressed and then dragged off keeps the capture — it
    /// is still the node every move routes to — but stops looking pressed.
    /// Collapsing the two would either leave a button lit while the pointer
    /// is elsewhere, or drop the gesture the moment it left the rect.
    pub active: bool,
    /// Whether this node holds pointer capture this frame.
    ///
    /// Outlives [`PlacementSemantics::active`] on a drag that leaves the rect,
    /// and is what makes a gesture survive crossing a modal's edge
    /// (`contracts/interaction-state.md` §7).
    pub captured: bool,
    /// Declared read-only state.
    ///
    /// **Not** a weaker `disabled`. A read-only node stays Tab-reachable, stays
    /// a legal focus target, and keeps its focus ring; what it declares is
    /// fewer interactions, never a second refusal path
    /// (`contracts/interaction-state.md` §5). Mapping it onto `disabled` in
    /// AccessKit, or adding it to the focus filter, is the defect that rule
    /// exists to prevent.
    pub read_only: bool,
    /// Declared skeleton state: this node stands in for content that has not
    /// arrived, so it paints as a placeholder shape rather than as itself.
    ///
    /// The top of `contracts/interaction-state.md` §4's precedence ladder — a
    /// skeleton is not hoverable, pressable, or disabled-looking, it is simply
    /// not there yet. Carried, hashed and projected here; the resolver that
    /// ranks it against the others lands with the state work.
    pub skeleton: bool,
    /// Declared disabled state.
    pub disabled: bool,
    /// Declared selected state.
    pub selected: bool,
    /// Declared expanded state.
    pub expanded: Option<bool>,
    /// Declared staleness of the projection behind this node.
    pub stale: bool,
    /// Hosts a deliberately endless animation, so it never blocks settle.
    pub ambient: bool,
    /// Driver action kinds this node accepts.
    pub actions: Vec<Interaction>,
    /// Total rows behind a virtualized collection, materialized or not.
    pub total_count: Option<usize>,
}

/// One node's final geometry and paint state for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// Canonical key-path id.
    pub id: String,
    /// The node's kind.
    pub kind: NodeKind,
    /// Final rect in logical units. Device rounding happens once, at petrify.
    pub rect: Rect,
    /// Paint order within the frame. Ties break on placement order.
    pub z: i32,
    /// The clip in force, in logical units.
    pub clip: Rect,
    /// Cumulative opacity in `[0, 1]`.
    pub opacity: f32,
    /// Paint-relevant state.
    pub paint: PaintState,
    /// Semantic payload.
    pub semantics: PlacementSemantics,
    /// Index of the parent placement, or `None` for the root.
    pub parent: Option<usize>,
}

impl Placement {
    /// Whether any of this node's rect is inside the clip the composer set.
    ///
    /// The same test [`crate::input::hit_test`] uses for a pointer: a row
    /// the collection placed in overscan is still in the frame, but it is
    /// not on screen, not clickable, and not Tab-reachable.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.rect.overlaps(self.clip)
    }
}

/// A text run a placement draws, in the form the shaper needs to reproduce
/// exactly the galley the layout was measured against.
///
/// Every field here reaches the digest through
/// [`crate::frame::digest::hash_paint_content`], which destructures this
/// struct with no rest pattern: a new field will not compile until someone
/// decides whether it changes the picture.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPaint {
    /// The content.
    pub text: String,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<String>,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
}

/// The pointer a surface draws back at the node it is anchored to.
///
/// Petra's fourth engine-drawn primitive, beside rects, outlines and text
/// (`contracts/view-tree.md` §Hosted content). It is deliberately **not**
/// carried as a `custom` payload: [`PaintContent::is_hosted`] makes `custom`
/// digest-blind, so a caret riding it could move from one side of a popover
/// to the other without moving the frame digest — a wrong picture under a
/// digest that says it is right.
///
/// Every field is engine-computed from the resolved placement
/// (`contracts/anchored-placement.md` §5) and none is authored: the author
/// cannot know which side the surface ended up on, because the fallback
/// ladder decides that against the window at placement time.
///
/// Reaches the digest through
/// [`crate::frame::digest::hash_paint_content`], which destructures this
/// struct with no rest pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretPaint {
    /// Which side of the anchor the surface was finally placed on — the
    /// resolved edge, after any flip, not the declared one.
    pub side: Edge,
    /// Tip x, logical units: the point that touches the anchor.
    pub tip_x: f32,
    /// Tip y, logical units.
    pub tip_y: f32,
    /// Base width along the surface's near edge, logical units.
    pub w: f32,
    /// Depth from the surface's near edge out to the tip, logical units.
    pub h: f32,
}

/// What a placement draws, beyond its rect.
///
/// Kept beside the placements rather than inside them: the renderer needs the
/// strings, the digest needs a hash, and a driver `frame` response ships the
/// placements without the payload at all. They are different jobs, so they are
/// different arrays, and `PetrifiedFrame` holds the two at equal length with
/// the same index.
///
/// This is **not** the same as being outside the digest, which is what it used
/// to mean. Every field below decides the picture — `gorgon-petra-egui`'s
/// painter reads `tokens` for the fill, the outline, and the text colour, and
/// reads `text` for the galley — so all of it enters the frame digest through
/// [`PaintState::paint_hash`], written by [`PlacementSink::attach`]. What the
/// placement never carries is the *string*; what it always carries is a hash
/// that moves when the string does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PaintContent {
    /// The text run to draw, if this node draws one.
    pub text: Option<TextPaint>,
    /// Image source, if this node draws one.
    pub image: Option<String>,
    /// Registered painter name, if this node is a custom kind.
    pub custom: Option<String>,
    /// Token references by role name, resolved against the frame's theme
    /// snapshot at paint time.
    ///
    /// Carries `props.tokens` verbatim, state-decorated keys and all: a
    /// binding for `background` and one for `background@hover` both arrive
    /// here, and `crate::token::resolve_slot` picks between them at paint
    /// time from this placement's own flags.
    ///
    /// Deliberately not pre-resolved. Collapsing to the winning key would
    /// make this map differ by hover, which puts an interaction state into
    /// the placement stream, and it would erase the bindings an agent reading
    /// the frame needs in order to see what a control *would* do.
    pub tokens: BTreeMap<String, String>,
    /// The caret this placement draws back at its anchor, for a `surface`
    /// anchored to a node and resolved onto a side. `None` for every other
    /// placement, and for an anchored surface whose caret would fall off its
    /// own rounded corner ([`CaretPaint`]).
    pub caret: Option<CaretPaint>,
    /// The draw list a [`crate::tree::NodeKind::Canvas`] executes, `None` for
    /// every other kind.
    ///
    /// Behind an `Arc` because a canvas that does not change hands the same
    /// allocation back every frame, and a payload this large copied per frame
    /// would undo what `Arc<ViewNode>` sharing buys the tree above it.
    ///
    /// **Not** a `custom` payload wearing a different name. `custom` reaches
    /// the digest as a painter's name and is therefore
    /// [`PaintContent::is_hosted`]; this reaches the digest as the picture,
    /// command by command and float by float
    /// (`contracts/draw-list.md` §4), and a canvas is hosted only when it
    /// draws a `Sprite`, whose decoded pixels the digest genuinely cannot see.
    pub canvas: Option<Arc<DrawList>>,
}

impl PaintContent {
    /// Whether this node draws nothing of its own — a bare container.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_none()
            && self.image.is_none()
            && self.custom.is_none()
            && self.tokens.is_empty()
            // A caret is a shape on the screen, so a payload carrying one is
            // not empty — and this line is what keeps it out of
            // `hash_paint_content`'s zero shortcut, which would otherwise
            // make a flipped caret invisible to the digest on a surface that
            // binds no tokens at all.
            && self.caret.is_none()
            // Same reason, one payload later: a canvas that binds no tokens
            // and draws a hundred shapes is not an empty payload, and the
            // zero shortcut in `hash_paint_content` would make every one of
            // those shapes invisible to the digest if it were.
            && self.canvas.is_none()
    }

    /// Whether the host, not Petra, produces this node's pixels.
    ///
    /// True for exactly the two payload members whose *content* the digest
    /// does not reach: [`PaintContent::image`], which the digest hashes as a
    /// source string, and [`PaintContent::custom`], which it hashes as a
    /// painter's name (`contracts/frame-identity.md`, "Not covered": *the
    /// pixels of hosted content*). Petra placed and clipped the region; a
    /// decoder or a registered painter filled it. So two frames in which one
    /// painter drew two different pictures into one rect carry the **same**
    /// digest, and a consumer that wants pixel equality over such a region
    /// must ask for a pixel comparison instead.
    ///
    /// [`PaintContent::text`] and [`PaintContent::tokens`] are **not** hosted:
    /// the digest hashes the string and the token names themselves, so a
    /// changed word or a rebound colour is a changed digest.
    ///
    /// This reads the payload, never the geometry. A hosted node clipped down
    /// to nothing still answers `true` — the flag is an upper bound on where
    /// the digest is blind, and over-reporting costs a consumer a pixel
    /// comparison it did not need, where under-reporting would have it trust
    /// a digest that cannot see the difference.
    ///
    /// This is **not** the question "can this node repaint on its own";
    /// [`PaintContent::repaints_itself`] is. The two answered the same for
    /// every payload that has ever existed, which is exactly why they were one
    /// function until they were split.
    #[must_use]
    pub fn is_hosted(&self) -> bool {
        self.image.is_some()
            || self.custom.is_some()
            // Narrow on purpose. A geometry-only canvas is *not* hosted: every
            // coordinate it draws is hashed, so digest equality really is
            // picture equality over its rect. A canvas with at least one
            // `Sprite` is, because the digest sees the asset's name and its
            // rects and never the decoded pixels — the same blindness
            // `image` has (`contracts/draw-list.md` §6).
            || self
                .canvas
                .as_ref()
                .is_some_and(|list| list.references_assets())
    }

    /// Whether this node can put new pixels on the screen without Petra
    /// placing a new frame.
    ///
    /// The wider of the two predicates `contracts/draw-list.md` §6 binds, and
    /// the one the ambient ledger is written against
    /// ([`crate::anim::AmbientLedger::observe`]): the ledger's question is
    /// *"who could have asked for this repaint"*, which is not
    /// [`PaintContent::is_hosted`]'s question, *"where is the digest blind"*.
    ///
    /// Today the two answer the same for every payload member that exists,
    /// because `custom` — a registered painter free to draw whatever it likes
    /// each frame — has been the only self-repainting content Petra has ever
    /// carried. They are separate functions anyway, ahead of the payload that
    /// separates them, because the alternative is finding this hole a second
    /// time from the other end: an undeclared repainting surface invisible to
    /// the lane that exists to catch it (FR-030, `research.md` D-05).
    ///
    /// The case that separates them is a **geometry-only canvas**: a draw list
    /// with no `Sprite` in it is fully digest-visible — every coordinate it
    /// draws is hashed — so it is never hosted, yet it rebuilds its list each
    /// frame and so always repaints itself. That case exists as of T121/T122,
    /// and the `|| self.canvas.is_some()` below is where the two predicates
    /// finally part company. Neither consumer changed when it landed, which is
    /// the point of having split them first.
    #[must_use]
    pub fn repaints_itself(&self) -> bool {
        // Every canvas, regardless of assets. A canvas rebuilds and resubmits
        // its list to change anything, so it is always a surface that could
        // have asked for the repaint the ambient ledger is trying to attribute
        // (FR-030).
        self.is_hosted() || self.canvas.is_some()
    }
}

/// Where a container sends the placements it produces.
///
/// Placements arrive in tree pre-order; the sink assigns indices and is what
/// makes `parent` correct without every container tracking it.
pub trait PlacementSink {
    /// Record one placement and return its index.
    fn push(&mut self, placement: Placement) -> usize;
    /// Index of the placement a child should name as its parent.
    fn current_parent(&self) -> Option<usize>;
    /// Make `index` the parent for placements pushed until the matching
    /// [`PlacementSink::leave`].
    fn enter(&mut self, index: usize);
    /// Restore the previous parent.
    fn leave(&mut self);
    /// Placements recorded so far, in pre-order.
    fn placed(&self) -> &[Placement];
    /// Re-emit a subtree taken unchanged from a previous frame, returning
    /// the index its root landed at.
    ///
    /// The copy is byte-identical except for [`Placement::parent`], which
    /// holds absolute indices and is rebased by the difference between where
    /// the subtree sat and where it now sits. Rebasing is safe precisely
    /// because `parent` is not hashed: it is redundant with `id`, which is
    /// the full key path. Every other field — including the rect — carries
    /// over untouched, which is sound only because placements hold *absolute*
    /// geometry and the caller has already established that the slot offered
    /// to this subtree is the same one it had.
    fn reuse_subtree(&mut self, sub: SubtreeCopy<'_>) -> usize;

    /// Record the [`Slot`] the placement at `index` was offered.
    ///
    /// Called by the dispatcher once the node's whole subtree has been
    /// placed, for the same reason [`PlacementSink::attach`] is: the
    /// dispatcher holds the effective slot (after this node's own opacity
    /// and z have been folded in) and the index, so no container has to
    /// remember to hand either back.
    ///
    /// This is the input an incremental pass compares against. A placement
    /// carries the rect the node *took*, which a container may make smaller
    /// than what it was *given*; only the offer decides whether re-placing
    /// the subtree would produce the same answer
    /// (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`,
    /// condition 2). Storing the taken rect instead would silently accept a
    /// subtree whose parent moved the box around it.
    fn note_slot(&mut self, index: usize, slot: Slot);

    /// Attach the paint payload for the placement at `index`.
    ///
    /// Called by the dispatcher, not by containers: every container pushes its
    /// own placement first, so the dispatcher knows the index without the
    /// container having to hand it back.
    ///
    /// An implementation MUST also set the placement's
    /// [`PaintState::paint_hash`] to
    /// [`crate::frame::digest::hash_paint_content`] of `content`. A sink that
    /// stores the payload and leaves the hash at zero produces frames whose
    /// digest is blind to every token binding and every typography choice in
    /// them — the exact defect this field exists to close.
    /// [`crate::frame::PetrifiedFrame::paint_hashes_agree`] is the check that
    /// catches a sink that skipped it.
    fn attach(&mut self, index: usize, content: PaintContent);

    /// How many placements have been recorded.
    fn len(&self) -> usize {
        self.placed().len()
    }

    /// Whether nothing has been placed yet.
    fn is_empty(&self) -> bool {
        self.placed().is_empty()
    }
}

/// A sink that collects placements into a vector in pre-order.
#[derive(Debug, Default)]
pub struct PlacementList {
    placements: Vec<Placement>,
    content: Vec<PaintContent>,
    /// How many placements each index's subtree occupies, itself included:
    /// placement `i`'s subtree is the contiguous range
    /// `[i, i + subtree_len[i])`. See [`PlacementList::into_parts`] for why
    /// this is a parallel array rather than a field on [`Placement`].
    subtree_len: Vec<usize>,
    /// The slot each placement was offered, at the same index. `None` until
    /// the dispatcher notes it, which it does for every node it places —
    /// [`PlacementList::into_parts`] refuses a list where one is missing
    /// rather than substituting a default, because a wrong slot in the memo
    /// is a stale subtree in some later frame.
    slots: Vec<Option<Slot>>,
    /// The Merkle subtree hash for any placement that came in through
    /// [`PlacementSink::reuse_subtree`], at the same index. `None` for a
    /// placement this walk built, which must be hashed. Skipping the fold for
    /// a reused subtree is the second half of what reuse buys: the first is
    /// not building the placements, and this is not re-hashing them.
    reused_hashes: Vec<Option<[u8; 32]>>,
    stack: Vec<usize>,
}

impl PlacementList {
    /// An empty list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The collected placements, in tree pre-order.
    #[must_use]
    pub fn as_slice(&self) -> &[Placement] {
        &self.placements
    }

    /// Take just the collected placements.
    #[must_use]
    pub fn into_vec(self) -> Vec<Placement> {
        self.placements
    }

    /// Take the collected placements, their paint payloads, and their
    /// subtree extents, in the same order and at equal length.
    ///
    /// In debug builds, checks the extents against a second, independent
    /// derivation of the same tree — a walk of `Placement::parent` — and
    /// panics naming the first placement where the two disagree. Two
    /// derivations of one tree that can silently drift apart is exactly the
    /// kind of defect that later shows up as a wrong subtree copied by the
    /// incremental placement path.
    ///
    /// # Panics
    /// If any placement's slot was never noted. That is a dispatcher bug, not
    /// an input error, and it is fatal rather than defaulted: a missing slot
    /// would be silently filled with a rect no node was offered, and the
    /// first incremental frame to compare against it would reuse a subtree
    /// whose parent had moved.
    #[must_use]
    pub fn into_parts(self) -> PlacedTree {
        debug_assert_eq!(self.placements.len(), self.content.len());
        debug_assert_eq!(self.placements.len(), self.subtree_len.len());
        debug_assert_eq!(self.placements.len(), self.slots.len());
        debug_assert!(
            extents_match_parents(&self.placements, &self.subtree_len),
            "subtree_len disagrees with Placement::parent"
        );
        let slots = self
            .slots
            .iter()
            .enumerate()
            .map(|(i, slot)| {
                slot.unwrap_or_else(|| {
                    panic!(
                        "placement {i} ({}) was pushed without its slot ever \
                         being noted; every node the dispatcher places must \
                         reach `PlacementSink::note_slot`",
                        self.placements[i].id
                    )
                })
            })
            .collect();
        PlacedTree {
            placements: self.placements,
            content: self.content,
            subtree_len: self.subtree_len,
            slots,
        }
    }

    /// The paint payloads, indexed alongside [`PlacementList::as_slice`].
    #[must_use]
    pub fn content(&self) -> &[PaintContent] {
        &self.content
    }

    /// The subtree extents, indexed alongside [`PlacementList::as_slice`].
    /// See [`PlacementList::into_parts`].
    #[must_use]
    pub fn subtree_len(&self) -> &[usize] {
        &self.subtree_len
    }

    /// Per-placement Merkle hashes already known from a previous frame, at
    /// the same index as [`PlacementList::as_slice`]. `None` means "this walk
    /// built it, hash it".
    #[must_use]
    pub fn reused_hashes(&self) -> &[Option<[u8; 32]>] {
        &self.reused_hashes
    }

    /// The slot noted for `index`, or `None` if the dispatcher has not
    /// reached it yet. Mid-walk this is genuinely `None` for every ancestor
    /// still on the stack, which is why it is not a `&[Slot]`.
    #[must_use]
    pub fn slot(&self, index: usize) -> Option<Slot> {
        self.slots.get(index).copied().flatten()
    }

    /// How many placements were collected.
    #[must_use]
    pub fn len(&self) -> usize {
        self.placements.len()
    }

    /// Whether nothing was placed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }
}

/// One subtree lifted from a previous frame, ready to be re-emitted.
///
/// Every slice is the same length and covers the same contiguous placement
/// range, and `base` is the index that range started at in the frame it came
/// from — needed because [`Placement::parent`] holds absolute indices and a
/// copied subtree almost never lands at the same absolute position.
#[derive(Clone, Copy, Debug)]
pub struct SubtreeCopy<'a> {
    /// The subtree's placements, in pre-order, root first.
    pub placements: &'a [Placement],
    /// Their paint payloads.
    pub content: &'a [PaintContent],
    /// Their subtree extents.
    pub subtree_len: &'a [usize],
    /// The slots they were offered.
    pub slots: &'a [Slot],
    /// Their Merkle subtree hashes.
    pub hashes: &'a [[u8; 32]],
    /// The index `placements[0]` sat at in the frame this came from.
    pub base: usize,
}

/// Everything one placement walk produced, at equal length and equal index.
///
/// A named struct rather than a tuple because there are now four arrays and
/// three of them are `Vec`s of the same shape: `(placements, content,
/// subtree_len, slots)` is exactly the kind of signature where two get
/// swapped at a call site and nothing complains.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedTree {
    /// Every node's final geometry, in tree pre-order.
    pub placements: Vec<Placement>,
    /// What each placement draws.
    pub content: Vec<PaintContent>,
    /// How many placements each index's subtree occupies, itself included.
    pub subtree_len: Vec<usize>,
    /// The slot each placement was offered — what an incremental pass
    /// compares to decide whether re-placing would change anything.
    pub slots: Vec<Slot>,
}

impl PlacementSink for PlacementList {
    fn push(&mut self, mut placement: Placement) -> usize {
        placement.parent = self.stack.last().copied();
        self.placements.push(placement);
        self.content.push(PaintContent::default());
        // A leaf until proven otherwise: `leave` grows this to the full
        // subtree size once every descendant has been pushed.
        self.subtree_len.push(1);
        // Noted by the dispatcher once this node's subtree is complete.
        self.slots.push(None);
        // Built by this walk, so it has no inherited hash.
        self.reused_hashes.push(None);
        self.placements.len() - 1
    }

    fn placed(&self) -> &[Placement] {
        &self.placements
    }

    fn reuse_subtree(&mut self, sub: SubtreeCopy<'_>) -> usize {
        debug_assert_eq!(sub.placements.len(), sub.content.len());
        debug_assert_eq!(sub.placements.len(), sub.subtree_len.len());
        debug_assert_eq!(sub.placements.len(), sub.slots.len());
        debug_assert_eq!(sub.placements.len(), sub.hashes.len());
        let root = self.placements.len();
        let parent_of_root = self.stack.last().copied();
        for (offset, placement) in sub.placements.iter().enumerate() {
            let mut copy = placement.clone();
            copy.parent = if offset == 0 {
                parent_of_root
            } else {
                // Inside the copied range every parent is also inside it, so
                // the rebase is one shift. A parent outside the range would
                // mean the caller handed over something that is not a subtree.
                let old = placement
                    .parent
                    .expect("a non-root placement in a reused subtree must name a parent");
                // A real `assert!`, not `debug_assert!`: `digest.rs:401` already
                // enforces the analogous pre-order invariant (`parent` names a
                // strictly earlier index) in release, and this is the same class
                // of check — one comparison per reused placement — on the same
                // caller-suppliable data (`SubtreeCopy` has no sealed
                // constructor). Leaving it compiled out in release would let a
                // malformed subtree rebase silently in the one build the checked
                // path is supposed to protect.
                assert!(
                    old >= sub.base && old < sub.base + sub.placements.len(),
                    "reused subtree is not self-contained: a placement names parent {old}, \
                     outside [{}, {})",
                    sub.base,
                    sub.base + sub.placements.len()
                );
                Some(old - sub.base + root)
            };
            self.placements.push(copy);
        }
        self.content.extend_from_slice(sub.content);
        self.subtree_len.extend_from_slice(sub.subtree_len);
        self.slots.extend(sub.slots.iter().copied().map(Some));
        self.reused_hashes
            .extend(sub.hashes.iter().copied().map(Some));
        root
    }

    fn note_slot(&mut self, index: usize, slot: Slot) {
        self.slots[index] = Some(slot);
    }

    fn attach(&mut self, index: usize, content: PaintContent) {
        let hash = crate::frame::digest::hash_paint_content(&content);
        let Some(slot) = self.content.get_mut(index) else {
            // No payload slot means no placement either — the two vectors are
            // pushed together. Nothing landed, so nothing is hashed; leaving
            // the placement's hash alone keeps the two in agreement.
            return;
        };
        *slot = content;
        if let Some(placement) = self.placements.get_mut(index) {
            placement.paint.paint_hash = hash;
        }
    }

    fn current_parent(&self) -> Option<usize> {
        self.stack.last().copied()
    }

    fn enter(&mut self, index: usize) {
        self.stack.push(index);
    }

    fn leave(&mut self) {
        // Placements arrive in strict pre-order and every container brackets
        // its children with `enter`/`leave` — no container defers — so by the
        // time the matching `leave` runs, every descendant this node will
        // ever have has already been pushed, and the subtree is exactly the
        // contiguous range from this index to the current length.
        if let Some(parent_idx) = self.stack.pop() {
            self.subtree_len[parent_idx] = self.placements.len() - parent_idx;
        }
    }
}

/// Recompute each placement's subtree size from [`Placement::parent`] alone,
/// and compare it against `subtree_len`. Two independent derivations of the
/// same tree; used only to check they agree.
fn extents_match_parents(placements: &[Placement], subtree_len: &[usize]) -> bool {
    let n = placements.len();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (child_idx, p) in placements.iter().enumerate() {
        if let Some(parent_idx) = p.parent {
            children[parent_idx].push(child_idx);
        }
    }
    let mut computed = vec![1usize; n];
    for i in (0..n).rev() {
        let sum: usize = children[i].iter().map(|&c| computed[c]).sum();
        computed[i] = 1 + sum;
    }
    computed == subtree_len
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use std::sync::Arc;

    use super::{
        PaintContent, PaintState, Placement, PlacementList, PlacementSemantics, PlacementSink,
        TextPaint,
    };
    use crate::draw::{ColorRef, Command, Corners, DrawList, Paint};
    use crate::geom::Rect;
    use crate::layout::Slot;
    use crate::tree::{NodeKind, TextWrap};

    fn placement(id: &str) -> Placement {
        Placement {
            id: id.into(),
            kind: NodeKind::Text,
            rect: Rect::ZERO,
            z: 0,
            clip: Rect::ZERO,
            opacity: 1.0,
            paint: PaintState::default(),
            semantics: PlacementSemantics::default(),
            parent: None,
        }
    }

    #[test]
    fn the_sink_wires_parents_from_the_walk() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let a = list.push(placement("/root/a"));
        list.enter(a);
        list.push(placement("/root/a/x"));
        list.leave();
        list.push(placement("/root/b"));
        list.leave();

        let out = list.as_slice();
        assert_eq!(out.len(), 4);
        assert_eq!(out[0].parent, None);
        assert_eq!(out[1].parent, Some(0));
        assert_eq!(out[2].parent, Some(1));
        assert_eq!(out[3].parent, Some(0));
        let ids: Vec<&str> = out.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["/root", "/root/a", "/root/a/x", "/root/b"]);
    }

    #[test]
    fn a_placement_overrides_a_caller_supplied_parent() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let mut lying = placement("/root/a");
        lying.parent = Some(99);
        list.push(lying);
        assert_eq!(list.as_slice()[1].parent, Some(0));
        assert_eq!(list.current_parent(), Some(0));
    }

    /// The same tree as `the_sink_wires_parents_from_the_walk`: `/root` has
    /// two children, `/root/a` has one. Each subtree's length is itself plus
    /// every descendant, and the whole tree occupies one contiguous range
    /// from the root.
    #[test]
    fn subtree_len_covers_exactly_each_nodes_descendants() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let a = list.push(placement("/root/a"));
        list.enter(a);
        list.push(placement("/root/a/x"));
        list.leave();
        list.push(placement("/root/b"));
        list.leave();

        assert_eq!(list.subtree_len(), &[4, 2, 1, 1]);
    }

    /// A leaf that never calls `enter`/`leave` keeps the length `push` gave
    /// it: one, itself alone.
    #[test]
    fn a_childless_placement_has_subtree_len_one() {
        let mut list = PlacementList::new();
        list.push(placement("/root"));
        assert_eq!(list.subtree_len(), &[1]);
    }

    /// `into_parts` hands back the same lengths `subtree_len` exposed before
    /// consuming the list, alongside the placements and payloads at equal
    /// length.
    #[test]
    fn into_parts_carries_the_same_extents() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let a = list.push(placement("/root/a"));
        list.leave();
        // The dispatcher notes these during a real walk; this test drives the
        // sink directly, so it stands in for the dispatcher.
        list.note_slot(a, Slot::new(Rect::new(0.0, 0.0, 5.0, 5.0)));
        list.note_slot(root, Slot::new(Rect::new(0.0, 0.0, 10.0, 10.0)));

        let placed = list.into_parts();
        assert_eq!(placed.placements.len(), 2);
        assert_eq!(placed.content.len(), 2);
        assert_eq!(placed.subtree_len, vec![2, 1]);
        assert_eq!(placed.slots.len(), 2);
        assert_eq!(placed.slots[1].rect, Rect::new(0.0, 0.0, 5.0, 5.0));
    }

    /// The sabotage proof for the debug assertion in `into_parts`: build a
    /// `subtree_len` that disagrees with `Placement::parent` and show
    /// [`extents_match_parents`] says so. `into_parts` itself cannot be
    /// sabotaged from outside the module — its `subtree_len` is always the
    /// one the walk built — so this drives the checking function directly,
    /// the same way `into_parts`'s `debug_assert!` does.
    #[test]
    fn extents_match_parents_catches_a_disagreement() {
        let mut list = PlacementList::new();
        let root = list.push(placement("/root"));
        list.enter(root);
        let a = list.push(placement("/root/a"));
        list.leave();
        list.note_slot(a, Slot::new(Rect::new(0.0, 0.0, 5.0, 5.0)));
        list.note_slot(root, Slot::new(Rect::new(0.0, 0.0, 10.0, 10.0)));
        let placed = list.into_parts();
        let (placements, subtree_len) = (placed.placements, placed.subtree_len);

        assert!(super::extents_match_parents(&placements, &subtree_len));

        let mut wrong = subtree_len.clone();
        wrong[0] = 1; // claims the root has no children; `parent` says it does
        assert!(
            !super::extents_match_parents(&placements, &wrong),
            "a subtree_len that ignores a real child must be caught"
        );
    }

    /// A geometry-only draw list: one filled rect, no `Sprite`.
    fn geometry_only() -> Arc<DrawList> {
        Arc::new(
            DrawList::new(vec![Command::Rect {
                rect: Rect::new(0.0, 0.0, 8.0, 8.0),
                radius: Corners::SQUARE,
                snap: false,
                paint: Paint::filled(ColorRef::Token("status.ok".into())),
            }])
            .expect("one rect is inside every bound"),
        )
    }

    /// The same list plus one `Sprite`, which is the only thing that makes a
    /// canvas hosted.
    fn with_a_sprite() -> Arc<DrawList> {
        Arc::new(
            DrawList::new(vec![Command::Sprite {
                asset: crate::draw::AssetRef::host("cat.png"),
                dst: Rect::new(0.0, 0.0, 8.0, 8.0),
                src: None,
                fit: crate::draw::Fit::Contain,
                tint: None,
            }])
            .expect("one sprite is inside every bound"),
        )
    }

    /// `is_hosted` answers for exactly the two payload members the digest
    /// reaches by name rather than by content, and for nothing else.
    ///
    /// Written as a table so a member moving from one side of the line to the
    /// other is one changed row rather than a silently absent case. The two
    /// text rows matter most: a hosted flag that answered `true` for a text
    /// run would tell every consumer of a plain label frame to fall back to
    /// pixel comparison, which the digest covers perfectly well.
    #[test]
    fn only_an_image_or_a_custom_painter_makes_a_payload_hosted() {
        let text = TextPaint {
            text: "hi".into(),
            style: None,
            wrap: TextWrap::Wrap,
            max_lines: None,
        };
        let mut tokens = BTreeMap::new();
        tokens.insert("background".to_owned(), "surface.raised".to_owned());

        let cases: [(&str, PaintContent, bool); 8] = [
            ("a bare container", PaintContent::default(), false),
            (
                "a text run",
                PaintContent {
                    text: Some(text.clone()),
                    ..PaintContent::default()
                },
                false,
            ),
            (
                "token bindings alone",
                PaintContent {
                    tokens: tokens.clone(),
                    ..PaintContent::default()
                },
                false,
            ),
            (
                "an image",
                PaintContent {
                    image: Some("logo.png".into()),
                    ..PaintContent::default()
                },
                true,
            ),
            (
                "a custom painter",
                PaintContent {
                    custom: Some("gauge".into()),
                    ..PaintContent::default()
                },
                true,
            ),
            (
                "an image and a painter on one node",
                PaintContent {
                    text: Some(text),
                    image: Some("logo.png".into()),
                    custom: Some("gauge".into()),
                    tokens,
                    caret: None,
                    canvas: None,
                },
                true,
            ),
            (
                "a geometry-only canvas",
                PaintContent {
                    canvas: Some(geometry_only()),
                    ..PaintContent::default()
                },
                false,
            ),
            (
                "a canvas that draws a sprite",
                PaintContent {
                    canvas: Some(with_a_sprite()),
                    ..PaintContent::default()
                },
                true,
            ),
        ];

        for (what, content, want) in cases {
            assert_eq!(content.is_hosted(), want, "{what}");
            // The wider predicate is a *superset* of this one, on every
            // payload shape, always. That direction is the half a future
            // canvas term could get wrong: adding `canvas` to `is_hosted`
            // without adding it to `repaints_itself` would make a
            // sprite-carrying canvas hosted and yet not repaint-capable,
            // which is the ambient hole of `research.md` D-05 reopened from
            // the other end.
            assert!(
                !content.is_hosted() || content.repaints_itself(),
                "{what}: hosted content must always be able to repaint itself"
            );
        }
    }

    /// A bare container repaints nothing of its own.
    ///
    /// The lower bound on the wider predicate, kept beside its upper bound
    /// above: a predicate that answered `true` for every placement would
    /// satisfy the superset check and make the ambient ledger attribute an
    /// unexplained repaint to every node in the frame.
    #[test]
    fn a_payload_that_draws_nothing_repaints_nothing() {
        let empty = PaintContent::default();
        assert!(!empty.repaints_itself());
        let text = PaintContent {
            text: Some(TextPaint {
                text: "hi".into(),
                style: None,
                wrap: TextWrap::Wrap,
                max_lines: None,
            }),
            ..PaintContent::default()
        };
        assert!(
            !text.repaints_itself(),
            "a text run is redrawn by Petra, never by itself"
        );
    }

    /// **The divergence.** `contracts/draw-list.md` §6 binds two predicates
    /// and forbids them collapsing into one. Wave 1 split them ahead of the
    /// payload that separates them and could not test the split, because no
    /// payload could yet answer the two questions differently. This is that
    /// test.
    ///
    /// A geometry-only canvas is the one case:
    ///
    /// * **not hosted** — every coordinate it draws is in the digest, so a
    ///   screenshot consumer holding `(seq, digest)` may trust it over the
    ///   canvas's rect exactly as it does over a label;
    /// * **repaint-capable** — it rebuilds its list to change anything, so it
    ///   is a surface the ambient ledger must attribute an unexplained repaint
    ///   to, declared `ambient` or not.
    ///
    /// Collapsing them either way is a real defect with a name. Widening
    /// `is_hosted` sends every canvas frame to a pixel comparison it does not
    /// need. Narrowing `repaints_itself` takes a self-repainting surface out
    /// of the set FR-030 audits, which is the hole `research.md` D-05 is
    /// about.
    #[test]
    fn a_geometry_only_canvas_is_digest_visible_and_still_repaints_itself() {
        let plain = PaintContent {
            canvas: Some(geometry_only()),
            ..PaintContent::default()
        };
        assert!(
            !plain.is_hosted(),
            "a list with no Sprite has no pixels the digest cannot see"
        );
        assert!(
            plain.repaints_itself(),
            "a canvas rebuilds its own list, so it can put new pixels up              without Petra placing a new frame"
        );
        assert!(!plain.is_empty(), "it draws a rect");

        let sprited = PaintContent {
            canvas: Some(with_a_sprite()),
            ..PaintContent::default()
        };
        assert!(
            sprited.is_hosted(),
            "the digest sees the asset's name and its rects, never its pixels"
        );
        assert!(sprited.repaints_itself());

        // The superset direction, stated over the one pair that finally
        // separates: hosted is a subset of repaint-capable, and the gap is
        // exactly the geometry-only canvas.
        assert!(
            !plain.is_hosted() && plain.repaints_itself(),
            "this is the pair the two predicates exist to tell apart"
        );
    }
}
