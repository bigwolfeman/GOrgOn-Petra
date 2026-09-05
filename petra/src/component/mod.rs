//! Petra's component library: the default authoring vocabulary above the
//! primitives (FR-052, FR-058).
//!
//! `crate::tree` gives an author twelve node kinds and a bag of style
//! tokens; nothing there stops a tree from declaring a clickable node with
//! no role, or a status readout that is a colour and nothing else. This
//! module is the layer where that stops being possible. Contract C13 (see
//! `.agents/research/08-22-2026/Petra-Vocabulary-Build/PLAN.md`) fixes the
//! set at exactly thirteen components, chosen from what `gallery.rs` and the
//! inspector's planned panels actually compose rather than from what FR-052
//! could in principle mean:
//!
//! | Component | Role it sets | Interactive |
//! |---|---|---|
//! | [`button`] / [`primary_button`] | `Role::Button` | yes |
//! | [`text`] | none | no |
//! | [`heading`] | none | no |
//! | [`field`] | `Role::TextInput` | yes |
//! | [`checkbox`] | `Role::Button` + selected | yes |
//! | [`radio`] | `Role::Button` + selected | yes |
//! | [`toggle`] | `Role::Button` + selected | yes |
//! | [`tab_bar`] | `Role::TabList` | no |
//! | [`tab`] | `Role::Tab` + selected | yes |
//! | [`progress`] | `Role::Progress` | no |
//! | [`status`] | `Role::Status` | no |
//! | [`section`] | none | no |
//! | [`list_row`] | `Role::ListItem` + selected | yes |
//!
//! [`icon`] is not a fourteenth C13 component. It is a visual part a labeled
//! control composes (FR-026): a canvas that draws a named [`IconMark`], with
//! no role and no interactions of its own. The control that contains it still
//! owns the label. A swatch is not an icon.
//!
//! `Role::Dialog`, `Role::List`, and `Role::Table` each appear once in the
//! gallery and stay expressed through the primitives directly — a component
//! with one consumer is a helper, not a library member, and C13 draws the
//! line there deliberately.
//!
//! Thirteen **components**. [`primary_button`] is [`button`] with the
//! accent spent on it, not a fourteenth thing: same role, same
//! interactions, same body (`button::labelled`); what differs is two
//! colours and an elevation. [`icon`] is a mark those shapes carry, not
//! a fifteenth. C13 fixed the set of *shapes an author can compose*.
//! Emphasis is a property of one of those shapes rather than a new one.
//! The alternative — an `emphasis: Emphasis` parameter on `button` —
//! was refused because every existing call site would have had to name a
//! default, which is a lot of churn to express "this one is louder".
//!
//! # How FR-058 is enforced
//!
//! Every interactive entry above takes its label as a required, non-`Option`
//! positional parameter and sets its own role and interactions inside the
//! function body — never through a builder step a caller could choose not
//! to take. There is no `Button::new(key)` anywhere in this module, and no
//! `with_role`/`with_label`/`set_role`/`set_label` method exists to undo a
//! role or label after construction (gate C1-4): the only way to get a
//! `ViewNode` out of [`button`], [`field`], [`checkbox`], [`radio`],
//! [`toggle`], [`tab`], or [`list_row`] is to supply the label they demand.
//! [`status`] enforces the parallel FR-015 guarantee by taking a whole,
//! already-validated [`crate::token::StatusToken`] rather than reassembling
//! one from separate colour/shape/text arguments — see that module's doc
//! for why a constructor alone was once not enough.
//!
//! # Where the token names live
//!
//! [`tokens`] is the one place a design-token name is spelled as a literal
//! string. Every component reaches for a constant there rather than writing
//! `"spacing-04"` at its own call site, so a rename in the shipped
//! vocabulary is a one-line fix instead of a sweep across a dozen files —
//! and [`tokens`]'s own test proves every one of those constants is
//! actually declared in [`crate::token::standard_vocabulary`] (gate C1-7).
//!
//! # What this module does not do
//!
//! It does not replace the primitives: every function here returns a
//! `ViewNode` built from `NodeKind::Stack`, `Grid`, `Text`, `Input`,
//! `Spacer`, or `Canvas` — kinds from `crate::tree` — and any layout this
//! library does not cover (an arbitrary grid, a custom surface) is still
//! composed from those primitives directly, which stay public (gate C1-10).

mod accordion;
mod ai_label;
mod breadcrumb;
mod button;
mod code_snippet;
mod contained_list;
mod content_switcher;
mod controls;
mod data_table;
mod date_picker;
mod dropdown;
mod field;
mod file_uploader;
mod form;
mod icon;
mod inline_loading;
mod link;
mod list;
mod list_box;
mod list_row;
mod loading;
mod menu;
mod menu_button;
mod modal;
mod notification;
mod number_input;
mod pagination;
mod popover;
mod progress;
mod progress_indicator;
mod search;
mod section;
mod select;
mod slider;
mod status;
mod structured_list;
mod tabs;
mod tag;
mod text;
mod tile;
mod toggletip;
mod tokens;
mod tooltip;
mod tree_view;
mod ui_shell;

pub use accordion::{accordion, accordion_item, accordion_item_lg, accordion_item_sm};
pub use ai_label::{
    ai_label, ai_label_2xs, ai_label_inline, ai_label_inline_lg, ai_label_inline_sm, ai_label_lg,
    ai_label_mini, ai_label_revert, ai_label_sm, ai_label_with_actions, ai_label_xl, ai_label_xs,
};
pub use breadcrumb::{breadcrumb, breadcrumb_item, breadcrumb_item_current};
pub use button::{
    button, button_lg, button_sm, button_xs, danger_button, danger_ghost_button,
    danger_tertiary_button, ghost_button, primary_button, tertiary_button,
};
pub use code_snippet::{code_snippet, code_snippet_inline, code_snippet_multi};
pub use contained_list::{contained_list, contained_list_disclosed};
pub use content_switcher::{content_switcher, content_switcher_item};
pub use controls::{
    CheckState, checkbox, checkbox_group, checkbox_indeterminate, checkbox_readonly,
    checkbox_tristate, radio, radio_group, toggle, toggle_sm,
};
pub use data_table::{
    data_table, data_table_row, data_table_row_expandable, data_table_row_lg, data_table_row_md,
    data_table_row_sm, data_table_row_xl, data_table_row_xs, data_table_sort_header,
    data_table_zebra,
};
pub use date_picker::{date_picker, date_picker_open};
pub use dropdown::{dropdown, dropdown_open, dropdown_option};
pub use field::{
    field, field_fluid, field_invalid, field_labeled, field_lg, field_readonly, field_sm,
    field_validated, labeled,
    valued,
};
pub use file_uploader::{file_uploader, file_uploader_item};
pub use form::form;
pub use icon::{IconBox, IconMark, IconTone, icon, icon_in, icon_toned};
pub use inline_loading::{inline_loading, inline_loading_finished};
pub use link::{link, link_inline};
pub use list::{list_item, list_item_with, ordered_list, unordered_list};
pub use list_row::list_row;
pub use loading::{loading, loading_sm, spinner_phase};
pub use menu::{menu, menu_item};
pub use menu_button::menu_button;
pub use modal::{modal, modal_passive};
pub use notification::{
    notification, notification_actionable, notification_inline, notification_toast,
};
pub use number_input::{number_input, number_input_invalid, number_input_lg, number_input_sm};
pub use pagination::{PaginationPicker, pagination, pagination_items, pagination_items_open};
pub use popover::{popover, popover_with};
pub use progress::{progress, progress_sm, progress_with_helper};
pub use progress_indicator::{progress_indicator, progress_step};
pub use search::{search, search_lg, search_sm};
pub use section::section;
pub use select::{select, select_lg, select_open, select_sm};
pub use slider::{
    slider, slider_input_text, slider_readonly, slider_value_at, slider_value_of_input,
};
pub use status::status;
pub use structured_list::{
    structured_list, structured_list_row, structured_list_sized, structured_list_weights_at,
};
pub use tabs::{contained_tab, contained_tab_bar, tab, tab_bar, vertical_tab, vertical_tab_bar};
pub use tag::{dismissible_tag, selectable_tag, tag, tag_lg, tag_sm};
pub use text::{heading, text};
pub use tile::{clickable_tile, expandable_tile, selectable_tile, tile};
pub use toggletip::toggletip;
pub use tooltip::tooltip;
pub use tree_view::{tree_item, tree_item_xs, tree_view};
pub use ui_shell::{
    ui_shell_header, ui_shell_header_action, ui_shell_header_action_icon,
    ui_shell_header_menu_trigger, ui_shell_header_nav_item, ui_shell_left_panel,
    ui_shell_left_panel_divider, ui_shell_left_panel_item, ui_shell_left_panel_rail,
    ui_shell_left_panel_subitem, ui_shell_right_panel, ui_shell_right_panel_divider,
    ui_shell_switcher, ui_shell_switcher_item,
};

use std::sync::Arc;

use crate::geom::Axis;
use crate::token::{BORDER_SUBTLE_TOKENS, FIELD_TOKENS, LAYER_TOKENS, TokenName};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, NodeKind, Props, ViewNode};

/// The deepest seat [`on_layer`] will honour.
///
/// The shipped layer set has four entries and a control needs two of them —
/// one for the ground it sits on, one for itself — so the deepest seat that
/// still has a step left in it is index 2 (`surface.layer-two` under
/// `surface.layer-three`). That covers page under card under control, which
/// is every nesting `gallery.rs` and the inspector's panels actually build.
///
/// A deeper `depth` is clamped to this rather than allowed to run off the
/// end of the set, because the alternative is worse in a specific way: the
/// end of the array is `surface.layer-three` twice, so a node and its ground
/// would resolve to the **same colour** and the control would vanish. A
/// clamped seat draws the deepest step the ramp can express; an unclamped
/// one draws nothing at all and reports success.
pub const MAX_LAYER_DEPTH: usize = 2;

/// Declare `node` and everything under it unavailable.
///
/// The library's disabled variant, and the replacement for the composition
/// every caller was writing by hand — clear the interactions, set the flag,
/// and then reach for a third channel because the first two are invisible.
/// The third channel used to be `Props.opacity`, which is
/// `contracts/interaction-state.md` §5's named example of the thing this
/// exists to stop: a whole subtree faded at paint time is a state no gate can
/// read, no driver can query, and no theme can retune.
///
/// # What it does
///
/// * `Semantics.disabled = true` on every node in the subtree, and
/// * `interactions.clear()` on every node in the subtree.
///
/// # Why the whole subtree
///
/// `disabled` is a per-node flag with no inheritance — `layout::semantics_of`
/// reads each node's own declaration — and a component's chrome and its label
/// are two nodes. Setting the flag on the wrapper alone would leave
/// [`button`]'s label resolving its *enabled* `foreground`, so an unavailable
/// button would draw live ink on a flat card. Every node in the subtree is
/// unavailable, so every node says so.
///
/// # Where the two visible channels are
///
/// Neither is here, and that is the point (FR-010: never colour alone, and
/// never opacity alone):
///
/// * the **ink** comes from the `@disabled` token family the component
///   already declared ([`button`] binds `foreground@disabled`), resolved by
///   `crate::token::state`'s precedence chain;
/// * the **elevation** is dropped by the painter for any node whose resolved
///   rank is disabled, which is the channel that survives a reader who cannot
///   separate the colours at all.
///
/// So this function declares a fact and the two channels follow from it,
/// rather than a caller painting the fact three times and hoping.
#[must_use]
pub fn disabled(mut node: ViewNode) -> ViewNode {
    node.semantics.disabled = true;
    node.interactions.clear();
    // `Arc::unwrap_or_clone` rather than a walk over `&mut`: children are
    // shared handles, and a shared subtree that is disabled in one place and
    // live in another has to become two subtrees. The clone is what makes it
    // two. It costs this subtree its pointer-identity against the previous
    // frame — `layout::reuse` compares children by `Arc::ptr_eq` — which is
    // the right trade: a control that just became unavailable is not a
    // subtree worth carrying over unchanged.
    node.children = node
        .children
        .into_iter()
        .map(|child| Arc::new(disabled(Arc::unwrap_or_clone(child))))
        .collect();
    node
}

/// Re-seat a component onto the layer it is actually being placed on.
///
/// # The problem this solves
///
/// A component cannot know how deeply it is nested, so every one of them
/// picks its fill from a two-name vocabulary: [`tokens::SURFACE_BASE`] when
/// it wants to disappear into its ground (an unselected [`tab`] or
/// [`list_row`]), [`tokens::SURFACE_RAISED`] when it wants to stand out from
/// it ([`button`], [`field`], a selected `tab`). Both choices are right on a
/// page and both are wrong on a card, where the card has already spent
/// `surface.raised`: a raised control on a raised card has no edge at all,
/// and a base-filled control on a raised card reads as the *selected* one.
///
/// `gallery.rs` carried a private fix for this that gave the control a
/// `text.muted` outline instead of a fill, with a comment naming the reason:
/// *"the third level of elevation is drawn with a line, because there is no
/// third fill to spend."* There are now four fills. This is the same rule
/// with the line taken out and the missing tones put in, and it lives in the
/// library because it is the library's vocabulary that makes it necessary.
///
/// # What it does
///
/// `depth` is the [`crate::token::LAYER_TOKENS`] index of the surface this
/// node is being placed **on**. Only the node's own `background` binding is
/// rewritten, and only when it names one of the two tones above:
///
/// | bound background | becomes |
/// |---|---|
/// | `surface.base` | `LAYER_TOKENS[depth]` — the ground itself, so the node disappears into it |
/// | `surface.raised` / `surface.layer-one` | `LAYER_TOKENS[depth + 1]` — one step ahead of the ground |
/// | `surface.layer-two` / `surface.layer-three` | `LAYER_TOKENS[depth + 2]` / `[depth + 3]`, clamped |
/// | anything else | untouched |
///
/// "Anything else" is load-bearing, not a fall-through: an accent fill or a
/// status colour is a deliberate choice by whoever bound it, and a re-seating
/// pass that second-guessed those would be doing something other than what
/// its name says.
///
/// # Why an ordinal shifts too
///
/// It did not, and the reasoning was that an ordinal layer name is an
/// absolute choice the author meant. That reading makes this operator
/// non-composable, and the modal is where it showed: `modal.rs` seats its
/// Cancel one step above the dialog, so the dialog was `surface.raised` and
/// Cancel `surface.layer-two`. The gallery then re-seats the whole page at
/// depth 1 — every component sits on a card, one step up from the page —
/// which moved the dialog to `layer-two` and left Cancel where it was.
/// Measured on the capture: both `#333333`, byte for byte. The Cancel
/// button had no edge, no fill of its own and no separation from the dialog
/// it sat in, and the operator's report was "idk what I am looking at".
///
/// Every one of these names is a *distance from the ground*, so re-seating
/// has to move all of them by the same amount or it does not preserve the
/// relationships it exists to preserve.
///
/// The ramp has four steps, so `depth + 3` clamps at the top of the ramp. A component
/// that stacks three surfaces of its own and is then mounted one step up
/// loses its topmost separation. That is a real limit of a four-step ramp
/// and not something this function can paper over.
///
/// Children are not touched. That is the same line
/// [`crate::tree::ViewNode`]'s public fields already draw between composing
/// a component and forking one — reaching into [`button`]'s label node to
/// repaint it would make this function a fork of every component it is
/// applied to. A caller that needs a whole subtree re-seated calls this on
/// each node it owns.
///
/// # What it deliberately does not do
///
/// It never adds a border. The whole point of having four fills is that the
/// depth cue is tonal, and an operator that quietly re-introduced an outline
/// would put back exactly the wireframe this replaced.
///
/// # The shape this is standing in for
///
/// The right long-term home for this is the tree: a `Surface` that carries
/// its own layer index, with the presenter resolving a relative fill against
/// the nearest such ancestor. That reaches `Props`, acceptance, the frame
/// digest and every capture baseline, so it is not this change. Until then
/// the depth is an argument at the call site, which is honest about the fact
/// that somebody has to know it.
#[must_use]
pub fn on_layer(mut node: ViewNode, depth: usize) -> ViewNode {
    let depth = depth.min(MAX_LAYER_DEPTH);
    let step = match node
        .props
        .tokens
        .get("background")
        .map(TokenName::as_str)
        .unwrap_or_default()
    {
        tokens::SURFACE_BASE => 0,
        tokens::SURFACE_RAISED | "surface.layer-one" => 1,
        "surface.layer-two" => 2,
        "surface.layer-three" => 3,
        _ => return node,
    };
    // The index, not the argument: `MAX_LAYER_DEPTH` clamps `depth` so that
    // `depth + 1` stays in range, and an ordinal can ask for three more.
    let reseated = LAYER_TOKENS[(depth + step).min(LAYER_TOKENS.len() - 1)];
    node.props
        .tokens
        .insert("background".into(), tokens::t(reseated));
    node
}

/// A hairline rule across the inline axis, in `fill`.
///
/// Carbon draws a `1px solid $border-subtle` boundary between the rows of
/// most of its list-shaped components, and binding the shared `"border"`
/// token instead gets a four-sided box — `pagination.rs`'s own comment says
/// so, and the UI shell header had exactly that defect. This is the one
/// shape that draws one line.
///
/// An empty `Stack`, not a `Spacer`: a spacer answers an unbounded query at
/// a huge extent and would blow a `FitContent` grid track out to the
/// viewport, where an empty stack measures zero on its main axis and lets
/// [`crate::geom::Align::Stretch`] fill the cell it sits in.
///
/// `ui_shell::accent_mark`, `tabs::indicator_bar` and two places in
/// `pagination.rs` each grew their own copy of this before it had a home.
/// They should come here; they are held open by other work as this lands.
#[must_use]
pub fn rule(key: impl Into<Key>, thickness: f32, fill: &str) -> ViewNode {
    let mut node = stack(key, crate::geom::Axis::Horizontal, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), tokens::t(fill));
    node.constraints.vertical = crate::tree::AxisConstraint {
        min: Some(thickness),
        max: Some(thickness),
        priority: 0,
    };
    node
}

/// The three token names one seat in the layer stack binds.
///
/// Returned as a triple rather than three separate calls because the two
/// rules FR-004a states are *relationships between* these names, and a caller
/// that fetched them one at a time could satisfy each call and still bind an
/// inconsistent set. See [`layer_tokens`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayerTokens {
    /// The fill of the surface at this seat.
    pub background: &'static str,
    /// The fill of an input sitting **on** that surface.
    pub field: &'static str,
    /// The boundary drawn around that input.
    pub border: &'static str,
}

/// FR-004a's two layering rules, as arithmetic on one seat number.
///
/// # The two rules
///
/// Carbon states them in prose, quoted exactly: *"A field is considered a
/// layer on top of the background it is placed on, for example a field placed
/// on a `$layer-02` background will use `$field-03`. Border tokens however,
/// pair with its same number, for example `$field-03` pairs with
/// `$border-strong-03` in a text input."* So:
///
/// ```text
/// background := LAYER_TOKENS[n]           (n = 0 is `surface.base`)
/// field      := FIELD_TOKENS[n]           i.e. `field-0{n+1}`, one ahead
/// border     := BORDER_SUBTLE_TOKENS[n]   i.e. pairs with the field's number
/// ```
///
/// The border rule is the one that gets lost in prose, because "pairs with
/// its same number" is a statement about the *field's* number and not the
/// background's. Written as two array reads at the same index it is hard to
/// get wrong and trivial to check —
/// [`tests::the_layer_triple_puts_the_field_one_ahead_and_the_border_beside_it`]
/// reads every seat's triple and fails if either ordinal slips.
///
/// # Why this resolves at construction and not at paint
///
/// Carbon resolves both rules from CSS ancestry. Petra has a tree but no
/// cascade, and the alternative — an ambient depth counter consumed by the
/// painter — was weighed and declined: the frame digest hashes the token
/// names a node binds, so a node re-seated from one layer to another already
/// digests differently under this scheme and for free, where a paint-time
/// counter would need a **new digest input** and re-baseline the published
/// frame-identity reference and all five parity vectors. It also makes
/// FR-024's *"re-seating is a single operation"* fall out of one argument.
///
/// `depth` is clamped to [`MAX_LAYER_DEPTH`], for the reason recorded there:
/// past the end of the ramp a node and its ground resolve to the same colour
/// and the node vanishes while every check still passes.
///
/// # No component calls this yet, on purpose
///
/// [`on_layer`] is still how a component is re-seated, and it rewrites only
/// the `background` binding. Moving the library onto this triple binds
/// `field` and `border` names that no component binds today, which moves
/// every component's frame digest and every stored capture —
/// `contracts/token-vocabulary.md` §11 puts that at step 7, one flag day
/// rather than forty-two. This is the arithmetic landing ahead of it, proved
/// total, so the flag day is a rebinding and not also a design argument.
#[must_use]
pub fn layer_tokens(depth: usize) -> LayerTokens {
    let depth = depth.min(MAX_LAYER_DEPTH);
    LayerTokens {
        background: LAYER_TOKENS[depth],
        field: FIELD_TOKENS[depth],
        border: BORDER_SUBTLE_TOKENS[depth],
    }
}

/// A `Stack` on `axis`, gapped by `spacing`, with no other props set.
///
/// Every component with more than one visual part is built from this — the
/// one place `NodeKind::Stack` is spelled inside the library, so a bug in
/// how a row or column is assembled has one place to be found and fixed
/// rather than a dozen.
pub(crate) fn stack(
    key: impl Into<Key>,
    axis: Axis,
    spacing: Option<&str>,
    children: Vec<ViewNode>,
) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(axis),
            spacing: spacing.map(tokens::t),
            ..Props::default()
        })
        .with_children(children)
}

/// A fixed-extent, unlabelled rectangle: the drawn box every swatch-shaped
/// piece of chrome in the library is built from (a checkbox's box, a
/// toggle's knob and pad, a status dot, a progress fill and track).
///
/// Never exported. A bare coloured box carries no semantics of its own —
/// FR-058 has nothing to say about it because nothing here is interactive
/// or labelled — which is exactly why it is a building block a component
/// composes rather than a component the library ships on its own.
pub(crate) fn swatch(
    key: impl Into<Key>,
    w: f32,
    h: f32,
    background: Option<&str>,
    border: Option<&str>,
    radius: Option<&str>,
) -> ViewNode {
    let mut props = Props::default();
    if let Some(name) = background {
        props.tokens.insert("background".into(), tokens::t(name));
    }
    if let Some(name) = border {
        props.tokens.insert("border".into(), tokens::t(name));
    }
    if let Some(name) = radius {
        props.tokens.insert("radius".into(), tokens::t(name));
    }
    ViewNode::new(NodeKind::Spacer, key)
        .with_props(props)
        .with_constraints(Constraints {
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

/// [`InsetRefs::symmetric`] over two of this module's spacing constants,
/// named the way `InsetRefs::symmetric` itself is: horizontal first.
pub(crate) fn pad(horizontal: &str, vertical: &str) -> InsetRefs {
    InsetRefs::symmetric(tokens::t(horizontal), tokens::t(vertical))
}

/// Logical extent of a [`caret`] on both axes: Carbon's 16px glyph box.
pub(crate) const CARET_SIZE: f32 = 16.0;

/// Which way a [`caret`] points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaretDirection {
    /// Expanded: the triangle points down.
    Down,
    /// Collapsed: the triangle points right.
    Right,
}

/// A disclosure caret: one filled triangle in [`tokens::ICON_SECONDARY`] on a
/// 16×16 canvas, pointing [`CaretDirection::Down`] when the thing it fronts
/// is open and [`CaretDirection::Right`] when it is shut.
///
/// Never exported, and not an [`IconMark`]. It predates the icon vocabulary
/// growing past `Check`: [`IconMark::ChevronDown`] and
/// [`IconMark::ChevronUp`] exist now (2026-09-04), and this is the shape a
/// tree branch and an expandable tile still draw instead of spelling the
/// word `expanded` next to their label. The pending swap is
/// `icon_toned(key, IconMark::ChevronDown, IconTone::Secondary)` for the
/// tile and a `ChevronRight` mark for the tree, then delete this; that is
/// the tile and tree rows' owners' change. Like [`swatch`] it carries
/// no semantics of its own: the branch or tile that composes it owns the
/// `Semantics.expanded` fact, and the revealed children are the channel a
/// reader who cannot see the triangle still gets (FR-026).
///
/// Geometry, canvas-local: an 8-wide, 4-tall isoceles triangle centred in
/// the box, which is the CaretDown glyph's own proportion. The points are
/// whole units so nothing in the fill lands on a half-pixel edge and gets
/// snapped off centre at 1x (see
/// `.agents/notes/proposed/bug-fix/2026-09-04-a-half-pixel-inset-snaps-a-small-mark-off-centre.md`).
///
/// # Panics
/// Never in practice: one three-vertex convex path is inside every draw-list
/// bound. A panic here means an edit broke convexity, which is a defect.
pub(crate) fn caret(key: impl Into<Key>, direction: CaretDirection) -> ViewNode {
    use crate::draw::{ColorRef, Command, DrawList, Paint, PathVerb};
    use crate::geom::Point;

    let (a, b, c) = match direction {
        CaretDirection::Down => (
            Point::new(4.0, 6.0),
            Point::new(12.0, 6.0),
            Point::new(8.0, 10.0),
        ),
        CaretDirection::Right => (
            Point::new(6.0, 4.0),
            Point::new(6.0, 12.0),
            Point::new(10.0, 8.0),
        ),
    };
    let paint = Paint::filled(ColorRef::Token(
        tokens::t(tokens::ICON_SECONDARY).as_str().to_owned(),
    ));
    let list = DrawList::new(vec![Command::Path {
        verbs: vec![
            PathVerb::MoveTo(a),
            PathVerb::LineTo(b),
            PathVerb::LineTo(c),
        ],
        closed: true,
        paint,
    }])
    .unwrap_or_else(|err| panic!("caret draw list refused: {err}"));
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(CARET_SIZE),
                max: Some(CARET_SIZE),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(CARET_SIZE),
                max: Some(CARET_SIZE),
                priority: 0,
            },
        })
}

#[cfg(test)]
mod tests;
