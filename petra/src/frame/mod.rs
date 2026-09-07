//! Petrify: turning a view tree into a frame with an identity.
//!
//! One pass produces every placement, one rounding rule turns logical units
//! into device pixels, and one digest fingerprints the result. A frame that
//! completed petrify has an identity; a query mid-negotiation answers the last
//! completed frame (`contracts/frame-identity.md`).

pub mod digest;
pub mod placement;
pub mod rounding;
pub mod viewport;

use std::collections::BTreeSet;

use crate::geom::Rect;
use crate::layout::reuse::{FrameMemo, ReuseState, ReuseStats};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot};
use crate::tree::{KeyPath, Role, ValidatedTree};

pub use digest::{FrameDigest, canonical_decimal, hash_text};
pub use placement::{
    CaretPaint, PaintContent, PaintState, Placement, PlacementList, PlacementSemantics,
    PlacementSink, TextPaint, TextRunPaint,
};
pub use rounding::{DeviceRect, round_coord, round_rect};
pub use viewport::Viewport;

/// How much motion the frame carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransitionActivity {
    /// Transitions still moving toward a target.
    pub running: usize,
    /// Declared-endless animations. These never settle and never block a
    /// driver's settle wait (`contracts/animation.md`).
    pub ambient: usize,
}

impl TransitionActivity {
    /// Whether the frame is settled: nothing is running that is expected to
    /// stop.
    #[must_use]
    pub fn is_settled(self) -> bool {
        self.running == 0
    }
}

/// The finished output of one layout pass.
#[derive(Clone, Debug, PartialEq)]
pub struct PetrifiedFrame {
    /// Monotone within one application run, starting at 1, never reused.
    pub seq: u64,
    /// Content fingerprint over placements and paint state.
    pub digest: FrameDigest,
    /// Every node's final geometry, in tree pre-order.
    pub placements: Vec<Placement>,
    /// What each placement draws, at the same index and the same length.
    ///
    /// Not hashed *directly* — the digest must be recomputable from
    /// `(viewport, placements)` alone, because that is all a driver `frame`
    /// response carries (`contracts/driver-protocol.md`). It reaches the
    /// digest as `placements[i].paint.paint_hash`, written when the payload
    /// was attached. [`PetrifiedFrame::paint_hashes_agree`] is what checks the
    /// two have not drifted.
    pub content: Vec<PaintContent>,
    /// Every placement's Merkle subtree hash, at the same index and the same
    /// length as [`PetrifiedFrame::placements`].
    ///
    /// `subtree_hashes[0]` — the root's — is the value
    /// [`PetrifiedFrame::digest`] was built from
    /// (`digest::root_hash_from`). This is what a reused subtree hands back
    /// to its parent: [`petrify_with_memo`] copies a subtree from the
    /// previous frame and reuses the hash recorded here instead of
    /// re-walking it
    /// (`.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`).
    pub subtree_hashes: Vec<[u8; 32]>,
    /// How many placements each index's subtree occupies, itself included —
    /// see [`crate::frame::placement::PlacementList`] — at the same index
    /// and the same length as [`PetrifiedFrame::placements`].
    ///
    /// This is the range a reuse pass copies: placement `i`'s subtree is
    /// `placements[i..i + subtree_len[i]]`.
    pub subtree_len: Vec<usize>,
    /// The [`Slot`] each placement was *offered*, at the same index and the
    /// same length.
    ///
    /// Not part of the frame's identity and not on the wire: a driver
    /// `frame` response carries placements, and the digest is recomputable
    /// from `(viewport, placements)` alone. This array exists for the
    /// incremental path, which cannot decide whether re-placing a subtree
    /// would change anything by looking at the rect the node *took* — a
    /// container may hand a child less than it asked for, and two different
    /// offers can produce the same taken rect.
    pub slots: Vec<Slot>,
    /// What the frame was negotiated against.
    pub viewport: Viewport,
    /// Motion in force at petrify time.
    pub transitions: TransitionActivity,
}

impl PetrifiedFrame {
    /// Everything under the highlight, as plain text laid out the way the
    /// frame lays it out.
    ///
    /// `None` when nothing is selected. The bytes come from
    /// [`PaintContent::text`] and the ranges from [`PaintContent::selection`],
    /// which is the same pair the painter fills its bands from — so what
    /// leaves on the clipboard is by construction what the operator can see
    /// lit up. There is no second derivation of either to drift.
    ///
    /// # What "laid out the way the frame lays it out" means
    ///
    /// Three rules, and each one is a fact the frame already holds rather
    /// than a guess about intent:
    ///
    /// 1. **Two runs on the same line are one line of text.** Same line means
    ///    their rects overlap vertically. They are joined by a single space
    ///    unless one side already supplies whitespace at the seam, which is
    ///    what keeps `"Read "` + `"the inline form"` from gaining a second
    ///    gap and `"1."` + `"Clone"` from losing the only one it had.
    ///
    /// 2. **A new line starts a new line.**
    ///
    /// 3. **A run inside nested lists is indented two spaces per level below
    ///    the first.** The level is the count of [`Role::List`] and
    ///    [`Role::Tree`] ancestors, which is exact; reading the indent off the
    ///    rects would be reading a number the layout chose for pixels back as
    ///    though it were structure.
    ///
    /// The operator asked for markdown — *"This should also probably copy to
    /// clip in markdown if we can?"* — and this is as far as that goes
    /// honestly. An ordered list's markers are painted text (`1.`, `a.`,
    /// `i.`) and copy themselves. An unordered list's are drawn shapes
    /// (`crate::component::list`: *"a typed marker cannot be sized, centred
    /// or snapped"*), so they are not in the text and are not invented here.
    /// A link has no target in the model at all — [`crate::component::link`]
    /// takes a label and no href — so `[text](url)` has no url to write.
    /// Transcribing pictures into syntax is a separate decision from copying
    /// text, and it is not this function's to make.
    #[must_use]
    pub fn selected_text(&self) -> Option<String> {
        let mut out = String::new();
        let mut previous: Option<&Placement> = None;
        for (index, content) in self.content.iter().enumerate() {
            let (Some(range), Some(text)) = (content.selection.clone(), content.text.as_ref())
            else {
                continue;
            };
            let Some(taken) = text.text.get(range) else {
                continue;
            };
            let placement = &self.placements[index];
            match previous {
                None => out.push_str(&self.indent_of(index)),
                Some(previous) if shares_a_line(previous, placement) => {
                    if !out.ends_with(char::is_whitespace)
                        && !taken.starts_with(char::is_whitespace)
                    {
                        out.push(' ');
                    }
                }
                Some(_) => {
                    out.push('\n');
                    out.push_str(&self.indent_of(index));
                }
            }
            out.push_str(taken);
            previous = Some(placement);
        }
        (!out.is_empty()).then_some(out)
    }

    /// The leading spaces for a run, two per list level below the first.
    fn indent_of(&self, index: usize) -> String {
        let mut levels = 0usize;
        let mut cursor = Some(index);
        while let Some(at) = cursor {
            let placement = &self.placements[at];
            if matches!(placement.semantics.role, Some(Role::List | Role::Tree)) {
                levels += 1;
            }
            cursor = placement.parent;
        }
        " ".repeat(levels.saturating_sub(1) * 2)
    }

    /// The placement with `id`, if the frame has one.
    #[must_use]
    pub fn placement(&self, id: &str) -> Option<&Placement> {
        self.placements.iter().find(|p| p.id == id)
    }

    /// What the placement with `id` draws, if the frame has one.
    ///
    /// The peer of [`PetrifiedFrame::placement`] for the questions geometry
    /// cannot answer: which token a node bound, which bytes of it are
    /// selected. `content` is a parallel array rather than a member of
    /// `Placement` — a driver `frame` response carries the placements and not
    /// the payloads — so reaching it means finding the index, and three
    /// callers had grown their own copy of that walk.
    #[must_use]
    pub fn content_of(&self, id: &str) -> Option<&PaintContent> {
        let index = self.placements.iter().position(|p| p.id == id)?;
        self.content.get(index)
    }

    /// Every placement paired with what it draws, in tree pre-order.
    pub fn drawn(&self) -> impl Iterator<Item = (&Placement, &PaintContent)> {
        self.placements.iter().zip(self.content.iter())
    }

    /// Every placement paired with what it draws, in paint order: ascending
    /// `z`, then tree order. The sort is stable, so two frames with identical
    /// input paint in identical order.
    #[must_use]
    pub fn paint_pairs(&self) -> Vec<(&Placement, &PaintContent)> {
        let mut out: Vec<(&Placement, &PaintContent)> = self.drawn().collect();
        out.sort_by_key(|(p, _)| p.z);
        out
    }

    /// Whether any placement in this frame is hosted — whether the host, not
    /// Petra, produced some of this frame's pixels
    /// ([`PaintContent::is_hosted`]).
    ///
    /// This is the flag FR-060 requires the frame to carry, and the one the
    /// driver's `frame` and `screenshot` responses surface
    /// (`contracts/frame-identity.md`, "Not covered"). It tells a consumer
    /// which of two claims it is holding:
    ///
    /// - `false`: two screenshots carrying one digest are the same picture.
    ///   Digest equality is picture equality.
    /// - `true`: "this screenshot is of the frame with this digest" still
    ///   holds, but two screenshots with one digest may show different
    ///   pictures, because the digest hashes an image's source string and a
    ///   custom painter's name, never what either drew. A gate that wants
    ///   pixel equality over such a region asks for a pixel comparison.
    ///
    /// Derived from [`PetrifiedFrame::content`] on demand rather than stored
    /// beside it. A stored copy would be a second source of truth over the
    /// same public field, free to drift from it exactly the way a stale
    /// `paint_hash` can — and [`PetrifiedFrame::paint_hashes_agree`] exists
    /// because that drift is not hypothetical. There is nothing here to keep
    /// in sync, so nothing here can go stale.
    ///
    /// The frame is the unit because that is the unit a screenshot verifies.
    /// Which placements are hosted is [`PetrifiedFrame::hosted_placements`].
    #[must_use]
    pub fn hosted(&self) -> bool {
        // Over `content` rather than `drawn()`: the zip in `drawn` stops at
        // the shorter of the two arrays, so a frame whose payload array
        // outran its placements — which `paint_hashes_agree` calls a
        // disagreement — would hide a hosted payload in the tail. Both
        // arrays are public; this one answers over all of the one it reads.
        self.content.iter().any(PaintContent::is_hosted)
    }

    /// Every hosted placement paired with what it draws, in tree pre-order.
    ///
    /// The rects a pixel comparison covers when
    /// [`PetrifiedFrame::hosted`] is true: these are the regions where digest
    /// equality does not imply picture equality, and nowhere else in the
    /// frame is.
    ///
    /// Pairs the two arrays, so on a malformed frame whose payload array
    /// outran its placements this can come up empty where
    /// [`PetrifiedFrame::hosted`] says `true` — there is no placement to
    /// report. That frame's digest already describes a picture it no longer
    /// draws; [`PetrifiedFrame::paint_hashes_agree`] is the check that names
    /// it, and neither accessor is the place to paper over it.
    pub fn hosted_placements(&self) -> impl Iterator<Item = (&Placement, &PaintContent)> {
        self.drawn().filter(|(_, content)| content.is_hosted())
    }

    /// Every placement that can put new pixels on the screen without Petra
    /// placing a new frame, paired with what it draws, in tree pre-order.
    ///
    /// A superset of [`PetrifiedFrame::hosted_placements`] and a different
    /// question, kept apart on purpose: that one answers *"where is the digest
    /// blind"*, this one answers *"who could have asked for this repaint"*.
    /// See [`PaintContent::repaints_itself`] for why the two coincide today
    /// and what separates them.
    ///
    /// This is the set the ambient ledger attributes an unexplained repaint to
    /// ([`crate::anim::AmbientLedger::observe`]), and the set FR-030's
    /// `idle-audit` gate is to be written against when it is built. A consumer
    /// wanting the pixel-vs-digest set instead wants
    /// [`PetrifiedFrame::hosted_placements`].
    pub fn self_repainting_placements(&self) -> impl Iterator<Item = (&Placement, &PaintContent)> {
        self.drawn()
            .filter(|(_, content)| content.repaints_itself())
    }

    /// Whether every placement's paint hash still describes the payload at the
    /// same index.
    ///
    /// The digest hashes `paint.paint_hash`; the renderer draws `content`.
    /// Both are public fields, so the two can be made to disagree — and a
    /// frame whose payload was edited after petrify would keep the digest of
    /// the picture it no longer draws. This is the check that says so; a
    /// length mismatch is a disagreement too, since the tail has no hash to
    /// compare against.
    #[must_use]
    pub fn paint_hashes_agree(&self) -> bool {
        self.placements.len() == self.content.len()
            && self
                .drawn()
                .all(|(p, c)| p.paint.paint_hash == digest::hash_paint_content(c))
    }

    /// Placements in paint order: ascending `z`, then placement order.
    ///
    /// A stable sort is required, not merely convenient: ties inside one layer
    /// must resolve to tree order, or two frames with identical input could
    /// paint in different orders.
    #[must_use]
    pub fn paint_order(&self) -> Vec<&Placement> {
        let mut out: Vec<&Placement> = self.placements.iter().collect();
        out.sort_by_key(|p| p.z);
        out
    }
}

/// Allocates frame sequence numbers for one application run.
#[derive(Debug)]
pub struct FrameCounter {
    next: u64,
}

impl Default for FrameCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameCounter {
    /// A counter whose first frame is 1.
    #[must_use]
    pub fn new() -> Self {
        Self { next: 1 }
    }

    /// The sequence number the next petrify will take.
    #[must_use]
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Take the next sequence number.
    pub fn take(&mut self) -> u64 {
        let seq = self.next;
        self.next += 1;
        seq
    }
}

/// Negotiate `tree` against `viewport`, carrying over from `memo` every
/// subtree that provably did not change, and report what was carried.
///
/// This is [`petrify`] with a previous frame to compare against. `dirty` is
/// the set [`FrameMemo::dirty_ids`] derives from the host's [`ChangeSet`] and
/// from the state the engine can diff for itself; it says *where* to look,
/// and pointer identity then decides what may actually be kept. The design,
/// including why both are needed, is
/// `.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`.
///
/// The frame this returns is byte-for-byte what [`petrify`] would have
/// produced from the same tree — placements, paint payloads, subtree hashes
/// and digest alike. `gorgon/petra/tests/incremental_frames.rs` asserts that
/// equality directly rather than comparing digests, because a pass that
/// reused nothing would match on the digest too.
///
/// A memo that cannot be used at all — a theme or scale change, or an empty
/// previous frame — costs one comparison and then a full negotiation. There
/// is no failure mode here that produces a wrong frame slowly; the failure
/// mode is a correct frame at full price.
pub fn petrify_with_memo<'a>(
    seq: u64,
    tree: ValidatedTree<'a>,
    ctx: &mut LayoutCtx<'a>,
    memo: &'a FrameMemo,
    dirty: &'a BTreeSet<String>,
    viewport: Viewport,
    transitions: TransitionActivity,
) -> (PetrifiedFrame, ReuseStats) {
    // Theme and scale are inputs to every placement this crate produces:
    // `PaintState::token_revision` carries the theme snapshot, and the scale
    // decides the device rounding the digest hashes. Neither is visible in a
    // node's identity or its slot, so a subtree can be pointer-identical and
    // identically offered and still owe a different placement when either
    // moves. There is no partial answer here — every placement is affected —
    // so the whole memo is set aside and the frame is negotiated in full.
    let reusable_at_all =
        memo.theme_rev == ctx.theme_rev && memo.scale == ctx.scale && !memo.placements.is_empty();
    if reusable_at_all {
        ctx.reuse = Some(ReuseState::new(memo, dirty));
    }
    let frame = petrify(seq, tree, ctx, viewport, transitions);
    // Behind `debug_assertions` only: every caller of this function already
    // holds a dirty set derived from `FrameMemo::dirty_ids`, which is `None`
    // (and so unreachable here — see that function) for `ChangeSet::All`.
    // So whenever `ctx.reuse` was set up at all, `dirty` came from `Nodes` or
    // `None`, and the verifier's precondition holds without this function
    // needing to see the original `ChangeSet` itself.
    #[cfg(debug_assertions)]
    if let Some(state) = ctx.reuse.as_ref() {
        state.verify_declaration(&tree);
    }
    let stats = ctx
        .reuse
        .as_ref()
        .map(ReuseState::stats)
        .unwrap_or_default();
    ctx.reuse = None;
    (frame, stats)
}

/// Whether two placements sit on the same line of the picture.
///
/// Overlap on the vertical, not equality of `y`: a link and the prose it sits
/// in are the same line even when one is a point taller than the other, and a
/// list marker and its label are the same line even when the marker is
/// centred in a taller row.
fn shares_a_line(a: &Placement, b: &Placement) -> bool {
    a.rect.y < b.rect.y + b.rect.h && b.rect.y < a.rect.y + a.rect.h
}

/// Negotiate `tree` against `viewport` and produce a frame with an identity.
///
/// The root is offered the viewport exactly and **placed into the viewport**,
/// whatever it answers. `measure` stays honest — a root whose minimums do not
/// fit answers with what it actually needs, and `crate::layout::stack` has a
/// test pinning that it does not clamp — but placement is where the budget is
/// enforced, so the root is handed the real window and concedes into it.
///
/// This was the other way round at first: the slot was built from the root's
/// own response. Two things followed, and both were wrong. A root asking for
/// more than the window was placed oversized and clipped, so
/// `place` saw `wanted == available`, never called `concede`, and the whole
/// FR-005 concession order was unreachable at the top level — the one place a
/// window is guaranteed to be a hard budget. And a root answering *smaller*
/// than the window did not fill it: an app whose root is a stack around one
/// short label rendered as a narrow column against the left edge.
pub fn petrify(
    seq: u64,
    tree: ValidatedTree<'_>,
    ctx: &mut LayoutCtx<'_>,
    viewport: Viewport,
    transitions: TransitionActivity,
) -> PetrifiedFrame {
    let tree = &*tree;
    let mut path = KeyPath::root();
    let offer = SizeProposal {
        horizontal: Proposal::Exact(viewport.size.w),
        vertical: Proposal::Exact(viewport.size.h),
    };
    // The measure pass still runs: it populates the measurement cache that
    // `place` reads back, and skipping it would make every container
    // re-negotiate from scratch during placement.
    let _ = crate::layout::measure(tree, ctx, &mut path, offer);
    let viewport_rect = Rect::new(0.0, 0.0, viewport.size.w, viewport.size.h);
    let slot = Slot {
        rect: viewport_rect,
        z: 0,
        clip: viewport_rect,
        opacity: 1.0,
        window: viewport_rect,
    };
    let mut sink = PlacementList::new();
    crate::layout::place(tree, ctx, &mut path, slot, &mut sink);
    debug_assert!(
        path.is_empty(),
        "the walk must leave the path as it found it"
    );
    let mut reused_hashes = sink.reused_hashes().to_vec();
    let placed = sink.into_parts();
    let (mut placements, mut content, subtree_len, slots) = (
        placed.placements,
        placed.content,
        placed.subtree_len,
        placed.slots,
    );
    // The one payload member no walk can write, because whether a run is
    // inside the span depends on placements the walk had not reached when it
    // passed that run. See `crate::layout::selection`.
    if let Some(selection) = ctx.state.text_selection.as_ref() {
        let touched = crate::layout::selection::resolve(&mut placements, &mut content, selection);
        // A highlight written onto a subtree carried over from the previous
        // frame makes the hash that came with it a lie. `FrameMemo::dirty_ids`
        // already refuses to carry a subtree the selection touches, so this
        // clears nothing on any pass that behaved; it is here because the
        // failure it prevents — a stale Merkle hash over a changed picture —
        // is silent, and one `if` is cheaper than the frame that proves it.
        for index in touched {
            let mut cursor = Some(index);
            while let Some(at) = cursor {
                if let Some(slot) = reused_hashes.get_mut(at) {
                    *slot = None;
                }
                cursor = placements[at].parent;
            }
        }
    }
    // Computed once: the array is what a reuse pass needs, and the root
    // entry is what the frame digest is built from — no reason to walk the
    // Merkle tree a second time to get the same root hash.
    let subtree_hashes = digest::subtree_hashes_with(viewport.scale, &placements, &reused_hashes);
    let digest = digest::digest_from_root(&viewport, digest::root_hash_from(&subtree_hashes));
    let frame = PetrifiedFrame {
        seq,
        digest,
        placements,
        content,
        subtree_hashes,
        subtree_len,
        slots,
        viewport,
        transitions,
    };
    debug_assert!(
        frame.paint_hashes_agree(),
        "a sink attached a paint payload without hashing it into the \
         placement: the digest of this frame is blind to what it draws"
    );
    frame
}

#[cfg(test)]
mod tests {
    use super::{FrameCounter, TransitionActivity, Viewport, petrify};
    use crate::geom::Size;
    use crate::testing::{Harness, MonoContent, NoRows, validated, validated_with};
    use crate::token::ThemeMode;
    use crate::tree::{AxisConstraint, Constraints, NodeKind, Props, Registry, ViewNode};

    fn frame_of(tree: &ViewNode, w: f32, h: f32) -> super::PetrifiedFrame {
        let mut harness = Harness::with(MonoContent::default(), NoRows);
        petrify(
            1,
            validated(tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(w, h), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    /// [`frame_of`] against a registry that knows one custom painter name, so
    /// a tree may declare a `custom` node without failing acceptance.
    fn frame_of_custom(tree: &ViewNode, painter: &str, w: f32, h: f32) -> super::PetrifiedFrame {
        let mut registry = Registry::new();
        registry.register_custom_kind(painter);
        let mut harness = Harness::with(MonoContent::default(), NoRows);
        petrify(
            1,
            validated_with(tree, &registry),
            &mut harness.ctx(),
            Viewport::new(Size::new(w, h), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    fn text(key: &str, body: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key).with_props(Props {
            text: Some(body.to_string()),
            ..Props::default()
        })
    }

    /// A root that wants less than the window still gets the window.
    ///
    /// The slot used to be built from the root's own response, so an app whose
    /// root was a stack around one short label rendered in a column as wide as
    /// that label, hard against the left edge, with the rest of the window
    /// blank. Nothing asserted the root rect, so nothing noticed.
    #[test]
    fn a_root_smaller_than_the_window_still_fills_it() {
        let tree = ViewNode::new(NodeKind::Stack, "root").child(text("t", "hi"));
        let frame = frame_of(&tree, 800.0, 600.0);
        let root = &frame.placements[0];
        assert_eq!(root.id, "/root");
        assert_eq!(
            (root.rect.x, root.rect.y, root.rect.w, root.rect.h),
            (0.0, 0.0, 800.0, 600.0),
            "the root is the window, not its own preferred size"
        );
    }

    /// A root that wants more than the window concedes into it.
    ///
    /// With the slot taken from the root's response, `place` saw
    /// `wanted == available`, so `concede` never ran and the overflow was
    /// clipped with no truncation flag anywhere — FR-005's concession order was
    /// unreachable at the top level, which is the one place the budget is
    /// genuinely hard.
    #[test]
    fn a_root_larger_than_the_window_concedes_into_it_and_says_so() {
        let tall = Constraints {
            vertical: AxisConstraint {
                min: Some(400.0),
                ..AxisConstraint::default()
            },
            ..Constraints::default()
        };
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(crate::geom::Axis::Vertical),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Spacer, "a").with_constraints(tall))
            .child(ViewNode::new(NodeKind::Spacer, "b").with_constraints(tall));

        let frame = frame_of(&tree, 300.0, 200.0);
        let root = &frame.placements[0];
        assert_eq!(
            (root.rect.w, root.rect.h),
            (300.0, 200.0),
            "the root is clamped to the window it was given"
        );
        assert!(
            root.paint.truncated,
            "800 units of declared minimum in a 200-unit window must be \
             reported as truncated, not clipped in silence: {:?}",
            root.paint
        );
        for p in &frame.placements {
            assert!(
                p.rect.bottom() <= 200.0 + 1e-3,
                "{} runs to {} in a 200-unit window",
                p.id,
                p.rect.bottom()
            );
        }
    }

    #[test]
    fn sequence_numbers_start_at_one_and_never_repeat() {
        let mut counter = FrameCounter::new();
        assert_eq!(counter.peek(), 1);
        let seen: Vec<u64> = (0..5).map(|_| counter.take()).collect();
        assert_eq!(seen, [1, 2, 3, 4, 5]);
        assert_eq!(counter.peek(), 6);
    }

    /// A frame with nothing in it is not hosted, and asking does not panic.
    ///
    /// `petrify` always places a root, so this case is reachable only by
    /// building the frame directly — which the digest's own empty-frame
    /// reference vector already does. The accessor must answer it anyway: a
    /// consumer holds a `PetrifiedFrame`, not a promise about how it was made.
    #[test]
    fn a_frame_with_no_placements_is_not_hosted() {
        let viewport = Viewport::new(Size::new(1280.0, 800.0), ThemeMode::Dark);
        let frame = super::PetrifiedFrame {
            seq: 1,
            digest: super::digest::digest(&viewport, &[]),
            placements: Vec::new(),
            content: Vec::new(),
            subtree_hashes: Vec::new(),
            subtree_len: Vec::new(),
            slots: Vec::new(),
            viewport,
            transitions: TransitionActivity::default(),
        };
        assert!(!frame.hosted(), "there is nothing here to be hosted");
        assert_eq!(frame.hosted_placements().count(), 0);
    }

    /// Text and containers are not hosted: the digest hashes the string and
    /// the token names, so digest equality really is picture equality here.
    ///
    /// This is the half of the flag that has teeth. A flag stuck at `true`
    /// would tell every consumer of every ordinary frame to fall back to
    /// pixel comparison, and the FR-040 guarantee would be worth nothing.
    #[test]
    fn a_frame_of_text_and_containers_is_not_hosted() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(text("a", "hello"))
            .child(ViewNode::new(NodeKind::Spacer, "gap"))
            .child(text("b", "world"));
        let frame = frame_of(&tree, 200.0, 100.0);
        assert!(frame.placements.len() >= 4, "{:?}", frame.placements);
        assert!(
            !frame.hosted(),
            "text and containers are hashed by content, not by name"
        );
        assert_eq!(frame.hosted_placements().count(), 0);
        assert_eq!(
            frame.self_repainting_placements().count(),
            0,
            "nothing here draws itself either"
        );
    }

    /// The wider iterator never loses a hosted placement.
    ///
    /// `self_repainting_placements` is a superset of `hosted_placements` by
    /// construction ([`PaintContent::repaints_itself`]), and the ambient
    /// ledger moved onto it on that basis. A narrowing — a canvas term added
    /// to `is_hosted` and forgotten here, say — would take a surface out of
    /// the set FR-030 attributes against, which is exactly the hole
    /// `research.md` D-05 names.
    #[test]
    fn every_hosted_placement_is_also_self_repainting() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(text("caption", "a picture"))
            .child(ViewNode::new(NodeKind::Image, "pic").with_props(Props {
                image: Some("logo.png".to_string()),
                ..Props::default()
            }));
        let frame = frame_of(&tree, 200.0, 100.0);
        let hosted: Vec<&str> = frame
            .hosted_placements()
            .map(|(p, _)| p.id.as_str())
            .collect();
        let repainting: Vec<&str> = frame
            .self_repainting_placements()
            .map(|(p, _)| p.id.as_str())
            .collect();
        assert_eq!(hosted, ["/root/pic"], "the fixture must host something");
        for id in &hosted {
            assert!(
                repainting.contains(id),
                "{id} is hosted but not reported as able to repaint itself"
            );
        }
    }

    /// Nesting does not hide a hosted node: the flag is over every placement
    /// in the frame, not over the root or the top level.
    ///
    /// The image here sits inside a scroll, so it reaches the placement list
    /// through the scrolled path rather than the plain one.
    #[test]
    fn an_image_inside_a_scrolled_subtree_makes_the_whole_frame_hosted() {
        let tree = ViewNode::new(NodeKind::Stack, "root").child(
            ViewNode::new(NodeKind::Scroll, "sc").child(
                ViewNode::new(NodeKind::Stack, "inner")
                    .child(text("caption", "a picture"))
                    .child(ViewNode::new(NodeKind::Image, "pic").with_props(Props {
                        image: Some("logo.png".to_string()),
                        ..Props::default()
                    })),
            ),
        );
        let frame = frame_of(&tree, 200.0, 100.0);
        assert!(frame.hosted(), "an image three levels down is still hosted");
        let hosted: Vec<&str> = frame
            .hosted_placements()
            .map(|(p, _)| p.id.as_str())
            .collect();
        assert_eq!(
            hosted,
            ["/root/sc/inner/pic"],
            "only the image is hosted; its ancestors draw their own pixels"
        );
    }

    /// A hosted node clipped down to nothing still reports hosted.
    ///
    /// The flag reads the payload, never the geometry. That is deliberate and
    /// this pins it: the flag is an upper bound on where the digest is blind,
    /// and the safe direction is to over-report. Deciding it from the clip
    /// would make it depend on the renderer honouring that clip exactly, and
    /// on empty-rect arithmetic, to answer a question whose wrong answer is a
    /// consumer trusting a digest that cannot see the difference.
    #[test]
    fn a_hosted_node_clipped_away_still_reports_hosted() {
        let tall = Constraints {
            vertical: AxisConstraint {
                min: Some(400.0),
                ..AxisConstraint::default()
            },
            ..Constraints::default()
        };
        let tree = ViewNode::new(NodeKind::Scroll, "sc").child(
            ViewNode::new(NodeKind::Stack, "inner")
                .with_props(Props {
                    axis: Some(crate::geom::Axis::Vertical),
                    ..Props::default()
                })
                .child(ViewNode::new(NodeKind::Spacer, "push").with_constraints(tall))
                .child(ViewNode::new(NodeKind::Custom, "gauge").with_props(Props {
                    custom_kind: Some("gauge".to_string()),
                    ..Props::default()
                })),
        );
        let frame = frame_of_custom(&tree, "gauge", 200.0, 40.0);
        let gauge = frame
            .placement("/sc/inner/gauge")
            .expect("the custom node is placed even when the scroll clips it away");
        assert!(
            gauge.rect.w > 0.0 && gauge.rect.h > 0.0,
            "a zero-area gauge would pass the next assertion for the wrong \
             reason: rect {:?}",
            gauge.rect
        );
        let visible = gauge.rect.intersect(gauge.clip);
        assert!(
            visible.w == 0.0 || visible.h == 0.0,
            "this fixture must clip the gauge away entirely, or it proves \
             nothing: rect {:?}, clip {:?}, visible {visible:?}",
            gauge.rect,
            gauge.clip
        );
        assert!(
            frame.hosted(),
            "the flag reads the payload, so a clipped-away painter still counts"
        );
    }

    /// Ambient animations run forever by declaration, so a frame with only
    /// ambient motion is settled. The driver's settle wait depends on this.
    #[test]
    fn ambient_motion_does_not_block_settle() {
        assert!(
            TransitionActivity {
                running: 0,
                ambient: 3
            }
            .is_settled()
        );
        assert!(
            !TransitionActivity {
                running: 1,
                ambient: 0
            }
            .is_settled()
        );
    }
}
