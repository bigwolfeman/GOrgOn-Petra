//! The focus indicator's three figures, and why none of them is a colour.
//!
//! FR-015's rule is that no Petra-provided status may carry meaning by colour
//! alone, and keyboard focus is a status like any other. The indicator
//! answers that with a **shape that is present or absent**: a focused node
//! grows a figure an unfocused node does not have. Print the frame in
//! greyscale and the figure is still there.
//!
//! # The three figures
//!
//! Which one a node wears is [`crate::tree::FocusFigure`]'s, declared by the
//! component. This module owns the measurements, and there is one method per
//! figure:
//!
//! | Figure | Method | Where it lands |
//! |---|---|---|
//! | `Border` | [`FocusRing::bands`] | two concentric strokes **inside** the rect |
//! | `BarUnder` | [`FocusRing::bar`] | one strip **below** the rect |
//! | `Sides` | [`FocusRing::sides`] | two strips **outside** the left and right edges |
//! | `BarInside` | [`FocusRing::bar_inside`] | the same strip, on the rect's **own bottom edge** |
//!
//! The two bars are handed the **marked run** rather than the control's own
//! rect — the control's leading label at the control's vertical extent. See
//! [`crate::focus::marked_rect`] for the rule and why it is not a fraction
//! of the control's width.
//!
//! `Border` is Carbon's, and Petra's **exception** rather than its default —
//! [`crate::tree::FocusFigure`] carries the policy and the operator's rule
//! behind it. `@include focus-outline('outline')`
//! is `outline: 2px solid $focus; outline-offset: -2px`
//! (`@carbon/styles/scss/utilities/_focus-outline.scss` line 29), used 75
//! times across 62 component files, and no Carbon rule anywhere produces an
//! underline. The halo band inside the accent stroke is Carbon's too:
//! a primary button focuses with `box-shadow: inset 0 0 0
//! $button-outline-width $button-focus-color, inset 0 0 0
//! $button-border-width $background` (`components/button/_mixins.scss` line
//! 133), one unit of ground between the focus stroke and the button's fill.
//! Petra needs that band more sharply than Carbon does, because
//! [`RING_TOKEN`] is byte-identical to `accent.primary` and a primary
//! button's fill *is* `accent.primary`: without the halo the ring on that one
//! control is exactly invisible.
//!
//! `BarUnder` and `Sides` have no Carbon citation as focus figures. They are
//! Petra's, and `BarUnder` is the default one: the operator's rule is that
//! underlines are preferred to boxes, and a box is what a control wears only
//! when it is packed tightly enough that a bar hung below it would land on
//! its neighbour. `Sides` is for a well a person types into and for a
//! standalone control they press, which should not read the same as a row in
//! a list.
//!
//! # Whose rect
//!
//! What shape the indicator is lives here. *Whose rect it goes on* is a
//! separate question, answered by [`crate::tree::FocusShownOn`] on the node
//! itself: a field's input leaf shows focus on the well around it, and a
//! tree item shows it on its head row rather than on its whole expanded
//! subtree.
//!
//! # Why here and not in the adapter
//!
//! How thick a band is and where it sits relative to the node is a design
//! decision a second renderer must reproduce, and D-069 keeps this crate free
//! of any toolkit. What `gorgon-petra-egui` owns is the drawing.

use crate::geom::Rect;

/// Token naming the indicator's accent. Bound to the theme's accent, so every
/// figure is the same blue as the primary button, not a second hue.
pub const RING_TOKEN: &str = "focus.ring";

/// Token naming the halo band [`FocusFigure::Border`] draws immediately
/// inside its accent stroke. The page ground: white in the light theme,
/// near-black in the dark one.
///
/// [`FocusFigure::Border`]: crate::tree::FocusFigure::Border
pub const HALO_TOKEN: &str = "focus.ring-halo";

/// Token naming the shadow that seats a `BarUnder` bar on the card it hangs
/// over. One of [`crate::token::SHADOW_GEOMETRY`]'s two names, never a new
/// one.
///
/// `shadow.raised`, not `shadow.overlay`. Overlay drops the shadow four
/// units under a bar three units tall, so the shadow's own pixels never
/// touch the bar's and it stands as a second stripe below it — R6, "there
/// are just 2 lines instead of a shadow", light mode, 2026-09-05. Raised
/// drops two, less than the bar's three, so the two overlap and the shadow
/// reads as a shadow. `the_bar_is_taller_than_its_shadow_is_displaced`
/// holds that inequality.
///
/// `Border` and `Sides` cast nothing. A ring on the node's own edge needs no
/// seating, and a 40-unit bar standing beside a filled well needs none
/// either — the overlay smudge past the well's bottom rule read as the bar
/// overhanging the well (rows 22 and 28, 2026-09-05).
pub const BAR_SHADOW_TOKEN: &str = "shadow.raised";

/// The focus indicator's measurements, in logical units. One struct, three
/// figures; each field says which of them reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusRing {
    /// Height of a `BarUnder` bar, and width of a `Sides` bar.
    pub thickness: f32,
    /// Gap between the node's bottom edge and a `BarUnder` bar's top edge.
    pub gap: f32,
    /// Gap between a well's left or right edge and the `Sides` bar beside it.
    ///
    /// Wider than [`Self::gap`]. A bar under sits in the empty run below a
    /// pill; a side bar stands beside a filled well whose edge is a hard
    /// colour step, and at two units the bar read as part of that edge —
    /// the operator's "cramped" and "needs a bit of padding" on the Search
    /// and Number input rows, 2026-09-05.
    pub hug_gap: f32,
    /// Width of the `Border` accent stroke, drawn inside the node's own edge.
    pub stroke: f32,
    /// Width of the ground-coloured band `Border` draws immediately inside
    /// its accent stroke, so the ring never disappears into a fill of its
    /// own colour.
    pub halo: f32,
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::STANDARD
    }
}

impl FocusRing {
    /// The shipped measurements.
    ///
    /// `thickness` is three, not two: a 2-unit strip was a hairline under a
    /// 40-unit button. `stroke` is two, not three: Carbon's
    /// `focus-outline('outline')` is `2px` (`utilities/_focus-outline.scss:29`),
    /// and a closed ring on four edges does not have the hairline problem a
    /// bar hung under one edge has.
    pub const STANDARD: Self = Self {
        thickness: 3.0,
        gap: 2.0,
        hug_gap: 4.0,
        stroke: 2.0,
        halo: 1.0,
    };

    /// `BarUnder` for a focused node marking `rect`: the full width of
    /// `rect`, [`Self::gap`] below its bottom edge, [`Self::thickness`]
    /// tall. `the_bar_sits_below_the_marked_run` asserts it.
    ///
    /// `rect` is the **marked run**, not the control: horizontally the
    /// control's leading label, vertically the control itself. The engine
    /// composes it in [`crate::focus::marked_rect`], which is also where the
    /// reasoning for that lives. Both bars take the whole of it, so the two
    /// figures differ in exactly one measurement — where they seat.
    #[must_use]
    pub fn bar(self, rect: Rect) -> Rect {
        Rect::new(
            rect.x,
            rect.bottom() + self.gap,
            rect.w.max(0.0),
            self.thickness,
        )
    }

    /// `BarInside` for a focused node marking `rect`.
    ///
    /// The same run [`Self::bar`] takes, [`Self::thickness`] tall, seated on
    /// `rect`'s own bottom edge. No [`Self::gap`]: a gap is the clear run
    /// between a node and a bar outside it, and there is nothing outside
    /// here.
    ///
    /// **Contained by construction**, the property [`Self::bands`] has and
    /// [`Self::bar`] does not. That is the whole reason it exists, and the
    /// only way it differs from [`Self::bar`]: a row that stacks flush
    /// against the next one can wear an underline without painting into it.
    /// `the_inside_bar_never_leaves_its_node` asserts the containment on all
    /// four edges and `the_two_bars_differ_only_in_where_they_sit` asserts
    /// that nothing else about the two shapes disagrees.
    #[must_use]
    pub fn bar_inside(self, rect: Rect) -> Rect {
        // A node shorter than the bar keeps the bar inside it rather than
        // growing one that overhangs the top: a 2-unit row shows a 2-unit
        // stripe, not a 3-unit one starting above its own edge.
        let h = self.thickness.min(rect.h.max(0.0));
        Rect::new(rect.x, rect.bottom() - h, rect.w.max(0.0), h)
    }

    /// `Sides` for a focused node occupying `rect`: left and right bars,
    /// [`Self::thickness`] wide, [`Self::hug_gap`] outside each edge,
    /// exactly the node's height.
    ///
    /// Outside the node, so a well is bracketed rather than underlined, and
    /// never taller than it: a bar past the well's bottom rule reads as an
    /// overhang. `the_sides_sit_outside_the_left_and_right` asserts it.
    #[must_use]
    pub fn sides(self, rect: Rect) -> [Rect; 2] {
        let left = Rect::new(
            rect.x - self.hug_gap - self.thickness,
            rect.y,
            self.thickness,
            rect.h.max(0.0),
        );
        let right = Rect::new(
            rect.right() + self.hug_gap,
            rect.y,
            self.thickness,
            rect.h.max(0.0),
        );
        [left, right]
    }

    /// `Border` for a focused node occupying `rect`: the two concentric
    /// bands, outermost first, each as the rect its stroke is drawn *inside*
    /// of and that stroke's width.
    ///
    /// Contained by construction — neither band ever leaves `rect`, so a
    /// list row can show focus without painting into its neighbours, and a
    /// composer clip equal to the node's own rect never cuts the indicator.
    #[must_use]
    pub fn bands(self, rect: Rect) -> [(Rect, f32); 2] {
        [(rect, self.stroke), (rect.inset(self.stroke), self.halo)]
    }

    /// The four edges of the `Border` accent stroke, clockwise from the
    /// top: `[top, right, bottom, left]`.
    ///
    /// The same ring [`Self::bands`]' outer entry describes, cut into the
    /// four rects a four-band caret flies as. Corners belong to two edges
    /// at once and are covered twice; that overlap is deliberate, because a
    /// band that stopped short of the corner would leave a notch mid-flight.
    ///
    /// This is the geometry a test asks "which edges does the ring mark?"
    /// with. A selection indicator pinned to one edge of a node can coincide
    /// with one of these four; it can never coincide with the other three,
    /// which is what keeps focus legible on a selected control.
    #[must_use]
    pub fn border_edges(self, rect: Rect) -> [Rect; 4] {
        let s = self.stroke;
        [
            Rect::new(rect.x, rect.y, rect.w, s),
            Rect::new(rect.right() - s, rect.y, s, rect.h),
            Rect::new(rect.x, rect.bottom() - s, rect.w, s),
            Rect::new(rect.x, rect.y, s, rect.h),
        ]
    }

    /// How far outside the node's own rect an indicator (not its shadow) can
    /// reach: the wider of the two gaps plus the thickness.
    ///
    /// `Border` and `BarInside` reach nowhere, so this is `BarUnder`'s and
    /// `Sides`' number and the clip widening that reads it is theirs alone.
    #[must_use]
    pub fn overhang(self) -> f32 {
        self.gap.max(self.hug_gap) + self.thickness
    }
}

#[cfg(test)]
mod tests {
    use super::FocusRing;
    use crate::geom::Rect;

    /// `bar` takes the whole run it is handed and hangs it `gap` below.
    ///
    /// The run is a control's label at the control's vertical extent, so the
    /// rect here stands for a 90-wide word on a 40-tall row.
    #[test]
    fn the_bar_sits_below_the_marked_run() {
        let ring = FocusRing::STANDARD;
        let run = Rect::new(10.0, 20.0, 90.0, 40.0);
        let bar = ring.bar(run);
        assert_eq!(bar, Rect::new(10.0, 62.0, 90.0, 3.0));
        assert_eq!(
            (bar.x, bar.w),
            (run.x, run.w),
            "the bar is the run: narrowing it here would be a second width \
             rule, which is the defect `crate::focus::marked_rect` removed"
        );
        assert_eq!(bar.y, run.bottom() + ring.gap);
        assert_eq!(ring.overhang(), 7.0, "the hug gap is the wider reach");
    }

    /// The containment `BarInside` exists for: the bar never crosses any of
    /// the four edges of the node it marks.
    ///
    /// A flush-stacked row is the case. `BarUnder` on the same node reaches
    /// `gap + thickness` past the bottom edge and lands on the row below,
    /// which is what eight components wrote a comment about and worked
    /// around with a box.
    #[test]
    fn the_inside_bar_never_leaves_its_node() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let bar = ring.bar_inside(node);

        assert_eq!(bar, Rect::new(10.0, 57.0, 90.0, 3.0));
        assert!(bar.x >= node.x && bar.right() <= node.right(), "{bar:?}");
        assert!(bar.y >= node.y && bar.bottom() <= node.bottom(), "{bar:?}");
        assert_eq!(
            bar.bottom(),
            node.bottom(),
            "the bar sits on the node's own bottom edge, not floating above it"
        );
    }

    /// One underline, drawn at two heights. The operator read the pair as
    /// two cursors on 2026-09-06 because `bar_inside` had also been given a
    /// second width, so this pins the shapes together on every measurement
    /// except the one the pair exists for.
    ///
    /// Falsify by giving either bar a width rule of its own.
    #[test]
    fn the_two_bars_differ_only_in_where_they_sit() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let inside = ring.bar_inside(node);
        let under = ring.bar(node);

        assert_eq!(inside.w, under.w, "one width rule, not two");
        assert_eq!(inside.x, under.x, "and one horizontal rule");
        assert_eq!(inside.h, under.h, "same thickness");
        assert_eq!(
            (inside.x, inside.w),
            (node.x, node.w),
            "both take the whole run they are handed: {inside:?}"
        );

        assert!(
            under.bottom() > node.bottom(),
            "the one difference: `bar` leaves the node"
        );
        assert_eq!(inside.bottom(), node.bottom(), "and `bar_inside` does not");
    }

    /// A node shorter than the bar keeps the bar inside it. Without the
    /// clamp the bar would start above the node's top edge, which is the
    /// containment failing in the one case it is most likely to be noticed:
    /// a hairline row.
    #[test]
    fn a_node_shorter_than_the_bar_still_contains_it() {
        let ring = FocusRing::STANDARD;
        let thin = Rect::new(0.0, 0.0, 60.0, 2.0);
        let bar = ring.bar_inside(thin);
        assert_eq!(bar.h, 2.0, "clamped to the node's height, not 3");
        assert_eq!(bar.w, thin.w, "width is untouched by the height clamp");
        assert!(bar.y >= thin.y, "{bar:?} starts above {thin:?}");
        assert_eq!(bar.bottom(), thin.bottom());
    }

    #[test]
    fn the_sides_sit_outside_the_left_and_right() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let [left, right] = ring.sides(node);
        assert_eq!(left, Rect::new(3.0, 20.0, 3.0, 40.0));
        assert_eq!(right, Rect::new(104.0, 20.0, 3.0, 40.0));
        assert_eq!(left.right(), node.x - ring.hug_gap);
        assert_eq!(right.x, node.right() + ring.hug_gap);
        assert!(
            ring.hug_gap > ring.gap,
            "a side bar stands off a filled well further than a bar under \
             stands off a pill"
        );
        assert_eq!(left.y, node.y, "a side bar starts at the well's top");
        assert_eq!(
            left.bottom(),
            node.bottom(),
            "a side bar never overhangs the well's bottom rule"
        );
        assert_eq!(right.h, node.h);
    }

    #[test]
    fn the_border_lies_inside_the_node_it_rings() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let [(accent, stroke), (halo, halo_w)] = ring.bands(node);
        assert_eq!(accent, node, "the accent stroke's outer edge is the node's");
        assert_eq!(stroke, ring.stroke);
        assert_eq!(
            halo,
            Rect::new(12.0, 22.0, 86.0, 36.0),
            "the halo band sits immediately inside the accent stroke"
        );
        assert_eq!(halo_w, ring.halo);
        assert!(
            accent.x >= node.x
                && accent.y >= node.y
                && accent.right() <= node.right()
                && accent.bottom() <= node.bottom(),
            "the border reaches outside the node it rings: {accent:?} in {node:?}"
        );
    }

    #[test]
    fn a_node_narrower_than_the_figures_does_not_invert_them() {
        let tiny = Rect::new(0.0, 0.0, 1.0, 1.0);
        let bar = FocusRing::STANDARD.bar(tiny);
        assert!(bar.w >= 0.0 && bar.h >= 0.0, "{bar:?}");
        assert!(
            (bar.w - tiny.w).abs() < 1e-6,
            "the bar is the run it is given"
        );
        let [(accent, _), (halo, _)] = FocusRing::STANDARD.bands(tiny);
        assert!(accent.w >= 0.0 && accent.h >= 0.0, "{accent:?}");
        assert_eq!(
            halo.w, 0.0,
            "the halo collapses rather than turning inside out"
        );
        assert_eq!(halo.h, 0.0);
    }

    #[test]
    fn every_figure_changes_the_silhouette_by_more_than_a_hairline() {
        let ring = FocusRing::default();
        assert!(
            ring.thickness >= 2.0,
            "a bar thinner than this reads as a border, not as an indicator"
        );
        assert!(ring.gap >= 0.0);
        assert!(
            ring.stroke >= 2.0,
            "a stroke thinner than this reads as a border, not as an indicator"
        );
        assert!(
            ring.halo > 0.0,
            "without the halo the border vanishes on a node filled with the \
             accent, which is what a primary button is"
        );
    }

    /// The shadow that seats a `BarUnder` bar on the card must be displaced
    /// by less than the bar is tall, or it clears the bar and stands as its
    /// own stripe under it. That is R6: "there are just 2 lines instead of a
    /// shadow", light mode, 2026-09-05.
    ///
    /// The rule is here, beside the thickness it constrains, so that raising
    /// the shadow or thinning the bar fails a test in this file rather than
    /// in a screenshot three waves later.
    /// The four `Border` edges are the ring, and nothing else.
    ///
    /// Each is `stroke` deep, each lies inside the node, and together they
    /// leave exactly the inside of the ring uncovered. `paint::caret_bands`
    /// flies these four and `component::tabs` measures against them, so
    /// this is the one place their shape is pinned.
    #[test]
    fn the_border_edges_line_the_inside_of_the_node() {
        let ring = FocusRing::STANDARD;
        let node = Rect::new(10.0, 20.0, 90.0, 40.0);
        let [top, right, bottom, left] = ring.border_edges(node);

        assert_eq!(top.h, ring.stroke, "the top band is a stroke deep");
        assert_eq!(bottom.h, ring.stroke, "the bottom band is a stroke deep");
        assert_eq!(left.w, ring.stroke, "the left band is a stroke wide");
        assert_eq!(right.w, ring.stroke, "the right band is a stroke wide");

        for (name, band) in [
            ("top", top),
            ("right", right),
            ("bottom", bottom),
            ("left", left),
        ] {
            assert_eq!(
                band.intersect(node),
                band,
                "the {name} band {band:?} leaves the node {node:?}, so a \
                 clip equal to the node's own rect would cut the ring"
            );
        }

        // Nothing reaches the middle: the ring is a line, not a fill.
        let middle = node.inset(ring.stroke);
        for (name, band) in [
            ("top", top),
            ("right", right),
            ("bottom", bottom),
            ("left", left),
        ] {
            assert!(
                !band.overlaps(middle),
                "the {name} band {band:?} reaches into {middle:?}, which is \
                 the node's content and not the ring"
            );
        }
    }

    #[test]
    fn the_bar_is_taller_than_its_shadow_is_displaced() {
        let geometry = crate::token::SHADOW_GEOMETRY
            .iter()
            .find(|(name, _)| *name == crate::token::focus::BAR_SHADOW_TOKEN)
            .expect("the bar's shadow token has geometry")
            .1;
        let drop = f32::from(geometry.offset[1]);
        assert!(
            drop < FocusRing::STANDARD.thickness,
            "{} drops {drop} under a {}-unit bar, so the bar's own pixels do \
             not overlap it and it reads as a second line",
            crate::token::focus::BAR_SHADOW_TOKEN,
            FocusRing::STANDARD.thickness
        );
        assert_eq!(geometry.offset[0], 0, "the drop is straight down");
    }
}
