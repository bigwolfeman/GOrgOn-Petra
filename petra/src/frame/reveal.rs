//! Scroll and focus requests: an application asking the host to show a
//! target, or to move focus onto it.
//!
//! The wheel, the scrollbar and focus stepping move a scroll because of input.
//! An application sometimes has to move one for a reason of its own: a
//! go-to-line in a code view, a search hit, a tree row that a different panel
//! selected. Without a request it could only rebuild the tree and hope, and a
//! virtualized list does not even place the row it would want to show.
//!
//! A [`ScrollRequest`] names what to show, by node id or by row of a
//! collection, and how to align it. [`PetrifiedFrame::reveal`] answers it
//! against a placed frame with the offset to write. The answer is a pure
//! function of the frame and the offsets it was placed with, so a host, a
//! driver and a test all resolve a request the same way.
//!
//! # Why a row is named by its row source
//!
//! An application knows the [`crate::layout::RowSource`] name it gave a list,
//! because it answers `rows` for that name every frame. It does not readily
//! know the key path the list was mounted at, which depends on every
//! container above it. The collection's placement carries the name
//! ([`crate::frame::PlacementSemantics::row_source`]), so the frame resolves
//! it. A placed row's span is its own placement: the collection names its
//! first materialized row ([`crate::frame::PlacementSemantics::first_row`]).
//! A row that is not placed has the span the host's
//! [`crate::layout::RowExtents`] gives it: measured extents for rows an
//! earlier frame placed, the estimate elsewhere. That is exact once the rows
//! before the target are measured, and the frame after the first scroll
//! measures them, so one correction lands the target exactly.
//!
//! # Focus
//!
//! A [`FocusTarget`] names a node the same two ways. A row is named by its
//! own key inside the collection, the key the application gave the row node
//! it returned from `rows`. Only a placed row can take focus, so a host pairs
//! a focus request with a [`ScrollRequest`] for a row off screen and lands the
//! focus once the scroll has placed it.

use std::collections::BTreeMap;

use crate::geom::{Axis, Rect};
use crate::layout::RowExtents;
use crate::tree::NodeKind;

use super::{PetrifiedFrame, Placement};

/// What a [`ScrollRequest`] asks to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevealTarget {
    /// The placement with this id.
    Node(String),
    /// The first placement, in paint order, whose own key (the last segment
    /// of its id, unescaped) is this. For an app that knows the key it gave
    /// a node but not the path the node was mounted at.
    Key(String),
    /// Row `index` of the collection that reads `source`. An index past the
    /// end shows the last row.
    Row {
        /// The collection's [`crate::layout::RowSource`] name.
        source: String,
        /// The row, from 0.
        index: usize,
    },
}

/// Where a revealed target lands in the viewport. `Nearest` and `Center`
/// leave a target that is already wholly in view where it is; `Start` and
/// `Upper` always land it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevealAlign {
    /// The least scroll that shows the whole target.
    #[default]
    Nearest,
    /// The target's middle on the viewport's middle: a jump the eye can find.
    Center,
    /// The target's leading edge on the viewport's leading edge, even when
    /// the target is already in view: a strip of pages that must start at a
    /// given page (SwiftUI's `scrollTo(id, anchor: .leading)`).
    Start,
    /// The target's leading edge a quarter of the way into the viewport,
    /// even when the target is already in view: a jump that keeps context
    /// before the target and room after it (a go-to-line, a search hit).
    /// A quarter rather than a third, so the target's whole extent sits in
    /// the first third at any viewport length.
    Upper,
}

/// The share of the viewport [`RevealAlign::Upper`] puts before its target.
const UPPER_SHARE: f32 = 0.25;

/// An application's request that a scroll show a target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollRequest {
    /// What to show.
    pub target: RevealTarget,
    /// Where it lands.
    pub align: RevealAlign,
}

impl ScrollRequest {
    /// Show row `index` of the collection reading `source`.
    #[must_use]
    pub fn row(source: impl Into<String>, index: usize, align: RevealAlign) -> Self {
        Self {
            target: RevealTarget::Row {
                source: source.into(),
                index,
            },
            align,
        }
    }

    /// Show the placement `id`.
    #[must_use]
    pub fn node(id: impl Into<String>, align: RevealAlign) -> Self {
        Self {
            target: RevealTarget::Node(id.into()),
            align,
        }
    }
}

/// An application's request that a scroll move by a distance, as a key press
/// on a focused scroll does. The host owns the offsets, so the application
/// names the scroll and the step, never the offset.
///
/// Unlike a [`ScrollRequest`] it names no content to show: a code view that
/// scrolls sideways on the arrow keys has no node at a column to reveal.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollBy {
    /// The scroll to move: the nearest `Scroll` at or above this target.
    pub target: RevealTarget,
    /// The step along the horizontal axis, in layout units. Positive shows
    /// content further right.
    pub dx: f32,
    /// The step along the vertical axis, in layout units. Positive shows
    /// content further down.
    pub dy: f32,
}

impl ScrollBy {
    /// Move the scroll at or above the node `id` by `dx` across and `dy`
    /// down.
    #[must_use]
    pub fn node(id: impl Into<String>, dx: f32, dy: f32) -> Self {
        Self {
            target: RevealTarget::Node(id.into()),
            dx,
            dy,
        }
    }
}

/// A node an application wants focused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FocusTarget {
    /// The placement with this id.
    Node(String),
    /// The first placement, in paint order, whose own key is this. For a
    /// node whose full path the application does not build, the same rule
    /// as [`RevealTarget::Key`].
    Key(String),
    /// The row keyed `key` of the collection that reads `source`.
    Row {
        /// The collection's [`crate::layout::RowSource`] name.
        source: String,
        /// The row node's own key.
        key: String,
    },
}

/// [`PetrifiedFrame::reveal`]'s answer.
#[derive(Clone, Debug, PartialEq)]
pub enum Reveal {
    /// Write `offset` into `scroll`'s offset entry. Already clamped to
    /// `[0, max_offset]`.
    Scroll {
        /// The scroll placement's id.
        scroll: String,
        /// The offset that shows the target.
        offset: f32,
        /// The most the scroll can move, for a host that re-clamps.
        max_offset: f32,
    },
    /// The target is wholly in view, or no scroll encloses it: nothing to
    /// write.
    InView,
    /// The frame does not place the target, or the collection has no rows
    /// yet. A host keeps the request and asks again on a later frame.
    Unplaced,
}

impl PetrifiedFrame {
    /// The id of the node `target` names, when this frame places it.
    #[must_use]
    pub fn focus_id(&self, target: &FocusTarget) -> Option<String> {
        match target {
            FocusTarget::Node(id) => self.placement(id).map(|p| p.id.clone()),
            FocusTarget::Key(key) => self.placement_keyed(key).map(|p| p.id.clone()),
            FocusTarget::Row { source, key } => {
                let collection = self.collection_of(source)?;
                let id = format!(
                    "{}/{}",
                    collection.id,
                    crate::tree::Key::new(key.as_str()).escaped()
                );
                self.placement(&id).map(|p| p.id.clone())
            }
        }
    }

    /// How many whole rows of the collection reading `source` its scroll
    /// shows at once: the page a PageUp or PageDown moves. `None` when the
    /// frame does not place the collection or it has no rows.
    #[must_use]
    pub fn rows_in_view(&self, source: &str) -> Option<usize> {
        let collection = self.collection_of(source)?;
        let count = collection.semantics.total_count.filter(|n| *n > 0)?;
        let scroll = self.scroll_ancestor(&collection.id)?;
        let axis = self.scroll_axis(&scroll.id);
        let viewport = self.scroll_content(&scroll.id)?.clip.size().along(axis);
        let pitch = collection.rect.size().along(axis) / count as f32;
        (pitch > 0.0).then(|| ((viewport / pitch).floor() as usize).max(1))
    }

    /// The collection placement that reads `source`.
    fn collection_of(&self, source: &str) -> Option<&Placement> {
        self.placements
            .iter()
            .find(|p| p.semantics.row_source.as_deref() == Some(source))
    }

    /// The nearest placement at or above `id` that is a `Scroll`.
    ///
    /// Walks `Placement::parent`, bounded by the placement count: `Placement`
    /// is public with no sealed constructor, so a malformed or cyclic
    /// `parent` chain is refused rather than trusted.
    #[must_use]
    pub fn scroll_ancestor(&self, id: &str) -> Option<&Placement> {
        let start = self.placements.iter().position(|p| p.id == id)?;
        self.scroll_at_or_above(Some(start))
    }

    fn scroll_at_or_above(&self, from: Option<usize>) -> Option<&Placement> {
        let mut cursor = from;
        let mut steps = 0usize;
        while let Some(i) = cursor {
            let placement = self.placements.get(i)?;
            if placement.kind == NodeKind::Scroll {
                return Some(placement);
            }
            cursor = placement.parent;
            steps += 1;
            if steps > self.placements.len() {
                return None;
            }
        }
        None
    }

    /// `scroll`'s own placed child: what its offset moves. A `Scroll` has at
    /// most one child (`layout::scroll::place` reads only the first).
    #[must_use]
    pub fn scroll_content(&self, scroll: &str) -> Option<&Placement> {
        let idx = self.placements.iter().position(|p| p.id == scroll)?;
        self.placements.iter().find(|p| p.parent == Some(idx))
    }

    /// The axis `scroll`'s content overflows on, read from geometry: the
    /// frame carries placements, not the `ScrollProps` that named the axis.
    /// `layout::scroll::place` places the child at its full content size and
    /// clips it to the viewport, so the axis whose extent passes its clip is
    /// the scrolling one. Content that overflows neither reads `Vertical`,
    /// which is inert: its overflow is 0, so every write clamps to 0.
    #[must_use]
    pub fn scroll_axis(&self, scroll: &str) -> Axis {
        // A two-axis scroll overflows both ways, so geometry cannot name its
        // main axis; the placement says it.
        if let Some(cross) = self
            .placement(scroll)
            .and_then(|placement| placement.semantics.cross_axis)
        {
            return cross.cross();
        }
        let Some(content) = self.scroll_content(scroll) else {
            return Axis::Vertical;
        };
        let overflow_h = content.rect.h - content.clip.h;
        let overflow_w = content.rect.w - content.clip.w;
        if overflow_h >= overflow_w {
            Axis::Vertical
        } else {
            Axis::Horizontal
        }
    }

    /// The offset `scroll` was placed with along its axis, read off the
    /// geometry: `layout::scroll::place` puts its content at the viewport's
    /// origin less the offset. `None` when the frame places no such scroll
    /// or it has no content.
    #[must_use]
    pub fn scroll_offset(&self, scroll: &str) -> Option<f32> {
        let axis = self.scroll_axis(scroll);
        let content = self.scroll_content(scroll)?;
        Some(leading(content.clip, axis) - leading(content.rect, axis))
    }

    /// How far `scroll`'s content can move along `axis` past its viewport;
    /// 0 when it names no placement, has no content, or fits.
    #[must_use]
    pub fn scroll_overflow(&self, scroll: &str, axis: Axis) -> f32 {
        let Some(content) = self.scroll_content(scroll) else {
            return 0.0;
        };
        (content.rect.size().along(axis) - content.clip.size().along(axis)).max(0.0)
    }

    /// The offset that answers `request`, given the `offsets` this frame was
    /// placed with and the row `extents` the host's layout has measured.
    #[must_use]
    pub fn reveal(
        &self,
        request: &ScrollRequest,
        offsets: &BTreeMap<String, f32>,
        extents: &RowExtents,
    ) -> Reveal {
        self.answer(request, offsets, extents, false)
    }

    /// The correction to an answer [`Self::reveal`] gave on an earlier
    /// frame, read off this frame's measured placements.
    ///
    /// A row the frame did not materialize is found from the ledger, which
    /// estimates every row no frame has measured. Once the scroll has
    /// brought the row on, this frame places it where it really is, and this
    /// answers the offset that lands it as the request asked. Rows keep the
    /// positions the ledger gives them whatever the offset, so that offset
    /// lands it exactly. A target the first scroll did not bring on is
    /// estimated again from the ledger, which now holds the rows that scroll
    /// measured. A host that scrolled for a request asks this on the frames
    /// that follow, writes any offset it answers, and stops at
    /// [`Reveal::InView`].
    ///
    /// One difference from [`Self::reveal`]: [`RevealAlign::Center`] lands
    /// even when the target is already in view, because the first answer
    /// moved it there by an estimate and the request asked for the middle.
    /// [`RevealAlign::Nearest`] still leaves a target that is wholly in view
    /// where it is.
    #[must_use]
    pub fn correct_reveal(
        &self,
        request: &ScrollRequest,
        offsets: &BTreeMap<String, f32>,
        extents: &RowExtents,
    ) -> Reveal {
        self.answer(request, offsets, extents, true)
    }

    /// [`Self::reveal`] and [`Self::correct_reveal`]: `measured` asks for
    /// the second.
    fn answer(
        &self,
        request: &ScrollRequest,
        offsets: &BTreeMap<String, f32>,
        extents: &RowExtents,
        measured: bool,
    ) -> Reveal {
        let Some(Span {
            scroll,
            lead,
            trail,
        }) = self.target_span(&request.target, extents)
        else {
            return Reveal::Unplaced;
        };
        let Some(scroll) = scroll else {
            return Reveal::InView;
        };
        let axis = self.scroll_axis(&scroll);
        let Some(viewport) = self.scroll_content(&scroll).map(|c| c.clip) else {
            return Reveal::InView;
        };
        let view_lead = leading(viewport, axis);
        let view_trail = view_lead + viewport.size().along(axis);
        let always_lands = matches!(request.align, RevealAlign::Start | RevealAlign::Upper)
            || (measured && request.align == RevealAlign::Center);
        if !always_lands && lead >= view_lead && trail <= view_trail {
            return Reveal::InView;
        }
        let max_offset = self.scroll_overflow(&scroll, axis);
        // The offset the frame was placed with: `layout::scroll::place`
        // re-clamps a stale entry and never writes it back. A correction
        // reads it off the frame itself, because every edge it measures is
        // relative to that placement, and a host may have written a newer
        // entry since this frame was placed.
        let current = match self.scroll_offset(&scroll) {
            Some(placed) if measured => placed,
            _ => offsets.get(&scroll).copied().unwrap_or(0.0),
        }
        .clamp(0.0, max_offset);
        let shift = match request.align {
            RevealAlign::Nearest if lead < view_lead || trail - lead > view_trail - view_lead => {
                lead - view_lead
            }
            RevealAlign::Nearest => trail - view_trail,
            RevealAlign::Center => (lead + trail) / 2.0 - (view_lead + view_trail) / 2.0,
            RevealAlign::Start => lead - view_lead,
            RevealAlign::Upper => lead - view_lead - (view_trail - view_lead) * UPPER_SHARE,
        };
        let offset = (current + shift).clamp(0.0, max_offset);
        // `Start` and `Upper` skip the in-view test above, so an aligned
        // target is in view here, not a write of the offset it already has:
        // either it sits within half a unit of where it was asked to, or the
        // scroll is at an end and cannot move it closer.
        let pinned = offset <= 0.0 || offset >= max_offset;
        if always_lands && (shift.abs() < 0.5 || (pinned && (offset - current).abs() < 0.5)) {
            return Reveal::InView;
        }
        Reveal::Scroll {
            scroll,
            offset,
            max_offset,
        }
    }

    /// Where `target` is along its scroll's axis ([`Span`]). `None` when the
    /// frame cannot place the target.
    fn target_span(&self, target: &RevealTarget, extents: &RowExtents) -> Option<Span> {
        match target {
            RevealTarget::Node(_) | RevealTarget::Key(_) => {
                let placement = match target {
                    RevealTarget::Node(id) => self.placements.iter().find(|p| &p.id == id)?,
                    RevealTarget::Key(key) => self.placement_keyed(key)?,
                    RevealTarget::Row { .. } => return None,
                };
                // A scroll does not reveal itself inside itself.
                let scroll = self
                    .scroll_at_or_above(placement.parent)
                    .map(|p| p.id.clone());
                let axis = scroll
                    .as_deref()
                    .map_or(Axis::Vertical, |s| self.scroll_axis(s));
                let lead = leading(placement.rect, axis);
                Some(Span {
                    scroll,
                    lead,
                    trail: lead + placement.rect.size().along(axis),
                })
            }
            RevealTarget::Row { source, index } => {
                let at = self
                    .placements
                    .iter()
                    .position(|p| p.semantics.row_source.as_deref() == Some(source.as_str()))?;
                let collection = &self.placements[at];
                let count = collection.semantics.total_count.filter(|n| *n > 0)?;
                let scroll = self.scroll_ancestor(&collection.id).map(|p| p.id.clone());
                let axis = scroll
                    .as_deref()
                    .map_or(Axis::Vertical, |s| self.scroll_axis(s));
                let index = (*index).min(count - 1);
                if let Some(row) = self.placed_row(at, index) {
                    let lead = leading(row.rect, axis);
                    return Some(Span {
                        scroll,
                        lead,
                        trail: lead + row.rect.size().along(axis),
                    });
                }
                let list = extents.list(&collection.id);
                let (before, extent) = match list.filter(|l| l.count() == count) {
                    Some(list) => (list.prefix(index), list.extent(index)),
                    // Never placed: the uniform grid is all there is.
                    None => {
                        let pitch = collection.rect.size().along(axis) / count as f32;
                        (index as f32 * pitch, pitch)
                    }
                };
                let lead = leading(collection.rect, axis) + before;
                Some(Span {
                    scroll,
                    lead,
                    trail: lead + extent,
                })
            }
        }
    }

    /// Row `index` of the collection placed at `at`, when this frame
    /// materialized it: the k-th row child is row
    /// [`crate::frame::PlacementSemantics::first_row`] + k.
    fn placed_row(&self, at: usize, index: usize) -> Option<&Placement> {
        let first = self.placements[at].semantics.first_row?;
        self.placements
            .iter()
            .filter(|p| p.parent == Some(at))
            .nth(index.checked_sub(first)?)
    }
}

impl PetrifiedFrame {
    /// The id of the `Scroll` a [`ScrollBy`] to `target` moves: the target
    /// itself when it is one, else its nearest enclosing one. `None` when the
    /// frame does not place the target or no scroll encloses it.
    #[must_use]
    pub fn scroll_id(&self, target: &RevealTarget) -> Option<String> {
        let placement = match target {
            RevealTarget::Node(id) => self.placement(id)?,
            RevealTarget::Key(key) => self.placement_keyed(key)?,
            RevealTarget::Row { source, .. } => self.collection_of(source)?,
        };
        self.scroll_ancestor(&placement.id).map(|p| p.id.clone())
    }

    /// The first placement whose own key is the one `target` names.
    fn placement_keyed(&self, key: &str) -> Option<&crate::frame::Placement> {
        let escaped = crate::tree::Key::from(key).escaped();
        let tail = format!("/{escaped}");
        // The suffix test is cheap; the parse rules out a `/` that was an
        // escaped part of a longer key.
        self.placements.iter().find(|p| {
            p.id.ends_with(&tail)
                && crate::tree::KeyPath::parse(&p.id)
                    .and_then(|path| path.segments().last().map(|k| k.as_str() == key))
                    .unwrap_or(false)
        })
    }
}

/// Where a reveal target is along its scroll's axis.
struct Span {
    /// The scroll enclosing the target, or `None` when no scroll does.
    scroll: Option<String>,
    /// The target's leading edge.
    lead: f32,
    /// The target's trailing edge.
    trail: f32,
}

/// A rect's origin along `axis`.
fn leading(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.x,
        Axis::Vertical => rect.y,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Reveal, RevealAlign, RevealTarget, ScrollRequest};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::Size;
    use crate::layout::RowExtents;
    use crate::testing::{GeneratedRows, Harness, MonoContent, validated};
    use crate::token::ThemeMode;
    use crate::tree::{NodeKind, Props, ViewNode};

    const SOURCE: &str = "lines";
    const PITCH: f32 = 20.0;
    const VIEW: f32 = 200.0;

    /// A 200-tall scroll over a 1000-row collection of 20-tall rows.
    fn list() -> ViewNode {
        let rows = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(1000),
            source: Some(SOURCE.into()),
            estimated_extent: Some(PITCH),
            ..Props::default()
        });
        ViewNode::new(NodeKind::Scroll, "list").child(rows)
    }

    fn frame(offset: f32) -> PetrifiedFrame {
        let mut harness = Harness::with(
            MonoContent::default(),
            GeneratedRows::new(SOURCE, 1000).each_row_tall(PITCH),
        );
        harness.set_scroll("/list", offset);
        let tree = list();
        petrify(
            1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(300.0, VIEW), ThemeMode::Dark),
            TransitionActivity::default(),
        )
    }

    fn offsets(offset: f32) -> BTreeMap<String, f32> {
        BTreeMap::from([("/list".to_owned(), offset)])
    }

    /// No ledger: an unplaced row of a list no frame has measured is on the
    /// uniform grid.
    fn none() -> RowExtents {
        RowExtents::new()
    }

    /// A row far below the window: centred, it lands mid-viewport; nearest,
    /// its bottom edge meets the viewport's bottom edge. Neither row 500 nor
    /// its neighbours are materialized, so the span comes from the pitch.
    #[test]
    fn an_unmaterialized_row_is_revealed_by_its_pitch() {
        let frame = frame(0.0);
        assert!(frame.placement("/list/rows/row-500").is_none());
        let center = frame.reveal(
            &ScrollRequest::row(SOURCE, 500, RevealAlign::Center),
            &offsets(0.0),
            &none(),
        );
        assert_eq!(
            center,
            Reveal::Scroll {
                scroll: "/list".into(),
                offset: 500.0 * PITCH + PITCH / 2.0 - VIEW / 2.0,
                max_offset: 1000.0 * PITCH - VIEW,
            }
        );
        let nearest = frame.reveal(
            &ScrollRequest::row(SOURCE, 500, RevealAlign::Nearest),
            &offsets(0.0),
            &none(),
        );
        assert!(matches!(
            nearest,
            Reveal::Scroll { offset, .. } if offset == 501.0 * PITCH - VIEW
        ));
    }

    /// `Start` lands a row in view at the top, and a row already at the top
    /// is in view. A `Key` target finds row 5 by its own key.
    #[test]
    fn start_lands_the_target_at_the_leading_edge_even_in_view() {
        use super::RevealTarget;
        let frame = frame(0.0);
        let start = |target: RevealTarget, at: f32| {
            frame.reveal(
                &ScrollRequest {
                    target,
                    align: RevealAlign::Start,
                },
                &offsets(at),
                &none(),
            )
        };
        assert!(matches!(
            start(RevealTarget::Key("row-5".into()), 0.0),
            Reveal::Scroll { offset, .. } if offset == 5.0 * PITCH
        ));
        assert_eq!(
            start(RevealTarget::Key("row-0".into()), 0.0),
            Reveal::InView
        );
        assert_eq!(
            start(RevealTarget::Key("row-none".into()), 0.0),
            Reveal::Unplaced
        );
    }

    /// `Upper` puts the row a quarter of the way down the viewport, from
    /// far below the window and from inside it alike; a row too near the top
    /// to have a quarter above it clamps to offset 0 and is in view.
    #[test]
    fn upper_lands_the_row_a_quarter_of_the_way_down() {
        let frame = frame(0.0);
        let upper = |index: usize| {
            frame.reveal(
                &ScrollRequest::row(SOURCE, index, RevealAlign::Upper),
                &offsets(0.0),
                &none(),
            )
        };
        let quarter = VIEW * 0.25;
        assert_eq!(
            upper(500),
            Reveal::Scroll {
                scroll: "/list".to_owned(),
                offset: 500.0 * PITCH - quarter,
                max_offset: 1000.0 * PITCH - VIEW,
            }
        );
        // Row 8 is in view at the bottom; it still moves up.
        assert!(matches!(
            upper(8),
            Reveal::Scroll { offset, .. } if offset == 8.0 * PITCH - quarter
        ));
        assert_eq!(upper(1), Reveal::InView);
    }

    /// Rows that alternate 8 and 56 tall under a 20-unit estimate: every
    /// row measures something the estimate does not say.
    struct Mixed;

    fn mixed_height(index: usize) -> f32 {
        if index.is_multiple_of(2) { 8.0 } else { 56.0 }
    }

    impl crate::layout::RowSource for Mixed {
        fn rows(
            &mut self,
            _source: &str,
            range: std::ops::Range<usize>,
        ) -> Vec<std::sync::Arc<ViewNode>> {
            range
                .filter(|i| *i < 1000)
                .map(|i| std::sync::Arc::new(GeneratedRows::row_of_height(i, mixed_height(i))))
                .collect()
        }
    }

    /// A host's view of the mixed list: one harness, so the ledger
    /// remembers what earlier frames measured, and every anchor shift
    /// applied as the host applies it.
    struct MixedRig {
        harness: Harness<MonoContent, Mixed>,
        seq: u64,
    }

    impl MixedRig {
        fn new() -> Self {
            Self {
                harness: Harness::with(MonoContent::default(), Mixed),
                seq: 0,
            }
        }

        fn offset(&self) -> f32 {
            self.harness.state.scroll_offset("/list")
        }

        /// Petrify at `offset`, then again for as long as the frame moved
        /// the ledger's anchor: the frame a host shows once it is idle.
        fn frame_at(&mut self, offset: f32) -> PetrifiedFrame {
            self.harness.set_scroll("/list", offset);
            loop {
                self.seq += 1;
                let frame = petrify(
                    self.seq,
                    validated(&list()),
                    &mut self.harness.ctx(),
                    Viewport::new(Size::new(300.0, VIEW), ThemeMode::Dark),
                    TransitionActivity::default(),
                );
                let before = self.offset();
                self.harness.apply_shifts();
                if self.offset() == before {
                    return frame;
                }
            }
        }
    }

    fn mixed_frame(offset: f32) -> PetrifiedFrame {
        MixedRig::new().frame_at(offset)
    }

    /// What a host does with a request: answer it against the ledger, then
    /// correct it on each frame the scroll places until it is in view. The
    /// frame the last offset placed, the last answer, and how many
    /// corrections wrote an offset.
    fn land(request: &ScrollRequest) -> (PetrifiedFrame, Reveal, usize) {
        let mut rig = MixedRig::new();
        let mut frame = rig.frame_at(0.0);
        let mut answer = frame.reveal(request, &offsets(0.0), &rig.harness.extents);
        let mut corrections = 0;
        for _ in 0..=4 {
            let Reveal::Scroll { offset: next, .. } = answer else {
                break;
            };
            frame = rig.frame_at(next);
            answer = frame.correct_reveal(request, &offsets(rig.offset()), &rig.harness.extents);
            if matches!(answer, Reveal::Scroll { .. }) {
                corrections += 1;
            }
        }
        (frame, answer, corrections)
    }

    /// A row of a list whose rows differ in height lands exactly where the
    /// request asked, for every alignment and every row of a sixty-row
    /// sweep, after at most two corrections.
    ///
    /// The first answer estimates the rows the ledger has not measured; the
    /// frame that scroll places measures the rows around the target, and a
    /// correction from a placed target lands it, because a row keeps the
    /// position the ledger gives it whatever the offset. When the estimate
    /// put the target past the first window, one correction from the
    /// updated ledger brings it on first. Before the ledger the window's
    /// first row sat on the estimate's grid, so moving the offset moved
    /// every row, and no offset centred row 300 of this list at all.
    #[test]
    fn a_row_of_mixed_heights_lands_on_its_measured_place() {
        for index in (300..360).chain([501]) {
            for align in [
                RevealAlign::Center,
                RevealAlign::Upper,
                RevealAlign::Start,
                RevealAlign::Nearest,
            ] {
                let (frame, answer, corrections) = land(&ScrollRequest::row(SOURCE, index, align));
                assert_eq!(answer, Reveal::InView, "{align:?} {index}: did not settle");
                assert!(
                    corrections <= 2,
                    "{align:?} {index}: {corrections} corrections; one lands a target the \
                     first scroll placed, a second one it did not"
                );
                let row = frame
                    .placement(&format!("/list/rows/row-{index}"))
                    .unwrap_or_else(|| panic!("{align:?}: row {index} is not placed"))
                    .rect;
                assert_eq!(row.h, mixed_height(index));
                // A scroll places its content at `-offset`, so a row's rect
                // is already in the viewport's frame, whose top is 0.
                let want = match align {
                    RevealAlign::Center => VIEW / 2.0 - row.h / 2.0,
                    RevealAlign::Upper => VIEW * 0.25,
                    RevealAlign::Start => 0.0,
                    // From above: the least scroll puts its bottom on the
                    // viewport's bottom.
                    RevealAlign::Nearest => VIEW - row.h,
                };
                assert!(
                    (row.y - want).abs() < 0.5,
                    "{align:?}: row {index} at {} in the viewport, asked for {want}",
                    row.y
                );
            }
        }
        // The control: the estimate alone leaves row 501 well off, so the
        // landing above is the correction's doing. Centred, because `Start`
        // makes the target the first visible row, which the ledger places
        // where it says and so lands even from an estimate.
        let request = ScrollRequest::row(SOURCE, 501, RevealAlign::Center);
        let Reveal::Scroll { offset, .. } =
            mixed_frame(0.0).reveal(&request, &offsets(0.0), &none())
        else {
            panic!("row 501 is below the fold and must scroll");
        };
        let want = VIEW / 2.0 - mixed_height(501) / 2.0;
        let first = MixedRig::new()
            .frame_at(offset)
            .placement("/list/rows/row-501")
            .map(|p| p.rect.y);
        assert!(
            first.is_none_or(|y| (y - want).abs() > 10.0),
            "the estimate alone landed row 501 at {first:?}, so this proves nothing"
        );
    }

    /// A materialized row answers from its own placement, so an estimate
    /// that is wrong for the rows in view does not move a row that is
    /// already wholly visible.
    #[test]
    fn a_placed_row_answers_from_its_measured_rect() {
        let frame = mixed_frame(0.0);
        // Rows 0..=6 measure 8 + 56 + 8 + 56 + 8 + 56 + 8 = 200, so row 7
        // is placed at 200..256, below the 200-tall view. Its estimate,
        // 140..160, would call it in view.
        let nearest = frame.reveal(
            &ScrollRequest::row(SOURCE, 7, RevealAlign::Nearest),
            &offsets(0.0),
            &none(),
        );
        assert!(
            matches!(nearest, Reveal::Scroll { offset, .. } if offset == 256.0 - VIEW),
            "{nearest:?}"
        );
    }

    /// A row above the window aligns its top edge with the viewport's top.
    #[test]
    fn a_row_above_the_window_meets_its_top_edge() {
        let at = 400.0 * PITCH;
        let frame = frame(at);
        let nearest = frame.reveal(
            &ScrollRequest::row(SOURCE, 100, RevealAlign::Nearest),
            &offsets(at),
            &none(),
        );
        assert!(matches!(
            nearest,
            Reveal::Scroll { offset, .. } if offset == 100.0 * PITCH
        ));
    }

    /// A row in view stays where it is, under either alignment.
    #[test]
    fn a_row_in_view_does_not_move() {
        let frame = frame(0.0);
        for align in [RevealAlign::Nearest, RevealAlign::Center] {
            assert_eq!(
                frame.reveal(
                    &ScrollRequest::row(SOURCE, 3, align),
                    &offsets(0.0),
                    &none()
                ),
                Reveal::InView
            );
        }
    }

    /// An index past the end shows the last row; the offset clamps to the
    /// scroll's range.
    #[test]
    fn an_index_past_the_end_shows_the_last_row() {
        let frame = frame(0.0);
        assert!(matches!(
            frame.reveal(
                &ScrollRequest::row(SOURCE, 5000, RevealAlign::Center),
                &offsets(0.0)
            , &none()),
            Reveal::Scroll { offset, max_offset, .. } if offset == max_offset
        ));
    }

    /// A source the frame does not place, and a node id it does not place,
    /// are unplaced: the host keeps the request for a later frame.
    #[test]
    fn an_unknown_target_is_unplaced() {
        let frame = frame(0.0);
        assert_eq!(
            frame.reveal(
                &ScrollRequest::row("other", 1, RevealAlign::Nearest),
                &offsets(0.0),
                &none()
            ),
            Reveal::Unplaced
        );
        assert_eq!(
            frame.reveal(
                &ScrollRequest::node("/list/nope", RevealAlign::Nearest),
                &offsets(0.0),
                &none()
            ),
            Reveal::Unplaced
        );
    }

    /// A placed row is found by its own key; an unplaced one is not.
    #[test]
    fn a_focus_target_names_a_placed_row_by_its_key() {
        use super::FocusTarget;
        let frame = frame(0.0);
        let row = |key: &str| FocusTarget::Row {
            source: SOURCE.into(),
            key: key.into(),
        };
        assert_eq!(
            frame.focus_id(&row("row-3")).as_deref(),
            Some("/list/rows/row-3")
        );
        assert_eq!(frame.focus_id(&row("row-900")), None, "not placed");
        assert_eq!(
            frame
                .focus_id(&FocusTarget::Node("/list".into()))
                .as_deref(),
            Some("/list")
        );
        assert_eq!(
            frame.focus_id(&FocusTarget::Key("list".into())).as_deref(),
            Some("/list"),
            "a key names the placement without its path"
        );
        assert_eq!(frame.focus_id(&FocusTarget::Key("nope".into())), None);
    }

    /// A 200-tall scroll over 20-tall rows shows ten.
    #[test]
    fn rows_in_view_is_the_viewport_over_the_pitch() {
        assert_eq!(frame(0.0).rows_in_view(SOURCE), Some(10));
        assert_eq!(frame(0.0).rows_in_view("other"), None);
    }

    /// A placed node below the window is revealed by its own rect.
    #[test]
    fn a_placed_node_is_revealed_by_its_rect() {
        let frame = frame(0.0);
        let id = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/row-12"))
            .map(|p| p.id.clone())
            .expect("row 12 is in the overscan window");
        assert!(matches!(
            frame.reveal(&ScrollRequest::node(id, RevealAlign::Nearest), &offsets(0.0), &none()),
            Reveal::Scroll { offset, .. } if offset == 13.0 * PITCH - VIEW
        ));
    }

    /// A scroll is found from itself, from a node inside it, from a key and
    /// from a row source, and a node outside every scroll finds none.
    #[test]
    fn a_scroll_is_found_from_any_target_inside_it() {
        let frame = frame(0.0);
        let found = |target: RevealTarget| frame.scroll_id(&target);
        assert_eq!(
            found(RevealTarget::Node("/list".into())).as_deref(),
            Some("/list")
        );
        assert_eq!(
            found(RevealTarget::Node("/list/rows".into())).as_deref(),
            Some("/list")
        );
        assert_eq!(
            found(RevealTarget::Key("list".into())).as_deref(),
            Some("/list")
        );
        assert_eq!(
            found(RevealTarget::Row {
                source: SOURCE.into(),
                index: 3
            })
            .as_deref(),
            Some("/list")
        );
        assert_eq!(found(RevealTarget::Node("/nowhere".into())), None);
    }
}
