//! Kind-specific node parameters.
//!
//! Three classes of parameter live in the one bag below, and they are typed
//! differently on purpose (`contracts/view-tree.md` §"Styling parameters are
//! token references", FR-053):
//!
//! * **Styling** — `spacing`, `column_spacing`, `row_spacing`, `padding`,
//!   `style` (typography), and `tokens` (colour and corner radius, by slot).
//!   These are *taste*, and taste belongs to the theme, so they carry
//!   [`TokenName`]s and resolve through a [`ThemeSnapshot`]. An author cannot
//!   write `spacing: 7`, because 7 is not a step anybody chose — and, since
//!   the field itself is `Option<TokenName>` rather than `Option<String>`,
//!   an author cannot write `style: "#3a3a3a"` either: [`TokenName::new`]
//!   refuses anything that parses as a colour or a number before a `Props`
//!   can even deserialize. FR-053 names five styling families — spacing,
//!   padding, corner radius, typography, colour — and this is the last of
//!   them landing: `spacing`/`padding` were token-typed first, `style` and
//!   `tokens` (which carries `radius` alongside `background`/`border`/
//!   `foreground`) close the set.
//! * **Layout declaration** — `constraints`, track sizes, `align`, `axis`,
//!   `wrap`, `max_lines`, `span`. These stay numeric. A minimum width is a
//!   fact about a label, not a taste decision: no theme can make a
//!   three-digit counter fit in two digits' worth of room.
//! * **Performance hint** — `overscan`, `estimated_extent`, `total_count`.
//!   These stay numeric too. `overscan` changes no pixel; it changes how much
//!   is materialized off-screen, and it belongs to no theme.
//!
//! `props` is deliberately one flat bag of optional plain values rather than a
//! Rust enum: the wire form must be the shape a Lua table produces
//! (`contracts/view-tree.md`, FR-011), and a table has no tag. Coherence
//! between `kind` and `props` is enforced once, at tree acceptance
//! ([`crate::tree::validate`]), so container code reads through the typed
//! views below and never re-checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// `Align` is deliberately *not* imported: this module declares an `Align` of
// its own — the anchor alignment in [`Anchor::Node`] — and the two are
// different vocabularies over the same word. `crate::geom::Align` is where a
// child sits inside the space its parent gives it (and has a `Stretch` a
// point on an anchor's edge cannot have); [`Align`] here is where a surface
// sits along the edge of the node it is anchored to. Every use of the former
// in this file is spelled out in full so a reader never has to guess which
// one is meant.
use crate::geom::{Axis, Insets};
use crate::token::{ThemeSnapshot, TokenName};

/// How a text node handles content it cannot fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextWrap {
    /// Wrap at word boundaries, then clip.
    #[default]
    Wrap,
    /// Single line, ellipsis at the truncation point.
    Ellipsis,
    /// Single line, hard clip with no marker.
    Clip,
}

impl TextWrap {
    /// The policy's wire name.
    ///
    /// The frame digest hashes this string (`contracts/frame-identity.md` §3),
    /// so it is part of a serialization format, not a debug label: changing a
    /// name here changes every digest and needs the domain-prefix bump that
    /// [`crate::frame::digest::DOMAIN`] documents. The names are the same ones
    /// serde emits, and a test below pins that.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wrap => "wrap",
            Self::Ellipsis => "ellipsis",
            Self::Clip => "clip",
        }
    }
}

/// How one grid track is sized.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum TrackSize {
    /// A fixed logical extent.
    Fixed {
        /// The extent.
        value: f32,
    },
    /// A share of the leftover space, proportional to `weight`.
    Weight {
        /// Relative share. Non-positive weights are a tree-acceptance error.
        weight: f32,
    },
    /// The track's own content extent, probed with `Unspecified`.
    FitContent,
}

/// How many contiguous grid tracks one child covers on each axis.
///
/// A **count**, never a range: the author says how many tracks the child
/// covers, and the grid decides where the run starts (FR-061). No cell address
/// appears in the tree, so no tree can name a cell that does not exist, and
/// re-ordering children cannot leave a stale coordinate behind. Both counts
/// default to one, which is exactly the single cell every child occupied
/// before spanning existed — a tree that declares no span is seated,
/// measured, and digested identically to one written before this type.
///
/// A count of zero, or one larger than the axis has tracks, is a
/// tree-acceptance violation rather than a clamp
/// ([`crate::tree::Violation::GridSpanOutOfRange`]): a silently shortened span
/// puts a child somewhere the author did not write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridSpan {
    /// Column tracks covered, counting the child's own column as the first.
    pub columns: usize,
    /// Row tracks covered, counting the child's own row as the first.
    pub rows: usize,
}

impl Default for GridSpan {
    /// One track on each axis — one cell, the pre-FR-061 behaviour.
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
        }
    }
}

impl GridSpan {
    /// The one-cell span. Named so layout can state the default rather than
    /// spell `Self { columns: 1, rows: 1 }` at each site.
    pub const ONE: Self = Self {
        columns: 1,
        rows: 1,
    };

    /// Track count on `axis`, floored at one.
    ///
    /// The floor is a defensive one, not a policy: tree acceptance refuses a
    /// zero count before layout ever runs, and [`crate::frame::petrify`] takes
    /// only a [`crate::tree::ValidatedTree`]. It exists so a container called
    /// out of turn divides by a span width of one rather than of zero.
    #[must_use]
    pub fn on(self, axis: Axis) -> usize {
        match axis {
            Axis::Horizontal => self.columns,
            Axis::Vertical => self.rows,
        }
        .max(1)
    }
}

/// The most row tracks a grid can have before any child spans: the rows it
/// declared, or one per child, whichever is more.
///
/// A **ceiling**, not a count. The grid's real row count comes from its
/// seating, and a span can push that past this number — rows have always grown
/// to fit content, and a spanning child is content like any other. Deriving
/// the count instead (`child_count.div_ceil(ncols)`) is wrong the moment a
/// span exists, because children and cells stop being the same quantity: four
/// children in four columns occupy five cells if one of them spans two, so the
/// last child sits in row 1 and `4.div_ceil(4)` still answers one row.
///
/// What it is for is bounding a declared row span. Rows grow, so no positive
/// row span can ever "run past the last row" the way an over-wide column span
/// runs past the last column — the grid would just grow another row. A run
/// longer than this covers rows nothing in the tree asks for, and without the
/// bound a `usize` typo in one `span.rows` would materialize every one of
/// them.
///
/// `ncols` is deliberately not a parameter: one row per child is the ceiling
/// whatever the width, and dividing by the width is the exact mistake
/// described above.
pub(crate) fn max_row_tracks(declared_rows: usize, child_count: usize) -> usize {
    declared_rows.max(child_count)
}

/// Content insets named one edge at a time, as token references.
///
/// [`Insets`] one level up: same four edges, same `all`/`symmetric`
/// constructors, same "an absent edge is no inset" reading. The difference is
/// what an edge holds — a name here, a number there — and which side of
/// resolution it lives on. `InsetRefs` is what an author writes;
/// [`Insets`] is what a container measures with, and
/// [`Props::padding`] is the one step between them.
///
/// The two types are kept separate rather than made generic over the edge
/// type. A generic would let a resolved `Insets<TokenName>` and an authored
/// `Insets<f32>` be spelled the same way at a call site, and the whole point
/// of FR-053 is that those are different things: one has been through a
/// theme and one has not.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InsetRefs {
    /// Top edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<TokenName>,
    /// Right edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<TokenName>,
    /// Bottom edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<TokenName>,
    /// Left edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<TokenName>,
}

impl InsetRefs {
    /// The same token on all four edges — [`Insets::all`]'s shape.
    #[must_use]
    pub fn all(name: TokenName) -> Self {
        Self {
            top: Some(name.clone()),
            right: Some(name.clone()),
            bottom: Some(name.clone()),
            left: Some(name),
        }
    }

    /// A horizontal token on left/right and a vertical one on top/bottom —
    /// [`Insets::symmetric`]'s shape, and the argument order goes with it
    /// (horizontal first).
    #[must_use]
    pub fn symmetric(horizontal: TokenName, vertical: TokenName) -> Self {
        Self {
            top: Some(vertical.clone()),
            right: Some(horizontal.clone()),
            bottom: Some(vertical),
            left: Some(horizontal),
        }
    }
}

/// Which surface layer an overlay lives on. Higher layers paint later.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// Anchored popup (menu, completion list).
    Popup,
    /// Transient message, does not take focus.
    Toast,
    /// Focus-trapping dialog.
    Modal,
    /// A frame-wide seat above everything (drag ghost, command palette scrim).
    FrameWide,
}

impl Layer {
    /// The base z-order for this layer. Nodes inside a layer order by
    /// `props.z`, then by child order.
    #[must_use]
    pub fn base_z(self) -> i32 {
        match self {
            Self::Popup => 1_000,
            Self::Toast => 2_000,
            Self::Modal => 3_000,
            Self::FrameWide => 4_000,
        }
    }
}

/// Where a surface sits along the edge of the node it is anchored to.
///
/// The cross-axis half of an anchored placement: [`Edge`] picks which side of
/// the anchor the surface prefers, and this picks where along that side it
/// starts. The pair spells out all twelve placements Carbon names, which is
/// why Carbon's eight deprecated combined aliases are not ported
/// (`contracts/anchored-placement.md` §3).
///
/// **Not [`crate::geom::Align`]**, which is the cross-axis placement of a
/// child inside the space its parent gives it. Two differences make them two
/// types rather than one: this one defaults to [`Align::Center`] (a caret
/// under the middle of a button is the shape every popover library ships),
/// while a child in a stack defaults to `Start`; and `Stretch` is meaningless
/// here, because a surface is sized by its own content and has no track to
/// fill.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    /// Flush with the anchor's leading corner on the cross axis.
    Start,
    /// Centred on the anchor's cross-axis midpoint.
    #[default]
    Center,
    /// Flush with the anchor's trailing corner on the cross axis.
    End,
}

impl Align {
    /// This alignment's wire name, the same spelling `serde` reads and writes.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }

    /// The leading cross-axis coordinate for a surface of `surface` extent
    /// aligned against an anchor of `anchor` extent starting at `at`.
    ///
    /// Unlike [`crate::geom::Align::offset`], the result is not floored at
    /// the anchor's leading edge: a surface wider than its anchor legitimately
    /// overhangs it on both sides when centred, and clamping that back to the
    /// anchor's own corner would silently turn every `Center` on a narrow
    /// button into a `Start`.
    #[must_use]
    pub fn leading(self, at: f32, anchor: f32, surface: f32) -> f32 {
        match self {
            Self::Start => at,
            Self::Center => at + (anchor - surface) / 2.0,
            Self::End => at + anchor - surface,
        }
    }
}

/// Where an overlay surface attaches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum Anchor {
    /// Attached to another node's rect, by that node's key-path id.
    Node {
        /// Semantic id of the anchor node.
        id: String,
        /// Which edge of the anchor the surface prefers.
        edge: Edge,
        /// Where along that edge the surface starts.
        ///
        /// Defaulted so every `{ id, edge }` tree written before this field
        /// existed parses unchanged and resolves identically — centred, the
        /// reading those trees already had.
        #[serde(default, skip_serializing_if = "is_default_align")]
        align: Align,
        /// Gap between the anchor's edge and the surface, as a spacing token.
        ///
        /// A token reference and never a literal, for the reason
        /// [`Props::spacing`] is one (FR-053): the distance between a button
        /// and its menu is a taste decision, and taste is the theme's. Absent
        /// means no gap, which is what absence meant before this field
        /// existed.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<TokenName>,
    },
    /// Attached to a point in the viewport.
    Point {
        /// Horizontal position, logical units.
        x: f32,
        /// Vertical position, logical units.
        y: f32,
    },
    /// Centred in the viewport.
    Viewport,
}

/// Whether an [`Align`] is the one absence already meant.
///
/// Only for `skip_serializing_if`: a tree that never mentioned `align` must
/// round-trip through JSON without growing a key, or "every existing tree
/// parses and resolves identically" would hold on the way in and fail on the
/// way out.
fn is_default_align(align: &Align) -> bool {
    *align == Align::default()
}

/// Preferred side of an anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    /// Above the anchor.
    Top,
    /// Below the anchor.
    Bottom,
    /// Left of the anchor.
    Left,
    /// Right of the anchor.
    Right,
}

impl Edge {
    /// This edge's wire name, the same spelling `serde` reads and writes.
    ///
    /// Also what the frame digest hashes for a caret's resolved side
    /// (`crate::frame::digest::hash_paint_content`): a name rather than a
    /// discriminant, so reordering this enum can never silently rewrite a
    /// published digest.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    /// The side across the anchor from this one.
    ///
    /// Total by construction — every edge has exactly one opposite — which
    /// is what `contracts/anchored-placement.md` §4 step 2 requires of the
    /// flip: there is no edge the ladder can fail to find a second side for.
    #[must_use]
    pub fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// The axis this edge is a side of: the axis a surface anchored to it
    /// grows away from the anchor along, and the axis a flip reverses.
    #[must_use]
    pub fn axis(self) -> Axis {
        match self {
            Self::Top | Self::Bottom => Axis::Vertical,
            Self::Left | Self::Right => Axis::Horizontal,
        }
    }
}

/// What an overlay does when its preferred placement does not fit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClampRule {
    /// Try the opposite edge, then shrink.
    #[default]
    Flip,
    /// Keep the edge, reduce the extent to fit.
    Shrink,
    /// Keep the extent, make the surface scroll.
    Scroll,
}

/// How an overlay treats input aimed past it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputPolicy {
    /// Swallow everything outside the surface; focus is trapped inside.
    Block,
    /// Let input through to what is underneath.
    Passthrough,
    /// Let input through, and close on the first outside press.
    #[default]
    DismissOutside,
}

/// Every kind-specific parameter a built-in node can carry.
///
/// Absent fields serialize away, so the JSON of a plain stack is three keys,
/// not thirty.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Props {
    /// Main axis for `stack`, scrolling axis for `scroll`, run direction for
    /// `separator`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<Axis>,
    /// Gap between stack children, reserved before distribution.
    ///
    /// A token reference (FR-053): the gap between two controls is a taste
    /// decision, and taste is the theme's. Absent still means
    /// [`DEFAULT_SPACING`] — token-typing changes what an author may *say*,
    /// not what silence means.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<TokenName>,
    /// Cross-axis alignment of children.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<crate::geom::Align>,
    /// Grid column tracks, leading to trailing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<TrackSize>,
    /// Grid row tracks, leading to trailing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<TrackSize>,
    /// Gap between grid columns. A token reference, like [`Props::spacing`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_spacing: Option<TokenName>,
    /// Gap between grid rows. A token reference, like [`Props::spacing`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_spacing: Option<TokenName>,
    /// How many grid tracks this node covers on each axis.
    ///
    /// Declared on the **child**, read by the `grid` parent: it is the child
    /// that knows it is a week-long event or a full-width header. Absent is
    /// the one-cell span every child had before FR-061, and it serializes
    /// away, so no tree written before spanning existed changes shape.
    /// Declaring it under a parent that is not a `grid` is a violation
    /// ([`crate::tree::Violation::SpanOutsideGrid`]) rather than a silently
    /// ignored field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<GridSpan>,
    /// Extra logical extent materialized past each end of a scroll viewport.
    ///
    /// Declared on the **`scroll`**, which is what has a viewport and an
    /// offset. A `collection` inside one reads its ancestor's value off the
    /// scroll context the walk carries (`layout::ScrollFrame`), and tree
    /// acceptance refuses `overscan` on a `collection` that has a `scroll`
    /// ancestor rather than silently ignoring it
    /// ([`crate::tree::Violation::ScrollParamOwnedByAncestor`]).
    ///
    /// A `collection` with no `scroll` ancestor keeps its own: nothing can
    /// scroll it, so nothing else can own the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overscan: Option<f32>,
    /// Total row count of a `collection`, including unmaterialized rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_count: Option<usize>,
    /// Name of the store-side row source a `collection` reads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Estimated main-axis extent of one collection row, used to size the
    /// scroll range before rows are materialized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_extent: Option<f32>,
    /// Text content for `text` and initial content for `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Truncation policy for `text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<TextWrap>,
    /// Maximum rendered lines before truncation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<usize>,
    /// Typography token reference, or `None` for the theme's body style.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<TokenName>,
    /// Image source identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Placeholder text for `input`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Surface layer for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    /// Anchor for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Anchor>,
    /// Off-screen rule for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clamp: Option<ClampRule>,
    /// Outside-input rule for `surface`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_policy: Option<InputPolicy>,
    /// The picture a `canvas` draws.
    ///
    /// Behind an `Arc` for the same reason [`crate::tree::ViewNode::children`]
    /// is: a canvas that did not change hands the same allocation back every
    /// frame, and a list of a few thousand commands cloned per frame would
    /// undo what the tree's own sharing buys.
    ///
    /// The list is checked once, at construction
    /// ([`crate::draw::DrawList::new`]), against every bound in
    /// `contracts/draw-list.md` §5 — so a `Props` that holds one holds a list
    /// that has already passed. Declaring it on any kind other than `canvas`
    /// paints nothing: `crate::layout::paint_content_of` reads it for
    /// `NodeKind::Canvas` alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas: Option<std::sync::Arc<crate::draw::DrawList>>,
    /// Registered painter name for `custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_kind: Option<String>,
    /// Explicit z-order within the node's layer. Ties break on child order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<i32>,
    /// Paint opacity in `[0, 1]`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f32>,
    /// Content insets from this node's own edges. Honoured only by container
    /// kinds (`NodeKind::is_container`) — a leaf has no children to inset,
    /// and `padding` on a leaf is a tree-acceptance violation rather than a
    /// silently ignored declaration
    /// ([`crate::tree::Violation::PaddingOnLeafKind`]).
    ///
    /// Token references, one per edge ([`InsetRefs`]), for the same reason
    /// [`Props::spacing`] is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<InsetRefs>,
    /// Token references by paint slot (`background`, `foreground`, `border`,
    /// `radius`, …).
    ///
    /// The map *key* is a slot name — a role in the paint pass, decided by
    /// the painter (`gorgon_petra_egui::paint::KNOWN_SLOTS`), not a design
    /// token — so it stays a plain `String`. The map *value* is a
    /// [`TokenName`]: FR-013's refusal of a literal colour or number in this
    /// position used to be a validation-time check
    /// (`tree::validate::is_style_literal`, since deleted); it is now a
    /// construction-time one, because a `String` that looks like `#3a3a3a`
    /// or `12` cannot become a `TokenName` in the first place, on this map's
    /// value type or on the wire deserializing into it.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, TokenName>,
    /// Per-state overrides of [`Props::tokens`]: state name, then slot name,
    /// then the token that slot takes while the node is in that state
    /// (`contracts/interaction-state.md` §6).
    ///
    /// Authors declare what a hovered button looks like; they never branch on
    /// a raw hover bool, because the bool is engine state and the tree is
    /// authored before the engine has any. The collapse into the flat map the
    /// painter reads happens once, in the placement dispatcher
    /// ([`crate::layout::place`]) — never in a container, so the twelve node
    /// kinds cannot each get the precedence differently — and the resolved
    /// name enters `paint_hash`, so a state change that rebinds a colour
    /// moves the frame digest.
    ///
    /// The outer key is one of [`STATE_NAMES`]; a name outside that set is a
    /// tree-acceptance violation rather than a declaration that quietly never
    /// fires ([`crate::tree::Violation::UnknownStateName`]).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub state_tokens: BTreeMap<String, BTreeMap<String, TokenName>>,
}

/// Every state name [`Props::state_tokens`] may key on, in the order the
/// placement dispatcher applies them: later entries win.
///
/// The order is the precedence, written down once. It runs from the most
/// durable condition to the most momentary — what a node *is* (`selected`,
/// `read-only`), then what the keyboard is on (`focus`), then what the
/// pointer is doing (`hover`, `active`) — and ends with `disabled`, which
/// wins over everything: a control that cannot be operated must not paint as
/// though the pointer over it means anything.
pub const STATE_NAMES: &[&str] = &[
    "selected",
    "read-only",
    "focus",
    "hover",
    "active",
    "disabled",
];

/// Resolved `stack` parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackProps {
    /// Distribution axis.
    pub axis: Axis,
    /// Gap reserved between adjacent children.
    pub spacing: f32,
    /// Cross-axis alignment.
    pub align: crate::geom::Align,
}

/// Resolved `grid` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct GridProps {
    /// Column tracks.
    pub columns: Vec<TrackSize>,
    /// Row tracks. Empty means rows are implicit and `FitContent`.
    pub rows: Vec<TrackSize>,
    /// Gap between columns.
    pub column_spacing: f32,
    /// Gap between rows.
    pub row_spacing: f32,
    /// Cross-axis alignment inside a cell.
    pub align: crate::geom::Align,
}

/// Resolved `scroll` parameters.
///
/// Read by the `scroll` itself and, through the scroll context the layout
/// walk carries, by any `collection` inside it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollProps {
    /// The axis that scrolls. The other axis passes the parent's proposal
    /// through unchanged.
    pub axis: Axis,
    /// Extra logical extent a virtualized child materializes past each end
    /// of the viewport.
    pub overscan: f32,
}

/// Resolved `collection` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct CollectionProps {
    /// Rows in the whole collection, materialized or not.
    pub total_count: usize,
    /// Store-side row source name.
    pub source: String,
    /// Estimated extent of one row along the scrolling axis.
    pub estimated_extent: f32,
}

/// Resolved `text` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct TextProps<'a> {
    /// The content.
    pub text: &'a str,
    /// Truncation policy.
    pub wrap: TextWrap,
    /// Line cap, or `None` for unlimited.
    pub max_lines: Option<usize>,
    /// Typography token name, or `None` for the theme's body style.
    pub style: Option<&'a str>,
}

/// Resolved `surface` parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceProps<'a> {
    /// Which layer.
    pub layer: Layer,
    /// Where it attaches.
    pub anchor: &'a Anchor,
    /// What it does when it does not fit.
    pub clamp: ClampRule,
    /// What it does with outside input.
    pub input_policy: InputPolicy,
}

/// Default gap between stack children when none is declared.
pub const DEFAULT_SPACING: f32 = 0.0;

/// The one place a styling token reference becomes a number.
///
/// Absence answers [`DEFAULT_SPACING`], which is what absence answered before
/// these props were token-typed: the type change governs what an author may
/// *say*, not what silence means.
///
/// A present name always resolves, and this function has no arm for the case
/// where it does not. That is a property of the pipeline, not an optimism:
/// [`crate::token::Theme::build`] proves a theme assigns every vocabulary
/// name, at its declared kind, at a usable extent, and tree acceptance
/// refuses a reference to a name the vocabulary does not declare. A `None`
/// from [`ThemeSnapshot::spacing`] therefore means one of those two proofs is
/// broken, and the honest response is to say so and stop — a fallback here
/// would paint a plausible page over a broken theme and let it ship.
#[must_use]
pub fn resolve_spacing(theme: &ThemeSnapshot, reference: &Option<TokenName>) -> f32 {
    let Some(name) = reference else {
        return DEFAULT_SPACING;
    };
    theme.spacing(name).unwrap_or_else(|| {
        panic!(
            "petra: `{name}` does not resolve to a gap in the theme at revision {}; \
             a validated tree cannot reference a name the vocabulary does not \
             declare at TokenKind::Spacing, so either tree acceptance or \
             Theme::build let this through",
            theme.revision()
        )
    })
}

/// [`resolve_spacing`] on four edges. An absent edge is no inset, the same
/// way an absent `padding` is [`Insets::NONE`] — the two readings have to
/// agree, or `{ "top": "spacing.sm" }` and a fully spelled-out `InsetRefs`
/// with three absent edges would mean different things.
#[must_use]
pub fn resolve_insets(theme: &ThemeSnapshot, reference: &Option<InsetRefs>) -> Insets {
    let Some(refs) = reference else {
        return Insets::NONE;
    };
    Insets {
        top: resolve_spacing(theme, &refs.top),
        right: resolve_spacing(theme, &refs.right),
        bottom: resolve_spacing(theme, &refs.bottom),
        left: resolve_spacing(theme, &refs.left),
    }
}
/// Default overscan for a scroll container, in logical units.
pub const DEFAULT_OVERSCAN: f32 = 64.0;
/// Fallback row extent when a collection declares none.
pub const DEFAULT_ROW_EXTENT: f32 = 24.0;

impl Props {
    /// Resolved `stack` parameters. Valid for any node that passed acceptance
    /// as a `stack`; defaults are the documented ones, never a guess about a
    /// missing required field.
    ///
    /// `theme` is what turns the declared gap token into logical units. It is
    /// a parameter rather than something this type holds because a `Props` is
    /// authored once and read under whichever theme is in force for the frame
    /// being measured — the same tree under two themes is two different sets
    /// of gaps, and there is no cached copy of the earlier one to go stale.
    #[must_use]
    pub fn stack(&self, theme: &ThemeSnapshot) -> StackProps {
        StackProps {
            axis: self.axis.unwrap_or(Axis::Vertical),
            spacing: resolve_spacing(theme, &self.spacing),
            align: self.align.unwrap_or_default(),
        }
    }

    /// Resolved `grid` parameters. `theme` resolves the two gap tokens, as in
    /// [`Props::stack`].
    #[must_use]
    pub fn grid(&self, theme: &ThemeSnapshot) -> GridProps {
        GridProps {
            columns: self.columns.clone(),
            rows: self.rows.clone(),
            column_spacing: resolve_spacing(theme, &self.column_spacing),
            row_spacing: resolve_spacing(theme, &self.row_spacing),
            align: self.align.unwrap_or_default(),
        }
    }

    /// Resolved `scroll` parameters.
    ///
    /// `overscan` falls back to [`DEFAULT_OVERSCAN`] for an absent value and
    /// for a present-but-unusable one (negative or non-finite): it widens a
    /// materialization window below, and a stray author input must not be
    /// able to turn that window inside out.
    #[must_use]
    pub fn scroll(&self) -> ScrollProps {
        ScrollProps {
            overscan: self
                .overscan
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(DEFAULT_OVERSCAN),
            axis: self.axis.unwrap_or(Axis::Vertical),
        }
    }

    /// Resolved grid span. Absent declares one cell, which is
    /// [`GridSpan::ONE`] — the same "absence is the documented default" rule
    /// [`Props::padding`] follows.
    #[must_use]
    pub fn span(&self) -> GridSpan {
        self.span.unwrap_or(GridSpan::ONE)
    }

    /// Resolved content insets. Absent declares no padding, which resolves to
    /// `Insets::NONE` — the same "absence is the documented default" rule
    /// every other resolver here follows (`DEFAULT_SPACING`, `DEFAULT_OVERSCAN`).
    #[must_use]
    pub fn padding(&self, theme: &ThemeSnapshot) -> Insets {
        resolve_insets(theme, &self.padding)
    }

    /// Resolved `collection` parameters, or `None` when the node is not a
    /// collection that passed acceptance.
    #[must_use]
    pub fn collection(&self) -> Option<CollectionProps> {
        Some(CollectionProps {
            total_count: self.total_count?,
            source: self.source.clone()?,
            estimated_extent: self.estimated_extent.unwrap_or(DEFAULT_ROW_EXTENT),
        })
    }

    /// Resolved `text` parameters.
    #[must_use]
    pub fn text(&self) -> TextProps<'_> {
        TextProps {
            text: self.text.as_deref().unwrap_or(""),
            wrap: self.wrap.unwrap_or_default(),
            max_lines: self.max_lines,
            style: self.style.as_ref().map(TokenName::as_str),
        }
    }

    /// Resolved `surface` parameters, or `None` when no anchor is declared.
    #[must_use]
    pub fn surface(&self) -> Option<SurfaceProps<'_>> {
        Some(SurfaceProps {
            layer: self.layer?,
            anchor: self.anchor.as_ref()?,
            clamp: self.clamp.unwrap_or_default(),
            input_policy: self.input_policy.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GridSpan, InsetRefs, Layer, Props, TextWrap, TrackSize, max_row_tracks};
    use crate::geom::{Axis, Insets};
    use crate::token::{ThemeSnapshot, TokenName, light};

    /// A well-formed name, for the fixtures below.
    fn n(name: &str) -> TokenName {
        TokenName::new(name).expect("test token names are well-formed")
    }

    /// The shipped light theme as a snapshot: `spacing.xs` is 4 units and
    /// `spacing.sm` is 8, which is what the resolutions below expect.
    fn theme() -> ThemeSnapshot {
        ThemeSnapshot::new(light(), 1)
    }

    /// One cell on both axes, which is what every child had before FR-061.
    /// Pinned as a test rather than trusted to the `Default` impl staying
    /// written that way: the whole no-movement claim for existing trees rests
    /// on an absent span resolving to exactly this.
    #[test]
    fn an_absent_span_resolves_to_one_cell() {
        assert_eq!(GridSpan::default(), GridSpan::ONE);
        assert_eq!(GridSpan::ONE.columns, 1);
        assert_eq!(GridSpan::ONE.rows, 1);
        assert_eq!(Props::default().span(), GridSpan::ONE);
        assert_eq!(GridSpan::ONE.on(Axis::Horizontal), 1);
        assert_eq!(GridSpan::ONE.on(Axis::Vertical), 1);
    }

    /// A Lua table writes the axis it cares about and leaves the other one
    /// out. FR-011 requires the wire shape stay something a table can produce,
    /// and a table that says `{ rows = 4 }` must not be read as a zero-column
    /// span.
    #[test]
    fn a_span_that_names_one_axis_leaves_the_other_at_one() {
        let rows_only: GridSpan = serde_json::from_str(r#"{"rows":4}"#).unwrap();
        assert_eq!(
            rows_only,
            GridSpan {
                columns: 1,
                rows: 4
            }
        );
        let cols_only: GridSpan = serde_json::from_str(r#"{"columns":3}"#).unwrap();
        assert_eq!(
            cols_only,
            GridSpan {
                columns: 3,
                rows: 1
            }
        );
        let neither: GridSpan = serde_json::from_str("{}").unwrap();
        assert_eq!(neither, GridSpan::ONE);
    }

    /// `deny_unknown_fields`, the same as every other props type here: a
    /// misspelled axis is refused, not silently read as the default.
    #[test]
    fn a_misspelled_span_axis_is_refused_rather_than_defaulted() {
        let err = serde_json::from_str::<GridSpan>(r#"{"column":3}"#).unwrap_err();
        assert!(
            err.to_string().contains("unknown field"),
            "expected an unknown-field refusal, got {err}"
        );
    }

    /// A `Props` with no span serializes without the key at all, so no tree
    /// written before spanning existed changes shape on the wire — and a
    /// `Props` that does declare one round-trips to the same value.
    #[test]
    fn a_span_round_trips_and_an_absent_one_writes_no_key() {
        let plain = Props {
            columns: vec![TrackSize::FitContent],
            ..Props::default()
        };
        let json = serde_json::to_string(&plain).unwrap();
        assert!(
            !json.contains("span"),
            "an undeclared span must not appear on the wire, got {json}"
        );
        assert_eq!(serde_json::from_str::<Props>(&json).unwrap(), plain);

        let spanning = Props {
            span: Some(GridSpan {
                columns: 2,
                rows: 4,
            }),
            ..Props::default()
        };
        let json = serde_json::to_string(&spanning).unwrap();
        assert_eq!(json, r#"{"span":{"columns":2,"rows":4}}"#);
        assert_eq!(serde_json::from_str::<Props>(&json).unwrap(), spanning);
        assert_eq!(
            serde_json::from_str::<Props>(&json).unwrap().span(),
            GridSpan {
                columns: 2,
                rows: 4
            }
        );
    }

    /// The row ceiling is one row per child, never `children / columns`.
    ///
    /// Dividing is the mistake this function exists to name: four children in
    /// four columns take five cells once one of them spans two, so the last
    /// child sits in row 1 while `4.div_ceil(4)` still answers one row. A
    /// ceiling derived that way would refuse a two-row span on a grid that is
    /// about to have two rows.
    #[test]
    fn the_row_ceiling_counts_children_not_cells_per_row() {
        assert_eq!(max_row_tracks(0, 4), 4);
        assert_eq!(max_row_tracks(96, 20), 96, "declared rows win when larger");
        assert_eq!(max_row_tracks(2, 9), 9, "children win when they are more");
        assert_eq!(max_row_tracks(0, 0), 0);
    }

    /// One vocabulary for one enum. The digest hashes
    /// [`TextWrap::as_str`] and the wire form uses serde's name; if the two
    /// drift, a frame received over the driver protocol and a frame petrified
    /// locally would hash the same policy to two different strings and the
    /// SC-004 cross-target claim would be false with nothing to show for it.
    #[test]
    fn the_wrap_policy_has_one_name_in_the_digest_and_on_the_wire() {
        for wrap in [TextWrap::Wrap, TextWrap::Ellipsis, TextWrap::Clip] {
            let json = serde_json::to_string(&wrap).unwrap();
            assert_eq!(
                json,
                format!("\"{}\"", wrap.as_str()),
                "{wrap:?} serializes as {json} but digests as {:?}",
                wrap.as_str()
            );
            let back: TextWrap = serde_json::from_str(&json).unwrap();
            assert_eq!(back, wrap);
        }
    }

    #[test]
    fn an_empty_props_serializes_to_an_empty_table() {
        let json = serde_json::to_string(&Props::default()).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn declared_fields_round_trip() {
        let mut props = Props {
            axis: Some(Axis::Horizontal),
            spacing: Some(n("spacing.sm")),
            columns: vec![
                TrackSize::Fixed { value: 100.0 },
                TrackSize::Weight { weight: 1.0 },
            ],
            wrap: Some(TextWrap::Ellipsis),
            layer: Some(Layer::Modal),
            padding: Some(InsetRefs::symmetric(n("spacing.xs"), n("spacing.sm"))),
            ..Props::default()
        };
        props
            .tokens
            .insert("background".into(), n("surface.raised"));
        let json = serde_json::to_string(&props).unwrap();
        let back: Props = serde_json::from_str(&json).unwrap();
        assert_eq!(back, props);
        assert!(json.contains("\"axis\":\"horizontal\""), "{json}");
        assert!(
            json.contains("\"type\":\"fit-content\"") || json.contains("\"weight\""),
            "{json}"
        );
    }

    /// A misspelled prop is a typo the author wants to hear about, not a
    /// silently ignored field that makes a panel lay out wrong.
    #[test]
    fn unknown_props_are_refused() {
        let err = serde_json::from_str::<Props>(r#"{"spaceing": 8}"#).unwrap_err();
        assert!(err.to_string().contains("spaceing"), "{err}");
    }

    #[test]
    fn defaults_are_the_documented_ones() {
        let props = Props::default();
        let theme = theme();
        assert_eq!(props.stack(&theme).axis, Axis::Vertical);
        assert_eq!(props.stack(&theme).spacing, super::DEFAULT_SPACING);
        assert_eq!(props.grid(&theme).column_spacing, super::DEFAULT_SPACING);
        assert_eq!(props.grid(&theme).row_spacing, super::DEFAULT_SPACING);
        assert_eq!(props.scroll().axis, Axis::Vertical);
        assert!(props.collection().is_none());
        assert!(props.surface().is_none());
        assert_eq!(props.text().text, "");
        assert_eq!(props.padding(&theme), Insets::NONE);
    }

    /// `Props.padding` round-trips through serde the same way every other
    /// declared field does — pinned separately from `declared_fields_round_trip`
    /// because its serde shape (a four-edge struct, not a plain scalar) is
    /// worth checking on its own.
    #[test]
    fn padding_round_trips_through_serde() {
        let props = Props {
            padding: Some(InsetRefs::symmetric(n("spacing.xs"), n("spacing.sm"))),
            ..Props::default()
        };
        let json = serde_json::to_string(&props).unwrap();
        assert_eq!(
            json,
            r#"{"padding":{"top":"spacing.sm","right":"spacing.xs","bottom":"spacing.sm","left":"spacing.xs"}}"#
        );
        let back: Props = serde_json::from_str(&json).unwrap();
        assert_eq!(back, props);
        assert_eq!(back.padding(&theme()), Insets::symmetric(4.0, 8.0));

        // Absent padding serializes away entirely, and resolves to NONE.
        let bare = Props::default();
        let bare_json = serde_json::to_string(&bare).unwrap();
        assert!(!bare_json.contains("padding"), "{bare_json}");
        assert_eq!(bare.padding(&theme()), Insets::NONE);
    }

    /// `InsetRefs` mirrors [`Insets`] one level up, so the two constructors
    /// have to seat their arguments on the same edges. Checked against
    /// [`Insets`] itself rather than against a hand-written expectation: if
    /// `Insets::symmetric` ever swapped its arguments, this fails with it
    /// instead of quietly agreeing with the old order.
    #[test]
    fn the_two_inset_constructors_agree_edge_for_edge() {
        let theme = theme();
        let all = Props {
            padding: Some(InsetRefs::all(n("spacing.sm"))),
            ..Props::default()
        };
        assert_eq!(all.padding(&theme), Insets::all(8.0));

        let sym = Props {
            padding: Some(InsetRefs::symmetric(n("spacing.xs"), n("spacing.sm"))),
            ..Props::default()
        };
        assert_eq!(sym.padding(&theme), Insets::symmetric(4.0, 8.0));
    }

    /// A partly declared `InsetRefs` insets only the edges it names, and an
    /// empty one insets nothing — the same reading an absent `padding` gets.
    /// The empty form also has to survive the wire: `{}` is what a Lua table
    /// with no keys produces.
    #[test]
    fn an_unnamed_edge_is_no_inset_and_an_empty_refs_writes_an_empty_table() {
        let theme = theme();
        let top_only = Props {
            padding: Some(InsetRefs {
                top: Some(n("spacing.lg")),
                ..InsetRefs::default()
            }),
            ..Props::default()
        };
        assert_eq!(
            top_only.padding(&theme),
            Insets {
                top: 16.0,
                ..Insets::NONE
            }
        );
        assert_eq!(
            serde_json::to_string(&top_only).unwrap(),
            r#"{"padding":{"top":"spacing.lg"}}"#
        );

        let empty = Props {
            padding: Some(InsetRefs::default()),
            ..Props::default()
        };
        assert_eq!(serde_json::to_string(&empty).unwrap(), r#"{"padding":{}}"#);
        assert_eq!(
            serde_json::from_str::<Props>(r#"{"padding":{}}"#).unwrap(),
            empty
        );
        assert_eq!(empty.padding(&theme), Insets::NONE);
    }

    /// FR-053's whole point, on the wire. A styling prop is a token
    /// reference, so a literal in that slot is refused at deserialization —
    /// by [`TokenName`]'s own structural check, before any tree ever reaches
    /// acceptance. This is what replaced the numeric range check tree
    /// acceptance used to run on `spacing`.
    #[test]
    fn a_literal_in_a_styling_slot_is_refused_on_the_wire() {
        for bad in [
            r#"{"spacing":8}"#,
            r#"{"spacing":-1}"#,
            r#"{"spacing":"16"}"#,
            r##"{"spacing":"#aabbcc"}"##,
            r#"{"column_spacing":12}"#,
            r#"{"row_spacing":"4"}"#,
            r#"{"padding":{"top":6}}"#,
            r#"{"padding":{"left":"8"}}"#,
        ] {
            assert!(
                serde_json::from_str::<Props>(bad).is_err(),
                "{bad} names a value in a slot that only takes a token"
            );
        }
    }

    /// The layout-declaration and performance-hint classes stay numeric
    /// (FR-053). A minimum width is a fact about a label and `overscan`
    /// changes no pixel, so neither belongs to a theme — and this is the
    /// test that fails if a later pass token-types them by reflex.
    #[test]
    fn declarations_and_hints_are_still_numbers() {
        let props: Props = serde_json::from_str(
            r#"{"overscan":128,"estimated_extent":24,"total_count":900,"max_lines":3,
                "columns":[{"type":"fixed","value":220}]}"#,
        )
        .expect("numeric declarations and hints stay numeric");
        assert_eq!(props.overscan, Some(128.0));
        assert_eq!(props.estimated_extent, Some(24.0));
        assert_eq!(props.total_count, Some(900));
        assert_eq!(props.max_lines, Some(3));
        assert_eq!(props.columns, vec![TrackSize::Fixed { value: 220.0 }]);
    }

    /// The gap a container reserves tracks the theme it is measured under,
    /// which is the whole reason resolution takes a snapshot instead of
    /// happening once at authoring time.
    #[test]
    fn one_declaration_resolves_to_two_gaps_under_two_themes() {
        let props = Props {
            spacing: Some(n("spacing.sm")),
            column_spacing: Some(n("spacing.lg")),
            ..Props::default()
        };
        assert_eq!(props.stack(&theme()).spacing, 8.0);
        assert_eq!(props.grid(&theme()).column_spacing, 16.0);

        let mut values = light().values().clone();
        values.insert(n("spacing.sm"), crate::token::TokenValue::Spacing(40.0));
        let roomy = ThemeSnapshot::new(
            crate::token::Theme::build(
                crate::token::ThemeMode::Light,
                &crate::token::standard_vocabulary(),
                values,
            )
            .expect("light()'s assignments with one gap replaced are complete"),
            2,
        );
        assert_eq!(props.stack(&roomy).spacing, 40.0);
        assert_eq!(
            props.grid(&roomy).column_spacing,
            16.0,
            "only the reassigned name moved"
        );
    }

    #[test]
    fn layer_z_bases_are_ordered() {
        assert!(Layer::Popup.base_z() < Layer::Toast.base_z());
        assert!(Layer::Toast.base_z() < Layer::Modal.base_z());
        assert!(Layer::Modal.base_z() < Layer::FrameWide.base_z());
    }
}
