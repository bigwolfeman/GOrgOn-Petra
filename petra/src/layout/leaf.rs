//! Leaf kinds that are not text: image, input, spacer, separator, custom,
//! canvas.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Axis, Size};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::{KeyPath, NodeKind, TextWrap, ViewNode};

/// The extent a spacer answers an `Unbounded` probe with when it declares no
/// maximum.
///
/// A response must be a concrete finite size (`contracts/view-tree.md`), so
/// "infinitely flexible" needs a number. This one is far past any real
/// viewport, which makes a spacer the most flexible child in every stack it
/// joins — the behaviour authors expect — without putting an infinity into a
/// digest input.
pub const SPACER_MAX_EXTENT: f32 = 65_535.0;

/// Thickness of a **flat** separator across its run axis, in logical units.
///
/// A separator carrying the rule material's trigger token measures
/// [`crate::token::rule::thickness`] instead, which is the groove's four
/// absolute device pixels converted at the display scale. The two are
/// deliberately different numbers: a flat line is one logical unit at any
/// density because it is a line, and a groove is a machined edge whose
/// strokes are pinned to the physical pixel grid.
pub const SEPARATOR_THICKNESS: f32 = 1.0;

/// Whether this node's `background` is the rule material's trigger token.
///
/// Read at measure time as well as at paint time, because a separator's own
/// thickness depends on it: the material needs four device pixels and a flat
/// line needs one. Only the plain slot is consulted — a measurement has no
/// interaction state, so `background@hover` cannot change how much room a
/// rule reserves.
pub(crate) fn is_material(node: &ViewNode) -> bool {
    node.props
        .tokens
        .get("background")
        .is_some_and(|token| token.as_str() == crate::token::rule::MATERIAL_TOKEN)
}

/// Measure a non-text leaf.
pub fn measure(node: &ViewNode, ctx: &mut LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    match node.kind {
        NodeKind::Image => {
            let source = node.props.image.as_deref().unwrap_or("");
            ctx.content.image(source, proposal)
        }
        NodeKind::Input => measure_input(node, ctx, proposal),
        NodeKind::Spacer => Size::new(
            spacer_extent(proposal.horizontal),
            spacer_extent(proposal.vertical),
        ),
        NodeKind::Separator => measure_separator(node, ctx, proposal),
        NodeKind::Custom => {
            let name = node.props.custom_kind.as_deref().unwrap_or("");
            ctx.content.custom(name, proposal)
        }
        // A draw list has no intrinsic size, which is the whole reason
        // `canvas` is a second kind beside `custom` rather than a mode of it
        // (`contracts/draw-list.md` §7, `research.md` D-06). The line above
        // asks the measurement registry because a host-registered measurer
        // supplies an answer; there is no measurer for a list of coordinates,
        // and reading a bounding box off the commands would be a *different*
        // answer — one that changes every time the author moves a point,
        // relaying out the whole frame around a picture that was only ever
        // supposed to fill the box it was given.
        NodeKind::Canvas => Size::new(
            canvas_extent(proposal.horizontal),
            canvas_extent(proposal.vertical),
        ),
        // Reached only if `measure_kind` gains a kind and forgets to route it.
        other => unreachable!("{} is not a plain leaf kind", other.as_str()),
    }
}

/// Place a non-text leaf: one placement, no children.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let content_hash = match node.kind {
        NodeKind::Input => {
            crate::frame::digest::hash_text(node.props.text.as_deref().unwrap_or(""))
        }
        NodeKind::Image => {
            crate::frame::digest::hash_text(node.props.image.as_deref().unwrap_or(""))
        }
        _ => 0,
    };
    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash,
            truncated: false,
            overflowed: false,
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
}

/// A canvas's extent on one axis: exactly what it was offered.
///
/// The negotiated proposal is the whole of the answer. `Unbounded` is the one
/// case with a decision in it, and it answers `0`, not [`SPACER_MAX_EXTENT`]:
/// an unbounded probe asks "how big would you like to be", and a picture with
/// no intrinsic size would like to be nothing. Answering the spacer's number
/// instead would make a canvas the greediest child in every stack it joined,
/// which is a layout opinion a draw list has no standing to hold. An author
/// who wants a canvas to take room says so in `constraints`, which the
/// dispatcher clamps this response with — the one place a canvas's size is
/// ever decided.
fn canvas_extent(proposal: Proposal) -> f32 {
    match proposal {
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero | Proposal::Unspecified | Proposal::Unbounded => 0.0,
    }
}

fn spacer_extent(proposal: Proposal) -> f32 {
    match proposal {
        Proposal::Exact(v) => v.max(0.0),
        // A spacer has no natural size: its ideal is nothing, and it grows
        // only when a parent offers it room.
        Proposal::Zero | Proposal::Unspecified => 0.0,
        Proposal::Unbounded => SPACER_MAX_EXTENT,
    }
}

/// A separator's size: the run it was offered along `props.axis`, and a
/// thickness it chooses for itself across that axis.
///
/// **The thickness is not the caller's to pick.** That is the whole point of
/// the kind. Before 2026-09-09 a divider was written as a childless stack
/// with a `border.subtle` fill and a pinned one-unit height, seven times
/// across the component library, and every one of them reserved one logical
/// unit for a material that paints four device pixels. They all painted flat
/// because a fill is not an edge slot, so the defect was invisible until the
/// same token grooved in a table and did not groove in an accordion. A kind
/// that measures its own material cannot drift that way.
fn measure_separator(node: &ViewNode, ctx: &LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    let axis = node.props.axis.unwrap_or(Axis::Horizontal);
    let along = match proposal.axis(axis) {
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero | Proposal::Unspecified => 0.0,
        Proposal::Unbounded => SPACER_MAX_EXTENT,
    };
    let across = if is_material(node) {
        crate::token::rule::thickness(ctx.scale)
    } else {
        SEPARATOR_THICKNESS
    };
    Size::from_axes(axis, along, across)
}

fn measure_input(node: &ViewNode, ctx: &mut LayoutCtx<'_>, proposal: SizeProposal) -> Size {
    // An empty field still occupies a line, and its placeholder is what sets
    // the field's natural width. Measuring the empty string instead would make
    // a fresh field collapse and then jump on the first keystroke.
    let props = node.props.text();
    let content = match node.props.text.as_deref() {
        Some(text) if !text.is_empty() => text,
        _ => node.props.placeholder.as_deref().unwrap_or(" "),
    };
    // Same inset the painter uses for `Input` (`spacing-04` each side). A
    // shrink-wrapped field has to ask for that room at measure time or the
    // first glyph sits on the border. When the offer is finite, shape the
    // run against the inner width so the insets do not overflow the slot.
    let inset = ctx.spacing(&Some(
        crate::token::TokenName::new("spacing-04").expect("spacing-04 is in the vocabulary"),
    ));
    let inner_width = proposal
        .horizontal
        .available()
        .map(|w| (w - 2.0 * inset).max(0.0));
    // Input defaults to one line; a textarea sets wrap + max_lines.
    //
    // Props.wrap and Props.max_lines are both None on a plain field, and
    // TextWrap's Default is Wrap. Feeding those through would grow field()
    // the moment a parent offered a finite width, which is the opposite of
    // a field. Clip plus a one-line cap is the field's silence. Wrap with
    // the line cap left open is the textarea's declaration
    // (`component/textarea.rs` sets Wrap and leaves max_lines alone; the
    // well's vertical max is a constraint, not a line count).
    let input_wrap = node.props.wrap.unwrap_or(TextWrap::Clip);
    let input_max_lines = match node.props.max_lines {
        Some(n) => Some(n),
        None if input_wrap == TextWrap::Wrap => None,
        None => Some(1),
    };
    let req = crate::layout::TextRequest {
        text: content,
        style: props.style,
        wrap: input_wrap,
        max_lines: input_max_lines,
        available_width: inner_width,
    };
    let size = ctx.content.text(&req).size;
    // A field fills the width it is given and asks for all the width there
    // is: Carbon's input is `inline-size: 100%` of its form item, and a
    // field beside two fixed steppers in a row takes the rest of the row
    // (`_number-input.scss`). Only the ideal and minimum probes answer with
    // the content's own width, which is what keeps a fresh field from
    // collapsing under an open cross-axis offer and jumping on the first
    // keystroke. `Exact` answers the offer even when the offer is narrower
    // than the run: a parent places, it does not force, and a field clipped
    // to its slot is what a field does with a run longer than itself.
    let width = match proposal.horizontal {
        Proposal::Exact(w) => w.max(0.0),
        Proposal::Unbounded => SPACER_MAX_EXTENT,
        Proposal::Zero | Proposal::Unspecified => size.w + 2.0 * inset,
    };
    Size::new(width, size.h)
}

#[cfg(test)]
mod tests {
    use super::{SEPARATOR_THICKNESS, SPACER_MAX_EXTENT, canvas_extent, spacer_extent};
    use crate::geom::Size;
    use crate::layout::{Proposal, SizeProposal};
    use crate::testing::Harness;
    use crate::tree::{KeyPath, NodeKind, Props, TextWrap, ViewNode};

    fn measured(node: &ViewNode, horizontal: Proposal) -> Size {
        let mut h = Harness::new();
        let mut ctx = h.ctx();
        let mut path = KeyPath::root();
        let proposal = SizeProposal::both(Proposal::Unspecified)
            .with_axis(crate::geom::Axis::Horizontal, horizontal);
        crate::layout::measure(node, &mut ctx, &mut path, proposal)
    }

    /// A field fills the width it is offered and asks for all the width
    /// there is; only the ideal probe answers with its content.
    ///
    /// The number input is the case: a value cell beside two fixed steppers
    /// in a row. A field that hugged its text under an exact offer left the
    /// row's remainder trailing after the steppers, so the controls sat in
    /// the middle of the well and the empty right half of the field was
    /// dead to a click (`22-number-input.png`, 2026-09-04).
    #[test]
    fn a_field_fills_its_offer_and_hugs_only_its_ideal() {
        let field = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some("Count".into()),
            ..Props::default()
        });
        let ideal = measured(&field, Proposal::Unspecified);
        assert!(
            ideal.w > 0.0 && ideal.w < 300.0,
            "ideal hugs the placeholder: {ideal:?}"
        );
        let exact = measured(&field, Proposal::Exact(300.0));
        assert_eq!(exact.w, 300.0, "an exact offer is taken whole");
        assert_eq!(exact.h, ideal.h, "the height is the run's");
        let narrow = measured(&field, Proposal::Exact(10.0));
        assert_eq!(
            narrow.w, 10.0,
            "a narrow offer is taken too: a parent places"
        );
        let unbounded = measured(&field, Proposal::Unbounded);
        assert_eq!(
            unbounded.w, SPACER_MAX_EXTENT,
            "asked how wide it would like to be, a field wants the row"
        );
    }

    /// Input defaults to one line. A textarea sets wrap (and optionally a
    /// line cap); that is what lets the well grow with its content.
    ///
    /// MonoContent: 8.0 per character, 16.0 per line. `spacing-04` is 12
    /// each side, so Exact(56) offers 32 of inner width, 4 chars per line.
    /// Sixteen characters wrap to four lines when Wrap is on and uncapped.
    #[test]
    fn a_wrapping_field_grows_and_a_silent_field_stays_one_line() {
        let text = "abcdefghijklmnop";
        let offer = Proposal::Exact(56.0);

        let silent = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            text: Some(text.into()),
            ..Props::default()
        });
        let silent_size = measured(&silent, offer);
        assert_eq!(
            silent_size.h, 16.0,
            "a field that sets neither wrap nor max_lines stays one line: {silent_size:?}"
        );
        assert_eq!(silent_size.w, 56.0, "an exact offer is still taken whole");

        let empty = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some(text.into()),
            ..Props::default()
        });
        assert_eq!(
            measured(&empty, offer).h,
            16.0,
            "an empty field still measures its placeholder, and still as one line"
        );

        let wrapping = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            text: Some(text.into()),
            wrap: Some(TextWrap::Wrap),
            ..Props::default()
        });
        let wrapping_size = measured(&wrapping, offer);
        assert_eq!(
            wrapping_size.h, 64.0,
            "wrap without a line cap grows with the run: {wrapping_size:?}"
        );
        assert_eq!(wrapping_size.w, 56.0, "an exact offer is still taken whole");

        let wrapping_empty = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some(text.into()),
            wrap: Some(TextWrap::Wrap),
            ..Props::default()
        });
        assert_eq!(
            measured(&wrapping_empty, offer).h,
            64.0,
            "an empty wrapping field still measures its placeholder, and the placeholder wraps"
        );

        let capped = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            text: Some(text.into()),
            wrap: Some(TextWrap::Wrap),
            max_lines: Some(2),
            ..Props::default()
        });
        let capped_size = measured(&capped, offer);
        assert_eq!(
            capped_size.h, 32.0,
            "wrap plus a line cap of two is two lines, not four: {capped_size:?}"
        );
    }

    #[test]
    fn a_spacer_is_the_most_flexible_child_in_the_room() {
        assert_eq!(spacer_extent(Proposal::Zero), 0.0);
        assert_eq!(spacer_extent(Proposal::Unspecified), 0.0);
        assert_eq!(spacer_extent(Proposal::Unbounded), SPACER_MAX_EXTENT);
        assert_eq!(spacer_extent(Proposal::Exact(40.0)), 40.0);
        assert!(SPACER_MAX_EXTENT.is_finite());
    }

    /// A canvas fills its offer and asks for nothing: the negotiated
    /// proposal is the whole of its size.
    ///
    /// The `Unbounded` row is the one that matters. A canvas answering
    /// [`SPACER_MAX_EXTENT`] there would out-flex every real child in the
    /// stack beside it, and it would do so because of a number this file
    /// picked rather than anything the author declared.
    #[test]
    fn a_canvas_takes_its_offer_and_asks_for_nothing() {
        assert_eq!(canvas_extent(Proposal::Exact(120.0)), 120.0);
        assert_eq!(canvas_extent(Proposal::Zero), 0.0);
        assert_eq!(canvas_extent(Proposal::Unspecified), 0.0);
        assert_eq!(
            canvas_extent(Proposal::Unbounded),
            0.0,
            "a picture with no intrinsic size would like to be nothing"
        );
        assert_ne!(
            canvas_extent(Proposal::Unbounded),
            spacer_extent(Proposal::Unbounded),
            "a canvas is not a spacer; the two answer an unbounded probe              differently on purpose"
        );
        assert_eq!(canvas_extent(Proposal::Exact(-5.0)), 0.0);
    }

    #[test]
    fn a_separator_is_one_unit_thick() {
        assert_eq!(SEPARATOR_THICKNESS, 1.0);
    }
}
