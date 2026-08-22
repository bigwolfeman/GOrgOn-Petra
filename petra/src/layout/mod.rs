//! Layout negotiation: proposals down, responses up, one placement per node.
//!
//! The engine measures with [`measure`] and places with [`place`]. Both
//! dispatch on [`crate::tree::NodeKind`] into the per-container modules in this
//! directory; both are pure functions of the tree, the state snapshot, the
//! viewport, the theme snapshot, and the scale (FR-006). Nothing in this module
//! knows about a toolkit: content measurement enters through
//! [`ContentMeasure`] and row materialization through [`RowSource`], and both
//! are implemented outside this crate.

pub mod constraints;
pub mod grid;
pub mod leaf;
pub mod overlay;
pub mod overlay_surface;
pub mod proposal;
pub mod scroll;
pub mod stack;
pub mod text;

use std::collections::BTreeMap;
use std::ops::Range;

use crate::frame::placement::{PlacementSemantics, PlacementSink};
use crate::geom::{Rect, Scale, Size};
use crate::tree::{KeyPath, NodeKind, Role, TextWrap, ViewNode};

pub use proposal::{MeasureCache, MeasureKey, Proposal, SizeProposal};

/// What a text node needs measured.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRequest<'a> {
    /// The content.
    pub text: &'a str,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<&'a str>,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
    /// Width the run may use, or `None` for an open probe.
    pub available_width: Option<f32>,
}

/// What a shaper answers with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextMeasurement {
    /// Extent of the shaped run in logical units.
    pub size: Size,
    /// Whether content was dropped to fit.
    pub truncated: bool,
    /// Rendered line count.
    pub lines: usize,
}

/// Measurement of content the engine cannot size on its own.
///
/// This is the U-09 boundary in trait form: `gorgon-petra` declares what it
/// needs measured, `gorgon-petra-egui` answers with shaped galleys, and no
/// egui type crosses.
pub trait ContentMeasure {
    /// Shape a text run.
    fn text(&mut self, req: &TextRequest<'_>) -> TextMeasurement;
    /// Size an image for an offer.
    fn image(&mut self, source: &str, proposal: SizeProposal) -> Size;
    /// Size a host-registered custom node for an offer.
    fn custom(&mut self, name: &str, proposal: SizeProposal) -> Size;
}

/// Materialization of virtualized collection rows.
///
/// A `collection` node declares a row count and a source name; the rows
/// themselves live in the store (D-075), so the engine asks for the window it
/// is about to place and never holds the whole list.
pub trait RowSource {
    /// The rows in `range` of `source`, in order. A source that cannot answer
    /// returns fewer rows; the caller reports the shortfall rather than
    /// fabricating nodes.
    fn rows(&mut self, source: &str, range: Range<usize>) -> Vec<ViewNode>;
}

/// The read-only state one frame negotiates against.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutState {
    /// Scroll offset per scroll-container id, in logical units along its axis.
    pub scroll_offsets: BTreeMap<String, f32>,
    /// The focused node id, if any.
    pub focused: Option<String>,
    /// Revision of the content behind this snapshot. Part of the cache key.
    pub content_rev: u64,
}

impl LayoutState {
    /// The scroll offset for `id`, clamped to zero or more.
    #[must_use]
    pub fn scroll_offset(&self, id: &str) -> f32 {
        self.scroll_offsets
            .get(id)
            .copied()
            .filter(|v| v.is_finite())
            .unwrap_or(0.0)
            .max(0.0)
    }
}

/// Everything a negotiation pass carries.
pub struct LayoutCtx<'a> {
    /// Content measurement boundary.
    pub content: &'a mut dyn ContentMeasure,
    /// Row materialization boundary.
    pub rows: &'a mut dyn RowSource,
    /// Memoized responses.
    pub cache: &'a mut MeasureCache,
    /// The state snapshot this frame reads.
    pub state: &'a LayoutState,
    /// Theme snapshot revision in force.
    pub theme_rev: u64,
    /// Display scale.
    pub scale: Scale,
}

impl LayoutCtx<'_> {
    /// The cache key for `path` under `proposal`.
    #[must_use]
    pub fn key(&self, path: &KeyPath, proposal: SizeProposal) -> MeasureKey {
        MeasureKey {
            node: path.id(),
            proposal,
            content_rev: self.state.content_rev,
            theme_rev: self.theme_rev,
            scale: self.scale,
        }
    }
}

/// The rect, paint order, clip, and opacity a parent gives one child.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Where the child goes, logical units.
    pub rect: Rect,
    /// Paint order.
    pub z: i32,
    /// The clip in force.
    pub clip: Rect,
    /// Cumulative opacity in `[0, 1]`.
    pub opacity: f32,
}

impl Slot {
    /// A slot filling `rect` with no clipping beyond it.
    #[must_use]
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            z: 0,
            clip: rect,
            opacity: 1.0,
        }
    }

    /// This slot moved to `rect`, keeping z, clip, and opacity.
    #[must_use]
    pub fn with_rect(self, rect: Rect) -> Self {
        Self { rect, ..self }
    }

    /// This slot with the clip narrowed to the intersection with `clip`.
    #[must_use]
    pub fn clipped_to(self, clip: Rect) -> Self {
        Self {
            clip: self.clip.intersect(clip),
            ..self
        }
    }

    /// This slot with `z` added to the paint order.
    #[must_use]
    pub fn above(self, z: i32) -> Self {
        Self {
            z: self.z.saturating_add(z),
            ..self
        }
    }

    /// This slot with opacity multiplied by `factor`.
    #[must_use]
    pub fn faded(self, factor: f32) -> Self {
        Self {
            opacity: (self.opacity * factor).clamp(0.0, 1.0),
            ..self
        }
    }
}

/// Measure `node` under `proposal`.
///
/// Applies the node's own constraints to whatever its kind answers, and
/// memoizes the result. `path` is the walk's key path: it is pushed and popped
/// here, so a caller passes the *parent's* path.
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    path.push(node.key.clone());
    let key = ctx.key(path, proposal);
    let answer = if let Some(hit) = ctx.cache.peek(&key) {
        hit
    } else {
        let raw = measure_kind(node, ctx, path, proposal);
        let clamped = node.constraints.clamp_size(raw).sane();
        ctx.cache.insert(key, clamped);
        clamped
    };
    path.pop();
    answer
}

/// Place `node` into `slot`, emitting one placement for it and for every node
/// under it. `path` is pushed and popped here, as in [`measure`].
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    path.push(node.key.clone());
    let slot = match node.props.opacity {
        Some(o) => slot.faded(o),
        None => slot,
    };
    let slot = match node.props.z {
        Some(z) => slot.above(z),
        None => slot,
    };
    place_kind(node, ctx, path, slot, sink);
    path.pop();
}

fn measure_kind(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    match node.kind {
        NodeKind::Stack => stack::measure(node, ctx, path, proposal),
        NodeKind::Grid => grid::measure(node, ctx, path, proposal),
        NodeKind::Overlay => overlay::measure(node, ctx, path, proposal),
        NodeKind::Scroll => scroll::measure(node, ctx, path, proposal),
        NodeKind::Collection => scroll::measure_collection(node, ctx, path, proposal),
        NodeKind::Surface => overlay_surface::measure(node, ctx, path, proposal),
        NodeKind::Text => text::measure(node, ctx, proposal),
        NodeKind::Image | NodeKind::Input | NodeKind::Spacer | NodeKind::Separator
        | NodeKind::Custom => leaf::measure(node, ctx, proposal),
    }
}

fn place_kind(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    match node.kind {
        NodeKind::Stack => stack::place(node, ctx, path, slot, sink),
        NodeKind::Grid => grid::place(node, ctx, path, slot, sink),
        NodeKind::Overlay => overlay::place(node, ctx, path, slot, sink),
        NodeKind::Scroll => scroll::place(node, ctx, path, slot, sink),
        NodeKind::Collection => scroll::place_collection(node, ctx, path, slot, sink),
        NodeKind::Surface => overlay_surface::place(node, ctx, path, slot, sink),
        NodeKind::Text => text::place(node, ctx, path, slot, sink),
        NodeKind::Image | NodeKind::Input | NodeKind::Spacer | NodeKind::Separator
        | NodeKind::Custom => leaf::place(node, ctx, path, slot, sink),
    }
}

/// The semantic payload for one node, built the same way by every container.
#[must_use]
pub fn semantics_of(node: &ViewNode) -> PlacementSemantics {
    PlacementSemantics {
        role: node.semantics.role.clone().or_else(|| default_role(node.kind)),
        label: node.semantics.label.clone(),
        value: node.semantics.value.clone(),
        disabled: node.semantics.disabled,
        selected: node.semantics.selected,
        expanded: node.semantics.expanded,
        stale: node.semantics.stale,
        ambient: node.ambient,
        actions: node.interactions.clone(),
        total_count: node.props.total_count,
    }
}

/// The role a built-in kind carries when the author declares none.
#[must_use]
pub fn default_role(kind: NodeKind) -> Option<Role> {
    Some(match kind {
        NodeKind::Stack | NodeKind::Grid | NodeKind::Overlay => Role::Pane,
        NodeKind::Scroll => Role::Scroll,
        NodeKind::Collection => Role::List,
        NodeKind::Surface => Role::Overlay,
        NodeKind::Text => Role::Label,
        NodeKind::Image => Role::Image,
        NodeKind::Input => Role::TextInput,
        NodeKind::Separator => Role::Separator,
        // A spacer is empty space and a custom node is whatever its host says
        // it is. Inventing a role for either would put a node in the
        // accessibility tree that describes nothing.
        NodeKind::Spacer | NodeKind::Custom => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{LayoutState, Slot, default_role, semantics_of};
    use crate::geom::Rect;
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

    #[test]
    fn slots_compose_clip_z_and_opacity() {
        let slot = Slot::new(Rect::new(0.0, 0.0, 100.0, 100.0))
            .clipped_to(Rect::new(10.0, 10.0, 50.0, 50.0))
            .above(5)
            .faded(0.5)
            .faded(0.5);
        assert_eq!(slot.clip, Rect::new(10.0, 10.0, 50.0, 50.0));
        assert_eq!(slot.z, 5);
        assert_eq!(slot.opacity, 0.25);
    }

    #[test]
    fn clipping_only_ever_narrows() {
        let slot = Slot::new(Rect::new(0.0, 0.0, 10.0, 10.0))
            .clipped_to(Rect::new(0.0, 0.0, 1000.0, 1000.0));
        assert_eq!(slot.clip, Rect::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn declared_semantics_win_over_the_kind_default() {
        let node = ViewNode::new(NodeKind::Text, "t").interactive(
            Role::Button,
            "Reload",
            &[Interaction::Click],
        );
        let sem = semantics_of(&node);
        assert_eq!(sem.role, Some(Role::Button));
        assert_eq!(sem.label.as_deref(), Some("Reload"));
        assert_eq!(sem.actions, vec![Interaction::Click]);
    }

    #[test]
    fn empty_and_host_defined_kinds_get_no_invented_role() {
        assert_eq!(default_role(NodeKind::Spacer), None);
        assert_eq!(default_role(NodeKind::Custom), None);
        assert_eq!(default_role(NodeKind::Scroll), Some(Role::Scroll));
    }

    #[test]
    fn a_missing_or_bad_scroll_offset_reads_as_zero() {
        let mut state = LayoutState::default();
        state.scroll_offsets.insert("/a".into(), -5.0);
        state.scroll_offsets.insert("/b".into(), f32::NAN);
        state.scroll_offsets.insert("/c".into(), 12.0);
        assert_eq!(state.scroll_offset("/a"), 0.0);
        assert_eq!(state.scroll_offset("/b"), 0.0);
        assert_eq!(state.scroll_offset("/c"), 12.0);
        assert_eq!(state.scroll_offset("/missing"), 0.0);
    }
}
