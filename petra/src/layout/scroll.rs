//! The scroll container and its virtualized `collection` child.
//!
//! `scroll` clips and offsets one child along a declared axis
//! (`contracts/view-tree.md` §"Negotiation algorithm" item 4: "the scrolling
//! axis proposes `Unbounded` to content; the viewport axis passes through").
//! `collection` is a virtualized row range read from
//! [`crate::layout::RowSource`] rather than a child list: only the visible
//! window plus declared overscan is ever materialized (FR-009), and
//! `total_count` reaches the semantic tree through
//! [`crate::layout::semantics_of`] regardless of how few rows are placed.
//!
//! # Row identity
//! A materialized row's placement id is the row node's own `key`
//! (`row-{index}` from [`crate::testing::GeneratedRows::row`], or whatever a
//! real store-side source names it) appended to the collection's path by the
//! ordinary [`crate::layout::place`] dispatch. Nothing in this module
//! invents or renumbers an id, so the same content index always produces the
//! same id no matter where the scroll window currently sits.
//!
//! # The scroll ancestor owns the axis and the overscan
//! Both belong to the container that has a viewport and an offset, so both
//! are read off the `scroll`, never off the `collection`. [`place`] and
//! [`measure`] push a [`crate::layout::ScrollFrame`] around their child, and
//! [`place_collection`]/[`measure_collection`] read the innermost frame back
//! off the context. Tree acceptance refuses a `collection` that declares
//! `overscan`, or an `axis` that disagrees with its scroll's, so a
//! declaration is never silently overridden
//! (`crate::tree::Violation::ScrollParamOwnedByAncestor`).
//!
//! A `collection` with *no* `scroll` ancestor is legal and keeps its own
//! `axis` and `overscan`: nothing can scroll it, so nothing else can own
//! those values, and its window is simply whatever its clip shows.
//!
//! # Where the offset comes from
//! Nowhere, as a number. A `scroll` places its child at `-offset`, so by the
//! time a `collection` is placed the offset is already in its rect: the
//! distance from the collection's own rect to the clip in force is the
//! offset that reached it, composed across every container and every nested
//! scroll in between. That is why this module never looks an offset up by
//! id for anything but the `scroll`'s own placement, and why a stale entry
//! under a `stack`'s id cannot be mistaken for one.
//!
//! # What `estimated_extent` governs, and what it cannot break
//! The estimate is the extent of a row no frame has measured. Every row a
//! frame places is measured and remembered in the host's
//! [`crate::layout::RowExtents`], so the content extent (`measure_collection`,
//! which never fetches a row — FR-009) and the window's first row
//! (`place_collection`) are sums of measured extents where known and the
//! estimate elsewhere ([`crate::layout::extents`]).
//!
//! Inside the window rows sit at their own measured extents, so two placed
//! rows are disjoint however wrong the estimate is. This used to be `index *
//! estimated_extent` for the position and the measured extent for the size:
//! the inspector's fiber list declared 28 for rows that measure 44, and every
//! row was drawn 16 units on top of the row above it. That shipped, passed
//! every frame-level test, and was found by looking at the picture. After
//! that, the window's first row still sat on the estimate's grid, which made
//! the end of a list of tall rows unreachable and jumped every visible row
//! when a wheel step moved the window's first row; the ledger is what closed
//! both.
//!
//! So a wrong estimate now costs a content extent that is wrong until the
//! rows are seen, and one more fetch when the first guess at the window's
//! last row falls short. Content still wants an honest `estimated_extent`.

use crate::frame::placement::{PaintState, Placement, PlacementSink};
use crate::geom::{Axis, Rect, Size};
use crate::layout::{LayoutCtx, Proposal, ScrollFrame, SizeProposal, Slot, semantics_of};
use crate::token::TokenName;
use crate::tree::props::{DEFAULT_ROW_EXTENT, ScrollIndicators, ScrollProps};
use crate::tree::{KeyPath, ViewNode};

/// Measure this container under `proposal`.
///
/// `path` already names this node: the dispatcher pushed it. Child measurement
/// goes through [`crate::layout::measure`], which pushes the child's own key.
///
/// Padding insets the *viewport* this container reports, exactly as it insets
/// the actual clip region at `place` time (see `place`'s doc): the padded
/// space is never available to the scrolling content, on either axis, so a
/// shorter viewport is what a parent negotiates against. The proposal is
/// shrunk by the padding first — the same reserve-before-distribute shape
/// `stack.rs`'s `spacing` already uses (`layout-insets.md` §4) — and the
/// padding is added back to whatever this container ends up reporting, so an
/// offer smaller than the padding itself still answers at least the padding
/// (a scroll container can never be smaller than its own padding).
pub fn measure(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let scroll = node.props.scroll();
    let axis = scroll.axis;
    let padding = ctx.padding(&node.props.padding);
    let Some(child) = node.children.first() else {
        // No content: nothing to negotiate against, and this container's
        // own declared `constraints` (applied centrally by the dispatcher
        // after this returns) are the only size signal left — never an echo
        // of whatever the parent offered. Echoing the proposal here (the
        // same shape the viewport axis uses below, once there *is* a child)
        // would turn a childless scroll into an elastic spacer that takes
        // whatever budget `distribute` offers it, which defeats the point
        // of a scroll region declaring its own minimum in a negotiation
        // (`layout_matrix.rs`'s `the_concession_order_is_slack_then_...`
        // fixture uses exactly this shape: an empty `Scroll` standing in
        // for not-yet-materialized content, relying on its own `min`, not
        // on echoing its offer, to hold a floor during `concede`).
        return Size::from_axes(axis, padding.along(axis), padding.along(axis.cross()));
    };

    let padded_proposal = SizeProposal {
        horizontal: proposal.horizontal.shrink(padding.along(Axis::Horizontal)),
        vertical: proposal.vertical.shrink(padding.along(Axis::Vertical)),
    };

    // The scrolling axis always probes the content's maximum useful extent,
    // regardless of what this container was itself offered on that axis —
    // that is the number a `Zero`/`Unbounded`/`Unspecified` proposal needs
    // below, and it is also how `place` later learns the content extent for
    // offset clamping. The cross axis passes the incoming (padded) offer
    // straight through, unchanged.
    let mut child_proposal = padded_proposal.with_axis(axis, Proposal::Unbounded);
    if scroll.cross_scroll {
        child_proposal = child_proposal.with_axis(axis.cross(), Proposal::Unbounded);
    }
    // Measured inside this container's own scroll frame: a `collection`
    // below answers a different size depending on which axis scrolls, and
    // that axis is this node's.
    let child_size = ctx.within_scroll(frame_for(node, path, padded_proposal), |ctx| {
        crate::layout::measure(child, ctx, path, child_proposal)
    });

    // On the scrolling axis this container answers what its *viewport*
    // takes, not what its content takes: an `Exact` offer is honoured
    // exactly, `Zero` is the minimum, and only the two open probes reach for
    // the content extent just measured.
    // A hugging scroll answers an `Exact` offer along its axis with the
    // shorter of the offer and its content (`Props::hug_content`).
    let viewport_along = |at: Axis| match padded_proposal.axis(at) {
        Proposal::Exact(v) if scroll.hug_content && at == axis => {
            v.max(0.0).min(child_size.along(at))
        }
        Proposal::Exact(v) => v.max(0.0),
        Proposal::Zero => 0.0,
        Proposal::Unbounded | Proposal::Unspecified => child_size.along(at),
    };
    // A two-axis scroll answers its viewport on the cross axis the same
    // way; a one-axis scroll passes its content's cross extent through.
    let cross_extent = if scroll.cross_scroll {
        viewport_along(axis.cross())
    } else {
        child_size.across(axis)
    };
    // A thumb's gutter is part of what this container takes when the offer
    // already shows the content overflows: `place` lays the content out
    // short of it, so an open offer that answered the content alone would
    // squeeze the content by the gutter.
    let automatic = scroll.indicators == ScrollIndicators::Automatic;
    let gutters = Gutters::of(ctx, axis, padding);
    let exact_overflow = |at: Axis| match padded_proposal.axis(at) {
        Proposal::Exact(v) => child_size.along(at) - v.max(0.0) > SCROLL_SLACK,
        _ => false,
    };
    let open = |at: Axis| {
        matches!(
            padded_proposal.axis(at),
            Proposal::Unbounded | Proposal::Unspecified
        )
    };
    let main_gutter = if automatic && exact_overflow(axis) && open(axis.cross()) {
        gutters.main
    } else {
        0.0
    };
    let cross_gutter =
        if automatic && scroll.cross_scroll && exact_overflow(axis.cross()) && open(axis) {
            gutters.cross
        } else {
            0.0
        };
    Size::from_axes(
        axis,
        viewport_along(axis) + padding.along(axis) + cross_gutter,
        cross_extent + padding.along(axis.cross()) + main_gutter,
    )
}

/// Place this container and everything under it into `slot`.
pub fn place(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let id = path.id();
    let semantics = semantics_of(node, &id, ctx.state);
    let me = sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
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
    sink.enter(me);

    if let Some(child) = node.children.first() {
        let scroll = node.props.scroll();
        let axis = scroll.axis;
        // The padded interior: this container's own placed rect (`slot.rect`,
        // pushed above, unmodified) never moves or shrinks for its own
        // padding — only what it offers its child does. Padding insets the
        // *viewport* here (`layout-insets.md` §3, §8 step 5), so `content`,
        // not `slot.rect`, is what every offset/clip computation below is
        // relative to: the padded strip is a permanent gutter the content
        // never enters, at either scroll extreme.
        let padding = ctx.padding(&node.props.padding);
        let gutters = Gutters::of(ctx, axis, padding);
        // A thumb owns a gutter along its edge: content is laid out and
        // clipped short of it, so a thumb never draws over a line of text
        // (`Gutters`). Which thumbs show depends on the content measured in
        // the port the gutters leave, so the port settles in at most four
        // rounds. A gutter, once taken, is kept for the frame (taking one
        // only shrinks the port, which can only add overflow), except a
        // one-axis scroll's gutter its content cannot shrink by, which is
        // given back once and not taken again.
        let automatic = scroll.indicators == ScrollIndicators::Automatic;
        let (mut main_bar, mut cross_bar) = (false, false);
        // A one-axis scroll cannot scroll across, so a gutter its content
        // cannot shrink by (a strip of tabs as tall as the port) would cut
        // the content off. That thumb stays over the content instead.
        let mut main_refused = false;
        let (content, frame, content_size, content_extent) = loop {
            let content = slot
                .rect
                .inset_edges(padding)
                .inset_edges(gutters.taken(main_bar, cross_bar));
            let viewport_size = content.size();
            // Same child-proposal shape as `measure`, but the cross axis is
            // now an `Exact` offer taken from the settled (padded) viewport
            // rather than whatever this container's own incoming proposal
            // happened to be: by `place` time negotiation is over and the
            // viewport size is a fact, not one of several offers being probed.
            let mut child_proposal =
                SizeProposal::exact(viewport_size).with_axis(axis, Proposal::Unbounded);
            if scroll.cross_scroll {
                child_proposal = child_proposal.with_axis(axis.cross(), Proposal::Unbounded);
            }
            let frame = frame_for(node, path, SizeProposal::exact(viewport_size));
            let content_size = ctx.within_scroll(frame.clone(), |ctx| {
                crate::layout::measure(child, ctx, path, child_proposal)
            });
            let viewport_extent = viewport_size.along(axis);
            // A filling scroll places its content at the longer of the
            // viewport and the content's minimum, not at its natural length:
            // a flexible child then takes the viewport's leftover, and a
            // nested scroll is never handed its whole content as a viewport
            // (`Props::fill_viewport`).
            let content_extent = if scroll.fill_viewport {
                let least = ctx.within_scroll(frame.clone(), |ctx| {
                    crate::layout::measure(
                        child,
                        ctx,
                        path,
                        child_proposal.with_axis(axis, Proposal::Zero),
                    )
                });
                least.along(axis).max(viewport_extent)
            } else {
                content_size.along(axis)
            };
            let overflows = |content: f32, viewport: f32| content - viewport > SCROLL_SLACK;
            let main_over = automatic && overflows(content_extent, viewport_extent);
            let cross_over = automatic
                && scroll.cross_scroll
                && overflows(
                    content_size.across(axis).max(viewport_size.across(axis)),
                    viewport_size.across(axis),
                );
            if main_bar
                && !scroll.cross_scroll
                && overflows(content_size.across(axis), viewport_size.across(axis))
            {
                main_refused = true;
            }
            let next_main = (main_bar || main_over) && !main_refused;
            let next_cross = cross_bar || cross_over;
            if (next_main, next_cross) == (main_bar, cross_bar) {
                break (content, frame, content_size, content_extent);
            }
            (main_bar, cross_bar) = (next_main, next_cross);
        };
        let viewport_size = content.size();
        let viewport_extent = viewport_size.along(axis);
        let max_offset = (content_extent - viewport_extent).max(0.0);

        // `scroll_offset` already floors a missing/negative/non-finite entry
        // at zero; clamping against `max_offset` is the other half — a
        // snapshot taken before a prepend or a resize can name an offset
        // past the content that exists now, and that must not blank the
        // viewport.
        let raw_offset = ctx.state.scroll_offset(&path.id());
        let offset = raw_offset.clamp(0.0, max_offset);

        let main_origin = origin_along(content, axis) - offset;
        // Across the axis, `child_proposal` was `exact`, and an exact
        // proposal is a contract: the content is as wide as the scrollport it
        // sits in, whatever its own widest line measured. `max` rather than a
        // plain substitution, so content that genuinely overflows keeps its
        // overflow and stays reachable by scrolling.
        //
        // This read `content_size.across(axis)` alone until 2026-09-09, and
        // the accident that hid it was that most scrolling content is wider
        // than its port. The one that was not — the inspector's fiber-detail
        // pane, 598 units of content in an 837-unit column — placed its card
        // at 598 and left a 239-unit step down the right edge of the window,
        // against the tab strip directly beneath it, which filled.
        //
        // The same sentence, for the same reason, is already three hundred
        // lines below on a collection's rows (`place_collection`). Two places
        // is one too many; both are in this file and both now say it.
        let cross_viewport = viewport_size.across(axis);
        let cross_content = content_size.across(axis).max(cross_viewport);
        // The cross offset, clamped the same way as the main one. Zero for a
        // one-axis scroll, whose content is never wider than offered.
        let cross_offset = if scroll.cross_scroll {
            ctx.state
                .cross_offset(&path.id())
                .clamp(0.0, (cross_content - cross_viewport).max(0.0))
        } else {
            0.0
        };
        let child_rect = shifted_across(
            axis_rect(content, axis, main_origin, content_extent, cross_content),
            axis,
            cross_offset,
        );
        // The child is placed at its full content size so ordinary children
        // (a stack of rows, say) lay out naturally; only the *paint* clip is
        // narrowed to this container's padded interior, which is what makes
        // overflow reachable-but-hidden rather than painted outside — and
        // keeps content from ever drawing over the padding gutter, at either
        // scroll extreme.
        let child_slot = slot.with_rect(child_rect).clipped_to(content);

        ctx.within_scroll(frame, |ctx| {
            crate::layout::place(child, ctx, path, child_slot, sink);
        });

        // After the content, in the gutters it left. Placed outside the
        // scroll context: the thumb sits on the port, it does not scroll.
        // Each track stops short of the other thumb's gutter, so the two
        // thumbs never cross in the corner.
        if automatic
            && let Some(rect) = thumb_rect(
                ctx,
                gutters.track(slot.rect, axis, cross_bar),
                axis,
                viewport_extent,
                content_extent,
                offset,
            )
        {
            let thumb_slot = slot.with_rect(rect).clipped_to(slot.rect);
            crate::layout::place(&INDICATOR, ctx, path, thumb_slot, sink);
        }
        // Overflow nav: a button at each end with content past it, over the
        // content's edge, outside the scroll context like the thumb.
        if scroll.indicators == ScrollIndicators::Buttons
            && max_offset > SCROLL_SLACK
            && let Some(length) = nav_length(ctx, content, axis)
        {
            if offset > SCROLL_SLACK {
                let rect = nav_rect(content, axis, length, false);
                crate::layout::place(
                    &NAV_BACK[axis_index(axis)],
                    ctx,
                    path,
                    slot.with_rect(rect),
                    sink,
                );
            }
            if max_offset - offset > SCROLL_SLACK {
                let rect = nav_rect(content, axis, length, true);
                crate::layout::place(
                    &NAV_FORWARD[axis_index(axis)],
                    ctx,
                    path,
                    slot.with_rect(rect),
                    sink,
                );
            }
        }
        // The cross thumb: the same drawing, along the other edge.
        if scroll.cross_scroll
            && automatic
            && let Some(rect) = thumb_rect(
                ctx,
                gutters.track(slot.rect, axis.cross(), main_bar),
                axis.cross(),
                cross_viewport,
                cross_content,
                cross_offset,
            )
        {
            let thumb_slot = slot.with_rect(rect).clipped_to(slot.rect);
            crate::layout::place(&CROSS_INDICATOR, ctx, path, thumb_slot, sink);
        }
    }

    sink.leave();
}

/// The key of a `scroll`'s indicator: the second child every `scroll` has
/// and no author writes. Tree acceptance refuses a `scroll` child with this
/// key ([`crate::tree::Violation::DuplicateSiblingKey`]), so the thumb's id
/// can never be an author's node.
pub const INDICATOR_KEY: &str = "scroll-indicator";

/// The key of a two-axis `scroll`'s cross indicator. Refused as an authored
/// child key for the reason [`INDICATOR_KEY`] is.
pub const CROSS_INDICATOR_KEY: &str = "scroll-indicator-cross";

/// The key of a [`ScrollIndicators::Buttons`] scroll's button at the
/// leading end. Refused as an authored child key for the reason
/// [`INDICATOR_KEY`] is; a host moves the scroll that places it back by a
/// page when it is pressed.
pub const NAV_BACK_KEY: &str = "scroll-back";

/// The key of a [`ScrollIndicators::Buttons`] scroll's button at the
/// trailing end. See [`NAV_BACK_KEY`].
pub const NAV_FORWARD_KEY: &str = "scroll-forward";

/// What one press of an overflow button asks of the scroll that places it:
/// the scroll's id and the signed step along its axis, the port's visible
/// length less the button, so the item that sat under the button is the
/// first one shown. [`None`] when `id` is not an overflow button.
#[must_use]
pub fn nav_step(frame: &crate::frame::PetrifiedFrame, id: &str) -> Option<(String, f32)> {
    let (scroll, key) = id.rsplit_once('/')?;
    let sign = match key {
        NAV_BACK_KEY => -1.0,
        NAV_FORWARD_KEY => 1.0,
        _ => return None,
    };
    let port = frame.placement(scroll)?;
    if port.kind != crate::tree::NodeKind::Scroll {
        return None;
    }
    let axis = frame.scroll_axis(scroll);
    let button = frame.placement(id)?.rect.size().along(axis);
    let page = (port.rect.size().along(axis) - button).max(button);
    Some((scroll.to_owned(), sign * page))
}

/// An overflow button's length along the axis: Carbon's `$spacing-08`
/// target, or [`None`] when two of them would not leave room for content.
fn nav_length(ctx: &LayoutCtx<'_>, port: Rect, axis: Axis) -> Option<f32> {
    let length = ctx.spacing(&Some(TokenName::new("spacing-08").expect("token name")));
    (port.size().along(axis) > 3.0 * length).then_some(length)
}

/// Where an overflow button sits: across the whole port at its leading
/// (`trailing` false) or trailing end.
fn nav_rect(port: Rect, axis: Axis, length: f32, trailing: bool) -> Rect {
    let start = if trailing {
        origin_along(port, axis) + port.size().along(axis) - length
    } else {
        origin_along(port, axis)
    };
    match axis {
        Axis::Horizontal => Rect::new(start, port.y, length, port.h),
        Axis::Vertical => Rect::new(port.x, start, port.w, length),
    }
}

const fn axis_index(axis: Axis) -> usize {
    match axis {
        Axis::Horizontal => 0,
        Axis::Vertical => 1,
    }
}

/// The leading-end buttons, horizontal then vertical, built once.
static NAV_BACK: std::sync::LazyLock<[ViewNode; 2]> = std::sync::LazyLock::new(|| {
    [
        nav_button(
            NAV_BACK_KEY,
            crate::component::IconMark::ChevronLeft,
            "Scroll left",
        ),
        nav_button(
            NAV_BACK_KEY,
            crate::component::IconMark::ChevronUp,
            "Scroll up",
        ),
    ]
});

/// The trailing-end buttons, horizontal then vertical, built once.
static NAV_FORWARD: std::sync::LazyLock<[ViewNode; 2]> = std::sync::LazyLock::new(|| {
    [
        nav_button(
            NAV_FORWARD_KEY,
            crate::component::IconMark::ChevronRight,
            "Scroll right",
        ),
        nav_button(
            NAV_FORWARD_KEY,
            crate::component::IconMark::ChevronDown,
            "Scroll down",
        ),
    ]
});

/// One overflow button: an opaque surface over the content's edge with the
/// chevron centred, a button in the semantic tree, lit on hover, activated
/// on release, and outside the focus order.
fn nav_button(key: &str, mark: crate::component::IconMark, label: &str) -> ViewNode {
    use crate::tree::{Behaviour, Intent, Interaction, Phase, Role};
    let chevron =
        crate::component::icon_toned("chevron", mark, crate::component::IconTone::Primary);
    let mut node = ViewNode::new(crate::tree::NodeKind::Stack, key)
        .with_props(crate::tree::Props {
            align: Some(crate::geom::Align::Center),
            justify: Some(crate::tree::Justify::Center),
            ..crate::tree::Props::default()
        })
        .child(chevron)
        .interactive(
            Role::Button,
            label,
            &[Interaction::Click, Interaction::Hover],
        )
        .with_behaviour(Behaviour {
            intent: Intent::Activate,
            phase: Phase::OnRelease,
        });
    for (role, token) in [
        ("background", "surface.base"),
        ("background@hover", "layer-hover"),
    ] {
        node.props.tokens.insert(
            role.into(),
            TokenName::new(token).expect("a well-formed token name"),
        );
    }
    node
}

/// The thumb's thickness across the axis.
const THUMB_THICKNESS: &str = "spacing-02";
/// The gap between the thumb and the port's edges. A host's border lies
/// over the port's edge (a border takes no space), so this is also the gap
/// to that line: at 2 the thumb sat one pixel off a tile border and read as
/// a doubled line.
const THUMB_INSET: &str = "spacing-02";
/// The shortest thumb, so a very long list still shows a target.
const THUMB_MIN_LENGTH: &str = "spacing-05";

/// The indicator node: a fill and a full corner, nothing else. One node for
/// every scroll in every frame, built once. `border-strong` is the theme's
/// tone for a non-text mark that must stand at 3:1 against its layer.
static INDICATOR: std::sync::LazyLock<ViewNode> =
    std::sync::LazyLock::new(|| indicator(INDICATOR_KEY));

/// The cross indicator: [`INDICATOR`]'s drawing under its own key.
static CROSS_INDICATOR: std::sync::LazyLock<ViewNode> =
    std::sync::LazyLock::new(|| indicator(CROSS_INDICATOR_KEY));

fn indicator(key: &str) -> ViewNode {
    let mut node = ViewNode::new(crate::tree::NodeKind::Stack, key);
    for (role, token) in [
        ("background", "border-strong"),
        ("radius", "shape.corner-full"),
    ] {
        node.props.tokens.insert(
            role.into(),
            TokenName::new(token).expect("a well-formed token name"),
        );
    }
    node
}

/// Overflow under this many logical units is rounding, not content: no
/// thumb and no gutter for it. [`thumb_rect`] uses the same slack.
const SCROLL_SLACK: f32 = 0.5;

/// The space each thumb owns along its edge: the far edge across from the
/// scrolling axis for the main thumb (the right of a vertical scroll), the
/// far edge along it for a two-axis scroll's cross thumb (the bottom).
///
/// A thumb is [`THUMB_THICKNESS`] wide with [`THUMB_INSET`] on both sides,
/// so its gutter is the thickness plus two insets: the inset off the port's
/// edge and the same gap again before the content. Padding on that edge
/// already keeps content off the thumb, so it counts toward the gutter, and
/// a scroll padded at least that much reserves nothing more.
///
/// An overlay thumb drew over the line under it: a code view's horizontal
/// thumb struck through its last visible line, and its vertical thumb hid
/// the last characters of every line at the right edge
/// (`.agents/notes/implemented/bug-fix/2026-11-22-scroll-thumbs-own-a-gutter.md`
/// in GOrgOn).
#[derive(Clone, Copy, Debug)]
struct Gutters {
    /// The scrolling axis.
    axis: Axis,
    /// What the main thumb's gutter adds past the padding on its edge.
    main: f32,
    /// What the cross thumb's gutter adds past the padding on its edge.
    cross: f32,
}

impl Gutters {
    fn of(ctx: &LayoutCtx<'_>, axis: Axis, padding: crate::geom::Insets) -> Self {
        let spacing = |name: &str| ctx.spacing(&Some(TokenName::new(name).expect("token name")));
        let gutter = spacing(THUMB_THICKNESS) + 2.0 * spacing(THUMB_INSET);
        // The main thumb runs along the axis on the far cross edge; the
        // cross thumb runs across it on the far edge along it.
        let (main_edge, cross_edge) = match axis {
            Axis::Vertical => (padding.right, padding.bottom),
            Axis::Horizontal => (padding.bottom, padding.right),
        };
        Self {
            axis,
            main: (gutter - main_edge).max(0.0),
            cross: (gutter - cross_edge).max(0.0),
        }
    }

    /// The insets the shown thumbs take from the padded port.
    fn taken(self, main: bool, cross: bool) -> crate::geom::Insets {
        let main = if main { self.main } else { 0.0 };
        let cross = if cross { self.cross } else { 0.0 };
        let (right, bottom) = match self.axis {
            Axis::Vertical => (main, cross),
            Axis::Horizontal => (cross, main),
        };
        crate::geom::Insets {
            top: 0.0,
            right,
            bottom,
            left: 0.0,
        }
    }

    /// The port a thumb along `along` runs in: `port`, short of the other
    /// thumb's gutter at its far end when that thumb shows.
    fn track(self, port: Rect, along: Axis, other_shows: bool) -> Rect {
        if !other_shows {
            return port;
        }
        let other = if along == self.axis {
            self.cross
        } else {
            self.main
        };
        match along {
            Axis::Vertical => Rect::new(port.x, port.y, port.w, (port.h - other).max(0.0)),
            Axis::Horizontal => Rect::new(port.x, port.y, (port.w - other).max(0.0), port.h),
        }
    }
}

/// Where a scroll's thumb sits, or [`None`] when the content fits.
///
/// The track is the port's own rect less the inset at each end, along the
/// far edge across from `axis`: the right edge of a vertical scroll, the
/// bottom of a horizontal one. The thumb's length is the visible share of
/// the content (never shorter than the minimum, never longer than the
/// track); its position is the offset's share of the scroll range.
fn thumb_rect(
    ctx: &LayoutCtx<'_>,
    port: Rect,
    axis: Axis,
    viewport: f32,
    content: f32,
    offset: f32,
) -> Option<Rect> {
    let range = content - viewport;
    if range <= SCROLL_SLACK || !range.is_finite() {
        return None;
    }
    let spacing = |name: &str| ctx.spacing(&Some(TokenName::new(name).expect("token name")));
    let (thick, inset, min) = (
        spacing(THUMB_THICKNESS),
        spacing(THUMB_INSET),
        spacing(THUMB_MIN_LENGTH),
    );
    let track = port.size().along(axis) - 2.0 * inset;
    if track <= 0.0 || port.size().across(axis) < thick + inset {
        return None;
    }
    let length = (track * viewport / content).max(min).min(track);
    let along =
        origin_along(port, axis) + inset + (track - length) * (offset / range).clamp(0.0, 1.0);
    Some(match axis {
        Axis::Vertical => Rect::new(port.x + port.w - inset - thick, along, thick, length),
        Axis::Horizontal => Rect::new(along, port.y + port.h - inset - thick, length, thick),
    })
}

/// The scroll context this `scroll` node imposes on everything under it.
fn frame_for(node: &ViewNode, path: &KeyPath, viewport: SizeProposal) -> ScrollFrame {
    ScrollFrame {
        id: path.id(),
        props: node.props.scroll(),
        viewport,
    }
}

/// The scrolling parameters in force for a `collection`.
///
/// The nearest enclosing `scroll` owns them. A `collection` outside every
/// scroll owns its own, and resolves them exactly as a `scroll` would.
fn collection_scroll(node: &ViewNode, ctx: &LayoutCtx<'_>) -> ScrollProps {
    ctx.enclosing_scroll()
        .map_or_else(|| node.props.scroll(), |frame| frame.props)
}

/// Measure a `collection` node: a virtualized row range read from a store-side
/// source through [`crate::layout::RowSource`].
pub fn measure_collection(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    proposal: SizeProposal,
) -> Size {
    let Some(collection) = node.props.collection() else {
        return Size::ZERO;
    };
    // The scrolling axis decides which axis carries the row total and which
    // passes the offer through, so it has to be the same axis `place` will
    // lay the rows out along — the ancestor's, not this node's.
    let axis = collection_scroll(node, ctx).axis;
    let row_extent = sane_row_extent(collection.estimated_extent);

    // Rows are never fetched to answer a measurement (FR-009): the content
    // extent is what earlier frames measured, plus the estimate for every
    // row no frame has placed ([`crate::layout::extents`]).
    let content_extent = ctx.extents.content_extent(
        &path.id(),
        collection.total_count,
        row_extent,
        ctx.theme_rev,
        ctx.scale,
    );
    let cross = proposal.axis(axis.cross()).available().unwrap_or(0.0);
    Size::from_axes(axis, content_extent, cross)
}

/// The rows a collection materializes this frame, measured and positioned.
struct Window {
    first: usize,
    rows: Vec<(std::sync::Arc<ViewNode>, Rect)>,
}

/// Place a `collection` node: only the visible window plus declared overscan
/// is materialized, and `total_count` reaches the semantic tree.
pub fn place_collection(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    slot: Slot,
    sink: &mut dyn PlacementSink,
) {
    let id = path.id();
    let mut semantics = semantics_of(node, &id, ctx.state);
    // Measured before the collection's own placement is pushed, so the
    // placement can say which row its first child is.
    let window = materialize(node, ctx, path, &id, slot);
    semantics.first_row = window.as_ref().map(|w| w.first);
    let me = sink.push(Placement {
        id,
        kind: node.kind,
        rect: slot.rect,
        z: slot.z,
        clip: slot.clip,
        opacity: slot.opacity,
        paint: PaintState {
            content_hash: 0,
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
    sink.enter(me);
    for (row_node, rect) in window.map(|w| w.rows).unwrap_or_default() {
        // Inherit the scroll's viewport clip. Overscan rows sit outside it
        // on purpose: they are placed so a later frame can scroll without a
        // hole, and they are not visible, hittable, or Tab-reachable until
        // they are.
        crate::layout::place(&row_node, ctx, path, slot.with_rect(rect), sink);
    }
    sink.leave();
}

/// Fetch, measure and position the rows of the window `slot` shows, and
/// fold what they measured into the ledger.
///
/// The window is the viewport plus the scroll's overscan on each side. Its
/// rows are found through the ledger ([`crate::layout::extents`]): measured
/// extents where a frame has placed the row, the estimate elsewhere. The
/// first visible row (the anchor) is placed where the ledger said it was at
/// the start of this pass; rows after it follow at their measured extents,
/// rows before it stack upward from it. So a measurement that differs from
/// the ledger moves only rows the viewport does not show, and the distance
/// the ledger moved the anchor goes to the enclosing scroll as a shift that
/// keeps it there on the next frame.
///
/// More rows are fetched past the first guess until their measured extents
/// cover the window, so rows shorter than the estimate leave no hole at the
/// bottom, up to one row per unit of window so a list of empty rows stays
/// virtualized.
fn materialize(
    node: &ViewNode,
    ctx: &mut LayoutCtx<'_>,
    path: &mut KeyPath,
    id: &str,
    slot: Slot,
) -> Option<Window> {
    let collection = node.props.collection()?;
    let scroll = collection_scroll(node, ctx);
    let scroll_id = ctx.enclosing_scroll().map(|frame| frame.id.clone());
    let axis = scroll.axis;
    let estimate = sane_row_extent(collection.estimated_extent);
    let count = collection.total_count;
    let cross_extent = slot.rect.size().across(axis);

    // `slot.rect` is this node's own placement: for a collection that is
    // the content-sized rect `measure_collection` answered (`place` above
    // positions a child at `-offset` and only narrows `clip` to the
    // viewport). The viewport is therefore `slot.clip` — the narrowest
    // region an ancestor has clipped this subtree to, which is exactly the
    // region a row can be seen in — never `slot.rect`.
    let viewport_extent = slot.clip.size().along(axis).max(0.0);
    // How far this list's own leading edge has been scrolled past the
    // visible region. Every enclosing scroll already moved `slot.rect` by
    // its offset, and every container in between already added its own
    // leading content, so this one subtraction is the composed answer — see
    // the module doc's "Where the offset comes from".
    let offset = origin_along(slot.clip, axis) - origin_along(slot.rect, axis);
    let origin = origin_along(slot.rect, axis);

    // What `measure_collection` answered for this list on this pass (or
    // would have), read before `open` can reset or resize the ledger. The
    // fold below compares against this, not against the ledger `open`
    // hands back: a count or basis change moves the total inside `open`,
    // and comparing after it would miss a content extent that every
    // ancestor already measured and cached.
    let measured_extent = ctx
        .extents
        .content_extent(id, count, estimate, ctx.theme_rev, ctx.scale);
    // The ledger as this pass found it: where the anchor is, and where every
    // row the window starts from is.
    let list = ctx
        .extents
        .open(id, estimate, cross_extent, ctx.theme_rev, ctx.scale, count);
    let before = list.list().clone();
    let total = before.total();
    // A list scrolled entirely out of the visible region gets an empty
    // window and materializes nothing, which is the honest answer: none of
    // its rows can be seen.
    let window_start = (offset - scroll.overscan).max(0.0);
    let window_end = (offset + viewport_extent + scroll.overscan).min(total);
    if count == 0 || window_end <= window_start {
        // Nothing to fetch, but `open` may still have moved the total.
        if total != measured_extent {
            ctx.cache.apply(&crate::layout::ChangeSet::Nodes(
                std::iter::once(id.to_owned()).collect(),
            ));
        }
        return None;
    }
    let anchor = before.index_at(offset.max(0.0));
    let anchor_at = before.prefix(anchor);
    let start = before.index_at(window_start).min(anchor);
    let budget = (window_end - window_start).ceil() as usize + 1;

    // Fetch and measure: from `start` through the anchor, then on until the
    // measured rows reach the window's end.
    let mut rows: Vec<(std::sync::Arc<ViewNode>, f32, f32)> = Vec::new();
    let mut next = start;
    ctx.extents.rows.begin(id);
    let mut reach = anchor_at;
    let mut guess_end = (before.index_at(window_end) + 1).min(count);
    while next < count && rows.len() < budget && (next <= anchor || reach < window_end) {
        let end = guess_end
            .max(next + 1)
            .min(count)
            .min(next + budget - rows.len());
        // The window is at most `viewport + 2 * overscan` wide, never
        // `total_count`: `rows` is never called with a range wider than
        // that. A source that cannot answer the whole range returns fewer
        // rows (`RowSource::rows`'s contract); the shortfall is reported by
        // placing fewer rows, never by fabricating the rest.
        let fetched = ctx.rows.rows(&collection.source, next..end);
        if fetched.is_empty() {
            break;
        }
        // A row that changed under its old key must not answer from the size
        // its old content took ([`crate::layout::row_memo`]).
        let fetched = graft_rows(ctx, id, fetched, path);
        for row_node in fetched {
            let row_proposal = SizeProposal::exact(Size::from_axes(axis, estimate, cross_extent));
            let row_size = crate::layout::measure(&row_node, ctx, path, row_proposal);
            let along = row_size.along(axis).max(0.0);
            // Across the axis, the proposal was `exact`, and an exact
            // proposal is a contract: a row is as wide as the list it sits
            // in, whatever its own content measured. Placing it at the
            // measured extent instead is what left a focus bar, a hover fill
            // and a selected fill stopping wherever that row's own text
            // happened to stop, ragged down the list. `max` rather than a
            // plain substitution so a row that genuinely overflows keeps its
            // overflow and the clip stays the thing that hides it.
            let across = row_size.across(axis).max(cross_extent);
            if next >= anchor {
                reach += along;
            }
            rows.push((row_node, along, across));
            next += 1;
        }
        guess_end = next + ((window_end - reach) / estimate).ceil().max(1.0) as usize;
    }

    // Positions: the anchor where the ledger had it, the rest by measured
    // extent around it.
    let anchor_slot = anchor.saturating_sub(start).min(rows.len());
    let mut leads = vec![0.0_f32; rows.len()];
    let mut cursor = anchor_at;
    for k in anchor_slot..rows.len() {
        leads[k] = cursor;
        cursor += rows[k].1;
    }
    let mut cursor = anchor_at;
    for k in (0..anchor_slot).rev() {
        cursor -= rows[k].1;
        leads[k] = cursor;
    }

    // Fold the measurements in, then hand the scroll the distance they moved
    // the anchor.
    let (moved, changed) = {
        let mut list =
            ctx.extents
                .open(id, estimate, cross_extent, ctx.theme_rev, ctx.scale, count);
        for (k, (row_node, along, _)) in rows.iter().enumerate() {
            let key = row_node.key.as_str();
            list.check(start + k, key);
            list.record(start + k, key, *along, anchor);
        }
        (
            list.list().prefix(anchor) - anchor_at,
            list.list().total() != measured_extent,
        )
    };
    if let Some(scroll_id) = scroll_id {
        ctx.extents.shift(&scroll_id, moved);
    }
    if changed {
        // The content extent this node and every ancestor measured is stale.
        ctx.cache.apply(&crate::layout::ChangeSet::Nodes(
            std::iter::once(id.to_owned()).collect(),
        ));
    }

    let rows = rows
        .into_iter()
        .zip(leads)
        .map(|((row_node, along, across), lead)| {
            let rect = axis_rect(slot.rect, axis, origin + lead, along, across);
            (row_node, rect)
        })
        .collect();
    Some(Window { first: start, rows })
}

/// `rows`, each grafted onto the same row from this list's last fetch, with
/// the measurements of every node in them that moved dropped.
///
/// One pass over the cache for the whole batch, not one per row: a row new
/// to the memo names every node it holds, and a scroll brings several in at
/// once, so a per-row `retain` would scan the cache once per row scrolled
/// into view.
///
/// The invalidation stops at each row's own root. The collection and its
/// ancestors measure from the extents ledger, not from their rows, and a row
/// that measures differently moves the ledger, which invalidates them on its
/// own path (`materialize`'s fold).
fn graft_rows(
    ctx: &mut LayoutCtx<'_>,
    list: &str,
    rows: Vec<std::sync::Arc<ViewNode>>,
    path: &mut KeyPath,
) -> Vec<std::sync::Arc<ViewNode>> {
    let mut doomed = std::collections::HashSet::new();
    let rows = rows
        .into_iter()
        .map(|row| {
            path.push(row.key.clone());
            let row_id = path.id();
            path.pop();
            let mut moved = Vec::new();
            let row = ctx.extents.rows.graft(list, row, path, &mut moved);
            for id in moved {
                // Every ancestor of `id` from its parent up to the row's own
                // root, each a prefix of `id` ending before a separator.
                // Copied only when not already doomed: the nodes of one row
                // share their ancestors, and a new row names every node it
                // holds.
                for end in KeyPath::ancestor_ends(&id) {
                    if end < row_id.len() {
                        break;
                    }
                    let ancestor = &id[..end];
                    if !doomed.contains(ancestor) {
                        doomed.insert(ancestor.to_owned());
                    }
                }
                doomed.insert(id);
            }
            row
        })
        .collect();
    ctx.cache.invalidate_nodes(&doomed);
    rows
}

/// A declared `estimated_extent`, or the shared default when the declared
/// value is not a usable positive number.
///
/// `Props::collection` already substitutes [`DEFAULT_ROW_EXTENT`] for an
/// *absent* value; this additionally guards a present-but-unusable one (zero,
/// negative, non-finite), since it is a divisor below and a stray author
/// input must not turn virtualization into a division by zero.
fn sane_row_extent(estimated: f32) -> f32 {
    if estimated.is_finite() && estimated > 0.0 {
        estimated
    } else {
        DEFAULT_ROW_EXTENT
    }
}

/// The origin of `rect` along `axis` (`x` for horizontal, `y` for vertical).
fn origin_along(rect: Rect, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => rect.x,
        Axis::Vertical => rect.y,
    }
}

/// Build a rect from a main-axis origin and extent plus a cross-axis extent,
/// keeping the cross-axis origin from `base`. The rect analogue of
/// [`Size::from_axes`], which `geom` has no equivalent of.
fn axis_rect(
    base: Rect,
    axis: Axis,
    main_origin: f32,
    main_extent: f32,
    cross_extent: f32,
) -> Rect {
    let main_extent = main_extent.max(0.0);
    let cross_extent = cross_extent.max(0.0);
    match axis {
        Axis::Horizontal => Rect::new(main_origin, base.y, main_extent, cross_extent),
        Axis::Vertical => Rect::new(base.x, main_origin, cross_extent, main_extent),
    }
}

/// `rect` moved back by `offset` across `axis`: where a two-axis scroll's
/// content sits for its cross offset.
fn shifted_across(rect: Rect, axis: Axis, offset: f32) -> Rect {
    match axis.cross() {
        Axis::Horizontal => Rect::new(rect.x - offset, rect.y, rect.w, rect.h),
        Axis::Vertical => Rect::new(rect.x, rect.y - offset, rect.w, rect.h),
    }
}

#[cfg(test)]
mod tests {
    use crate::frame::placement::{Placement, PlacementList};
    use crate::geom::{Axis, Rect, Size};
    use crate::layout::{MeasureCache, Proposal, SizeProposal, Slot};
    use crate::testing::{GeneratedRows, Harness, MonoContent, gap_token};
    use crate::tree::{AxisConstraint, Constraints, InsetRefs, KeyPath, NodeKind, Props, ViewNode};

    /// A spacer whose measured extent on the vertical axis is pinned to
    /// `main`, no matter what proposal it is probed with — a deterministic
    /// stand-in for "a child with a known content extent" that does not
    /// depend on this module's own `Unbounded`-probing behaviour to produce
    /// its size.
    fn fixed_extent_child(main: f32) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, "content").with_constraints(Constraints {
            horizontal: AxisConstraint::default(),
            vertical: AxisConstraint {
                min: Some(main),
                max: Some(main),
                priority: 0,
            },
        })
    }

    /// A `scroll` named `key` carrying `props`, wrapping a `collection` named
    /// `rows` that declares nothing about scrolling itself.
    fn collection_in_scroll(
        key: &str,
        props: Props,
        total_count: usize,
        estimated_extent: f32,
    ) -> ViewNode {
        let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(total_count),
            source: Some("fibers".into()),
            estimated_extent: Some(estimated_extent),
            ..Props::default()
        });
        ViewNode::new(NodeKind::Scroll, key)
            .with_props(props)
            .child(collection)
    }

    /// How many collection rows a pass materialized: placements keyed
    /// `row-{index}`, whatever containers and indicators sit around them.
    fn rows_placed(placements: &[Placement]) -> usize {
        placements
            .iter()
            .filter(|p| {
                p.id.rsplit('/')
                    .next()
                    .is_some_and(|key| key.starts_with("row-"))
            })
            .count()
    }

    /// The `scroll`/`collection` pair every virtualization test drives.
    ///
    /// `overscan` is declared on the **`scroll`**, which owns it; tree
    /// acceptance refuses it on the collection. It is also never
    /// `DEFAULT_OVERSCAN` in a caller that asserts a row count: it used to
    /// be 64.0 everywhere, which *is* the default, so the count came out the
    /// same whether the declaration travelled or not.
    fn scroll_with_collection(
        total_count: usize,
        estimated_extent: f32,
        overscan: f32,
    ) -> ViewNode {
        collection_in_scroll(
            "list",
            Props {
                overscan: Some(overscan),
                ..Props::default()
            },
            total_count,
            estimated_extent,
        )
    }

    #[test]
    fn the_scrolling_axis_proposes_unbounded_while_the_cross_axis_passes_through() {
        let mut h = Harness::new();
        let tree =
            ViewNode::new(NodeKind::Scroll, "list").child(ViewNode::new(NodeKind::Spacer, "gap"));
        let mut path = KeyPath::root();

        // The viewport axis (vertical) answers the exact offer it was given;
        // the cross axis (horizontal) passes that same offer straight to the
        // spacer, which answers it exactly too.
        let viewport = crate::layout::measure(
            &tree,
            &mut h.ctx(),
            &mut path,
            SizeProposal {
                horizontal: Proposal::Exact(120.0),
                vertical: Proposal::Exact(50.0),
            },
        );
        assert_eq!(viewport, Size::new(120.0, 50.0));

        // An `Unbounded` probe on the scroll axis reaches the content extent
        // regardless of the incoming vertical proposal — a spacer's
        // `Unbounded` answer is `SPACER_MAX_EXTENT` — while the cross axis
        // still passes the horizontal offer through unchanged.
        let content = crate::layout::measure(
            &tree,
            &mut h.ctx(),
            &mut path,
            SizeProposal {
                horizontal: Proposal::Exact(120.0),
                vertical: Proposal::Unbounded,
            },
        );
        assert_eq!(
            content,
            Size::new(120.0, crate::layout::leaf::SPACER_MAX_EXTENT)
        );
    }

    /// Place `tree` into a `w` x `h` port at `offset` and return the
    /// indicator's placement, if the scroll drew one.
    fn indicator_of(tree: &ViewNode, w: f32, h: f32, offset: f32) -> Option<Placement> {
        let mut harness = Harness::new();
        harness.set_scroll(&format!("/{}", tree.key.as_str()), offset);
        let mut sink = PlacementList::new();
        crate::layout::place(
            tree,
            &mut harness.ctx(),
            &mut KeyPath::root(),
            Slot::new(Rect::new(0.0, 0.0, w, h)),
            &mut sink,
        );
        let placed = sink.as_slice();
        let found = placed
            .iter()
            .position(|p| p.id.ends_with(super::INDICATOR_KEY))?;
        assert_eq!(
            found,
            placed.len() - 1,
            "the indicator is placed after the content"
        );
        Some(placed[found].clone())
    }

    /// The thumb's length is the visible share of the track and its
    /// position the offset's share of the range: 400 of content in a 100
    /// port is a quarter of the 92-unit track (100 less a 4-unit inset at
    /// each end), at the top at offset 0 and at the bottom at the end.
    #[test]
    fn the_indicator_is_the_visible_share_at_the_offsets_share() {
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(400.0));
        let top = indicator_of(&tree, 120.0, 100.0, 0.0).expect("400 in 100 scrolls");
        assert_eq!(top.rect, Rect::new(112.0, 4.0, 4.0, 23.0));
        let end = indicator_of(&tree, 120.0, 100.0, 300.0).expect("still scrolls");
        assert_eq!(end.rect, Rect::new(112.0, 73.0, 4.0, 23.0));
        assert!(
            end.semantics.actions.is_empty(),
            "the thumb takes no input, so no hit test lands on it: wheel and keys scroll the port"
        );
    }

    /// Three pages at half a 300-wide viewport each are 150 wide, whatever
    /// their content measures, so two show and the third pans in. Without
    /// the modifier a page in a horizontal scroll measures its content.
    #[test]
    fn a_container_relative_page_is_a_share_of_the_scroll_viewport() {
        use crate::tree::ContainerRelative;
        let page = |key: &str| {
            ViewNode::new(NodeKind::Spacer, key).with_props(Props {
                container_relative: Some(ContainerRelative {
                    axis: Axis::Horizontal,
                    fraction: 0.5,
                }),
                ..Props::default()
            })
        };
        let row = ViewNode::new(NodeKind::Stack, "pages")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(page("a"))
            .child(page("b"))
            .child(page("c"));
        let tree = ViewNode::new(NodeKind::Scroll, "strip")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(row);
        let mut harness = Harness::new();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut harness.ctx(),
            &mut KeyPath::root(),
            Slot::new(Rect::new(0.0, 0.0, 300.0, 80.0)),
            &mut sink,
        );
        let rect = |tail: &str| {
            sink.as_slice()
                .iter()
                .find(|p| p.id.ends_with(tail))
                .map(|p| p.rect)
                .unwrap_or_else(|| panic!("{tail} placed"))
        };
        assert_eq!(rect("/pages/a").w, 150.0);
        assert_eq!(rect("/pages/b").x, 150.0);
        assert_eq!(
            rect("/pages/c").x,
            300.0,
            "the third page starts past the viewport"
        );
        assert_eq!(rect("/pages").w, 450.0);
    }

    /// Along a horizontal axis the thumb runs on the bottom edge.
    #[test]
    fn a_horizontal_scrolls_indicator_runs_along_its_bottom_edge() {
        let wide = ViewNode::new(NodeKind::Spacer, "content").with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(500.0),
                max: Some(500.0),
                priority: 0,
            },
            vertical: AxisConstraint::default(),
        });
        let tree = ViewNode::new(NodeKind::Scroll, "strip")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(wide);
        let thumb = indicator_of(&tree, 200.0, 50.0, 0.0).expect("500 in 200 scrolls");
        assert_eq!(thumb.rect, Rect::new(4.0, 42.0, 192.0 * 200.0 / 500.0, 4.0));
    }

    /// Content that fits draws no thumb, and neither does a scroll that
    /// hides its indicators.
    #[test]
    fn a_port_that_fits_or_hides_its_indicator_draws_none() {
        let fits = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(80.0));
        assert!(indicator_of(&fits, 120.0, 100.0, 0.0).is_none());
        let hidden = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                indicators: Some(crate::tree::ScrollIndicators::Hidden),
                ..Props::default()
            })
            .child(fixed_extent_child(400.0));
        assert!(indicator_of(&hidden, 120.0, 100.0, 0.0).is_none());
    }

    #[test]
    fn placing_moves_the_child_by_exactly_the_negative_offset_and_clips_to_the_viewport() {
        let mut h = Harness::new();
        h.set_scroll("/list", 40.0);
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
            &mut sink,
        );

        let placements = sink.as_slice();
        // The scroll, its content, and the indicator the 400-unit content
        // in a 100-unit port earns.
        assert_eq!(placements.len(), 3);
        assert_eq!(placements[2].id, "/list/scroll-indicator");
        let child = &placements[1];
        // The thumb owns a 12-unit gutter on the right (4 thick, 4 off the
        // edge, 4 before the content), so the content is 108 wide.
        assert_eq!(
            child.rect,
            Rect::new(0.0, -40.0, 108.0, 400.0),
            "moved by exactly -offset on the scroll axis, full content extent \
             along it, the port less the thumb's gutter across it"
        );
        assert_eq!(
            child.clip,
            Rect::new(0.0, 0.0, 108.0, 100.0),
            "clipped to the viewport short of the thumb's gutter"
        );
        assert!(
            placements[2].rect.x >= child.clip.x + child.clip.w,
            "the thumb draws in its gutter, never over the content: {:?}",
            placements[2].rect
        );
    }

    /// The behaviour this leaf exists to add, pinned by name (gate G4).
    ///
    /// Padding insets the *viewport* a scroll offers/clips its content to —
    /// the content's own extent (the child's fixed 400.0) is untouched, and
    /// so is the scroll's own placed rect (padding is inside the box). Only
    /// what the scroll offers its child — the origin the content starts at,
    /// and the clip it is bounded by — shrinks to the padded interior.
    #[test]
    fn a_padded_scroll_insets_the_viewport_not_the_content_extent() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                padding: Some(InsetRefs::all(gap_token(10.0))),
                ..Props::default()
            })
            .child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
            &mut sink,
        );

        let placements = sink.as_slice();
        assert_eq!(placements.len(), 3, "scroll, content, indicator");
        let scroll = &placements[0];
        let child = &placements[1];

        assert_eq!(
            scroll.rect,
            Rect::new(0.0, 0.0, 120.0, 100.0),
            "padding is inside the box: the scroll's own placed rect never \
             moves or shrinks for its own padding"
        );
        // The right padding (10) counts toward the thumb's 12-unit gutter,
        // so the gutter takes 2 more across.
        assert_eq!(
            child.rect,
            Rect::new(10.0, 10.0, 98.0, 400.0),
            "content keeps its full, unshrunk 400.0 extent, placed from the \
             padded (10.0, 10.0) origin — not the outer (0.0, 0.0) one"
        );
        assert_eq!(
            child.clip,
            Rect::new(10.0, 10.0, 98.0, 80.0),
            "the clip is the padded 98x80 viewport (less the 2 the thumb's \
             gutter needs past the padding), not the outer 120x100 rect"
        );
    }

    /// The scroll gutter decision, both extremes: padding never scrolls away
    /// and never becomes visible content — the clip is identical whether the
    /// list is scrolled to the top or all the way to the bottom, and the
    /// content's near edge always sits flush with the padded rect's edge on
    /// that side. This is the case the module doc warns a wrong design "looks
    /// fine until content is longer than the viewport": a design that instead
    /// clipped to the *outer* rect would let the last row's bottom edge run
    /// past the padded interior and into the bottom gutter once scrolled all
    /// the way down, which this test would catch.
    #[test]
    fn a_padded_scroll_keeps_its_gutter_unbroken_at_both_scroll_extremes() {
        let place_at = |offset: f32| -> (Rect, Rect) {
            let mut h = Harness::new();
            h.set_scroll("/list", offset);
            let tree = ViewNode::new(NodeKind::Scroll, "list")
                .with_props(Props {
                    padding: Some(InsetRefs::all(gap_token(10.0))),
                    ..Props::default()
                })
                .child(fixed_extent_child(400.0));
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            crate::layout::place(
                &tree,
                &mut h.ctx(),
                &mut path,
                Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
                &mut sink,
            );
            let child = &sink.as_slice()[1];
            (child.rect, child.clip)
        };

        // Scrolled to the top (offset 0.0): the content's leading edge sits
        // exactly at the padded origin.
        let (top_rect, top_clip) = place_at(0.0);
        assert_eq!(
            top_rect.y, 10.0,
            "top gutter: content starts at the padded origin"
        );
        assert_eq!(top_clip, Rect::new(10.0, 10.0, 98.0, 80.0));

        // Scrolled to the bottom: max_offset = content 400.0 - viewport 80.0
        // = 320.0. The content's trailing edge lands exactly on the padded
        // rect's bottom edge (10.0 + 80.0 = 90.0), not the outer rect's
        // (100.0) — the bottom gutter is 10.0 wide, same as the top.
        let (bottom_rect, bottom_clip) = place_at(320.0);
        assert_eq!(bottom_rect.y, 10.0 - 320.0);
        assert_eq!(
            bottom_rect.y + bottom_rect.h,
            90.0,
            "content's trailing edge is flush with the padded rect's bottom \
             edge, not the outer rect's"
        );
        assert_eq!(
            bottom_clip, top_clip,
            "the clip never changes with scroll position: the gutter is a \
             fixed strip, not something the content can scroll into"
        );
    }

    /// `measure`'s open-probe (natural size) answer includes the padding,
    /// symmetrically on both axes, on top of whatever the content itself
    /// needs.
    #[test]
    fn a_padded_scrolls_natural_size_includes_its_own_padding() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                padding: Some(InsetRefs::all(gap_token(10.0))),
                ..Props::default()
            })
            .child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let size =
            crate::layout::measure(&tree, &mut h.ctx(), &mut path, SizeProposal::unspecified());
        // Content is 400.0 tall (fixed), 0.0 wide (an unconstrained spacer's
        // ideal is nothing) — plus 10.0 padding on every edge: 20.0 added to
        // each axis.
        assert_eq!(
            size,
            Size::new(20.0, 420.0),
            "the scroll's natural size is its content's extent plus its own padding"
        );
    }

    #[test]
    fn a_stale_offset_past_the_content_end_clamps_instead_of_blanking() {
        let mut h = Harness::new();
        h.set_scroll("/list", 100_000.0);
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(fixed_extent_child(400.0));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 120.0, 100.0)),
            &mut sink,
        );

        let child = &sink.as_slice()[1];
        // max_offset = content_extent(400) - viewport_extent(100) = 300.
        assert_eq!(
            child.rect.y, -300.0,
            "clamped to content_extent - viewport_extent, not the stale offset"
        );
        assert!(child.rect.y.is_finite());
        assert!(child.rect.bottom() > 0.0, "the viewport is not left blank");
    }

    /// SC-008 seed: a 100 000-row collection in a 400-unit viewport with
    /// 24-unit rows must never materialize more than a small, bounded window
    /// of rows, at any scroll position — including deep into the list.
    ///
    /// The window must also *be* at that scroll position. Counting rows is
    /// not enough on its own: a build that ignored the offset entirely and
    /// always drew the first screenful satisfied every bound below, so the
    /// first materialized index is asserted too.
    #[test]
    fn bounded_materialization_holds_at_every_scroll_offset() {
        // (offset, first row index). window_start = offset - 64 overscan,
        // floored into 24-unit rows: 0, floor(4936 / 24) = 205,
        // floor(1_999_936 / 24) = 83_330. The largest offset is still short
        // of the 2_399_600 the content allows, so none of them is clamped.
        for (offset, first_index) in [(0.0_f32, 0_usize), (5_000.0, 205), (2_000_000.0, 83_330)] {
            let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
            h.set_scroll("/list", offset);
            let tree = scroll_with_collection(100_000, 24.0, 64.0);
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            crate::layout::place(
                &tree,
                &mut h.ctx(),
                &mut path,
                Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
                &mut sink,
            );

            assert!(
                h.rows.served < 100,
                "offset {offset}: served {} of 100 000 rows, expected a small bounded window",
                h.rows.served
            );
            assert!(
                h.rows.max_index_seen < 100_000,
                "offset {offset}: touched index {}, which is out of range",
                h.rows.max_index_seen
            );
            let placed: Vec<&str> = sink
                .as_slice()
                .iter()
                .filter_map(|p| p.id.strip_prefix("/list/rows/row-"))
                .collect();
            assert_eq!(
                placed.first().copied(),
                Some(first_index.to_string().as_str()),
                "offset {offset}: the window must sit at the offset, not at the top"
            );
        }
    }

    /// Overscan places extra rows so a later frame can scroll without a
    /// hole. Those rows sit outside the viewport clip: they are in the
    /// frame, and they are not visible. Tab follows [`Placement::is_visible`],
    /// so they are not reachable until the offset moves them in.
    ///
    /// The fixture pins each row to the 28 the collection estimates. It did
    /// not, and could not: a bare `GeneratedRows` row measures one
    /// `MonoContent` line (16). Under the old uniform grid the rows were
    /// still *positioned* 28 apart, so this test saw hidden rows that its own
    /// content never earned. Now that a row lands where its measured extent
    /// puts it, eleven 16-unit rows all fit inside a 224-unit viewport and
    /// nothing is hidden — an honest under-fill, and the reason the fixture
    /// has to state its row height instead of borrowing the estimate's.
    #[test]
    fn overscan_rows_sit_outside_the_viewport_clip() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 100).each_row_tall(28.0),
        );
        let tree = scroll_with_collection(100, 28.0, 64.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        let viewport = Rect::new(0.0, 0.0, 200.0, 224.0);
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(viewport),
            &mut sink,
        );
        let rows: Vec<&Placement> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.contains("/row-"))
            .collect();
        let visible = rows.iter().filter(|p| p.is_visible()).count();
        let hidden = rows.iter().filter(|p| !p.is_visible()).count();
        assert!(
            hidden > 0,
            "overscan of 64 on 28-unit rows must place at least one row \
             past a 224-unit viewport; placed {}",
            rows.len()
        );
        assert!(
            (7..=9).contains(&visible),
            "the viewport holds eight 28-unit rows; saw {visible} visible of {}",
            rows.len()
        );
    }

    #[test]
    fn a_materialized_rows_id_is_stable_across_a_scroll() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        let tree = scroll_with_collection(100_000, 24.0, 64.0);
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        let id = "/list/rows/row-15";

        h.set_scroll("/list", 0.0);
        let mut path = KeyPath::root();
        let mut sink_a = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink_a);
        let a = sink_a
            .as_slice()
            .iter()
            .find(|p| p.id == id)
            .expect("row 15 is in the window at offset 0");

        h.set_scroll("/list", 240.0);
        let mut path = KeyPath::root();
        let mut sink_b = PlacementList::new();
        crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink_b);
        let b = sink_b
            .as_slice()
            .iter()
            .find(|p| p.id == id)
            .expect("row 15 is still in the window at offset 240");

        assert_eq!(a.id, b.id);
        assert_ne!(
            a.rect, b.rect,
            "the same row still moves when the scroll offset changes"
        );
    }

    /// The window is the viewport plus the *declared* overscan, not the
    /// default one.
    ///
    /// The overscan here is deliberately 120.0 rather than 64.0. Every caller
    /// of this fixture used to pass 64.0, which is `DEFAULT_OVERSCAN` — so the
    /// row count came out the same whether the declaration was read or
    /// ignored, and deleting the parameter left every scroll test green.
    /// Verified by sabotage: making `collection_scroll` ignore the enclosing
    /// frame's overscan fails this assertion.
    #[test]
    fn total_count_reaches_semantics_while_placement_count_is_only_the_window() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 100_000).each_row_tall(24.0),
        );
        h.set_scroll("/list", 0.0);
        let tree = scroll_with_collection(100_000, 24.0, 120.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let placements = sink.as_slice();
        let collection = placements
            .iter()
            .find(|p| p.id == "/list/rows")
            .expect("the collection itself is placed");
        assert_eq!(collection.semantics.total_count, Some(100_000));

        // Materialized rows, counted by key. At offset 0 with a
        // 400-unit viewport, the declared 120-unit overscan, and 24-unit rows
        // the window is ceil((400 + 120) / 24) = 22 rows. With the default
        // 64-unit overscan it would be 20, which is what makes this assertion
        // able to tell the two apart.
        let row_count = rows_placed(placements);
        assert_eq!(row_count, 22);
        assert!(row_count < 100_000);
    }

    #[test]
    fn a_row_extent_of_zero_falls_back_to_the_default_rather_than_dividing_by_zero() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 10));
        h.set_scroll("/list", 0.0);
        let tree = scroll_with_collection(10, 0.0, 8.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        // Must not panic, hang, or produce a NaN/infinite rect.
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 100.0, 100.0)),
            &mut sink,
        );
        for placement in sink.as_slice() {
            assert!(placement.rect.x.is_finite());
            assert!(placement.rect.y.is_finite());
            assert!(placement.rect.w.is_finite());
            assert!(placement.rect.h.is_finite());
        }
    }

    /// A `collection` with no `overscan` of its own must take the one its
    /// `scroll` ancestor declares. The window is the viewport plus that
    /// overscan at both ends.
    #[test]
    fn the_scrolls_declared_overscan_reaches_its_collection() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 100_000).each_row_tall(24.0),
        );
        h.set_scroll("/list", 0.0);
        let tree = collection_in_scroll(
            "list",
            Props {
                overscan: Some(120.0),
                ..Props::default()
            },
            100_000,
            24.0,
        );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        // Rows, counted by key. ceil((400 + 120) / 24) = 22
        // with the declared overscan; 20 with the 64-unit default.
        let row_count = rows_placed(sink.as_slice());
        assert_eq!(
            row_count, 22,
            "the window is the viewport plus the scroll's declared overscan, not the default"
        );
    }

    /// The offset that drives a collection is its `scroll` ancestor's, not
    /// whichever ancestor id happens to have an entry in the offset map.
    #[test]
    fn a_stale_offset_on_a_plain_ancestor_does_not_drive_the_collection() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 100_000).each_row_tall(24.0),
        );
        // `/root` is a `stack`. It is not a scroll container, and this entry
        // is stale. The `scroll` between it and the collection has never been
        // scrolled, so it has no entry at all.
        h.set_scroll("/root", 5_000.0);
        let tree = ViewNode::new(NodeKind::Stack, "root").child(collection_in_scroll(
            "list",
            Props::default(),
            100_000,
            24.0,
        ));
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let ids: Vec<&str> = sink.as_slice().iter().map(|p| p.id.as_str()).collect();
        assert!(
            ids.contains(&"/root/list/rows/row-0"),
            "an unscrolled scroll shows its first row; placed: {ids:?}"
        );
        assert!(
            !ids.iter().any(|id| id.starts_with("/root/list/rows/row-2")
                && id.len() > "/root/list/rows/row-2".len()),
            "no row from the stale 5000-unit window is materialized; placed: {ids:?}"
        );
    }

    /// A `collection` lays its rows out along its `scroll` ancestor's axis.
    #[test]
    fn the_scrolls_axis_reaches_its_collection() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 1_000));
        h.set_scroll("/strip", 0.0);
        let tree = collection_in_scroll(
            "strip",
            Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            },
            1_000,
            24.0,
        );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 400.0, 200.0)),
            &mut sink,
        );

        let rows: Vec<Rect> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.starts_with("/strip/rows/row-"))
            .map(|p| p.rect)
            .collect();
        assert!(rows.len() >= 2, "at least two rows are materialized");
        assert_eq!(rows[0].x, 0.0);
        assert_eq!(
            rows[1].x, 24.0,
            "rows advance along the scroll's horizontal axis"
        );
        assert_eq!(rows[0].y, rows[1].y, "rows share the cross-axis origin");
    }

    /// Nested scrolls: the *nearest* one drives the list, not the outermost
    /// one that happens to have an entry in the offset map.
    #[test]
    fn the_nearest_scroll_ancestor_wins_over_a_farther_one() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 100_000).each_row_tall(24.0),
        );
        h.set_scroll("/outer", 0.0);
        let inner = collection_in_scroll(
            "inner",
            Props {
                overscan: Some(0.0),
                ..Props::default()
            },
            100_000,
            24.0,
        );
        let tree = ViewNode::new(NodeKind::Scroll, "outer")
            .with_props(Props {
                overscan: Some(200.0),
                ..Props::default()
            })
            .child(inner);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        // Rows, counted by key. The inner scroll
        // declares no overscan at all, so the window is exactly the 400-unit
        // visible region: ceil(400 / 24) = 17 rows. The outer scroll's
        // 200-unit overscan would make it 25.
        let row_count = rows_placed(sink.as_slice());
        assert_eq!(
            row_count, 17,
            "the inner scroll's overscan is the one in force"
        );
    }

    /// A `collection` outside every `scroll` is legal, and keeps its own
    /// axis and overscan: nothing can scroll it, so nothing else owns them.
    #[test]
    fn a_collection_with_no_scroll_ancestor_keeps_its_own_axis_and_overscan() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 1_000));
        let tree = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(1_000),
            source: Some("fibers".into()),
            estimated_extent: Some(24.0),
            axis: Some(Axis::Horizontal),
            overscan: Some(96.0),
            ..Props::default()
        });
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 400.0, 200.0)),
            &mut sink,
        );

        let rows: Vec<Rect> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.starts_with("/rows/row-"))
            .map(|p| p.rect)
            .collect();
        // ceil((400 + 96) / 24) = 21 rows; the 64-unit default would give 20.
        assert_eq!(rows.len(), 21, "its own overscan is honoured");
        assert_eq!(rows[1].x, 24.0, "its own axis is honoured");
        assert_eq!(rows[0].y, rows[1].y);
    }

    /// A `surface` is anchored in viewport coordinates, so an ancestor
    /// `scroll` is not the scroll context of what is inside it.
    #[test]
    fn a_surface_starts_a_fresh_scroll_context() {
        use crate::tree::{Anchor, Layer};

        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 30).each_row_tall(24.0),
        );
        h.set_scroll("/list", 240.0);
        let collection = ViewNode::new(NodeKind::Collection, "rows").with_props(Props {
            total_count: Some(30),
            source: Some("fibers".into()),
            estimated_extent: Some(24.0),
            overscan: Some(96.0),
            ..Props::default()
        });
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                overscan: Some(200.0),
                ..Props::default()
            })
            .child(
                ViewNode::new(NodeKind::Surface, "popup")
                    .with_props(Props {
                        layer: Some(Layer::Popup),
                        anchor: Some(Anchor::Viewport),
                        ..Props::default()
                    })
                    .child(collection),
            );
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 400.0)),
            &mut sink,
        );

        let ids: Vec<&str> = sink.as_slice().iter().map(|p| p.id.as_str()).collect();
        assert!(
            ids.contains(&"/list/popup/rows/row-0"),
            "the popup is anchored, not scrolled by /list; placed: {ids:?}"
        );
        // Rows, counted by key. The popup is
        // clamped to the 400-unit window, and the collection keeps its own
        // 96-unit overscan: ceil((400 + 96) / 24) = 21 rows. The enclosing
        // scroll's 200-unit overscan would give 25.
        let row_count = rows_placed(sink.as_slice());
        assert_eq!(
            row_count, 21,
            "the enclosing scroll's overscan does not reach inside a surface"
        );
    }

    /// Scroll the whole 100 000-row list past the viewport and place every
    /// frame of it, then read the measurement cache.
    ///
    /// Returns `(entries, hits, misses, evictions)`.
    fn scroll_the_whole_list(capacity: usize) -> (usize, u64, u64, u64) {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 100_000));
        h.cache.set_capacity(capacity);
        let tree = scroll_with_collection(100_000, 24.0, 64.0);
        let rect = Rect::new(0.0, 0.0, 200.0, 400.0);
        // 5 000 frames a viewport apart covers 2 400 000 units, which is the
        // full 100 000 rows at 24 units each: every row is measured once.
        for frame in 0..5_000 {
            h.set_scroll("/list", frame as f32 * 480.0);
            let mut path = KeyPath::root();
            let mut sink = PlacementList::new();
            crate::layout::place(&tree, &mut h.ctx(), &mut path, Slot::new(rect), &mut sink);
        }
        let (hits, misses) = h.cache.stats();
        (h.cache.len(), hits, misses, h.cache.evictions())
    }

    /// SC-008's memory clause: "memory that does not grow with total row
    /// count". The measurement cache is keyed per node id, so before it was
    /// bounded this run left one entry per row ever scrolled past — 100 001
    /// entries for 100 000 rows, climbing by 20 every frame.
    #[test]
    fn scrolling_a_whole_collection_leaves_the_measure_cache_bounded() {
        let capacity = MeasureCache::DEFAULT_CAPACITY;
        let (entries, _, misses, evictions) = scroll_the_whole_list(capacity);
        assert!(
            entries <= capacity,
            "cache holds {entries} entries, over its {capacity}-entry bound"
        );
        assert!(
            misses > 100_000,
            "the run must actually measure the whole list, not a corner of it: \
             {misses} misses"
        );
        assert!(
            evictions > 0,
            "nothing was evicted, so this run never reached the bound and \
             proves nothing"
        );
    }

    /// Bounding the cache must not cost a hit: the working set of one frame,
    /// and of the frame before it, is orders of magnitude below the default
    /// bound, so the LRU only ever drops rows that have left the window.
    ///
    /// The control is the same run with the bound effectively removed. Equal
    /// hit and miss counts is the strongest available statement — not "the
    /// rate is still good", but "eviction changed nothing except memory".
    #[test]
    fn the_default_bound_costs_no_cache_hits() {
        let bounded = scroll_the_whole_list(MeasureCache::DEFAULT_CAPACITY);
        let unbounded = scroll_the_whole_list(usize::MAX);
        assert_eq!(
            (bounded.1, bounded.2),
            (unbounded.1, unbounded.2),
            "bounded (hits, misses) must equal unbounded's"
        );
        assert!(bounded.1 > 0, "a run with no hits would prove nothing");
        assert!(
            bounded.0 <= MeasureCache::DEFAULT_CAPACITY,
            "bounded run holds {} entries",
            bounded.0
        );
        assert!(
            unbounded.0 > 100_000,
            "the control must show the growth this bound removes: {} entries",
            unbounded.0
        );
        assert_eq!(unbounded.3, 0, "the control must not have evicted anything");
    }

    /// A count change moves the ledger's total inside `open`, after the
    /// collection already answered `measure` from `count × estimate`. The
    /// fold has to compare against that answer, or the size every ancestor
    /// cached outlives the frame and a kept cache serves it for good.
    ///
    /// Red before the fold compared against `measured_extent`: the fourth
    /// frame placed the collection at the 22-row estimate (616) while the
    /// ledger held the measured rows.
    #[test]
    fn a_count_change_that_moves_the_ledger_total_reaches_the_next_measure() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 22).each_row_tall(44.0),
        );
        let frame = |h: &mut Harness<MonoContent, GeneratedRows>, tree: &ViewNode| {
            let mut sink = PlacementList::new();
            crate::layout::place(
                tree,
                &mut h.ctx(),
                &mut KeyPath::root(),
                Slot::new(Rect::new(0.0, 0.0, 200.0, 224.0)),
                &mut sink,
            );
            sink.as_slice()
                .iter()
                .find(|p| p.id == "/list/rows")
                .expect("the collection is placed")
                .rect
                .h
        };
        let twenty = scroll_with_collection(20, 28.0, 0.0);
        frame(&mut h, &twenty);
        frame(&mut h, &twenty);
        // What a host's graft names when `total_count` moves.
        let grown = scroll_with_collection(22, 28.0, 0.0);
        h.cache.apply(&crate::layout::ChangeSet::node("/list/rows"));
        frame(&mut h, &grown);
        let kept = frame(&mut h, &grown);
        let ledger = h.extents.list("/list/rows").expect("a ledger").total();
        assert_eq!(
            kept, ledger,
            "the collection kept a size its ledger no longer holds"
        );
    }

    /// The defect the operator found in a screenshot, as a check.
    ///
    /// The inspector's fiber list declared `estimated_extent: 28` for rows
    /// that measure 44 — a stale constant, `list_row`'s vertical padding
    /// having moved from `spacing-02` to `spacing-04`. Every row was drawn 16
    /// units on top of the row above it, and the tall ones ran off the bottom
    /// of the card. Nothing in 130-odd frame-level tests could see it.
    ///
    /// Falsified against the old `main_origin = origin + row_index *
    /// row_extent`, which failed with the actual, observed text
    ///
    /// ```text
    /// /list/rows/row-1 starts at 28 but /list/rows/row-0 already runs to 44: rows overlap by 16
    /// ```
    #[test]
    fn a_row_taller_than_the_estimate_does_not_land_on_top_of_the_row_above_it() {
        let mut h = Harness::with(
            MonoContent::new(),
            GeneratedRows::new("fibers", 20).each_row_tall(44.0),
        );
        // The lie the fiber list was telling: 28 declared, 44 measured.
        let tree = scroll_with_collection(20, 28.0, 0.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 224.0)),
            &mut sink,
        );

        let mut rows: Vec<&Placement> = sink
            .as_slice()
            .iter()
            .filter(|p| p.id.contains("/row-"))
            .collect();
        rows.sort_by(|a, b| a.rect.y.total_cmp(&b.rect.y));
        assert!(
            rows.len() >= 2,
            "need two rows to compare; got {}",
            rows.len()
        );
        for pair in rows.windows(2) {
            let (above, below) = (pair[0], pair[1]);
            let bottom = above.rect.y + above.rect.h;
            assert!(
                below.rect.y >= bottom,
                "{} starts at {} but {} already runs to {}: rows overlap by {}",
                below.id,
                below.rect.y,
                above.id,
                bottom,
                bottom - below.rect.y
            );
        }
        // Not merely disjoint — flush. A list of uniform rows has no gaps
        // either, which is what rules out "place everything a mile apart" as
        // a way to pass the assertion above.
        assert_eq!(rows[1].rect.y - rows[0].rect.y, 44.0);
    }

    /// A row is as wide as the list it sits in, whatever its own text
    /// measured. The collection proposes its cross extent as `exact`, and
    /// `place_collection` used to place the row at the extent it measured
    /// instead — which is why a focus bar, a hover fill and a selected fill
    /// all stopped wherever that row's text stopped.
    ///
    /// Falsified by putting `row_size.across(axis)` back: the assertion
    /// failed with the actual, observed text
    ///
    /// ```text
    /// assertion `left == right` failed: row 0 is 40 wide inside a 200-wide list
    ///   left: 40.0
    ///  right: 200.0
    /// ```
    #[test]
    fn a_row_fills_the_lists_cross_extent_rather_than_hugging_its_own_text() {
        let mut h = Harness::with(MonoContent::new(), GeneratedRows::new("fibers", 5));
        let tree = scroll_with_collection(5, 16.0, 0.0);
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &tree,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 200.0, 224.0)),
            &mut sink,
        );

        let row = sink
            .as_slice()
            .iter()
            .find(|p| p.id.ends_with("/row-0"))
            .expect("row 0 was materialized");
        assert_eq!(
            row.rect.w, 200.0,
            "row 0 is {} wide inside a 200-wide list",
            row.rect.w
        );
    }

    /// The scroll's cross axis is an exact offer, so the content is as wide
    /// as the port, not as wide as its own widest line.
    ///
    /// The defect: the inspector's fiber-detail pane held 598 units of
    /// content in an 837-unit column and was placed at 598, leaving a
    /// 239-unit step down the right edge of the window against a tab strip
    /// that filled. Every other scroll in the tree hid it, because scrolling
    /// content is usually wider than its port.
    ///
    /// Falsified by putting `content_size.across(axis)` back on its own:
    ///
    /// ```text
    /// the content was placed 120 wide in a 300-wide scrollport: it hugged
    /// its own widest line instead of filling the port it was offered
    /// ```
    #[test]
    fn content_narrower_than_the_scrollport_still_fills_it() {
        let narrow = ViewNode::new(NodeKind::Spacer, "narrow").with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(120.0),
                max: Some(120.0),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(80.0),
                max: Some(80.0),
                priority: 0,
            },
        });
        let scroll = ViewNode::new(NodeKind::Scroll, "port")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(narrow);
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &scroll,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 300.0, 400.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        let content = placed
            .iter()
            .find(|p| p.id.ends_with("/narrow"))
            .expect("the content was placed");
        assert_eq!(
            content.rect.w, 300.0,
            "the content was placed {} wide in a 300-wide scrollport: it hugged its own widest \
             line instead of filling the port it was offered",
            content.rect.w
        );
    }

    /// The other half: content genuinely wider than its port keeps its
    /// overflow, so `max` and not a substitution.
    #[test]
    fn content_wider_than_the_scrollport_keeps_its_overflow() {
        let wide = ViewNode::new(NodeKind::Spacer, "wide").with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(500.0),
                max: Some(500.0),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(80.0),
                max: Some(80.0),
                priority: 0,
            },
        });
        let scroll = ViewNode::new(NodeKind::Scroll, "port")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(wide);
        let mut h = Harness::new();
        let mut path = KeyPath::root();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &scroll,
            &mut h.ctx(),
            &mut path,
            Slot::new(Rect::new(0.0, 0.0, 300.0, 400.0)),
            &mut sink,
        );
        let placed = sink.as_slice();
        let content = placed
            .iter()
            .find(|p| p.id.ends_with("/wide"))
            .expect("the content was placed");
        assert_eq!(content.rect.w, 500.0, "the overflow was clamped away");
    }

    #[test]
    fn axis_rect_keeps_the_cross_axis_origin_from_base() {
        let base = Rect::new(5.0, 7.0, 30.0, 40.0);
        let horizontal = super::axis_rect(base, Axis::Horizontal, 100.0, 20.0, 9.0);
        assert_eq!(horizontal, Rect::new(100.0, 7.0, 20.0, 9.0));
        let vertical = super::axis_rect(base, Axis::Vertical, 100.0, 20.0, 9.0);
        assert_eq!(vertical, Rect::new(5.0, 100.0, 9.0, 20.0));
    }

    /// A spacer pinned at `w` by `h`.
    fn sized(key: &str, w: f32, h: f32) -> ViewNode {
        ViewNode::new(NodeKind::Spacer, key).with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(w),
                max: Some(w),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(h),
                max: Some(h),
                priority: 0,
            },
        })
    }

    /// A two-axis scroll `code` over a row of a 30-wide pinned `gutter` and
    /// a 500-wide `text`, both 400 tall: 530 by 400 of content.
    fn two_axis() -> ViewNode {
        let mut gutter = sized("gutter", 30.0, 400.0).interactive(
            crate::tree::Role::Button,
            "gutter",
            &[crate::tree::Interaction::Click],
        );
        gutter.props.pinned = Some(true);
        gutter.props.z = Some(1);
        let row = ViewNode::new(NodeKind::Stack, "row")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(gutter)
            .child(sized("text", 500.0, 400.0));
        ViewNode::new(NodeKind::Scroll, "code")
            .with_props(Props {
                cross_scroll: Some(true),
                ..Props::default()
            })
            .child(row)
    }

    /// Place [`two_axis`] in a 200 by 100 port at the two offsets.
    fn place_two_axis(main: f32, cross: f32) -> Vec<Placement> {
        let mut h = Harness::new();
        h.set_scroll("/code", main);
        h.state.cross_offsets.insert("/code".to_owned(), cross);
        let mut sink = PlacementList::new();
        crate::layout::place(
            &two_axis(),
            &mut h.ctx(),
            &mut KeyPath::root(),
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        sink.as_slice().to_vec()
    }

    fn rect_of(placed: &[Placement], id: &str) -> Rect {
        placed
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("{id} is placed"))
            .rect
    }

    /// The content moves by both offsets, each clamped to its own overflow
    /// in the port the two thumb gutters leave (530 - 188 across, 400 - 88
    /// along), and the scroll's placement names the cross axis so the frame
    /// does not have to guess it from geometry.
    #[test]
    fn a_two_axis_scroll_offsets_and_clamps_both_axes() {
        let placed = place_two_axis(40.0, 150.0);
        assert_eq!(
            rect_of(&placed, "/code/row"),
            Rect::new(-150.0, -40.0, 530.0, 400.0)
        );
        let clamped = place_two_axis(9_000.0, 9_000.0);
        assert_eq!(
            rect_of(&clamped, "/code/row"),
            Rect::new(-342.0, -312.0, 530.0, 400.0)
        );
        let scroll = placed.iter().find(|p| p.id == "/code").expect("scroll");
        assert_eq!(scroll.semantics.cross_axis, Some(Axis::Horizontal));
    }

    /// Both thumbs, with the same drawing: the main one on the right edge,
    /// the cross one on the bottom, its length the visible share 188 / 530
    /// of a track that stops short of the main thumb's gutter. Each thumb
    /// sits in its own gutter, outside the content's clip, and the two do
    /// not cross in the corner. A one-axis scroll draws no cross thumb
    /// however wide its content.
    #[test]
    fn a_two_axis_scroll_draws_a_thumb_on_each_axis() {
        let placed = place_two_axis(0.0, 0.0);
        let thumb = |key: &str| {
            placed
                .iter()
                .find(|p| p.id == format!("/code/{key}"))
                .cloned()
        };
        let main = thumb(super::INDICATOR_KEY).expect("the main thumb");
        let cross = thumb(super::CROSS_INDICATOR_KEY).expect("the cross thumb");
        assert!(main.rect.x > 190.0 && main.rect.h < 100.0);
        assert!(cross.rect.y > 90.0, "along the bottom: {:?}", cross.rect);
        assert!(
            (cross.rect.w - 180.0 * 188.0 / 530.0).abs() < 0.01,
            "{:?}",
            cross.rect
        );
        let row = placed.iter().find(|p| p.id == "/code/row").expect("row");
        assert!(main.rect.x >= row.clip.x + row.clip.w, "{:?}", row.clip);
        assert!(cross.rect.y >= row.clip.y + row.clip.h, "{:?}", row.clip);
        assert!(
            main.rect.y + main.rect.h <= 88.0 && cross.rect.x + cross.rect.w <= 188.0,
            "the thumbs stop short of each other's gutter: {:?} {:?}",
            main.rect,
            cross.rect
        );
        let mut one_axis = two_axis();
        one_axis.props.cross_scroll = None;
        let mut h = Harness::new();
        let mut sink = PlacementList::new();
        crate::layout::place(
            &one_axis,
            &mut h.ctx(),
            &mut KeyPath::root(),
            Slot::new(Rect::new(0.0, 0.0, 200.0, 100.0)),
            &mut sink,
        );
        assert!(
            !sink
                .as_slice()
                .iter()
                .any(|p| p.id.ends_with(super::CROSS_INDICATOR_KEY))
        );
    }

    /// A pinned child stays at the port's leading edge while its siblings
    /// move under it, and only once scrolling would carry it past the edge.
    #[test]
    fn a_pinned_child_stays_at_the_leading_edge() {
        let at_rest = place_two_axis(0.0, 0.0);
        assert_eq!(rect_of(&at_rest, "/code/row/gutter").x, 0.0);
        let scrolled = place_two_axis(0.0, 150.0);
        assert_eq!(
            rect_of(&scrolled, "/code/row/gutter"),
            Rect::new(0.0, 0.0, 30.0, 400.0)
        );
        assert_eq!(rect_of(&scrolled, "/code/row/text").x, 30.0 - 150.0);
    }

    /// The pin is real geometry: a press over the gutter at a cross offset
    /// hits the gutter, not the text that scrolled under it.
    #[test]
    fn a_press_at_an_offset_hits_the_pinned_child() {
        use crate::frame::{TransitionActivity, Viewport, petrify};
        use crate::geom::Point;
        use crate::input::hit_test;
        use crate::testing::validated;
        use crate::token::ThemeMode;
        use crate::tree::Interaction;

        let mut h = Harness::new();
        h.state.cross_offsets.insert("/code".to_owned(), 150.0);
        let tree = two_axis();
        let frame = petrify(
            1,
            validated(&tree),
            &mut h.ctx(),
            Viewport::new(Size::new(200.0, 100.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        let hit = hit_test(&frame, Point::new(10.0, 50.0), Interaction::Click);
        assert_eq!(hit.map(|p| p.id.as_str()), Some("/code/row/gutter"));
        assert_eq!(frame.scroll_axis("/code"), Axis::Vertical);
        assert_eq!(frame.scroll_overflow("/code", Axis::Horizontal), 342.0);
    }

    /// Place `tree` in a `w` by `h` port at main offset `offset` and return
    /// every placement.
    fn placed_at(tree: &ViewNode, w: f32, h: f32, offset: f32) -> Vec<Placement> {
        let mut harness = Harness::new();
        harness.set_scroll(&format!("/{}", tree.key.as_str()), offset);
        let mut sink = PlacementList::new();
        crate::layout::place(
            tree,
            &mut harness.ctx(),
            &mut KeyPath::root(),
            Slot::new(Rect::new(0.0, 0.0, w, h)),
            &mut sink,
        );
        sink.as_slice().to_vec()
    }

    /// An open offer across a scroll whose content overflows the exact
    /// offer along it answers the thumb's gutter too, so the content is
    /// placed as wide as it measured and the thumb sits beside it.
    #[test]
    fn an_overflowing_scroll_answers_its_thumb_gutter_across() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list").child(sized("body", 80.0, 400.0));
        let open_across = SizeProposal {
            horizontal: Proposal::Unspecified,
            vertical: Proposal::Exact(100.0),
        };
        let size = crate::layout::measure(&tree, &mut h.ctx(), &mut KeyPath::root(), open_across);
        assert_eq!(
            size,
            Size::new(92.0, 100.0),
            "80 of content and a 12-unit gutter"
        );
        let fits = SizeProposal {
            horizontal: Proposal::Unspecified,
            vertical: Proposal::Exact(500.0),
        };
        let size = crate::layout::measure(&tree, &mut h.ctx(), &mut KeyPath::root(), fits);
        assert_eq!(
            size.w, 80.0,
            "content that fits draws no thumb and needs no gutter"
        );
    }

    /// A one-axis scroll whose content cannot narrow by the gutter (a strip
    /// as tall as its port) keeps the whole port and its thumb sits over
    /// the content, so nothing is cut off.
    #[test]
    fn a_gutter_the_content_cannot_give_is_not_taken() {
        let tree = ViewNode::new(NodeKind::Scroll, "strip")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                ..Props::default()
            })
            .child(sized("tabs", 400.0, 40.0));
        let placed = placed_at(&tree, 120.0, 40.0, 0.0);
        let tabs = placed.iter().find(|p| p.id == "/strip/tabs").expect("tabs");
        assert_eq!(tabs.clip, Rect::new(0.0, 0.0, 120.0, 40.0));
        assert!(placed.iter().any(|p| p.id.ends_with(super::INDICATOR_KEY)));
    }

    /// A `Buttons` scroll draws no thumb and a chevron button at each end
    /// with content past it: only forward at the start, both in the middle,
    /// only back at the end, none when the content fits. Each is a button
    /// in the semantic tree that takes no focus.
    #[test]
    fn a_buttons_scroll_shows_a_button_at_each_end_with_content_past_it() {
        let tree = |extent: f32| {
            ViewNode::new(NodeKind::Scroll, "strip")
                .with_props(Props {
                    axis: Some(Axis::Horizontal),
                    indicators: Some(crate::tree::ScrollIndicators::Buttons),
                    ..Props::default()
                })
                .child(sized("tabs", extent, 40.0))
        };
        let ends = |placed: &[Placement]| {
            let has = |key: &str| placed.iter().any(|p| p.id == format!("/strip/{key}"));
            (has(super::NAV_BACK_KEY), has(super::NAV_FORWARD_KEY))
        };
        let start = placed_at(&tree(400.0), 200.0, 40.0, 0.0);
        assert_eq!(ends(&start), (false, true));
        assert!(!start.iter().any(|p| p.id.ends_with(super::INDICATOR_KEY)));
        let forward = start
            .iter()
            .find(|p| p.id == "/strip/scroll-forward")
            .expect("forward");
        assert_eq!(forward.rect, Rect::new(160.0, 0.0, 40.0, 40.0));
        assert_eq!(forward.semantics.role, Some(crate::tree::Role::Button));
        assert!(
            !forward
                .semantics
                .actions
                .contains(&crate::tree::Interaction::Focus),
            "Carbon's overflow buttons are not Tab stops"
        );
        assert_eq!(
            ends(&placed_at(&tree(400.0), 200.0, 40.0, 100.0)),
            (true, true)
        );
        assert_eq!(
            ends(&placed_at(&tree(400.0), 200.0, 40.0, 200.0)),
            (true, false)
        );
        assert_eq!(
            ends(&placed_at(&tree(150.0), 200.0, 40.0, 0.0)),
            (false, false)
        );
    }

    /// A press on an overflow button asks for one page: the port less the
    /// button, back or forward.
    #[test]
    fn an_overflow_button_steps_its_scroll_by_a_page() {
        use crate::frame::{TransitionActivity, Viewport, petrify};
        use crate::testing::validated;
        use crate::token::ThemeMode;
        let tree = ViewNode::new(NodeKind::Scroll, "strip")
            .with_props(Props {
                axis: Some(Axis::Horizontal),
                indicators: Some(crate::tree::ScrollIndicators::Buttons),
                ..Props::default()
            })
            .child(sized("tabs", 400.0, 40.0));
        let mut h = Harness::new();
        h.set_scroll("/strip", 100.0);
        let frame = petrify(
            1,
            validated(&tree),
            &mut h.ctx(),
            Viewport::new(Size::new(200.0, 40.0), ThemeMode::Dark),
            TransitionActivity::default(),
        );
        assert_eq!(
            super::nav_step(&frame, "/strip/scroll-forward"),
            Some(("/strip".to_owned(), 160.0))
        );
        assert_eq!(
            super::nav_step(&frame, "/strip/scroll-back"),
            Some(("/strip".to_owned(), -160.0))
        );
        assert_eq!(super::nav_step(&frame, "/strip/tabs"), None);
    }

    /// A hugging scroll offered more than its content takes only its
    /// content; offered less, it takes the offer and scrolls.
    #[test]
    fn a_hugging_scroll_takes_no_more_than_its_content() {
        let mut h = Harness::new();
        let tree = ViewNode::new(NodeKind::Scroll, "list")
            .with_props(Props {
                hug_content: Some(true),
                ..Props::default()
            })
            .child(sized("body", 80.0, 120.0));
        let offer = |v: f32| SizeProposal {
            horizontal: Proposal::Exact(200.0),
            vertical: Proposal::Exact(v),
        };
        let hug = crate::layout::measure(&tree, &mut h.ctx(), &mut KeyPath::root(), offer(500.0));
        assert_eq!(hug.h, 120.0);
        let short = crate::layout::measure(&tree, &mut h.ctx(), &mut KeyPath::root(), offer(60.0));
        assert_eq!(short.h, 60.0);
        let mut plain = tree.clone();
        plain.props.hug_content = None;
        // A fresh harness: the cache keys a measurement by node id and offer.
        let mut h = Harness::new();
        let size = crate::layout::measure(&plain, &mut h.ctx(), &mut KeyPath::root(), offer(500.0));
        assert_eq!(size.h, 500.0, "a scroll takes all it is offered by default");
    }
}
