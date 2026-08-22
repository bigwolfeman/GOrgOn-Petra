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
use crate::geom::{Align, Rect, Size};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot};
use crate::tree::{KeyPath, TrackSize, ViewNode};

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
    let props = node.props.grid();
    let ncols = props.columns.len();
    if ncols == 0 {
        // Tree acceptance is expected to refuse a grid with no declared
        // columns before layout ever sees it; this is the defensive floor
        // for a container called out of turn, not a shape the contract
        // describes.
        return Size::ZERO;
    }
    let row_tracks = effective_row_tracks(&props.rows, node.children.len(), ncols);
    let col_widths = resolve_columns(
        node,
        ctx,
        path,
        &props.columns,
        props.column_spacing,
        proposal.horizontal,
        ncols,
    );
    let row_heights = resolve_rows(
        node,
        ctx,
        path,
        &row_tracks,
        props.row_spacing,
        proposal.vertical,
        (ncols, &col_widths),
    );
    let w = col_widths.iter().sum::<f32>() + reserved(props.column_spacing, ncols);
    let h = row_heights.iter().sum::<f32>() + reserved(props.row_spacing, row_tracks.len());
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
    let me = sink.push(Placement {
        id: path.id(),
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
            truncated: false,
            token_revision: ctx.theme_rev,
        },
        semantics: crate::layout::semantics_of(node),
        parent: None,
    });
    sink.enter(me);

    let props = node.props.grid();
    let ncols = props.columns.len();
    if ncols > 0 && !node.children.is_empty() {
        // Re-resolve against the rect this call actually received, the same
        // way `text::place` re-measures rather than trusting an earlier
        // probe: a parent may have probed several proposals before
        // committing this one.
        let offer = SizeProposal::exact(slot.rect.size());
        let row_tracks = effective_row_tracks(&props.rows, node.children.len(), ncols);
        let col_widths = resolve_columns(
            node,
            ctx,
            path,
            &props.columns,
            props.column_spacing,
            offer.horizontal,
            ncols,
        );
        let row_heights = resolve_rows(
            node,
            ctx,
            path,
            &row_tracks,
            props.row_spacing,
            offer.vertical,
            (ncols, &col_widths),
        );
        let col_x = cumulative_offsets(&col_widths, props.column_spacing);
        let row_y = cumulative_offsets(&row_heights, props.row_spacing);

        for (i, child) in node.children.iter().enumerate() {
            let col = i % ncols;
            let row = i / ncols;
            // `effective_row_tracks` always grows to fit every child, so this
            // is in range today; the guard is the floor against a future
            // change to that invariant, not a case this can reach now.
            let (Some(&x), Some(&y), Some(&w), Some(&h)) = (
                col_x.get(col),
                row_y.get(row),
                col_widths.get(col),
                row_heights.get(row),
            ) else {
                continue;
            };
            let cell = Rect::new(slot.rect.x + x, slot.rect.y + y, w, h);
            place_in_cell(child, ctx, path, cell, props.align, slot, sink);
        }
    }

    sink.leave();
}

/// Reserved spacing for `n` tracks: `n - 1` gaps, never negative.
fn reserved(spacing: f32, n: usize) -> f32 {
    spacing.max(0.0) * n.saturating_sub(1) as f32
}

/// Row tracks after the implicit-row rule: declared rows are used as given,
/// then grown with `FitContent` — the same sizing an implicit row gets when
/// none are declared — until every child has a row to land in row-major
/// order. A tree that declares more rows than its children need is left
/// exactly as declared; this only ever adds rows, never removes one.
fn effective_row_tracks(rows: &[TrackSize], child_count: usize, ncols: usize) -> Vec<TrackSize> {
    let needed = if ncols == 0 {
        0
    } else {
        child_count.div_ceil(ncols)
    };
    let mut out = rows.to_vec();
    while out.len() < needed {
        out.push(TrackSize::FitContent);
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
) -> Vec<f32> {
    let n = tracks.len();
    if n == 0 {
        return Vec::new();
    }

    let Some(avail) = container_probe.available() else {
        // Open probe (`Unbounded`/`Unspecified`): there is no total to
        // divide, so every track reports its own extent under the same
        // probe the container itself was asked. `Fixed` never asks content;
        // `Weight` has no "leftover" to be proportional to without a budget,
        // so it falls back to the same natural-extent read as `FitContent`.
        return tracks
            .iter()
            .enumerate()
            .map(|(i, track)| match track {
                TrackSize::Fixed { value } => value.max(0.0),
                TrackSize::FitContent | TrackSize::Weight { .. } => {
                    natural(i, container_probe).max(0.0)
                }
            })
            .collect();
    };

    // Closed probe (`Exact` or `Zero`, both answered by `.available()`): a
    // real budget to divide, left to right. `Fixed` takes its value
    // unclamped — a fixed track may overflow the offer, same as any child's
    // response may differ from what it was offered
    // (`contracts/view-tree.md`'s negotiation algorithm). Only `FitContent`
    // is explicitly clamped by the contract; `Weight` is self-limiting, since
    // it only ever draws from what running `budget` has left.
    let mut budget = (avail - reserved(spacing, n)).max(0.0);
    let mut widths = vec![0.0_f32; n];
    let mut weight_total = 0.0_f32;
    for (i, track) in tracks.iter().enumerate() {
        match track {
            TrackSize::Fixed { value } => {
                let w = value.max(0.0);
                widths[i] = w;
                budget = (budget - w).max(0.0);
            }
            TrackSize::FitContent => {
                let ideal = natural(i, Proposal::Unspecified).max(0.0);
                let w = ideal.min(budget);
                widths[i] = w;
                budget = (budget - w).max(0.0);
            }
            TrackSize::Weight { weight } => {
                weight_total += weight.max(0.0);
            }
        }
    }
    if weight_total > 0.0 {
        for (i, track) in tracks.iter().enumerate() {
            if let TrackSize::Weight { weight } = track {
                // `budget` can never go negative (every subtraction above
                // floors at zero), so a weight track's share never goes
                // negative either — it collapses to zero, not overflow.
                widths[i] = (budget * (weight.max(0.0) / weight_total)).max(0.0);
            }
        }
    }
    widths
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
    ncols: usize,
) -> Vec<f32> {
    distribute_tracks(tracks, spacing, container_probe, |col, probe| {
        column_natural_width(node, ctx, path, col, ncols, probe)
    })
}

/// Row heights for `tracks` against `container_probe` (the container's
/// vertical proposal), given the column widths already resolved.
fn resolve_rows(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    tracks: &[TrackSize],
    spacing: f32,
    container_probe: Proposal,
    cells: (usize, &[f32]),
) -> Vec<f32> {
    let (ncols, col_widths) = cells;
    distribute_tracks(tracks, spacing, container_probe, |row, probe| {
        row_natural_height(node, ctx, path, row, ncols, col_widths, probe)
    })
}

/// The widest response among column `col`'s children to `probe` on the
/// horizontal axis. The vertical axis is always `Unspecified`: a column's
/// width does not yet know what height its row will settle on.
fn column_natural_width(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    col: usize,
    ncols: usize,
    probe: Proposal,
) -> f32 {
    let proposal = SizeProposal {
        horizontal: probe,
        vertical: Proposal::Unspecified,
    };
    node.children
        .iter()
        .enumerate()
        .filter(|(i, _)| i % ncols == col)
        .map(|(_, child)| crate::layout::measure(child, ctx, path, proposal).w)
        .fold(0.0_f32, f32::max)
}

/// The tallest response among row `row`'s children to `probe` on the vertical
/// axis. Each cell's horizontal axis is `Exact` at its own column's already-
/// resolved width, per the contract's row rule.
fn row_natural_height(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    row: usize,
    ncols: usize,
    col_widths: &[f32],
    probe: Proposal,
) -> f32 {
    let mut tallest = 0.0_f32;
    for (i, child) in node.children.iter().enumerate() {
        if i / ncols != row {
            continue;
        }
        let w = col_widths.get(i % ncols).copied().unwrap_or(0.0);
        let proposal = SizeProposal {
            horizontal: Proposal::Exact(w),
            vertical: probe,
        };
        tallest = tallest.max(crate::layout::measure(child, ctx, path, proposal).h);
    }
    tallest
}

/// Leading offsets for a run of track sizes plus fixed spacing between them.
fn cumulative_offsets(sizes: &[f32], spacing: f32) -> Vec<f32> {
    let spacing = spacing.max(0.0);
    let mut offsets = Vec::with_capacity(sizes.len());
    let mut acc = 0.0_f32;
    for &size in sizes {
        offsets.push(acc);
        acc += size.max(0.0) + spacing;
    }
    offsets
}

/// Place one child into its resolved `cell`, per `GridProps::align`.
///
/// A cell is a box the child may be smaller than: every non-`Stretch`
/// alignment measures the child against the cell size (it may answer
/// smaller — a parent places, it does not force) and offsets the answer
/// inside the box with [`Align::offset`]. `Stretch` skips the measurement
/// and fills the cell outright, which is what "offers `Proposal::Exact` on
/// that axis" means for the one alignment that never leaves slack to offset.
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
        crate::layout::place(child, ctx, path, slot.with_rect(cell), sink);
        return;
    }
    let response = crate::layout::measure(child, ctx, path, SizeProposal::exact(cell.size()));
    let dx = align.offset(cell.w, response.w);
    let dy = align.offset(cell.h, response.h);
    let rect = Rect::new(cell.x + dx, cell.y + dy, response.w, response.h);
    crate::layout::place(child, ctx, path, slot.with_rect(rect), sink);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::placement::PlacementList;
    use crate::testing::Harness;
    use crate::tree::{NodeKind, Props};
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
        assert_eq!(widths, vec![100.0, 100.0, 200.0]);
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
        g.props.column_spacing = Some(10.0);
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
        assert_eq!(widths, vec![500.0, 0.0]);
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
                column_spacing: Some(5.0),
                row_spacing: Some(2.0),
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

    proptest! {
        // `fixed_vals` and `ncols` are bounded so the worst-case total
        // fixed-track width (4 tracks * 20.0) sits far under the smallest
        // generated offer (400.0) even after spacing. The contract lets a
        // `Fixed` track overflow the offer on purpose (`hand_computed_...`
        // above exercises exactly that), so an *unbounded* fixed generator
        // would make the "never exceeds the offer" property below false by
        // design, not by bug. This strategy is scoped to the regime where
        // that overflow allowance never triggers, and says so.
        #[test]
        fn every_child_gets_one_placement_in_a_non_overlapping_non_decreasing_grid(
            ncols in 1usize..=4,
            nchildren in 0usize..13,
            kinds in prop::collection::vec(0u8..3, 4),
            fixed_vals in prop::collection::vec(1.0f32..20.0, 4),
            weight_vals in prop::collection::vec(1.0f32..5.0, 4),
            spacing in 0.0f32..5.0,
            offer_w in 400.0f32..1500.0,
        ) {
            let columns = build_tracks(&kinds[..ncols], &fixed_vals, &weight_vals);
            let children: Vec<ViewNode> = (0..nchildren).map(|i| spacer(&format!("c{i}"))).collect();
            let mut g = grid(columns, children);
            g.props.column_spacing = Some(spacing);
            g.props.row_spacing = Some(spacing);

            let mut h = Harness::new();

            // The width property: a grid built from this bounded strategy
            // never reports wider than the exact offer it was measured
            // against.
            let mut mpath = path_at(&g);
            let measured = measure(
                &g,
                &mut h.ctx(),
                &mut mpath,
                SizeProposal::exact(Size::new(offer_w, 800.0)),
            );
            prop_assert!(measured.w <= offer_w + 1e-3, "grid reported {} wider than the {} offer", measured.w, offer_w);
            // Implicit rows are always `FitContent`, which is always clamped
            // to the remaining vertical budget, so the same bound holds on
            // height with no extra scoping needed.
            prop_assert!(measured.h <= 800.0 + 1e-3);

            let mut ppath = path_at(&g);
            let mut sink = PlacementList::new();
            place(&g, &mut h.ctx(), &mut ppath, Slot::new(Rect::new(0.0, 0.0, offer_w, 800.0)), &mut sink);

            // Every child gets exactly one placement (plus the grid's own).
            prop_assert_eq!(sink.len(), 1 + nchildren);

            let rects: Vec<Rect> = sink.as_slice()[1..].iter().map(|p| p.rect).collect();
            for (i, rect) in rects.iter().enumerate() {
                let row = i / ncols;
                // No two children in the same row overlap.
                for (j, other) in rects.iter().enumerate() {
                    if j / ncols == row && i != j {
                        prop_assert!(!rect.overlaps(*other), "row {row}: {rect:?} overlaps {other:?}");
                    }
                }
                // Column x-positions are non-decreasing left to right within
                // a row: consecutive children in the same row are declared
                // in ascending column order.
                if i + 1 < rects.len() && (i + 1) / ncols == row {
                    prop_assert!(rects[i + 1].x >= rect.x, "row {row}: x went backwards");
                }
            }
        }
    }
}
