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
use super::key::{Key, KeyPath};
use crate::geom::{Axis, Insets};
use crate::token::{ThemeSnapshot, TokenName};

/// One coloured stretch of a text node's content.
///
/// A run changes the **ink and nothing else**. It carries no font, no size
/// and no weight, because the thing this exists for — syntax colour, and
/// later an LSP `textDocument/semanticTokens` response — is a colour per
/// token over one face. Keeping it to colour is what makes it cheap: text is
/// shaped through [`crate::layout::ContentMeasure::text`], whose request
/// carries the string, the typography token, the wrap policy and the line
/// cap, so a run touches none of a measurement's inputs and every placed
/// rect on the page is the rect it was before.
///
/// `len` is a **byte** count, and the runs of a node cover its text exactly:
/// acceptance refuses a list that overruns, underruns, or splits a `char`
/// ([`crate::tree::Violation::TextRunsDoNotCoverTheText`]). A half-covering
/// list would paint a colour onto whatever followed it, which is a wrong
/// picture that nothing downstream could notice.
///
/// `foreground` is `None` for "the node's own ink", so a highlighter names
/// only the stretches it has an opinion about and the gaps between them cost
/// one entry each rather than a token lookup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextRun {
    /// Length of this run in bytes of the node's `text`.
    pub len: usize,
    /// Ink for this run, or `None` to take the node's own `foreground`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<TokenName>,
}

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
///
/// Two of the variants name a node; they differ only in *how*. [`Anchor::Node`]
/// carries the target's canonical key-path id verbatim, which is what a
/// hand-written tree that knows its own shape can supply. [`Anchor::Sibling`]
/// carries a bare [`Key`] and is resolved against the anchored surface's own
/// parent path: it is the form a component constructor uses, because a
/// constructor builds a node before any caller has decided where in the tree
/// to mount it and so cannot know its canonical id ([`KeyPath::sibling_id`]).
/// Both resolve to one canonical id before anything reads them —
/// [`Anchor::target_id`] is the one place that happens — so acceptance, the
/// harvest walk and placement all see a single kind of node anchor.
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
    /// Attached to the rect of a sibling of the anchored surface, by that
    /// sibling's bare key.
    ///
    /// Resolved as "the node keyed `key` in the same child list as this
    /// surface": the surface's own [`KeyPath`] with its last segment
    /// replaced by `key` ([`KeyPath::sibling_id`]). Nothing else is
    /// searched, so a key reused at another depth of the same tree can never
    /// be reached by mistake, and a `key` no sibling has is refused at
    /// acceptance exactly as an [`Anchor::Node`] naming nothing is
    /// ([`crate::tree::Violation::AnchorTargetMissing`], carrying the
    /// canonical id that was tried).
    ///
    /// This is the form every component constructor in
    /// [`crate::component`] uses: a constructor places its trigger and its
    /// popover side by side in one child list and names the trigger by the
    /// key it just chose, which is the only id it can possibly have in hand.
    Sibling {
        /// Key of the sibling node this surface attaches to.
        key: Key,
        /// Which edge of the anchor the surface prefers.
        edge: Edge,
        /// Where along that edge the surface starts. Defaulted for the same
        /// reason [`Anchor::Node`]'s is.
        #[serde(default, skip_serializing_if = "is_default_align")]
        align: Align,
        /// Gap between the anchor's edge and the surface, as a spacing
        /// token. Same rule as [`Anchor::Node`]'s.
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
    /// Docked *inside* an edge of the viewport, inset from it.
    ///
    /// [`Anchor::Viewport`] is the centre of the window and nothing else,
    /// which is the right answer for a modal and the wrong one for every
    /// region a design system pins to a window edge: Carbon's toast region
    /// is top-trailing, its side navigation is a left dock, its header
    /// panel is pinned to the trailing edge. Those were unspellable until
    /// this variant existed, and the standing workaround was to build a
    /// stand-in frame node the size of the window and seat the region in a
    /// cell of it.
    ///
    /// The pair `(edge, align)` reads exactly as it does on
    /// [`Anchor::Node`] — `{ edge: Top, align: End }` is "top-trailing" —
    /// with one deliberate difference in what `edge` means. A node anchor
    /// puts the surface *outside* the named edge, because a menu belongs
    /// beside its trigger and not on top of it. A viewport edge puts the
    /// surface *inside* it, because outside the window is nowhere.
    ///
    /// Additive on purpose: a variant name no tree written before it can
    /// carry cannot change how any of those trees parse, where widening
    /// [`Anchor::Viewport`] from a unit variant into a struct one would
    /// have rewritten its serde form for every tree that already spells it.
    ViewportEdge {
        /// Which edge of the window the surface docks against.
        edge: Edge,
        /// Where along that edge it sits. Defaulted, and read exactly as
        /// [`Anchor::Node`]'s is.
        #[serde(default, skip_serializing_if = "is_default_align")]
        align: Align,
        /// How far in from the window the surface is held, as a spacing
        /// token — a token reference and never a literal, for the reason
        /// [`Anchor::Node`]'s `offset` is one (FR-053).
        ///
        /// One number for both axes, because a docked region is inset from
        /// a *corner*: Carbon's toast is 16 down from the top and 16 in
        /// from the trailing edge, not 16 from one and flush against the
        /// other. It insets the docked edge, and it insets whichever end of
        /// that edge [`Align`] chose; [`Align::Center`] has no end to be
        /// held off, so a centred surface stays exactly centred whatever
        /// the offset says.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<TokenName>,
    },
}

/// The terms an anchor that names a node declares, read the same way
/// whichever of [`Anchor::Node`] and [`Anchor::Sibling`] declared them.
///
/// The placement ladder (`contracts/anchored-placement.md` §4) is a function
/// of `(edge, align, offset)` and the harvested rect; which spelling named
/// the rect is not one of its inputs, and this is the type that keeps it
/// from becoming one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeAnchor<'a> {
    /// Which edge of the anchor the surface prefers.
    pub edge: Edge,
    /// Where along that edge the surface starts.
    pub align: Align,
    /// Gap between the anchor's edge and the surface, as a spacing token.
    pub offset: Option<&'a TokenName>,
}

impl Anchor {
    /// The canonical id of the node this anchor names, given the anchored
    /// surface's own key path, or `None` for an anchor that names no node.
    ///
    /// `surface` is the path *of the surface carrying this anchor*, with the
    /// surface's own key as its last segment — the path every tree walk in
    /// this crate has in hand at the moment it reads `props.anchor`. It is
    /// the only context [`Anchor::Sibling`] needs and [`Anchor::Node`]
    /// ignores it, which is what lets every consumer resolve both through
    /// this one call rather than matching the variants apart.
    #[must_use]
    pub fn target_id(&self, surface: &KeyPath) -> Option<String> {
        match self {
            Self::Node { id, .. } => Some(id.clone()),
            Self::Sibling { key, .. } => Some(surface.sibling_id(key)),
            Self::Point { .. } | Self::Viewport | Self::ViewportEdge { .. } => None,
        }
    }

    /// Whether this anchor names a node at all — the two variants a harvest
    /// walk has to place first and an incremental frame has to re-resolve.
    #[must_use]
    pub fn names_node(&self) -> bool {
        matches!(self, Self::Node { .. } | Self::Sibling { .. })
    }

    /// The `(edge, align, offset)` a node anchor declares; `None` for an
    /// anchor that names no node.
    #[must_use]
    pub fn node_terms(&self) -> Option<NodeAnchor<'_>> {
        match self {
            Self::Node {
                edge,
                align,
                offset,
                ..
            }
            | Self::Sibling {
                edge,
                align,
                offset,
                ..
            } => Some(NodeAnchor {
                edge: *edge,
                align: *align,
                offset: offset.as_ref(),
            }),
            // A viewport edge declares the same three terms and is
            // deliberately not one of these: `NodeAnchor` is what the
            // *node* ladder reads, and handing it a reading with no
            // harvested rect behind it is how a caret ends up aimed at a
            // window edge. [`Anchor::ViewportEdge`] is destructured where
            // it is placed, the way [`Anchor::Point`] is.
            Self::Point { .. } | Self::Viewport | Self::ViewportEdge { .. } => None,
        }
    }

    /// The spacing token gapping this anchor from whatever it is anchored
    /// to, if one is declared. The one spacing reference on a node that does
    /// not live on a `props.*_spacing` field, which is why it has its own
    /// accessor.
    ///
    /// Every variant that can carry an offset answers here, not just the two
    /// that name a node: this is what tree acceptance checks the token
    /// reference through (`crate::tree::validate`) and what the test
    /// harness collects a fixture's spacing names through
    /// (`crate::testing`). A variant missing from this match would carry an
    /// unchecked, unbound token name — accepted by acceptance, then resolved
    /// to zero at placement, which is a dock silently flush against the
    /// window edge.
    #[must_use]
    pub fn offset(&self) -> Option<&TokenName> {
        match self {
            Self::Node { offset, .. }
            | Self::Sibling { offset, .. }
            | Self::ViewportEdge { offset, .. } => offset.as_ref(),
            Self::Point { .. } | Self::Viewport => None,
        }
    }
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

/// Main-axis placement of a `stack`'s children within whatever extent is
/// left over once distribution has given every child its answer — CSS
/// `justify-content`, for a stack.
///
/// The first three are [`crate::geom::Align`]'s three positions and read the
/// same way: they move the whole run's starting cursor and nothing else.
/// [`Justify::SpaceBetween`] is the one that has no cross-axis twin, which
/// is why this is its own enum rather than a fourth `Align` variant: it
/// spends the leftover *between* the children, growing every gap equally,
/// so the first child sits at the leading edge and the last at the
/// trailing one. Carbon's button with an icon is exactly this
/// (`.cds--btn { justify-content: space-between }`): label at the start,
/// glyph at the end, whatever the button's width. With one child there is
/// nothing to spread between and it reads as [`Justify::Start`].
///
/// None of these changes how much room a child got; a container that ran
/// out of room packs exactly as it always did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Justify {
    /// Leftover trails after the last child.
    #[default]
    Start,
    /// Leftover splits evenly before the first child and after the last.
    Center,
    /// Leftover leads before the first child.
    End,
    /// Leftover is spent evenly between adjacent children.
    SpaceBetween,
}

impl Justify {
    /// Where the run starts: the leading offset of the first child inside
    /// `available` when the run wants `wanted`. Floors at zero, so an
    /// oversized run is never pushed further past the container's edge.
    #[must_use]
    pub fn offset(self, available: f32, wanted: f32) -> f32 {
        match self {
            Self::Start | Self::SpaceBetween => 0.0,
            Self::Center => ((available - wanted) / 2.0).max(0.0),
            Self::End => (available - wanted).max(0.0),
        }
    }

    /// How much every one of `gaps` gaps grows: the leftover shared out
    /// between adjacent children for [`Justify::SpaceBetween`], nothing for
    /// the rest. Zero when there are no gaps to grow.
    #[must_use]
    pub fn spread(self, available: f32, wanted: f32, gaps: f32) -> f32 {
        match self {
            Self::SpaceBetween if gaps > 0.0 => ((available - wanted) / gaps).max(0.0),
            _ => 0.0,
        }
    }
}

/// Whether an anchored `surface` draws a caret back at its anchor.
///
/// Carbon's popover comes in a **caret tip** form and a **no tip** form
/// (`_popover.scss`, `--caret`), and the two are not interchangeable: a
/// tooltip, a popover and a toggletip point at the control that opened them,
/// while a list box — dropdown, select, menu, date picker — butts flush
/// against its field with no pointer at all. The engine computes the caret's
/// geometry (`contracts/anchored-placement.md` §5) and it used to compute one
/// for every anchored surface, so every list box in the library grew a beak
/// nobody asked for. This is the author's half of that decision: *whether*
/// there is a caret. *Where* it goes stays the engine's.
///
/// Absent means [`Tip::Caret`], which is what every anchored surface drew
/// before this field existed. Declared on a kind other than `surface`, or on
/// a surface whose anchor names no node, it changes nothing — there is no
/// caret to suppress — the same way `canvas` on a non-canvas paints nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tip {
    /// A caret grows out of the surface's near edge towards the anchor.
    #[default]
    Caret,
    /// No caret: the surface's near edge is a plain edge.
    Flush,
}

/// How broad an anchored `surface` is along the edge it hangs off.
///
/// Carbon's list box menu is `inline-size: 100%` of the field that opened
/// it, and a menu button's menu is at least as wide as its trigger. A
/// surface's own content cannot know its anchor's width — the field is
/// stretched by whatever column it sits in — so this is a relationship the
/// engine has to resolve at placement, after the anchor's rect is harvested.
///
/// Absent means [`Fit::Content`]: the surface is as broad as its content and
/// its constraints say, which is what every anchored surface was before this
/// field existed. Declared on a non-surface it changes nothing, and the same
/// goes for an anchor that names no edge at all ([`Anchor::Point`],
/// [`Anchor::Viewport`]).
///
/// [`Anchor::ViewportEdge`] does name an edge, and `Fit::Anchor` fits to it:
/// the window's own extent along that edge, less the inset the anchor
/// already holds the surface off each end by. A docked status bar that spans
/// its window is the shape this answers, and reading the window edge as "no
/// anchor" is what left one sitting at its content width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fit {
    /// As broad as the content and the surface's own constraints decide.
    #[default]
    Content,
    /// At least as broad as the anchor's edge; broader only if the content
    /// needs it. The anchor's extent wins over the surface's own `max`
    /// constraint, because a list narrower than its field is the defect this
    /// exists to prevent.
    ///
    /// The edge is the harvested rect's for a node anchor and the window's
    /// for an [`Anchor::ViewportEdge`]. Both are edges with an extent; only
    /// one of them is a node.
    Anchor,
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
    /// Main-axis placement of a `stack`'s children within whatever extent is
    /// left over once distribution has given every child its answer. See
    /// [`Justify`].
    ///
    /// Absence is [`Justify::Start`], which is the behaviour a `stack` had
    /// before this field existed: leftover main-axis space trails after the
    /// last child, because `stack::place` walked declaration order from
    /// `content`'s own leading edge with no offset. A declared value only
    /// ever moves the starting cursor, or — for [`Justify::SpaceBetween`] —
    /// the gaps; it changes nothing about how much room each child gets.
    ///
    /// Only `stack` honours this; a `grid`'s tracks are already sized by
    /// their own column/row rule, so there is no leftover extent for this to
    /// move.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justify: Option<Justify>,
    /// This node's own cross-axis alignment inside the space its `stack`
    /// parent gives it, overriding the parent's [`Props::align`] for this
    /// child alone — the same relationship CSS's `align-self` has to
    /// `align-items`.
    ///
    /// Declared on the **child**, read by the `stack` **parent**
    /// (`stack::place`), which is why it lives beside `align` rather than
    /// inside it: one node's `align` governs its own children, and a
    /// different node's `align_self` governs how *it* sits in somebody
    /// else's row. Absent means the parent's own `align` decides, exactly
    /// the behaviour every `stack` had before this field existed.
    ///
    /// Only a `stack` parent honours this today; a `grid` parent still
    /// places every cell by its own single `align`, uniformly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align_self: Option<crate::geom::Align>,
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
    /// Per-stretch ink over `text`, or empty for one run in the node's own
    /// `foreground` ([`TextRun`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<TextRun>,
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
    /// Whether a `surface` takes keyboard focus for as long as it is
    /// mounted: focus moves onto the first focusable node inside it when it
    /// appears, and back to whatever held focus at that moment when it goes
    /// away.
    ///
    /// Carbon's `Menu` is the component that needs this
    /// (`@carbon/react/lib/components/Menu/Menu.js`: `handleOpen` saves
    /// `document.activeElement` and focuses the list, an effect then seats
    /// item 0, and `handleClose` calls `returnFocus`). It is declared rather
    /// than inferred because the other anchored list boxes want the
    /// opposite: Carbon's Dropdown and Combo box keep DOM focus on the
    /// trigger and point at the highlighted option with
    /// `aria-activedescendant`, and a Date picker keeps focus in its field.
    /// A rule keyed on [`crate::tree::Role::Overlay`] would move focus in
    /// all four.
    ///
    /// **Silence is not the same as `Some(false)`** only to a reader: both
    /// mean "does not take focus". The flag is a `bool` so the surface can
    /// say so out loud where a component's intent would otherwise be a
    /// missing line.
    ///
    /// Declaring it on any kind other than `surface` does nothing, the same
    /// way [`Props::canvas`] on a non-canvas paints nothing:
    /// `crate::layout::overlay_surface::focus_taking_surfaces` reads it only
    /// where [`Props::surface`] answers `Some`. *When* the move happens is
    /// the host's business, not the tree's — `gorgon-petra-egui`'s
    /// `Host::reseat_focus_taking_surfaces` is the one implementation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub takes_focus: Option<bool>,
    /// Whether an anchored `surface` draws a caret. See [`Tip`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tip: Option<Tip>,
    /// How broad an anchored `surface` is along its anchor's edge. See
    /// [`Fit`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fit: Option<Fit>,
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
    /// The Markdown this node's picture stands for, when a reader copies it.
    ///
    /// A node that draws a mark in place of a character has no text for the
    /// clipboard to take: a list bullet is a [`Props::canvas`] and not a
    /// glyph, deliberately, because *"a typed marker cannot be sized, centred
    /// or snapped apart from a character"* (`crate::component::list`). Copying
    /// a list therefore lost every bullet while the generated `1.` `2.`
    /// counters — which are text — copied themselves.
    ///
    /// This is where the node that drew the picture says what the picture
    /// says in words, and it is a declaration rather than a rule the frame
    /// infers. The alternative was for
    /// [`crate::frame::PetrifiedFrame::selected_text`] to recognise a bullet
    /// by its shape — a canvas sitting before a label inside a
    /// [`crate::tree::Role::ListItem`] — which puts one component's
    /// arrangement into the engine's clipboard and gets the next drawn mark
    /// wrong. A task list's drawn checkbox would say `- [ ]` here and need no
    /// change anywhere else.
    ///
    /// Markdown, not a transcription. The mark Carbon draws at level two is
    /// a filled square, and `-` is what a reader wants in a document; a `▪`
    /// on the clipboard is a character no renderer reads as a list. The
    /// indent the copy already carries makes the level plain.
    ///
    /// **Read only from nodes that paint no text of their own.** Set on a
    /// `text` node it is inert, because the run's own string is already what
    /// the clipboard takes and emitting both would say everything twice.
    ///
    /// Not a digest input: it changes no pixel. `crate::frame::digest`
    /// destructures it and drops it, which is the decision that file's
    /// no-rest-pattern comment exists to force.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
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
}

/// Resolved `stack` parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackProps {
    /// Distribution axis.
    pub axis: Axis,
    /// Gap reserved between adjacent children.
    pub spacing: f32,
    /// Cross-axis alignment.
    pub align: crate::geom::Align,
    /// Main-axis placement of the whole run within any leftover extent.
    pub justify: Justify,
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
    /// Whether it draws a caret back at a node anchor.
    pub tip: Tip,
    /// How broad it is along a node anchor's edge.
    pub fit: Fit,
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
            justify: self.justify.unwrap_or_default(),
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
            tip: self.tip.unwrap_or_default(),
            fit: self.fit.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Align, Anchor, Edge, GridSpan, InsetRefs, KeyPath, Layer, Props, TextWrap, TrackSize,
        max_row_tracks,
    };
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
    /// `Anchor::Sibling` on the wire: a `key` where `Node` has an `id`, the
    /// same defaulted `align` and `offset`, and the same `type` tag scheme,
    /// so a tree written by hand can use either spelling.
    #[test]
    fn a_sibling_anchor_round_trips_through_serde() {
        let bare = Anchor::Sibling {
            key: "trigger".into(),
            edge: Edge::Bottom,
            align: Align::default(),
            offset: None,
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert_eq!(
            json,
            r#"{"type":"sibling","key":"trigger","edge":"bottom"}"#
        );
        assert_eq!(serde_json::from_str::<Anchor>(&json).unwrap(), bare);

        let full = Anchor::Sibling {
            key: "trigger".into(),
            edge: Edge::Right,
            align: Align::End,
            offset: Some(n("spacing.xs")),
        };
        let json = serde_json::to_string(&full).unwrap();
        assert_eq!(
            json,
            r#"{"type":"sibling","key":"trigger","edge":"right","align":"end","offset":"spacing.xs"}"#
        );
        assert_eq!(serde_json::from_str::<Anchor>(&json).unwrap(), full);
    }

    /// The two node-naming spellings resolve through one accessor, and the
    /// two that name no node answer `None` to all of it.
    #[test]
    fn target_id_resolves_a_sibling_against_the_surfaces_parent() {
        let surface = KeyPath::root()
            .child(&"root".into())
            .child(&"page".into())
            .child(&"popup".into());
        let sibling = Anchor::Sibling {
            key: "trigger".into(),
            edge: Edge::Top,
            align: Align::Start,
            offset: Some(n("spacing.xs")),
        };
        assert_eq!(
            sibling.target_id(&surface).as_deref(),
            Some("/root/page/trigger")
        );
        assert!(sibling.names_node());
        let terms = sibling.node_terms().unwrap();
        assert_eq!((terms.edge, terms.align), (Edge::Top, Align::Start));
        assert_eq!(sibling.offset(), Some(&n("spacing.xs")));

        let node = Anchor::Node {
            id: "/elsewhere/entirely".into(),
            edge: Edge::Left,
            align: Align::Center,
            offset: None,
        };
        assert_eq!(
            node.target_id(&surface).as_deref(),
            Some("/elsewhere/entirely"),
            "a node anchor's id is taken as declared, wherever the surface is"
        );
        assert!(node.names_node());
        assert_eq!(node.offset(), None);

        for other in [
            Anchor::Point { x: 1.0, y: 2.0 },
            Anchor::Viewport,
            Anchor::ViewportEdge {
                edge: Edge::Top,
                align: Align::End,
                offset: None,
            },
        ] {
            assert_eq!(other.target_id(&surface), None);
            assert!(!other.names_node());
            assert!(other.node_terms().is_none());
            assert_eq!(other.offset(), None);
        }
    }

    /// A viewport edge names no node, so it never reaches the harvest walk,
    /// the cycle scan, or the node ladder — but it *does* carry a spacing
    /// token, and [`Anchor::offset`] is the one accessor that has to see it.
    ///
    /// Acceptance checks the token reference through this accessor
    /// (`crate::tree::validate`) and the test harness collects a fixture's
    /// spacing names through it (`crate::testing`). Reading the offset off
    /// `node_terms` instead — which is what it did until this variant
    /// existed — leaves a docked surface naming a token nothing declares and
    /// nothing checks, resolved to zero at placement: a dock silently flush
    /// against the window edge.
    #[test]
    fn a_viewport_edge_names_no_node_but_still_carries_its_offset() {
        let surface = KeyPath::root().child(&"root".into()).child(&"toast".into());
        let docked = Anchor::ViewportEdge {
            edge: Edge::Top,
            align: Align::End,
            offset: Some(n("spacing.xs")),
        };
        assert_eq!(docked.target_id(&surface), None);
        assert!(!docked.names_node());
        assert!(
            docked.node_terms().is_none(),
            "a window edge is not a node, and the node ladder must not read it as one"
        );
        assert_eq!(
            docked.offset(),
            Some(&n("spacing.xs")),
            "the offset accessor is what acceptance and the fixture harness \
             both check the token reference through"
        );
    }

    /// `Anchor::ViewportEdge` on the wire, and the reason it is additive:
    /// its `type` tag is a name no tree written before it can carry, so
    /// every anchor already in a file parses exactly as it did. Widening
    /// `Anchor::Viewport` from a unit variant into a struct one would have
    /// rewritten `{"type":"viewport"}` for every tree that spells it.
    #[test]
    fn a_viewport_edge_anchor_round_trips_through_serde() {
        let bare = Anchor::ViewportEdge {
            edge: Edge::Top,
            align: Align::default(),
            offset: None,
        };
        let json = serde_json::to_string(&bare).unwrap();
        assert_eq!(json, r#"{"type":"viewport-edge","edge":"top"}"#);
        assert_eq!(serde_json::from_str::<Anchor>(&json).unwrap(), bare);

        let full = Anchor::ViewportEdge {
            edge: Edge::Top,
            align: Align::End,
            offset: Some(n("spacing.xs")),
        };
        let json = serde_json::to_string(&full).unwrap();
        assert_eq!(
            json,
            r#"{"type":"viewport-edge","edge":"top","align":"end","offset":"spacing.xs"}"#
        );
        assert_eq!(serde_json::from_str::<Anchor>(&json).unwrap(), full);

        // The variant beside it is untouched: the unit form still reads and
        // writes as the bare tag it always did.
        let json = serde_json::to_string(&Anchor::Viewport).unwrap();
        assert_eq!(json, r#"{"type":"viewport"}"#);
        assert_eq!(
            serde_json::from_str::<Anchor>(r#"{"type":"viewport"}"#).unwrap(),
            Anchor::Viewport
        );
    }

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
