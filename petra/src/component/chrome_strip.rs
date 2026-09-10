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
//! 3. **A collapsed rule.** A [`crate::tree::NodeKind::Separator`] measures
//!    its along-axis extent straight off the size proposal it is offered
//!    (`layout::leaf::measure_separator`: `Proposal::Exact(v) => v.max(0.0)`)
//!    and nothing else supplies one — a childless stack with no declared
//!    extent offers it `Proposal::Unspecified`, which resolves to zero. A
//!    vertical rule dropped into a strip built by hand paints nothing at
//!    all, which is the fourth, easiest-to-miss mechanism: not a missing
//!    token, a missing *proposal*.
//!
//! `chrome_strip` is the shipped default that carries the author past all
//! three: it fixes the strip's own height so a `Separator` inside it always
//! has something to measure against, stretches the cross axis so a child
//! fills it, and pads it so nothing sits flush against the edge. Every
//! value it sets is a token name or a cited constant, so a theme still owns
//! the numbers — this component owns only the shape.
//!
//! Not a Carbon inventory entry (`component::registry` does not list it):
//! it names no anatomy from the Carbon component set, just an assembly
//! this library's other components (a status bar, a toolbar) would
//! otherwise each have had to rebuild.

use super::tokens::{SIZE_LG, SPACING_02, SPACING_03, SPACING_05, SURFACE_BASE, t};
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
    node.props.align = Some(Align::Stretch);
    node.props.padding = Some(pad(SPACING_05, SPACING_02));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
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
    fn the_strip_declares_padding_stretch_and_a_pinned_height() {
        let node = chrome_strip("bar", vec![a_child("child")]);
        assert!(node.props.padding.is_some(), "no padding declared");
        assert_eq!(node.props.align, Some(Align::Stretch));
        assert_eq!(node.constraints.vertical.min, Some(48.0));
        assert_eq!(node.constraints.vertical.max, Some(48.0));
        assert_eq!(node.constraints.vertical.min, node.constraints.vertical.max);
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
