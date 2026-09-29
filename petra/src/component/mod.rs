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

pub mod kit;
pub mod params;
pub mod registry;
pub mod writes;

pub(crate) use kit::{
    CARET_SIZE, CaretDirection, bind_corners, caret, pad, pin_block, stack, swatch,
};

mod accordion;
mod ai_label;
mod avatar;
mod breadcrumb;
mod button;
mod button_group;
mod chrome_strip;
mod code_snippet;
/// The colour-blindness lane at the level of a rendered component.
/// Test-only; see `crate::token::colourblind` for the arithmetic.
#[cfg(test)]
mod colourblind;
mod contained_list;
mod content_switcher;
mod context_menu;
mod controls;
mod data_table;
mod date_picker;
mod drawer;
mod dropdown;
mod field;
mod file_uploader;
mod form;
mod icon;
mod inline_loading;
mod input_group;
mod link;
mod list;
mod list_box;
mod list_row;
mod loading;
mod menu;
mod menu_button;
mod menubar;
mod modal;
mod notification;
mod number_input;
mod otp;
mod pagination;
mod popover;
mod progress;
mod progress_indicator;
mod rating;
mod search;
mod section;
mod select;
mod slider;
mod status;
mod structured_list;
mod tabs;
mod tag;
mod text;
mod textarea;
mod tile;
mod toggle_button;
mod toggletip;
mod tokens;
mod tooltip;
mod tree_view;
mod ui_shell;

pub use accordion::{
    accordion, accordion_item, accordion_item_lg, accordion_item_sm, accordion_item_spaced,
    accordion_item_spaced_lg, accordion_item_spaced_sm, accordion_item_with,
    accordion_item_with_spaced, accordion_spaced,
};
pub use ai_label::{
    ai_label, ai_label_2xs, ai_label_inline, ai_label_inline_lg, ai_label_inline_sm, ai_label_lg,
    ai_label_mini, ai_label_revert, ai_label_sm, ai_label_with_actions, ai_label_xl, ai_label_xs,
};
pub use avatar::{
    avatar, avatar_group, avatar_lg, avatar_md, avatar_with, avatar_with_image, avatar_with_status,
    avatar_xs,
};
pub use breadcrumb::{
    breadcrumb, breadcrumb_item, breadcrumb_item_current, breadcrumb_item_icon,
    breadcrumb_overflow, breadcrumb_with_separator,
};
pub use button::{
    button, button_2xl, button_lg, button_sm, button_xl, button_xs, danger_button,
    danger_ghost_button, danger_tertiary_button, ghost_button, primary_button, tertiary_button,
};
pub use button_group::{button_group, button_group_flush};
pub use chrome_strip::{DIVIDER_EXTENT, chrome_strip};
pub use code_snippet::{
    COPY_FEEDBACK, COPY_FEEDBACK_KEY, COPY_FEEDBACK_SECONDS, CodeInk, MULTI_CAP, MULTI_CAP_LINES,
    SHOW_LESS, SHOW_MORE, code_runs, code_snippet, code_snippet_copied, code_snippet_inline,
    code_snippet_multi, code_snippet_multi_capped,
};
pub use contained_list::{contained_list, contained_list_disclosed};
pub use content_switcher::{content_switcher, content_switcher_item};
pub use context_menu::context_menu;
pub use controls::{
    CheckState, checkbox, checkbox_described, checkbox_group, checkbox_indeterminate,
    checkbox_readonly, checkbox_required, checkbox_tristate, checkbox_warning, radio,
    radio_described, radio_group, radio_required, radio_warning, toggle, toggle_sm,
};
pub use data_table::{
    SortDirection, data_table, data_table_batch_action, data_table_batch_bar,
    data_table_batch_cancel, data_table_grip_row, data_table_menu, data_table_placed_weights,
    data_table_row, data_table_row_actions, data_table_row_expandable,
    data_table_row_expandable_actions, data_table_row_lg, data_table_row_md,
    data_table_row_menu_trigger, data_table_row_sm, data_table_row_xl, data_table_row_xs,
    data_table_sized, data_table_skeleton, data_table_sort_header, data_table_toolbar,
    data_table_toolbar_menu, data_table_weights_at, data_table_zebra, data_table_zebra_sized,
};
pub use date_picker::{
    Calendar, date_picker, date_picker_open, date_picker_showing, date_picker_showing_selection,
};
pub use drawer::{docked, drawer, sheet};
pub use dropdown::{
    dropdown, dropdown_lg, dropdown_open, dropdown_option, dropdown_sm, dropdown_xs,
};
pub use field::{
    field, field_described, field_fluid, field_invalid, field_labeled, field_lg, field_readonly,
    field_required, field_sm, field_validated, field_warning, hinted, labeled, valued,
};
pub use file_uploader::{
    file_uploader, file_uploader_item, file_uploader_item_edit, file_uploader_item_invalid,
    file_uploader_item_warning, file_uploader_with,
};
pub use form::form;
pub use icon::{IconBox, IconMark, IconTone, icon, icon_in, icon_toned};
pub use inline_loading::{inline_loading, inline_loading_finished};
pub use input_group::{
    input_group, input_group_seamless, input_group_with_addon, input_group_with_addon_seamless,
};
pub use link::{link, link_inline};
pub use list::{
    Bullet, BulletScheme, list_item, list_item_with, ordered_list, unordered_list,
    unordered_list_with,
};
pub use list_row::{LIST_ROW_EXTENT, list_row, list_row_with};
pub use loading::{advance_ambient_spinners, loading, loading_sm, spinner_phase};
pub use menu::{menu, menu_flyout, menu_item, menu_item_with};
pub use menu_button::menu_button;
pub use menubar::{menubar, menubar_top};
pub use modal::{modal, modal_passive};
pub use notification::{
    NotificationKind, notification, notification_actionable, notification_actionable_kind,
    notification_inline, notification_inline_kind, notification_toast, notification_toast_kind,
};
pub use number_input::{
    number_input, number_input_invalid, number_input_lg, number_input_sm, number_input_warning,
};
pub use otp::otp;
pub use pagination::{
    PaginationPicker, pagination, pagination_items, pagination_items_open, pagination_nav,
    pagination_numbers, pagination_page_size, pagination_range,
};
pub use popover::{popover, popover_with, popover_with_placement};
pub use progress::{progress, progress_sm, progress_with_helper};
pub use progress_indicator::{progress_indicator, progress_step};
pub use rating::rating;
pub use search::{search, search_lg, search_sm};
pub use section::section;
pub use select::{select, select_lg, select_open, select_sm};
pub use slider::{
    slider, slider_input_text, slider_readonly, slider_value_at, slider_value_of_input,
};
pub use status::status;
pub use structured_list::{
    structured_list, structured_list_placed_weights, structured_list_row, structured_list_sized,
    structured_list_weights_at,
};
pub use tabs::{contained_tab, contained_tab_bar, tab, tab_bar, vertical_tab, vertical_tab_bar};
pub use tag::{dismissible_tag, selectable_tag, tag, tag_lg, tag_sm, tag_status, tag_with_avatar};
pub use text::{heading, text};
pub use textarea::{
    TEXTAREA_MAX_HEIGHT, textarea, textarea_invalid, textarea_validated, textarea_warning,
};
pub use tile::{clickable_tile, expandable_tile, selectable_tile, tile};
pub use toggle_button::{toggle_button, toggle_button_group, toggle_button_icon};
pub use toggletip::{toggletip, toggletip_with};
pub use tooltip::{tooltip, tooltip_anchored};
pub use tree_view::{tree_item, tree_item_xs, tree_view};
pub use ui_shell::{
    LeftPanelMode, ui_shell_header, ui_shell_header_action, ui_shell_header_action_icon,
    ui_shell_header_menu_trigger, ui_shell_header_nav_item, ui_shell_left_panel,
    ui_shell_left_panel_divider, ui_shell_left_panel_icon_item, ui_shell_left_panel_icon_subitem,
    ui_shell_left_panel_in, ui_shell_left_panel_item, ui_shell_left_panel_rail,
    ui_shell_left_panel_subitem, ui_shell_right_panel, ui_shell_right_panel_divider,
    ui_shell_switcher, ui_shell_switcher_item,
};

use std::sync::Arc;

use crate::token::{BORDER_SUBTLE_TOKENS, FIELD_TOKENS, LAYER_TOKENS, TokenName};
use crate::tree::{InsetRefs, Interaction, Key, NodeKind, ViewNode};

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

/// Declare that a node answers a right click as well as whatever it already
/// answered.
///
/// # Why this exists at all
///
/// [`crate::tree::Interaction::SecondaryClick`] shipped with spec 009 T014
/// and **no constructor in this crate declared it**, for eleven months. The
/// one caller that wanted it, the gallery's Context menu page, reached past
/// every constructor and pushed the variant onto a built node:
///
/// ```ignore
/// let mut trigger = button(TRIGGER, "Show menu");
/// trigger.interactions.push(Interaction::SecondaryClick);
/// ```
///
/// That works in Rust and is unreachable from anywhere else. A registry
/// caller sends a **name and a parameter table**; `registry::expand_node`
/// replaces the reference with `build(..)` whole, so nothing set on the node
/// carrying the reference survives. Right click was therefore a capability
/// only a Rust host could grant, on a vocabulary both languages are supposed
/// to share.
///
/// # Why a modifier and not a named button
///
/// Operator ruling, 2026-09-19. The alternative was
/// `secondary_click_button(key, label)`, which matches how the twelve button
/// variants already spell themselves — the size and the emphasis live in the
/// name, never in an argument. It lost because it serves buttons and nothing
/// else, and the capability is not a button's. A list row, a tile or a tag
/// that wants a context menu wants exactly this and would each need their
/// own constructor.
///
/// [`disabled`] is the precedent, and it is the stronger half of the
/// argument: it already **clears** `interactions` through this same registry
/// path. A modifier that appends one variant is a smaller freedom than one
/// that removes them all.
///
/// # What it does not do
///
/// Only this node, never the subtree — the opposite of [`disabled`]. A right
/// click is aimed at one target and routes to the node under the pointer;
/// declaring it on a button's label as well would make the label a second
/// target for the same gesture, and `required_interaction_during` would find
/// two. `disabled` walks the subtree because unavailability is a fact about
/// every part of a control. Reachability is a fact about one node.
///
/// It also appends rather than replaces, so a node keeps the `Click` it
/// already declared and stays reachable by a primary press and by
/// Enter/Space. That is what the Context menu page wants: right click is the
/// documented trigger, and a plain click opens it too, so the control is not
/// keyboard-dead.
///
/// Appending twice would declare the variant twice, which no reader of
/// `interactions` expects, so a node that already declares it is returned
/// unchanged.
#[must_use]
pub fn also_secondary_click(mut node: ViewNode) -> ViewNode {
    if !node.interactions.contains(&Interaction::SecondaryClick) {
        node.interactions.push(Interaction::SecondaryClick);
    }
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

/// Re-seat a mounted subtree's fills against the card it sits on: [`on_layer`]
/// at a flat depth of 1, over the card's descendants and not the card.
///
/// [`on_layer`] re-seats one node, so a caller that needs a whole subtree
/// re-seated walks — this is that walk, named once. Depth is a flat 1 for the
/// whole subtree rather than counting nesting, which is honest about what it
/// is: the card is one step up from the page, and a component that nests its
/// own surfaces deeper needs the `Surface` node kind [`on_layer`]'s own doc
/// names as the real fix.
///
/// The card itself keeps its fill: it is the ground the content is re-seated
/// *against*, and moving the ground too would undo the step the depth
/// argument exists to take.
///
/// # Why this is a registry row and not a gallery-private walk
///
/// This walk used to live in `petra/petra-egui/src/bin/gallery/catalog.rs`
/// (`Catalog::seated`/`Catalog::seat_card`), and spec 013 T010's Lua chrome
/// mirror could not express it there: `registry::expand_node` returns
/// `build(..)` whole, [`on_layer`] is shallow, and a Lua author cannot reach
/// the nodes a constructor builds for itself. The mirror could either
/// re-implement the walk over its own authored tables — a second
/// implementation of the thing that draws, exactly what spec 013 exists to
/// remove — or call this one. Registered in `registry/atoms.rs`:
/// `catalog.rs` and a Lua `ui.seat_card{ node = … }` share one walk
/// (`.agents/notes/implemented/architecture/2026-09-18-the-gallery-chrome-reaches-lua.md`).
#[must_use]
pub fn seat_card(mut card: ViewNode) -> ViewNode {
    card.children = card
        .children
        .into_iter()
        .map(|child| Arc::new(seated(ViewNode::clone(&child), 1)))
        .collect();
    card
}

/// [`seat_card`]'s walk: children first (they carry fills of their own), then
/// the node. Recursive so a nested surface that binds a layer fill moves with
/// its parent instead of staying behind one layer down.
fn seated(mut node: ViewNode, depth: usize) -> ViewNode {
    node.children = node
        .children
        .into_iter()
        .map(|child| Arc::new(seated(ViewNode::clone(&child), depth)))
        .collect();
    on_layer(node, depth)
}

/// Put `padding` over whatever the node's own constructor baked.
///
/// **The way a caller changes a baked inset.** A constructor decides its own
/// padding — [`list_row`] binds Carbon's item inset — and every other door is
/// shut: `registry::expand_node` returns `build(...)` whole and discards the
/// fields on the node carrying a component reference, so a `props.padding`
/// written beside `ui.list_row{...}` never reaches the built node
/// (`.agents/notes/proposed/architecture/2026-09-09-registered-constructor-override-channel.md`).
/// This modifier is the narrow door through the registry for that one field;
/// the general override channel that note sizes is still the eventual answer
/// and is still open.
///
/// Its call site is the gallery's own index pane: 42 rows in 900 logical
/// units, where Carbon's item inset dropped six rows off the bottom of the
/// pane until the pane's own density was put back over it
/// (`petra/petra-egui/src/bin/gallery/catalog.rs`'s `index_row`, its Lua
/// mirror `lua_parity/catalog.lua`'s `index_row`). The four
/// selection-state fills stay in the component — this changes the inset and
/// nothing else, so a fifth selection state is still added once.
///
/// # What it deliberately does not do
///
/// Nothing beyond `props.padding`. Not the tokens, not the constraints, not
/// the semantics: a modifier that could repaint a node's fill would be a fork
/// of every component it touches, which is the same line [`on_layer`] draws
/// and deliberately does not cross.
#[must_use]
pub fn padded(mut node: ViewNode, padding: InsetRefs) -> ViewNode {
    node.props.padding = Some(padding);
    node
}

/// A hairline rule running along `axis`, in `fill`.
///
/// **The one way to write a standalone divider.** Carbon draws a `1px solid
/// $border-subtle` boundary between the rows of most of its list-shaped
/// components, and this is that line as a node of its own, for the cases
/// where the rule does not belong to something else. Where it does — a
/// table row, a field, a list option — bind an edge slot on that thing
/// instead; `crate::token::rule` describes both constructions and why there
/// are two.
///
/// # Why this takes no thickness
///
/// It used to, and that was the defect. A rule at [`crate::token::rule::MATERIAL_TOKEN`]
/// (`border.subtle`) is a two-stroke groove four **absolute device pixels**
/// deep, and seven dividers across this library each pinned their own one
/// logical unit for it. They painted flat rather than clipped, because each
/// was a childless stack carrying a `background` fill and a fill is not an
/// edge slot, so the same token grooved in a data table and drew a flat line
/// in an accordion. Both halves of that came from a number at the call site.
///
/// [`NodeKind::Separator`] removes the number: `measure_separator` answers
/// [`crate::token::rule::thickness`] for the material and
/// [`crate::layout::leaf::SEPARATOR_THICKNESS`] for anything else, and the
/// painter grooves the whole rect. An author says *what the line is*, never
/// how thick.
///
/// # The run comes from the parent's cross extent, not from its alignment
///
/// A separator measures its length off the size proposal it is offered, and
/// a stack offers every child its own full cross extent — so a rule runs the
/// container's whole width or height under `Align::Start`, `Center` and
/// `Stretch` alike. `component::tests`'s
/// `a_rule_runs_full_height_in_a_row_whatever_that_row_aligns_its_children_to`
/// measures all three off real frames, because this was believed to be a
/// Stretch-only behaviour for long enough to distort a component around it
/// (see [`super::chrome_strip`]).
///
/// What the parent still owes a rule is a cross extent worth running along.
/// A row as tall as its own tallest child gives a rule exactly that much, so
/// a strip that wants a full-height divider pins its height.
#[must_use]
pub fn rule(key: impl Into<Key>, axis: crate::geom::Axis, fill: &str) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Separator, key);
    node.props.axis = Some(axis);
    node.props
        .tokens
        .insert("background".into(), tokens::t(fill));
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

#[cfg(test)]
mod tests;
