//! Host-owned flying focus indicator.
//!
//! The *geometry* of all three figures is
//! [`gorgon_petra::token::FocusRing`]. The *flight* is this type.
//!
//! # Four bands, one per edge
//!
//! The caret is four 4-vector springs `(x, y, w, h)`, one per edge of the
//! node: top, right, bottom, left. A band may have zero extent. Every figure
//! is a position of those four:
//!
//! | Figure | top | right | bottom | left |
//! |---|---|---|---|---|
//! | `Border` | inside the top edge | inside the right | inside the bottom | inside the left |
//! | `BarUnder` | flat on the top edge | flat on the right | the bar, below the node | flat on the left |
//! | `Sides` | flat on the top edge | the right bar, outside | flat on the bottom | the left bar, outside |
//!
//! "Flat" is zero depth: a line along the edge the band belongs to, with no
//! thickness. So every transition is one spring per band and none of them is
//! a special case:
//!
//! * `bar → border` — the bottom band walks up from below the node onto its
//!   bottom edge and widens to full; the other three thicken out of their
//!   own edges.
//! * `sides → border` — left and right walk inward and thin from
//!   `thickness` to `stroke`; top and bottom close across.
//! * `bar → sides` — the bottom band flattens onto the node's bottom edge
//!   while left and right thicken out of the node's own sides.
//!
//! This subsumes the split/merge the two-quad model did for underline and
//! hug: the figure stays a consequence of geometry and the springs never
//! learn which figure they are drawing.
//!
//! Three figures make three unordered pairs, and the two-quad model could
//! express exactly one of them — a closed ring is not two quads. That is why
//! this is four.
//!
//! # Flight is not the settled picture
//!
//! Flight never enters petrify or the digest. Mid-flight pixels differ from a
//! jump; a settled frame matches one. Screenshot lanes already wait for
//! settle.
//!
//! That contract is what lets the painter draw the two states differently,
//! and it does: a settled caret is drawn as its figure is designed — the ring
//! takes the node's corner radius and lays its ground-coloured halo inside
//! itself, the bar casts the shadow that seats it on the card — while a
//! caret in flight is the four bands, filled flat. The halo answers "the ring
//! disappears into a fill of its own colour", and the shadow answers "a
//! 3-unit strip floats off the card"; both are questions about a control the
//! operator is looking at, not about 160 ms of travel. What does *not* change
//! at settle is where any band is: the springs land on the figure's own
//! geometry, so nothing moves on that frame.
//!
//! Reduced motion snaps. First focus spawns on the target so the bands do not
//! streak from the origin. The hop is a 160 ms spring at every distance.
//! Skipping came from a fat `dt` after idle (SC-002 paints nothing, so the
//! next Tab can carry seconds). A gap larger than a vsync is one frame of
//! progress, not a clamp on every tick. Nothing focused hides it. SC-002:
//! once settled the host stops asking for frames.

use egui::Rect;
use gorgon_petra::anim::Spring;
use gorgon_petra::tree::FocusFigure;

/// How close each component must be, in logical units, before we snap.
const SETTLE_POS: f64 = 0.5;
/// How quiet velocity must be, in logical units per second, before we snap.
const SETTLE_VEL: f64 = 10.0;
/// Stuck-clock fallback, and the stand-in for an idle gap.
const FRAME_DT: f64 = 1.0 / 60.0;
/// Bigger than a 30 Hz vsync, smaller than "we were asleep". Real frames
/// (4 ms at 240 Hz, 16 ms at 60 Hz) pass through so a 160 ms hop stays
/// 160 ms of wall time. A gap above this is replaced with [`FRAME_DT`],
/// not clamped — `min(dt, 1/120)` made 60 Hz take twice as long.
const IDLE_GAP: f64 = 0.04;
const HOP_DURATION: f64 = 0.16;
const HOP_BOUNCE: f64 = 0.12;

/// How many bands the caret flies: one per edge of the node.
pub const BANDS: usize = 4;

/// Where the caret is flying to: the focused id, the destination node, the
/// four bands at that destination, the figure they add up to, its `radius`
/// token, and the clip the caret may not paint past.
#[derive(Clone, Copy, Debug)]
pub struct CaretDest<'a> {
    /// The focused placement's id. The spring keys its flight on this.
    pub id: &'a str,
    /// The rect the figure is drawn on, device-snapped.
    pub mark: Rect,
    /// The bands at the destination, in edge order: top, right, bottom, left.
    pub bands: [Rect; BANDS],
    /// The figure those bands add up to.
    pub figure: FocusFigure,
    /// The destination's `radius` token, snapped rather than sprung: a corner
    /// radius is a shape token rather than one of the four numbers a band's
    /// spring owns, and only the settled `Border` reads it.
    pub radius: Option<&'a str>,
    /// The clip the caret may not paint past.
    pub clip: Rect,
}

/// The four bands that fly between focus targets.
#[derive(Debug)]
pub struct FocusCaret {
    spring: Spring,
    reduced_motion: bool,
    last_time: Option<f64>,
    id: Option<String>,
    /// Four bands, each `(x, y, w, h)`, in edge order.
    x: [[f64; 4]; BANDS],
    v: [[f64; 4]; BANDS],
    visible: bool,
    moving: bool,
    figure: FocusFigure,
    radius: Option<String>,
    mark: Rect,
    clip: Rect,
}

impl Default for FocusCaret {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusCaret {
    /// Hidden, reduced-motion off, default spring.
    #[must_use]
    pub fn new() -> Self {
        Self {
            spring: Spring::duration_bounce(HOP_DURATION, HOP_BOUNCE)
                .expect("0.16 s / bounce 0.12 is inside Spring's legal range"),
            reduced_motion: false,
            last_time: None,
            id: None,
            x: [[0.0; 4]; BANDS],
            v: [[0.0; 4]; BANDS],
            visible: false,
            moving: false,
            figure: FocusFigure::default(),
            radius: None,
            mark: Rect::NOTHING,
            clip: Rect::NOTHING,
        }
    }

    /// Snap instead of interpolating. Also snaps the current bands, if any, on
    /// the next [`Self::tick`].
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
        if on {
            self.v = [[0.0; 4]; BANDS];
            self.moving = false;
        }
    }

    /// Whether a later pass must be scheduled so the bands can keep moving.
    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.moving
    }

    /// The interpolated bands this frame, or `None` when nothing is focused.
    ///
    /// Bands with no area are dropped: a `BarUnder` is one band and three
    /// flat lines, and painting a zero-height rect is work with no pixels.
    /// Overlap is not dropped — a settled `Border`'s four bands meet at the
    /// corners, and filling the same opaque colour twice is the same pixels.
    #[must_use]
    pub fn bands(&self) -> Option<Vec<Rect>> {
        if !self.visible {
            return None;
        }
        Some(
            self.x
                .iter()
                .map(|band| rect_of(*band))
                .filter(|r| r.width() > 0.0 && r.height() > 0.0)
                .collect(),
        )
    }

    /// First painted band, for tests that track one rectangle.
    #[must_use]
    pub fn rect(&self) -> Option<Rect> {
        self.bands().and_then(|bands| bands.into_iter().next())
    }

    /// The figure of the current destination.
    #[must_use]
    pub fn figure(&self) -> FocusFigure {
        self.figure
    }

    /// The destination node rect, which is what a **settled** caret is drawn
    /// from: the figure as it is designed, not the four flat bands.
    #[must_use]
    pub fn mark(&self) -> Rect {
        self.mark
    }

    /// The `radius` token of the current destination, if it named one.
    #[must_use]
    pub fn radius(&self) -> Option<&str> {
        self.radius.as_deref()
    }

    /// Composer clip for the current target. The caret may not paint past it
    /// once settled.
    #[must_use]
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// The id the springs are currently flying toward, if any.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Retarget to this frame's destination, or hide when `target` is `None`.
    /// `now` is `egui::InputState::time`.
    pub fn tick(&mut self, target: Option<CaretDest<'_>>, now: f64) {
        let dt = self.take_dt(now);
        let Some(dest) = target else {
            self.id = None;
            self.visible = false;
            self.moving = false;
            self.v = [[0.0; 4]; BANDS];
            return;
        };
        self.figure = dest.figure;
        self.radius = dest.radius.map(str::to_owned);
        self.mark = dest.mark;
        self.clip = dest.clip;
        let goal = [
            components(dest.bands[0]),
            components(dest.bands[1]),
            components(dest.bands[2]),
            components(dest.bands[3]),
        ];
        let spawn = !self.visible || self.reduced_motion;
        if spawn {
            self.x = goal;
            self.v = [[0.0; 4]; BANDS];
            self.visible = true;
            self.moving = false;
            self.id = Some(dest.id.to_owned());
            return;
        }
        let mut still = false;
        // `Spring` is `Copy`, so taking it by value here splits the borrow:
        // the loop below holds `self.x` and `self.v` mutably at the same time.
        let spring = self.spring;
        for (band, (xs, vs)) in self.x.iter_mut().zip(self.v.iter_mut()).enumerate() {
            let target = &goal[band];
            for (i, (x, v)) in xs.iter_mut().zip(vs.iter_mut()).enumerate() {
                let (p, vel) = spring.evaluate_scalar(*x, *v, target[i], dt);
                *x = p;
                *v = vel;
                if (p - target[i]).abs() > SETTLE_POS || vel.abs() > SETTLE_VEL {
                    still = true;
                }
            }
            // Width and height are extents, never negative: a spring that
            // undershoots its target would otherwise hand the painter an
            // inverted rect.
            xs[2] = xs[2].max(0.0);
            xs[3] = xs[3].max(0.0);
        }
        if !still {
            self.x = goal;
            self.v = [[0.0; 4]; BANDS];
        }
        self.moving = still;
        self.visible = true;
        self.id = Some(dest.id.to_owned());
    }

    fn take_dt(&mut self, now: f64) -> f64 {
        let dt = match self.last_time {
            Some(prev) if now > prev => now - prev,
            _ => 0.0,
        };
        self.last_time = Some(now);
        if dt <= 0.0 || dt > IDLE_GAP {
            FRAME_DT
        } else {
            dt
        }
    }
}

fn components(band: Rect) -> [f64; 4] {
    [
        f64::from(band.min.x),
        f64::from(band.min.y),
        f64::from(band.width()),
        f64::from(band.height()),
    ]
}

fn rect_of(x: [f64; 4]) -> Rect {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    Rect::from_min_size(
        egui::pos2(x[0] as f32, x[1] as f32),
        egui::vec2(x[2].max(0.0) as f32, x[3].max(0.0) as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::{BANDS, CaretDest, FRAME_DT, FocusCaret};
    use egui::{Rect, pos2, vec2};
    use gorgon_petra::geom::{Rect as PetraRect, Scale};
    use gorgon_petra::token::FocusRing;
    use gorgon_petra::tree::FocusFigure;

    fn bar(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    fn wide_open() -> Rect {
        Rect::from_min_max(pos2(-10_000.0, -10_000.0), pos2(10_000.0, 10_000.0))
    }

    /// The destination for `node` wearing `figure`, built the way the host
    /// builds it — through `paint::caret_bands`, so these tests fly the same
    /// geometry the painter draws.
    fn dest(id: &str, mark: Rect, figure: FocusFigure) -> CaretDest<'_> {
        CaretDest {
            id,
            mark,
            bands: crate::paint::caret_bands(mark, figure, Scale::ONE),
            figure,
            radius: None,
            clip: wide_open(),
        }
    }

    /// A destination whose bands are given directly, for the two legacy
    /// spawn/settle tests that name their own rects.
    fn raw<'a>(id: &'a str, bands: [Rect; BANDS], figure: FocusFigure) -> CaretDest<'a> {
        CaretDest {
            id,
            mark: bands[2],
            bands,
            figure,
            radius: None,
            clip: wide_open(),
        }
    }

    fn under(id: &str, r: Rect) -> CaretDest<'_> {
        raw(
            id,
            [
                Rect::from_min_size(r.min, vec2(r.width(), 0.0)),
                Rect::from_min_size(r.right_top(), vec2(0.0, 0.0)),
                r,
                Rect::from_min_size(r.min, vec2(0.0, 0.0)),
            ],
            FocusFigure::BarUnder,
        )
    }

    fn advance(caret: &mut FocusCaret, target: CaretDest<'_>, frames: usize, t0: f64) {
        for i in 0..frames {
            caret.tick(Some(target), t0 + FRAME_DT * (i as f64 + 1.0));
        }
    }

    fn node_rect() -> Rect {
        Rect::from_min_size(pos2(40.0, 60.0), vec2(120.0, 40.0))
    }

    #[test]
    fn first_focus_spawns_on_the_target_not_the_origin() {
        let mut caret = FocusCaret::new();
        let target = bar(80.0, 40.0, 60.0, 3.0);
        caret.tick(Some(under("a", target)), 0.0);
        assert_eq!(caret.rect(), Some(target));
        assert_eq!(
            caret.bands().map(|b| b.len()),
            Some(1),
            "a bar under is one band and three flat lines"
        );
        assert!(!caret.is_moving(), "a spawn is a placement, not a flight");
        assert_eq!(caret.id(), Some("a"));
    }

    #[test]
    fn a_retarget_moves_the_bar_toward_the_new_node() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(120.0, 0.0, 90.0, 3.0);
        caret.tick(Some(under("a", a)), 0.0);
        caret.tick(Some(under("b", b)), FRAME_DT);
        let mid = caret.rect().expect("visible in flight");
        assert!(caret.is_moving());
        assert!(
            mid.min.x > a.min.x && mid.min.x < b.min.x,
            "x should be between the two targets, got {}",
            mid.min.x
        );
        assert!(
            mid.width() > a.width() && mid.width() < b.width(),
            "width should ease, got {}",
            mid.width()
        );
        assert_eq!(caret.bands().map(|b| b.len()), Some(1));
    }

    #[test]
    fn reduced_motion_snaps_on_the_next_tick() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(200.0, 40.0, 40.0, 3.0);
        caret.tick(Some(under("a", a)), 0.0);
        caret.set_reduced_motion(true);
        caret.tick(Some(under("b", b)), FRAME_DT);
        assert_eq!(caret.rect(), Some(b));
        assert!(!caret.is_moving());
    }

    #[test]
    fn nothing_focused_hides_the_bar() {
        let mut caret = FocusCaret::new();
        caret.tick(Some(under("a", bar(10.0, 10.0, 40.0, 3.0))), 0.0);
        caret.tick(None, FRAME_DT);
        assert_eq!(caret.rect(), None);
        assert!(!caret.is_moving());
        assert_eq!(caret.id(), None);
    }

    #[test]
    fn a_flight_settles_and_stops_asking_for_frames() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(180.0, 24.0, 72.0, 3.0);
        caret.tick(Some(under("a", a)), 0.0);
        advance(&mut caret, under("b", b), 45, 0.0);
        assert!(
            !caret.is_moving(),
            "45 frames at 60 Hz is 0.75 s, well past 160 ms"
        );
        let landed = caret.rect().expect("still visible once settled");
        assert!((landed.min.x - b.min.x).abs() < 0.01);
        assert!((landed.min.y - b.min.y).abs() < 0.01);
        assert!((landed.width() - b.width()).abs() < 0.01);
        assert!((landed.height() - b.height()).abs() < 0.01);
    }

    #[test]
    fn a_stuck_clock_still_makes_progress() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(120.0, 0.0, 60.0, 3.0);
        caret.tick(Some(under("a", a)), 1.0);
        caret.tick(Some(under("b", b)), 1.0);
        let mid = caret.rect().expect("visible");
        assert!(
            mid.min.x > 0.0,
            "dt=0 must fall back to one frame, not freeze at the origin of the flight"
        );
    }

    #[test]
    fn an_idle_dt_does_not_skip_a_long_flight() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 40.0, 3.0);
        let b = bar(900.0, 700.0, 40.0, 3.0);
        caret.tick(Some(under("a", a)), 0.0);
        caret.tick(Some(under("b", b)), 8.0);
        let mid = caret.rect().expect("visible");
        assert!(caret.is_moving(), "a 8 s gap must not land in one paint");
        let dist = ((mid.min.x - a.min.x).powi(2) + (mid.min.y - a.min.y).powi(2)).sqrt();
        assert!(
            dist < 400.0,
            "one tick after idle must lerp, not leap; moved {dist}"
        );
    }

    #[test]
    fn a_60hz_frame_is_not_halved() {
        let a = bar(0.0, 0.0, 40.0, 3.0);
        let b = bar(900.0, 0.0, 40.0, 3.0);
        let mut full = FocusCaret::new();
        full.tick(Some(under("a", a)), 0.0);
        full.tick(Some(under("b", b)), 1.0 / 60.0);
        let mut half = FocusCaret::new();
        half.tick(Some(under("a", a)), 0.0);
        half.tick(Some(under("b", b)), 1.0 / 120.0);
        let moved_full = full.rect().expect("visible").min.x;
        let moved_half = half.rect().expect("visible").min.x;
        assert!(
            moved_full > moved_half,
            "a 16 ms vsync dt must advance more than 8 ms; 60Hz={moved_full} 120Hz={moved_half}"
        );
    }

    /// The split the two-quad model did for free, kept: a bar under becomes
    /// two side bars by growing left and right out of the node's own edges
    /// while the bar flattens onto its bottom edge.
    #[test]
    fn a_bar_under_splits_into_sides() {
        let node = node_rect();
        let end = crate::paint::caret_bands(node, FocusFigure::Sides, Scale::ONE);
        let mut caret = FocusCaret::new();
        caret.tick(Some(dest("a", node, FocusFigure::BarUnder)), 0.0);
        assert_eq!(caret.bands().map(|b| b.len()), Some(1));
        caret.tick(Some(dest("b", node, FocusFigure::Sides)), FRAME_DT);
        let bands = caret.bands().expect("visible");
        assert!(caret.is_moving());
        assert_eq!(
            bands.len(),
            3,
            "one frame in, the bar has not flattened away yet and the two \
             sides have started: {bands:?}"
        );
        let live = caret.x;
        assert!(
            live[3][2] > 0.0 && live[3][2] < f64::from(end[3].width()),
            "the left band must be growing, not arrived: {}",
            live[3][2]
        );
        assert!(
            live[3][0] < f64::from(node.min.x) && live[3][0] > f64::from(end[3].min.x),
            "the left band must be walking outward: {}",
            live[3][0]
        );
        assert!(
            live[2][3] > 0.0 && live[2][3] < f64::from(FocusRing::STANDARD.thickness),
            "the bar must be flattening, not gone: {}",
            live[2][3]
        );
        advance(&mut caret, dest("b", node, FocusFigure::Sides), 45, 1.0);
        assert!(!caret.is_moving());
        assert_eq!(caret.bands().map(|b| b.len()), Some(2));
    }

    /// And back: two side bars become one bar under.
    #[test]
    fn sides_merge_into_a_bar_under() {
        let node = node_rect();
        let mut caret = FocusCaret::new();
        caret.tick(Some(dest("a", node, FocusFigure::Sides)), 0.0);
        assert_eq!(caret.bands().map(|b| b.len()), Some(2));
        caret.tick(Some(dest("b", node, FocusFigure::BarUnder)), FRAME_DT);
        assert!(caret.is_moving());
        assert_eq!(
            caret.bands().map(|b| b.len()),
            Some(3),
            "in flight the two sides are still there and the bar has started"
        );
        advance(&mut caret, dest("b", node, FocusFigure::BarUnder), 45, 1.0);
        assert!(!caret.is_moving());
        assert_eq!(caret.bands().map(|b| b.len()), Some(1));
        let landed = caret.rect().expect("merged");
        let bar = crate::paint::caret_bands(node, FocusFigure::BarUnder, Scale::ONE)[2];
        assert!((landed.min.x - bar.min.x).abs() < 0.01);
        assert!((landed.width() - bar.width()).abs() < 0.01);
        assert!((landed.height() - bar.height()).abs() < 0.01);
    }

    #[test]
    fn a_settled_caret_reports_the_node_and_figure_it_landed_on() {
        let mut caret = FocusCaret::new();
        let node = node_rect();
        caret.tick(Some(dest("a", node, FocusFigure::Border)), 0.0);
        assert_eq!(caret.mark(), node);
        assert_eq!(caret.figure(), FocusFigure::Border);
        assert_eq!(
            caret.bands().map(|b| b.len()),
            Some(4),
            "a border is four live bands"
        );
    }

    /// Every one of the three unordered pairs morphs.
    ///
    /// The claim is per band and per frame: on an intermediate frame each
    /// band must lie strictly between where it started and where it is going,
    /// on every component that actually changes. A band that reads its start
    /// value on one frame and its end value on the next is a snap wearing a
    /// spring's clothes, and `between` refuses both ends.
    ///
    /// The two-quad model could express exactly one of these three pairs.
    ///
    /// # How this goes red
    ///
    /// Set `reduced_motion` in the fixture, or give any figure's absent bands
    /// a position other than the edge they belong to — they then arrive from
    /// somewhere the eye can follow, and the `between` window they must sit
    /// inside no longer holds them.
    #[test]
    fn every_pair_of_figures_morphs_band_by_band() {
        use FocusFigure::{BarUnder, Border, Sides};

        /// Strictly between, with room for a spring that has barely left.
        fn between(name: &str, band: usize, comp: usize, from: f32, to: f32, now: f32) {
            if (from - to).abs() < 0.01 {
                assert!(
                    (now - from).abs() < 0.01,
                    "{name}: band {band} component {comp} moved off {from} to \
                     {now} though the two figures put it in the same place"
                );
                return;
            }
            let (lo, hi) = if from < to { (from, to) } else { (to, from) };
            assert!(
                now > lo + 0.01 && now < hi - 0.01,
                "{name}: band {band} component {comp} is at {now}, not strictly \
                 inside ({from} -> {to}). At an end it is a snap, not a morph."
            );
        }

        let node = node_rect();
        for (from, to) in [
            (BarUnder, Sides),
            (BarUnder, Border),
            (Sides, Border),
            (Sides, BarUnder),
            (Border, BarUnder),
            (Border, Sides),
        ] {
            let name = format!("{from:?} -> {to:?}");
            let start = crate::paint::caret_bands(node, from, Scale::ONE);
            let end = crate::paint::caret_bands(node, to, Scale::ONE);

            let mut caret = FocusCaret::new();
            caret.tick(Some(dest("a", node, from)), 0.0);
            assert!(!caret.is_moving(), "{name}: the spawn must not be a flight");

            // Three frames in: far enough that a spring has visibly moved,
            // early enough that it cannot have arrived (the hop is 160 ms,
            // about ten frames).
            for i in 0..3 {
                caret.tick(Some(dest("b", node, to)), FRAME_DT * f64::from(i + 1));
            }
            assert!(caret.is_moving(), "{name}: the caret is not in flight");
            let mid = caret.x;
            for band in 0..BANDS {
                let a = [
                    start[band].min.x,
                    start[band].min.y,
                    start[band].width(),
                    start[band].height(),
                ];
                let b = [
                    end[band].min.x,
                    end[band].min.y,
                    end[band].width(),
                    end[band].height(),
                ];
                for comp in 0..4 {
                    #[allow(clippy::cast_possible_truncation)]
                    between(&name, band, comp, a[comp], b[comp], mid[band][comp] as f32);
                }
            }

            advance(&mut caret, dest("b", node, to), 60, 1.0);
            assert!(!caret.is_moving(), "{name}: the hop never settled");
            for (band, landed) in end.iter().enumerate() {
                assert_eq!(
                    rect_of_f64(caret.x[band]),
                    *landed,
                    "{name}: band {band} did not land on the figure"
                );
            }
        }
    }

    fn rect_of_f64(x: [f64; 4]) -> Rect {
        super::rect_of(x)
    }

    /// A band that is absent in both figures never moves, and a band that is
    /// absent in one grows out of the edge it belongs to rather than flying
    /// in from somewhere else.
    ///
    /// This is the property that makes the model work: "absent" is zero
    /// depth on the node's own edge, not "somewhere off screen".
    #[test]
    fn an_absent_band_sits_flat_on_the_edge_it_belongs_to() {
        let node = PetraRect::new(40.0, 60.0, 120.0, 40.0);
        let e = Rect::from_min_size(pos2(node.x, node.y), vec2(node.w, node.h));

        let bar = crate::paint::caret_bands(e, FocusFigure::BarUnder, Scale::ONE);
        assert_eq!(bar[0].height(), 0.0, "no top band under a bar");
        assert_eq!(bar[0].min.y, e.min.y, "and it waits on the top edge");
        assert_eq!(bar[0].width(), e.width());
        assert_eq!(bar[1].width(), 0.0, "no right band under a bar");
        assert_eq!(bar[1].min.x, e.max.x, "and it waits on the right edge");
        assert_eq!(bar[3].width(), 0.0, "no left band under a bar");
        assert_eq!(bar[3].min.x, e.min.x, "and it waits on the left edge");

        let sides = crate::paint::caret_bands(e, FocusFigure::Sides, Scale::ONE);
        assert_eq!(sides[0].height(), 0.0, "no top band beside a well");
        assert_eq!(sides[2].height(), 0.0, "no bottom band beside a well");
        assert_eq!(sides[2].min.y, e.max.y, "and it waits on the bottom edge");

        let border = crate::paint::caret_bands(e, FocusFigure::Border, Scale::ONE);
        for (i, band) in border.iter().enumerate() {
            assert!(
                band.width() > 0.0 && band.height() > 0.0,
                "a border has four live bands; {i} is {band:?}"
            );
        }
    }
}
