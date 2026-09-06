//! The view node: plain nested data, no closures, no state, no toolkit types.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::tree::key::Key;
use crate::tree::props::Props;

/// What a node is.
///
/// Thirteen built-ins. `Custom` defers measurement and painting to a
/// registered implementation named by `props.custom_kind`; an unregistered
/// name is a tree-acceptance error, never a render-time surprise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    /// One-axis distribution container.
    Stack,
    /// Two-axis track container.
    Grid,
    /// Children share the container's rect; z-order decides paint order.
    Overlay,
    /// Clips and offsets its content along one axis.
    Scroll,
    /// A virtualized row range read from a store-side source.
    Collection,
    /// An overlay surface (popup, toast, modal, frame-wide seat).
    Surface,
    /// A text run.
    Text,
    /// A bitmap or vector image.
    Image,
    /// An editable text field.
    Input,
    /// Empty, flexible space.
    Spacer,
    /// A one-logical-unit rule.
    Separator,
    /// Host-registered measurement and painting.
    Custom,
    /// A draw list the engine executes itself.
    ///
    /// A second kind beside [`NodeKind::Custom`], never a mode of it
    /// (`contracts/draw-list.md` §7, `research.md` D-06). The two differ on
    /// both halves of what a node kind decides:
    ///
    /// * **Size.** A `custom` node asks the measurement registry, because a
    ///   host-registered measurer supplies one. A draw list has no intrinsic
    ///   size at all — a list of coordinates is not a request for room — so a
    ///   canvas sizes from its layout constraints alone and never reaches that
    ///   registry ([`crate::layout::leaf::measure`]).
    /// * **The digest.** A `custom` node's picture reaches the frame digest as
    ///   the painter's *name*; a canvas's picture reaches it as the picture,
    ///   through [`crate::frame::digest::hash_paint_content`]. Merging the two
    ///   would make the wider of the two behaviours apply to both, and the
    ///   wider one is "the digest is blind here".
    Canvas,
}

impl NodeKind {
    /// Whether this kind lays out children.
    #[must_use]
    pub fn is_container(self) -> bool {
        matches!(
            self,
            Self::Stack
                | Self::Grid
                | Self::Overlay
                | Self::Scroll
                | Self::Collection
                | Self::Surface
        )
    }

    /// The kind's wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Grid => "grid",
            Self::Overlay => "overlay",
            Self::Scroll => "scroll",
            Self::Collection => "collection",
            Self::Surface => "surface",
            Self::Text => "text",
            Self::Image => "image",
            Self::Input => "input",
            Self::Spacer => "spacer",
            Self::Separator => "separator",
            Self::Custom => "custom",
            // A distinct string, so a driver response, the semantic tree and
            // the undrawn report all name which kind is on screen without
            // inspecting the payload (`contracts/draw-list.md` §7).
            Self::Canvas => "canvas",
        }
    }
}

/// Per-axis clamp and flexibility priority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AxisConstraint {
    /// Lower bound on the response, logical units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f32>,
    /// Upper bound on the response, logical units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f32>,
    /// Distribution priority. Higher groups negotiate first and give last.
    #[serde(skip_serializing_if = "is_zero")]
    pub priority: i32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &i32) -> bool {
    *v == 0
}

impl AxisConstraint {
    /// Clamp `value` into this constraint. `min` wins over `max` when the two
    /// contradict, so a node never reports less than its declared minimum.
    #[must_use]
    pub fn clamp(self, value: f32) -> f32 {
        let mut out = value;
        if let Some(max) = self.max {
            out = out.min(max);
        }
        if let Some(min) = self.min {
            out = out.max(min);
        }
        out
    }
}

/// Per-axis constraints on a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Constraints {
    /// Horizontal clamp and priority.
    #[serde(skip_serializing_if = "is_default_axis")]
    pub horizontal: AxisConstraint,
    /// Vertical clamp and priority.
    #[serde(skip_serializing_if = "is_default_axis")]
    pub vertical: AxisConstraint,
}

fn is_default_axis(v: &AxisConstraint) -> bool {
    *v == AxisConstraint::default()
}

impl Constraints {
    /// The constraint on `axis`.
    #[must_use]
    pub fn axis(self, axis: crate::geom::Axis) -> AxisConstraint {
        match axis {
            crate::geom::Axis::Horizontal => self.horizontal,
            crate::geom::Axis::Vertical => self.vertical,
        }
    }

    /// Clamp a size on both axes.
    #[must_use]
    pub fn clamp_size(self, size: crate::geom::Size) -> crate::geom::Size {
        crate::geom::Size::new(self.horizontal.clamp(size.w), self.vertical.clamp(size.h))
    }
}

/// An interaction intent a node emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Interaction {
    /// Primary activation.
    Click,
    /// Press-move-release.
    Drag,
    /// Pointer entry and exit.
    Hover,
    /// Can take keyboard focus.
    Focus,
    /// Accepts text input while focused.
    TextEdit,
    /// Accepts scroll deltas.
    Scroll,
    /// Accepts key events while focused.
    Key,
}

/// What a node is, for the semantic tree.
///
/// The wire form is the role name; a host role prints as `custom:<name>`
/// (`contracts/semantic-tree.md`). Additive-only after first release.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A region of the screen.
    Pane,
    /// An ordered collection.
    List,
    /// One entry in a list.
    ListItem,
    /// An activatable control.
    Button,
    /// An editable text field.
    TextInput,
    /// Static text.
    Label,
    /// A state readout. Always carries a label: the shape-plus-text rule.
    Status,
    /// One tab in a tab list.
    Tab,
    /// A row of tabs.
    TabList,
    /// A scrollable region.
    Scroll,
    /// A layer above the main content.
    Overlay,
    /// A transient message.
    Toast,
    /// A focus-trapping surface.
    Dialog,
    /// A visual rule.
    Separator,
    /// A picture.
    Image,
    /// A determinate or indeterminate progress readout.
    Progress,
    /// A hierarchy.
    Tree,
    /// One entry in a tree.
    TreeItem,
    /// A grid of rows and cells.
    Table,
    /// One row of a table.
    Row,
    /// One cell of a row.
    Cell,
    /// A host-defined role, exposed under its own name.
    Custom(String),
}

/// Every built-in role name, in declaration order. Gates and documentation
/// read this; [`Role::parse`] and [`Role::as_wire`] are the two halves that
/// must agree with it, and `role_names_round_trip` is what makes them.
pub const ROLE_NAMES: &[&str] = &[
    "pane",
    "list",
    "listitem",
    "button",
    "textinput",
    "label",
    "status",
    "tab",
    "tablist",
    "scroll",
    "overlay",
    "toast",
    "dialog",
    "separator",
    "image",
    "progress",
    "tree",
    "treeitem",
    "table",
    "row",
    "cell",
];

impl Role {
    /// The role's wire name. `Custom("gutter")` prints as `custom:gutter`.
    #[must_use]
    pub fn as_wire(&self) -> String {
        match self {
            Self::Custom(name) => format!("custom:{name}"),
            other => (*other.builtin_name().expect("non-custom role has a name")).to_owned(),
        }
    }

    /// The static wire name of a built-in role, or `None` for [`Role::Custom`].
    #[must_use]
    pub fn builtin_name(&self) -> Option<&'static str> {
        let name = match self {
            Self::Pane => "pane",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Button => "button",
            Self::TextInput => "textinput",
            Self::Label => "label",
            Self::Status => "status",
            Self::Tab => "tab",
            Self::TabList => "tablist",
            Self::Scroll => "scroll",
            Self::Overlay => "overlay",
            Self::Toast => "toast",
            Self::Dialog => "dialog",
            Self::Separator => "separator",
            Self::Image => "image",
            Self::Progress => "progress",
            Self::Tree => "tree",
            Self::TreeItem => "treeitem",
            Self::Table => "table",
            Self::Row => "row",
            Self::Cell => "cell",
            Self::Custom(_) => return None,
        };
        Some(name)
    }

    /// Parse a wire name.
    ///
    /// # Errors
    /// Returns the unrecognised name. `custom:` with an empty suffix is
    /// rejected: an unnamed host role would collide with every other one.
    pub fn parse(wire: &str) -> Result<Self, String> {
        if let Some(name) = wire.strip_prefix("custom:") {
            return if name.is_empty() {
                Err(wire.to_owned())
            } else {
                Ok(Self::Custom(name.to_owned()))
            };
        }
        match wire {
            "pane" => Ok(Self::Pane),
            "list" => Ok(Self::List),
            "listitem" => Ok(Self::ListItem),
            "button" => Ok(Self::Button),
            "textinput" => Ok(Self::TextInput),
            "label" => Ok(Self::Label),
            "status" => Ok(Self::Status),
            "tab" => Ok(Self::Tab),
            "tablist" => Ok(Self::TabList),
            "scroll" => Ok(Self::Scroll),
            "overlay" => Ok(Self::Overlay),
            "toast" => Ok(Self::Toast),
            "dialog" => Ok(Self::Dialog),
            "separator" => Ok(Self::Separator),
            "image" => Ok(Self::Image),
            "progress" => Ok(Self::Progress),
            "tree" => Ok(Self::Tree),
            "treeitem" => Ok(Self::TreeItem),
            "table" => Ok(Self::Table),
            "row" => Ok(Self::Row),
            "cell" => Ok(Self::Cell),
            other => Err(other.to_owned()),
        }
    }
}

impl Serialize for Role {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.as_wire())
    }
}

impl<'de> Deserialize<'de> for Role {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let wire = String::deserialize(de)?;
        Self::parse(&wire).map_err(|bad| {
            serde::de::Error::custom(format!(
                "unknown semantic role {bad:?}; expected one of {} or custom:<name>",
                ROLE_NAMES.join(", ")
            ))
        })
    }
}

/// The **shape** keyboard focus takes on a node.
///
/// Three figures, and the choice is made by the component that builds the
/// node. A host must not guess it from the role: a select field, a dropdown
/// field, a date field and a toggletip trigger are all [`Role::Button`] and
/// are all wells a person picks into, while a menu item is [`Role::Button`]
/// and is not. Until 2026-09-05 `gorgon-petra-egui` decided the figure from
/// `role == TextInput`, in two places, and every field-shaped button
/// underlined into the surface it had just opened.
///
/// *Whose* rect the figure is drawn on is the other half of the question,
/// and it is [`FocusShownOn`]'s. The two are orthogonal: a node declares a
/// shape and a target, and every pairing is legal.
///
/// # Which figure a control wears
///
/// The operator set this policy on 2026-09-05, after a pass that left all
/// forty-two rows wearing [`Self::Border`] by inheriting it: *"it varies by
/// type (and should be overloadable per element) ... side bars on text
/// inputs. side bars on toggle tip and buttons. underlines are preferred to
/// boxes, boxes are just for when underlines stick too far off and look
/// bad."*
///
/// * [`Self::Sides`] — anything a person types into, and anything they press
///   that stands on its own: a field, a search, a button, a toggletip
///   trigger, a link.
/// * [`Self::BarUnder`] — **the default**, and what everything else wears: a
///   control with clear run below it, a checkbox row, a tag, a tile.
/// * [`Self::Border`] — the exception, for a control packed against a
///   neighbour. A bar hangs five units below the bottom edge
///   ([`crate::token::FocusRing::gap`] plus
///   [`crate::token::FocusRing::thickness`]), so on a stacked row it lands
///   *on* the next row instead of in empty space. That is the operator's
///   "sticks too far off": a menu item, a table row, a tab, a calendar cell.
///
/// The preference is the default on purpose. A component that declares
/// nothing gets the figure asked for most often, and every departure carries
/// its reason at the declaration — which is the opposite of the state this
/// policy replaced, where the exception was the default and no shipped
/// component declared anything.
///
/// Wire form is the kebab-case variant name; absent means [`Self::BarUnder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusFigure {
    /// A closed ring on the rect's own edge: a [`crate::token::FocusRing::stroke`]
    /// accent band with a [`crate::token::FocusRing::halo`] ground band
    /// immediately inside it, both following the node's corner radius.
    ///
    /// Carbon's one focus figure: `@include focus-outline('outline')` is
    /// `outline: 2px solid $focus; outline-offset: -2px`
    /// (`utilities/_focus-outline.scss:29`), used 75 times across 62
    /// component files. Petra keeps it as the **exception** rather than the
    /// default, per the policy above: it is what a control wears when it is
    /// packed against a neighbour and a bar under it would land on that
    /// neighbour. The halo is Carbon's too — a primary
    /// button focuses with `box-shadow: inset 0 0 0 $button-outline-width
    /// $button-focus-color, inset 0 0 0 $button-border-width $background`
    /// (`components/button/_mixins.scss:133`) — and Petra needs it more
    /// sharply, because `focus.ring` is byte-identical to `accent.primary`
    /// and a primary button's fill *is* `accent.primary`.
    ///
    /// Contained by construction: neither band leaves the rect, so a row in
    /// a dense list shows focus without painting into its neighbours. That
    /// containment is the whole reason it is the figure for packed rows.
    Border,
    /// A bar under the rect: [`crate::token::FocusRing::thickness`] tall,
    /// [`crate::token::FocusRing::gap`] below the bottom edge, and as wide
    /// as the control's **leading label** — see
    /// [`crate::focus::marked_rect`], which answers the width for both bar
    /// figures and says why it is not a fraction of the control.
    ///
    /// Carbon has no focus underline anywhere, so this is a Petra figure.
    /// It is nonetheless **the default**: the operator's rule is *"underlines
    /// are preferred to boxes"*, and the figure a component gets by
    /// declaring nothing should be the one wanted most often. A departure
    /// from it wants a reason; staying on it does not.
    ///
    /// It needs run below the node. Five units of it — [`crate::token::FocusRing::gap`]
    /// plus [`crate::token::FocusRing::thickness`] — and a control that does
    /// not have that much clear space under it wants [`Self::Border`]
    /// instead.
    ///
    /// It was briefly on all three tab variants, on a misread of
    /// `components/tabs/_tabs.scss:596`. That line is under `// Item
    /// Selected` and binds `$border-interactive`; Carbon focuses a tab with
    /// `focus-outline('outline')` at `:497-499`. Two accent marks two units
    /// apart, told apart only by width, was the result. See
    /// `component::tabs`' `a_tabs_ring_marks_edges_its_indicator_does_not`
    /// for why an indicator on one edge does not crowd a ring on four.
    #[default]
    BarUnder,
    /// Bars outside the rect's left and right edges:
    /// [`crate::token::FocusRing::thickness`] wide,
    /// [`crate::token::FocusRing::hug_gap`] clear of each edge, exactly the
    /// rect's height.
    ///
    /// A field well, and — by the operator's rule of 2026-09-05 — a button,
    /// a toggletip trigger and a link as well. Also a Petra figure: Carbon
    /// puts `focus-outline('outline')` on a text input too
    /// (`components/text-input/_text-input.scss:55`).
    ///
    /// Its virtue on a standalone control is that it takes no vertical room.
    /// A button in a row of buttons has neighbours to its left and right and
    /// clear space above and below; a bar under it is fine, but the sides
    /// read harder against the button's own filled edge, and on an inline
    /// link the bar would sit exactly where `link_inline`'s own underline
    /// already is.
    Sides,
    /// A bar on the rect's **own bottom edge, inside it**:
    /// [`crate::token::FocusRing::thickness`] tall, over the control's
    /// leading label ([`crate::focus::marked_rect`]). The same stripe
    /// [`Self::BarUnder`] draws, moved up out of the neighbour's rect and
    /// into the node's own — the seat is the **only** thing that differs
    /// between the two, which is what
    /// `the_two_bars_differ_only_in_where_they_sit` holds.
    ///
    /// **The figure for a control that stacks flush.** Contained by
    /// construction, exactly as [`Self::Border`] is, so it can never land on
    /// the row below and never leaves a clip equal to the node's own rect.
    /// It is also still an underline, which is the shape the operator asks
    /// for and the shape a box is not.
    ///
    /// It exists because the library had no such figure and paid for that
    /// four times. A packed row wants an underline; [`Self::BarUnder`] hangs
    /// five units into its neighbour; the only contained figure was a box;
    /// so every packed component took the box, and each round of "fewer
    /// boxes, please" moved one to [`Self::BarUnder`], collided, and moved
    /// it back. `list_row`, `menu_item`, `tree_item`, `structured_list_row`,
    /// the data table's rows, the dropdown's options, the date picker's
    /// cells and five `ui_shell` rows all carry the same sentence at their
    /// declaration — *rows stack flush, so a bar under one lands on the
    /// next* — which is this figure's specification, written eight times
    /// over before the figure existed.
    ///
    /// It casts no shadow. [`Self::BarUnder`]'s
    /// [`crate::token::focus::BAR_SHADOW_TOKEN`] seats a bar on the card it
    /// hangs *over*; this one hangs over nothing, because it sits inside the
    /// fill it marks.
    BarInside,
}

impl FocusFigure {
    /// Whether this is the default figure, so the wire form can omit it.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::BarUnder
    }

    /// Whether this figure's width comes from the control's leading label
    /// rather than from the control's own rect.
    ///
    /// True for the two bars, false for the ring and the brackets: an
    /// underline marks a word, while a ring and a pair of brackets mark a
    /// whole control. [`crate::focus::marked_rect`] is the one caller and
    /// carries the reasoning.
    #[must_use]
    pub fn marks_the_label(&self) -> bool {
        matches!(self, Self::BarUnder | Self::BarInside)
    }
}

/// **Whose** rect a node's [`FocusFigure`] is drawn on.
///
/// Orthogonal to the shape. Two of these point away from the node that holds
/// focus, and they point opposite ways: [`Self::OnWell`] up the ancestor
/// chain, [`Self::OnHead`] down into the subtree. Both fall back to the
/// focused node's own rect rather than going blind.
///
/// Wire form is the kebab-case variant name; absent means [`Self::Own`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FocusShownOn {
    /// This node's own rect. Buttons, tabs, radios, menu items, list rows.
    #[default]
    Own,
    /// This node's own rect, and this node is the **hull** any descendant
    /// declaring [`Self::OnWell`] is shown on. A field well: a text input,
    /// a select or dropdown field, a date field, a toggletip trigger.
    Well,
    /// The nearest ancestor declaring [`Self::Well`], not this node.
    ///
    /// The `Input` leaf inside a search or number well: the leaf holds focus
    /// and the well shows it, the way Carbon's `:focus-within` puts
    /// `.cds--search--focus` on the wrapper and never on the `<input>`. A
    /// button inside the same well — a number stepper — keeps [`Self::Own`],
    /// because Carbon gives it its own outline (`_number-input.scss:178`).
    /// With no such ancestor the leaf shows focus on itself rather than
    /// going blind.
    OnWell,
    /// The nearest descendant declaring [`Self::Head`], not this node.
    ///
    /// A tree item: the item holds focus and its rect spans its whole
    /// expanded subtree, so an indicator on its own rect wraps the last
    /// grandchild. Carbon draws the ring on the item's head row and never on
    /// the item — `.cds--tree-node:focus > .cds--tree-node__label`
    /// (`_treeview.scss:59`), a direct-child selector, with `:focus` itself
    /// given `outline: none` on the line above.
    ///
    /// The mirror of [`Self::OnWell`], which points at an ancestor. With no
    /// such descendant the node shows focus on itself rather than going blind.
    OnHead,
    /// The head row a focused ancestor declaring [`Self::OnHead`] draws on.
    ///
    /// A marker and nothing more: the node carrying it is not interactive
    /// and never holds focus itself. Focused anyway, it shows focus on its
    /// own rect, which is what it is.
    Head,
}

impl FocusShownOn {
    /// Whether this is the default target, so the wire form can omit it.
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::Own
    }
}

/// Semantic declarations carried by a node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Semantics {
    /// What this node is. Required on interactive nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    /// Human-readable name. Required on interactive nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Current value for inputs and status readouts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Declared disabled state.
    #[serde(skip_serializing_if = "is_false")]
    pub disabled: bool,
    /// Declared read-only state: the node shows a value it will not let this
    /// author edit.
    ///
    /// Not a softer `disabled`, and not implemented as one
    /// (`contracts/interaction-state.md` §5). A read-only node keeps its place
    /// in focus order, keeps its focus ring, and keeps answering `Hover`; what
    /// it drops is the interaction it will not honour, declared once in
    /// [`ViewNode::interactions`] rather than refused a second time deeper in.
    /// The contract also calls declaring both this and `disabled` a defect;
    /// the audit rule that reports it is not written yet, so today the two
    /// coexist silently.
    #[serde(skip_serializing_if = "is_false")]
    pub read_only: bool,
    /// Declared skeleton state: this node is a placeholder for content that
    /// has not arrived.
    ///
    /// Outranks every other state in `contracts/interaction-state.md` §4's
    /// ladder: a skeleton is not a disabled control and not an empty one, it
    /// is the shape of a control that is still loading, so hover, press and
    /// disabled styling all give way to it. The flag is carried, digested and
    /// projected here; the ladder that acts on it lands with the state
    /// resolver.
    #[serde(skip_serializing_if = "is_false")]
    pub skeleton: bool,
    /// Declared selected state.
    #[serde(skip_serializing_if = "is_false")]
    pub selected: bool,
    /// Declared expanded state, for expandables only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    /// Declared staleness: the projection behind this node is past its
    /// freshness bound.
    #[serde(skip_serializing_if = "is_false")]
    pub stale: bool,
    /// The shape keyboard focus takes on this node. Declared by the
    /// component, never inferred from [`Self::role`]; see [`FocusFigure`].
    #[serde(skip_serializing_if = "FocusFigure::is_default")]
    pub focus_figure: FocusFigure,
    /// Which rect that shape is drawn on. Orthogonal to the shape; see
    /// [`FocusShownOn`].
    #[serde(skip_serializing_if = "FocusShownOn::is_default")]
    pub focus_shown_on: FocusShownOn,
    /// When an ancestor draws a bar figure, the bar spans **this** node
    /// rather than the ancestor's own content.
    ///
    /// A third, separate question from the other two: [`FocusFigure`] says
    /// what shape, [`FocusShownOn`] says on whose rect, and this says how
    /// wide. It narrows only the horizontal run — the bar still seats on the
    /// rect [`FocusShownOn`] chose, so a row keeps its stripe on its own
    /// bottom edge while the stripe spans only the row's title.
    ///
    /// # Why a component has to say it
    ///
    /// [`crate::focus::marked_rect`] otherwise takes the control's whole
    /// content, and that is right for a toggle, a tag and a checkbox, whose
    /// parts are all *the control*. It is wrong for a row with a **trailing
    /// affordance**. An accordion header is 868 wide with an 79-wide title
    /// at x 308 and a chevron at x 1128, so its content run is 836 — a
    /// stripe that spans the row and reads as the container's own rule,
    /// which is the picture the content rule replaced.
    ///
    /// Nothing geometric separates that chevron from a tag's dismiss cross,
    /// which *is* part of its control. Both are a trailing canvas a gap away
    /// from the label. The difference is what the control means by them, so
    /// the control is what says.
    #[serde(default, skip_serializing_if = "is_false")]
    pub focus_run: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(v: &bool) -> bool {
    !*v
}

/// A reference to a transition definition by name.
///
/// The tree holds a name, not a definition: `contracts/view-tree.md` keeps the
/// tree plain data, and the registry that resolves the name lives in
/// [`crate::anim`]. An unresolvable name is a tree-acceptance error.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransitionRef(pub String);

impl TransitionRef {
    /// A reference to the named definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The definition name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// One element of a surface description.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewNode {
    /// What this node is.
    pub kind: NodeKind,
    /// Identity within the parent. Duplicate sibling keys refuse the tree.
    pub key: Key,
    /// Kind-specific parameters.
    #[serde(default, skip_serializing_if = "is_default_props")]
    pub props: Props,
    /// Clamps and distribution priority.
    #[serde(default, skip_serializing_if = "is_default_constraints")]
    pub constraints: Constraints,
    /// Declared animation for this node's animatable changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionRef>,
    /// Interaction intents this node emits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<Interaction>,
    /// Semantic declarations.
    #[serde(default, skip_serializing_if = "is_default_semantics")]
    pub semantics: Semantics,
    /// Marks a deliberately endless animation, excluded from settle.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ambient: bool,
    /// Ordered children. Container kinds only.
    ///
    /// Shared via `Arc` so a host can hand back the exact allocation a
    /// previous frame placed and let the engine skip re-placing that
    /// subtree (`Arc::ptr_eq` is the proof of "unchanged" —
    /// `.agents/notes/proposed/architecture/2026-08-22-petra-incremental-frames.md`).
    /// Serde's `rc` feature serializes each `Arc` independently: a
    /// round-tripped tree does **not** preserve sharing, and that is fine —
    /// a deserialized tree has no previous frame to be shared with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Arc<ViewNode>>,
}

fn is_default_props(v: &Props) -> bool {
    *v == Props::default()
}

fn is_default_constraints(v: &Constraints) -> bool {
    *v == Constraints::default()
}

fn is_default_semantics(v: &Semantics) -> bool {
    *v == Semantics::default()
}

impl ViewNode {
    /// A node of `kind` with `key` and nothing else declared.
    pub fn new(kind: NodeKind, key: impl Into<Key>) -> Self {
        Self {
            kind,
            key: key.into(),
            props: Props::default(),
            constraints: Constraints::default(),
            transition: None,
            interactions: Vec::new(),
            semantics: Semantics::default(),
            ambient: false,
            children: Vec::new(),
        }
    }

    /// Replace the props.
    #[must_use]
    pub fn with_props(mut self, props: Props) -> Self {
        self.props = props;
        self
    }

    /// Replace the constraints.
    #[must_use]
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Replace the children, building a fresh `Arc` for each.
    ///
    /// Correct but slower than [`ViewNode::with_shared_children`] when the
    /// caller already holds an `Arc` a previous frame used: rebuilding an
    /// identical subtree here always yields a new allocation, so it can
    /// never be pointer-equal to what the engine cached, and the subtree is
    /// re-placed rather than reused.
    #[must_use]
    pub fn with_children(mut self, children: Vec<ViewNode>) -> Self {
        self.children = children.into_iter().map(Arc::new).collect();
        self
    }

    /// Append one child, wrapping it in a fresh `Arc`.
    ///
    /// Correct but slower than [`ViewNode::child_shared`] when the caller
    /// already holds an `Arc` a previous frame used: rebuilding an
    /// identical subtree here always yields a new allocation, so it can
    /// never be pointer-equal to what the engine cached, and the subtree is
    /// re-placed rather than reused.
    #[must_use]
    pub fn child(mut self, child: ViewNode) -> Self {
        self.children.push(Arc::new(child));
        self
    }

    /// Replace the children with subtrees the caller already holds.
    ///
    /// Handing back the same `Arc` a previous frame used is what lets the
    /// engine skip re-placing that subtree (`Arc::ptr_eq` proves it is
    /// unchanged). Rebuilding an identical subtree with
    /// [`ViewNode::with_children`] instead is always correct, only slower.
    #[must_use]
    pub fn with_shared_children(mut self, children: Vec<Arc<ViewNode>>) -> Self {
        self.children = children;
        self
    }

    /// Append one child subtree the caller already holds.
    ///
    /// Handing back the same `Arc` a previous frame used is what lets the
    /// engine skip re-placing that subtree (`Arc::ptr_eq` proves it is
    /// unchanged). Rebuilding an identical subtree with [`ViewNode::child`]
    /// instead is always correct, only slower.
    #[must_use]
    pub fn child_shared(mut self, child: Arc<ViewNode>) -> Self {
        self.children.push(child);
        self
    }

    /// Declare interactions and the semantics they require.
    #[must_use]
    pub fn interactive(
        mut self,
        role: Role,
        label: impl Into<String>,
        intents: &[Interaction],
    ) -> Self {
        self.semantics.role = Some(role);
        self.semantics.label = Some(label.into());
        self.interactions = intents.to_vec();
        self
    }

    /// Declare the **shape** keyboard focus takes on this node.
    ///
    /// Chainable counterpart to writing `semantics.focus_figure` directly,
    /// so a component built in expression position can state its figure at
    /// the point it declares its interactions rather than needing a `let
    /// mut` binding to reach back into. Omitting it leaves
    /// [`FocusFigure::BarUnder`], which is the operator's preferred figure —
    /// see [`FocusFigure`] for which control wears which.
    #[must_use]
    pub fn with_focus_figure(mut self, figure: FocusFigure) -> Self {
        self.semantics.focus_figure = figure;
        self
    }

    /// Declare **whose** rect this node's focus figure is drawn on.
    ///
    /// Chainable counterpart to writing `semantics.focus_shown_on` directly,
    /// for the same reason as [`Self::with_focus_figure`].
    #[must_use]
    pub fn with_focus_run(mut self) -> Self {
        self.semantics.focus_run = true;
        self
    }

    /// Chainable counterpart to writing `semantics.focus_shown_on` directly,
    #[must_use]
    pub fn with_focus_shown_on(mut self, shown_on: FocusShownOn) -> Self {
        self.semantics.focus_shown_on = shown_on;
        self
    }

    /// Replace the semantics.
    #[must_use]
    pub fn with_semantics(mut self, semantics: Semantics) -> Self {
        self.semantics = semantics;
        self
    }

    /// Name a transition definition.
    #[must_use]
    pub fn with_transition(mut self, name: impl Into<String>) -> Self {
        self.transition = Some(TransitionRef::new(name));
        self
    }

    /// Mark this node as hosting a deliberately endless animation.
    #[must_use]
    pub fn with_ambient(mut self, ambient: bool) -> Self {
        self.ambient = ambient;
        self
    }

    /// Whether this node declares any interaction.
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        !self.interactions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Constraints, Interaction, NodeKind, Role, ViewNode};
    use crate::testing::gap;
    use crate::tree::props::Props;

    /// The wire form is the shape a Lua table produces: declared keys only.
    #[test]
    fn a_bare_node_serializes_to_two_keys() {
        let node = ViewNode::new(NodeKind::Stack, "root");
        let json = serde_json::to_string(&node).unwrap();
        assert_eq!(json, r#"{"kind":"stack","key":"root"}"#);
    }

    #[test]
    fn a_populated_tree_round_trips() {
        let tree = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                spacing: gap(8.0),
                ..Props::default()
            })
            .child(ViewNode::new(NodeKind::Text, "title").with_props(Props {
                text: Some("Fibers".into()),
                ..Props::default()
            }))
            .child(ViewNode::new(NodeKind::Input, "filter").interactive(
                Role::TextInput,
                "Filter fibers",
                &[Interaction::Focus, Interaction::TextEdit],
            ));
        let json = serde_json::to_string(&tree).unwrap();
        let back: ViewNode = serde_json::from_str(&json).unwrap();
        assert_eq!(back, tree);
    }

    #[test]
    fn unknown_node_fields_are_refused() {
        let err = serde_json::from_str::<ViewNode>(r#"{"kind":"stack","key":"a","kids":[]}"#)
            .unwrap_err();
        assert!(err.to_string().contains("kids"), "{err}");
    }

    /// `NodeKind` is a closed thirteen-variant enum with
    /// `#[serde(rename_all = "lowercase")]`, so a kind name outside the
    /// thirteen is unrepresentable in Rust and is refused here, at the
    /// deserialization boundary, before a tree exists for
    /// `crate::tree::validate` to walk. That is why `Violation` carries no
    /// `UnknownKind` variant: one would be dead code, since nothing can ever
    /// construct a `ViewNode` whose `kind` holds such a value for `validate`
    /// to see (`specs/003-petra-layout-engine/tasks.md`, T009).
    #[test]
    fn an_unknown_node_kind_is_refused_at_deserialization() {
        let err = serde_json::from_str::<ViewNode>(r#"{"kind":"widget","key":"a"}"#).unwrap_err();
        assert!(err.to_string().contains("widget"), "{err}");
    }

    #[test]
    fn custom_roles_carry_their_name() {
        assert_eq!(Role::Button.as_wire(), "button");
        assert_eq!(Role::Custom("gutter".into()).as_wire(), "custom:gutter");
        assert_eq!(
            Role::parse("custom:gutter"),
            Ok(Role::Custom("gutter".into()))
        );
        assert!(Role::parse("custom:").is_err());
        assert!(Role::parse("widget").is_err());
    }

    /// `as_wire` and `parse` are two hand-written halves of one table. This is
    /// what stops them drifting: every name in `ROLE_NAMES` must parse, and
    /// every parse must print back to the same name.
    #[test]
    fn role_names_round_trip() {
        for name in super::ROLE_NAMES {
            let role = Role::parse(name).unwrap_or_else(|_| panic!("{name} must parse"));
            assert_eq!(&role.as_wire(), name);
            assert_eq!(role.builtin_name(), Some(*name));
        }
        assert_eq!(super::ROLE_NAMES.len(), 21);
        assert!(Role::Custom("x".into()).builtin_name().is_none());
    }

    #[test]
    fn min_beats_max_when_they_contradict() {
        let c = Constraints {
            horizontal: super::AxisConstraint {
                min: Some(50.0),
                max: Some(10.0),
                priority: 0,
            },
            vertical: super::AxisConstraint::default(),
        };
        assert_eq!(c.horizontal.clamp(30.0), 50.0);
    }

    #[test]
    fn container_kinds_are_the_six_that_place_children() {
        let containers: Vec<&str> = [
            NodeKind::Stack,
            NodeKind::Grid,
            NodeKind::Overlay,
            NodeKind::Scroll,
            NodeKind::Collection,
            NodeKind::Surface,
            NodeKind::Text,
            NodeKind::Image,
            NodeKind::Input,
            NodeKind::Spacer,
            NodeKind::Separator,
            NodeKind::Custom,
            NodeKind::Canvas,
        ]
        .into_iter()
        .filter(|k| k.is_container())
        .map(NodeKind::as_str)
        .collect();
        assert_eq!(
            containers,
            [
                "stack",
                "grid",
                "overlay",
                "scroll",
                "collection",
                "surface"
            ]
        );
    }

    /// `canvas` is its own wire name, distinct from `custom`.
    ///
    /// `contracts/draw-list.md` §7 turns on this string: a driver response,
    /// the semantic tree and the undrawn report all name which kind is on
    /// screen from it, and if a canvas printed as `custom` every one of them
    /// would describe a digest-visible picture as a digest-blind one.
    #[test]
    fn a_canvas_is_a_second_kind_beside_custom_and_says_so() {
        assert_eq!(NodeKind::Canvas.as_str(), "canvas");
        assert_ne!(NodeKind::Canvas, NodeKind::Custom);
        assert_ne!(NodeKind::Canvas.as_str(), NodeKind::Custom.as_str());
        assert!(!NodeKind::Canvas.is_container(), "a canvas places no child");
        let node: ViewNode = serde_json::from_str(r#"{"kind":"canvas","key":"plot"}"#).unwrap();
        assert_eq!(node.kind, NodeKind::Canvas);
        assert_eq!(
            serde_json::to_string(&ViewNode::new(NodeKind::Canvas, "plot")).unwrap(),
            r#"{"kind":"canvas","key":"plot"}"#
        );
    }

    /// `child_shared` attaches the caller's own allocation, not a copy of it
    /// — that identity is what lets the engine skip re-placing an unchanged
    /// subtree (`Arc::ptr_eq` against what the previous frame placed).
    #[test]
    fn child_shared_attaches_the_same_allocation() {
        let shared = Arc::new(ViewNode::new(NodeKind::Text, "title"));
        let node = ViewNode::new(NodeKind::Stack, "root").child_shared(Arc::clone(&shared));
        assert!(Arc::ptr_eq(&shared, &node.children[0]));
    }

    /// `child` always builds a fresh `Arc`, even for an identical subtree.
    /// This pins the false-negative behaviour as intended: a host that does
    /// not opt into sharing gets a correct but non-reusable subtree, never a
    /// wrongly-reused one.
    #[test]
    fn child_does_not_share_across_two_builds() {
        let a =
            ViewNode::new(NodeKind::Stack, "root").child(ViewNode::new(NodeKind::Text, "title"));
        let b =
            ViewNode::new(NodeKind::Stack, "root").child(ViewNode::new(NodeKind::Text, "title"));
        assert!(!Arc::ptr_eq(&a.children[0], &b.children[0]));
    }
}
