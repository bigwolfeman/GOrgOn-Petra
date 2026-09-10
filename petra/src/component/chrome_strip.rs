//! `chrome_strip` — a horizontal strip of chrome: a status bar, a toolbar.
//!
//! # Why this exists
//!
//! An author composing a strip of chrome out of bare primitives (a
//! [`super::stack`] holding whatever controls belong there) gets three
//! defects for free, none of them announced:
//!
//! 1. **No padding.** `Props::padding` is `Option<InsetRefs>` and absence
//!    resolves to `Insets::NONE` by design (`tree/props.rs`'s own
//!    `padding_round_trips_through_serde` test: "Absent padding serializes
//!    away entirely, and resolves to NONE"). A bare stack packs its
//!    children flush against the strip's own edge.
//! 2. **A start-aligned row.** `Props::align` defaults to `Align::Start`,
//!    so children shorter than the strip's own block extent sit at its top
//!    rather than centred on it.
//! 3. **A rule with nothing bounding it.** A
//!    [`crate::tree::NodeKind::Separator`] measures its length off the size
//!    proposal it is offered, and under an *open* cross proposal it answers
//!    the whole extent available — inflating the row it sits in to whatever
//!    the viewport allows. A strip of chrome with a divider in it and no
//!    declared height is not a strip; it is a full-height column.
//!
//! `chrome_strip` is the shipped default that carries the author past all
//! three: it fixes the strip's own height so a `Separator` inside it has a
//! bounded extent to run along, centres the cross axis so a control and the
//! label beside it share one midline, and pads it so nothing sits flush
//! against the edge. Every value it sets is a token name or a cited
//! constant, so a theme still owns the numbers — this component owns only
//! the shape.
//!
//! # Why centred rather than stretched
//!
//! It stretched, once, for a reason that turned out not to be true. This
//! doc used to say a vertical [`super::rule`] "paints nothing at all"
//! without `Align::Stretch`, so the strip stretched every child to keep its
//! divider. `component::tests`'s
//! `a_rule_runs_full_height_in_a_row_whatever_that_row_aligns_its_children_to`
//! measures all three alignments off real frames and places the same rule
//! each time. The pinned height was doing that work the whole while; the
//! alignment never was.
//!
//! What stretching *did* do was break the strip's plainest child. A
//! [`super::text`] handed a 40-unit tall box paints its glyphs at the top of
//! it, so a status bar's own label sat ten units above the checkbox and the
//! button beside it, in the shipped demo, under a green suite. An operator's
//! screenshot is what found it.
//!
//! # Why no `background`
//!
//! A strip is a layout, not a surface. It bound `surface.base` and painted
//! it over whatever it was mounted in, which made a Luau plugin's status bar
//! — a surface that had asked for `surface.layer-one` precisely so it would
//! read as chrome against the page — invisible: same fill, no boundary,
//! nothing to tell an operator where the page stopped and the bar began.
//! The container owns the fill. An author who wants one on the strip itself
//! still binds `background` on the node this returns.
//!
//! Not a Carbon inventory entry (`component::registry` does not list it):
//! it names no anatomy from the Carbon component set, just an assembly
//! this library's other components (a status bar, a toolbar) would
//! otherwise each have had to rebuild.

use super::tokens::{SIZE_LG, SPACING_02, SPACING_03, SPACING_05};
use super::{pad, pin_block, stack};
use crate::geom::{Align, Axis};
use crate::tree::{Key, ViewNode};

/// A horizontal strip of chrome, `SIZE_LG` (48) tall, holding `children`
/// gapped by [`SPACING_03`] (8).
///
/// # The padding is derived, not picked
///
/// Vertical padding is [`SPACING_02`] (4) on each edge: `SIZE_LG - 2 *
/// SPACING_02 == SIZE_MD` (48 - 8 = 40, `crate::token::shipped`'s
/// `("size-lg", 48.0)` and `("spacing-02", 4.0)`), so a default `size-md`
/// control ([`super::button`], [`super::field`], …) dropped into this strip
/// clears its inner box exactly, with nothing left over and nothing
/// clipped. [`tests::the_strip_pins_forty_eight_and_a_size_md_child_clears_at_forty`]
/// proves the arithmetic off a laid-out frame rather than restating it in
/// prose. Horizontal padding is [`SPACING_05`] (16), the same inline
/// clearance [`super::section`] and [`super::list_row`] already spend.
#[must_use]
pub fn chrome_strip(key: impl Into<Key>, children: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), children);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_02));
    node.with_constraints(pin_block(SIZE_LG))
}

#[cfg(test)]
mod tests {
    use super::chrome_strip;
    use crate::component::tokens::SIZE_MD;
    use crate::frame::{TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ThemeMode, standard_vocabulary};
    use crate::tree::{Constraints, NodeKind, Props, Registry, ViewNode};

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(child: ViewNode) -> crate::frame::PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(child);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn placed_height(frame: &crate::frame::PetrifiedFrame, key: &str) -> f32 {
        let suffix = format!("/root/{key}");
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(&suffix))
            .unwrap_or_else(|| panic!("missing placement ending {suffix}"))
            .rect
            .h
    }

    fn a_child(key: &'static str) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key).with_constraints(Constraints {
            vertical: crate::tree::AxisConstraint {
                min: Some(SIZE_MD),
                max: Some(SIZE_MD),
                priority: 0,
            },
            ..Constraints::default()
        })
    }

    /// S4.2: every declared field, off the node the constructor returns.
    #[test]
    fn the_strip_declares_padding_centring_and_a_pinned_height() {
        let node = chrome_strip("bar", vec![a_child("child")]);
        assert!(node.props.padding.is_some(), "no padding declared");
        assert_eq!(node.props.align, Some(Align::Center));
        assert_eq!(node.constraints.vertical.min, Some(48.0));
        assert_eq!(node.constraints.vertical.max, Some(48.0));
        assert_eq!(node.constraints.vertical.min, node.constraints.vertical.max);
    }

    /// The defect the operator's screenshot found: a status bar's own label
    /// riding ten units above the checkbox and the button beside it.
    ///
    /// The discriminator is the label's **height**, not its centre. Under
    /// `Align::Stretch` a text node's placement rect is the strip's whole
    /// 40-unit inner box, and the two rects still share a midline — so an
    /// assertion on centres passes on the broken layout. What a stretched
    /// text node does is paint its glyphs at the *top* of that box, which
    /// only shows up as the rect being taller than the type it holds. So this
    /// asserts both: the label's box is its own 20-unit type, and that box
    /// sits on the same midline as the control.
    ///
    /// Falsified by putting `Align::Stretch` back and leaving everything else
    /// alone: this failed with the actual, observed text
    ///
    /// ```text
    /// the label's box is 40 tall, the same as the 40-tall control beside it:
    /// it was stretched, and a stretched text node paints its glyphs at the
    /// top of its box rather than on the strip's midline
    /// ```
    #[test]
    fn a_labels_box_is_its_own_type_and_sits_on_the_controls_midline() {
        let node = chrome_strip(
            "bar",
            vec![crate::component::text("label", "watcher"), a_child("child")],
        );
        let frame = petrify_lone(node);
        let rect = |key: &str| {
            let suffix = format!("/root/{key}");
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(&suffix))
                .unwrap_or_else(|| panic!("missing placement ending {suffix}"))
                .rect
        };
        let label = rect("bar/label");
        let control = rect("bar/child");
        assert!(
            label.h < control.h,
            "the label's box is {} tall, the same as the {}-tall control beside it: it was \
             stretched, and a stretched text node paints its glyphs at the top of its box \
             rather than on the strip's midline",
            label.h,
            control.h
        );
        assert_eq!(
            label.y + label.h / 2.0,
            control.y + control.h / 2.0,
            "label midline {} against control midline {}",
            label.y + label.h / 2.0,
            control.y + control.h / 2.0
        );
    }

    /// A vertical rule still runs the strip's full inner height now that the
    /// strip centres rather than stretches. The strip's pinned height is what
    /// bounds it, and that has not changed — see this module's doc on why
    /// stretching was never the thing keeping the divider alive.
    #[test]
    fn a_rule_in_a_centred_strip_still_runs_the_full_inner_height() {
        let node = chrome_strip(
            "bar",
            vec![
                crate::component::text("label", "watcher"),
                crate::component::rule(
                    "rule",
                    Axis::Vertical,
                    crate::component::tokens::BORDER_SUBTLE,
                ),
            ],
        );
        let frame = petrify_lone(node);
        let run = placed_height(&frame, "bar/rule");
        assert_eq!(run, 40.0, "the rule ran {run} of the strip's 40");
    }

    /// A strip paints no fill of its own, so the surface it is mounted in
    /// keeps the one its author chose. It used to bind `surface.base` and
    /// cover a plugin's `surface.layer-one` status bar with the same colour
    /// as the page behind it.
    #[test]
    fn the_strip_binds_no_background_of_its_own() {
        let node = chrome_strip("bar", vec![a_child("child")]);
        assert!(
            !node.props.tokens.contains_key("background"),
            "the strip bound a background and will paint over its container: {:?}",
            node.props.tokens
        );
    }

    /// S4.3: the derivation, proved off a laid-out frame rather than
    /// restated in prose. `SIZE_LG - 2 * SPACING_02 == SIZE_MD`: the strip
    /// itself measures 48, and a `size-md` (40) child clears it with the
    /// 4-unit top/bottom pad on each side and nothing left over.
    ///
    /// Falsified (S4.4) by reverting the constructor body to a bare
    /// `stack(key, Axis::Horizontal, None, children)` with no align, no
    /// padding and no constraints: this test's own assertion failed with
    /// the actual, observed text
    ///
    /// ```text
    /// assertion `left == right` failed
    ///   left: 40.0
    ///  right: 48.0
    /// ```
    ///
    /// (an unconstrained stack measures its own extent off its content —
    /// one already-pinned 40-tall child — not the 48 [`SIZE_LG`] pins), and
    /// the sibling declaration test failed first, at `no padding declared`,
    /// on the same reverted body.
    #[test]
    fn the_strip_pins_forty_eight_and_a_size_md_child_clears_at_forty() {
        let node = chrome_strip("bar", vec![a_child("child")]);
        let frame = petrify_lone(node);
        assert_eq!(placed_height(&frame, "bar"), 48.0);
        assert_eq!(placed_height(&frame, "bar/child"), SIZE_MD);
    }
}
