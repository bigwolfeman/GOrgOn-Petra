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

use crate::frame::placement::{PaintContent, PlacementSemantics, PlacementSink, TextPaint};
use crate::geom::{Rect, Scale, Size};
use crate::tree::props::ScrollProps;
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

/// One enclosing `scroll` container, as the subtree inside it sees it.
///
/// A `collection` is virtualized against the `scroll` that carries it, and
/// the scrolling parameters belong to that ancestor rather than to the list
/// (`contracts/view-tree.md` §"Virtualized collections": "a `scroll`
/// container with a collection child materializes only the visible window
/// plus declared overscan"). The walk is what knows the ancestry, so the walk
/// is what carries it: [`scroll::place`] and [`scroll::measure`] push a frame
/// around their child, and [`scroll::place_collection`] reads the innermost
/// one back off [`LayoutCtx::enclosing_scroll`].
///
/// The frame deliberately carries no offset and no viewport rect. The offset
/// is already in the geometry — a `scroll` places its child at `-offset`, so
/// the distance from a descendant's own rect to the clip in force *is* the
/// offset that reached it, composed across however many containers and
/// however many nested scrolls sit between them. Copying the number in here
/// as well would be a second source for one fact.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollFrame {
    /// Canonical id of the `scroll` node this frame belongs to.
    pub id: String,
    /// That node's resolved scrolling parameters.
    pub props: ScrollProps,
}

/// The chain of `scroll` ancestors in force at one point in the walk,
/// innermost last.
///
/// Constructed empty and mutated only by [`LayoutCtx::within_scroll`] and
/// [`LayoutCtx::outside_scroll`], which bracket exactly one call each: no
/// container can push without popping.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScrollStack(Vec<ScrollFrame>);

impl ScrollStack {
    /// An empty stack: the walk starts outside every scroll container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The innermost enclosing `scroll`, or `None` outside every scroll.
    #[must_use]
    pub fn innermost(&self) -> Option<&ScrollFrame> {
        self.0.last()
    }

    /// How many `scroll` containers enclose this point in the walk.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.0.len()
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
    /// The `scroll` ancestors of wherever the walk currently is. Start it
    /// empty; the walk maintains it.
    pub scroll: ScrollStack,
}

impl LayoutCtx<'_> {
    /// The cache key for `path` under `proposal`.
    ///
    /// The scroll context is deliberately not part of the key: a node's key
    /// path already determines its ancestry, so two entries under the same
    /// `path` cannot have been measured under two different scroll frames.
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

    /// The innermost enclosing `scroll`, or `None` outside every scroll.
    #[must_use]
    pub fn enclosing_scroll(&self) -> Option<&ScrollFrame> {
        self.scroll.innermost()
    }

    /// Run `f` with `frame` as the innermost enclosing scroll.
    ///
    /// The push and the pop bracket one call with nothing between them, so
    /// there is no early return, no `?`, and no conditional that can leave
    /// the stack unbalanced. (A panic escaping `f` unwinds out of the whole
    /// negotiation pass, which drops the context, so an unbalanced stack is
    /// not observable there either.)
    pub fn within_scroll<R>(&mut self, frame: ScrollFrame, f: impl FnOnce(&mut Self) -> R) -> R {
        self.scroll.0.push(frame);
        let answer = f(self);
        self.scroll.0.pop();
        answer
    }

    /// Run `f` with no enclosing scroll at all.
    ///
    /// A `surface` floats free of its ancestors' flow, and that includes
    /// their scrolling: it is anchored in viewport coordinates and is not
    /// moved by an ancestor `scroll`'s offset (`layout::overlay_surface`).
    /// A list inside a popup is therefore not virtualized against the panel
    /// the popup was declared in. Bracketed the same way as
    /// [`LayoutCtx::within_scroll`].
    pub fn outside_scroll<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let saved = std::mem::take(&mut self.scroll);
        let answer = f(self);
        self.scroll = saved;
        answer
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
    // Every container pushes its own placement before any child's, so the
    // index the dispatcher noted before the call is this node's. Attaching
    // here rather than inside each container means no container has to
    // remember to carry paint content, and the twelve kinds cannot drift
    // apart on what "carrying" means.
    let index = sink.len();
    place_kind(node, ctx, path, slot, sink);
    if sink.len() > index {
        debug_assert_eq!(
            sink.placed()[index].id,
            path.id(),
            "a container must push its own placement before its children's"
        );
        let content = paint_content_of(node);
        if !content.is_empty() {
            sink.attach(index, content);
        }
    }
    path.pop();
}

/// What this node draws, beyond its rect.
///
/// Derived from the tree rather than from the placement, because the placement
/// deliberately holds only what the digest hashes
/// (`contracts/frame-identity.md`).
#[must_use]
pub fn paint_content_of(node: &ViewNode) -> PaintContent {
    let props = &node.props;
    let text = match node.kind {
        NodeKind::Text => Some(props.text.clone().unwrap_or_default()),
        // An empty field draws its placeholder, which is why the placeholder
        // is what gets painted rather than the empty string.
        NodeKind::Input => Some(match props.text.as_deref() {
            Some(t) if !t.is_empty() => t.to_owned(),
            _ => props.placeholder.clone().unwrap_or_default(),
        }),
        _ => None,
    };
    PaintContent {
        text: text.map(|text| TextPaint {
            text,
            style: props.style.clone(),
            wrap: props.wrap.unwrap_or_default(),
            max_lines: props.max_lines,
        }),
        image: match node.kind {
            NodeKind::Image => props.image.clone(),
            _ => None,
        },
        custom: match node.kind {
            NodeKind::Custom => props.custom_kind.clone(),
            _ => None,
        },
        tokens: props.tokens.clone(),
    }
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
        NodeKind::Image
        | NodeKind::Input
        | NodeKind::Spacer
        | NodeKind::Separator
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
        NodeKind::Image
        | NodeKind::Input
        | NodeKind::Spacer
        | NodeKind::Separator
        | NodeKind::Custom => leaf::place(node, ctx, path, slot, sink),
    }
}

/// The semantic payload for one node, built the same way by every container.
#[must_use]
pub fn semantics_of(node: &ViewNode) -> PlacementSemantics {
    PlacementSemantics {
        role: node
            .semantics
            .role
            .clone()
            .or_else(|| default_role(node.kind)),
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

    /// The dispatcher attaches paint content; containers never have to.
    #[test]
    fn the_dispatcher_pairs_every_placement_with_what_it_draws() {
        use crate::frame::{PlacementList, TransitionActivity, Viewport, petrify};
        use crate::geom::Size;
        use crate::testing::Harness;
        use crate::token::ThemeMode;
        use crate::tree::Props;

        let tree = ViewNode::new(NodeKind::Stack, "root")
            .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                text: Some("Fibers".into()),
                style: Some("heading".into()),
                ..Props::default()
            }))
            .child(ViewNode::new(NodeKind::Spacer, "gap"));

        let mut h = Harness::new();
        let frame = petrify(
            1,
            &tree,
            &mut h.ctx(),
            Viewport::new(Size::new(200.0, 100.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );

        assert_eq!(frame.placements.len(), frame.content.len());
        let drawn: Vec<(&str, Option<&str>)> = frame
            .drawn()
            .map(|(p, c)| (p.id.as_str(), c.text.as_ref().map(|t| t.text.as_str())))
            .collect();
        assert_eq!(
            drawn,
            [
                ("/root", None),
                ("/root/title", Some("Fibers")),
                ("/root/gap", None),
            ]
        );
        let title = frame.content[1].text.as_ref().unwrap();
        assert_eq!(title.style.as_deref(), Some("heading"));
        assert!(
            frame.content[0].is_empty(),
            "a bare stack draws nothing of its own"
        );

        // The sink is what pairs them, so a hand-driven walk agrees.
        let mut sink = PlacementList::new();
        assert_eq!(sink.len(), 0);
        assert!(sink.is_empty());
        let mut path = crate::tree::KeyPath::root();
        super::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        assert_eq!(sink.content().len(), sink.as_slice().len());
    }

    /// An empty field paints its placeholder; a filled one paints its content.
    #[test]
    fn an_empty_field_draws_its_placeholder() {
        use crate::tree::Props;
        let empty = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some("Filter…".into()),
            ..Props::default()
        });
        assert_eq!(
            super::paint_content_of(&empty).text.unwrap().text,
            "Filter…"
        );
        let filled = ViewNode::new(NodeKind::Input, "f").with_props(Props {
            text: Some("fiber".into()),
            placeholder: Some("Filter…".into()),
            ..Props::default()
        });
        assert_eq!(super::paint_content_of(&filled).text.unwrap().text, "fiber");
    }

    /// Token references reach the renderer through the payload, not through
    /// the placement — but they still reach the digest, hashed into
    /// `PaintState::paint_hash` when the payload is attached.
    ///
    /// This comment used to end "the digest hashes the theme revision, not the
    /// names", and stated a defect as though it were a design: the theme
    /// revision is one global number, so rebinding this node's `background`
    /// from `surface.raised` to `status.down` repainted the panel and moved no
    /// digest. `gorgon/petra/tests/frame_digest_coverage.rs` is where the two
    /// bindings are now proven to differ.
    #[test]
    fn token_references_reach_the_payload() {
        use crate::tree::Props;
        let mut props = Props::default();
        props
            .tokens
            .insert("background".into(), "surface.raised".into());
        let node = ViewNode::new(NodeKind::Stack, "panel").with_props(props);
        let content = super::paint_content_of(&node);
        assert_eq!(
            content.tokens.get("background").map(String::as_str),
            Some("surface.raised")
        );
        assert!(!content.is_empty());
        assert_ne!(
            crate::frame::digest::hash_paint_content(&content),
            0,
            "a node that binds a token draws something, so its payload hash \
             must not be the zero a bare container carries"
        );
    }

    /// The scroll context is a stack: the innermost frame answers, the
    /// bracket restores what it found, and a `surface`-style reset restores
    /// too.
    #[test]
    fn the_scroll_context_nests_and_unwinds() {
        use super::{ScrollFrame, ScrollStack};
        use crate::geom::Axis;
        use crate::testing::Harness;
        use crate::tree::props::{DEFAULT_OVERSCAN, ScrollProps};

        let frame = |id: &str, overscan: f32| ScrollFrame {
            id: id.to_owned(),
            props: ScrollProps {
                axis: Axis::Vertical,
                overscan,
            },
        };

        let mut h = Harness::new();
        let mut ctx = h.ctx();
        assert_eq!(ctx.scroll, ScrollStack::new());
        assert!(
            ctx.enclosing_scroll().is_none(),
            "the walk starts outside every scroll"
        );

        ctx.within_scroll(frame("/outer", 200.0), |ctx| {
            assert_eq!(
                ctx.enclosing_scroll().map(|f| f.id.as_str()),
                Some("/outer")
            );
            ctx.within_scroll(frame("/outer/inner", DEFAULT_OVERSCAN), |ctx| {
                let inner = ctx.enclosing_scroll().expect("inside two scrolls");
                assert_eq!(inner.id, "/outer/inner", "the innermost frame answers");
                assert_eq!(inner.props.overscan, DEFAULT_OVERSCAN);
                assert_eq!(ctx.scroll.depth(), 2);

                ctx.outside_scroll(|ctx| {
                    assert!(
                        ctx.enclosing_scroll().is_none(),
                        "a surface clears the chain"
                    );
                    assert_eq!(ctx.scroll.depth(), 0);
                });
                assert_eq!(ctx.scroll.depth(), 2, "the chain is restored");
            });
            assert_eq!(
                ctx.enclosing_scroll().map(|f| f.id.as_str()),
                Some("/outer")
            );
        });
        assert_eq!(ctx.scroll.depth(), 0, "every push is popped");
    }

    /// The walk leaves the stack the way it found it, whatever the tree.
    #[test]
    fn placing_a_tree_of_scrolls_leaves_the_context_empty() {
        use crate::frame::PlacementList;
        use crate::testing::Harness;
        use crate::tree::Props;

        let tree = ViewNode::new(NodeKind::Scroll, "outer").child(
            ViewNode::new(NodeKind::Stack, "body")
                .child(ViewNode::new(NodeKind::Scroll, "inner").child(
                    ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
                        total_count: Some(4),
                        source: Some("fibers".into()),
                        ..Props::default()
                    }),
                ))
                .child(ViewNode::new(NodeKind::Text, "footer")),
        );

        let mut h = Harness::new();
        let mut ctx = h.ctx();
        let mut path = crate::tree::KeyPath::root();
        let mut sink = PlacementList::new();
        super::place(
            &tree,
            &mut ctx,
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        assert_eq!(ctx.scroll.depth(), 0);
        assert!(ctx.enclosing_scroll().is_none());
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
