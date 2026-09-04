//! Host-owned flying focus indicator.
//!
//! The bar's *geometry* is [`gorgon_petra::token::FocusRing`] — thickness, gap,
//! two-thirds width, hugs. The *flight* is this type: two 4-vector springs
//! `(x, y, w, h)` that retarget when the focused id changes. An underline is
//! two copies of the same strip; a hug is a left bar and a right bar. A hop
//! from one figure to the other splits or merges those two quads instead of
//! swapping the figure in one paint.
//!
//! Flight never enters petrify or the digest. Mid-flight pixels differ from
//! a jump; a settled frame matches one. Screenshot lanes already wait for
//! settle.
//!
//! Reduced motion snaps. First focus spawns on the target so the bar does not
//! streak from the origin. The hop is a 160 ms spring at every distance.
//! Skipping came from a fat `dt` after idle (SC-002 paints nothing, so the
//! next Tab can carry seconds). A gap larger than a vsync is one frame of
//! progress, not a clamp on every tick. Nothing focused hides it. SC-002:
//! once settled the host stops asking for frames.

use egui::Rect;
use gorgon_petra::anim::Spring;

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

/// How the interpolated node is drawn when settled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CaretFigure {
    /// A bar under the node. Buttons, tabs, radios.
    #[default]
    Underline,
    /// Vertical bars hugging the left and right. Text fields.
    Hug,
}

/// The underline that flies between focus targets.
#[derive(Debug)]
pub struct FocusCaret {
    spring: Spring,
    reduced_motion: bool,
    last_time: Option<f64>,
    id: Option<String>,
    /// Two quads, each `(x, y, w, h)`.
    x: [[f64; 4]; 2],
    v: [[f64; 4]; 2],
    visible: bool,
    moving: bool,
    figure: CaretFigure,
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
            x: [[0.0; 4]; 2],
            v: [[0.0; 4]; 2],
            visible: false,
            moving: false,
            figure: CaretFigure::Underline,
            clip: Rect::NOTHING,
        }
    }

    /// Snap instead of interpolating. Also snaps the current bar, if any, on
    /// the next [`Self::tick`].
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
        if on {
            self.v = [[0.0; 4]; 2];
            self.moving = false;
        }
    }

    /// Whether a later pass must be scheduled so the bar can keep moving.
    #[must_use]
    pub fn is_moving(&self) -> bool {
        self.moving
    }

    /// The interpolated bars this frame, or `None` when nothing is focused.
    ///
    /// Two copies of the same underline collapse to one rect so a settled
    /// strip is not painted twice (the overlay shadow would stack).
    #[must_use]
    pub fn bars(&self) -> Option<Vec<Rect>> {
        if !self.visible {
            return None;
        }
        let a = rect_of(self.x[0]);
        let b = rect_of(self.x[1]);
        if rects_close(a, b) {
            Some(vec![a])
        } else {
            Some(vec![a, b])
        }
    }

    /// First painted bar, for tests that track one rectangle.
    #[must_use]
    pub fn rect(&self) -> Option<Rect> {
        self.bars().and_then(|bars| bars.into_iter().next())
    }

    /// Underline or hug of the current destination.
    #[must_use]
    pub fn figure(&self) -> CaretFigure {
        self.figure
    }

    /// Composer clip for the current target. The bar may not paint past it
    /// once settled.
    #[must_use]
    pub fn clip(&self) -> Rect {
        self.clip
    }

    /// The id the spring is currently flying toward, if any.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Retarget to this frame's focused bars, or hide when `target` is
    /// `None`. `now` is `egui::InputState::time`.
    ///
    /// `dest` is two quads: an underline repeats the strip; a hug is left
    /// then right.
    pub fn tick(&mut self, target: Option<(&str, [Rect; 2], CaretFigure, Rect)>, now: f64) {
        let dt = self.take_dt(now);
        let Some((id, dest_bars, figure, clip)) = target else {
            self.id = None;
            self.visible = false;
            self.moving = false;
            self.v = [[0.0; 4]; 2];
            return;
        };
        self.figure = figure;
        self.clip = clip;
        let dest = [components(dest_bars[0]), components(dest_bars[1])];
        let spawn = !self.visible || self.reduced_motion;
        if spawn {
            self.x = dest;
            self.v = [[0.0; 4]; 2];
            self.visible = true;
            self.moving = false;
            self.id = Some(id.to_owned());
            self.figure = figure;
            return;
        }
        let mut still = false;
        // `Spring` is `Copy`, so taking it by value here splits the borrow:
        // the loop below holds `self.x` and `self.v` mutably at the same time.
        let spring = self.spring;
        for (bar, (xs, vs)) in self.x.iter_mut().zip(self.v.iter_mut()).enumerate() {
            let target = &dest[bar];
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
            self.x = dest;
            self.v = [[0.0; 4]; 2];
        }
        self.moving = still;
        self.visible = true;
        self.id = Some(id.to_owned());
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

fn components(bar: Rect) -> [f64; 4] {
    [
        f64::from(bar.min.x),
        f64::from(bar.min.y),
        f64::from(bar.width()),
        f64::from(bar.height()),
    ]
}

fn rect_of(x: [f64; 4]) -> Rect {
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    Rect::from_min_size(
        egui::pos2(x[0] as f32, x[1] as f32),
        egui::vec2(x[2].max(0.0) as f32, x[3].max(0.0) as f32),
    )
}

fn rects_close(a: Rect, b: Rect) -> bool {
    (a.min.x - b.min.x).abs() < 0.5
        && (a.min.y - b.min.y).abs() < 0.5
        && (a.width() - b.width()).abs() < 0.5
        && (a.height() - b.height()).abs() < 0.5
}

#[cfg(test)]
mod tests {
    use super::{CaretFigure, FRAME_DT, FocusCaret};
    use egui::{Rect, pos2, vec2};

    fn bar(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    fn underline(id: &str, r: Rect) -> (&str, [Rect; 2], CaretFigure, Rect) {
        (
            id,
            [r, r],
            CaretFigure::Underline,
            Rect::from_min_max(pos2(-10_000.0, -10_000.0), pos2(10_000.0, 10_000.0)),
        )
    }

    fn hug(id: &str, left: Rect, right: Rect) -> (&str, [Rect; 2], CaretFigure, Rect) {
        (
            id,
            [left, right],
            CaretFigure::Hug,
            Rect::from_min_max(pos2(-10_000.0, -10_000.0), pos2(10_000.0, 10_000.0)),
        )
    }

    fn advance(
        caret: &mut FocusCaret,
        target: (&str, [Rect; 2], CaretFigure, Rect),
        frames: usize,
        t0: f64,
    ) {
        for i in 0..frames {
            caret.tick(Some(target), t0 + FRAME_DT * (i as f64 + 1.0));
        }
    }

    #[test]
    fn first_focus_spawns_on_the_target_not_the_origin() {
        let mut caret = FocusCaret::new();
        let target = bar(80.0, 40.0, 60.0, 3.0);
        caret.tick(Some(underline("a", target)), 0.0);
        assert_eq!(caret.rect(), Some(target));
        assert_eq!(caret.bars().map(|b| b.len()), Some(1));
        assert!(!caret.is_moving(), "a spawn is a placement, not a flight");
        assert_eq!(caret.id(), Some("a"));
    }

    #[test]
    fn a_retarget_moves_the_bar_toward_the_new_node() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(120.0, 0.0, 90.0, 3.0);
        caret.tick(Some(underline("a", a)), 0.0);
        caret.tick(Some(underline("b", b)), FRAME_DT);
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
        assert_eq!(caret.bars().map(|b| b.len()), Some(1));
    }

    #[test]
    fn reduced_motion_snaps_on_the_next_tick() {
        let mut caret = FocusCaret::new();
        let a = bar(0.0, 0.0, 60.0, 3.0);
        let b = bar(200.0, 40.0, 40.0, 3.0);
        caret.tick(Some(underline("a", a)), 0.0);
        caret.set_reduced_motion(true);
        caret.tick(Some(underline("b", b)), FRAME_DT);
        assert_eq!(caret.rect(), Some(b));
        assert!(!caret.is_moving());
    }

    #[test]
    fn nothing_focused_hides_the_bar() {
        let mut caret = FocusCaret::new();
        caret.tick(Some(underline("a", bar(10.0, 10.0, 40.0, 3.0))), 0.0);
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
        caret.tick(Some(underline("a", a)), 0.0);
        advance(&mut caret, underline("b", b), 45, 0.0);
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
        caret.tick(Some(underline("a", a)), 1.0);
        caret.tick(Some(underline("b", b)), 1.0);
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
        caret.tick(Some(underline("a", a)), 0.0);
        caret.tick(Some(underline("b", b)), 8.0);
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
        full.tick(Some(underline("a", a)), 0.0);
        full.tick(Some(underline("b", b)), 1.0 / 60.0);
        let mut half = FocusCaret::new();
        half.tick(Some(underline("a", a)), 0.0);
        half.tick(Some(underline("b", b)), 1.0 / 120.0);
        let moved_full = full.rect().expect("visible").min.x;
        let moved_half = half.rect().expect("visible").min.x;
        assert!(
            moved_full > moved_half,
            "a 16 ms vsync dt must advance more than 8 ms; 60Hz={moved_full} 120Hz={moved_half}"
        );
    }

    #[test]
    fn an_underline_splits_into_hugs() {
        let mut caret = FocusCaret::new();
        let under = bar(40.0, 80.0, 80.0, 3.0);
        let left = bar(10.0, 20.0, 3.0, 40.0);
        let right = bar(130.0, 20.0, 3.0, 40.0);
        caret.tick(Some(underline("a", under)), 0.0);
        caret.tick(Some(hug("b", left, right)), FRAME_DT);
        let bars = caret.bars().expect("visible");
        assert_eq!(bars.len(), 2, "the strip must split, not snap to hugs");
        assert!(caret.is_moving());
        assert!(
            bars[0].min.x < under.min.x + 1.0,
            "left copy should start moving toward the left hug, got {}",
            bars[0].min.x
        );
        assert!(
            bars[1].min.x > under.min.x,
            "right copy should start moving toward the right hug, got {}",
            bars[1].min.x
        );
        assert!(
            bars[0].height() > under.height() && bars[0].height() < left.height(),
            "height should ease from strip to hug, got {}",
            bars[0].height()
        );
    }

    #[test]
    fn hugs_merge_into_an_underline() {
        let mut caret = FocusCaret::new();
        let left = bar(10.0, 20.0, 3.0, 40.0);
        let right = bar(130.0, 20.0, 3.0, 40.0);
        let under = bar(40.0, 80.0, 80.0, 3.0);
        caret.tick(Some(hug("a", left, right)), 0.0);
        assert_eq!(caret.bars().map(|b| b.len()), Some(2));
        caret.tick(Some(underline("b", under)), FRAME_DT);
        let bars = caret.bars().expect("visible");
        assert_eq!(bars.len(), 2, "two hugs must still be two bars in flight");
        advance(&mut caret, underline("b", under), 45, FRAME_DT);
        assert!(!caret.is_moving());
        assert_eq!(caret.bars().map(|b| b.len()), Some(1));
        let landed = caret.rect().expect("merged");
        assert!((landed.min.x - under.min.x).abs() < 0.01);
        assert!((landed.width() - under.width()).abs() < 0.01);
        assert!((landed.height() - under.height()).abs() < 0.01);
    }
}
