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
//! # UI shell left panel ([`ui_shell_left_panel_in`])
//!
//! **A docked navigation region with width modes**, not a list in a box.
//! Every modifier in `_side-nav.scss:65-117` sets `inline-size` and nothing
//! else, so [`LeftPanelMode`] is the component: Rail 48, Fixed 256,
//! Expandable 0↔256 on the header hamburger's one boolean
//! (`HeaderContainer.js:24-35`), Hidden 0. One tree, four widths, and
//! `overflow: hidden` deciding how much of each row survives — which the
//! panel gets from being a `Grid`, whose cells clip.
//!
//! Rail's hover-to-expand (`SideNav.js:34-102`, a 100 ms `enterDelayMs`) is
//! not built: nothing in the interaction layer expresses a delayed state
//! transition yet. **UX** is not built either — its defining behaviour is
//! collapsing to zero width below the `lg` breakpoint, and a fixed-size
//! pane has no viewport breakpoint to collapse at. Both are spec 004's
//! layout call, not this row's anatomy.
//!
//! [`ui_shell_left_panel_item`] is a branch (sub-menu, children non-empty)
//! or a leaf (flat link), and [`ui_shell_left_panel_icon_item`] is the same
//! row with Carbon's `__icon` slot filled — the one Rail needs, because 48px
//! of clipped label is nothing to look at.
//! [`ui_shell_left_panel_subitem`] is the plain-link row inside an expanded
//! branch — a separate constructor because its type is different
//! (`$body-compact-01` vs. the branch's `$heading-compact-01`, slice-f "Key
//! numbers"), not a variant of the same row.
//!
//! # UI shell right panel ([`ui_shell_right_panel`], [`ui_shell_switcher`])
//!
//! **A docked region on the trailing edge**, opened by a header action:
//! `position: fixed; inset-block: mini-units(6) 0; inset-inline-end: 0`,
//! width 0 shut and 256 open (`_header-panel.scss`). It is not a popover,
//! which is what this file built until the operator said so; see
//! [`right_panel`] for what changed and what the change costs. Carbon's
//! generic "empty header panel" case is [`ui_shell_right_panel`]; its one
//! named content type is the **Switcher** ([`ui_shell_switcher`], centred
//! children per `.cds--switcher { align-items: center }`), a flat list of
//! destinations you pick one of.
//!
//! [`ui_shell_switcher_item`] **does** carry a selected state, reversing
//! this module's earlier reading of the docs usage page; the reasoning, and
//! the sentence it outranks, are on that function.
//!
//! # Glyphs
//!
//! The controls Carbon draws icon-only draw Carbon's own glyphs
//! ([`super::icon::IconMark`], traced from `@carbon/icons-react`) and keep
//! their word as the accessible name, never as body text:
//! [`ui_shell_header_menu_trigger`] is `Menu` / `Close` at Carbon's 20px
//! header box (`HeaderMenuButton.tsx` renders both at `size={20}`), a
//! [`ui_shell_header_action`] is `Notification`, `Search` or `Switcher` at
//! the same box, and [`ui_shell_left_panel_item`]'s sub-menu chevron is
//! `ChevronDown` / `ChevronUp` at 16 (`.cds--side-nav__submenu-chevron >
//! svg`, `convert.to-rem(16px)`, turned 180° when expanded). Help and
//! account have no glyph yet; a header action with any other label still
//! surfaces the word, the second-channel convention [`super::tree_view`]
//! uses for its caret.
//!
//! The skip-to-content link (slice-f "Browser assumptions": a CSS
//! `clip: rect(0,0,0,0)`-until-focus idiom with no retained-mode
//! equivalent) is not part of any row here — a keyboard "jump to content"
//! affordance is a whole-application focus-order fact, and belongs to
//! spec 004's shell, not to one row's anatomy.
//!
//! Neither panel pins itself to a viewport edge, and neither should:
//! `crate::tree::Anchor` has `Node`, `Sibling`, `Point` and `Viewport`, and
//! `Viewport` centres — there is no viewport-edge dock
//! (`contracts/component-anatomy.md` Open §1 leaves that to
//! `contracts/anchored-placement.md`). Both panels are therefore plain flow
//! nodes with Carbon's own extents, and the caller states where the region
//! goes. A gallery page does that with one `Grid` standing in for the
//! viewport, a 48-unit header row over a content row; the real application
//! shell (spec 004) does it with the window. Responsive nav collapse below
//! the `lg`/`md` breakpoints is skipped throughout for the same reason
//! `LeftPanelMode` skips UX: a fixed-size pane has no breakpoint.
//!
//! Every interactive constructor sets role, label, and interactions inside
//! itself (FR-058); none takes an optional builder step to unset them.
//! Callers wrap any of them with [`super::disabled`] for the unavailable
//! state — none of the three rows documents its own disabled/read-only/
//! skeleton state (slice-f: "Does NOT have" disabled/read-only/skeleton on
//! the header or left panel shells themselves; undocumented for right
//! panel items), so none is built here.

use super::icon::{IconBox, IconMark, IconTone, icon_in, icon_toned};
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_ACTIVE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER,
    SPACING_03, SPACING_05, SPACING_07, SURFACE_BASE, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING_SM, t,
};
use super::{pin_block, stack};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, FocusFigure, FocusShownOn, InsetRefs, Intent,
    Interaction, Justify, Key, NodeKind, Phase, Props, Role, Semantics, TextWrap, TrackSize,
    ViewNode,
};

/// Spec 010: product home / one-shot header name.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// Spec 010: nav / panel / switcher rows — exclusive among siblings.
const SELECTS_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Select,
    phase: Phase::OnRelease,
};

/// Spec 010: menu trigger / header action panel open.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
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
/// A left-panel row's own inline padding (`padding: 0 mini-units(2)`,
/// `_side-nav.scss:214` and `:350`) = 16, and therefore the inline offset a
/// top-level label sits at.
const LEFT_PANEL_INSET: f32 = 16.0;
/// A nested link's `padding-inline-start` (`mini-units(4)`,
/// `_side-nav.scss:314-318`) = 32.
const LEFT_PANEL_NEST_INSET: f32 = 32.0;
/// A nested link's `padding-inline-start` **inside an icon-bearing item**
/// (`mini-units(9)`, `_side-nav.scss:321-324`) = 72. Off the shipped
/// spacing ramp on purpose: Carbon states the side nav's boxes in
/// `mini-units`, not in `$spacing-*`, and 72 has no ramp step
/// (`token/shipped.rs` runs 48, 64, 80). See [`gutter`].
const LEFT_PANEL_ICON_NEST_INSET: f32 = 72.0;
/// A row icon's trailing margin (`mini-units(3)`, `_side-nav.scss:415-417`)
/// = 24.
const LEFT_PANEL_ICON_GAP: f32 = 24.0;

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
    // V6: was `None` spacing, the same shape as the `NameKind`/`12Decrement
    // Increment` defects V2 and V4 fixed — two adjacent actions ("Notifica-
    // tions", "App switcher") rendered as one glued word,
    // "NotificationsApp switcher". `SPACING_05` (16px) is the header's own
    // horizontal clearance figure, SOURCED slice-f.md:157 "Header link /
    // sub-menu / sub-menu-item padding: 0 16px (mini-units(2) = 16px)".
    // No gap. Carbon's `.cds--header__global` is a plain flex row with
    // `justify-content: flex-end` and no `gap`, and its actions are 48x48
    // cells butted against each other (slice-f.md:150). The `SPACING_05` that
    // used to be here was V6's fix for two *word* actions rendering as one
    // glued "NotificationsApp switcher"; that padding now lives on the word
    // action itself, where Carbon puts it, so the row does not have to push
    // two icons 16px apart to keep two words legible.
    children.push(stack("actions", Axis::Horizontal, None, actions));

    let mut bar = stack("bar", Axis::Horizontal, None, children);
    bar.props.align = Some(Align::Stretch);

    // V6: a bottom-only rule, not the 4-sided box the shared `"border"`
    // token always paints (`pagination.rs`'s own comment: "binding
    // `\"border\"` gets a 4-sided box — grep confirms it"). The header
    // previously bound `"border"` directly on this Stack; with every child
    // packed edge-to-edge (`None` spacing) and most of them opaque, that box
    // was invisible everywhere a child's own background covered it and
    // showed through as a stray top-AND-bottom hairline only where a child
    // painted no background of its own (`name`, `spacer`) — the "stray
    // horizontal rules above and below the nav area" the picture showed.
    // Carbon draws exactly one line: `border-block-end: 1px solid
    // $border-subtle`, SOURCED slice-f.md:154. A real 1-row-tall divider,
    // the same Grid-row shape `ui_shell_header_nav_item`'s own `indicator`
    // already uses in this file, replaces the token.
    let divider = accent_mark("divider", Axis::Horizontal, 1.0, Some(BORDER_SUBTLE));

    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            rows: vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Fixed { value: 1.0 },
            ],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![bar, divider])
        .with_constraints(pin_block(MINI_UNIT_6));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.semantics = Semantics {
        role: Some(Role::Pane),
        ..Semantics::default()
    };
    node
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
    // `Border` on every control in the **header**, and on the left panel's
    // rows. Not on the right panel's switcher rows, which have the run for a
    // bar and keep it.
    //
    // The header is a 48-unit bar whose bottom edge *is* the top of the page,
    // so a bar hung two units below a header control paints on the page
    // rather than in the header. Measured on the UI shell right panel page:
    // `bar/name`'s bar at y 305 lands on `shell-content/shell-page`, and the
    // switcher trigger's on `shell-switcher`. The left panel's rows have the
    // same trouble for the ordinary reason — they stack closer than the five
    // units a bar needs.
    //
    // `BarInside` answers both without a box, which is the operator's call of
    // 2026-09-06 for this row. The stripe sits on the control's own bottom
    // edge, so it is inside the header bar by construction and cannot reach
    // the page under it or the row under it.
    node.interactive(Role::Button, product_name, INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .with_focus_figure(FocusFigure::BarInside)
}

/// The hamburger / menu trigger. Not a caller-labelled control: the
/// accessible name and the glyph ([`IconMark::Menu`] shut,
/// [`IconMark::Close`] open, Carbon's `HeaderMenuButton` at `size={20}`)
/// are both derived from `open`, never taken as a parameter that could go
/// stale against the state that drives them. `Semantics.selected` carries
/// the persistent "panel is open" condition Carbon expresses as the
/// `--active` class.
pub fn ui_shell_header_menu_trigger(key: impl Into<Key>, open: bool) -> ViewNode {
    let state_word = if open { "Close" } else { "Open" };
    let label = format!("{state_word} navigation menu");
    let caption = icon_in(
        "glyph",
        if open {
            IconMark::Close
        } else {
            IconMark::Menu
        },
        IconBox::Header,
        IconTone::Primary,
    );
    // The same 48x48 icon button as any other header utility (slice-f.md:155
    // measures the trigger and the action with one figure), so it is built by
    // the same function rather than by a copy of it: the copy had drifted, and
    // was still parking its glyph flush against the button's inline-start
    // edge after the actions were centred.
    header_action_sized(key, label, caption, open, HeaderActionFit::Square)
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
        padding: Some(InsetRefs {
            top: Some(t(SPACING_05)),
            ..InsetRefs::default()
        }),
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
    // `BarUnder`, and this row alone among the header's controls.
    //
    // `header_name` explains why every other one is `BarInside`: the header
    // bar's bottom edge is the top of the page, so a bar hung two units
    // below a header control paints on the page. That is still true here.
    // What is also true here and nowhere else in the header is that this row
    // carries a current-page accent on its own bottom edge (`mark`, three
    // units, `accent_mark("indicator", Axis::Horizontal, ...)` above), and a
    // `BarInside` stripe sits exactly on it — so the one row where focus
    // matters most, the page you are already on, showed no focus at all.
    //
    // The operator's rule of 2026-09-06 settles the trade: an element that
    // already has a blue line at its bottom when selected takes the bar
    // below it. The bar reaches three units onto the page and is legible
    // there; an invisible focus ring is not legible anywhere.
    let mut node = node
        .interactive(Role::Button, label, INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE)
        .with_focus_figure(FocusFigure::BarUnder);
    node.semantics.selected = current;
    node
}

/// One header global/utility action, named by one of Carbon's documented
/// utility labels. A [`MINI_UNIT_6`] square. `active` is the persistent
/// "this action's panel is open" condition (Carbon's `--active` class,
/// `background: $layer`), not a momentary press — that is
/// `background@active` composing automatically through
/// `crate::token::state`.
///
/// The glyph follows the label through [`header_action_mark`]:
/// `"Notifications"`, `"Search"` and `"App switcher"` draw Carbon's
/// `Notification`, `Search` and `Switcher`; any other label is drawn as its
/// word, since there is no glyph for it yet. A caller that already knows
/// its glyph names it with [`ui_shell_header_action_icon`] instead; this
/// constructor exists so the label alone still produces the right picture.
pub fn ui_shell_header_action(
    key: impl Into<Key>,
    label: impl Into<String>,
    active: bool,
) -> ViewNode {
    let label = label.into();
    match header_action_mark(&label) {
        Some(mark) => ui_shell_header_action_icon(key, label, mark, active),
        None => {
            let mut caption = text("label", label.clone());
            caption.props.style = Some(t(TYPOGRAPHY_BODY));
            caption
                .props
                .tokens
                .insert("foreground".into(), t(TEXT_PRIMARY));
            header_action_sized(key, label, caption, active, HeaderActionFit::Word)
        }
    }
}

/// [`ui_shell_header_action`] with the glyph chosen by the caller.
///
/// The glyph is drawn at [`IconBox::Header`] (Carbon's 20px header box) in
/// `$icon-secondary` at rest and `$icon-primary` while `active`
/// (`_header.scss`: `.cds--header__action svg { fill: $icon-secondary }`,
/// `.cds--header__action--active > svg { fill: $icon-primary }`). `label`
/// is the accessible name and the only channel besides the picture.
pub fn ui_shell_header_action_icon(
    key: impl Into<Key>,
    label: impl Into<String>,
    mark: IconMark,
    active: bool,
) -> ViewNode {
    let label = label.into();
    let tone = if active {
        IconTone::Primary
    } else {
        IconTone::Secondary
    };
    let glyph = icon_in("glyph", mark, IconBox::Header, tone);
    header_action(key, label, glyph, active)
}

/// The glyph Carbon's documented header utilities draw, by their label.
///
/// Carbon's own five names are "Notifications", "Search", "Help",
/// "Account" and "App switcher" (slice-f). Help and account have no mark in
/// the vocabulary yet, so they fall through to the word.
fn header_action_mark(label: &str) -> Option<IconMark> {
    match label {
        "Notifications" => Some(IconMark::Notification),
        "Search" => Some(IconMark::Search),
        "App switcher" => Some(IconMark::Switcher),
        _ => None,
    }
}

fn header_action(key: impl Into<Key>, label: String, caption: ViewNode, active: bool) -> ViewNode {
    header_action_sized(key, label, caption, active, HeaderActionFit::Square)
}

/// How wide a header action is allowed to be.
///
/// Carbon's `.cds--header__action` is an icon button and is `48x48px` on both
/// axes, flush against its neighbour (slice-f.md:150, :155). Our text
/// fallback exists only because two of Carbon's five documented utilities
/// ("Help", "Account") have no glyph in the vocabulary yet, and a word does
/// not fit in 48px; it takes the header link's own `0 16px` padding instead
/// (slice-f.md:148), so it separates itself from its neighbour rather than
/// asking the row for a gap.
enum HeaderActionFit {
    /// 48x48 exactly. Carbon's icon action.
    Square,
    /// 48 tall, at least 48 wide, 16px of inline padding. The word fallback.
    Word,
}

fn header_action_sized(
    key: impl Into<Key>,
    label: String,
    caption: ViewNode,
    active: bool,
    fit: HeaderActionFit,
) -> ViewNode {
    let mut props = Props {
        axis: Some(Axis::Horizontal),
        align: Some(Align::Center),
        // The main axis. `align` is the cross one, so without this the glyph
        // sat flush against the inline-start edge of its own 48px box —
        // measured at 14px left of centre, and reported as "these buttons are
        // not properly centred in their button".
        justify: Some(Justify::Center),
        ..Props::default()
    };
    if matches!(fit, HeaderActionFit::Word) {
        props.padding = Some(InsetRefs {
            left: Some(t(SPACING_05)),
            right: Some(t(SPACING_05)),
            ..InsetRefs::default()
        });
    }
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
        .with_constraints(match fit {
            HeaderActionFit::Square => square_hit_box(MINI_UNIT_6),
            HeaderActionFit::Word => icon_hit_box(MINI_UNIT_6),
        });
    // `Border`, for the reason `header_name` gives.
    //
    // The one control in this module that owns its text, and the one that is
    // a `<button class="cds--header__action">` rather than an `<a>`. Every
    // other row here is an anchor — the product name, the header nav items,
    // both left-panel rows, the switcher rows — and an anchor's words are the
    // destination's name, which a reader may want to copy. See
    // `crate::component::link`.
    let mut node = node
        .interactive(Role::Button, label, INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text()
        .with_focus_figure(FocusFigure::BarInside);
    node.semantics.selected = active;
    node
}

// ---------------------------------------------------------------------
// UI shell left panel (row 41)
// ---------------------------------------------------------------------

/// Carbon's four side-nav width modes. Every modifier in
/// `_side-nav.scss:65-117` sets `inline-size` and nothing else, so the mode
/// *is* a width — the tree, the current mark and the interaction states are
/// identical across all four.
///
/// | Mode | Width | Carbon |
/// |---|---|---|
/// | [`LeftPanelMode::Rail`] | [`MINI_UNIT_6`] (48) | `--rail`, `:65-67` |
/// | [`LeftPanelMode::Fixed`] | [`LEFT_PANEL_WIDTH`] (256) | `--fixed`, `:108-110` |
/// | [`LeftPanelMode::Expandable`] | 0 or 256 | `--expanded`, `:73-75`; the boolean `HeaderContainer.js:24-35` hands both to [`ui_shell_header_menu_trigger`] and to here |
/// | [`LeftPanelMode::Hidden`] | 0 | `--hidden`, `:69-71` |
///
/// Rail and Hidden are widths, not different trees: Carbon's
/// `.cds--side-nav { overflow: hidden }` (`:30-46`) is what turns 48 into an
/// icon-only column and 0 into nothing, and [`left_panel`] gets the same
/// effect from a `Grid`, whose cells clip (`layout/grid.rs`). So a caller
/// hands the same items to every mode and the panel's own width decides how
/// much of each row survives — which is also why [`ui_shell_left_panel_icon_item`]
/// exists: without an icon there is nothing left inside 48 to see.
///
/// Rail's hover-to-expand (`SideNav.js:34-102`, `enterDelayMs = 100`) is not
/// built. It needs a delayed state transition, and nothing in the
/// interaction layer expresses a delay yet; the collapsed geometry is here
/// and the timer is a later row's problem.
///
/// **UX is not a fifth mode.** `--ux` is `inline-size: 256` plus
/// `inset-block-start: $spacing-09` (`_side-nav.scss:48-56`), and
/// `$spacing-09` is `3rem` = **48**
/// (`@carbon/layout/scss/generated/_spacing.scss:52`), which is exactly the
/// header's own block-size. slice-f.md:196 reads that as 32; under T070 the
/// SCSS wins. So `--ux` says "sit under the header" rather than "offset by
/// an arbitrary amount" — `SideNav.js:56` agrees, defaulting
/// `isChildOfHeader` to true — and where a panel sits is the caller's fact,
/// not a width. Below `lg` it collapses to 0, which is a viewport
/// breakpoint a fixed-size pane does not have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LeftPanelMode {
    /// 48: icon-only.
    Rail,
    /// 256, always.
    #[default]
    Fixed,
    /// 0 or 256, driven by the header's hamburger.
    Expandable {
        /// Whether the hamburger is currently open.
        expanded: bool,
    },
    /// 0.
    Hidden,
}

impl LeftPanelMode {
    /// This mode's `inline-size`, the only thing a modifier sets.
    #[must_use]
    pub fn width(self) -> f32 {
        match self {
            Self::Rail => MINI_UNIT_6,
            Self::Fixed => LEFT_PANEL_WIDTH,
            Self::Expandable { expanded: true } => LEFT_PANEL_WIDTH,
            Self::Expandable { expanded: false } | Self::Hidden => 0.0,
        }
    }
}

/// The left panel in `mode`.
pub fn ui_shell_left_panel_in(
    key: impl Into<Key>,
    mode: LeftPanelMode,
    items: Vec<ViewNode>,
) -> ViewNode {
    left_panel(key, items, mode.width())
}

/// The Fixed left panel: [`LEFT_PANEL_WIDTH`] (256), non-collapsible.
pub fn ui_shell_left_panel(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    ui_shell_left_panel_in(key, LeftPanelMode::Fixed, items)
}

/// The Rail left panel: collapsed to [`MINI_UNIT_6`] (48), icon-only width.
/// Hover-to-expand is a delayed state transition this constructor does not
/// drive; it ships the collapsed geometry the anatomy needs to exist at
/// all. Build its items with [`ui_shell_left_panel_icon_item`], or 48px of
/// clipped label is all there is to see.
pub fn ui_shell_left_panel_rail(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    ui_shell_left_panel_in(key, LeftPanelMode::Rail, items)
}

/// A `Grid`, not a `Stack`, for two reasons that are really one.
///
/// A grid **clips its cells** (`layout/grid.rs`'s `clipped_to(cell)`), which
/// is Carbon's `.cds--side-nav { overflow: hidden }` (`_side-nav.scss:33`).
/// That is the whole of Rail and Hidden: at 48 a row's label runs off the
/// end and is cut, at 0 the entire row is, and neither mode needs the items
/// rebuilt. A stack passes its own clip straight through
/// (`layout/stack.rs:170`), so the 0-width panel this file used to build
/// would have painted its rows across the page beside it.
///
/// A grid's implicit rows are also `FitContent` (`page/common.rs`'s
/// `column`), which is what a run of 32-tall rows wants; a vertical stack
/// placed at an exact height divides that height among its children
/// instead.
fn left_panel(key: impl Into<Key>, items: Vec<ViewNode>, width: f32) -> ViewNode {
    let mut props = Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        // `.cds--side-nav__items { padding: 1rem 0 0 }`
        // (`_side-nav.scss:130-135`): 16 before the first row, and nothing
        // on the other three edges.
        padding: Some(InsetRefs {
            top: Some(t(SPACING_05)),
            ..InsetRefs::default()
        }),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(props)
        .with_children(items);
    node.semantics = Semantics {
        role: Some(Role::Pane),
        ..Semantics::default()
    };
    node.with_constraints(pin_inline(width))
}

/// A top-level left-panel row: a flat link when `children` is empty, a
/// sub-menu title when it is not. `expanded` only matters for a sub-menu —
/// its nested children mount only while `expanded` is true, and the caret
/// is [`IconMark::ChevronUp`] / [`IconMark::ChevronDown`] beside the
/// declared `Semantics.expanded` and the mounted children (FR-026).
/// `selected` shows
/// [`LEFT_PANEL_ACCENT`] at the inline-start edge plus `Semantics.selected`.
/// Title type is [`TYPOGRAPHY_HEADING_SM`] (Carbon `$heading-compact-01`).
///
/// No icon: see [`ui_shell_left_panel_icon_item`], which is the same row
/// with Carbon's `__icon` slot filled.
pub fn ui_shell_left_panel_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    left_panel_item(key, label, None, expanded, selected, children)
}

/// [`ui_shell_left_panel_item`] with Carbon's `__icon` slot filled: `mark`
/// at 16 ([`IconBox::Glyph`], `.cds--side-nav__icon > svg { block-size:
/// mini-units(2) }`), [`IconTone::Secondary`] at rest and
/// [`IconTone::Primary`] when this row or a child of it is the current page
/// (`_side-nav.scss:296-298`), with [`LEFT_PANEL_ICON_GAP`] (24) after it.
///
/// This is the constructor [`LeftPanelMode::Rail`] needs. A 48px panel clips
/// every row at 48, and 16 padding plus a 16 glyph centres the mark in
/// exactly that column; a row with no icon clips to blank.
pub fn ui_shell_left_panel_icon_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    mark: IconMark,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    left_panel_item(key, label, Some(mark), expanded, selected, children)
}

fn left_panel_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    icon: Option<IconMark>,
    expanded: bool,
    selected: bool,
    children: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let is_branch = !children.is_empty();
    // `.cds--side-nav__item--active`: React sets it on the item whose
    // *descendant* is the current page, and Carbon reads it twice.
    // `_side-nav.scss:283-298` fills the title and draws its accent only
    // while `aria-expanded="false"` — an open branch shows the mark on the
    // child itself — but `:290-298` steps the title's ink and the icon's
    // fill either way. Collapsing "Kernel" over a current "Fibers" used to
    // leave nothing on screen saying which page was open.
    let child_current = children.iter().any(|child| child.semantics.selected);
    let marked = selected || (child_current && !expanded);
    let mut row = left_panel_row(&RowShape {
        label: &label,
        typography: TYPOGRAPHY_HEADING_SM,
        icon,
        inset: LEFT_PANEL_INSET,
        branch: is_branch.then_some(expanded),
        marked,
        emphasised: selected || child_current,
    });
    // The item below holds focus and its rect spans its whole expanded
    // sub-menu, so a figure on that rect marks the *group* rather than the
    // row a person is standing on. Photographed 2026-09-06 with "Kernel"
    // focused and open: the item measured 64 tall for a 32-tall row, and
    // the stripe landed between "Fibers" and "Petra".
    //
    // The same pair `component::tree_view` uses, and Carbon narrows the
    // same way — `.cds--side-nav__submenu:focus` puts its outline on the
    // title, never on the `<ul>` under it.
    row.semantics.focus_shown_on = FocusShownOn::Head;
    row.semantics.focus_figure = FocusFigure::BarInside;

    let mut parts = vec![row];
    if expanded && is_branch {
        // No padding on the list. The indent is the *row's* own
        // `padding-inline-start` (`_side-nav.scss:314-324`), built into
        // [`ui_shell_left_panel_subitem`]: Carbon's current-page accent is
        // an absolutely positioned `::before` at `inset-inline-start: 0` of
        // a full-width link, so a nested current row's bar still sits on
        // the panel's own edge. The `spacing.07` inset this list used to
        // carry moved the bar in with it, and pushed the label to 51.
        let mut nest = stack("children", Axis::Vertical, None, children);
        nest.props.align = Some(Align::Stretch);
        parts.push(nest);
    }

    let mut node = stack(key, Axis::Vertical, None, parts);
    // Every row spans the panel, at every width. Without it a vertical
    // stack leaves each child at its natural size, so the sub-menu's
    // chevron parked itself against the end of its own label instead of at
    // the panel's trailing edge where `.cds--side-nav__submenu`'s flex row
    // puts it, and a selected row's fill stopped at the end of its word.
    node.props.align = Some(Align::Stretch);
    bind_row_states(&mut node);
    let mut node = node
        .interactive(Role::Button, label, INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE)
        .with_focus_figure(FocusFigure::BarInside);
    // See the `Head` declaration on `row` above: this item's rect spans its
    // expanded sub-menu, and the figure belongs on the title row.
    node.semantics.focus_shown_on = FocusShownOn::OnHead;
    node.semantics.selected = marked;
    node.semantics.expanded = Some(expanded);
    node
}

/// A plain link nested one level under an expanded
/// [`ui_shell_left_panel_item`] sub-menu, at [`LEFT_PANEL_NEST_INSET`] (32,
/// `_side-nav.scss:314-318`). This row is a leaf and never grows further
/// children (Carbon documents one nesting level).
/// Type is [`TYPOGRAPHY_BODY`] (Carbon `$body-compact-01`) — a step lighter
/// than a top-level [`ui_shell_left_panel_item`] title, which is the one
/// documented difference between the two rows (slice-f "Key numbers").
///
/// Under an icon-bearing branch, use
/// [`ui_shell_left_panel_icon_subitem`] instead.
pub fn ui_shell_left_panel_subitem(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    left_panel_subitem(key, label, selected, LEFT_PANEL_NEST_INSET)
}

/// [`ui_shell_left_panel_subitem`] nested under an
/// [`ui_shell_left_panel_icon_item`]: [`LEFT_PANEL_ICON_NEST_INSET`] (72)
/// rather than 32 (`_side-nav.scss:321-324`).
///
/// It carries no icon of its own — Carbon gives the icon to the top-level
/// item only. The name says which branch it nests under, because that is
/// the only thing the two constructors differ on and Carbon states it as a
/// class on the *ancestor* (`.__item--icon a.__link`), which a child node
/// cannot read for itself. A caller that already chose
/// [`ui_shell_left_panel_icon_item`] knows the answer.
pub fn ui_shell_left_panel_icon_subitem(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    left_panel_subitem(key, label, selected, LEFT_PANEL_ICON_NEST_INSET)
}

fn left_panel_subitem(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
    inset: f32,
) -> ViewNode {
    let label = label.into();
    let row = left_panel_row(&RowShape {
        label: &label,
        typography: TYPOGRAPHY_BODY,
        icon: None,
        inset,
        branch: None,
        marked: selected,
        emphasised: selected,
    });
    let mut node = stack(key, Axis::Vertical, None, vec![row]);
    node.props.align = Some(Align::Stretch);
    bind_row_states(&mut node);
    let mut node = node
        .interactive(Role::Button, label, INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE)
        .with_focus_figure(FocusFigure::BarInside);
    node.semantics.selected = selected;
    node
}

/// Everything one left-panel row is, gathered so the builder takes one
/// argument instead of seven.
struct RowShape<'a> {
    /// The row's text, and its accessible name.
    label: &'a str,
    /// Carbon's type step for this level.
    typography: &'static str,
    /// Carbon's `__icon` slot; `None` for a row without one.
    icon: Option<IconMark>,
    /// The label's inline offset from the panel edge — Carbon's
    /// `padding-inline-start` on the link.
    inset: f32,
    /// `Some(expanded)` for a sub-menu title, `None` for a leaf.
    branch: Option<bool>,
    /// Fill this row `layer-selected` and draw its accent bar.
    marked: bool,
    /// Step the label and the icon to their primary tones.
    emphasised: bool,
}

/// One row: the body, with the current-page accent laid **over** it.
///
/// The accent used to be a 3-unit `Fixed` grid track beside the body, which
/// is a track the body then started after — so a top-level label sat at 19
/// and a nested one at 51, against Carbon's 16 and 32. Carbon's bar is
/// `position: absolute` (`_side-nav.scss:392-402`) and takes no flow space
/// at all, so this is a [`NodeKind::Overlay`]: every child gets the
/// container's whole rect (`layout/overlay.rs`), the body pads itself off
/// the panel edge, and the bar sits on top of the first three units of it.
fn left_panel_row(shape: &RowShape<'_>) -> ViewNode {
    let mut caption = text("label", shape.label.to_string());
    caption.props.style = Some(t(shape.typography));
    caption.props.wrap = Some(TextWrap::Ellipsis);
    // `.cds--side-nav__link-text { color: $text-secondary }`
    // (`_side-nav.scss:365-375`), stepping to `$text-primary` for the
    // current page (`:381-385`) and for an item holding it (`:290-293`).
    // Both rows bound `text.primary` at rest, which spent the loudest ink
    // in the theme on every row and left the current one with nowhere to go.
    caption.props.tokens.insert(
        "foreground".into(),
        t(if shape.emphasised {
            TEXT_PRIMARY
        } else {
            TEXT_MUTED
        }),
    );

    let mut lead = Vec::new();
    if shape.inset > LEFT_PANEL_INSET {
        lead.push(gutter("indent", shape.inset - LEFT_PANEL_INSET));
    }
    if let Some(mark) = shape.icon {
        lead.push(icon_in(
            "icon",
            mark,
            IconBox::Glyph,
            if shape.emphasised {
                IconTone::Primary
            } else {
                IconTone::Secondary
            },
        ));
        lead.push(gutter("icon-gap", LEFT_PANEL_ICON_GAP));
    }
    lead.push(caption);
    let mut lead = stack("lead", Axis::Horizontal, None, lead);
    lead.props.align = Some(Align::Center);
    // `with_focus_run`: the row's focus stripe spans the icon and the word,
    // not the row. A branch row also carries a chevron pinned at the panel's
    // trailing edge, so without this the run is 224 of the row's 256 — near
    // enough to the full-width stripe the operator rejected on this very
    // panel on 2026-09-06. The leaf rows have no chevron and would be
    // unaffected either way; declaring it on `lead` for all of them keeps
    // every row in the panel measured the same way.
    let lead = lead.with_focus_run();

    let mut row_parts = vec![lead];
    if let Some(expanded) = shape.branch {
        // `.cds--side-nav__submenu-chevron > svg`: `ChevronDown`, 16px,
        // `$icon-secondary`, turned 180° while `aria-expanded`.
        row_parts.push(icon_toned(
            "chevron",
            if expanded {
                IconMark::ChevronUp
            } else {
                IconMark::ChevronDown
            },
            IconTone::Secondary,
        ));
    }
    let mut body = stack("body", Axis::Horizontal, None, row_parts);
    body.props.align = Some(Align::Center);
    // Two children at most, so `SpaceBetween` reads as "label at the
    // inline-start edge, chevron at the inline-end one" — Carbon's flex row
    // with a truncating `__link-text` and the chevron last. The old
    // `SPACING_03` gap parked the chevron against the label instead, which
    // is what the reference shot shows it is not.
    body.props.justify = Some(Justify::SpaceBetween);
    body.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });

    let fill = shape.marked.then_some(ACCENT_PRIMARY);
    let mut accent = stack(
        "accent",
        Axis::Horizontal,
        None,
        vec![accent_mark("bar", Axis::Vertical, LEFT_PANEL_ACCENT, fill)],
    );
    accent.props.align = Some(Align::Stretch);

    ViewNode::new(NodeKind::Overlay, "row")
        .with_props(Props {
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![body, accent])
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

/// The rule between left-panel items. Width comes from the panel's own
/// [`Align::Stretch`], the same trick [`accent_mark`] uses for a bar that
/// must fill a cell it does not own the extent of; the thickness is the rule
/// material's and is not pinned here, because it is not this module's to
/// pick. See [`crate::token::rule`].
pub fn ui_shell_left_panel_divider(key: impl Into<Key>) -> ViewNode {
    super::rule(key, Axis::Horizontal, BORDER_SUBTLE)
}

// ---------------------------------------------------------------------
// UI shell right panel (row 42)
// ---------------------------------------------------------------------

/// A generic right panel: Carbon's "empty header panel" case.
///
/// `open` is the whole of Carbon's open/shut state:
/// `.cds--header-panel` is `inline-size: 0` and
/// `.cds--header-panel--expanded` is `mini-units(32)` = 256
/// (`_header-panel.scss`, the file entire). Shut is a zero-width panel that
/// is still mounted, never an unmounted one — which is why this takes a
/// boolean rather than leaving the caller to mount it conditionally, and
/// why the panel clips (see [`right_panel`]).
///
/// `label` is the panel's accessible name: a region names itself even
/// though it is not itself a click target.
pub fn ui_shell_right_panel(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    content: Vec<ViewNode>,
) -> ViewNode {
    right_panel(key, label, open, content, Align::Start)
}

/// The Switcher: a right panel whose content is centred
/// (`.cds--switcher { align-items: center }`, slice-f "Anatomy"). Build
/// `items` from [`ui_shell_switcher_item`] and space them with
/// [`ui_shell_right_panel_divider`].
pub fn ui_shell_switcher(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    right_panel(key, label, open, items, Align::Center)
}

/// A **docked region**, not a floating surface.
///
/// This was a `NodeKind::Surface` on [`Layer::Popup`], anchored to a
/// sibling's bottom edge with its height set by its own content — a
/// popover, which is the one shape Carbon is not using here.
/// `_header-panel.scss` is eleven declarations long and every one of them
/// is about a box pinned to two edges of the viewport:
/// `position: fixed; inset-block: mini-units(6) 0; inset-inline-end: 0`,
/// which reads "from the bottom of the header to the bottom of the screen,
/// on the trailing edge". Height comes from the viewport, not from the
/// items; the transition is on `width` alone; and shut is width 0.
///
/// So it is a plain flow node the caller drops into a cell of its own shell
/// layout, and a `Grid` at that, because a grid clips its cells
/// (`layout/grid.rs`) and Carbon's `overflow: hidden` is what makes the
/// shut panel's content disappear rather than spill across the page.
///
/// What is lost with the surface is `InputPolicy::DismissOutside`, which
/// only a surface can declare. Carbon dismisses this panel on any click
/// that is neither inside it nor on a header action
/// (`HeaderPanel.js:31-66`); a docked region has to be closed by its own
/// trigger, by a sibling trigger, or by picking an item. See this row's
/// Agent Note.
fn right_panel(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    content: Vec<ViewNode>,
    align: Align,
) -> ViewNode {
    let mut inner = stack("content", Axis::Vertical, None, content);
    inner.props.align = Some(align);
    // `.cds--switcher__item:nth-child(1) { margin-block-start: $spacing-05 }`
    // (`_switcher.scss`): 16 before the first row. Stated on the container
    // rather than on the first child, the same way the left panel's
    // `__items` states its own (`_side-nav.scss:130-135`) — one inset, one
    // place, and it cannot drift as items are added.
    inner.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });

    // `border-inline-start`, as a node rather than as a `border` binding.
    // `.cds--header-panel--expanded` puts a rule on its two *inline* edges
    // and none on its block ones; a `border` token draws a box, which laid
    // a bright rule across the foot of the panel where Carbon has open
    // viewport. This is the same trick `ui_shell_header` uses for its own
    // bottom rule, for the same reason.
    let rule = accent_mark("rule", Axis::Vertical, 1.0, Some(BORDER_SUBTLE));

    let mut props = Props {
        columns: vec![
            TrackSize::Fixed { value: 1.0 },
            TrackSize::Weight { weight: 1.0 },
        ],
        rows: vec![TrackSize::Weight { weight: 1.0 }],
        align: Some(Align::Stretch),
        ..Props::default()
    };
    // `$layer` on a `$background` page, plus the rule above: a fill step
    // *and* a boundary, so the panel's edge does not rest on colour alone
    // (FR-010).
    props.tokens.insert("background".into(), t(SURFACE_RAISED));
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(props)
        .with_children(vec![rule, inner]);
    node.constraints = pin_inline(if open { RIGHT_PANEL_WIDTH } else { 0.0 });
    node.semantics = Semantics {
        role: Some(Role::Pane),
        label: Some(label.into()),
        ..Semantics::default()
    };
    node
}

/// One switcher row. Type is [`TYPOGRAPHY_HEADING_SM`] (Carbon
/// `$heading-compact-01`, slice-f "Key numbers"), ink
/// [`TEXT_MUTED`] at rest and [`TEXT_PRIMARY`] when `selected`
/// (`.cds--switcher__item-link { color: $text-secondary }`,
/// `--selected { background: $layer-selected; color: $text-primary }`).
///
/// **`selected` reverses a decision this module argued in writing.** It
/// used to have no such parameter, on the strength of the docs usage page:
/// *"there is no selected state for right panel items… the item remains
/// unselected"*. That was one source out of three, and the weakest one.
/// `_switcher.scss` carries `.cds--switcher__item-link--selected` and
/// `SwitcherItem.js:29` carries an `isSelected` prop, so the SCSS and the
/// React API agree with each other and disagree with the prose — and T070
/// ranks the SCSS first when they disagree. slice-f.md:228 and :242 flag
/// the tension as unresolved; it is resolved, two to one, and this is the
/// side it lands on. The old reasoning is kept here rather than deleted
/// because the next reader will find the same usage-page sentence and needs
/// to know it was read and outranked.
///
/// `align_self: Stretch` (`Props::align_self`) overrides `right_panel`'s own
/// `Align::Center` (`ui_shell_switcher` builds its `content` stack with
/// `align: Center` — Carbon's `.cds--switcher { align-items: center }`) so
/// this row is full width, per slice-f.md:227, while
/// [`ui_shell_right_panel_divider`] beside it keeps the container's own
/// `Center` and stays the narrower, centred rule it already is. `align`
/// here (`Align::Center`) is a different fact: it governs how *this* row
/// centres its own child (`caption`) on its own cross axis, unrelated to
/// how the row sits in somebody else's.
pub fn ui_shell_switcher_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    caption.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
    );
    let mut props = Props {
        axis: Some(Axis::Horizontal),
        align: Some(Align::Center),
        align_self: Some(Align::Stretch),
        padding: Some(InsetRefs {
            left: Some(t(SPACING_05)),
            right: Some(t(SPACING_05)),
            ..InsetRefs::default()
        }),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(SURFACE_RAISED));
    props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    let node = ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .with_children(vec![caption])
        .with_constraints(pin_block(SWITCHER_ROW));
    let mut node = node
        .interactive(Role::Button, label, INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE);
    node.semantics.selected = selected;
    node
}

/// A divider between switcher rows. [`SWITCHER_DIVIDER_WIDTH`] (224) wide,
/// deliberately narrower than the 256px panel — Carbon leaves a margin on
/// both sides rather than running the rule edge to edge.
///
/// The node carries `$spacing-03` of padding above and below the rule,
/// because `.cds--switcher__item--divider` carries `margin: $spacing-03
/// $spacing-05` and the block half of that is 8 each way. A margin has no
/// retained-mode equivalent, so the gap is padding on a wrapper — which is
/// why the rule is a child and not the node itself. Flush against the rows
/// above and below is what the reference shot says it is not.
///
/// The width is pinned because 224 is a **run length**, Carbon's own inset
/// rule. The thickness is not pinned: that is the material's, and pinning it
/// here at one unit is what made this divider paint a flat line while a
/// data-table row grooved on the same token.
pub fn ui_shell_right_panel_divider(key: impl Into<Key>) -> ViewNode {
    let mut rule = super::rule("rule", Axis::Horizontal, BORDER_SUBTLE);
    rule.constraints.horizontal = AxisConstraint {
        min: Some(SWITCHER_DIVIDER_WIDTH),
        max: Some(SWITCHER_DIVIDER_WIDTH),
        priority: 0,
    };
    let mut node = stack(key, Axis::Vertical, None, vec![rule]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        top: Some(t(SPACING_03)),
        bottom: Some(t(SPACING_03)),
        ..InsetRefs::default()
    });
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
/// A fixed-width empty column inside a row's body: `width` of nothing.
///
/// Carbon states the two gaps this covers as box properties — a nested
/// link's `padding-inline-start` (`_side-nav.scss:314-324`) and the
/// `__icon`'s `margin-inline-end` (`:415-417`) — and one of the values,
/// `mini-units(9)` = 72, has no step on the shipped spacing ramp, which
/// runs 48, 64, 80 (`token/shipped.rs`). Carbon sizes the side nav in
/// `mini-units`, not in `$spacing-*`, so the ramp is not going to grow one.
/// A pinned extent states the gap in the units [`Constraints`] already
/// carry (FR-053) instead of inventing a token name for it, which is the
/// one thing this file must never do.
///
/// [`accent_mark`] with no fill, and for the same reason it is a `Stack`
/// and not a `Spacer`: a spacer answers an unbounded query at a huge
/// extent.
fn gutter(key: &'static str, width: f32) -> ViewNode {
    let mut node = accent_mark(key, Axis::Vertical, width, None);
    // An empty vertical stack measures zero on its own main axis, and a
    // zero-height placement is what `assert_no_degenerate_or_overflowing`
    // is for. It costs nothing to be the row's full height — the node
    // paints nothing either way — and a gap that is a real rect is a gap
    // the frame record can be read for.
    node.props.align_self = Some(Align::Stretch);
    node
}

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

/// Carbon's 48x48 header hit box, with the width as a FLOOR rather than a
/// pin.
///
/// Carbon sizes these boxes for an icon-only glyph. This library draws a
/// WORD in them instead (FR-026, see this module's own doc), and a word
/// does not fit a box measured for a 16px glyph. Pinning `max` as well as
/// `min` on the inline axis clips the label -- the defect already found in
/// [`super::modal`]'s close button and [`super::number_input`]'s steppers,
/// where the label is fixed and short enough that it took a frame-level
/// geometry test to see it.
///
/// Here the label is worse: it is caller-supplied
/// ([`ui_shell_header_action`] takes it as a parameter), so no fixed
/// measurement makes the pin safe. The block axis stays pinned because the
/// header's own height genuinely is 48px in Carbon; the inline axis keeps
/// 48 as the minimum tap target and lets the label decide the rest.
/// A box that is exactly `size` on both axes — Carbon's `.cds--header__action`.
///
/// [`icon_hit_box`]'s peer, and the difference is the horizontal `max`: that
/// one names a *minimum* target, so the row's spare width can stretch it. An
/// icon action must not stretch, because a centred glyph in a stretched box
/// no longer sits under the pointer where the picture says it does.
fn square_hit_box(size: f32) -> Constraints {
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

fn icon_hit_box(size: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(size),
            max: None,
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
        HEADER_ACCENT, IconBox, IconMark, IconTone, LEFT_PANEL_ACCENT, LEFT_PANEL_ICON_GAP,
        LEFT_PANEL_ICON_NEST_INSET, LEFT_PANEL_INSET, LEFT_PANEL_NEST_INSET, LEFT_PANEL_ROW,
        LEFT_PANEL_WIDTH, LeftPanelMode, MINI_UNIT_6, RIGHT_PANEL_WIDTH, SWITCHER_DIVIDER_WIDTH,
        SWITCHER_ROW, icon_in, icon_toned, ui_shell_header, ui_shell_header_action,
        ui_shell_header_action_icon, ui_shell_header_menu_trigger, ui_shell_header_nav_item,
        ui_shell_left_panel, ui_shell_left_panel_divider, ui_shell_left_panel_icon_item,
        ui_shell_left_panel_icon_subitem, ui_shell_left_panel_in, ui_shell_left_panel_item,
        ui_shell_left_panel_rail, ui_shell_left_panel_subitem, ui_shell_right_panel,
        ui_shell_right_panel_divider, ui_shell_switcher, ui_shell_switcher_item,
    };
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::text::text;
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_SELECTED, SPACING_05, SURFACE_RAISED, TEXT_MUTED,
        TEXT_PRIMARY,
    };
    use crate::frame::PetrifiedFrame;

    use crate::testing::inks;
    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Interaction, Justify, NodeKind, Role, ViewNode};

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

        // V6: the header must never bind the shared `"border"` token — it
        // always paints a 4-sided box (`pagination.rs`'s own comment), not
        // Carbon's single `border-block-end` (slice-f.md:154). The real
        // bottom rule is this dedicated 1px divider row instead.
        assert!(
            !node.props.tokens.contains_key("border"),
            "the header must not bind \"border\" (it draws a 4-sided box); \
             the bottom-only rule below is the `divider` child instead"
        );
        let divider = named(&node, "divider");
        assert_eq!(token(divider, "background"), Some(BORDER_SUBTLE));
        assert_eq!(divider.constraints.vertical.min, Some(1.0));
        assert_eq!(divider.constraints.vertical.max, Some(1.0));
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
        // V6: the header's outer node is now a `Grid` of [`bar`, `divider`]
        // (the bottom-border fix above), so product-to-global order lives
        // one level down, on `bar`'s own children, not `node`'s.
        assert_eq!(named(&node, "bar").children[0].key.as_str(), "trigger");
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
        let glyph = named(&closed, "glyph");
        assert_eq!(glyph.kind, NodeKind::Canvas);
        assert_eq!(
            glyph.props.text, None,
            "the trigger is a glyph, not the word Open"
        );
        assert_eq!(
            glyph.props.canvas,
            icon_in("glyph", IconMark::Menu, IconBox::Header, IconTone::Primary)
                .props
                .canvas,
            "a shut trigger draws Carbon's Menu"
        );
        assert_eq!(closed.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(closed.constraints.vertical.min, Some(MINI_UNIT_6));

        let open = ui_shell_header_menu_trigger("trigger", true);
        assert_eq!(
            open.semantics.label.as_deref(),
            Some("Close navigation menu")
        );
        assert!(open.semantics.selected);
        assert_eq!(
            named(&open, "glyph").props.canvas,
            icon_in("glyph", IconMark::Close, IconBox::Header, IconTone::Primary)
                .props
                .canvas,
            "an open trigger draws Carbon's Close"
        );
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
    fn header_action_is_48_tall_with_a_48_floor_and_a_required_label() {
        let node = ui_shell_header_action("notify", "Notifications", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Notifications"));
        assert!(!node.semantics.selected);
        let glyph = named(&node, "glyph");
        assert_eq!(glyph.kind, NodeKind::Canvas);
        assert_eq!(
            glyph.props.text, None,
            "the action is a glyph, not the word"
        );
        assert_eq!(
            glyph.props.canvas,
            icon_in(
                "glyph",
                IconMark::Notification,
                IconBox::Header,
                IconTone::Secondary
            )
            .props
            .canvas,
            "a resting action draws its glyph in $icon-secondary"
        );
        assert_eq!(node.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(
            node.constraints.horizontal.max,
            Some(MINI_UNIT_6),
            "an icon action is 48x48 exactly (slice-f.md:150), pinned on both \
             axes. This used to take 48 as a floor with no ceiling, on the \
             reasoning that the box held a caller-supplied word — but a \
             labelled utility resolves to the glyph path and holds no word at \
             all, so the row's spare width was free to stretch it"
        );
        assert_eq!(node.constraints.vertical.min, Some(MINI_UNIT_6));
        assert_eq!(node.constraints.vertical.max, Some(MINI_UNIT_6));
        assert_eq!(
            node.props.justify,
            Some(Justify::Center),
            "the glyph centres on the main axis too. `align` is the cross \
             one, and with only that set the glyph sat flush against the \
             inline-start edge, 14px left of centre in its own 48px button"
        );

        // The word fallback is the case the pin would break, and it is a
        // different case: "Help" has no glyph in the vocabulary, so it keeps
        // 48 as a floor and carries the header link's own 16px inline padding
        // (slice-f.md:148) rather than asking the actions row for a gap.
        let word = ui_shell_header_action("help", "Help", false);
        assert_eq!(word.constraints.horizontal.min, Some(MINI_UNIT_6));
        assert_eq!(
            word.constraints.horizontal.max, None,
            "a word does not fit in 48px; pinning `max` here would clip it"
        );
        let padding = word.props.padding.as_ref().expect("a word action pads");
        assert_eq!(
            padding.left.as_ref().map(TokenName::as_str),
            Some(SPACING_05)
        );
        assert_eq!(
            padding.right.as_ref().map(TokenName::as_str),
            Some(SPACING_05)
        );

        let active = ui_shell_header_action("notify", "Notifications", true);
        assert!(active.semantics.selected);
        assert_eq!(token(&active, "background@selected"), Some(SURFACE_RAISED));
        assert_eq!(
            named(&active, "glyph").props.canvas,
            icon_in(
                "glyph",
                IconMark::Notification,
                IconBox::Header,
                IconTone::Primary
            )
            .props
            .canvas,
            "an active action draws its glyph in $icon-primary"
        );
    }

    /// Carbon's three documented utilities with a glyph in the vocabulary
    /// draw it by label; a label with no glyph still surfaces its word, and
    /// the explicit constructor draws whatever it is handed.
    #[test]
    fn header_action_glyph_follows_carbons_documented_labels() {
        for (label, mark) in [
            ("Notifications", IconMark::Notification),
            ("Search", IconMark::Search),
            ("App switcher", IconMark::Switcher),
        ] {
            let node = ui_shell_header_action("a", label, false);
            assert_eq!(node.semantics.label.as_deref(), Some(label));
            assert_eq!(
                named(&node, "glyph").props.canvas,
                icon_in("glyph", mark, IconBox::Header, IconTone::Secondary)
                    .props
                    .canvas,
                "{label} draws {mark:?}"
            );
            assert!(!has_key(&node, "label"), "{label} is not also spelled out");
        }

        let help = ui_shell_header_action("help", "Help", false);
        assert_eq!(named(&help, "label").props.text.as_deref(), Some("Help"));
        assert!(
            !has_key(&help, "glyph"),
            "Help has no glyph yet and says so"
        );

        let explicit = ui_shell_header_action_icon("x", "Anything", IconMark::Close, false);
        assert_eq!(explicit.semantics.label.as_deref(), Some("Anything"));
        assert_eq!(
            named(&explicit, "glyph").props.canvas,
            icon_in(
                "glyph",
                IconMark::Close,
                IconBox::Header,
                IconTone::Secondary
            )
            .props
            .canvas
        );
    }

    // -- left panel ------------------------------------------------------

    /// Every one of Carbon's four modifiers sets `inline-size` and nothing
    /// else (`_side-nav.scss:65-117`), so the four modes are four widths on
    /// one tree — including the two that are zero, which stay mounted the
    /// way `--hidden` does rather than being dropped by the caller.
    #[test]
    fn each_width_mode_pins_carbons_own_inline_size() {
        for (mode, want) in [
            (LeftPanelMode::Rail, MINI_UNIT_6),
            (LeftPanelMode::Fixed, LEFT_PANEL_WIDTH),
            (
                LeftPanelMode::Expandable { expanded: true },
                LEFT_PANEL_WIDTH,
            ),
            (LeftPanelMode::Expandable { expanded: false }, 0.0),
            (LeftPanelMode::Hidden, 0.0),
        ] {
            let panel = ui_shell_left_panel_in("nav", mode, vec![]);
            assert_eq!(panel.semantics.role, Some(Role::Pane), "{mode:?}");
            assert_eq!(panel.constraints.horizontal.min, Some(want), "{mode:?}");
            assert_eq!(panel.constraints.horizontal.max, Some(want), "{mode:?}");
        }
        assert_eq!(LEFT_PANEL_WIDTH, 256.0);
        assert_eq!(MINI_UNIT_6, 48.0);

        // The two named entry points are the two named modes.
        assert_eq!(
            ui_shell_left_panel("nav", vec![])
                .constraints
                .horizontal
                .min,
            Some(LEFT_PANEL_WIDTH)
        );
        assert_eq!(
            ui_shell_left_panel_rail("nav", vec![])
                .constraints
                .horizontal
                .min,
            Some(MINI_UNIT_6)
        );
    }

    /// Rail and Hidden are widths, and the clip is what makes them read as
    /// modes. A `Stack` passes its parent's clip straight through
    /// (`layout/stack.rs:170`), so the 0-width panel this file used to
    /// build would have painted its rows across the page beside it; a
    /// `Grid` clips each cell (`layout/grid.rs`).
    #[test]
    fn the_panel_clips_its_rows_so_a_narrow_mode_hides_them() {
        assert_eq!(
            ui_shell_left_panel_in("nav", LeftPanelMode::Hidden, vec![]).kind,
            NodeKind::Grid,
            "a panel that does not clip cannot have a zero-width mode"
        );
    }

    #[test]
    fn leaf_item_has_no_chevron_and_no_nested_children() {
        let node = ui_shell_left_panel_item("home", "Home", false, false, vec![]);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Home"));
        assert_eq!(node.semantics.expanded, Some(false));
        assert!(!has_key(&node, "chevron"));
        assert!(!has_key(&node, "children"));
        assert_eq!(
            named(&node, "row").constraints.vertical.min,
            Some(LEFT_PANEL_ROW)
        );
        assert_eq!(LEFT_PANEL_ROW, 32.0);
    }

    #[test]
    fn branch_item_mounts_children_only_while_expanded() {
        let child = ui_shell_left_panel_subitem("child", "Fibers", false);
        let expanded = ui_shell_left_panel_item("kernel", "Kernel", true, false, vec![child]);
        assert_eq!(expanded.semantics.expanded, Some(true));
        let chevron = named(&expanded, "chevron");
        assert_eq!(chevron.kind, NodeKind::Canvas);
        assert_eq!(
            chevron.props.text, None,
            "the chevron is a glyph, not a word"
        );
        assert_eq!(
            chevron.props.canvas,
            icon_toned("chevron", IconMark::ChevronUp, IconTone::Secondary)
                .props
                .canvas,
            "an expanded sub-menu points its chevron up"
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
            named(&collapsed, "chevron").props.canvas,
            icon_toned("chevron", IconMark::ChevronDown, IconTone::Secondary)
                .props
                .canvas,
            "a collapsed sub-menu points its chevron down"
        );
        assert!(!has_key(&collapsed, "child"));
        assert!(!has_key(&collapsed, "children"));
    }

    #[test]
    fn selected_item_shows_the_accent_and_the_flag() {
        let node = ui_shell_left_panel_item("home", "Home", false, true, vec![]);
        assert!(node.semantics.selected);
        let mark = named(&node, "bar");
        assert_eq!(token(mark, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(mark.constraints.horizontal.min, Some(LEFT_PANEL_ACCENT));
        assert_eq!(LEFT_PANEL_ACCENT, 3.0);
        assert_eq!(token(&node, "background@selected"), Some(LAYER_SELECTED));

        let subitem = ui_shell_left_panel_subitem("child", "Fibers", true);
        assert!(subitem.semantics.selected);
        assert_eq!(subitem.semantics.role, Some(Role::Button));
        assert_eq!(token(&subitem, "background@selected"), Some(LAYER_SELECTED));
    }

    /// The divider pins no thickness of its own. That is the assertion: it
    /// used to pin one logical unit for a material that paints four device
    /// pixels, so it drew a flat line while a data-table row grooved on the
    /// same token. `NodeKind::Separator` measures the material instead.
    #[test]
    fn left_panel_divider_pins_no_thickness_of_its_own() {
        let node = ui_shell_left_panel_divider("rule");
        assert_eq!(node.kind, NodeKind::Separator);
        assert_eq!(node.constraints.vertical.min, None);
        assert_eq!(node.constraints.vertical.max, None);
        assert_eq!(token(&node, "background"), Some(BORDER_SUBTLE));
    }

    // -- right panel -------------------------------------------------

    /// `_header-panel.scss` entire: width 0 shut, `mini-units(32)` = 256
    /// open, and no anchor of any kind. It used to be an
    /// `Anchor::Sibling`-anchored `Layer::Popup` `Surface` — a popover,
    /// which is the one shape Carbon is not using here.
    #[test]
    fn the_right_panel_is_a_docked_region_that_is_256_open_and_0_shut() {
        let open = ui_shell_right_panel("notifications-panel", "Notifications", true, vec![]);
        assert_eq!(
            open.kind,
            NodeKind::Grid,
            "a docked region is a flow node, not a floating surface"
        );
        assert_eq!(open.props.layer, None, "nothing about it floats");
        assert_eq!(open.props.anchor, None, "and nothing about it anchors");
        assert_eq!(open.semantics.role, Some(Role::Pane));
        assert_eq!(open.semantics.label.as_deref(), Some("Notifications"));
        assert!(open.interactions.is_empty());
        assert_eq!(open.constraints.horizontal.min, Some(RIGHT_PANEL_WIDTH));
        assert_eq!(open.constraints.horizontal.max, Some(RIGHT_PANEL_WIDTH));
        assert_eq!(RIGHT_PANEL_WIDTH, 256.0);

        let shut = ui_shell_right_panel("notifications-panel", "Notifications", false, vec![]);
        assert_eq!(
            shut.constraints.horizontal.max,
            Some(0.0),
            "shut is width 0 and still mounted, never unmounted"
        );
    }

    /// The switcher's rows, its rule, and the selected state the SCSS and
    /// the React API both carry (`ui_shell_switcher_item`'s doc for why
    /// this reverses an earlier decision).
    #[test]
    fn switcher_hosts_items_and_dividers_and_carries_a_selected_state() {
        let node = ui_shell_switcher(
            "switcher",
            "App switcher",
            true,
            vec![
                ui_shell_switcher_item("a", "Petra", true),
                ui_shell_right_panel_divider("d1"),
                ui_shell_switcher_item("b", "Inspector", false),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::Pane));
        assert!(has_key(&node, "a"));
        assert!(has_key(&node, "b"));
        let item = named(&node, "a");
        assert_eq!(item.semantics.role, Some(Role::Button));
        assert_eq!(item.semantics.label.as_deref(), Some("Petra"));
        assert_eq!(item.constraints.vertical.min, Some(SWITCHER_ROW));
        assert_eq!(SWITCHER_ROW, 32.0);

        // Two channels, not one: the flag the engine paints
        // `layer-selected` from, and the ink step on the label itself.
        assert!(item.semantics.selected);
        assert_eq!(token(item, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(
            token(named(item, "label"), "foreground"),
            Some(TEXT_PRIMARY)
        );
        let rest = named(&node, "b");
        assert!(!rest.semantics.selected);
        assert_eq!(token(named(rest, "label"), "foreground"), Some(TEXT_MUTED));

        // The rule is 224 wide with `$spacing-03` above and below
        // (`_switcher.scss`). The width is pinned because 224 is a run
        // length; the thickness is not, because it is the material's.
        let divider = named(&node, "d1");
        let rule = named(divider, "rule");
        assert_eq!(rule.kind, NodeKind::Separator);
        assert_eq!(
            rule.constraints.horizontal.min,
            Some(SWITCHER_DIVIDER_WIDTH)
        );
        assert_eq!(rule.constraints.vertical.max, None);
        assert_eq!(SWITCHER_DIVIDER_WIDTH, 224.0);
    }

    /// Both right-panel constructors mount beside the header action Carbon
    /// says opens them, wherever the caller puts the pair — here two
    /// containers below the root, the gallery catalog's own depth. They no
    /// longer *anchor* to that action (they are docked regions now), but
    /// the pairing is still the composition every caller writes, so it is
    /// still the one checked.
    #[test]
    fn right_panels_validate_beside_their_trigger_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "ui_shell_right_panel",
            vec![
                ui_shell_header_action("notify", "Notifications", true),
                ui_shell_right_panel(
                    "notifications-panel",
                    "Notifications",
                    true,
                    vec![text("note", "No new notifications.")],
                ),
            ],
        );
        crate::component::tests::assert_mounts_at_catalog_depth(
            "ui_shell_switcher",
            vec![
                ui_shell_header_action("apps", "App switcher", true),
                ui_shell_switcher(
                    "switcher",
                    "App switcher",
                    true,
                    vec![
                        ui_shell_switcher_item("a", "Petra", false),
                        ui_shell_right_panel_divider("d1"),
                        ui_shell_switcher_item("b", "Inspector", false),
                    ],
                ),
            ],
        );
    }

    // -- frame-level checks (geometry, focus, contrast) -----------------
    //
    // Nothing in this module anchors any more. The header, all four
    // left-panel width modes and both right panels are plain flow nodes,
    // so every one of them petrifies standalone the same way every other
    // non-anchored component in this crate does. The right-panel checks
    // below used to extract `right_panel`'s inner `"content"` Stack,
    // because the outer node was an `Anchor::Sibling` `Surface` that could
    // not petrify without a target; they petrify the whole panel now.

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn assert_no_degenerate_or_overflowing(label: &str, frame: &PetrifiedFrame) {
        assert!(!frame.placements.is_empty(), "{label}: nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{label}: {} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{label}: {} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent(label, p, &frame.placements[parent_idx]);
            }
        }
    }

    /// Check C/D: the header with a menu trigger, current and non-current
    /// nav, and an active and an inactive action, all in one frame. This is
    /// the class-4 suspect this group's own brief names explicitly: the
    /// menu trigger and every header action are Carbon-sized 48×48 icon-only
    /// hit boxes (`icon_hit_box(MINI_UNIT_6)`) that this library filled with
    /// a WORD ("Open"/"Close", "Notifications", "Search") until 2026-09-04;
    /// they draw Carbon's 20px glyphs now, and a label with no glyph still
    /// surfaces its word (this module's own doc "Glyphs").
    #[test]
    fn header_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = ui_shell_header(
            "header",
            "GOrgOn",
            Some(ui_shell_header_menu_trigger("trigger", true)),
            vec![
                ui_shell_header_nav_item("overview", "Overview", true),
                ui_shell_header_nav_item("fibers", "Fibers", false),
            ],
            vec![
                ui_shell_header_action("notify", "Notifications", true),
                ui_shell_header_action("search", "Search", false),
            ],
        );
        let frame = petrify_lone(node);
        assert_no_degenerate_or_overflowing("header", &frame);
    }

    /// The class-4 guard, on the one label in this module a caller chooses.
    ///
    /// [`ui_shell_header_action`] takes its label as a parameter, so no
    /// audit of Carbon's own five action names ("Notifications", "Search",
    /// "Help", "Account", "App switcher") can prove the box is wide enough
    /// -- the next caller picks a longer word. Under the pinned
    /// `max == min == 48` this module shipped, a long label overflowed its
    /// own rect and was clipped; `icon_hit_box` keeps 48 as the tap-target
    /// floor and lets the label set the width.
    ///
    /// Falsify by restoring `max: Some(size)` on `icon_hit_box`'s
    /// horizontal axis: this fails naming the label placement.
    #[test]
    fn a_caller_supplied_header_action_label_is_never_clipped() {
        for label in [
            "Notifications",
            "App switcher",
            "User profile and account settings",
        ] {
            let node = ui_shell_header(
                "header",
                "GOrgOn",
                None,
                vec![ui_shell_header_nav_item("overview", "Overview", true)],
                vec![ui_shell_header_action("action", label, false)],
            );
            let frame = petrify_lone(node);
            assert_no_degenerate_or_overflowing(label, &frame);
        }
    }

    /// Check F: the name, an enabled nav item, and an enabled action all
    /// declare `Focus` and are reachable; the header shell itself (a
    /// `Role::Pane`, no interactions) is not.
    #[test]
    fn header_children_are_focus_reachable_and_the_shell_is_not() {
        let node = ui_shell_header(
            "header",
            "GOrgOn",
            Some(ui_shell_header_menu_trigger("trigger", false)),
            vec![ui_shell_header_nav_item("overview", "Overview", true)],
            vec![ui_shell_header_action("notify", "Notifications", false)],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let reachable = |suffix: &str| {
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"));
            focus.order().iter().any(|o| o == &placement.id)
        };
        assert!(reachable("/trigger"), "menu trigger must be reachable");
        assert!(reachable("/name"), "product name link must be reachable");
        assert!(reachable("/overview"), "nav item must be reachable");
        assert!(reachable("/notify"), "header action must be reachable");
        assert!(
            !frame
                .placements
                .iter()
                .any(|p| p.id.ends_with("/header") && focus.order().iter().any(|o| o == &p.id)),
            "the header shell itself (Role::Pane, no interactions) must not \
             be a Tab stop"
        );
    }

    /// Check E: the product name, a current and a non-current nav label, a
    /// resting and an active header action label — every text child against
    /// the header's own resting fill, in both themes.
    #[test]
    fn header_text_clears_aa_contrast_against_the_header_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = ui_shell_header(
                "header",
                "GOrgOn",
                Some(ui_shell_header_menu_trigger("trigger", false)),
                vec![
                    ui_shell_header_nav_item("overview", "Overview", true),
                    ui_shell_header_nav_item("fibers", "Fibers", false),
                ],
                vec![ui_shell_header_action("notify", "Notifications", true)],
            );
            let header_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("header binds a resting background");
            let header_bg = color(&theme, header_bg_name.as_str());
            for (label, text_key, host_key) in [
                ("name", "label", "name"),
                ("current-nav", "label", "overview"),
                ("resting-nav", "label", "fibers"),
                ("action", "glyph", "notify"),
                ("trigger", "glyph", "trigger"),
            ] {
                let host = named(&node, host_key);
                let text_node = named(host, text_key);
                let inks = inks(text_node);
                assert!(!inks.is_empty(), "{label}: binds an ink");
                let opacity = text_node.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
                    let fg = color(&theme, fg_name.as_str())
                        .faded(opacity)
                        .over(header_bg);
                    let ratio = fg.contrast_ratio(header_bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{label} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        header_bg_name.as_str()
                    );
                }
            }
        }
    }

    /// Class 2 falsification: the header nav item's current-state
    /// `indicator` is an `accent_mark` — an unconstrained-along-its-axis
    /// empty `Stack` that only paints a non-zero rect if the Grid cell's own
    /// `Align::Stretch` fills it in (this module's own `accent_mark` doc).
    /// The invariant under test is that a *current* item's indicator draws
    /// a real, non-zero-width bar — proved here by reproducing the zero
    /// rect directly rather than reading the code: strip `Align::Stretch`
    /// from the Grid, and the indicator's width collapses to 0, the same
    /// failure Group 5 reproduced for Tabs' selected indicator.
    #[test]
    fn falsification_removing_grid_stretch_collapses_the_current_nav_indicator_to_zero() {
        let mut current = ui_shell_header_nav_item("overview", "Overview", true);
        // The whole nav item IS the Grid (`ViewNode::new(NodeKind::Grid,
        // key)`), so strip the align directly off its own props.
        current.props.align = None;
        let frame = petrify_lone(current);
        let indicator = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/indicator"))
            .expect("indicator is placed");
        assert_eq!(
            indicator.rect.w, 0.0,
            "removing Align::Stretch must reproduce the zero-width defect \
             this test proves the shipped code does not have"
        );
    }

    // -- left panel frame-level checks -----------------------------------

    fn left_panel_sample() -> ViewNode {
        ui_shell_left_panel(
            "shell-left",
            vec![
                ui_shell_left_panel_item("home", "Home", false, true, vec![]),
                ui_shell_left_panel_item(
                    "kernel",
                    "Kernel",
                    true,
                    false,
                    vec![
                        ui_shell_left_panel_subitem("fibers-item", "Fibers", true),
                        ui_shell_left_panel_subitem("trace-item", "Trace", false),
                    ],
                ),
                ui_shell_left_panel_divider("rule"),
                ui_shell_left_panel_item("settings", "Settings", false, false, vec![]),
            ],
        )
    }

    /// Check C/D across the Fixed panel (selected leaf, expanded branch
    /// with a selected and an unselected subitem, a divider, a plain leaf)
    /// and the Rail panel with a real item (collapsed-width geometry).
    ///
    /// An *empty* Rail (`vec![]`, as `fixed_panel_is_256_and_rail_is_48`
    /// above still constructs) is not tested for a non-degenerate rect
    /// here: `left_panel` pins only the inline axis (this module's own
    /// doc — the panel's block-size is spec 004's docked-viewport
    /// concern, out of this row's anatomy), so a childless panel's height
    /// is legitimately whatever its real parent grants it, not a defect
    /// this component's own file can fix.
    #[test]
    fn left_panel_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        assert_no_degenerate_or_overflowing("fixed", &petrify_lone(left_panel_sample()));
        assert_no_degenerate_or_overflowing(
            "rail",
            &petrify_lone(ui_shell_left_panel_rail(
                "shell-rail",
                vec![ui_shell_left_panel_item(
                    "home",
                    "Home",
                    false,
                    false,
                    vec![],
                )],
            )),
        );
    }

    /// Where Carbon puts the four inline offsets a left-panel row has, in
    /// placed geometry rather than in props.
    ///
    /// Every one of them was wrong. The accent bar was a 3-unit `Fixed`
    /// grid track that the body started *after*, so a top-level label sat
    /// at 19 against Carbon's 16; the nested list carried a `spacing.07`
    /// left inset on top of that, so a nested label sat at 51 against
    /// Carbon's 32; and the whole run started flush against the panel's top
    /// edge, with no `__items { padding: 1rem 0 0 }`.
    #[test]
    fn a_left_panel_row_places_its_label_where_carbon_states_it() {
        let panel = ui_shell_left_panel(
            "nav",
            vec![
                ui_shell_left_panel_item(
                    "kernel",
                    "Kernel",
                    true,
                    false,
                    vec![ui_shell_left_panel_subitem("fibers", "Fibers", true)],
                ),
                ui_shell_left_panel_icon_item(
                    "petra",
                    "Petra",
                    IconMark::Edit,
                    true,
                    false,
                    vec![ui_shell_left_panel_icon_subitem("trace", "Trace", false)],
                ),
            ],
        );
        let frame = petrify_lone(panel);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ends with {suffix:?}"))
                .rect
        };
        let panel_rect = rect("/nav");
        let at = |suffix: &str| rect(suffix).x - panel_rect.x;

        assert!(
            (at("/kernel/row/body/lead/label") - LEFT_PANEL_INSET).abs() < 0.01,
            "a top-level label sits at {} , Carbon's `padding: 0 mini-units(2)` \
             says {LEFT_PANEL_INSET}",
            at("/kernel/row/body/lead/label")
        );
        assert!(
            (at("/fibers/row/body/lead/label") - LEFT_PANEL_NEST_INSET).abs() < 0.01,
            "a nested label sits at {}, `__menu a.__link \
             {{ padding-inline-start: mini-units(4) }}` says {LEFT_PANEL_NEST_INSET}",
            at("/fibers/row/body/lead/label")
        );
        assert!(
            (at("/trace/row/body/lead/label") - LEFT_PANEL_ICON_NEST_INSET).abs() < 0.01,
            "a nested label under an icon-bearing item sits at {}, \
             `.__item--icon a.__link` says {LEFT_PANEL_ICON_NEST_INSET}",
            at("/trace/row/body/lead/label")
        );
        assert!(
            (at("/petra/row/body/lead/icon") - LEFT_PANEL_INSET).abs() < 0.01,
            "the icon starts at the row's own inset, not after it"
        );
        assert!(
            (at("/petra/row/body/lead/label")
                - (LEFT_PANEL_INSET + IconMark::Edit.extent(IconBox::Glyph) + LEFT_PANEL_ICON_GAP))
                .abs()
                < 0.01,
            "an icon-bearing row's label sits {} past the panel edge, wanted \
             16 + a 16 glyph + a {LEFT_PANEL_ICON_GAP} margin",
            at("/petra/row/body/lead/label")
        );

        // The current-page bar overlays the row rather than reserving a
        // track: it starts on the panel's own edge even for the nested row
        // Carbon indents by 32, because `::before` is `inset-inline-start: 0`
        // of a full-width link.
        assert!(
            at("/fibers/row/accent/bar").abs() < 0.01,
            "the nested current row's accent is at {}, not on the panel edge",
            at("/fibers/row/accent/bar")
        );
        assert!(
            (rect("/fibers/row/accent/bar").w - LEFT_PANEL_ACCENT).abs() < 0.01,
            "and it is still 3 wide"
        );

        // `.cds--side-nav__items { padding: 1rem 0 0 }`.
        assert!(
            (rect("/kernel/row").y - panel_rect.y - LEFT_PANEL_INSET).abs() < 0.01,
            "the first row starts {} below the panel's top edge, Carbon says 16",
            rect("/kernel/row").y - panel_rect.y
        );
    }

    /// A collapsed branch whose child is the current page keeps the mark.
    ///
    /// `_side-nav.scss:283-289`: `--item--active __submenu[aria-expanded='false']`
    /// takes `$background-selected` and its own 3px `::before`. Collapsing
    /// "Kernel" over a current "Fibers" used to unmount the child and leave
    /// nothing on screen saying which page was open.
    #[test]
    fn a_collapsed_branch_wears_its_current_childs_mark() {
        let current = || ui_shell_left_panel_subitem("fibers", "Fibers", true);

        let collapsed = ui_shell_left_panel_item("kernel", "Kernel", false, false, vec![current()]);
        assert!(
            !has_key(&collapsed, "fibers"),
            "the child is unmounted, which is the whole reason the parent has \
             to carry the mark"
        );
        assert!(collapsed.semantics.selected, "so the fill fires");
        assert_eq!(
            token(named(&collapsed, "bar"), "background"),
            Some(ACCENT_PRIMARY),
            "and the accent is drawn"
        );
        assert_eq!(
            token(named(&collapsed, "label"), "foreground"),
            Some(TEXT_PRIMARY),
            "and the title steps to primary ink (`:290-293`)"
        );

        // Expanded, the mark belongs to the child; the title still steps.
        let expanded = ui_shell_left_panel_item("kernel", "Kernel", true, false, vec![current()]);
        assert!(
            !expanded.semantics.selected,
            "an open branch must not fill: the child is on screen wearing the \
             mark itself, and filling both says two pages are current"
        );
        assert_eq!(token(named(&expanded, "bar"), "background"), None);
        assert_eq!(
            token(named(&expanded, "label"), "foreground"),
            Some(TEXT_PRIMARY)
        );

        // A branch with no current child is plain, open or shut.
        let plain = ui_shell_left_panel_item(
            "kernel",
            "Kernel",
            false,
            false,
            vec![ui_shell_left_panel_subitem("fibers", "Fibers", false)],
        );
        assert!(!plain.semantics.selected);
        assert_eq!(
            token(named(&plain, "label"), "foreground"),
            Some(TEXT_MUTED)
        );
    }

    /// Check F: the selected leaf, the branch, both subitems, and the
    /// plain leaf all declare `Focus` and are reachable; a collapsed
    /// branch's own subitems (if any existed) would not mount at all —
    /// mirroring `tree_view`'s identical rule for the same reason.
    #[test]
    fn left_panel_items_are_focus_reachable() {
        let frame = petrify_lone(left_panel_sample());
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        for suffix in [
            "/home",
            "/kernel",
            "/fibers-item",
            "/trace-item",
            "/settings",
        ] {
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"));
            assert!(
                focus.order().iter().any(|o| o == &placement.id),
                "{suffix} must be focus reachable"
            );
        }
    }

    /// Check E: a selected leaf, an expanded branch title, a selected and
    /// an unselected subitem, and a plain leaf — every label against its
    /// own item's resting fill, in both themes.
    #[test]
    fn left_panel_labels_clear_aa_contrast_against_their_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                (
                    "selected-leaf",
                    ui_shell_left_panel_item("home", "Home", false, true, vec![]),
                ),
                (
                    "branch",
                    ui_shell_left_panel_item(
                        "kernel",
                        "Kernel",
                        true,
                        false,
                        vec![ui_shell_left_panel_subitem("child", "Fibers", false)],
                    ),
                ),
                (
                    "selected-subitem",
                    ui_shell_left_panel_subitem("child", "Fibers", true),
                ),
                (
                    "resting-subitem",
                    ui_shell_left_panel_subitem("child", "Trace", false),
                ),
            ] {
                let bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: item binds a resting background"));
                let bg = color(&theme, bg_name.as_str());
                let row = named(&node, "row");
                let text_label = named(row, "label");
                let fg_name = text_label
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{label}: label binds a foreground"));
                let opacity = text_label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    bg_name.as_str()
                );
            }
        }
    }

    /// Class 2 falsification, the left panel's own accent bar.
    ///
    /// `accent_mark` is built `Axis::Vertical`, so [`LEFT_PANEL_ACCENT`]
    /// pins the *thickness* (width) unconditionally and it is the *height*
    /// that only survives because the bar's own carrier declares
    /// `Align::Stretch`. The carrier is a one-child `Stack` laid over the
    /// row by a [`NodeKind::Overlay`], rather than a `Fixed` grid track
    /// beside the body, so the bar takes no flow space and the label sits
    /// where Carbon puts it; stripping the stretch reproduces a
    /// zero-**height** bar.
    #[test]
    fn falsification_removing_carrier_stretch_collapses_the_selected_accent_to_zero() {
        let mut node = ui_shell_left_panel_item("home", "Home", false, true, vec![]);
        // `node` is the outer `Stack` `ui_shell_left_panel_item` builds;
        // `row` is its first child, and the accent carrier is `row`'s
        // second (`body` is the first, and paints under it).
        let row = std::sync::Arc::make_mut(&mut node.children[0]);
        std::sync::Arc::make_mut(&mut row.children[1]).props.align = None;
        let frame = petrify_lone(node);
        let bar = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/accent/bar"))
            .expect("bar is placed");
        assert_eq!(
            bar.rect.h, 0.0,
            "removing Align::Stretch must reproduce the zero-height defect \
             this test proves the shipped code does not have"
        );
        assert_eq!(
            bar.rect.w, LEFT_PANEL_ACCENT,
            "the thickness axis is pinned unconditionally and must not move"
        );
    }

    // -- right panel ----------------------------------------------------

    /// Check C/D: the Switcher's content (two items plus a divider) places
    /// with real rects and draws nothing larger than them, and so does a
    /// generic panel actually holding content (a caller-supplied note).
    #[test]
    fn right_panel_content_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let generic = ui_shell_right_panel(
            "notifications-panel",
            "Notifications",
            true,
            vec![text("note", "No new notifications.")],
        );
        assert_no_degenerate_or_overflowing("generic", &petrify_lone(generic));

        let switcher = ui_shell_switcher(
            "switcher",
            "App switcher",
            true,
            vec![
                ui_shell_switcher_item("a", "Petra", true),
                ui_shell_right_panel_divider("d1"),
                ui_shell_switcher_item("b", "Inspector", false),
            ],
        );
        assert_no_degenerate_or_overflowing("switcher", &petrify_lone(switcher));
    }

    /// Carbon's own documented base case — "empty header panel" (slice-f
    /// "UI shell right panel" Variants: "the right panel is a generic
    /// content container — 'empty header panel' is shown in docs as the
    /// base case") — is a literally childless `content` with no `background`
    /// of its own (only the outer anchored `Surface` binds one). Its
    /// `paint.paint_hash` is 0 the same way `progress_step`'s `icon-top`
    /// spacer's is (`PaintState`'s own doc: "zero when the node draws
    /// nothing of its own") — a 0×0 rect here covers no fewer pixels than
    /// any other size would, so this is the inert case, not a defect, and
    /// is checked as such rather than skipped.
    #[test]
    fn the_empty_header_panel_case_is_inert_not_degenerate() {
        let generic = ui_shell_right_panel("notifications-panel", "Notifications", true, vec![]);
        let content = named(&generic, "content").clone();
        let frame = petrify_lone(content);
        let root = frame
            .placements
            .iter()
            .find(|p| p.id == "/root/content")
            .expect("content is placed");
        assert_eq!(
            root.paint.paint_hash, 0,
            "an empty content node with no background of its own must draw \
             nothing — if this starts drawing, the degenerate-rect check \
             above must stop excluding it"
        );
    }

    /// Check F: switcher items declare `Focus` and are reachable, selected
    /// and not. `Switcher.js:33-71` gives the list roving arrow-key focus;
    /// what is checked here is the weaker property the focus tree can see,
    /// that both rows are in the order at all.
    #[test]
    fn switcher_items_are_focus_reachable() {
        let switcher = ui_shell_switcher(
            "switcher",
            "App switcher",
            true,
            vec![ui_shell_switcher_item("a", "Petra", true)],
        );
        let frame = petrify_lone(switcher);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let placement = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/a"))
            .expect("switcher item is placed");
        assert!(
            focus.order().iter().any(|o| o == &placement.id),
            "switcher item must be focus reachable"
        );
    }

    /// `ui_shell_switcher_item`'s `align_self: Stretch` (`Props::align_self`)
    /// makes the row span `content`'s own resolved width, per
    /// slice-f.md:227, while [`ui_shell_right_panel_divider`] beside it
    /// keeps `content`'s own `Align::Center` and stays the narrower,
    /// centred rule it already was — the property `VISUAL-AUDIT.md`'s
    /// "Still open" list named as unverifiable because the catalog cannot
    /// mount this nested composition (no anchored surface renders in the
    /// gallery). Checked in placed geometry instead, the same way
    /// `pagination`'s equivalent fix is.
    #[test]
    fn switcher_item_stretches_full_width_while_the_divider_stays_centred() {
        let switcher = ui_shell_switcher(
            "switcher",
            "App switcher",
            true,
            vec![
                ui_shell_switcher_item("a", "Petra", false),
                ui_shell_right_panel_divider("d1"),
                ui_shell_switcher_item("b", "Inspector", false),
            ],
        );
        // The open panel pins itself to `RIGHT_PANEL_WIDTH` (256), so the
        // whole thing petrifies as it ships. It used to need `content`
        // extracted and that width pinned onto it by hand, because the
        // outer node was an anchored `Surface` with no target in an
        // isolated tree.
        let frame = petrify_lone(switcher);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ends with {suffix:?}"))
                .rect
        };
        // 255, not 256: the panel spends one unit on its own
        // `border-inline-start` rule, exactly as Carbon's border box does.
        let outer = rect("/content");
        assert_eq!(outer.w, RIGHT_PANEL_WIDTH - 1.0);
        for item in ["/a", "/b"] {
            let r = rect(item);
            assert_eq!(r.x, outer.x, "{item}: must be flush at the leading edge");
            assert_eq!(r.w, outer.w, "{item}: must span the content's full width");
        }
        let divider = rect("/d1");
        assert_eq!(
            divider.w, SWITCHER_DIVIDER_WIDTH,
            "the divider's own declared width must not be affected by its \
             sibling's `align_self`"
        );
        assert!(
            divider.w < outer.w,
            "the divider must stay narrower than a full-width item, or this \
             test proves nothing about the two behaving differently"
        );
    }

    /// Check E: the switcher item label against the panel's own resting
    /// fill ([`SURFACE_RAISED`], the fill the outer `Surface` binds and the
    /// content sits on once mounted), in both themes.
    #[test]
    fn switcher_item_label_clears_aa_contrast_against_the_panel_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let switcher = ui_shell_switcher(
                "switcher",
                "App switcher",
                true,
                vec![ui_shell_switcher_item("a", "Petra", false)],
            );
            let panel_bg_name = switcher
                .props
                .tokens
                .get("background")
                .expect("panel binds a resting background");
            let panel_bg = color(&theme, panel_bg_name.as_str());
            let item = named(&switcher, "a");
            let text_label = named(item, "label");
            let fg_name = text_label
                .props
                .tokens
                .get("foreground")
                .expect("label binds a foreground");
            let opacity = text_label.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str())
                .faded(opacity)
                .over(panel_bg);
            let ratio = fg.contrast_ratio(panel_bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "switcher item label at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                panel_bg_name.as_str()
            );
        }
    }

    /// Class 4 suspect sweep: every realistic Carbon action label this
    /// module's own doc names ("What none of the three rows build" —
    /// hamburger, search, notification, help, account, switcher/apps) run
    /// through the pinned 48×48 [`ui_shell_header_action`] box. Carbon
    /// names these as short, single-purpose labels (slice-f "Icons":
    /// "search, notification, help, account/user, switcher/apps"), and
    /// this sweep is what actually decides whether the class-4 shape
    /// (word pinned inside an icon-only hit box) fires for the labels this
    /// row realistically carries — not a guess from reading the code.
    #[test]
    fn realistic_action_labels_do_not_overflow_the_pinned_hit_box() {
        for label in [
            "Notifications",
            "Search",
            "Help",
            "Account",
            "User profile",
            "App switcher",
        ] {
            let node = ui_shell_header_action("action", label, false);
            let frame = petrify_lone(node);
            assert_no_degenerate_or_overflowing(label, &frame);
        }
    }
}
