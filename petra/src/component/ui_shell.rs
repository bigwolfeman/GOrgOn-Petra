//! `ui_shell` — Carbon UI shell header and side panels (slice-f, rows 40-42,
//! spec 005 T110).
//!
//! Three rows, three anatomies. Sources: slice-f.md "UI shell header" /
//! "UI shell left panel" / "UI shell right panel";
//! `contracts/component-anatomy.md`.
//!
//! # UI shell header ([`ui_shell_header`])
//!
//! One fixed [`MINI_UNIT_6`] (48px) horizontal bar, product-to-global
//! order:
//! 1. `menu_trigger` — [`ui_shell_header_menu_trigger`], only when the
//!    product has a collapsible left panel (Carbon "Header with sidenav").
//! 2. `name` — the product identity, [`Role::Button`] (Carbon's
//!    `.cds--header__name` is a link back to the product's home view).
//! 3. `nav` — already-built [`ui_shell_header_nav_item`]s under
//!    [`Role::List`]. Always present, empty when the caller passes none —
//!    one stable anatomy across Carbon's four header "Types" rather than a
//!    shape that branches by variant (`contracts/component-anatomy.md` §1).
//! 4. `spacer` — flexible space (`NodeKind::Spacer`), Carbon's
//!    `flex: 1 1 0%` on `.cds--header__global`.
//! 5. `actions` — already-built [`ui_shell_header_action`]s, pushed to the
//!    trailing edge by the spacer.
//!
//! # UI shell left panel ([`ui_shell_left_panel`], [`ui_shell_left_panel_rail`])
//!
//! Two of Carbon's three width variants: **Fixed** (256px, the default) and
//! **Rail** (48px, [`MINI_UNIT_6`] again — Carbon's rail width and header
//! height are the same `mini-units(6)`). **UX** is not built: its defining
//! behaviour is collapsing to zero width below the `lg` breakpoint
//! (slice-f "UI shell left panel" Variants), and a fixed-size pane has no
//! viewport breakpoint to collapse at — that is spec 004's layout call, not
//! this row's anatomy. [`ui_shell_left_panel_item`] is a branch (sub-menu,
//! children non-empty) or a leaf (flat link); [`ui_shell_left_panel_subitem`]
//! is the plain-link row inside an expanded branch — a separate constructor
//! because its type is different (`$body-compact-01` vs. the branch's
//! `$heading-compact-01`, slice-f "Key numbers"), not a variant of the same
//! row.
//!
//! # UI shell right panel ([`ui_shell_right_panel`], [`ui_shell_switcher`])
//!
//! One anchored surface, Carbon's generic "empty header panel" case
//! ([`ui_shell_right_panel`]) or its one named content type, the
//! **Switcher** ([`ui_shell_switcher`], centred children per
//! `.cds--switcher { align-items: center }`). [`ui_shell_switcher_item`]
//! ships with **no selected state**: the docs usage page states flatly
//! "there is no selected state for right panel items… the item remains
//! unselected" even for the current view, while the switcher's own SCSS
//! carries a `--selected` link style the two docs pages never reconcile
//! (slice-f "UI shell right panel" States, Unverified). Building a selected
//! state here would pick a side of an unresolved tension the ground truth
//! flags but does not settle; the stronger, general, explicitly-worded
//! usage-page rule is the one this file honours.
//!
//! # What none of the three rows build
//!
//! No icon glyph exists for hamburger, search, notification, help,
//! account, or switcher/apps — [`super::icon::IconMark`] ships exactly one
//! mark (a toggle tick) and inventing five more here would be exactly the
//! guessed-asset problem `contracts/component-anatomy.md` §3 exists to
//! stop. Every control that Carbon draws icon-only therefore surfaces its
//! *word* instead ([`ui_shell_header_menu_trigger`]'s "Open"/"Close",
//! [`ui_shell_left_panel_item`]'s "expanded"/"collapsed" — the same
//! second-channel convention [`super::tree_view`] and [`super::menu_button`]
//! already use for exactly this reason).
//!
//! The skip-to-content link (slice-f "Browser assumptions": a CSS
//! `clip: rect(0,0,0,0)`-until-focus idiom with no retained-mode
//! equivalent) is not part of any row here — a keyboard "jump to content"
//! affordance is a whole-application focus-order fact, and belongs to
//! spec 004's shell, not to one row's anatomy. The right panel's true
//! placement — docked to the viewport's right edge, `inset-block` from the
//! header to the viewport bottom (slice-f "UI shell right panel" Key
//! numbers) — is also out of reach: [`crate::tree::Anchor::Node`] anchors
//! to a *node's rect*, not a viewport edge, so [`ui_shell_right_panel`]
//! anchors to its trigger icon instead (`contracts/component-anatomy.md`
//! Open §1 leaves viewport-edge docking to `contracts/anchored-placement.md`,
//! not this contract). Responsive nav collapse below the `lg`/`md`
//! breakpoints is skipped throughout for the same reason `ui_shell_left_panel`
//! skips UX: a fixed-size pane has no breakpoint. All of the above are
//! spec 004 application-shell concerns, not Petra component vocabulary.
//!
//! Every interactive constructor sets role, label, and interactions inside
//! itself (FR-058); none takes an optional builder step to unset them.
//! Callers wrap any of them with [`super::disabled`] for the unavailable
//! state — none of the three rows documents its own disabled/read-only/
//! skeleton state (slice-f: "Does NOT have" disabled/read-only/skeleton on
//! the header or left panel shells themselves; undocumented for right
//! panel items), so none is built here.

use super::stack;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_ACTIVE, LAYER_HOVER, LAYER_SELECTED,
    LAYER_SELECTED_HOVER, SPACING_03, SPACING_05, SPACING_07, SURFACE_BASE, SURFACE_RAISED,
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING_SM, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    Align as AnchorAlign, Anchor, AxisConstraint, ClampRule, Constraints, Edge, InputPolicy,
    InsetRefs, Interaction, Key, Layer, NodeKind, Props, Role, Semantics, TextWrap, TrackSize,
    ViewNode,
};

/// Carbon `mini-units(6)` (`_functions.scss`): the header's block-size,
/// every header action / hamburger target, and the left panel's collapsed
/// rail width all key off this one number (slice-f "Key numbers", both
/// "UI shell header" and "UI shell left panel").
const MINI_UNIT_6: f32 = 48.0;

/// Header current-page accent bar thickness (`3px`, slice-f "UI shell
/// header" Key numbers).
const HEADER_ACCENT: f32 = 3.0;

/// Left panel expanded/fixed width (`mini-units(32)` = 256px).
const LEFT_PANEL_WIDTH: f32 = 256.0;
/// Left panel default link/sub-menu row height (`mini-units(4)` = 32px).
const LEFT_PANEL_ROW: f32 = 32.0;
/// Left panel selected accent bar thickness (`3px`, full item block-size).
const LEFT_PANEL_ACCENT: f32 = 3.0;

/// Right panel fixed width (`mini-units(32)` = 256px — `_header-panel.scss`
/// and the docs style page agree; no min/max spread, one width).
const RIGHT_PANEL_WIDTH: f32 = 256.0;
/// Switcher item row height (`$spacing-07` = 32px).
const SWITCHER_ROW: f32 = 32.0;
/// Switcher divider width (`224px`, fixed, not full-bleed — leaves a margin
/// inside the 256px panel).
const SWITCHER_DIVIDER_WIDTH: f32 = 224.0;

const INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

// ---------------------------------------------------------------------
// UI shell header (row 40)
// ---------------------------------------------------------------------

/// The header bar. See the module doc for anatomy and what is omitted.
pub fn ui_shell_header(
    key: impl Into<Key>,
    product_name: impl Into<String>,
    menu_trigger: Option<ViewNode>,
    nav: Vec<ViewNode>,
    actions: Vec<ViewNode>,
) -> ViewNode {
    let mut children = Vec::with_capacity(nav.len() + actions.len() + 4);
    if let Some(trigger) = menu_trigger {
        children.push(trigger);
    }
    children.push(header_name("name", product_name));

    let mut nav_list = stack("nav", Axis::Horizontal, None, nav);
    nav_list.props.align = Some(Align::Stretch);
    nav_list.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    children.push(nav_list);

    children.push(ViewNode::new(NodeKind::Spacer, "spacer"));
    children.push(stack("actions", Axis::Horizontal, None, actions));

    let mut node = stack(key, Axis::Horizontal, None, children);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.semantics = Semantics {
        role: Some(Role::Pane),
        ..Semantics::default()
    };
    node.with_constraints(pin_block(MINI_UNIT_6))
}

/// The product identity link. `label` is `product_name` (FR-058). Padding
/// is left [`SPACING_05`] (16) / right [`SPACING_07`] (32) — Carbon's
/// `0 32px 0 16px`, both edges an exact hit on the shipped ramp. No "IBM"
/// prefix split: GOrgOn is not an IBM product.
fn header_name(key: &'static str, product_name: impl Into<String>) -> ViewNode {
    let product_name = product_name.into();
    let mut caption = text("label", product_name.clone());
    caption.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_07)),
        ..InsetRefs::default()
    });
    node.interactive(Role::Button, product_name, INTENTS)
}

/// The hamburger / menu trigger. Not a caller-labelled control: the
/// accessible name and the visible second-channel word are both derived
/// from `open`, never taken as a parameter that could go stale against the
/// state that drives them. `Semantics.selected` carries the persistent
/// "panel is open" condition Carbon expresses as the `--active` class.
pub fn ui_shell_header_menu_trigger(key: impl Into<Key>, open: bool) -> ViewNode {
    let state_word = if open { "Close" } else { "Open" };
    let label = format!("{state_word} navigation menu");
    let mut caption = text("label", state_word);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut props = Props {
        align: Some(Align::Center),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    props
        .tokens
        .insert("background@selected".into(), t(SURFACE_RAISED));
    let node = ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..props
        })
        .with_children(vec![caption])
        .with_constraints(pin_square(MINI_UNIT_6));
    let mut node = node.interactive(Role::Button, label, INTENTS);
    node.semantics.selected = open;
    node
}

/// One header nav link. `current` sets `Semantics.selected` and shows a
/// [`HEADER_ACCENT`]-thick bottom accent bar plus a colour swap
/// ([`TEXT_PRIMARY`] vs. [`TEXT_MUTED`]) — never colour alone. Carbon's own
/// typography does not step weight for the current link (`$body-compact-01`
/// throughout, slice-f "Key numbers"), so this does not either.
pub fn ui_shell_header_nav_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    current: bool,
) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY));
    caption.props.tokens.insert(
        "foreground".into(),
        t(if current { TEXT_PRIMARY } else { TEXT_MUTED }),
    );
    let mut body = stack("body", Axis::Horizontal, None, vec![caption]);
    body.props.align = Some(Align::Center);
    body.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });

    let fill = current.then_some(ACCENT_PRIMARY);
    let mark = accent_mark("indicator", Axis::Horizontal, HEADER_ACCENT, fill);

    let mut props = Props {
        columns: vec![TrackSize::FitContent],
        rows: vec![
            TrackSize::Weight { weight: 1.0 },
            TrackSize::Fixed {
                value: HEADER_ACCENT,
            },
        ],
        align: Some(Align::Stretch),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));

    let node = ViewNode::new(NodeKind::Grid, key)
        .with_props(props)
        .with_children(vec![body, mark])
        .with_constraints(pin_block(MINI_UNIT_6));
    let mut node = node.interactive(Role::Button, label, INTENTS);
    node.semantics.selected = current;
    node
}

/// One header global/utility action. `label` names what the icon would
/// have said (there is no shipped icon vocabulary for it, module doc). A
/// [`MINI_UNIT_6`] square. `active` is the persistent "this action's panel
/// is open" condition (Carbon's `--active` class, `background: $layer`),
/// not a momentary press — that is `background@active` composing
/// automatically through `crate::token::state`.
pub fn ui_shell_header_action(
    key: impl Into<Key>,
    label: impl Into<String>,
    active: bool,
) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_BODY));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut props = Props {
        axis: Some(Axis::Horizontal),
        align: Some(Align::Center),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    props
        .tokens
        .insert("background@selected".into(), t(SURFACE_RAISED));
    let node = ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .with_children(vec![caption])
        .with_constraints(pin_square(MINI_UNIT_6));
    let mut node = node.interactive(Role::Button, label, INTENTS);
    node.semantics.selected = active;
    node
}

// ---------------------------------------------------------------------
// UI shell left panel (row 41)
// ---------------------------------------------------------------------

/// The Fixed left panel: [`LEFT_PANEL_WIDTH`] (256), non-collapsible.
pub fn ui_shell_left_panel(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    left_panel(key, items, LEFT_PANEL_WIDTH)
}

/// The Rail left panel: collapsed to [`MINI_UNIT_6`] (48), icon-only width.
/// Hover-to-expand is pointer-capture behaviour this constructor does not
/// drive; it ships the collapsed geometry the anatomy needs to exist at
/// all.
pub fn ui_shell_left_panel_rail(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    left_panel(key, items, MINI_UNIT_6)
}

fn left_panel(key: impl Into<Key>, items: Vec<ViewNode>, width: f32) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, items);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.semantics = Semantics {
        role: Some(Role::Pane),
        ..Semantics::default()
    };
    node.with_constraints(pin_inline(width))
}

/// A top-level left-panel row: a flat link when `children` is empty, a
/// sub-menu title when it is not. `expanded` only matters for a sub-menu —
/// its nested children mount only while `expanded` is true, and the caret
/// is the word `"expanded"`/`"collapsed"` (FR-026, matching
/// [`super::tree_view::tree_item`]'s convention). `selected` shows
/// [`LEFT_PANEL_ACCENT`] at the inline-start edge plus `Semantics.selected`.
/// Title type is [`TYPOGRAPHY_HEADING_SM`] (Carbon `$heading-compact-01`).
pub fn ui_shell_left_panel_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let is_branch = !children.is_empty();
    let row = left_panel_row(&label, TYPOGRAPHY_HEADING_SM, is_branch, expanded, selected);

    let mut parts = vec![row];
    if expanded && is_branch {
        let mut nest = stack("children", Axis::Vertical, None, children);
        nest.props.padding = Some(InsetRefs {
            left: Some(t(SPACING_07)),
            ..InsetRefs::default()
        });
        parts.push(nest);
    }

    let mut node = stack(key, Axis::Vertical, None, parts);
    bind_row_states(&mut node);
    let mut node = node.interactive(Role::Button, label, INTENTS);
    node.semantics.selected = selected;
    node.semantics.expanded = Some(expanded);
    node
}

/// A plain link nested one level under an expanded
/// [`ui_shell_left_panel_item`] sub-menu. Indent is the parent's job (the
/// nested list's own [`SPACING_07`] left inset); this row is a leaf and
/// never grows further children (Carbon documents one nesting level).
/// Type is [`TYPOGRAPHY_BODY`] (Carbon `$body-compact-01`) — a step lighter
/// than a top-level [`ui_shell_left_panel_item`] title, which is the one
/// documented difference between the two rows (slice-f "Key numbers").
pub fn ui_shell_left_panel_subitem(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let row = left_panel_row(&label, TYPOGRAPHY_BODY, false, false, selected);
    let mut node = stack(key, Axis::Vertical, None, vec![row]);
    bind_row_states(&mut node);
    let mut node = node.interactive(Role::Button, label, INTENTS);
    node.semantics.selected = selected;
    node
}

fn left_panel_row(
    label: &str,
    typography: &str,
    is_branch: bool,
    expanded: bool,
    selected: bool,
) -> ViewNode {
    let mut caption = text("label", label.to_string());
    caption.props.style = Some(t(typography));
    caption.props.wrap = Some(TextWrap::Ellipsis);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut row_parts = vec![caption];
    if is_branch {
        let disclosure = if expanded { "expanded" } else { "collapsed" };
        row_parts.push(text("chevron", disclosure));
    }
    let mut body = stack("body", Axis::Horizontal, Some(SPACING_03), row_parts);
    body.props.align = Some(Align::Center);
    body.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });

    let fill = selected.then_some(ACCENT_PRIMARY);
    let mark = accent_mark("accent", Axis::Vertical, LEFT_PANEL_ACCENT, fill);

    ViewNode::new(NodeKind::Grid, "row")
        .with_props(Props {
            columns: vec![
                TrackSize::Fixed {
                    value: LEFT_PANEL_ACCENT,
                },
                TrackSize::Weight { weight: 1.0 },
            ],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![mark, body])
        .with_constraints(pin_block(LEFT_PANEL_ROW))
}

/// The four interaction-state fills a left-panel item's outer node binds:
/// resting, hover, press, selected, and selected-hover. Bound on the
/// outer clickable node, not the inner `"row"` Grid — matching
/// [`super::tree_view::tree_item`], whose accent/background split this
/// mirrors.
fn bind_row_states(node: &mut ViewNode) {
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@active", LAYER_ACTIVE),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
}

/// A 1px rule between left-panel items. Width comes from the panel's own
/// [`Align::Stretch`]; only the block-size is pinned here, the same trick
/// [`accent_mark`] uses for a bar that must fill a cell it does not own the
/// extent of.
pub fn ui_shell_left_panel_divider(key: impl Into<Key>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    node.constraints.vertical = AxisConstraint {
        min: Some(1.0),
        max: Some(1.0),
        priority: 0,
    };
    node
}

// ---------------------------------------------------------------------
// UI shell right panel (row 42)
// ---------------------------------------------------------------------

/// A generic right panel: Carbon's "empty header panel" case. Anchored to
/// `anchor_id` (the [`ui_shell_header_action`] that opens it). `label` is
/// the panel's accessible name (FR-058-adjacent, mirroring
/// [`super::popover::popover_with`] — an overlay names itself even though
/// it is not itself a click target). See the module doc for what viewport
/// docking this anchor does not do.
pub fn ui_shell_right_panel(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    content: Vec<ViewNode>,
) -> ViewNode {
    right_panel(key, label, anchor_id, content, Align::Start)
}

/// The Switcher: a right panel whose content is centred
/// (`.cds--switcher { align-items: center }`, slice-f "Anatomy"). Build
/// `items` from [`ui_shell_switcher_item`] and space them with
/// [`ui_shell_right_panel_divider`].
pub fn ui_shell_switcher(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    right_panel(key, label, anchor_id, items, Align::Center)
}

fn right_panel(
    key: impl Into<Key>,
    label: impl Into<String>,
    anchor_id: impl Into<String>,
    content: Vec<ViewNode>,
    align: Align,
) -> ViewNode {
    let mut inner = stack("content", Axis::Vertical, None, content);
    inner.props.align = Some(align);

    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Node {
                id: anchor_id.into(),
                edge: Edge::Bottom,
                align: AnchorAlign::End,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            ..Props::default()
        })
        .child(inner);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.constraints = pin_inline(RIGHT_PANEL_WIDTH);
    node.semantics = Semantics {
        role: Some(Role::Overlay),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

/// One switcher row. No `selected` parameter — see the module doc for why.
/// Type is [`TYPOGRAPHY_HEADING_SM`] (Carbon `$heading-compact-01`, slice-f
/// "Key numbers").
pub fn ui_shell_switcher_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut props = Props {
        axis: Some(Axis::Horizontal),
        align: Some(Align::Center),
        padding: Some(InsetRefs {
            left: Some(t(SPACING_05)),
            right: Some(t(SPACING_05)),
            ..InsetRefs::default()
        }),
        ..Props::default()
    };
    props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    let node = ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .with_children(vec![caption])
        .with_constraints(pin_block(SWITCHER_ROW));
    node.interactive(Role::Button, label, INTENTS)
}

/// A divider between switcher rows. [`SWITCHER_DIVIDER_WIDTH`] (224) wide,
/// deliberately narrower than the 256px panel — Carbon leaves a margin on
/// both sides rather than running the rule edge to edge.
pub fn ui_shell_right_panel_divider(key: impl Into<Key>) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    node.constraints = Constraints {
        horizontal: AxisConstraint {
            min: Some(SWITCHER_DIVIDER_WIDTH),
            max: Some(SWITCHER_DIVIDER_WIDTH),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(1.0),
            max: Some(1.0),
            priority: 0,
        },
    };
    node
}

// ---------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------

/// A selected/current indicator bar, thickness on the cross-axis, filled
/// (or transparent) along `along`. The same shape as
/// `super::tabs::indicator_bar`: an empty `Stack`, not a `Spacer` — a
/// spacer answers an unbounded query at a huge extent and would blow a
/// `FitContent` grid track out to the viewport; an empty stack measures
/// zero on its main axis and lets [`Align::Stretch`] fill the cell it sits
/// in instead.
fn accent_mark(key: &'static str, along: Axis, thickness: f32, fill: Option<&str>) -> ViewNode {
    let mut node = stack(key, along, None, vec![]);
    if let Some(name) = fill {
        node.props.tokens.insert("background".into(), t(name));
    }
    match along {
        Axis::Horizontal => {
            node.constraints.vertical = AxisConstraint {
                min: Some(thickness),
                max: Some(thickness),
                priority: 0,
            };
        }
        Axis::Vertical => {
            node.constraints.horizontal = AxisConstraint {
                min: Some(thickness),
                max: Some(thickness),
                priority: 0,
            };
        }
    }
    node
}

fn pin_block(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

fn pin_square(size: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(size),
            max: Some(size),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(size),
            max: Some(size),
            priority: 0,
        },
    }
}

fn pin_inline(w: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(w),
            max: Some(w),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HEADER_ACCENT, LEFT_PANEL_ACCENT, LEFT_PANEL_ROW, LEFT_PANEL_WIDTH, MINI_UNIT_6,
        RIGHT_PANEL_WIDTH, SWITCHER_DIVIDER_WIDTH, SWITCHER_ROW, ui_shell_header,
        ui_shell_header_action, ui_shell_header_menu_trigger, ui_shell_header_nav_item,
        ui_shell_left_panel, ui_shell_left_panel_divider, ui_shell_left_panel_item,
        ui_shell_left_panel_rail, ui_shell_left_panel_subitem, ui_shell_right_panel,
        ui_shell_right_panel_divider, ui_shell_switcher, ui_shell_switcher_item,
    };
    use crate::component::tokens::{
        ACCENT_PRIMARY, LAYER_SELECTED, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
    };
    use crate::tree::{Anchor, Edge, Interaction, NodeKind, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn has_key(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| has_key(child, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    // -- header --------------------------------------------------------

    #[test]
    fn header_is_a_pane_pinned_to_48_with_a_bottom_border() {
        let node = ui_shell_header("header", "GOrgOn", None, vec![], vec![]);
        assert_eq!(node.semantics.role, Some(Role::Pane));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.constraints.vertical.min, Some(MINI_UNIT_6));
        assert_eq!(node.constraints.vertical.max, Some(MINI_UNIT_6));
        assert_eq!(MINI_UNIT_6, 48.0);
        assert!(!has_key(&node, "trigger"));
        let name = named(&node, "name");
        assert_eq!(name.semantics.role, Some(Role::Button));
        assert_eq!(name.semantics.label.as_deref(), Some("GOrgOn"));
    }

    #[test]
    fn header_always_has_nav_and_actions_even_when_empty() {
        let node = ui_shell_header("header", "GOrgOn", None, vec![], vec![]);
        let nav = named(&node, "nav");
        assert_eq!(nav.semantics.role, Some(Role::List));
        assert!(nav.children.is_empty());
        let actions = named(&node, "actions");
        assert!(actions.children.is_empty());
        assert!(has_key(&node, "spacer"));
    }

    #[test]
    fn header_hosts_menu_trigger_nav_and_actions_when_supplied() {
        let node = ui_shell_header(
            "header",
            "GOrgOn",
            Some(ui_shell_header_menu_trigger("trigger", false)),
            vec![ui_shell_header_nav_item("overview", "Overview", true)],
            vec![ui_shell_header_action("notify", "Notifications", false)],
        );
        assert_eq!(node.children[0].key.as_str(), "trigger");
        assert!(has_key(&node, "overview"));
        assert!(has_key(&node, "notify"));
    }

    #[test]
    fn menu_trigger_derives_label_and_word_from_open_never_taking_one() {
        let closed = ui_shell_header_menu_trigger("trigger", false);
        assert_eq!(closed.semantics.role, Some(Role::Button));
        assert_eq!(
            closed.semantics.label.as_deref(),
            Some("Open navigation menu")
        );
        assert!(!closed.semantics.selected);
        assert_eq!(named(&closed, "label").props.text.as_deref(), Some("Open"));
        assert_eq!(closed.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(closed.constraints.vertical.min, Some(MINI_UNIT_6));

        let open = ui_shell_header_menu_trigger("trigger", true);
        assert_eq!(
            open.semantics.label.as_deref(),
            Some("Close navigation menu")
        );
        assert!(open.semantics.selected);
        assert_eq!(named(&open, "label").props.text.as_deref(), Some("Close"));
        assert_eq!(token(&open, "background@selected"), Some(SURFACE_RAISED));
    }

    #[test]
    fn nav_item_declares_current_never_colour_alone() {
        let current = ui_shell_header_nav_item("overview", "Overview", true);
        assert_eq!(current.semantics.role, Some(Role::Button));
        assert_eq!(current.semantics.label.as_deref(), Some("Overview"));
        assert!(current.semantics.selected);
        assert!(current.interactions.contains(&Interaction::Click));
        assert_eq!(current.constraints.vertical.min, Some(MINI_UNIT_6));
        let mark = named(&current, "indicator");
        assert_eq!(token(mark, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(mark.constraints.vertical.min, Some(HEADER_ACCENT));
        assert_eq!(HEADER_ACCENT, 3.0);
        assert_eq!(
            named(&current, "label")
                .props
                .tokens
                .get("foreground")
                .map(|t| t.as_str()),
            Some(TEXT_PRIMARY)
        );

        let resting = ui_shell_header_nav_item("overview", "Overview", false);
        assert!(!resting.semantics.selected);
        assert_eq!(
            named(&resting, "label")
                .props
                .tokens
                .get("foreground")
                .map(|t| t.as_str()),
            Some(TEXT_MUTED)
        );
        assert_eq!(token(named(&resting, "indicator"), "background"), None);
    }

    #[test]
    fn header_action_is_a_48_square_with_a_required_label() {
        let node = ui_shell_header_action("notify", "Notifications", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Notifications"));
        assert!(!node.semantics.selected);
        assert_eq!(node.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(node.constraints.horizontal.max, Some(MINI_UNIT_6));
        assert_eq!(node.constraints.vertical.min, Some(MINI_UNIT_6));
        assert_eq!(node.constraints.vertical.max, Some(MINI_UNIT_6));

        let active = ui_shell_header_action("notify", "Notifications", true);
        assert!(active.semantics.selected);
        assert_eq!(token(&active, "background@selected"), Some(SURFACE_RAISED));
    }

    // -- left panel ------------------------------------------------------

    #[test]
    fn fixed_panel_is_256_and_rail_is_48() {
        let fixed = ui_shell_left_panel("nav", vec![]);
        assert_eq!(fixed.semantics.role, Some(Role::Pane));
        assert_eq!(fixed.constraints.horizontal.min, Some(LEFT_PANEL_WIDTH));
        assert_eq!(fixed.constraints.horizontal.max, Some(LEFT_PANEL_WIDTH));
        assert_eq!(LEFT_PANEL_WIDTH, 256.0);

        let rail = ui_shell_left_panel_rail("nav", vec![]);
        assert_eq!(rail.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(rail.constraints.horizontal.max, Some(MINI_UNIT_6));
    }

    #[test]
    fn leaf_item_has_no_chevron_and_no_nested_children() {
        let node = ui_shell_left_panel_item("home", "Home", false, false, vec![]);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Home"));
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(!has_key(&node, "chevron"));
        assert!(!has_key(&node, "children"));
        assert_eq!(named(&node, "row").constraints.vertical.min, Some(LEFT_PANEL_ROW));
        assert_eq!(LEFT_PANEL_ROW, 32.0);
    }

    #[test]
    fn branch_item_mounts_children_only_while_expanded() {
        let child = ui_shell_left_panel_subitem("child", "Fibers", false);
        let expanded = ui_shell_left_panel_item(
            "kernel",
            "Kernel",
            true,
            false,
            vec![child],
        );
        assert_eq!(expanded.semantics.expanded, Some(true));
        assert_eq!(
            named(&expanded, "chevron").props.text.as_deref(),
            Some("expanded")
        );
        assert!(has_key(&expanded, "child"));

        let collapsed = ui_shell_left_panel_item(
            "kernel",
            "Kernel",
            false,
            false,
            vec![ui_shell_left_panel_subitem("child", "Fibers", false)],
        );
        assert_eq!(
            named(&collapsed, "chevron").props.text.as_deref(),
            Some("collapsed")
        );
        assert!(!has_key(&collapsed, "child"));
        assert!(!has_key(&collapsed, "children"));
    }

    #[test]
    fn selected_item_shows_the_accent_and_the_flag() {
        let node = ui_shell_left_panel_item("home", "Home", false, true, vec![]);
        assert!(node.semantics.selected);
        let mark = named(&node, "accent");
        assert_eq!(token(mark, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(mark.constraints.horizontal.min, Some(LEFT_PANEL_ACCENT));
        assert_eq!(LEFT_PANEL_ACCENT, 3.0);
        assert_eq!(token(&node, "background@selected"), Some(LAYER_SELECTED));

        let subitem = ui_shell_left_panel_subitem("child", "Fibers", true);
        assert!(subitem.semantics.selected);
        assert_eq!(subitem.semantics.role, Some(Role::Button));
        assert_eq!(token(&subitem, "background@selected"), Some(LAYER_SELECTED));
    }

    #[test]
    fn left_panel_divider_is_one_pixel_tall() {
        let node = ui_shell_left_panel_divider("rule");
        assert_eq!(node.constraints.vertical.min, Some(1.0));
        assert_eq!(node.constraints.vertical.max, Some(1.0));
    }

    // -- right panel -------------------------------------------------

    #[test]
    fn right_panel_is_an_overlay_anchored_to_its_trigger() {
        let node = ui_shell_right_panel(
            "notifications-panel",
            "Notifications",
            "notify",
            vec![],
        );
        assert_eq!(node.kind, NodeKind::Surface);
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert_ne!(node.semantics.role, Some(Role::Dialog));
        assert_eq!(node.semantics.label.as_deref(), Some("Notifications"));
        assert!(node.interactions.is_empty());
        match &node.props.anchor {
            Some(Anchor::Node { id, edge, .. }) => {
                assert_eq!(id, "notify");
                assert_eq!(*edge, Edge::Bottom);
            }
            other => panic!("expected Anchor::Node, got {other:?}"),
        }
        assert_eq!(node.constraints.horizontal.min, Some(RIGHT_PANEL_WIDTH));
        assert_eq!(node.constraints.horizontal.max, Some(RIGHT_PANEL_WIDTH));
        assert_eq!(RIGHT_PANEL_WIDTH, 256.0);
    }

    #[test]
    fn switcher_hosts_items_and_dividers_with_no_selected_state() {
        let node = ui_shell_switcher(
            "switcher",
            "App switcher",
            "apps",
            vec![
                ui_shell_switcher_item("a", "Petra"),
                ui_shell_right_panel_divider("d1"),
                ui_shell_switcher_item("b", "Inspector"),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::Overlay));
        assert!(has_key(&node, "a"));
        assert!(has_key(&node, "b"));
        let item = named(&node, "a");
        assert_eq!(item.semantics.role, Some(Role::Button));
        assert_eq!(item.semantics.label.as_deref(), Some("Petra"));
        assert_eq!(item.constraints.vertical.min, Some(SWITCHER_ROW));
        assert_eq!(SWITCHER_ROW, 32.0);
        let divider = named(&node, "d1");
        assert_eq!(divider.constraints.horizontal.min, Some(SWITCHER_DIVIDER_WIDTH));
        assert_eq!(SWITCHER_DIVIDER_WIDTH, 224.0);
    }
}
