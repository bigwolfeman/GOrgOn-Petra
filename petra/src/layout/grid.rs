//! The grid container: fixed, weighted, and fit-content track sizing.
//!
//! `contracts/view-tree.md` binds one sentence: "rows/columns resolved by the
//! same probe vocabulary (min/ideal/max per track), with declared track
//! sizing (fixed, flexible-weight, fit-content)." Columns resolve first,
//! against the container's own horizontal proposal; rows resolve second,
//! because a `FitContent` row's height depends on the column widths its cells
//! already have (`contracts/view-tree.md`'s grid rule, spelled out per-track
//! in the T017 task brief). [`distribute_tracks`] is the one place both axes
//! go through, so the closed-budget and open-probe cases are written once.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Align, Axis, Rect, Size};
use crate::layout::constraints::FIT_EPSILON;
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::props::{GridSpan, max_row_tracks};
use crate::tree::{KeyPath, NodeKind, TrackSize, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let ncols = node.props.columns.len();
    if ncols == 0 {
        // Tree acceptance is expected to refuse a grid with no declared
        // columns before layout ever sees it; this is the defensive floor
        // for a container called out of turn, not a shape the contract
        // describes.
        return Size::ZERO;
    }
    // Padding is reserved before track sizing, the same way `column_spacing`/
    // `row_spacing` already are: a `Weight` track must never see budget the
    // padding already spent. Added back onto the reported size afterward, so
    // the grid's own measured extent already accounts for its padding —
    // exactly parallel to how it already accounts for spacing.
    let padding = ctx.padding(&node.props.padding);
    let column_spacing = ctx.spacing(&node.props.column_spacing);
    let row_spacing = ctx.spacing(&node.props.row_spacing);
    let flow = Flow::seat(&node.children, ncols, node.props.rows.len());
    let row_tracks = effective_row_tracks(&node.props.rows, &flow);
    let col_widths = resolve_columns(
        node,
        ctx,
        path,
        &node.props.columns,
        column_spacing,
        proposal.horizontal.shrink(padding.along(Axis::Horizontal)),
        &flow,
    );
    let row_heights = resolve_rows(
        node,
        ctx,
        path,
        &row_tracks,
        proposal.vertical.shrink(padding.along(Axis::Vertical)),
        RowContext {
            col_widths: &col_widths.extents,
            col_spacing: col_widths.spacing,
            row_spacing,
            flow: &flow,
        },
    );
    let w = col_widths.extents.iter().sum::<f32>()
        + reserved(col_widths.spacing, ncols)
        + padding.along(Axis::Horizontal);
    let h = row_heights.extents.iter().sum::<f32>()
        + reserved(row_heights.spacing, row_tracks.len())
        + padding.along(Axis::Vertical);
    Size::new(w, h)
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let ncols = node.props.columns.len();
    let align = node.props.align.unwrap_or_default();
    // Padding insets what the grid offers its children; the grid's own
    // placed rect below stays `slot.rect`, unmodified by its own padding
    // (padding is inside the box, not around it).
    let padding = ctx.padding(&node.props.padding);
    let column_spacing = ctx.spacing(&node.props.column_spacing);
    let row_spacing = ctx.spacing(&node.props.row_spacing);
    let content = slot.rect.inset_edges(padding);

    // Resolve the tracks before pushing this container's own placement. The
    // dispatcher requires a container to push itself before any child, and
    // resolving pushes nothing — but the resolution is what decides whether
    // this grid had to truncate, and `PaintState::truncated` is written once,
    // at push time. Resolving first is how that flag can carry a true value;
    // it used to be the literal `false`, so a grid that overflowed its offer
    // reported a clean frame.
    let mut plan = None;
    let mut truncated = false;
    if ncols > 0 && !node.children.is_empty() {
        // Re-resolve against the rect this call actually received, the same
        // way `text::place` re-measures rather than trusting an earlier
        // probe: a parent may have probed several proposals before
        // committing this one. `content`, not `slot.rect`: track sizing must
        // never see the budget the padding already spent.
        let offer = SizeProposal::exact(content.size());
        let flow = Flow::seat(&node.children, ncols, node.props.rows.len());
        let row_tracks = effective_row_tracks(&node.props.rows, &flow);
        let col_widths = resolve_columns(
            node,
            ctx,
            path,
            &node.props.columns,
            column_spacing,
            offer.horizontal,
            &flow,
        );
        let row_heights = resolve_rows(
            node,
            ctx,
            path,
            &row_tracks,
            offer.vertical,
            RowContext {
                col_widths: &col_widths.extents,
                col_spacing: col_widths.spacing,
                row_spacing,
                flow: &flow,
            },
        );
        truncated = col_widths.truncated || row_heights.truncated;

        // Absolute coordinates, seeded from the inset content rect, not the
        // grid's own outer rect: see `cumulative_offsets`.
        //
        // The far edges come from `content` for the same reason the origins
        // do. A run that ends the axis has no neighbour to take its far edge
        // from, and where the tracks partition the box (`Tracks::fills`) the
        // box's own edge is that far edge — the identical float the grid's
        // last row must land on. `content`, not `slot.rect`: with padding the
        // tracks end at the padding, and the grid's rect does not.
        let cells = Cells {
            col_x: cumulative_offsets(content.x, &col_widths.extents, col_widths.spacing),
            row_y: cumulative_offsets(content.y, &row_heights.extents, row_heights.spacing),
            col_far: col_widths.fills.then(|| content.right()),
            row_far: row_heights.fills.then(|| content.bottom()),
            col_w: col_widths.extents,
            row_h: row_heights.extents,
            col_spacing: col_widths.spacing,
            row_spacing: row_heights.spacing,
            flow,
        };

        // A child that answers larger than its cell is clamped into it by
        // `place_in_cell`. Ask now, so the container can report the clamp;
        // the answer is memoized (for the non-Stretch arm), so
        // `place_in_cell` re-reads it rather than re-negotiating.
        //
        // The two arms use different evidence because they place
        // differently. Non-Stretch alignments *measure* the child (they ask
        // what it wants and may get back more than the cell) and clamp the
        // response down; the loss is the gap between what was asked for and
        // what fits. Stretch never measures — `place_in_cell` imposes the
        // cell outright and never asks the child's natural size — so a
        // `measure` call here would cost a cache slot to answer a question
        // Stretch itself never asks. The loss for Stretch is visible from
        // the declared constraint alone: `AxisConstraint::clamp` returns
        // more than the cell exactly when a declared minimum exceeds it
        // (min wins over max), which is the one way a Stretch child can
        // still lose room to `place_in_cell`'s `.min(cell.w)` / `.min(cell.h)`
        // floor.
        for (i, child) in node.children.iter().enumerate() {
            let Some(cell) = cells.of(i) else {
                continue;
            };
            let cell = cell.size();
            if child.kind == NodeKind::Surface {
                // A surface never takes the cell: it floats against the
                // window (`overlay_surface`), and its constraints bound that
                // floating box. A scrim asking for more than any window
                // would otherwise report this grid as truncated.
                continue;
            }
            // `align_self` overrides the container's own `align` for this
            // one child, exactly as `layout::stack` reads it. Read here as
            // well as at `place_in_cell` because the two arms below gather
            // *different evidence*, and a child placed by one rule while
            // measured by the other would report a truncation it does not
            // have — or hide one it does.
            if child.props.align_self.unwrap_or(align) == Align::Stretch {
                if child.constraints.horizontal.clamp(cell.w) > cell.w + FIT_EPSILON
                    || child.constraints.vertical.clamp(cell.h) > cell.h + FIT_EPSILON
                {
                    truncated = true;
                }
            } else {
                path.push(child.key.clone());
                let response = crate::layout::measure(child, ctx, path, SizeProposal::exact(cell));
                path.pop();
                if response.w > cell.w + FIT_EPSILON || response.h > cell.h + FIT_EPSILON {
                    truncated = true;
                }
            }
        }
        plan = Some(cells);
    }

    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    let me = sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated,
            // A grid places its cells; it never re-measures a run against the
            // rect it handed out. Vertical overflow is decided by the leaf
            // that owns the content (`layout::text::place`), so a container
            // that claimed it here would be guessing.
            overflowed: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
    sink.enter(me);

    if let Some(cells) = plan {
        for (i, child) in node.children.iter().enumerate() {
            // `effective_row_tracks` always grows to fit every child, and tree
            // acceptance refuses a span that runs past the last track, so
            // `Cells::of` is in range today; the `None` arm is the floor
            // against a future change to either invariant, not a case this can
            // reach now.
            let Some(cell) = cells.of(i) else {
                continue;
            };
            place_in_cell(
                child,
                ctx,
                path,
                cell,
                child.props.align_self.unwrap_or(align),
                slot,
                sink,
            );
        }
    }

    sink.leave();
}

/// Reserved spacing for `n` tracks: `n - 1` gaps, never negative.
fn reserved(spacing: f32, n: usize) -> f32 {
    spacing.max(0.0) * n.saturating_sub(1) as f32
}

/// The resolved cell geometry of one grid, everything [`place`] needs to turn
/// a child index into the rect that child occupies.
///
/// Kept as one value rather than four parallel `Vec`s in a tuple because the
/// truncation probe and the placement loop must agree on every cell exactly —
/// they used to derive it twice from `i % ncols` and `i / ncols`, which was
/// harmless while a cell was one track wide and is not once a span can make it
/// wider. [`Cells::of`] is now the single answer both read.
struct Cells {
    /// Leading absolute x of each column.
    col_x: Vec<f32>,
    /// Leading absolute y of each row.
    row_y: Vec<f32>,
    /// Resolved column extents.
    col_w: Vec<f32>,
    /// Resolved row extents.
    row_h: Vec<f32>,
    /// Column gap actually used, which is not always the declared one — see
    /// [`Tracks::spacing`].
    col_spacing: f32,
    /// Row gap actually used.
    row_spacing: f32,
    /// The x the columns end on when they partition the content box, and
    /// `None` when they stop short of it. See [`seam_extent`] and
    /// [`Tracks::fills`].
    col_far: Option<f32>,
    /// The y the rows end on, on the same terms.
    row_far: Option<f32>,
    /// Where each child sits.
    flow: Flow,
}

impl Cells {
    /// The rect child `i` occupies, or `None` when its leading track is out of
    /// range.
    ///
    /// The extent is the child's whole track run — the tracks it spans plus
    /// the gaps between them — so a child spanning three columns is offered,
    /// and clipped to, all three of them and the two gaps, not just its own.
    /// [`seam_extent`] is what turns that into a float, and it takes the run's
    /// far edge from whatever sits on the other side of it: the neighbour's
    /// own offset, or the axis end for a run that has no neighbour.
    fn of(&self, i: usize) -> Option<Rect> {
        let run = self.flow.runs.get(i)?;
        let (&x, &y) = (self.col_x.get(run.col)?, self.row_y.get(run.row)?);
        Some(Rect::new(
            x,
            y,
            seam_extent(
                &self.col_x,
                &self.col_w,
                self.col_far,
                self.col_spacing,
                run.col,
                run.ncols,
            ),
            seam_extent(
                &self.row_y,
                &self.row_h,
                self.row_far,
                self.row_spacing,
                run.row,
                run.nrows,
            ),
        ))
    }
}

/// The block of cells one child occupies: its leading cell and the track
/// counts it covers on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellRun {
    /// Leading column.
    col: usize,
    /// Leading row.
    row: usize,
    /// Column tracks covered, at least one.
    ncols: usize,
    /// Row tracks covered, at least one.
    nrows: usize,
}

impl CellRun {
    /// The leading track and the count on `axis`.
    fn on(self, axis: Axis) -> (usize, usize) {
        match axis {
            Axis::Horizontal => (self.col, self.ncols),
            Axis::Vertical => (self.row, self.nrows),
        }
    }
}

/// Where every child of one grid sits.
///
/// Children are seated in declaration order by a cursor that walks the cells
/// row-major, takes the run its span asks for, and moves past it. Two rules
/// keep the seating honest, and both exist because a span is a count of cells
/// and cells are a finite resource:
///
/// * A run that does not fit in what is left of the current row starts the
///   next row instead. A three-column child cannot begin in the last column of
///   a four-column grid and finish in the next row: the contract says the
///   tracks it covers are **contiguous**, and a wrapped run is two pieces.
/// * A run never lands on a cell an earlier child already took. A row-spanning
///   child reaches down into rows the cursor has not reached yet, so the
///   claims have to be remembered rather than inferred from the cursor
///   position.
///
/// This is what makes the span usable rather than decorative. Seating child
/// `i` at the bare `(i % ncols, i / ncols)` — its index, ignoring every span
/// declared before it — puts the child after a two-column span directly on top
/// of that span's second cell, with no arrangement of the tree able to avoid
/// it, because the index *is* the column. `no_two_children_ever_share_a_cell`
/// is the property that pins the rule this replaces it with.
///
/// The cursor only ever moves forward, so seating every child costs one pass
/// over the cells, and a hole an earlier wrap left behind stays a hole. That
/// is a choice, not an oversight: backfilling would make a child's seat depend
/// on children declared after it, and FR-006's digest is easier to trust when
/// reading order and seating order agree.
///
/// A grid with no declared span produces exactly the seating the grid had
/// before FR-061: every run is one cell, the cursor advances one cell per
/// child and never skips, so child `i` lands on `(i % ncols, i / ncols)`.
#[derive(Clone, Debug, Default)]
struct Flow {
    /// One run per child, in declaration order.
    runs: Vec<CellRun>,
    /// How many rows the seating consumed.
    rows: usize,
}

impl Flow {
    /// Seat `children` in a grid `ncols` wide.
    fn seat(children: &[std::sync::Arc<ViewNode>], ncols: usize, declared_rows: usize) -> Self {
        let mut flow = Self {
            runs: Vec::with_capacity(children.len()),
            rows: 0,
        };
        if ncols == 0 {
            // Tree acceptance refuses a grid with no columns; this is the
            // defensive floor for a container called out of turn.
            return flow;
        }
        // The same ceiling tree acceptance measures a row span against, so the
        // clamp below and `Violation::GridSpanOutOfRange` can never disagree
        // about which spans are reachable. Floored at one so a childless grid
        // still seats a run rather than a run of no rows.
        let row_cap = max_row_tracks(declared_rows, children.len()).max(1);
        // Row-major occupancy, grown a row at a time as runs reach into rows
        // the cursor has not visited.
        let mut taken: Vec<bool> = Vec::with_capacity(children.len());
        let mut cursor = 0_usize;
        for child in children {
            let span: GridSpan = child.props.span();
            // Tree acceptance refuses a span larger than its axis has tracks
            // (`Violation::GridSpanOutOfRange`), so both clamps are defensive
            // floors for a container called out of turn. Without the column
            // one, a run wider than a row would wrap forever looking for a
            // home; without the row one, a `usize` typo would ask for an
            // occupancy map the machine does not have.
            let ncols_spanned = span.on(Axis::Horizontal).min(ncols);
            let nrows_spanned = span.on(Axis::Vertical).min(row_cap);
            let run = loop {
                let col = cursor % ncols;
                let row = cursor / ncols;
                if col + ncols_spanned > ncols {
                    // Not enough of this row left; the run stays contiguous by
                    // starting the next one.
                    cursor = (row + 1) * ncols;
                    continue;
                }
                let needed = (row + nrows_spanned) * ncols;
                if taken.len() < needed {
                    taken.resize(needed, false);
                }
                let free = (row..row + nrows_spanned)
                    .all(|r| (col..col + ncols_spanned).all(|c| !taken[r * ncols + c]));
                if !free {
                    cursor += 1;
                    continue;
                }
                for r in row..row + nrows_spanned {
                    for c in col..col + ncols_spanned {
                        taken[r * ncols + c] = true;
                    }
                }
                cursor = row * ncols + col + ncols_spanned;
                break CellRun {
                    col,
                    row,
                    ncols: ncols_spanned,
                    nrows: nrows_spanned,
                };
            };
            flow.rows = flow.rows.max(run.row + run.nrows);
            flow.runs.push(run);
        }
        flow
    }

    /// Child `i`'s run, or a one-cell run at the origin when `i` names no
    /// child. The fallback is unreachable through [`measure`] and [`place`],
    /// which only ever index their own children.
    fn run(&self, i: usize) -> CellRun {
        self.runs.get(i).copied().unwrap_or(CellRun {
            col: 0,
            row: 0,
            ncols: 1,
            nrows: 1,
        })
    }
}

/// A placed run's extent, taken from the seam its neighbour will start at.
///
/// [`span_extent`] sums a run's own tracks left to right; [`cumulative_offsets`]
/// accumulates absolutely across the whole axis. Both reach the same
/// mathematical boundary, by different associations, and `f32` does not agree
/// that those are one number. For a single-cell child that never mattered —
/// its far edge *is* the next track's origin, the same float from the same
/// addition — which is exactly the guarantee `cumulative_offsets`' own doc
/// comment says it exists to provide.
///
/// Spanning broke it. Measured, 2026-08-22, 401x307 at scale 1.0 over 96
/// weighted quarter-hour rows: a run of 8 starting at row 41 ended at
/// `138.41664 + 24.083332 = 162.49997`, while `cumulative_offsets` put row 49
/// at `162.50002`. A 4.6e-05 disagreement, except 162.5 is a rounding
/// boundary, so `crate::frame::rounding` sent the two sides of one seam to
/// device rows 162 and 163 — a visible one-pixel gap between two abutting
/// quarter-hours, at scale 1.0, on a display with no fractional scaling at
/// all. `petra/tests/layout_matrix.rs`'s
/// `a_week_view_lays_out_from_one_grid` is what surfaced it.
///
/// So a run that has a neighbour reads its far edge back out of the offsets
/// the neighbour will use, and the seam is bit-identical by construction
/// rather than by luck.
///
/// A run ending on the **last** track has no neighbour, and summing there put
/// the same defect at the other end of the axis. Measured, 2026-08-23, a
/// hundred weighted rows in 301 units: the rows ended at `300.99994` while the
/// grid itself ended at `301.0`, and at scale 1.5 those are device rows 451
/// and 452 — a one-pixel band of light along the bottom of the grid. Over 5
/// scales x 4 row counts x 6 heights x 4 spans: 37 splits, every one at the
/// axis end, zero at an interior seam.
///
/// `far` closes that. It is the coordinate the *tracks themselves* end on when
/// they partition the whole content box — which is a structural fact, not a
/// measured one: weighted tracks are defined to absorb the budget, so their
/// far edge **is** the content box's far edge whatever `budget * (w / total)`
/// rounds to. The last run reads it the way an interior run reads its
/// neighbour's offset, and the residue of a hundred roundings lands inside the
/// last track instead of between the grid and its own last row.
///
/// `far` is `None` — and the sum is the answer again — exactly when the tracks
/// do **not** partition the box: three 40-unit columns in a 300-unit grid stop
/// at 132, and anchoring them to 300 would stretch the last column by 168
/// units. [`Tracks::fills`] is what tells the two apart, and it is decided
/// where the distribution happens rather than guessed from the numbers here;
/// a numeric "did they add up to the budget?" test would be the tolerance this
/// whole function exists to avoid.
///
/// [`span_extent`] is unchanged and still correct for **measurement**, where
/// there is no seam and no offsets array yet.
fn seam_extent(
    offsets: &[f32],
    track_extents: &[f32],
    far: Option<f32>,
    spacing: f32,
    start: usize,
    count: usize,
) -> f32 {
    let end = start.saturating_add(count.max(1));
    match (offsets.get(start), offsets.get(end)) {
        (Some(&lead), Some(&next)) => next - spacing.max(0.0) - lead,
        // The axis end, standing in for the neighbour that is not there. The
        // floor is the same defensive floor `span_extent` has: it cannot fire
        // while the offsets are monotonic, and it costs nothing when it does
        // not, because `max` on a positive `f32` returns that same `f32`.
        (Some(&lead), None) if end == offsets.len() => match far {
            Some(far) => (far - lead).max(0.0),
            None => span_extent(track_extents, spacing, start, count),
        },
        _ => span_extent(track_extents, spacing, start, count),
    }
}

/// The extent a child spanning `count` tracks from `start` negotiates
/// against: the summed extents of those tracks **plus the `count - 1` gaps
/// between them**.
///
/// The gaps are not decoration. Two 40-unit columns 8 apart offer a child
/// spanning both of them 88 units, not 80: the gap is interior to the run, so
/// nothing else can claim it. Dropping the term makes every multi-track child
/// negotiate against less space than it actually has, and it is the exact
/// clause `spanning_extent_counts_the_interior_gaps` holds down.
///
/// `count == 1` returns the track's own extent untouched, with no arithmetic
/// applied to it at all. That is deliberate: a tree that declares no span must
/// reach [`crate::frame::petrify`] with bit-identical rects to the ones it had
/// before this function existed, and `x + 0.0 - 0.0` is not a guarantee about
/// `f32`, it is a hope.
///
/// Track indices past the end read as zero rather than panicking. Tree
/// acceptance refuses a span that runs past the last track
/// ([`crate::tree::Violation::GridSpanOutOfRange`]) and `petrify` takes only a
/// validated tree, so this is the defensive floor for a container called out
/// of turn, not a shape the contract describes.
fn span_extent(track_extents: &[f32], spacing: f32, start: usize, count: usize) -> f32 {
    let spacing = spacing.max(0.0);
    let mut total = 0.0_f32;
    for (nth, track) in (start..start.saturating_add(count.max(1))).enumerate() {
        if nth > 0 {
            // The gap contribution: interior to the run, so nothing outside
            // the span can claim it, and the child negotiates against it.
            total += spacing;
        }
        total += track_extents.get(track).copied().unwrap_or(0.0);
    }
    total
}

/// One track's content-driven extent, accumulated from the children that
/// touch it.
///
/// This is the FR-061 sizing rule in one place, so both axes obey it
/// identically. `contracts/view-tree.md`: a spanning child "contributes to
/// those tracks' sizing only where no single-track child already determines
/// them — a spanning child never makes a track larger than the largest
/// single-track child in it, which is what keeps track sizing independent of
/// span resolution order and therefore deterministic."
///
/// So the two populations are kept apart rather than folded into one running
/// maximum:
///
/// * Any single-track child in this track **determines** it. Its largest
///   answer is the track's extent, and a spanning child cannot add to that —
///   not by a unit. Presence determines, not size: a single-track child that
///   answers zero still fixes the track at zero, because it is still an author
///   putting something in exactly this track and nowhere else.
/// * Only where no single-track child occupies the track at all does a
///   spanning child get a say, and then it asks for an even share of what it
///   wanted, after the interior gaps it already gets for free are taken off.
///
/// Both branches are a plain maximum over per-child values, each computed from
/// that one child's own measurement alone. Nothing is subtracted from a
/// running remainder, so no track's extent depends on which spanning child was
/// resolved first — the property FR-006's digest rests on. The alternative
/// (CSS Grid's distribute-the-excess pass) needs an explicit sort to be
/// deterministic at all, and still leaves track sizing a function of
/// resolution order.
#[derive(Clone, Copy, Debug, Default)]
struct TrackNatural {
    /// Largest answer from a child occupying this track and no other on this
    /// axis. `None` means no such child exists, which is the only case a
    /// spanning child can speak into.
    single: Option<f32>,
    /// Largest per-track share claimed by a spanning child.
    spanned: f32,
}

impl TrackNatural {
    /// Record a child that occupies this track and no other on this axis.
    fn single(&mut self, extent: f32) {
        self.single = Some(self.single.unwrap_or(0.0).max(extent));
    }

    /// Record a child that covers this track along with `count - 1` others.
    fn spanning(&mut self, extent: f32, spacing: f32, count: usize) {
        let count = count.max(1);
        // The child is already getting the interior gaps (see `span_extent`),
        // so only the part of its answer the tracks themselves have to carry
        // is shared out among them.
        let share = (extent - reserved(spacing, count)).max(0.0) / count as f32;
        self.spanned = self.spanned.max(share);
    }

    /// The track's content extent.
    fn resolve(self) -> f32 {
        self.single.unwrap_or(self.spanned)
    }
}

/// Row tracks after the implicit-row rule: declared rows are used as given,
/// then grown with `FitContent` — the same sizing an implicit row gets when
/// none are declared — until every row [`Flow::seat`] used has a track. A tree
/// that declares more rows than its children need is left exactly as declared;
/// this only ever adds rows, never removes one.
///
/// The count comes from the seating rather than from `children.len() / ncols`
/// because a row-spanning child reaches into rows no child's index names.
fn effective_row_tracks(rows: &[TrackSize], flow: &Flow) -> Vec<TrackSize> {
    let mut out = rows.to_vec();
    while out.len() < flow.rows {
        out.push(TrackSize::FitContent);
    }
    out
}

/// What [`spend_declared`] found: the tracks that name their own size have
/// taken it, and what the weight tracks divide is what is left.
struct Declared {
    /// Extents so far. Weight tracks are still zero; they are filled in by
    /// the caller, which is the only step that needs `weight_total`.
    widths: Vec<f32>,
    /// The budget the weight tracks share.
    budget: f32,
    /// Summed weights, zero when no track on this axis is weighted.
    weight_total: f32,
    /// Whether a track declared more room than the budget still held.
    ///
    /// Recorded here, per track, against the running budget — not recovered
    /// afterwards by adding the extents back up. Those are the same question
    /// in exact arithmetic and not in `f32`, and the re-summed form is the one
    /// that reported a plain weighted grid as truncated: see
    /// [`distribute_tracks`].
    overflowed: bool,
}

/// The first distribution pass: every track that declares its own size takes
/// it, left to right, out of `budget`.
///
/// `Fixed` takes its declared value whether or not the budget can carry it —
/// that is what makes it fixed, and reporting the overflow rather than
/// silently shrinking is what [`Declared::overflowed`] is for. `FitContent`
/// asks its content and is clamped to what remains, so it can never overflow.
/// `Weight` declares nothing here; it is a share of whatever this pass leaves.
///
/// Lifted out of [`distribute_tracks`] so that function reads as the three
/// steps it is — reserve the gaps, spend the declared sizes, share the rest —
/// rather than carrying all three in one body.
fn spend_declared(
    tracks: &[TrackSize],
    budget: f32,
    natural: &mut impl FnMut(usize, Proposal) -> f32,
) -> Declared {
    let mut out = Declared {
        widths: vec![0.0_f32; tracks.len()],
        budget,
        weight_total: 0.0,
        overflowed: false,
    };
    for (i, track) in tracks.iter().enumerate() {
        match track {
            TrackSize::Fixed { value } => {
                let w = value.max(0.0);
                out.widths[i] = w;
                if w > out.budget + FIT_EPSILON {
                    out.overflowed = true;
                }
                out.budget = (out.budget - w).max(0.0);
            }
            TrackSize::FitContent => {
                let ideal = natural(i, Proposal::Unspecified).max(0.0);
                let w = ideal.min(out.budget);
                out.widths[i] = w;
                out.budget = (out.budget - w).max(0.0);
            }
            TrackSize::Weight { weight } => {
                out.weight_total += weight.max(0.0);
            }
        }
    }
    out
}

/// Resolve one axis of grid tracks against `container_probe` (the
/// container's own proposal on this axis).
///
/// `natural` measures one track's content-driven extent for a given probe;
/// columns and rows differ only in what the *other* axis of that probe is
/// (a column probes its children at an ideal cross-axis size because row
/// heights are not decided yet; a row probes at the exact column width its
/// cells already have), so the caller supplies it rather than this function
/// assuming one.
fn distribute_tracks(
    tracks: &[TrackSize],
    spacing: f32,
    container_probe: Proposal,
    mut natural: impl FnMut(usize, Proposal) -> f32,
) -> Tracks {
    let n = tracks.len();
    if n == 0 {
        return Tracks::default();
    }

    let Some(avail) = container_probe.available() else {
        // Open probe (`Unbounded`/`Unspecified`): there is no total to
        // divide, so every track reports its own extent under the same
        // probe the container itself was asked. `Fixed` never asks content;
        // `Weight` has no "leftover" to be proportional to without a budget,
        // so it falls back to the same natural-extent read as `FitContent`.
        return Tracks {
            extents: tracks
                .iter()
                .enumerate()
                .map(|(i, track)| match track {
                    TrackSize::Fixed { value } => value.max(0.0),
                    TrackSize::FitContent | TrackSize::Weight { .. } => {
                        natural(i, container_probe).max(0.0)
                    }
                })
                .collect(),
            spacing: spacing.max(0.0),
            // An open probe has no budget, so nothing can fail to fit in it,
            // and nothing partitions it either: there is no far edge for a
            // last track to close on.
            truncated: false,
            fills: false,
        };
    };

    // Closed probe (`Exact` or `Zero`, both answered by `.available()`): a
    // real budget to divide, left to right. `Fixed` takes its declared value
    // here; `FitContent` is clamped to what is left; `Weight` only ever draws
    // from the running `budget`, so it is self-limiting.
    //
    // Declaring is not the same as getting, though. A row of `Fixed` tracks
    // can add up to more than the container was offered, and the pass below
    // is what stops that from becoming children placed outside their parent,
    // overlapping each other, with nothing anywhere saying so — which is what
    // this function used to do, and what `contracts/view-tree.md`'s "never
    // silently overlapped" forbids.
    // The gaps come out of the budget before any track does — and the gaps
    // themselves are not exempt from it. Four columns with 16 units of spacing
    // need 48 units of gap, which does not fit a 40-unit grid, and adding them
    // unconditionally made a childless grid report wider than the offer it had
    // just been handed. When that happens the spacing shrinks to what is left
    // and every track collapses; it is a truncation, and it is reported as one.
    let mut spacing = spacing.max(0.0);
    let mut forced = false;
    if reserved(spacing, n) > avail {
        spacing = if n > 1 { avail / (n - 1) as f32 } else { 0.0 };
        forced = true;
    }
    let Declared {
        mut widths,
        budget,
        weight_total,
        overflowed,
    } = spend_declared(
        tracks,
        (avail - reserved(spacing, n)).max(0.0),
        &mut natural,
    );
    // Shrinking the gaps to fit is itself a declaration that did not fit.
    let overflowed = overflowed || forced;
    if weight_total > 0.0 {
        for (i, track) in tracks.iter().enumerate() {
            if let TrackSize::Weight { weight } = track {
                // `budget` can never go negative (every subtraction in
                // `spend_declared` floors at zero), so a weight track's share
                // never goes negative either — it collapses to zero, not
                // overflow.
                widths[i] = (budget * (weight.max(0.0) / weight_total)).max(0.0);
            }
        }
        if !overflowed {
            return Tracks {
                extents: widths,
                spacing,
                truncated: false,
                fills: true,
            };
        }
        // A weight track is a share of `budget`, and `budget` is what the
        // fixed and fit-content tracks left. A share of a budget cannot
        // exceed the budget, so this axis fits, exactly, by construction —
        // and it is a partition of `avail`, which is what `Tracks::fills`
        // says and what lets a run ending on the last track close on the
        // content box's own edge (`seam_extent`).
        //
        // [`fit_to_budget`] is not asked, because asking it means re-deriving
        // the total by summing the extents back up, and summing is the one
        // question this axis cannot answer. Measured, 2026-08-23: 96 weighted
        // rows of `727 / 96` accumulate to `727.0003`, which is over
        // `FIT_EPSILON`, so a grid that had truncated nothing reported
        // `truncated` — a digest input — and then cut 3e-4 off its last row
        // to "fit" a budget it already fitted. The overflow that can really
        // happen here is a *declared* extent bigger than the budget, and
        // `overflowed` catches that in `spend_declared`, per track, while the
        // budget is still a number nobody has had to re-sum.
    }
    let mut fitted = fit_to_budget(widths, spacing, avail);
    fitted.truncated |= forced;
    // Not a partition: either nothing draws on the leftover budget (the tracks
    // declare their own size and the grid may be bigger than all of them), or
    // the cut is what decided where they stop. Either way the last track ends
    // where its own extent ends, not where the content box does.
    fitted.fills = false;
    fitted
}

/// Resolved track extents, the spacing actually used between them, and
/// whether anything had to be cut to fit.
///
/// `spacing` is carried rather than re-read from the props because it is not
/// always what the author declared: a grid can be offered less than its own
/// gaps need, and in that case the gaps shrink too. Reading the declared value
/// back at placement time would put the tracks back where they did not fit.
#[derive(Clone, Debug, Default, PartialEq)]
struct Tracks {
    extents: Vec<f32>,
    spacing: f32,
    truncated: bool,
    /// Whether these tracks partition the whole budget they were given, so
    /// that the last one's far edge **is** the content box's far edge.
    ///
    /// A structural answer, decided by which sizing kinds took part, not a
    /// numeric one recovered by adding the extents back up — the sum is
    /// precisely the quantity that cannot be trusted here ([`seam_extent`]).
    ///
    /// A weighted track is defined as a share of whatever budget the fixed and
    /// fit-content tracks left, so any grid with one absorbs its axis exactly.
    /// Without one the tracks say how big they are and the grid may be larger;
    /// three 40-unit columns in a 300-unit grid end at 132 and must stay
    /// there. Truncation is excluded for the same reason from the other side:
    /// it deliberately collapses the trailing tracks to zero, and a collapsed
    /// last track anchored to the far edge would be handed back all the room
    /// the cut just took away.
    fills: bool,
}

/// Force `extents` to fit `avail` once spacing is reserved, and report whether
/// anything had to give.
///
/// This is FR-005's concession order arriving at its third step. There is no
/// flexible slack left to take — `FitContent` and `Weight` were already
/// clamped to the running budget above, so anything still over is `Fixed`
/// tracks declaring more than exists — and a grid track is not a declared
/// scroll region, so it cannot absorb. That leaves truncation.
///
/// Truncation runs left to right: each track keeps as much as remains, and
/// tracks past the end collapse to zero rather than every track shrinking
/// proportionally. A `Fixed` track means "this many units"; honouring the
/// declaration for the tracks that fit, and being visibly empty past that, is
/// a more legible failure than silently making every column narrower than it
/// asked for.
fn fit_to_budget(extents: Vec<f32>, spacing: f32, avail: f32) -> Tracks {
    let gaps = reserved(spacing, extents.len());
    let total: f32 = extents.iter().sum::<f32>() + gaps;
    if total <= avail + FIT_EPSILON {
        return Tracks {
            extents,
            spacing,
            truncated: false,
            fills: false,
        };
    }
    let mut left = (avail - gaps).max(0.0);
    let cut = extents
        .into_iter()
        .map(|want| {
            let got = want.min(left);
            left -= got;
            got
        })
        .collect();
    Tracks {
        extents: cut,
        spacing,
        truncated: true,
        fills: false,
    }
}

/// Column widths for `tracks` against `container_probe` (the container's
/// horizontal proposal).
fn resolve_columns(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    tracks: &[TrackSize],
    spacing: f32,
    container_probe: Proposal,
    flow: &Flow,
) -> Tracks {
    distribute_tracks(tracks, spacing, container_probe, |col, probe| {
        column_natural_width(node, ctx, path, col, flow, spacing, probe)
    })
}

/// Row heights for `tracks` against `container_probe` (the container's
/// vertical proposal), given the column widths already resolved.
fn resolve_rows(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    tracks: &[TrackSize],
    container_probe: Proposal,
    rows: RowContext<'_>,
) -> Tracks {
    distribute_tracks(tracks, rows.row_spacing, container_probe, |row, probe| {
        row_natural_height(node, ctx, path, row, &rows, probe)
    })
}

/// Everything sizing a row track needs beyond the node and the probe.
///
/// Gathered into one value because the row axis, unlike the column axis, is
/// resolved second and so depends on what the first one settled: a cell's
/// height is measured at the exact width its column run already has, which
/// takes the resolved column extents, the gap actually used between them, and
/// the seating that says which columns the cell covers.
#[derive(Clone, Copy)]
struct RowContext<'a> {
    /// Column extents, already resolved.
    col_widths: &'a [f32],
    /// Gap actually used between columns, which is not always the declared
    /// one — see [`Tracks::spacing`].
    col_spacing: f32,
    /// Gap declared between rows.
    row_spacing: f32,
    /// Where each child sits.
    flow: &'a Flow,
}

/// Column `col`'s content width under `probe` on the horizontal axis. The
/// vertical axis is always `Unspecified`: a column's width does not yet know
/// what height its row will settle on.
///
/// `spacing` is this axis's **declared** gap, needed to take the interior gaps
/// off a spanning child's answer before it is shared out — see [`TrackNatural`]
/// for the rule that decides which children get a say here at all. Declared
/// rather than resolved because [`distribute_tracks`] has not decided yet
/// whether the gaps themselves have to shrink; the two differ only when the
/// container was offered less than its own gaps need, and in that case
/// `distribute_tracks` has already driven the budget to zero, so every
/// `FitContent` track collapses whatever this answers. Placement never reads
/// the declared value: [`Cells`] carries `Tracks::spacing`, the gap actually
/// used.
fn column_natural_width(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    col: usize,
    flow: &Flow,
    spacing: f32,
    probe: Proposal,
) -> f32 {
    let proposal = SizeProposal {
        horizontal: probe,
        vertical: Proposal::Unspecified,
    };
    let mut natural = TrackNatural::default();
    for (i, child) in node.children.iter().enumerate() {
        let (start, count) = flow.run(i).on(Axis::Horizontal);
        if col < start || col >= start.saturating_add(count) {
            continue;
        }
        let w = crate::layout::measure(child, ctx, path, proposal).w;
        if count == 1 {
            natural.single(w);
        } else {
            natural.spanning(w, spacing, count);
        }
    }
    natural.resolve()
}

/// Row `row`'s content height under `probe` on the vertical axis. Each cell's
/// horizontal axis is `Exact` at the width its own column run already
/// resolved to, per the contract's row rule — for a column-spanning child that
/// is the summed widths of the columns it covers plus the gaps between them,
/// which is the whole of what "negotiates against the summed track extents"
/// means on this axis.
fn row_natural_height(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    row: usize,
    rows: &RowContext<'_>,
    probe: Proposal,
) -> f32 {
    let RowContext {
        col_widths,
        col_spacing,
        row_spacing,
        flow,
    } = *rows;
    let mut natural = TrackNatural::default();
    for (i, child) in node.children.iter().enumerate() {
        let run = flow.run(i);
        let (start, count) = run.on(Axis::Vertical);
        if row < start || row >= start.saturating_add(count) {
            continue;
        }
        let proposal = SizeProposal {
            horizontal: Proposal::Exact(span_extent(col_widths, col_spacing, run.col, run.ncols)),
            vertical: probe,
        };
        let h = crate::layout::measure(child, ctx, path, proposal).h;
        if count == 1 {
            natural.single(h);
        } else {
            natural.spanning(h, row_spacing, count);
        }
    }
    natural.resolve()
}

/// Leading **absolute** coordinates for a run of track sizes plus fixed
/// spacing between them, starting at `origin`.
///
/// Absolute, not relative, for the same reason `layout/stack.rs` accumulates
/// an absolute cursor: a relative offset makes track `i`'s trailing edge
/// `(origin + offset) + extent` while track `i + 1`'s leading edge is
/// `origin + (offset + extent)`. Exact arithmetic calls those one number;
/// `f32` disagrees often enough that `crate::frame::rounding` rounds the two
/// sides of one seam to two different device pixels, leaving a one-pixel gap
/// or overlap between columns that share an edge.
/// `petra/tests/layout_matrix.rs`'s
/// `abutting_grid_cells_share_a_device_edge_from_a_shifted_origin` pins it.
fn cumulative_offsets(origin: f32, sizes: &[f32], spacing: f32) -> Vec<f32> {
    let spacing = spacing.max(0.0);
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut acc = origin;
    for &size in sizes {
        offsets.push(acc);
        acc += size.max(0.0) + spacing;
    }
    offsets
}

/// Place one child into its resolved `cell`, per the alignment the caller
/// resolved for it — the child's own `Props::align_self` where it declares
/// one, and the grid's `align` otherwise.
///
/// A grid ignored `align_self` entirely until 2026-09-06, which made it a
/// property a component could declare and silently not get. The case that
/// found it: Carbon's inline code snippet is `display: inline` — a chip the
/// width of the words in it — and every column in this library is a
/// single-track `Grid`, so the chip filled the card. Nothing reported it,
/// because "declared and dropped" is not a shape any gate here looks for.
///
/// A cell is a box the child may be smaller than: every non-`Stretch`
/// alignment measures the child against the cell size (it may answer
/// smaller — a parent places, it does not force) and offsets the answer
/// inside the box with [`Align::offset`]. `Stretch` skips the measurement and
/// fills the cell on both axes, but a declared maximum on either axis still
/// wins (2026-08-22: constraints beat Stretch, see `.agents/tallies/QUESTIONS.md`
/// Round 3 item 2 and
/// `.agents/notes/implemented/bug-fix/2026-08-22-petra-stretch-honours-constraints.md`).
/// That fill is routed through `AxisConstraint::clamp` directly rather than
/// through a `crate::layout::measure` call: Stretch has no natural size to
/// ask for, only a declared clamp to respect, and `AxisConstraint::clamp` is
/// the crate's one definition of that.
fn place_in_cell(
    child: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    cell: Rect,
    align: Align,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    if align == Align::Stretch {
        // A declared minimum bigger than the cell still wins in the clamp
        // (`AxisConstraint::clamp`: min wins over max), but placement never
        // grows past the cell the grid actually has to give — the same rule
        // the non-Stretch arm below applies via `.min(cell.w)` /
        // `.min(cell.h)`.
        let w = child.constraints.horizontal.clamp(cell.w).min(cell.w);
        let h = child.constraints.vertical.clamp(cell.h).min(cell.h);
        let dx = align.offset(cell.w, w);
        let dy = align.offset(cell.h, h);
        let rect = Rect::new(cell.x + dx, cell.y + dy, w, h);
        crate::layout::place(
            child,
            ctx,
            path,
            slot.with_rect(rect).clipped_to(cell),
            sink,
        );
        return;
    }
    let response = crate::layout::measure(child, ctx, path, SizeProposal::exact(cell.size()));
    // Clamped to the cell, not placed at whatever the child answered. Some
    // leaves answer their intrinsic size regardless of the offer — an `Image`
    // is the obvious one — and placing that answer verbatim put a 64-unit
    // image into a 30-unit cell, overlapping the next column's child with
    // nothing to say it had happened. `contracts/view-tree.md`: children are
    // "never silently overlapped".
    let w = response.w.min(cell.w);
    let h = response.h.min(cell.h);
    let dx = align.offset(cell.w, w);
    let dy = align.offset(cell.h, h);
    let rect = Rect::new(cell.x + dx, cell.y + dy, w, h);
    // Clipped to the cell as well as sized to it, so a child that draws past
    // its own rect still cannot bleed into its neighbour.
    crate::layout::place(
        child,
        ctx,
        path,
        slot.with_rect(rect).clipped_to(cell),
        sink,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::placement::PlacementList;
    use crate::frame::rounding::round_rect;
    use crate::geom::Scale;
    use crate::geom::{Axis, Insets};
    use crate::testing::{Harness, MonoContent, NoRows, gap, gap_token};
    use crate::tree::{AxisConstraint, Constraints, InsetRefs, NodeKind, Props};
    use proptest::prelude::*;

    fn spacer(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key)
    }

    fn text(key: &str, s: &str) -> ViewNode {
        ViewNode::new(NodeKind::Text, key).with_props(Props {
            text: Some(s.to_owned()),
            ..Props::default()
        })
    }

    fn grid(columns: Vec<TrackSize>, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns,
                ..Props::default()
            })
            .with_children(children)
    }

    /// Mirrors how the dispatcher in `layout/mod.rs` would have arrived here:
    /// it pushes the node's own key before calling into the kind module.
    fn path_at(node: &ViewNode) -> KeyPath {
        let mut path = KeyPath::root();
        path.push(node.key.clone());
        path
    }

    // MonoContent: 8.0 logical units per character, 16.0 per line.

    // ---- FR-061: grid track spanning -------------------------------------

    /// A grid `ncols` wide of equal `Fixed` columns, with `spacing` between
    /// them and one declared `Fixed` row, so every extent below is a number
    /// the test states rather than one it measured.
    fn fixed_grid(ncols: usize, width: f32, spacing: f32, children: Vec<ViewNode>) -> ViewNode {
        ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: width }; ncols],
                rows: vec![TrackSize::Fixed { value: 20.0 }; 4],
                column_spacing: gap(spacing),
                row_spacing: gap(0.0),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(children)
    }

    fn spanning(key: &str, columns: usize, rows: usize) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key).with_props(Props {
            span: Some(GridSpan { columns, rows }),
            ..Props::default()
        })
    }

    /// A text child that spans, so a span can be given real content to
    /// negotiate with.
    fn spanning_text(key: &str, s: &str, columns: usize, rows: usize) -> ViewNode {
        ViewNode::new(NodeKind::Text, key).with_props(Props {
            text: Some(s.to_owned()),
            span: Some(GridSpan { columns, rows }),
            ..Props::default()
        })
    }

    /// The rects `place` produced for the children, in declaration order.
    fn child_rects(g: &ViewNode, offer: Rect) -> Vec<Rect> {
        let mut h = Harness::new();
        // Swept fixtures generate fractional gaps, which the pre-bound
        // whole-unit range does not cover. The names carry the numbers.
        h.bind_tree_gaps(g);
        let mut path = path_at(g);
        let mut sink = PlacementList::new();
        place(g, &mut h.ctx(), &mut path, Slot::new(offer), &mut sink);
        sink.as_slice()[1..].iter().map(|p| p.rect).collect()
    }

    /// The column extents this grid resolves to under `probe`.
    fn column_extents(g: &ViewNode, probe: Proposal) -> Vec<f32> {
        let mut h = Harness::new();
        h.bind_tree_gaps(g);
        let ctx = h.ctx();
        let column_spacing = ctx.spacing(&g.props.column_spacing);
        let flow = Flow::seat(&g.children, g.props.columns.len(), g.props.rows.len());
        let mut path = path_at(g);
        resolve_columns(
            g,
            &mut h.ctx(),
            &mut path,
            &g.props.columns,
            column_spacing,
            probe,
            &flow,
        )
        .extents
    }

    /// Hand-computed: three 40-unit columns 6 apart, a child covering the
    /// first two. It gets 40 + 6 + 40 = 86, not 80. The gap is interior to
    /// the run, so nothing outside the span can claim it, and a child that
    /// negotiated against 80 would leave six units of its own cell unused
    /// with no way to find out.
    /// The last-track branch: a run that ends the axis must cover its tracks
    /// *and* close on the float the grid's own content closes on.
    ///
    /// `seam_extent` reads a run's far edge out of the neighbour's offset, and
    /// a run ending on the last track has no neighbour. That branch is the one
    /// the week-view sweep cannot reach — nothing sits below or right of a run
    /// that ends the axis, so no seam assertion ever compares across it (leaf
    /// C3 named this gap rather than glossing it).
    ///
    /// Three claims, because the branch can be wrong in three directions.
    ///
    /// The first two run on `Fixed` tracks that under-fill nothing and sum
    /// exactly: the run must cover its tracks *and their interior gaps*, and
    /// it must stop where the tracks stop rather than overshooting into space
    /// no track owns. Those are the claims `ed65e08` wrote, and they are still
    /// the right claims — but every number in that fixture (40, 6, 132) is
    /// exactly representable in `f32`, so both hold under *any* summation
    /// order. **That fixture cannot fail on a summation defect**, which is why
    /// it sat green through the one below for a day.
    ///
    /// The third claim is the one it could not make. Weighted tracks partition
    /// the content box by construction, so the last one's far edge *is* the
    /// content box's far edge — but `budget * (w / total)` is not exactly
    /// representable, and summing a hundred of them left the last row's bottom
    /// at `300.99994` against a grid bottom of `301.0`. At scale 1.5 that is
    /// device row 451 against 452: a one-pixel light band along the bottom of
    /// the grid, the same artefact `feaa3b7` closed at the interior seams, at
    /// the other end of the axis. Measured over 5 scales x 4 row counts x 6
    /// heights x 4 spans: 37 splits, every one at the axis end.
    ///
    /// `assert_eq!` on raw `f32` for the same reason the interior seam uses
    /// it: the edge is one number, not two near ones. A tolerance here is the
    /// bug wearing the test's clothes.
    #[test]
    fn a_run_ending_on_the_last_track_covers_its_tracks_and_ends_where_the_grid_does() {
        // Three 40-unit columns, 6 apart: content spans 40*3 + 6*2 = 132.
        let g = fixed_grid(3, 40.0, 6.0, vec![spacer("a"), spanning("tail", 2, 1)]);
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 132.0, 80.0));
        let tail = rects[1];
        assert_eq!(
            tail.w, 86.0,
            "columns 1-2 plus the one interior gap: 40 + 6 + 40"
        );
        assert_eq!(
            tail.right(),
            132.0,
            "the run ends where the grid's content ends, not past it"
        );

        // Tracks whose extents are not exactly representable, at the display
        // scale the split shows up on. `h / rows` is never a binary fraction
        // in any of these, so the accumulated far edge and the grid's own far
        // edge are only equal if the code makes them equal.
        let scale = Scale::new(1.5).unwrap();
        for (rows, h) in [(100_usize, 301.0_f32), (48, 401.0), (96, 727.0)] {
            for span in [1_usize, 3] {
                let head = rows - span;
                let children = (0..head)
                    .map(|i| spacer(&format!("pre{i}")))
                    .chain([spanning("tail", 1, span)])
                    .collect::<Vec<_>>();
                let g = ViewNode::new(NodeKind::Grid, "g")
                    .with_props(Props {
                        columns: vec![TrackSize::Fixed { value: 40.0 }],
                        rows: vec![TrackSize::Weight { weight: 1.0 }; rows],
                        row_spacing: gap(0.0),
                        column_spacing: gap(0.0),
                        align: Some(Align::Stretch),
                        ..Props::default()
                    })
                    .with_children(children);
                let offer = Rect::new(0.0, 0.0, 40.0, h);
                let tail = *child_rects(&g, offer).last().unwrap();
                assert_eq!(
                    tail.bottom(),
                    offer.bottom(),
                    "rows={rows} h={h} span={span}: the last row must end on the \
                     grid's own float, not {} — delta {:e}",
                    tail.bottom(),
                    offer.bottom() - tail.bottom()
                );
                assert_eq!(
                    round_rect(tail, scale).bottom(),
                    round_rect(offer, scale).bottom(),
                    "rows={rows} h={h} span={span}: one device pixel of light \
                     along the bottom of the grid at scale 1.5"
                );
            }
        }
    }

    /// A spanning run's far edge is the *same float* its neighbour starts at.
    ///
    /// The regression this pins was real and visible. `span_extent` sums a
    /// run's own tracks; `cumulative_offsets` accumulates absolutely across the
    /// axis. Exact arithmetic calls those one number and `f32` does not, and
    /// when the disagreement straddles a rounding boundary the two sides of one
    /// seam land on different device pixels.
    ///
    /// The numbers here are the measured case, not an invented one: 96 weighted
    /// quarter-hour rows in 289 units at scale 1.0, a run of 8 rows starting at
    /// row 41. Summing gave `138.41664 + 24.083332 = 162.49997`; the offsets put
    /// row 49 at `162.50002`; 162.5 is a rounding boundary, so the block ended
    /// on device row 162 and its neighbour began on 163.
    ///
    /// `assert_eq!` on `f32` is deliberate. "Close enough" is exactly the bug —
    /// the whole point is that the seam is one number, not two near ones.
    #[test]
    fn a_spanning_run_ends_on_the_float_its_neighbour_begins_at() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 40.0 }],
                rows: vec![TrackSize::Weight { weight: 1.0 }; 96],
                row_spacing: gap(0.0),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(
                (0..41)
                    .map(|i| spacer(&format!("pre{i}")))
                    .chain([spanning("block", 1, 8), spacer("after")])
                    .collect::<Vec<_>>(),
            );
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 40.0, 289.0));
        let block = rects[41];
        let after = rects[42];
        assert_eq!(
            block.bottom(),
            after.y,
            "the seam must be one float: block ends {}, neighbour starts {}, \
             delta {:e}",
            block.bottom(),
            after.y,
            after.y - block.bottom()
        );
    }

    #[test]
    fn a_spanning_child_is_offered_the_gap_between_the_tracks_it_covers() {
        let g = fixed_grid(3, 40.0, 6.0, vec![spanning("wide", 2, 1), spacer("c")]);
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 132.0, 80.0));
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 86.0, 20.0),  // wide: columns 0-1 plus the gap
                Rect::new(92.0, 0.0, 40.0, 20.0), // c: column 2, unmoved
            ]
        );
    }

    /// The row-axis statement of the same rule, so neither axis can lose the
    /// gap term while the other keeps it: two 20-unit rows 9 apart give a
    /// two-row child 49 units, not 40.
    #[test]
    fn the_gap_between_spanned_rows_counts_too() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 30.0 }],
                rows: vec![TrackSize::Fixed { value: 20.0 }; 3],
                row_spacing: gap(9.0),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![spanning("tall", 1, 2), spacer("b")]);
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 30.0, 78.0));
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 30.0, 49.0),  // rows 0-1 plus the 9-unit gap
                Rect::new(0.0, 58.0, 30.0, 20.0), // b: row 2, unmoved
            ]
        );
    }

    /// **The binding invariant.** Adding a spanning child to a grid whose
    /// columns are already determined must not move one of them.
    ///
    /// `contracts/view-tree.md`: "a spanning child never makes a track larger
    /// than the largest single-track child in it, which is what keeps track
    /// sizing independent of span resolution order and therefore
    /// deterministic." The spanning child here asks for as much as the
    /// generator can make it ask for; every column still answers exactly what
    /// its own single-track child asked for. Merge the two populations into
    /// one running maximum and this goes red on the first wide span.
    #[test]
    fn adding_a_spanning_child_never_moves_a_track_a_single_track_child_set() {
        proptest!(|(
            widths in prop::collection::vec(1_usize..24, 2..5),
            span_chars in 0_usize..200,
            span_cols in 2_usize..5,
            spacing in prop::sample::select(vec![0.0_f32, 4.0, 17.5]),
        )| {
            let ncols = widths.len();
            let span_cols = span_cols.min(ncols);
            let columns = vec![TrackSize::FitContent; ncols];
            // One single-track text per column, each with its own content
            // width. These are what determine the columns.
            let determined: Vec<ViewNode> = widths
                .iter()
                .enumerate()
                .map(|(i, chars)| text(&format!("d{i}"), &"x".repeat(*chars)))
                .collect();

            let before = ViewNode::new(NodeKind::Grid, "g")
                .with_props(Props {
                    columns: columns.clone(),
                    column_spacing: gap(spacing),
                    ..Props::default()
                })
                .with_children(determined.clone());
            let baseline = column_extents(&before, Proposal::Unspecified);
            // MonoContent is 8 units per character, so each column is its own
            // child's run width and nothing else.
            prop_assert_eq!(
                &baseline,
                &widths.iter().map(|c| *c as f32 * 8.0).collect::<Vec<f32>>()
            );

            // Now the only change: one spanning child, appended.
            let mut after = before.clone();
            after.children.push(std::sync::Arc::new(spanning_text(
                "wide",
                &"x".repeat(span_chars),
                span_cols,
                1,
            )));
            prop_assert_eq!(
                column_extents(&after, Proposal::Unspecified),
                baseline,
                "a {}-column span of {} chars moved a determined track",
                span_cols,
                span_chars,
            );
        });
    }

    /// The other half of the rule: where **no** single-track child claims a
    /// column, the spanning child does get a say — otherwise a grid whose only
    /// content spans would collapse to nothing.
    ///
    /// Two fit-content columns 4 apart and one child covering both. The child
    /// wants 80 units; 4 of them are the gap it already has, so the two
    /// columns take 38 each and the run comes to exactly the 80 asked for.
    #[test]
    fn a_span_sizes_the_columns_no_single_track_child_claims() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::FitContent; 2],
                column_spacing: gap(4.0),
                ..Props::default()
            })
            .child(spanning_text("wide", "xxxxxxxxxx", 2, 1));
        assert_eq!(column_extents(&g, Proposal::Unspecified), vec![38.0, 38.0]);
    }

    /// Presence determines, not size. A single-track child that answers zero
    /// still fixes its column at zero: it is an author putting something in
    /// exactly that column and nowhere else, and a span reaching over it must
    /// not overrule that.
    #[test]
    fn a_single_track_child_that_answers_zero_still_holds_its_column_down() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::FitContent; 2],
                ..Props::default()
            })
            // Row 0: an empty text in column 0, nothing in column 1.
            // Row 1: a wide child spanning both.
            .child(text("empty", ""))
            .child(spanning_text("wide", "xxxxxxxxxx", 2, 1));
        // Column 0 is held at zero by `empty`; column 1 has no single-track
        // child, so the span sizes it to its even share of 80.
        assert_eq!(column_extents(&g, Proposal::Unspecified), vec![0.0, 40.0]);
    }

    /// Track sizing must not depend on the order spans are resolved in, which
    /// is what FR-006's digest rests on. Two spanning children covering the
    /// same columns from different rows produce the same columns whichever
    /// order they are declared in — a distribute-the-excess pass would not.
    #[test]
    fn column_sizing_does_not_depend_on_which_span_is_resolved_first() {
        proptest!(|(a in 0_usize..40, b in 0_usize..40)| {
            let columns = vec![TrackSize::FitContent; 2];
            let build = |first: &str, second: &str| {
                ViewNode::new(NodeKind::Grid, "g")
                    .with_props(Props {
                        columns: columns.clone(),
                        ..Props::default()
                    })
                    .child(spanning_text("p", first, 2, 1))
                    .child(spanning_text("q", second, 2, 1))
            };
            let fwd = column_extents(&build(&"x".repeat(a), &"x".repeat(b)), Proposal::Unspecified);
            let rev = column_extents(&build(&"x".repeat(b), &"x".repeat(a)), Proposal::Unspecified);
            prop_assert_eq!(fwd, rev);
        });
    }

    /// No two children ever share a cell, for any shape of span.
    ///
    /// This is the property that made the cursor necessary. Seating child `i`
    /// at the bare `(i % ncols, i / ncols)` puts the child after a two-column
    /// span directly on top of that span's second cell, and no arrangement of
    /// the tree can avoid it, because under that rule the index *is* the
    /// column.
    #[test]
    fn no_two_children_ever_share_a_cell() {
        proptest!(|(
            ncols in 1_usize..5,
            spans in prop::collection::vec((1_usize..5, 1_usize..4), 1..9),
        )| {
            let children: Vec<ViewNode> = spans
                .iter()
                .enumerate()
                .map(|(i, (c, r))| spanning(&format!("c{i}"), (*c).min(ncols), *r))
                .collect();
            let g = ViewNode::new(NodeKind::Grid, "g")
                .with_props(Props {
                    columns: vec![TrackSize::FitContent; ncols],
                    ..Props::default()
                })
                .with_children(children);
            let flow = Flow::seat(&g.children, ncols, 0);
            prop_assert_eq!(flow.runs.len(), spans.len());
            for (i, run) in flow.runs.iter().enumerate() {
                // Contiguous and inside the grid on the column axis.
                prop_assert!(
                    run.col + run.ncols <= ncols,
                    "run {i} at {run:?} runs past column {ncols}"
                );
                prop_assert!(run.row + run.nrows <= flow.rows);
                for (j, other) in flow.runs.iter().enumerate().take(i) {
                    let cols_meet =
                        run.col < other.col + other.ncols && other.col < run.col + run.ncols;
                    let rows_meet =
                        run.row < other.row + other.nrows && other.row < run.row + run.nrows;
                    prop_assert!(
                        !(cols_meet && rows_meet),
                        "child {i} at {run:?} shares a cell with child {j} at {other:?}"
                    );
                }
            }
        });
    }

    /// **The no-movement guarantee, stated as the seating rule it rests on.**
    ///
    /// With every span at one the cursor advances one cell per child and never
    /// skips, so child `i` seats at exactly `(i % ncols, i / ncols)` — the
    /// rule this grid followed before FR-061 existed. That is *why* a tree
    /// with no declared span keeps its rects, and therefore its frame digest.
    #[test]
    fn a_tree_with_no_span_seats_every_child_exactly_where_its_index_says() {
        proptest!(|(ncols in 1_usize..6, n in 0_usize..21)| {
            let children: Vec<ViewNode> =
                (0..n).map(|i| spacer(&format!("c{i}"))).collect();
            let g = ViewNode::new(NodeKind::Grid, "g")
                .with_props(Props {
                    columns: vec![TrackSize::FitContent; ncols],
                    ..Props::default()
                })
                .with_children(children);
            let flow = Flow::seat(&g.children, ncols, 0);
            for i in 0..n {
                prop_assert_eq!(
                    flow.runs[i],
                    CellRun { col: i % ncols, row: i / ncols, ncols: 1, nrows: 1 }
                );
            }
            prop_assert_eq!(flow.rows, n.div_ceil(ncols));
        });
    }

    /// A run that does not fit what is left of a row starts the next one
    /// rather than wrapping across the edge: the contract says the tracks a
    /// child covers are contiguous, and a wrapped run is two pieces.
    #[test]
    fn a_run_too_wide_for_the_rest_of_its_row_starts_the_next_row() {
        let g = fixed_grid(
            3,
            10.0,
            0.0,
            vec![spacer("a"), spanning("wide", 3, 1), spacer("z")],
        );
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 30.0, 60.0));
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 10.0, 20.0),  // a: row 0, column 0
                Rect::new(0.0, 20.0, 30.0, 20.0), // wide: all of row 1
                Rect::new(0.0, 40.0, 10.0, 20.0), // z: row 2, the hole is not backfilled
            ]
        );
    }

    /// A row-spanning child reaches into rows the cursor has not visited, so
    /// the next child has to step around it rather than land underneath.
    #[test]
    fn a_row_span_pushes_later_children_past_the_cells_it_holds() {
        let g = fixed_grid(
            2,
            10.0,
            0.0,
            vec![spanning("tall", 1, 2), spacer("b"), spacer("c")],
        );
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 20.0, 40.0));
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 10.0, 40.0),   // tall: column 0, rows 0-1
                Rect::new(10.0, 0.0, 10.0, 20.0),  // b: column 1, row 0
                Rect::new(10.0, 20.0, 10.0, 20.0), // c: column 1, row 1 — not (0,1)
            ]
        );
    }

    /// A span across a `Weight` track negotiates against what the weights
    /// actually resolved to, not against a declared number: 200 units, 20 of
    /// gap, a 40-unit fixed column, and 140 split evenly between two weights.
    /// The child covering both weight columns gets 70 + 10 + 70.
    #[test]
    fn a_span_across_weight_tracks_uses_their_resolved_extents() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 40.0 },
                    TrackSize::Weight { weight: 1.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                rows: vec![TrackSize::Fixed { value: 20.0 }],
                column_spacing: gap(10.0),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![spacer("a"), spanning("wide", 2, 1)]);
        let rects = child_rects(&g, Rect::new(0.0, 0.0, 200.0, 20.0));
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 40.0, 20.0),
                Rect::new(50.0, 0.0, 150.0, 20.0),
            ]
        );
    }

    /// The cell clamp still applies, and it clamps to the whole run rather
    /// than to one track. A child whose declared minimum is wider than the two
    /// columns it covers is cut to those two columns and the gap between them,
    /// and the grid reports the loss instead of letting it bleed into the next
    /// column.
    #[test]
    fn a_child_too_wide_for_its_run_is_clamped_to_the_run_and_reported() {
        let mut wide = spanning("wide", 2, 1);
        wide.constraints = Constraints {
            horizontal: AxisConstraint {
                min: Some(500.0),
                ..AxisConstraint::default()
            },
            ..Constraints::default()
        };
        let g = fixed_grid(3, 40.0, 6.0, vec![wide, spacer("c")]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 132.0, 20.0)),
            &mut sink,
        );
        let rects: Vec<Rect> = sink.as_slice()[1..].iter().map(|p| p.rect).collect();
        assert_eq!(
            rects[0],
            Rect::new(0.0, 0.0, 86.0, 20.0),
            "clamped to the run"
        );
        assert_eq!(
            rects[1],
            Rect::new(92.0, 0.0, 40.0, 20.0),
            "neighbour unmoved"
        );
        assert!(
            sink.as_slice()[0].paint.truncated,
            "a child cut down to its run is a truncation the grid must report"
        );
    }

    /// A spanning cell's trailing edge lands on the same device pixel as the
    /// leading edge of the column after it, from an origin that is not on a
    /// pixel boundary. Without this the seam between a span and its neighbour
    /// grows or loses a pixel at fractional scale, which is the same defect
    /// `cumulative_offsets` documents for ordinary adjacent cells.
    #[test]
    fn a_spanning_cell_shares_a_device_edge_with_the_column_after_it() {
        for scale_factor in [1.0_f32, 1.25, 1.5, 2.0] {
            let scale = Scale::new(scale_factor).unwrap();
            let g = fixed_grid(3, 13.3, 0.0, vec![spanning("wide", 2, 1), spacer("c")]);
            let rects = child_rects(&g, Rect::new(7.4, 3.1, 39.9, 20.0));
            let span = round_rect(rects[0], scale);
            let after = round_rect(rects[1], scale);
            assert_eq!(
                span.x + span.w,
                after.x,
                "at scale {scale_factor} the span's trailing edge and its neighbour's leading edge parted"
            );
        }
    }

    #[test]
    fn hand_computed_track_widths_pin_fixed_weight_and_spacing() {
        // Three columns, 10-unit gaps, 420-unit offer: 20 spent on the two
        // gaps, 100 on the fixed track, leaving 300 split 1:2 -> 100 and 200.
        let tracks = vec![
            TrackSize::Fixed { value: 100.0 },
            TrackSize::Weight { weight: 1.0 },
            TrackSize::Weight { weight: 2.0 },
        ];
        let widths = distribute_tracks(&tracks, 10.0, Proposal::Exact(420.0), |_, _| 0.0);
        assert_eq!(widths.extents, vec![100.0, 100.0, 200.0]);
        assert!(!widths.truncated, "400 units plus 20 of gap fits in 420");
    }

    /// The example above, run through the public `measure` entry point
    /// rather than the internal helper, with neutral `Spacer` children (a
    /// spacer under an `Exact` offer answers exactly that offer, so it never
    /// perturbs the column math and its own row collapses to zero height
    /// under the `Unspecified` probe a `FitContent` row gives it).
    #[test]
    fn the_pinned_widths_also_come_out_of_measure() {
        let mut g = grid(
            vec![
                TrackSize::Fixed { value: 100.0 },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 2.0 },
            ],
            vec![spacer("a"), spacer("b"), spacer("c")],
        );
        // Mutate the field directly rather than `.with_props(Props { .. })`:
        // the latter replaces the whole struct, which would silently drop
        // the `columns` the `grid()` helper just set.
        g.props.column_spacing = gap(10.0);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let size = measure(
            &g,
            &mut h.ctx(),
            &mut path,
            SizeProposal::exact(Size::new(420.0, 500.0)),
        );
        assert_eq!(size, Size::new(420.0, 0.0));
    }

    /// The collapse rule stated separately from the general distribution
    /// case: a fixed track alone can eat the whole budget, and the weight
    /// track behind it must land on zero, not swing negative.
    #[test]
    fn weight_columns_with_a_zero_remainder_collapse_to_zero_not_negative() {
        let tracks = vec![
            TrackSize::Fixed { value: 500.0 },
            TrackSize::Weight { weight: 1.0 },
        ];
        let widths = distribute_tracks(&tracks, 0.0, Proposal::Exact(100.0), |_, _| 0.0);
        // The weight track collapses to zero rather than going negative — the
        // property this test was written for. The fixed track is now also cut
        // to the offer: it used to keep its declared 500 in a 100-unit
        // container, which put every later column outside the parent.
        assert_eq!(widths.extents, vec![100.0, 0.0]);
        assert!(widths.truncated, "500 declared units do not fit in 100");
    }

    /// `FitContent` sizing from real child measurements, on both axes: two
    /// text children set column 0's ideal width from their widest unwrapped
    /// run, and the row heights that follow are each cell's wrapped height
    /// at that resolved width — hand-computed from `MonoContent`'s 8
    /// units/char, 16 units/line metric.
    #[test]
    fn fit_content_tracks_size_from_real_child_measurements() {
        // Column 0 (FitContent): "Hi" is 2*8=16 wide, "Hello!" is 6*8=48
        // wide; the column takes the wider one, 48.
        // Column 1 (Fixed): 50, unconditionally.
        // Row 0 ("Hi" at width 48, a spacer): "Hi" fits on one line at
        // width 48 (48/8 = 6 chars/line) -> height 16; the spacer answers
        // an `Unspecified` vertical probe with 0. Row height: 16.
        // Row 1 ("Hello!" at width 48): 6 chars exactly fills 6 chars/line,
        // one line -> height 16.
        let g = grid(
            vec![TrackSize::FitContent, TrackSize::Fixed { value: 50.0 }],
            vec![text("t0", "Hi"), spacer("s0"), text("t1", "Hello!")],
        );
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let size = measure(
            &g,
            &mut h.ctx(),
            &mut path,
            SizeProposal::exact(Size::new(300.0, 1000.0)),
        );
        assert_eq!(size, Size::new(98.0, 32.0));
    }

    /// Row-major placement, pinned exactly: a 2x2 grid of fixed tracks
    /// places its four children left-to-right, top-to-bottom, at the
    /// products of the declared track sizes.
    #[test]
    fn children_fill_cells_row_major_at_the_declared_track_rects() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 50.0 },
                    TrackSize::Fixed { value: 30.0 },
                ],
                rows: vec![
                    TrackSize::Fixed { value: 20.0 },
                    TrackSize::Fixed { value: 40.0 },
                ],
                column_spacing: gap(5.0),
                row_spacing: gap(2.0),
                ..Props::default()
            })
            .with_children(vec![spacer("a"), spacer("b"), spacer("c"), spacer("d")]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 85.0, 62.0)),
            &mut sink,
        );
        let rects: Vec<Rect> = sink.as_slice()[1..].iter().map(|p| p.rect).collect();
        assert_eq!(
            rects,
            vec![
                Rect::new(0.0, 0.0, 50.0, 20.0),   // a: col 0, row 0
                Rect::new(55.0, 0.0, 30.0, 20.0),  // b: col 1, row 0
                Rect::new(0.0, 22.0, 50.0, 40.0),  // c: col 0, row 1
                Rect::new(55.0, 22.0, 30.0, 40.0), // d: col 1, row 1
            ]
        );
    }

    /// The padding-inclusive counterpart to
    /// `children_fill_cells_row_major_at_the_declared_track_rects`: the same
    /// 2x2 fixed grid, now with 10 units of padding on every edge. Every
    /// track lands exactly where it did in the unpadded case, shifted by the
    /// padding's top-left origin — and the grid's own placed rect is still
    /// the full box it was offered, untouched by its own padding.
    #[test]
    fn a_padded_grid_insets_its_tracks() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 50.0 },
                    TrackSize::Fixed { value: 30.0 },
                ],
                rows: vec![
                    TrackSize::Fixed { value: 20.0 },
                    TrackSize::Fixed { value: 40.0 },
                ],
                column_spacing: gap(5.0),
                row_spacing: gap(2.0),
                padding: Some(InsetRefs::all(gap_token(10.0))),
                ..Props::default()
            })
            .with_children(vec![spacer("a"), spacer("b"), spacer("c"), spacer("d")]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        // 85x62 content (same as the unpadded case) plus 10 units of padding
        // on every edge: 105x82.
        let outer = Rect::new(0.0, 0.0, 105.0, 82.0);
        place(&g, &mut h.ctx(), &mut path, Slot::new(outer), &mut sink);

        assert_eq!(
            sink.as_slice()[0].rect,
            outer,
            "the grid's own placed rect is unaffected by its own padding"
        );
        let rects: Vec<Rect> = sink.as_slice()[1..].iter().map(|p| p.rect).collect();
        assert_eq!(
            rects,
            vec![
                Rect::new(10.0, 10.0, 50.0, 20.0), // a: col 0, row 0
                Rect::new(65.0, 10.0, 30.0, 20.0), // b: col 1, row 0
                Rect::new(10.0, 32.0, 50.0, 40.0), // c: col 0, row 1
                Rect::new(65.0, 32.0, 30.0, 40.0), // d: col 1, row 1
            ],
            "every track shifts by exactly the padding's (10, 10) origin \
             relative to the unpadded case"
        );
    }

    /// The padding-inclusive counterpart to `the_pinned_widths_also_come_out_of_measure`:
    /// padding is reserved before the same fixed/weight distribution runs,
    /// and added back onto the measured size afterward.
    #[test]
    fn a_padded_grid_adds_its_padding_to_the_measured_size() {
        let mut g = grid(
            vec![
                TrackSize::Fixed { value: 100.0 },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 2.0 },
            ],
            vec![spacer("a"), spacer("b"), spacer("c")],
        );
        g.props.column_spacing = gap(10.0);
        // 30 units horizontal total (15 + 15), 10 vertical total (5 + 5).
        g.props.padding = Some(crate::testing::gap_insets(Insets::symmetric(15.0, 5.0)));
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let size = measure(
            &g,
            &mut h.ctx(),
            &mut path,
            SizeProposal::exact(Size::new(420.0, 500.0)),
        );
        // Same 420-wide answer as the unpadded case (100 + 20 gap + 300 split
        // 1:2 = 390, plus the 30 units of horizontal padding = 420); height
        // is 0 (every spacer) plus the 10 units of vertical padding.
        assert_eq!(size, Size::new(420.0, 10.0));
    }

    /// The degenerate case symmetric to `stack.rs`'s
    /// `a_stack_too_small_for_its_own_gaps_keeps_its_children_inside`: a grid
    /// offered less room than its own declared padding needs collapses its
    /// content rect to zero (the same floor `Rect::inset_edges` already
    /// applies), reports the truncation, and never places a child with a
    /// negative size or outside its own rect — which is still exactly what
    /// it was offered.
    #[test]
    fn a_grid_too_small_for_its_own_padding_collapses_but_never_goes_negative() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 50.0 }],
                rows: vec![TrackSize::Fixed { value: 40.0 }],
                padding: Some(InsetRefs::all(gap_token(20.0))),
                ..Props::default()
            })
            .with_children(vec![spacer("a")]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        let outer = Rect::new(0.0, 0.0, 30.0, 30.0);
        place(&g, &mut h.ctx(), &mut path, Slot::new(outer), &mut sink);

        let placed = sink.as_slice();
        assert_eq!(
            placed[0].rect, outer,
            "the grid's own rect is unaffected by its own padding"
        );
        assert!(
            placed[0].paint.truncated,
            "a declared 50x40 track cannot fit in a 30x30 box once 20 units \
             of padding are reserved from every edge"
        );
        let child = placed[1].rect;
        assert!(
            child.w >= 0.0 && child.h >= 0.0,
            "collapsed to {child:?}, went negative"
        );
        assert!(
            child.x >= outer.x
                && child.y >= outer.y
                && child.right() <= outer.right() + 1e-3
                && child.bottom() <= outer.bottom() + 1e-3,
            "child at {child:?} outside the {outer:?} grid"
        );
    }

    fn build_tracks(kinds: &[u8], fixed_vals: &[f32], weight_vals: &[f32]) -> Vec<TrackSize> {
        kinds
            .iter()
            .enumerate()
            .map(|(i, k)| match k % 3 {
                0 => TrackSize::Fixed {
                    value: fixed_vals[i % fixed_vals.len()],
                },
                1 => TrackSize::Weight {
                    weight: weight_vals[i % weight_vals.len()],
                },
                _ => TrackSize::FitContent,
            })
            .collect()
    }

    /// A child that answers its intrinsic size whatever it is offered.
    /// `MonoContent::image` ignores the proposal by design, which is what
    /// makes this the right child for testing that a cell clamps.
    fn stubborn_image(key: &str) -> ViewNode {
        ViewNode::new(NodeKind::Image, key).with_props(Props {
            image: Some("logo.png".into()),
            ..Props::default()
        })
    }

    /// Declared tracks that do not fit are cut to the container, and the
    /// container says it cut them.
    ///
    /// A `Fixed` track used to keep its declared value whatever the offer
    /// was, so `columns: [Fixed(500), Fixed(500)]` in a 300-unit grid put the
    /// second child at x = 500 — outside its own parent, overlapping nothing
    /// because there was nothing out there, and reported `truncated: false`.
    #[test]
    fn fixed_tracks_that_do_not_fit_are_cut_and_reported() {
        let g = grid(
            vec![
                TrackSize::Fixed { value: 500.0 },
                TrackSize::Fixed { value: 500.0 },
            ],
            vec![spacer("a"), spacer("b")],
        );
        let mut h = Harness::new();
        let mut ppath = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut ppath,
            Slot::new(Rect::new(0.0, 0.0, 300.0, 100.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        assert!(
            placed[0].paint.truncated,
            "1000 declared units in a 300-unit grid is a truncation: {:?}",
            placed[0].paint
        );
        for child in &placed[1..] {
            assert!(
                child.rect.right() <= 300.0 + 1e-3,
                "{} runs to {} outside a 300-unit grid",
                child.id,
                child.rect.right()
            );
        }
    }

    /// A child that answers larger than its cell is clamped into it, does not
    /// overlap its neighbour, and the grid reports the clamp.
    ///
    /// An `Image` answers its intrinsic size regardless of the offer, and
    /// `Align::Start` — the default, not `Stretch` — used to place that answer
    /// verbatim. A wide image in a narrow column simply covered the next one.
    #[test]
    fn a_child_too_big_for_its_cell_is_clamped_not_allowed_to_overlap() {
        let g = grid(
            vec![
                TrackSize::Fixed { value: 30.0 },
                TrackSize::Fixed { value: 30.0 },
            ],
            vec![stubborn_image("img"), spacer("next")],
        );
        let mut h = Harness::with(
            MonoContent {
                image_size: Size::new(64.0, 20.0),
                ..MonoContent::default()
            },
            NoRows,
        );
        let mut ppath = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut ppath,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        let image = &placed[1];
        let next = &placed[2];
        assert!(
            image.rect.w <= 30.0 + 1e-3,
            "a 64-unit image in a 30-unit column must be clamped, not placed \
             at {}",
            image.rect.w
        );
        assert!(
            !image.rect.overlaps(next.rect),
            "{:?} overlaps {:?}",
            image.rect,
            next.rect
        );
        assert!(
            placed[0].paint.truncated,
            "the grid clamped a child and must say so: {:?}",
            placed[0].paint
        );
    }

    /// A grid honours a child's own `align_self`, the way a stack has since
    /// it was written.
    ///
    /// The case that found the gap: Carbon's inline code snippet is
    /// `display: inline` and declared `align_self: Start` to say so, and
    /// every column in this library is a single-track `Grid` with
    /// `Align::Stretch` — so the chip filled its card and the declaration
    /// went nowhere. Nothing reported it. "Declared and dropped" is not a
    /// shape any gate here looks for, which is why this test is written from
    /// both sides: the opted-out child hugs, and its sibling still stretches.
    #[test]
    fn one_childs_align_self_hugs_while_its_sibling_still_stretches() {
        let mut hugging = spacer("hug").with_constraints(max_on(Axis::Horizontal, 30.0));
        hugging.props.align_self = Some(Align::Start);
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 200.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![hugging, spacer("filling")]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        assert_eq!(
            placed[1].rect.w, 30.0,
            "the child declaring `align_self: Start` still filled its 200-unit \
             cell, so the grid dropped the declaration"
        );
        assert_eq!(
            placed[2].rect.w, 200.0,
            "the sibling that declared nothing must keep the grid's own \
             Stretch"
        );
    }

    fn max_on(axis: Axis, max: f32) -> Constraints {
        let c = AxisConstraint {
            min: None,
            max: Some(max),
            priority: 0,
        };
        match axis {
            Axis::Horizontal => Constraints {
                horizontal: c,
                ..Constraints::default()
            },
            Axis::Vertical => Constraints {
                vertical: c,
                ..Constraints::default()
            },
        }
    }

    /// The 2026-08-22 decision (`.agents/tallies/QUESTIONS.md` Round 3 item 2): a
    /// declared cross-axis maximum beats `Align::Stretch`, in a grid cell
    /// exactly as it does in a stack. Before the fix, `place_in_cell` skipped
    /// measuring a `Stretch` child entirely and placed it at the raw cell
    /// size, so a 30-unit-max spacer in a 200-unit column landed at 200.
    #[test]
    fn a_stretched_cell_stops_at_the_childs_declared_maximum() {
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![TrackSize::Fixed { value: 200.0 }],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![
                spacer("capped").with_constraints(max_on(Axis::Horizontal, 30.0)),
            ]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 50.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        assert_eq!(
            placed[1].rect.w, 30.0,
            "Stretch must stop at the child's declared max, not fill the \
             200-unit cell"
        );
    }

    /// A declared minimum bigger than the cell still must not place the
    /// child wider than the cell it was given — the same rule the non-Stretch
    /// arm of `place_in_cell` already applies via `.min(cell.w)` — and the
    /// grid must say so. `place`'s pre-placement truncation pass detects
    /// this for Stretch from the constraint alone, comparing
    /// `AxisConstraint::clamp` of the cell size against the cell size,
    /// without measuring the child, so `placed[0].paint.truncated` is
    /// `true` here exactly as it would be for any other alignment that lost
    /// the same room.
    #[test]
    fn a_stretched_cell_never_places_wider_than_the_cell_even_with_a_big_minimum() {
        let oversized = AxisConstraint {
            min: Some(300.0),
            max: Some(300.0),
            priority: 0,
        };
        let g = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![
                    TrackSize::Fixed { value: 100.0 },
                    TrackSize::Fixed { value: 100.0 },
                ],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![
                spacer("oversized").with_constraints(Constraints {
                    horizontal: oversized,
                    ..Constraints::default()
                }),
                spacer("next"),
            ]);
        let mut h = Harness::new();
        let mut path = path_at(&g);
        let mut sink = PlacementList::new();
        place(
            &g,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 50.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        assert_eq!(
            placed[1].rect.w, 100.0,
            "a declared min bigger than the cell must not place the child \
             wider than the 100-unit cell"
        );
        assert!(
            !placed[1].rect.overlaps(placed[2].rect),
            "{:?} overlaps {:?}",
            placed[1].rect,
            placed[2].rect
        );
        assert!(
            placed[0].paint.truncated,
            "a Stretch child clamped down by its own declared minimum is \
             still lost room, and the grid must report it: {:?}",
            placed[0].paint
        );
    }

    proptest! {
        // The strategy deliberately reaches the regime the first version
        // scoped away. `fixed_vals` used to be bounded to 1.0..20.0 with a
        // comment arguing that a `Fixed` track is allowed to overflow the
        // offer, so an unbounded generator would make the properties below
        // false "by design, not by bug". That reasoning was wrong: a track
        // wider than its container puts children outside their parent, which
        // `contracts/view-tree.md` forbids outright. Overflow is now cut and
        // reported, so the generator can range over it, and the properties
        // hold everywhere instead of holding in a chosen corner.
        //
        // Children are a mix of spacers (answer exactly what they are
        // offered) and images (answer their intrinsic size and ignore the
        // offer entirely) so the cell-clamping path is generated too.
        #[test]
        fn every_child_gets_one_placement_in_a_non_overlapping_grid_inside_its_parent(
            ncols in 1usize..=4,
            nchildren in 0usize..13,
            kinds in prop::collection::vec(0u8..3, 4),
            fixed_vals in prop::collection::vec(1.0f32..900.0, 4),
            weight_vals in prop::collection::vec(1.0f32..5.0, 4),
            spacing in 0.0f32..24.0,
            offer_w in 40.0f32..1500.0,
            image_every in 1usize..=3,
            padding_amt in 0.0f32..40.0,
        ) {
            let columns = build_tracks(&kinds[..ncols], &fixed_vals, &weight_vals);
            let children: Vec<ViewNode> = (0..nchildren)
                .map(|i| {
                    if i % image_every == 0 {
                        stubborn_image(&format!("c{i}"))
                    } else {
                        spacer(&format!("c{i}"))
                    }
                })
                .collect();
            let mut g = grid(columns, children);
            // The sweep generates gaps and paddings by value, so it binds
            // its own names for them rather than rounding onto either the
            // shipped ramp or the whole-unit fixture scale: the invariants
            // below are arithmetic on the exact generated numbers.
            let swept_gap = crate::token::TokenName::new("spacing.swept-gap").unwrap();
            let swept_pad = crate::token::TokenName::new("spacing.swept-pad").unwrap();
            g.props.column_spacing = Some(swept_gap.clone());
            g.props.row_spacing = Some(swept_gap.clone());

            let mut h = Harness::with(
                MonoContent { image_size: Size::new(120.0, 40.0), ..MonoContent::default() },
                NoRows,
            );
            h.bind_spacings(&[
                (swept_gap.as_str(), spacing),
                (swept_pad.as_str(), padding_amt),
            ]);

            let offer = Rect::new(0.0, 0.0, offer_w, 800.0);
            let mut mpath = path_at(&g);
            let measured = measure(
                &g,
                &mut h.ctx(),
                &mut mpath,
                SizeProposal::exact(offer.size()),
            );
            prop_assert!(measured.w <= offer_w + 1e-3, "grid reported {} wider than the {} offer", measured.w, offer_w);
            prop_assert!(measured.h <= 800.0 + 1e-3);

            // Padding is applied only from here on: `place()` re-resolves
            // independently of the `measure()` call above (it does not trust
            // a cached answer, per its own doc comment), so this is the
            // no-op-to-`measure` way to prove `place`'s own invariants below
            // — no child outside its parent, no overlap — still hold once
            // padding is threaded through track sizing. `padding_amt` is
            // always strictly less than `offer_w` (40.0..1500.0 vs
            // 0.0..40.0), so the inset content rect is always inside the
            // offer with room to spare; this generator is not meant to
            // exercise the separate degenerate-collapse case, which
            // `a_grid_too_small_for_its_own_padding_collapses_but_never_goes_negative`
            // already pins by hand.
            g.props.padding = Some(InsetRefs::all(swept_pad));

            let mut ppath = path_at(&g);
            let mut sink = PlacementList::new();
            place(&g, &mut h.ctx(), &mut ppath, Slot::new(offer), &mut sink);

            // Every child gets exactly one placement (plus the grid's own).
            prop_assert_eq!(sink.len(), 1 + nchildren);

            let rects: Vec<Rect> = sink.as_slice()[1..].iter().map(|p| p.rect).collect();
            for (i, rect) in rects.iter().enumerate() {
                // Inside the parent, on both axes. This is the property the
                // old bounded strategy could not have falsified.
                prop_assert!(
                    rect.right() <= offer.right() + 1e-3 && rect.bottom() <= offer.bottom() + 1e-3
                        && rect.x >= -1e-3 && rect.y >= -1e-3,
                    "child {i} at {rect:?} is outside its {offer:?} parent"
                );
                // No two children overlap, in any row, not just within one.
                for (j, other) in rects.iter().enumerate() {
                    if i != j {
                        prop_assert!(!rect.overlaps(*other), "{rect:?} overlaps {other:?}");
                    }
                }
                let row = i / ncols;
                if i + 1 < rects.len() && (i + 1) / ncols == row {
                    prop_assert!(rects[i + 1].x >= rect.x, "row {row}: x went backwards");
                }
            }
        }
    }
}
