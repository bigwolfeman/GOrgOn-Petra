//! The US1 acceptance matrix: one panel, every container kind, swept.
//!
//! `specs/003-petra-layout-engine/spec.md` US1 says it plainly: "Build a test
//! panel exercising every container kind and constraint; render it across a
//! matrix of sizes, scales, and content lengths; assert every placement obeys
//! the negotiation rules and the frame digest is stable per configuration."
//!
//! Every other layout test in this crate drives **one** container through
//! `crate::layout::place` by hand. That is the right shape for pinning an
//! algorithm and the wrong shape for three things:
//!
//! * **Composition.** A stack inside a grid inside an overlay negotiates
//!   through three different modules; a bug that only appears when a
//!   container's answer becomes another container's offer cannot be seen from
//!   inside either one.
//! * **Device space.** Layout is logical units end to end; `round_rect` turns
//!   them into pixels once, at petrify. `frame/rounding.rs` proves that eight
//!   *identical* rows at x = 0 tile at 1.25. It cannot prove that the rects a
//!   real negotiation produces — at fractional origins, after a concession —
//!   still tile. The seam and overlap assertions here run on `DeviceRect`, not
//!   on `Rect`.
//! * **Identity per configuration.** FR-006 is a claim about the whole matrix,
//!   not about one tree: the same configuration must petrify to the same
//!   digest, and two configurations that draw different pictures must not
//!   share one.
//!
//! # What this file sweeps, and what it samples
//!
//! Exhaustive: 6 viewport sizes × 5 display scales × 3 content lengths × 4
//! cross-axis alignments = 360 configurations, each petrified twice.
//! Sampled by rotation over that sweep, not by product: stack/grid spacing (2
//! values) and scroll offset (3 values). Rotating rather than multiplying
//! keeps the suite in the low seconds; every spacing meets every size, scale,
//! content length and alignment somewhere in the sweep, but not every triple
//! of the three sampled axes occurs.
//!
//! Deliberately outside the matrix, because they need a tree built to expose
//! them rather than a tree built to be representative: the FR-005 concession
//! order at three regimes, grid cell tiling, the seam property under
//! `proptest`, and scale invariance.
//!
//! # What the matrix cannot see
//!
//! [`MonoContent`] is a fixed-pitch measurer that ignores display scale, so
//! nothing here can catch a shaper that re-rasterizes wrongly at 1.75 — that
//! is a `gorgon-petra-egui` test. The semantic tree (T027) is not written yet,
//! so FR-020's "the semantic tree records that truncation occurred" is
//! asserted one layer down, on `Placement::paint.truncated`, which is the
//! field that projection will read.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use gorgon_petra::frame::{
    DeviceRect, FrameDigest, PetrifiedFrame, Placement, TransitionActivity, Viewport, petrify,
    round_rect,
};
use gorgon_petra::geom::{Align, Axis, Rect, Scale, Size};
use gorgon_petra::layout::LayoutState;
use gorgon_petra::testing::{GeneratedRows, Harness, MonoContent, validated};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, Key, KeyPath, Layer, NodeKind, Props, Registry,
    TextWrap, TrackSize, ViewNode, validate,
};
use proptest::prelude::*;

/// Slack for logical-unit comparisons. Extents are sums of measured floats;
/// a whole logical unit of drift would be a bug, a thousandth is arithmetic.
const EPS: f32 = 1e-3;

/// Rows behind the virtualized list. Large enough that a frame containing all
/// of them would be unmistakable in the placement count.
const TOTAL_ROWS: usize = 100_000;
/// Declared row extent, and the divisor the materialization window uses.
const ROW_EXTENT: f32 = 24.0;
/// Overscan declared on the `scroll`, not on the `collection` it carries.
const OVERSCAN: f32 = 48.0;
/// Declared horizontal maximum of the header badge.
const BADGE_MAX_W: f32 = 24.0;
/// Declared horizontal maximum of the metadata grid.
const META_MAX_W: f32 = 200.0;
/// Declared minimum extents of the scrolling list.
const LIST_MIN_H: f32 = 60.0;
const LIST_MIN_W: f32 = 96.0;

// ---------------------------------------------------------------------------
// The configuration matrix
// ---------------------------------------------------------------------------

/// One cell of the matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Config {
    /// Logical viewport size.
    viewport: Size,
    /// Display scale.
    scale: f32,
    /// Characters in the title and footer runs. At 8 units a character this
    /// is what decides whether text fits.
    content: usize,
    /// Cross-axis alignment of the panel and of the grid's cells.
    align: Align,
    /// Gap between stack children and between grid tracks.
    spacing: f32,
    /// Scroll offset applied to the list.
    offset: f32,
}

impl Config {
    /// A one-line name for a failure message. Every assertion in this file
    /// carries one, because "assertion failed" over 360 configurations is not
    /// a bug report.
    fn label(&self) -> String {
        format!(
            "{}x{} @{} content={} align={:?} spacing={} offset={}",
            self.viewport.w,
            self.viewport.h,
            self.scale,
            self.content,
            self.align,
            self.spacing,
            self.offset
        )
    }

    fn scale(&self) -> Scale {
        Scale::new(self.scale).expect("matrix scales are positive and finite")
    }
}

/// Viewport sizes. Two are deliberately not whole numbers and one is a prime
/// pair: a matrix of round sizes at round scales rounds to whole pixels and
/// proves nothing about `round_rect`.
const SIZES: &[Size] = &[
    Size { w: 320.0, h: 240.0 },
    Size { w: 401.0, h: 307.0 },
    Size { w: 640.0, h: 480.0 },
    Size { w: 137.5, h: 99.25 },
    Size {
        w: 1024.0,
        h: 768.0,
    },
    Size { w: 96.0, h: 64.0 },
];

/// Display scales. 1.75 is the awkward one the task names; 1.0 and 2.0 bracket
/// it with the two factors that cannot produce a half-pixel from a whole
/// logical unit.
const SCALES: &[f32] = &[1.0, 1.25, 1.5, 1.75, 2.0];

/// Scales the seam property runs at. Wider than the matrix's, and deliberately
/// awkward: 4/3 has no finite binary expansion at all, and 2.5 and 3.0 turn a
/// tenth of a logical unit into a quarter and a third of a pixel.
const SEAM_SCALES: &[f32] = &[1.0, 1.25, 1.333_333_3, 1.5, 1.75, 2.0, 2.5, 3.0];

/// Content lengths in characters: fits anywhere, fits some places, fits
/// nowhere.
const CONTENTS: &[usize] = &[2, 40, 400];

const ALIGNS: &[Align] = &[Align::Start, Align::Center, Align::End, Align::Stretch];

/// Sampled by rotation, not multiplied into the product. See the module doc.
const SPACINGS: &[f32] = &[0.0, 7.0];
const OFFSETS: &[f32] = &[0.0, 96.0, 250_000.0];

/// The whole matrix, in a fixed order so a failure names a reproducible cell.
fn matrix() -> Vec<Config> {
    let mut out = Vec::new();
    let mut n = 0usize;
    for &viewport in SIZES {
        for &scale in SCALES {
            for &content in CONTENTS {
                for &align in ALIGNS {
                    out.push(Config {
                        viewport,
                        scale,
                        content,
                        align,
                        spacing: SPACINGS[n % SPACINGS.len()],
                        offset: OFFSETS[n % OFFSETS.len()],
                    });
                    n += 1;
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The panel under test
// ---------------------------------------------------------------------------

/// Canonical id of the scrolling list, and so the key its offset is stored
/// under.
const LIST_ID: &str = "/root/panel/body/sidebar/list";

fn text_node(key: &str, body: &str, wrap: TextWrap) -> ViewNode {
    ViewNode::new(NodeKind::Text, Key::new(key)).with_props(Props {
        text: Some(body.to_owned()),
        wrap: Some(wrap),
        ..Props::default()
    })
}

fn stack_node(key: &str, axis: Axis, spacing: f32, align: Align) -> ViewNode {
    ViewNode::new(NodeKind::Stack, Key::new(key)).with_props(Props {
        axis: Some(axis),
        spacing: Some(spacing),
        align: Some(align),
        ..Props::default()
    })
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

/// The panel: every container kind Petra has, composed the way a real surface
/// composes them.
///
/// ```text
/// overlay  root                     the frame: panel plus floating surface
/// ├ stack  panel      vertical      header / rule / body / footer
/// │ ├ stack  header   horizontal    title, flexible gap, badge
/// │ ├ separator rule                a rigid one-unit child
/// │ ├ stack  body     horizontal    grid beside sidebar
/// │ │ ├ grid   meta                 Fixed(48) + Weight(1), four cells
/// │ │ └ stack  sidebar vertical     overlay above a scroll region
/// │ │   ├ overlay badges            two layered labels
/// │ │   └ scroll  list  vertical    min-constrained, so it can absorb
/// │ │     └ collection rows         100 000 rows, virtualized
/// │ └ text   footer                 wrapping text
/// └ surface popup                   Point-anchored, Flip-clamped
/// ```
fn panel(cfg: &Config) -> ViewNode {
    let run = "T".repeat(cfg.content);

    let header = stack_node("header", Axis::Horizontal, cfg.spacing, Align::Stretch)
        .child(text_node("title", &run, TextWrap::Ellipsis))
        .child(ViewNode::new(NodeKind::Spacer, "gap"))
        .child(
            text_node("badge", "OK", TextWrap::Clip)
                .with_constraints(max_on(Axis::Horizontal, BADGE_MAX_W)),
        );

    let rule = ViewNode::new(NodeKind::Separator, "rule").with_props(Props {
        axis: Some(Axis::Horizontal),
        ..Props::default()
    });

    let meta = ViewNode::new(NodeKind::Grid, "meta")
        .with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: 48.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            column_spacing: Some(cfg.spacing),
            row_spacing: Some(cfg.spacing),
            align: Some(cfg.align),
            ..Props::default()
        })
        .with_constraints(max_on(Axis::Horizontal, META_MAX_W))
        .child(text_node("k0", "state", TextWrap::Clip))
        .child(text_node("v0", &run, TextWrap::Ellipsis))
        // An `Image` answers its intrinsic size whatever the offer, so it is
        // the cell child that can overflow its cell. Without one, every cell
        // child here is text that measures itself down to the offer and the
        // grid's clamp is never exercised.
        .child(ViewNode::new(NodeKind::Image, "icon").with_props(Props {
            image: Some("fiber.svg".into()),
            ..Props::default()
        }))
        .child(text_node("v1", "kernel", TextWrap::Ellipsis));

    let badges = ViewNode::new(NodeKind::Overlay, "badges")
        .child(text_node("under", "background", TextWrap::Clip))
        .child(text_node("over", "!", TextWrap::Clip));

    let list = ViewNode::new(NodeKind::Scroll, "list")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            overscan: Some(OVERSCAN),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(LIST_MIN_W),
                max: None,
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(LIST_MIN_H),
                max: None,
                priority: 0,
            },
        })
        .child(
            ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                total_count: Some(TOTAL_ROWS),
                source: Some("fibers".into()),
                estimated_extent: Some(ROW_EXTENT),
                ..Props::default()
            }),
        );

    let sidebar = stack_node("sidebar", Axis::Vertical, cfg.spacing, Align::Stretch)
        .child(badges)
        .child(list);

    let body = stack_node("body", Axis::Horizontal, cfg.spacing, Align::Stretch)
        .child(meta)
        .child(sidebar);

    let panel = stack_node("panel", Axis::Vertical, cfg.spacing, cfg.align)
        .child(header)
        .child(rule)
        .child(body)
        .child(text_node("footer", &run, TextWrap::Wrap));

    let popup = ViewNode::new(NodeKind::Surface, "popup")
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Point { x: 240.0, y: 180.0 }),
            clamp: Some(ClampRule::Flip),
            ..Props::default()
        })
        .child(text_node(
            "tip",
            "unload blocks on 2 dependents",
            TextWrap::Wrap,
        ));

    ViewNode::new(NodeKind::Overlay, "root")
        .child(panel)
        .child(popup)
}

/// Petrify one configuration from a cold harness.
fn frame_of(cfg: &Config) -> PetrifiedFrame {
    let mut harness = harness_for(cfg);
    petrify_with(&mut harness, cfg)
}

fn harness_for(cfg: &Config) -> Harness<MonoContent, GeneratedRows> {
    let mut harness = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", TOTAL_ROWS));
    // The harness scale is the measurement cache's scale key; the viewport
    // scale is the digest's. A test that moved one without the other would be
    // measuring one frame and hashing another.
    harness.scale = cfg.scale();
    harness.state = LayoutState::default();
    harness.set_scroll(LIST_ID, cfg.offset);
    harness
}

fn petrify_with(harness: &mut Harness<MonoContent, GeneratedRows>, cfg: &Config) -> PetrifiedFrame {
    let tree = panel(cfg);
    petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(cfg.viewport, ThemeMode::Dark).with_scale(cfg.scale()),
        TransitionActivity::default(),
    )
}

// ---------------------------------------------------------------------------
// What the tree says about each container, for the frame to be judged against
// ---------------------------------------------------------------------------

/// The negotiation contract one container imposes on its children.
#[derive(Clone, Copy, Debug)]
struct ContainerRule {
    kind: NodeKind,
    /// Main axis, for a `stack`. Meaningless for the other kinds.
    axis: Axis,
    /// Cross-axis alignment, for a `stack` or a `grid`.
    align: Align,
    /// Declared gap. Meaningless for the kinds that have none.
    spacing: f32,
    /// Declared column count, for a `grid`. Zero for every other kind.
    ncols: usize,
}

/// Index every container in the tree by its canonical placement id.
///
/// Read from the *tree*, never from the frame: a frame that lost a container's
/// axis is exactly the bug these assertions exist to catch, so the expectation
/// has to come from the declaration.
fn container_rules(root: &ViewNode) -> BTreeMap<String, ContainerRule> {
    fn walk(node: &ViewNode, path: &mut KeyPath, out: &mut BTreeMap<String, ContainerRule>) {
        path.push(node.key.clone());
        if node.kind.is_container() {
            let stack = node.props.stack();
            let grid = node.props.grid();
            out.insert(
                path.id(),
                ContainerRule {
                    kind: node.kind,
                    axis: stack.axis,
                    align: if node.kind == NodeKind::Grid {
                        grid.align
                    } else {
                        stack.align
                    },
                    spacing: if node.kind == NodeKind::Grid {
                        grid.column_spacing
                    } else {
                        stack.spacing
                    },
                    ncols: if node.kind == NodeKind::Grid {
                        grid.columns.len()
                    } else {
                        0
                    },
                },
            );
        }
        for child in &node.children {
            walk(child, path, out);
        }
        path.pop();
    }
    let mut out = BTreeMap::new();
    walk(root, &mut KeyPath::root(), &mut out);
    out
}

// ---------------------------------------------------------------------------
// Geometry helpers
// ---------------------------------------------------------------------------

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.right() <= outer.right() + EPS
        && inner.bottom() <= outer.bottom() + EPS
}

fn is_empty(rect: Rect) -> bool {
    rect.w <= EPS || rect.h <= EPS
}

/// `(origin, end)` of a device rect along one axis.
fn device_span(rect: DeviceRect, axis: Axis) -> (i32, i32) {
    match axis {
        Axis::Horizontal => (rect.x, rect.right()),
        Axis::Vertical => (rect.y, rect.bottom()),
    }
}

fn logical_span(rect: Rect, axis: Axis) -> (f32, f32) {
    match axis {
        Axis::Horizontal => (rect.x, rect.right()),
        Axis::Vertical => (rect.y, rect.bottom()),
    }
}

/// Device rects never overlap, on either axis, unless one of them is empty.
fn device_overlaps(a: DeviceRect, b: DeviceRect) -> bool {
    if a.w == 0 || a.h == 0 || b.w == 0 || b.h == 0 {
        return false;
    }
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Children of each placement, by index, in placement order.
fn children_by_parent(placements: &[Placement]) -> BTreeMap<usize, Vec<usize>> {
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, p) in placements.iter().enumerate() {
        if let Some(parent) = p.parent {
            out.entry(parent).or_default().push(i);
        }
    }
    out
}

/// Whether each placement is inside the subtree rooted at `root`.
///
/// Placements arrive in tree pre-order and every parent index is smaller than
/// its child's, so one forward pass settles it.
fn subtree_mask(placements: &[Placement], root: usize) -> Vec<bool> {
    let mut mask = vec![false; placements.len()];
    mask[root] = true;
    for (i, p) in placements.iter().enumerate().skip(root + 1) {
        if let Some(parent) = p.parent {
            mask[i] = mask[parent];
        }
    }
    mask
}

// ---------------------------------------------------------------------------
// The invariants, checked on one petrified frame
// ---------------------------------------------------------------------------

/// Every assertion the matrix makes about one frame.
fn check_frame(cfg: &Config, frame: &PetrifiedFrame, rules: &BTreeMap<String, ContainerRule>) {
    let where_ = cfg.label();
    let placements = &frame.placements;
    let scale = frame.viewport.scale;

    assert!(
        !placements.is_empty(),
        "{where_}: a panel petrified to no placements at all"
    );
    assert_eq!(
        placements.len(),
        frame.content.len(),
        "{where_}: placements and paint payloads must stay at equal length"
    );
    assert!(
        frame.paint_hashes_agree(),
        "{where_}: a placement's paint hash disagrees with the payload it got"
    );

    // --- I1. One placement per node, pre-order, unique ids -----------------
    let mut seen = BTreeSet::new();
    for (i, p) in placements.iter().enumerate() {
        assert!(
            seen.insert(p.id.clone()),
            "{where_}: {} was placed twice; a node gets exactly one placement \
             per frame (FR-002)",
            p.id
        );
        match p.parent {
            None => assert_eq!(i, 0, "{where_}: {} is a second root", p.id),
            Some(parent) => assert!(
                parent < i,
                "{where_}: {} names a parent at {parent} that comes after it; \
                 placements are not in pre-order",
                p.id
            ),
        }
    }

    // --- I2. Virtualization is in force -----------------------------------
    // A frame describing 100 000 rows must not contain 100 000 placements.
    assert!(
        placements.len() < 120,
        "{where_}: {} placements for a {TOTAL_ROWS}-row list; the collection \
         is materializing more than its window (FR-009)",
        placements.len()
    );

    // --- I3. Clips only ever narrow ---------------------------------------
    for p in placements.iter() {
        let Some(parent) = p.parent.map(|i| &placements[i]) else {
            continue;
        };
        if is_empty(p.clip) {
            // Nothing of this node is visible; an empty clip carries the
            // origin of the disjointness, not a containable rect.
            continue;
        }
        assert!(
            contains(parent.clip, p.clip),
            "{where_}: {}'s clip {:?} escapes its parent {}'s clip {:?}; a \
             container may narrow the clip in force, never widen it",
            p.id,
            p.clip,
            parent.id,
            parent.clip
        );
    }

    // --- I3b. A scroll bounds everything under it to its own rect ---------
    // The narrowing rule above cannot see this on its own: a `scroll` that
    // simply forwarded the clip it was handed would still be *narrowing*,
    // while its content — deliberately placed at full content size, outside
    // the scroll's rect — painted straight over its neighbours. What makes
    // overflow "reachable but hidden" rather than "painted outside" is that
    // the clip in force under a `scroll` is the scroll's own rect.
    for (i, scroller) in placements.iter().enumerate() {
        if scroller.kind != NodeKind::Scroll {
            continue;
        }
        let mask = subtree_mask(placements, i);
        for (d, inside) in placements.iter().zip(&mask) {
            if !inside || d.id == scroller.id || is_empty(d.clip) {
                continue;
            }
            assert!(
                contains(scroller.rect, d.clip),
                "{where_}: {} under the scroll {} is clipped to {:?}, which \
                 escapes the scroll's own rect {:?}; a scroll's overflow must \
                 be hidden, not painted over its neighbours",
                d.id,
                scroller.id,
                d.clip,
                scroller.rect
            );
        }
    }

    // --- I4/I5/I6. Per-container contracts --------------------------------
    let kids = children_by_parent(placements);
    for (parent_index, child_indices) in &kids {
        let parent = &placements[*parent_index];
        let Some(rule) = rules.get(&parent.id) else {
            // A materialized collection row's parent is the collection, which
            // is in the tree; anything not in the tree is a row, and rows have
            // no children.
            continue;
        };
        match rule.kind {
            NodeKind::Stack => {
                check_stack(&where_, placements, parent, child_indices, *rule, scale)
            }
            NodeKind::Grid => check_grid(&where_, placements, parent, child_indices, *rule, scale),
            NodeKind::Overlay => check_overlay(&where_, placements, child_indices),
            // A `scroll` deliberately places its child at full content size
            // outside its own rect and clips it back; a `collection` places
            // rows across a content-sized rect; a `surface` floats free of the
            // slot it was handed. All three are checked by the clip rule
            // above, and none of them owes a no-overlap contract.
            _ => {}
        }
    }

    // --- I7. Declared maxima are honoured ---------------------------------
    // Both nodes take their maximum along their parent stack's *main* axis,
    // which is the axis a stack distributes and concedes on. A concession only
    // ever lowers an extent, so the clamped response is an upper bound on the
    // placement whatever the fit.
    for (id, max) in [
        ("/root/panel/header/badge", BADGE_MAX_W),
        ("/root/panel/body/meta", META_MAX_W),
    ] {
        let p = frame
            .placement(id)
            .unwrap_or_else(|| panic!("{where_}: {id} is missing from the frame"));
        assert!(
            p.rect.w <= max + EPS,
            "{where_}: {id} is {} wide against a declared max of {max} \
             (FR-005: negotiation honours declared constraints)",
            p.rect.w
        );
    }

    // --- I8. Truncation is recorded, in both directions (FR-020) ----------
    let title = frame
        .placement("/root/panel/header/title")
        .unwrap_or_else(|| panic!("{where_}: the title is missing from the frame"));
    let run_w = 8.0 * cfg.content as f32;
    let line_h = 16.0;
    let fits_wide = title.rect.w - run_w;
    let fits_tall = title.rect.h - line_h;
    // Skip the two boundary bands: `MonoContent` decides truncation by a floor
    // division, so a rect within float dust of an exact character boundary is
    // a coin toss, not a claim.
    if fits_wide.abs() > EPS && fits_tall.abs() > EPS {
        let expected = fits_wide < 0.0 || fits_tall < 0.0;
        assert_eq!(
            title.paint.truncated, expected,
            "{where_}: the title is {}x{} for a {run_w}x{line_h} run, so \
             truncated should be {expected}; hidden content must be \
             machine-detectable (FR-020)",
            title.rect.w, title.rect.h
        );
    }
}

/// A stack's children tile its main axis in declaration order, never overlap,
/// and stay inside it — logically and in device pixels.
fn check_stack(
    where_: &str,
    placements: &[Placement],
    parent: &Placement,
    kids: &[usize],
    rule: ContainerRule,
    scale: Scale,
) {
    let axis = rule.axis;
    let mut previous_end = f32::NEG_INFINITY;
    for &i in kids {
        let child = &placements[i];
        assert!(
            contains(parent.rect, child.rect),
            "{where_}: {} at {:?} is outside its stack {} at {:?}; content \
             placed outside its parent is unreachable",
            child.id,
            child.rect,
            parent.id,
            parent.rect
        );
        let (start, end) = logical_span(child.rect, axis);
        assert!(
            start >= previous_end - EPS,
            "{where_}: {} starts at {start}, before the previous sibling ended \
             at {previous_end}; stack siblings never overlap",
            child.id
        );
        previous_end = end;
    }

    // Device space. Two things, and they are not the same thing: adjacent
    // rects must not overlap after rounding, and rects that abut in logical
    // units must still abut after rounding. `round_rect` rounds four edges
    // independently so that the second holds; nothing but a test on real
    // negotiated rects can say whether it does.
    for (n, &i) in kids.iter().enumerate() {
        let a = round_rect(placements[i].rect, scale);
        for &j in &kids[n + 1..] {
            let b = round_rect(placements[j].rect, scale);
            assert!(
                !device_overlaps(a, b),
                "{where_}: {} {a:?} overlaps {} {b:?} in device pixels at \
                 scale {}",
                placements[i].id,
                placements[j].id,
                scale.factor()
            );
        }
    }
    for pair in kids.windows(2) {
        let (first, second) = (&placements[pair[0]], &placements[pair[1]]);
        let (_, first_end) = logical_span(first.rect, axis);
        let (second_start, _) = logical_span(second.rect, axis);
        if (second_start - first_end).abs() > 1e-4 {
            // A declared gap sits between them; there is no seam to close.
            continue;
        }
        let (_, dev_first_end) = device_span(round_rect(first.rect, scale), axis);
        let (dev_second_start, _) = device_span(round_rect(second.rect, scale), axis);
        assert_eq!(
            dev_second_start,
            dev_first_end,
            "{where_}: {} ends at logical {first_end} and {} starts at \
             {second_start}, but in device pixels at scale {} they are {} and \
             {}; abutting rects must share an edge or the row grows a seam",
            first.id,
            second.id,
            scale.factor(),
            dev_first_end,
            dev_second_start
        );
    }
}

/// A grid's children stay inside it and never overlap, logically or in device
/// pixels. Cell tiling itself is `grid_cells_tile_in_device_pixels`.
fn check_grid(
    where_: &str,
    placements: &[Placement],
    parent: &Placement,
    kids: &[usize],
    rule: ContainerRule,
    scale: Scale,
) {
    for &i in kids {
        let child = &placements[i];
        assert!(
            contains(parent.rect, child.rect),
            "{where_}: {} at {:?} is outside its grid {} at {:?}",
            child.id,
            child.rect,
            parent.id,
            parent.rect
        );
    }
    for (n, &i) in kids.iter().enumerate() {
        let a = &placements[i];
        for &j in &kids[n + 1..] {
            let b = &placements[j];
            assert!(
                !a.rect.overlaps(b.rect),
                "{where_}: grid cells {} {:?} and {} {:?} overlap",
                a.id,
                a.rect,
                b.id,
                b.rect
            );
            assert!(
                !device_overlaps(round_rect(a.rect, scale), round_rect(b.rect, scale)),
                "{where_}: grid cells {} and {} overlap in device pixels at \
                 scale {}",
                a.id,
                b.id,
                scale.factor()
            );
        }
    }

    // A stretched grid with no gaps hands each child exactly its cell, so the
    // children tile if and only if the cells do — and that has to survive
    // device rounding, which is what `cumulative_offsets` accumulates
    // absolutely for.
    if rule.align == Align::Stretch && rule.spacing <= 0.0 && rule.ncols > 0 {
        for (n, &i) in kids.iter().enumerate() {
            let a = &placements[i];
            // The next cell across, then the one below.
            for (neighbour, axis) in [
                (
                    (n % rule.ncols + 1 < rule.ncols).then(|| n + 1),
                    Axis::Horizontal,
                ),
                (Some(n + rule.ncols), Axis::Vertical),
            ] {
                let Some(&j) = neighbour.and_then(|n| kids.get(n)) else {
                    continue;
                };
                let b = &placements[j];
                let (_, a_end) = logical_span(a.rect, axis);
                let (b_start, _) = logical_span(b.rect, axis);
                if (b_start - a_end).abs() > 1e-4 {
                    continue;
                }
                let (_, dev_a_end) = device_span(round_rect(a.rect, scale), axis);
                let (dev_b_start, _) = device_span(round_rect(b.rect, scale), axis);
                assert_eq!(
                    dev_a_end, dev_b_start,
                    "{where_}: grid cells {} and {} abut at logical {a_end} on \
                     {axis:?} but land on device {dev_a_end} and {dev_b_start}",
                    a.id, b.id
                );
            }
        }
    }
}

/// An overlay's children *may* overlap — that is what an overlay is for — but
/// they must layer in declaration order, and one child's whole subtree must
/// paint before the next child's.
fn check_overlay(where_: &str, placements: &[Placement], kids: &[usize]) {
    for pair in kids.windows(2) {
        let (below, above) = (&placements[pair[0]], &placements[pair[1]]);
        assert!(
            above.z > below.z,
            "{where_}: overlay child {} sits at z {} and the later sibling {} \
             at z {}; z-order is child order unless declared",
            below.id,
            below.z,
            above.id,
            above.z
        );
        let below_mask = subtree_mask(placements, pair[0]);
        let above_mask = subtree_mask(placements, pair[1]);
        let below_top = placements
            .iter()
            .zip(&below_mask)
            .filter(|(_, m)| **m)
            .map(|(p, _)| p.z)
            .max()
            .expect("a subtree contains its own root");
        let above_floor = placements
            .iter()
            .zip(&above_mask)
            .filter(|(_, m)| **m)
            .map(|(p, _)| p.z)
            .min()
            .expect("a subtree contains its own root");
        assert!(
            above_floor > below_top,
            "{where_}: the subtree under {} reaches z {below_top} while the \
             subtree under {} starts at z {above_floor}; two overlay layers \
             interleave",
            below.id,
            above.id
        );
    }
}

// ---------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------

/// The acceptance matrix itself: US1 scenarios 1, 2 and 4.
///
/// Scenario 1 (placements satisfy their constraints, siblings do not overlap
/// unless composed in an overlay), scenario 2 (declared truncation is
/// recorded), scenario 4 (every node lays out at the new scale with no stale
/// placements) are all per-configuration claims, so they are checked per
/// configuration. Scenario 3 is `the_same_configuration_petrifies_to_one_digest`.
#[test]
fn every_configuration_satisfies_the_negotiation_contract() {
    let registry = Registry::new();
    let configs = matrix();
    assert_eq!(
        configs.len(),
        SIZES.len() * SCALES.len() * CONTENTS.len() * ALIGNS.len(),
        "the matrix lost a dimension"
    );
    for cfg in &configs {
        let tree = panel(cfg);
        validate(&tree, &registry).unwrap_or_else(|errors| {
            panic!("{}: the fixture is not a legal tree: {errors}", cfg.label())
        });
        let rules = container_rules(&tree);
        let frame = frame_of(cfg);
        check_frame(cfg, &frame, &rules);
    }
}

/// US1 scenario 3: the same tree, state, size and theme petrify to the same
/// digest — from a cold engine, from one that has already run this exact
/// configuration, and from one whose measurement cache was warmed on a
/// different configuration and then thrashed down to four entries.
///
/// The three histories are the point. A digest that is a pure function of the
/// declared inputs cannot notice any of them; a negotiation that reads
/// anything the cache carries between passes fails at least one.
#[test]
fn the_same_configuration_petrifies_to_one_digest() {
    for cfg in matrix() {
        let cold = frame_of(&cfg);

        let mut warm = harness_for(&cfg);
        let first = petrify_with(&mut warm, &cfg);
        let second = petrify_with(&mut warm, &cfg);

        let other = Config {
            viewport: Size::new(cfg.viewport.w + 53.0, cfg.viewport.h + 37.0),
            content: cfg.content + 11,
            ..cfg
        };
        let mut polluted = harness_for(&cfg);
        let _ = petrify_with(&mut polluted, &other);
        polluted.cache.set_capacity(4);
        let after_pollution = petrify_with(&mut polluted, &cfg);

        let where_ = cfg.label();
        assert_eq!(
            first.digest, second.digest,
            "{where_}: two passes over one configuration disagree"
        );
        assert_eq!(
            cold.digest, first.digest,
            "{where_}: a cold engine and a warm one disagree"
        );
        assert_eq!(
            cold.digest, after_pollution.digest,
            "{where_}: the frame depends on what the measurement cache was \
             asked before it — layout is not a pure function of its inputs"
        );
        assert_eq!(
            cold.placements, after_pollution.placements,
            "{where_}: the placements themselves differ, not merely the digest"
        );
    }
}

/// Two configurations that draw different pictures must not share one digest.
///
/// "Different picture" is deliberately measured on something the digest does
/// **not** hash directly: the id, device rect and truncation flag of every
/// placement, plus the text each one draws. Fingerprinting the digest's own
/// input would make this a tautology.
#[test]
fn configurations_that_draw_differently_never_share_a_digest() {
    type Picture = Vec<(String, DeviceRect, bool, String)>;

    fn picture(frame: &PetrifiedFrame) -> Picture {
        let scale = frame.viewport.scale;
        frame
            .drawn()
            .map(|(p, c)| {
                (
                    p.id.clone(),
                    round_rect(p.rect, scale),
                    p.paint.truncated,
                    c.text.as_ref().map(|t| t.text.clone()).unwrap_or_default(),
                )
            })
            .collect()
    }

    let mut by_digest: HashMap<FrameDigest, (String, Picture)> = HashMap::new();
    let mut distinct_pictures: HashSet<Picture> = HashSet::new();
    for cfg in matrix() {
        let frame = frame_of(&cfg);
        let shot = picture(&frame);
        distinct_pictures.insert(shot.clone());
        match by_digest.get(&frame.digest) {
            Some((other, seen)) => assert_eq!(
                seen,
                &shot,
                "{}: shares a digest with {other} while drawing a different \
                 picture — two frames with one digest are supposed to be two \
                 identical pictures",
                cfg.label()
            ),
            None => {
                by_digest.insert(frame.digest, (cfg.label(), shot));
            }
        }
    }
    // A matrix that collapsed to one picture would pass the loop above and
    // prove nothing.
    assert!(
        distinct_pictures.len() > SIZES.len(),
        "the matrix produced only {} distinct pictures across {} \
         configurations; it is not sweeping anything",
        distinct_pictures.len(),
        matrix().len()
    );
}

// ---------------------------------------------------------------------------
// FR-005: the concession order, at the three regimes that reach it
// ---------------------------------------------------------------------------

/// A column of: a rigid head, a flexible middle, a min-constrained scroll
/// region, and a rigid tail.
fn concession_column() -> ViewNode {
    let rigid = |key: &str| {
        ViewNode::new(NodeKind::Image, Key::new(key)).with_props(Props {
            image: Some("icon".into()),
            ..Props::default()
        })
    };
    let flexible = ViewNode::new(NodeKind::Spacer, "slack").with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(20.0),
            max: Some(80.0),
            priority: 0,
        },
        ..Constraints::default()
    });
    let scroller = ViewNode::new(NodeKind::Scroll, "log")
        .with_props(Props {
            axis: Some(Axis::Vertical),
            ..Props::default()
        })
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(50.0),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        });
    stack_node("col", Axis::Vertical, 0.0, Align::Stretch)
        .child(rigid("head"))
        .child(flexible)
        .child(scroller)
        .child(rigid("tail"))
}

fn concede_at(height: f32) -> PetrifiedFrame {
    let tree = concession_column();
    let mut harness = Harness::new();
    petrify(
        1,
        validated(&tree),
        &mut harness.ctx(),
        Viewport::new(Size::new(64.0, height), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn extent(frame: &PetrifiedFrame, id: &str) -> f32 {
    frame
        .placement(id)
        .unwrap_or_else(|| panic!("{id} is missing"))
        .rect
        .h
}

/// FR-005 in its own order: flexible slack, then declared scroll regions,
/// then truncation — and each step only after the one before it has nothing
/// left to give.
///
/// The column asks for 64 (head) + 80 (slack at its maximum) + 50 (the scroll
/// region's declared minimum) + 64 (tail) = 258 units. Three heights walk the
/// order:
///
/// | height | slack | log | what gave |
/// |---|---|---|---|
/// | 258 | 80 | 50 | nothing |
/// | 198 | 20 | 50 | slack alone, down to its own minimum |
/// | 158 | 20 | 10 | slack first, then the scroll region past its minimum |
/// | 100 | 20 | 0  | both, then the tail truncates |
#[test]
fn the_concession_order_is_slack_then_scroll_region_then_truncation() {
    let full = concede_at(258.0);
    assert_eq!(extent(&full, "/col/head"), 64.0, "the head never gives");
    assert_eq!(extent(&full, "/col/slack"), 80.0);
    assert_eq!(extent(&full, "/col/log"), 50.0);
    assert_eq!(extent(&full, "/col/tail"), 64.0);
    assert!(
        !full.placement("/col").unwrap().paint.truncated,
        "a fit that works truncates nothing"
    );

    // Step one alone: 60 units of overflow, and the slack has exactly 60 to
    // give before it reaches its declared minimum.
    let slack_only = concede_at(198.0);
    assert_eq!(
        extent(&slack_only, "/col/slack"),
        20.0,
        "the most flexible child gives first, down to its own minimum"
    );
    assert_eq!(
        extent(&slack_only, "/col/log"),
        50.0,
        "the scroll region keeps its declared minimum while slack remains"
    );
    assert_eq!(extent(&slack_only, "/col/tail"), 64.0);
    assert!(
        !slack_only.placement("/col").unwrap().paint.truncated,
        "spending flexible slack is not a truncation"
    );

    // Step two: the slack is exhausted and the scroll region absorbs the
    // remaining 40 units by shrinking past its own minimum. Nothing is lost —
    // it grows a scroll range instead, which is why this step precedes
    // truncation and not the other way round.
    let absorbed = concede_at(158.0);
    assert_eq!(extent(&absorbed, "/col/slack"), 20.0);
    assert_eq!(
        extent(&absorbed, "/col/log"),
        10.0,
        "the scroll region absorbs what the slack could not"
    );
    assert_eq!(
        extent(&absorbed, "/col/tail"),
        64.0,
        "the rigid tail keeps its content while a scroll region can still \
         absorb — that is the whole content of ScrollRegion preceding Truncate"
    );
    assert!(
        !absorbed.placement("/col").unwrap().paint.truncated,
        "absorption is not truncation"
    );

    // Step three: both earlier steps are spent and the tail loses content.
    let truncated = concede_at(100.0);
    assert_eq!(extent(&truncated, "/col/slack"), 20.0);
    assert_eq!(extent(&truncated, "/col/log"), 0.0);
    assert_eq!(
        extent(&truncated, "/col/head"),
        64.0,
        "truncation takes from the tail, so the leading content survives"
    );
    assert_eq!(
        extent(&truncated, "/col/tail"),
        16.0,
        "the tail keeps what is left of the budget and no more"
    );
    assert!(
        truncated.placement("/col").unwrap().paint.truncated,
        "content was lost, and a lost-content frame must say so"
    );

    // And through every regime, nothing was placed outside the column or on
    // top of a sibling.
    for (height, frame) in [
        (258.0, &full),
        (198.0, &slack_only),
        (158.0, &absorbed),
        (100.0, &truncated),
    ] {
        let mut bottom = 0.0f32;
        for p in frame.placements.iter().filter(|p| p.parent == Some(0)) {
            assert!(
                p.rect.y >= bottom - EPS,
                "at height {height}, {} starts at {} before {bottom}",
                p.id,
                p.rect.y
            );
            assert!(
                p.rect.bottom() <= height + EPS,
                "at height {height}, {} runs to {}",
                p.id,
                p.rect.bottom()
            );
            bottom = p.rect.bottom();
        }
    }
}

// ---------------------------------------------------------------------------
// FR-008: fractional scale
// ---------------------------------------------------------------------------

/// Grid cells tile in device pixels at every scale the matrix uses.
///
/// A grid with `Align::Stretch` and no spacing gives each child exactly its
/// cell, so the children tile if and only if the cells do. Edge rounding is
/// what makes that survive a fractional scale; origin-plus-size rounding would
/// leave a one-pixel seam or overlap between columns whose widths are not
/// whole pixels.
#[test]
fn grid_cells_tile_in_device_pixels_at_every_scale() {
    let cell = |key: &str| text_node(key, "cell", TextWrap::Clip);
    let grid = ViewNode::new(NodeKind::Grid, "g")
        .with_props(Props {
            columns: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Weight { weight: 2.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            rows: vec![
                TrackSize::Fixed { value: 17.5 },
                TrackSize::Fixed { value: 17.5 },
            ],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .child(cell("a"))
        .child(cell("b"))
        .child(cell("c"))
        .child(cell("d"))
        .child(cell("e"))
        .child(cell("f"));
    let grid = validated(&grid);

    for &scale in SCALES {
        for width in [100.0_f32, 137.0, 251.5, 400.0] {
            let mut harness = Harness::new();
            harness.scale = Scale::new(scale).unwrap();
            let frame = petrify(
                1,
                grid,
                &mut harness.ctx(),
                Viewport::new(Size::new(width, 40.0), ThemeMode::Dark)
                    .with_scale(Scale::new(scale).unwrap()),
                TransitionActivity::default(),
            );
            let cells: Vec<&Placement> = frame.placements.iter().skip(1).collect();
            assert_eq!(cells.len(), 6, "six cells at {width} @{scale}");
            let dev: Vec<DeviceRect> = cells
                .iter()
                .map(|p| round_rect(p.rect, frame.viewport.scale))
                .collect();
            for row in 0..2 {
                for col in 0..2 {
                    let left = dev[row * 3 + col];
                    let right = dev[row * 3 + col + 1];
                    assert_eq!(
                        left.right(),
                        right.x,
                        "{width} @{scale}: columns {col} and {} do not meet in \
                         device pixels ({left:?} then {right:?})",
                        col + 1
                    );
                }
            }
            for col in 0..3 {
                assert_eq!(
                    dev[col].bottom(),
                    dev[col + 3].y,
                    "{width} @{scale}: the two rows of column {col} do not \
                     meet in device pixels ({:?} then {:?})",
                    dev[col],
                    dev[col + 3]
                );
            }
        }
    }
}

/// Layout is logical units end to end: the same tree in the same viewport lays
/// out to the same logical rects at every display scale, and only the device
/// rounding differs (FR-008 — Petra re-renders at physical resolution, it does
/// not scale a placed layout).
#[test]
fn logical_placement_does_not_depend_on_display_scale() {
    let cfg = Config {
        viewport: Size::new(401.0, 307.0),
        scale: 1.0,
        content: 40,
        align: Align::Start,
        spacing: 7.0,
        offset: 96.0,
    };
    let base = frame_of(&cfg);
    let base_rects: Vec<(String, Rect)> = base
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.rect))
        .collect();

    for &scale in &SCALES[1..] {
        let scaled = frame_of(&Config { scale, ..cfg });
        let rects: Vec<(String, Rect)> = scaled
            .placements
            .iter()
            .map(|p| (p.id.clone(), p.rect))
            .collect();
        assert_eq!(
            rects, base_rects,
            "the logical layout moved at scale {scale}; logical units are \
             scale-free and only `round_rect` may see the factor"
        );
        assert_ne!(
            scaled.digest, base.digest,
            "scale {scale} produced the same digest as 1.0, but the device \
             pixels differ and the scale is a declared digest input"
        );
    }
}

// ---------------------------------------------------------------------------
// The seam property, over generated geometry
// ---------------------------------------------------------------------------

/// A nested column whose outer offset, inner extents and scale are all chosen
/// to land off whole pixels.
fn seam_tree(children: usize, spacing: f32, lead: f32) -> ViewNode {
    let mut inner = stack_node("inner", Axis::Vertical, spacing, Align::Stretch);
    for i in 0..children {
        inner = inner.child(ViewNode::new(NodeKind::Spacer, Key::new(format!("r{i}"))));
    }
    stack_node("outer", Axis::Vertical, 0.0, Align::Stretch)
        .child(
            ViewNode::new(NodeKind::Spacer, "lead").with_constraints(Constraints {
                vertical: AxisConstraint {
                    min: Some(lead),
                    max: Some(lead),
                    priority: 1,
                },
                ..Constraints::default()
            }),
        )
        .child(inner)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    /// Rows that abut in logical units must still abut in device pixels, at a
    /// fractional origin and a fractional scale.
    ///
    /// `frame/rounding.rs` proves this for hand-built rows at x = 0 with an
    /// identical extent. Here the rows come out of a real distribution, the
    /// column starts at a leading offset that is not a whole pixel, and the
    /// extents are whatever the equal-share algorithm produced — which is the
    /// arithmetic a seam would actually come from.
    #[test]
    fn abutting_rows_share_an_edge_in_device_pixels(
        children in 2usize..7,
        lead in 0.0f32..40.0,
        height in 40.0f32..400.0,
        scale_index in 0usize..SEAM_SCALES.len(),
    ) {
        let scale = Scale::new(SEAM_SCALES[scale_index]).unwrap();
        let tree = seam_tree(children, 0.0, lead);
        let mut harness = Harness::new();
        harness.scale = scale;
        let frame = petrify(
            1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(64.0, height), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let rows: Vec<&Placement> = frame
            .placements
            .iter()
            .filter(|p| p.id.starts_with("/outer/inner/r"))
            .collect();
        prop_assert_eq!(rows.len(), children);
        for pair in rows.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            prop_assert!(
                (b.rect.y - a.rect.bottom()).abs() <= 1e-4,
                "{} ends at {} and {} starts at {}; a zero-spacing stack must \
                 abut in logical units before device rounding is even asked",
                a.id, a.rect.bottom(), b.id, b.rect.y
            );
            let da = round_rect(a.rect, scale);
            let db = round_rect(b.rect, scale);
            prop_assert_eq!(
                da.bottom(),
                db.y,
                "at scale {} with lead {}, {} ends at device {} and {} starts \
                 at device {}",
                scale.factor(), lead, a.id, da.bottom(), b.id, db.y
            );
        }
    }
}

// ---------------------------------------------------------------------------
// A finding, pinned so it cannot change without someone deciding to
// ---------------------------------------------------------------------------

/// `Align::Stretch` stops at a child's declared cross-axis `max` exactly the
/// way the other three alignments already do.
///
/// Decided 2026-08-22 (`Ai-notes/QUESTIONS.md` Round 3 item 2): a declared
/// constraint beats `Align::Stretch`. This test replaces
/// `stretch_fills_past_a_declared_cross_axis_maximum`, which pinned the
/// opposite reading — Stretch filling the container's full cross extent
/// regardless of a smaller declared `max` — specifically so that reversing it
/// had to be a deliberate act, not a drive-by. It was; see
/// `.agents/notes/implemented/bug-fix/2026-08-22-petra-stretch-honours-constraints.md`
/// for the decision and `layout/stack.rs` / `layout/grid.rs` for the fix.
#[test]
fn stretch_stops_at_a_declared_cross_axis_maximum() {
    let capped =
        ViewNode::new(NodeKind::Spacer, "capped").with_constraints(max_on(Axis::Horizontal, 30.0));
    let column = stack_node("col", Axis::Vertical, 0.0, Align::Stretch).child(capped);

    let mut harness = Harness::new();
    let frame = petrify(
        1,
        validated(&column),
        &mut harness.ctx(),
        Viewport::new(Size::new(200.0, 50.0), ThemeMode::Dark),
        TransitionActivity::default(),
    );
    assert_eq!(
        frame.placement("/col/capped").unwrap().rect.w,
        30.0,
        "a declared max-30 constraint must win over Stretch filling the \
         200-unit column; if this now reports 200 the fix regressed"
    );

    // Every other alignment already honours the maximum; Stretch now agrees
    // with all three rather than being the one exception.
    for align in [Align::Start, Align::Center, Align::End] {
        let column = stack_node("col", Axis::Vertical, 0.0, align).child(
            ViewNode::new(NodeKind::Spacer, "capped")
                .with_constraints(max_on(Axis::Horizontal, 30.0)),
        );
        let mut harness = Harness::new();
        let frame = petrify(
            1,
            validated(&column),
            &mut harness.ctx(),
            Viewport::new(Size::new(200.0, 50.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        assert!(
            frame.placement("/col/capped").unwrap().rect.w <= 30.0 + EPS,
            "{align:?} placed a max-30 child at {} units wide",
            frame.placement("/col/capped").unwrap().rect.w
        );
    }
}

// ---------------------------------------------------------------------------
// The seam regression: abutting rects, non-zero container origin
// ---------------------------------------------------------------------------

/// A pinned coordinate for the container the seam cases nest inside.
fn pinned(key: &str, axis: Axis, value: f32) -> ViewNode {
    let c = AxisConstraint {
        min: Some(value),
        max: Some(value),
        priority: 1,
    };
    ViewNode::new(NodeKind::Spacer, Key::new(key)).with_constraints(match axis {
        Axis::Horizontal => Constraints {
            horizontal: c,
            ..Constraints::default()
        },
        Axis::Vertical => Constraints {
            vertical: c,
            ..Constraints::default()
        },
    })
}

/// Rows that abut exactly in logical units must land on one device edge, even
/// when their stack does not start at the origin.
///
/// The cases below are not decorative. Each one is a real configuration in
/// which a relative placement cursor produces two *different* f32 values for
/// one shared edge:
///
/// * child `i`'s trailing edge is `(slot.origin + cursor) + extent`
/// * child `i + 1`'s leading edge is `slot.origin + (cursor + extent)`
///
/// Exact arithmetic says those are the same number. `f32` says they differ by
/// an ulp about once in twenty thousand adjacent pairs — and `round_rect`
/// then rounds the two sides of one seam to two different pixels, which is a
/// one-pixel gap or a one-pixel overlap between rows that touch. It defeats
/// the entire reason `round_rect` rounds four edges instead of rounding an
/// origin and a size.
///
/// It is not a fractional-scale-only defect: the last two cases are at 1.0.
#[test]
fn abutting_rows_share_a_device_edge_from_a_shifted_origin() {
    // (leading offset, viewport height, rows, scale, what goes wrong)
    const CASES: &[(f32, f32, usize, f32, &str)] = &[
        (0.5, 48.0, 6, 2.0, "gap"),
        (0.5, 94.0, 6, 1.5, "gap"),
        (0.5, 96.25, 3, 1.5, "gap"),
        (0.5, 96.25, 6, 1.5, "overlap"),
        (0.75, 48.25, 6, 1.0, "gap at unit scale"),
        (1.0, 96.0, 6, 1.0, "gap at unit scale"),
        (1.0, 96.0, 6, 1.5, "overlap"),
    ];

    for &(lead, height, rows, scale, kind) in CASES {
        let scale = Scale::new(scale).unwrap();
        let mut inner = stack_node("inner", Axis::Vertical, 0.0, Align::Stretch);
        for i in 0..rows {
            inner = inner.child(ViewNode::new(NodeKind::Spacer, Key::new(format!("r{i}"))));
        }
        let tree = stack_node("outer", Axis::Vertical, 0.0, Align::Stretch)
            .child(pinned("lead", Axis::Vertical, lead))
            .child(inner);

        let mut harness = Harness::new();
        harness.scale = scale;
        let frame = petrify(
            1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(64.0, height), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let rows: Vec<&Placement> = frame
            .placements
            .iter()
            .filter(|p| p.id.starts_with("/outer/inner/r"))
            .collect();
        for pair in rows.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                (b.rect.y - a.rect.bottom()).abs() <= 1e-4,
                "lead {lead} height {height} scale {}: {} ends at {} and {} \
                 starts at {}; a zero-spacing stack must abut in logical units",
                scale.factor(),
                a.id,
                a.rect.bottom(),
                b.id,
                b.rect.y
            );
            let (da, db) = (round_rect(a.rect, scale), round_rect(b.rect, scale));
            assert_eq!(
                da.bottom(),
                db.y,
                "lead {lead} height {height} scale {} ({kind}): {} ends at \
                 device {} and {} starts at device {} — logical {} and {}",
                scale.factor(),
                a.id,
                da.bottom(),
                b.id,
                db.y,
                a.rect.bottom(),
                b.rect.y
            );
        }
    }
}

/// The same defect on the grid's cell offsets, which accumulate the same way.
#[test]
fn abutting_grid_cells_share_a_device_edge_from_a_shifted_origin() {
    // (leading offset, viewport width, scale)
    const CASES: &[(f32, f32, f32)] = &[
        (25.0, 47.0, 1.5),
        (26.0, 49.5, 1.5),
        (28.0, 48.5, 1.5),
        (49.0, 96.0, 1.5),
        (50.0, 92.5, 1.5),
    ];

    for &(lead, width, scale) in CASES {
        let scale = Scale::new(scale).unwrap();
        let grid = ViewNode::new(NodeKind::Grid, "g")
            .with_props(Props {
                columns: vec![
                    TrackSize::Weight { weight: 1.0 },
                    TrackSize::Weight { weight: 1.0 },
                    TrackSize::Weight { weight: 1.0 },
                ],
                column_spacing: Some(0.0),
                row_spacing: Some(0.0),
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .child(text_node("c0", "cell", TextWrap::Clip))
            .child(text_node("c1", "cell", TextWrap::Clip))
            .child(text_node("c2", "cell", TextWrap::Clip));
        let tree = stack_node("row", Axis::Horizontal, 0.0, Align::Stretch)
            .child(pinned("lead", Axis::Horizontal, lead))
            .child(grid);

        let mut harness = Harness::new();
        harness.scale = scale;
        let frame = petrify(
            1,
            validated(&tree),
            &mut harness.ctx(),
            Viewport::new(Size::new(width, 40.0), ThemeMode::Dark).with_scale(scale),
            TransitionActivity::default(),
        );
        let cells: Vec<&Placement> = frame
            .placements
            .iter()
            .filter(|p| p.id.starts_with("/row/g/c"))
            .collect();
        assert_eq!(cells.len(), 3, "lead {lead} width {width}: three cells");
        for pair in cells.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                (b.rect.x - a.rect.right()).abs() <= 1e-4,
                "lead {lead} width {width}: {} ends at {} and {} starts at {}",
                a.id,
                a.rect.right(),
                b.id,
                b.rect.x
            );
            let (da, db) = (round_rect(a.rect, scale), round_rect(b.rect, scale));
            assert_eq!(
                da.right(),
                db.x,
                "lead {lead} width {width} scale {}: cell {} ends at device \
                 {} and {} starts at device {} — logical {} and {}",
                scale.factor(),
                a.id,
                da.right(),
                b.id,
                db.x,
                a.rect.right(),
                b.rect.x
            );
        }
    }
}
