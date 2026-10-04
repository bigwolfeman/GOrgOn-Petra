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
use gorgon_petra::testing::{
    GeneratedRows, Harness, MonoContent, extended_vocabulary, gap, validated,
};
use gorgon_petra::token::{ThemeMode, ThemeSnapshot};
use gorgon_petra::tree::{
    Anchor, AxisConstraint, ClampRule, Constraints, GridSpan, Key, KeyPath, Layer, NodeKind, Props,
    Registry, TextWrap, TrackSize, ViewNode, validate,
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
        spacing: gap(spacing),
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
            column_spacing: gap(cfg.spacing),
            row_spacing: gap(cfg.spacing),
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
fn container_rules(root: &ViewNode, theme: &ThemeSnapshot) -> BTreeMap<String, ContainerRule> {
    fn walk(
        node: &ViewNode,
        theme: &ThemeSnapshot,
        path: &mut KeyPath,
        out: &mut BTreeMap<String, ContainerRule>,
    ) {
        path.push(node.key.clone());
        if node.kind.is_container() {
            // The same snapshot the frame under test was measured under, so
            // the gap this expectation is built from is the gap the engine
            // reserved — not a second, independently authored number.
            let stack = node.props.stack(theme);
            let grid = node.props.grid(theme);
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
            walk(child, theme, path, out);
        }
        path.pop();
    }
    let mut out = BTreeMap::new();
    walk(root, theme, &mut KeyPath::root(), &mut out);
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
    let configs = matrix();
    assert_eq!(
        configs.len(),
        SIZES.len() * SCALES.len() * CONTENTS.len() * ALIGNS.len(),
        "the matrix lost a dimension"
    );
    for cfg in &configs {
        let tree = panel(cfg);
        // The fixture spells its gaps with `testing::gap`, which mints a name
        // encoding the extent (`spacing.9units`). Those names are real token
        // references and are checked against a vocabulary like any other, so
        // the registry has to carry the same extended vocabulary the harness
        // builds — a bare `Registry::new()` declares nothing and refuses every
        // gap in the matrix. `validate` is called directly rather than through
        // `testing::validated` only so a refusal can name the configuration.
        let registry = Registry::with_vocabulary(extended_vocabulary(&tree));
        validate(&tree, &registry).unwrap_or_else(|errors| {
            panic!("{}: the fixture is not a legal tree: {errors}", cfg.label())
        });
        let rules = container_rules(&tree, &harness_for(cfg).theme);
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
/// Decided 2026-08-22 (`.agents/tallies/QUESTIONS.md` Round 3 item 2): a declared
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
                columns: vec![TrackSize::Weight { weight: 1.0 }; 3],
                column_spacing: gap(0.0),
                row_spacing: gap(0.0),
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

// ---------------------------------------------------------------------------
// SC-014 / FR-061: the week view
// ---------------------------------------------------------------------------

/// Days across the week.
const WEEK_DAYS: usize = 7;
/// Columns inside one day. Two, because the whole difficulty of a week view is
/// what it does when two things run at once: the second one has to sit
/// *beside* the first, not on top of it.
const DAY_LANES: usize = 2;
/// The hour gutter, plus every day's lanes.
const WEEK_COLS: usize = 1 + WEEK_DAYS * DAY_LANES;
/// Quarter-hour row tracks in one day. FR-061's own sentence: "an event
/// occupying four of ninety-six row tracks".
const DAY_SLOTS: usize = 96;
/// Row tracks: the day-name header, then the time axis.
const WEEK_ROWS: usize = 1 + DAY_SLOTS;
/// Quarter-hour slots in an hour, and so the row span of an hour label.
const SLOTS_PER_HOUR: usize = 4;
/// Declared height of the day-name header row.
const WEEK_HEADER_H: f32 = 18.0;
/// Declared width of the hour gutter column.
const WEEK_GUTTER_W: f32 = 34.0;

/// Day names, left to right.
const DAY_NAMES: [&str; WEEK_DAYS] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// One entry in a lane's chain: what is scheduled, and how many quarter-hour
/// slots it takes.
type Run = (&'static str, usize);

/// One entry in a day's plan.
enum Segment {
    /// Nothing runs beside this one, so it takes the day's whole width — a
    /// two-column span.
    Whole(&'static str, usize),
    /// Two things run at once. Each lane carries its own chain over the same
    /// stretch of the day, and the two chains need not change at the same
    /// moment: that is what makes a *partial* overlap expressible.
    Concurrent(&'static [Run], &'static [Run]),
}

impl Segment {
    /// Quarter-hour slots this segment covers.
    fn slots(&self) -> usize {
        match self {
            Self::Whole(_, slots) => *slots,
            Self::Concurrent(left, right) => {
                let lhs: usize = left.iter().map(|run| run.1).sum();
                let rhs: usize = right.iter().map(|run| run.1).sum();
                assert_eq!(
                    lhs, rhs,
                    "a concurrent region's two lanes must cover the same slots, \
                     or the day stops tiling its own column"
                );
                lhs
            }
        }
    }
}

/// A time-blocked Monday, with one pair that runs at exactly the same time.
const MONDAY: &[Segment] = &[
    Segment::Whole("Sleep", 24),
    Segment::Whole("Morning routine", 6),
    Segment::Whole("Commute", 2),
    // Fifteen minutes: one row track, and the unit every other extent in this
    // fixture is judged against.
    Segment::Whole("Standup", 1),
    Segment::Whole("Deep work: kernel", 8),
    // Ninety minutes over six of ninety-six row tracks — FR-061's example.
    Segment::Whole("Design review", 6),
    Segment::Whole("Lunch", 5),
    Segment::Concurrent(&[("Interview loop", 4)], &[("Build triage", 4)]),
    Segment::Whole("Paperwork", 4),
    Segment::Whole("Deep work: tests", 8),
    Segment::Whole("Commute home", 2),
    Segment::Whole("Dinner", 6),
    Segment::Whole("Reading", 6),
    Segment::Whole("Sleep", 14),
];

/// A Tuesday whose morning runs two lanes that change at different moments,
/// so every overlap in it is a partial one.
const TUESDAY: &[Segment] = &[
    Segment::Whole("Sleep", 26),
    Segment::Whole("Morning routine", 6),
    Segment::Concurrent(
        &[("Spec drafting", 8), ("Pairing: petra", 8)],
        &[("Email triage", 2), ("Vendor call", 8), ("Bug triage", 6)],
    ),
    Segment::Whole("Lunch", 4),
    Segment::Whole("Deep work: seating", 12),
    Segment::Whole("Retro", 4),
    Segment::Whole("Errands", 6),
    Segment::Whole("Dinner", 4),
    Segment::Whole("Evening", 8),
    Segment::Whole("Sleep", 10),
];

/// A Wednesday with nothing concurrent on it at all: every block takes the
/// day's whole width.
const WEDNESDAY: &[Segment] = &[
    Segment::Whole("Sleep", 28),
    Segment::Whole("Morning routine", 4),
    Segment::Whole("Deep work: grid spans", 16),
    Segment::Whole("Lunch", 4),
    Segment::Whole("Office hours", 8),
    Segment::Whole("Deep work: review", 8),
    Segment::Whole("Wrap-up", 2),
    Segment::Whole("Evening", 16),
    Segment::Whole("Sleep", 10),
];

/// A Thursday whose concurrent region puts one block beside two.
const THURSDAY: &[Segment] = &[
    Segment::Whole("Sleep", 26),
    Segment::Whole("Morning routine", 6),
    Segment::Whole("Standup", 1),
    Segment::Whole("Deep work: layout", 11),
    Segment::Concurrent(
        &[("Release call", 4)],
        &[("Doc pass", 2), ("Metrics review", 2)],
    ),
    Segment::Whole("Lunch", 4),
    Segment::Whole("Deep work: petrify", 16),
    Segment::Whole("Commute home", 2),
    Segment::Whole("Evening", 16),
    Segment::Whole("Sleep", 10),
];

/// A plain Friday.
const FRIDAY: &[Segment] = &[
    Segment::Whole("Sleep", 26),
    Segment::Whole("Morning routine", 6),
    Segment::Whole("Deep work: gates", 16),
    Segment::Whole("Lunch", 4),
    Segment::Whole("Demo", 4),
    Segment::Whole("One-on-ones", 8),
    Segment::Whole("Week review", 4),
    Segment::Whole("Evening", 18),
    Segment::Whole("Sleep", 10),
];

/// A Saturday, which starts later and runs longer blocks.
const SATURDAY: &[Segment] = &[
    Segment::Whole("Sleep", 32),
    Segment::Whole("Long run", 8),
    Segment::Whole("Chores", 8),
    Segment::Whole("Lunch", 4),
    Segment::Whole("Workshop", 12),
    Segment::Whole("Dinner with friends", 12),
    Segment::Whole("Evening", 10),
    Segment::Whole("Sleep", 10),
];

/// A Sunday held by one all-day block: a span over every row track the grid
/// has, and over both of the day's lanes.
const SUNDAY: &[Segment] = &[Segment::Whole("Offsite: annual planning", DAY_SLOTS)];

/// The week, left to right.
const WEEK: [&[Segment]; WEEK_DAYS] = [
    MONDAY, TUESDAY, WEDNESDAY, THURSDAY, FRIDAY, SATURDAY, SUNDAY,
];

/// What one cell of the week grid is. The role follows from where the cell
/// sits, and it decides which assertions apply to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CellRole {
    /// The corner above the gutter and left of the day names.
    Corner,
    /// A day name, spanning that day's lanes.
    DayHeader,
    /// An hour on the time axis, spanning that hour's quarter-hour rows.
    HourLabel,
    /// A scheduled block.
    Event,
}

/// One cell of the week grid: what it says, where it was declared to sit, and
/// how many tracks it was declared to cover.
///
/// The declared seat is what every assertion is judged against, and it is
/// never read back from the frame. A frame that seated a block in a cell the
/// author did not write is exactly the bug these assertions exist to catch, so
/// the expectation has to come from the declaration.
#[derive(Clone, Debug)]
struct WeekCell {
    /// Identity key, and so the tail of the placement id.
    key: String,
    /// The text the cell carries.
    label: String,
    /// Which of the four kinds of cell this is.
    role: CellRole,
    /// The day band the cell sits in, when it sits in one.
    day: Option<usize>,
    /// The lane inside that day, when the cell occupies exactly one of them.
    /// `None` for a block that takes the day's whole width.
    lane: Option<usize>,
    /// Declared leading column.
    col: usize,
    /// Declared leading row. Row zero is the day-name header.
    row: usize,
    /// Declared column span.
    ncols: usize,
    /// Declared row span.
    nrows: usize,
}

impl WeekCell {
    /// First quarter-hour slot, counting from midnight.
    fn slot(&self) -> usize {
        self.row.saturating_sub(1)
    }

    /// One past the last quarter-hour slot.
    fn slot_end(&self) -> usize {
        self.slot() + self.nrows
    }
}

/// One scheduled block. A block with no lane takes the day's whole width.
fn event_cell(day: usize, lane: Option<usize>, slot: usize, slots: usize, label: &str) -> WeekCell {
    let col = 1 + day * DAY_LANES + lane.unwrap_or(0);
    WeekCell {
        key: format!("e{col}-{}", slot + 1),
        label: label.to_owned(),
        role: CellRole::Event,
        day: Some(day),
        lane,
        col,
        row: slot + 1,
        ncols: if lane.is_some() { 1 } else { DAY_LANES },
        nrows: slots,
    }
}

/// The week's cells, in the order the grid reads them: row-major by declared
/// seat.
///
/// The plan tiles the grid exactly — every cell of every row is covered by
/// some block's run. That is not decoration, it is what Petra's grid asks for.
/// Seating is a forward-only cursor (`layout::grid`'s `Flow::seat`) with no
/// cell addresses in the tree at all, so a hole in the tiling could only be
/// crossed by declaring something to fill it, and a something-to-fill-it is
/// the spacer-based positioning SC-014 refuses. A week with every minute
/// allocated is a time-blocked week, which is what this fixture is; a week
/// view with a genuinely empty afternoon needs a container that places
/// children at declared rects, which this spec names as deferred rather than
/// missing.
fn week_cells() -> Vec<WeekCell> {
    let mut cells = vec![WeekCell {
        key: "corner".to_owned(),
        label: "wk 34".to_owned(),
        role: CellRole::Corner,
        day: None,
        lane: None,
        col: 0,
        row: 0,
        ncols: 1,
        nrows: 1,
    }];
    for (day, name) in DAY_NAMES.iter().enumerate() {
        cells.push(WeekCell {
            key: format!("day-{day}"),
            label: (*name).to_owned(),
            role: CellRole::DayHeader,
            day: Some(day),
            lane: None,
            col: 1 + day * DAY_LANES,
            row: 0,
            ncols: DAY_LANES,
            nrows: 1,
        });
    }
    for hour in 0..DAY_SLOTS / SLOTS_PER_HOUR {
        cells.push(WeekCell {
            key: format!("hour-{hour:02}"),
            label: format!("{hour:02}:00"),
            role: CellRole::HourLabel,
            day: None,
            lane: None,
            col: 0,
            row: 1 + hour * SLOTS_PER_HOUR,
            ncols: 1,
            nrows: SLOTS_PER_HOUR,
        });
    }
    for (day, plan) in WEEK.iter().enumerate() {
        let mut slot = 0;
        for segment in plan.iter() {
            match segment {
                Segment::Whole(label, slots) => {
                    cells.push(event_cell(day, None, slot, *slots, label));
                }
                Segment::Concurrent(left, right) => {
                    for (lane, chain) in [(0, left), (1, right)] {
                        let mut at = slot;
                        for (label, slots) in chain.iter() {
                            cells.push(event_cell(day, Some(lane), at, *slots, label));
                            at += slots;
                        }
                    }
                }
            }
            slot += segment.slots();
        }
        assert_eq!(
            slot, DAY_SLOTS,
            "{}'s plan covers {slot} quarter-hour slots, not {DAY_SLOTS}; the \
             week view tiles its grid exactly and a short day leaves a hole no \
             flow-seated grid can step over",
            DAY_NAMES[day]
        );
    }
    cells.sort_by_key(|cell| (cell.row, cell.col));
    cells
}

/// The week view: one grid, and text in it. No stack, no spacer, no nesting.
///
/// Columns are the hour gutter and then two weighted lanes per day, so the
/// days share the width the window has. Rows are the day-name header and then
/// ninety-six weighted quarter-hour tracks, so the day fills the height the
/// window has and an event's extent is its duration times a track.
fn week_view(cells: &[WeekCell], column_spacing: f32) -> ViewNode {
    let mut columns = Vec::with_capacity(WEEK_COLS);
    columns.push(TrackSize::Fixed {
        value: WEEK_GUTTER_W,
    });
    columns.resize(WEEK_COLS, TrackSize::Weight { weight: 1.0 });
    let mut rows = Vec::with_capacity(WEEK_ROWS);
    rows.push(TrackSize::Fixed {
        value: WEEK_HEADER_H,
    });
    rows.resize(WEEK_ROWS, TrackSize::Weight { weight: 1.0 });
    let mut grid = ViewNode::new(NodeKind::Grid, "week").with_props(Props {
        columns,
        rows,
        column_spacing: gap(column_spacing),
        // The time axis is continuous: 09:00 ends where 09:15 begins. A gap
        // between row tracks would be a gap in *time*, so the row gap is the
        // one number in this fixture that is not swept.
        row_spacing: gap(0.0),
        align: Some(Align::Stretch),
        ..Props::default()
    });
    for cell in cells {
        grid = grid.child(
            ViewNode::new(NodeKind::Text, Key::new(cell.key.clone())).with_props(Props {
                text: Some(cell.label.clone()),
                wrap: Some(TextWrap::Clip),
                span: Some(GridSpan {
                    columns: cell.ncols,
                    rows: cell.nrows,
                }),
                ..Props::default()
            }),
        );
    }
    grid
}

/// Slack for an extent a grid summed track by track, compared against the same
/// quantity written as one multiplication.
///
/// `layout::grid`'s `span_extent` adds a run's track extents left to right, so
/// a ninety-six track run is ninety-six `f32` additions, while `slots * unit`
/// is one multiplication. `f32` carries about seven digits and the two
/// arithmetics need not agree in the last of them. Four parts in a hundred
/// thousand per track is roughly two hundred times smaller than the
/// quarter-hour track a real duration bug would move an event by.
fn slot_slack(slots: usize, unit: f32) -> f32 {
    EPS + 4e-5 * slots as f32 * unit.abs()
}

/// SC-014: a week view — events occupying track ranges, overlapping events in
/// adjacent columns — lays out from one grid with zero spacer-based
/// positioning, at every window size and display scale in the US1 matrix.
///
/// This is the surface FR-061 was argued from, so the test is written to be
/// unsatisfiable by the fake FR-061 replaces. Without a row span, a ninety
/// minute event over ninety-six quarter-hour tracks can only be positioned by
/// computed `Spacer` siblings, and
/// `.agents/notes/implemented/feature/2026-08-22-petra-insets-and-padding-prop.md`
/// records why that degrades rather than merely being ugly: a rigid spacer and
/// a text label land in the same priority group in `stack::distribute`, the
/// label is usually the more flexible of the two, and `concede`'s
/// `FlexibleSlack` step therefore takes from the label — the content — before
/// it touches the spacer. So the tree is walked here, not trusted: the fixture
/// declares one grid and nothing else, and a spacer appearing in it fails the
/// test before a single rect is looked at.
///
/// What the frame is then held to, in this order — the pair claims first,
/// because they are SC-014's headline and because a failure there is the one
/// that names two events rather than a coordinate:
///
/// * one container in the frame, and it is the grid;
/// * no two cells share a unit of the surface, in logical units or in device
///   pixels;
/// * two events declared to run at once cover exactly the time they share, and
///   sit in adjacent columns rather than on top of each other;
/// * an event's extent is its duration: `n` quarter-hour tracks tall, so the
///   ninety-minute block is six times the fifteen-minute one and three times
///   the thirty-minute one;
/// * the time axis is continuous — cells that start on a row share a top edge,
///   and consecutive blocks in a column hand off with no seam, in logical
///   units and after device rounding;
/// * the day axis runs left to right, each event stays inside its day's band,
///   and a block with nothing beside it covers that band's whole width.
///
/// What it does not cover: a day with a genuinely empty afternoon. Petra's
/// grid seats by a forward-only cursor and takes no cell addresses, so an
/// empty cell in the middle of a row is unreachable without a filler child —
/// see [`week_cells`]. The spec names the container that would fix it
/// (children at declared rects) as deferred, not missing.
#[test]
fn a_week_view_lays_out_from_one_grid() {
    let cells = week_cells();

    // --- The fixture's own shape ------------------------------------------
    // Checked before anything is laid out. "A ninety-minute event is not the
    // height of a thirty-minute one" is worth nothing if the fixture quietly
    // stopped containing either of them.
    let events: Vec<usize> = cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.role == CellRole::Event)
        .map(|(i, _)| i)
        .collect();
    let quarter_at = *events
        .iter()
        .find(|&&i| cells[i].nrows == 1)
        .expect("the fixture schedules a fifteen-minute block: it is the unit");
    let ninety_at = *events
        .iter()
        .find(|&&i| cells[i].nrows == 6)
        .expect("the fixture schedules a ninety-minute block: six of ninety-six row tracks, FR-061's own example");
    let thirty_at = *events
        .iter()
        .find(|&&i| cells[i].nrows == 2)
        .expect("the fixture schedules a thirty-minute block, the one a ninety-minute block is measured against");
    assert!(
        events.iter().any(|&i| cells[i].nrows == DAY_SLOTS),
        "the fixture must schedule one all-day block, so a span that covers \
         every row track the grid has is exercised"
    );
    assert!(
        events.iter().any(|&i| cells[i].row == 1),
        "the fixture must schedule a block that starts on the first time track"
    );
    assert!(
        events.iter().any(|&i| cells[i].slot_end() == DAY_SLOTS),
        "the fixture must schedule a block that ends on the last time track"
    );

    // Pairs that run at once. They are the reason a day has two lanes, and the
    // reason SC-014 says "overlapping events in adjacent columns". The tuple
    // is ordered (left lane, right lane), not by declaration order: a lane's
    // chains change at different moments, so the later-declared block of a
    // pair is not always the right-hand one.
    let mut concurrent: Vec<(usize, usize)> = Vec::new();
    for (n, &i) in events.iter().enumerate() {
        for &j in &events[n + 1..] {
            let (a, b) = (&cells[i], &cells[j]);
            if a.day != b.day || a.lane.is_none() || b.lane.is_none() || a.lane == b.lane {
                continue;
            }
            if a.slot() < b.slot_end() && b.slot() < a.slot_end() {
                concurrent.push(if a.lane == Some(0) { (i, j) } else { (j, i) });
            }
        }
    }
    assert!(
        concurrent
            .iter()
            .any(|&(a, b)| cells[a].row == cells[b].row && cells[a].nrows == cells[b].nrows),
        "the fixture must schedule one pair that runs at exactly the same time"
    );
    assert!(
        concurrent
            .iter()
            .any(|&(a, b)| cells[a].row != cells[b].row || cells[a].nrows != cells[b].nrows),
        "the fixture must schedule one pair that runs at overlapping but not \
         identical times: a partial overlap is the case a whole-cell fake gets \
         right by accident"
    );

    // --- One grid, and no spacer in it ------------------------------------
    // The tree, not the frame: a spacer-positioned fake still petrifies to
    // rects that tile, so the refusal has to be made where the fake would be
    // written.
    fn tally(node: &ViewNode, out: &mut BTreeMap<&'static str, usize>) {
        *out.entry(node.kind.as_str()).or_default() += 1;
        for child in &node.children {
            tally(child, out);
        }
    }
    let declared = week_view(&cells, 0.0);
    let mut kinds = BTreeMap::new();
    tally(&declared, &mut kinds);
    assert_eq!(
        kinds.get("spacer"),
        None,
        "a spacer reached the week view. SC-014 asks for zero spacer-based \
         positioning, and FR-005's concession order is why: a rigid spacer and \
         a text label share a priority group, and `concede` takes from the \
         label first"
    );
    assert_eq!(
        kinds.get("grid").copied(),
        Some(1),
        "the week view lays out from one grid, and this tree declares {kinds:?}"
    );
    assert_eq!(
        kinds.get("text").copied(),
        Some(cells.len()),
        "every cell of the week is one text node, and this tree declares \
         {kinds:?} for {} cells",
        cells.len()
    );
    assert_eq!(
        kinds.len(),
        2,
        "the week view is a grid of text and nothing else; this tree also \
         declares {kinds:?}"
    );

    // --- The sweep ---------------------------------------------------------
    assert!(
        SCALES.contains(&1.25) && SCALES.contains(&1.5) && SCALES.contains(&1.75),
        "SC-014 is a claim at every display scale in the US1 matrix, and \
         SCALES no longer carries the fractional ones (1.25, 1.5, 1.75) that \
         make it a claim about anything"
    );
    let mut rotation = 0usize;
    for &viewport in SIZES {
        for &factor in SCALES {
            // Column spacing is sampled by rotation over the sweep, the way
            // the matrix samples its own spacings. Every gap meets every size
            // and every scale somewhere.
            let column_spacing = SPACINGS[rotation % SPACINGS.len()];
            rotation += 1;
            let where_ = format!(
                "{}x{} @{factor} gap={column_spacing}",
                viewport.w, viewport.h
            );
            let scale = Scale::new(factor).expect("the matrix scales are positive and finite");
            let node = week_view(&cells, column_spacing);
            let mut harness = Harness::new();
            harness.scale = scale;
            let frame = petrify(
                1,
                validated(&node),
                &mut harness.ctx(),
                Viewport::new(viewport, ThemeMode::Dark).with_scale(scale),
                TransitionActivity::default(),
            );

            // Placements arrive in declaration order, one per cell, so a cell
            // and its placement pair up by index — after that is checked by
            // id rather than assumed.
            assert_eq!(
                frame.placements.len(),
                cells.len() + 1,
                "{where_}: {} cells petrified to {} placements; the grid and \
                 one placement per cell is {}",
                cells.len(),
                frame.placements.len(),
                cells.len() + 1
            );
            for (i, cell) in cells.iter().enumerate() {
                assert_eq!(
                    frame.placements[i + 1].id,
                    format!("/week/{}", cell.key),
                    "{where_}: placement {} is not {}'s; the frame reordered \
                     the grid's children",
                    i + 1,
                    cell.key
                );
            }
            let rect = |i: usize| frame.placements[i + 1].rect;

            // --- W1. One grid, in the frame as well as in the tree ---------
            let containers: Vec<&Placement> = frame
                .placements
                .iter()
                .filter(|p| p.kind.is_container())
                .collect();
            assert_eq!(
                containers.len(),
                1,
                "{where_}: the week view must petrify to exactly one \
                 container, and this frame has {}",
                containers.len()
            );
            assert_eq!(
                containers[0].kind,
                NodeKind::Grid,
                "{where_}: the one container must be the grid, and it is a \
                 {:?}",
                containers[0].kind
            );
            let grid_rect = frame.placements[0].rect;

            // --- W2. Nothing draws on top of anything else -----------------
            for (i, cell) in cells.iter().enumerate() {
                assert!(
                    contains(grid_rect, rect(i)),
                    "{where_}: {} ({}) at {:?} is outside the week grid at \
                     {grid_rect:?}",
                    cell.key,
                    cell.label,
                    rect(i)
                );
            }
            for (i, a) in cells.iter().enumerate() {
                for (j, b) in cells.iter().enumerate().skip(i + 1) {
                    assert!(
                        !rect(i).overlaps(rect(j)),
                        "{where_}: {} ({}) at {:?} and {} ({}) at {:?} \
                         overlap; two cells of one grid never share a unit of \
                         the surface",
                        a.key,
                        a.label,
                        rect(i),
                        b.key,
                        b.label,
                        rect(j)
                    );
                    assert!(
                        !device_overlaps(round_rect(rect(i), scale), round_rect(rect(j), scale)),
                        "{where_}: {} ({}) and {} ({}) overlap in device \
                         pixels at scale {factor}, though their logical rects \
                         {:?} and {:?} do not",
                        a.key,
                        a.label,
                        b.key,
                        b.label,
                        rect(i),
                        rect(j)
                    );
                }
            }

            // The quarter-hour track, read off the frame rather than derived:
            // a one-track span is the track's own extent, untouched.
            let unit = rect(quarter_at).h;

            // The claim C3-5 asks for, made on two named blocks rather than
            // only through the general rule below: three times the duration is
            // three times the extent, whatever the window and whatever the
            // scale.
            let (thirty, ninety) = (rect(thirty_at).h, rect(ninety_at).h);
            assert!(
                (ninety - 3.0 * thirty).abs() <= slot_slack(6, unit),
                "{where_}: {} ({}) runs ninety minutes and stands {ninety} \
                 units tall while {} ({}) runs thirty and stands {thirty}; a \
                 ninety-minute event is three thirty-minute ones",
                cells[ninety_at].key,
                cells[ninety_at].label,
                cells[thirty_at].key,
                cells[thirty_at].label
            );

            // --- W3. Events that overlap in time sit side by side ----------
            // SC-014's headline, claimed in both directions: the pair covers
            // exactly the time it was declared to share, and none of the same
            // surface. Drop either event's row span and the first half of this
            // goes red naming both of them.
            for &(left, right) in &concurrent {
                let (a, b) = (&cells[left], &cells[right]);
                let (ra, rb) = (rect(left), rect(right));
                let shared = a.slot_end().min(b.slot_end()) - a.slot().max(b.slot());
                let want = shared as f32 * unit;
                let seen = ra.bottom().min(rb.bottom()) - ra.y.max(rb.y);
                assert!(
                    (seen - want).abs() <= slot_slack(shared, unit),
                    "{where_}: {} ({}) and {} ({}) overlap in time for \
                     {shared} quarter-hour tracks, which is {want} units, but \
                     their rects {ra:?} and {rb:?} share {seen} units of the \
                     time axis",
                    a.key,
                    a.label,
                    b.key,
                    b.label
                );
                assert!(
                    ra.right() <= rb.x + EPS,
                    "{where_}: {} ({}) and {} ({}) overlap in time, so they \
                     must sit in adjacent columns; {} ends at x {} and {} \
                     starts at x {}",
                    a.key,
                    a.label,
                    b.key,
                    b.label,
                    a.key,
                    ra.right(),
                    b.key,
                    rb.x
                );
            }

            // --- W4. An event's extent is its duration ---------------------
            for (i, cell) in cells.iter().enumerate().filter(|(_, c)| c.row > 0) {
                let want = cell.nrows as f32 * unit;
                assert!(
                    (rect(i).h - want).abs() <= slot_slack(cell.nrows, unit),
                    "{where_}: {} ({}) covers {} quarter-hour tracks, so it \
                     stands {want} units tall, and it is {}; a ninety-minute \
                     event is not the height of a thirty-minute one",
                    cell.key,
                    cell.label,
                    cell.nrows,
                    rect(i).h
                );
            }
            for (i, cell) in cells.iter().enumerate().filter(|(_, c)| c.row == 0) {
                assert!(
                    (rect(i).h - WEEK_HEADER_H).abs() <= EPS,
                    "{where_}: the header cell {} stands {} units tall, not \
                     the {WEEK_HEADER_H} it declares, so the time axis does \
                     not start where it says it does",
                    cell.label,
                    rect(i).h
                );
            }

            // --- W5. The time axis is one continuous run -------------------
            let mut first_on_row: Vec<Option<(usize, f32)>> = vec![None; WEEK_ROWS];
            for (i, cell) in cells.iter().enumerate() {
                match first_on_row[cell.row] {
                    None => first_on_row[cell.row] = Some((i, rect(i).y)),
                    Some((first, y)) => assert_eq!(
                        rect(i).y,
                        y,
                        "{where_}: {} ({}) and {} ({}) both start on row {}, \
                         so they share a top edge to the last bit of the \
                         float; they are at y {} and y {y}",
                        cell.key,
                        cell.label,
                        cells[first].key,
                        cells[first].label,
                        cell.row,
                        rect(i).y
                    ),
                }
            }
            for col in 0..WEEK_COLS {
                let chain: Vec<usize> = cells
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.row > 0 && c.col <= col && col < c.col + c.ncols)
                    .map(|(i, _)| i)
                    .collect();
                for pair in chain.windows(2) {
                    let (a, b) = (&cells[pair[0]], &cells[pair[1]]);
                    assert_eq!(
                        a.row + a.nrows,
                        b.row,
                        "{where_}: the fixture's column {col} runs {} ({}) \
                         into {} ({}) with a hole between them; a flow-seated \
                         grid cannot step over one",
                        a.key,
                        a.label,
                        b.key,
                        b.label
                    );
                    let (ra, rb) = (rect(pair[0]), rect(pair[1]));
                    assert_eq!(
                        ra.bottom(),
                        rb.y,
                        "{where_}: {} ({}) ends at y {} and {} ({}) begins at \
                         y {}; column {col}'s time axis has a gap in it. The \
                         two are one number, so they are one `f32`: \
                         `seam_extent` reads a run's far edge back out of the \
                         offsets its neighbour starts from",
                        a.key,
                        a.label,
                        ra.bottom(),
                        b.key,
                        b.label,
                        rb.y
                    );
                    // Exact, with no tolerance, and the exactness is earned
                    // rather than assumed. Two rects round to the same device
                    // edge only if they are the same `f32` first: rounding is
                    // per-coordinate, so a boundary that lands on an exact
                    // half pixel sends a pair that disagrees in the last bit
                    // to two different device rows.
                    //
                    // A single-track cell always had that, because its far
                    // edge *is* the next track's origin — the same float from
                    // the same addition in `cumulative_offsets`, which is what
                    // `abutting_grid_cells_share_a_device_edge_from_a_shifted_origin`
                    // pins. A spanning run did not, until 2026-08-22: this
                    // test found a run of 8 rows at 401x307 @1.0 ending on
                    // device row 162 while its neighbour began on 163, a
                    // visible gap between two quarter-hours on a display with
                    // no fractional scaling at all. `Cells::of` had summed the
                    // run's own tracks with `span_extent` instead of reading
                    // the neighbour's origin back.
                    //
                    // `layout::grid::seam_extent` closed it, and
                    // `a_spanning_run_ends_on_the_float_its_neighbour_begins_at`
                    // in that module holds the raw `f32` down at the unit
                    // level. This assertion is the same claim at the surface
                    // level: over the whole sweep, and after rounding. If it
                    // ever needs a tolerance again, the tolerance is the bug.
                    let (da, db) = (round_rect(ra, scale), round_rect(rb, scale));
                    assert_eq!(
                        da.bottom(),
                        db.y,
                        "{where_}: {} ({}) and {} ({}) abut in logical units \
                         but land on device rows {} and {} at scale {factor}; \
                         a seam opened between two quarter-hours",
                        a.key,
                        a.label,
                        b.key,
                        b.label,
                        da.bottom(),
                        db.y
                    );
                }
            }

            // --- W6. The day axis ------------------------------------------
            let band: Vec<Rect> = (0..WEEK_DAYS)
                .map(|day| {
                    let i = cells
                        .iter()
                        .position(|c| c.role == CellRole::DayHeader && c.day == Some(day))
                        .expect("every day carries a header cell");
                    rect(i)
                })
                .collect();
            for (day, b) in band.iter().enumerate().skip(1) {
                assert!(
                    band[day - 1].right() <= b.x + EPS,
                    "{where_}: {} ends at x {} and {} starts at x {}; the days \
                     run left to right and never cross",
                    DAY_NAMES[day - 1],
                    band[day - 1].right(),
                    DAY_NAMES[day],
                    b.x
                );
            }
            for (i, cell) in cells
                .iter()
                .enumerate()
                .filter(|(_, c)| c.role == CellRole::Event)
            {
                let day = cell.day.expect("an event names the day it is on");
                let (r, b) = (rect(i), band[day]);
                assert!(
                    r.x >= b.x - EPS && r.right() <= b.right() + EPS,
                    "{where_}: {} ({}) at {r:?} is outside {}'s column band \
                     {b:?}",
                    cell.key,
                    cell.label,
                    DAY_NAMES[day]
                );
                match cell.lane {
                    None => assert!(
                        (r.x - b.x).abs() <= EPS && (r.right() - b.right()).abs() <= EPS,
                        "{where_}: {} ({}) runs with nothing beside it, so its \
                         two-column span covers {}'s whole band {b:?}; it \
                         covers {r:?}",
                        cell.key,
                        cell.label,
                        DAY_NAMES[day]
                    ),
                    Some(0) => assert!(
                        (r.x - b.x).abs() <= EPS,
                        "{where_}: {} ({}) is in {}'s left lane, so it starts \
                         where the band does; the band starts at x {} and it \
                         starts at x {}",
                        cell.key,
                        cell.label,
                        DAY_NAMES[day],
                        b.x,
                        r.x
                    ),
                    Some(_) => assert!(
                        (r.right() - b.right()).abs() <= EPS,
                        "{where_}: {} ({}) is in {}'s right lane, so it ends \
                         where the band does; the band ends at x {} and it \
                         ends at x {}",
                        cell.key,
                        cell.label,
                        DAY_NAMES[day],
                        b.right(),
                        r.right()
                    ),
                }
            }
            for (i, cell) in cells
                .iter()
                .enumerate()
                .filter(|(_, c)| c.role == CellRole::HourLabel)
            {
                assert!(
                    rect(i).right() <= band[0].x + EPS,
                    "{where_}: the hour label {} ends at x {} and {} begins at \
                     x {}; the time axis stays left of the days",
                    cell.label,
                    rect(i).right(),
                    DAY_NAMES[0],
                    band[0].x
                );
            }

            // --- W7. FR-006: one configuration, one digest -----------------
            let mut again = Harness::new();
            again.scale = scale;
            let repeat = petrify(
                1,
                validated(&node),
                &mut again.ctx(),
                Viewport::new(viewport, ThemeMode::Dark).with_scale(scale),
                TransitionActivity::default(),
            );
            assert_eq!(
                frame.digest, repeat.digest,
                "{where_}: the same week petrified twice to two digests"
            );
        }
    }
}
