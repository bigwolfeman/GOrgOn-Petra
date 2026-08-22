//! Leaf kinds that are not text: image, input, spacer, separator, custom.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Axis, Size};
use crate::layout::{LayoutCtx, Proposal, SizeProposal, Slot, semantics_of};
use crate::tree::{KeyPath, NodeKind, ViewNode};

/// The extent a spacer answers an `Unbounded` probe with when it declares no
/// maximum.
///
/// A response must be a concrete finite size (`contracts/view-tree.md`), so
/// "infinitely flexible" needs a number. This one is far past any real
/// viewport, which makes a spacer the most flexible child in every stack it
/// joins — the behaviour authors expect — without putting an infinity into a
/// digest input.
pub const SPACER_MAX_EXTENT: f32 = 65_535.0;

/// Thickness of a separator across its run axis, in logical units.
pub const SEPARATOR_THICKNESS: f32 = 1.0;

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
        NodeKind::Separator => measure_separator(node, proposal),
        NodeKind::Custom => {
            let name = node.props.custom_kind.as_deref().unwrap_or("");
            ctx.content.custom(name, proposal)
        }
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
            token_revision: ctx.theme_rev,
            // Filled by `PlacementSink::attach` once the dispatcher
            // has the payload; no container owns this.
            paint_hash: 0,
        },
        semantics,
        parent: None,
    });
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

fn measure_separator(node: &ViewNode, proposal: SizeProposal) -> Size {
    let axis = node.props.axis.unwrap_or(Axis::Horizontal);
    let along = match proposal.axis(axis) {
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero | Proposal::Unspecified => 0.0,
        Proposal::Unbounded => SPACER_MAX_EXTENT,
    };
    Size::from_axes(axis, along, SEPARATOR_THICKNESS)
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
    let req = crate::layout::TextRequest {
        text: content,
        style: props.style,
        wrap: crate::tree::TextWrap::Clip,
        max_lines: Some(1),
        available_width: proposal.horizontal.available(),
    };
    ctx.content.text(&req).size
}

#[cfg(test)]
mod tests {
    use super::{SEPARATOR_THICKNESS, SPACER_MAX_EXTENT, spacer_extent};
    use crate::layout::Proposal;

    #[test]
    fn a_spacer_is_the_most_flexible_child_in_the_room() {
        assert_eq!(spacer_extent(Proposal::Zero), 0.0);
        assert_eq!(spacer_extent(Proposal::Unspecified), 0.0);
        assert_eq!(spacer_extent(Proposal::Unbounded), SPACER_MAX_EXTENT);
        assert_eq!(spacer_extent(Proposal::Exact(40.0)), 40.0);
        assert!(SPACER_MAX_EXTENT.is_finite());
    }

    #[test]
    fn a_separator_is_one_unit_thick() {
        assert_eq!(SEPARATOR_THICKNESS, 1.0);
    }
}
