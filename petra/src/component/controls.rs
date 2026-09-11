//! `checkbox`, `radio`, and `toggle` — the three binary controls.
//!
//! All three carry `Role::Button` plus a declared `selected` state (C13's
//! table): a checked checkbox, a chosen radio, and an "on" toggle are the
//! same semantic fact — one control is presently true — worn by three
//! different shapes. The shape channel is corner radius, which is enough
//! here and only here: a square and a circle are the two ends of the rect
//! family, so unlike `super::status` this pair needs no `silhouette` token
//! to separate them. A checkbox is a nearly-sharp square (`shape.corner-xs`,
//! via [`crate::token::CornerRole::BoxedMark`]); a radio and a toggle's
//! track are full pills — a round control reads as "one choice among
//! several" the way a square one does not, which is the same distinction a
//! browser's own checkbox/radio pair makes.
//!
//! The checkbox was `shape.corner-sm`, then flat `shape.corner-none` from
//! 2026-08-25, until this file's own flag day (`contracts/
//! token-vocabulary.md` §11 step 7) put it on `CornerRole::BoxedMark` — the
//! enum's own doc names "a checkbox's box" as its example — which resolves
//! to `shape.corner-xs` (2) on the 16-unit box. The interim square existed
//! because the shipped ramp had no step between `none` and `sm` at the time:
//! measured against radio on the same 12x12 box they then shared, a 4-unit
//! radius against a full one was 0.828 logical units of outline deviation at
//! its widest, under one device pixel at scale 1.0, so the only honest way
//! to make the distinction visible was to take the rounding off entirely.
//! `shape.corner-xs` (added 2026-08-25, FR-022) is exactly the step that was
//! missing; the checkbox now takes it instead of standing in for a token
//! that did not exist yet. The boxes themselves are Carbon's sizes: checkbox
//! 16×16, radio 18×18 (T070, SCSS, not the style-page 20).
//!
//! The radio box, the toggle knob and the toggle track are all stadiums, and
//! all three say so through [`crate::token::CornerRole::Pill`]. Carbon draws
//! the first two with `border-radius: 50%` and the third with a fixed 12px
//! on a 24-tall track, which is the same shape written a different way.
//! `Pill` exists because the fixed per-role radii cannot express any of
//! them: the largest is `Floating`'s 8, which only reaches a stadium on an
//! edge of 16 or under, and these are 18, 18 and 24. For one day in
//! September 2026 the track and the knob disagreed about it — the knob a
//! literal circle, the track routed through `Grouping` and rendered as a
//! 4-radius box — and a circular knob sitting in a boxy slot is what that
//! looked like.
//!
//! Selection is never carried by fill colour alone: every control here also
//! sets `Semantics.selected`, so the state survives with the colour turned
//! off. A checked checkbox nests `icon("tick", Check)` inside the keyed
//! `"box"`; a selected radio nests an inner dot. Indeterminate is a third
//! state (`Semantics.value = "mixed"`, `selected = false`), not a fake on.
//!
//! # Why these three keep a border when the rest of the library dropped one
//!
//! The 2026-08-25 design pass deleted the outline from the card, the field
//! and the progress rail and replaced it with a tonal step, because in each
//! of those the edge was decoration over a shape that already had a fill.
//! These are the exception, and the reason is structural rather than
//! aesthetic: **an unchecked checkbox and an unselected radio are nothing
//! but their outline.** Fill is `None` when the control is off and
//! `accent.primary` when it is on, so taking the border away while off does
//! not quieten the control, it deletes it. The toggle track is the same
//! argument one step weaker — it fills with the accent when on, but that
//! fill is what the knob slides *inside*, and a track a reader cannot find
//! the ends of does not read as a track.
//!
//! What changed for all three is the tone. They bound `text.muted`, a text
//! colour at 10.73:1 on a card; then [`BORDER_SUBTLE`]; and since
//! 2026-09-05 they bind [`BORDER_STRONG`].
//!
//! That last move is the point of the paragraph above, arriving in the
//! token layer. One border tone was doing two jobs — the hairline between
//! two table rows and the outline that *is* an unchecked checkbox — and it
//! was held to WCAG SC 1.4.11's 3:1 because of the second. That floor set
//! the tone of every divider on 42 pages and made the catalog read as a
//! wireframe. The two jobs are two names now: `border.subtle` is the
//! decorative rule at Carbon's own quiet 1.3:1, and [`BORDER_STRONG`] is
//! the control boundary still held at 3:1 on every layer by
//! `crate::token::shipped`'s
//! `border_strong_sits_between_the_subtle_border_and_every_text_tone`.
//! These three controls are on the control side, and the module comment
//! above is the argument for why.
//!
//! # The knob is white, in both themes
//!
//! Carbon's toggle handle is `background-color: $icon-on-color` on
//! `.cds--toggle__switch::before`, with **no `--checked` override**
//! (SOURCED
//! `.agents/research/08-25-2026/Carbon-Component-Inventory/slice-f.md:26`),
//! so it is white on and off. This library bound [`TEXT_ON_ACCENT`] for the
//! on state, which resolves to the theme's own `surface.base` — `#121212`
//! in dark — so row 36 drew a **black knob** inside a blue track. Wave D
//! recorded it and left it for the token layer
//! (`.agents/notes/implemented/feature/2026-09-05-the-notification-carries-a-kind-and-two-rows-leave-carbon-on-purpose.md`);
//! [`TEXT_ON_COLOR`] is that name, white in every theme the way Carbon's
//! `$text-on-color` and `$icon-on-color` are, and the on-knob binds it.
//!
//! The **off** knob stays [`TEXT_PRIMARY`], which is a departure Carbon
//! does not make and this library cannot avoid: the off track is
//! `surface.raised`, and white on `#222222` would be a knob at 14.2:1 —
//! louder than the page's own prose, on the control that is *not* doing
//! anything. `text.primary` is `#f2f2f2` in dark and `#1a1a1a` in light, so
//! the off knob reads correctly in both polarities where a fixed white
//! would vanish in light.

use super::field::warning_helper;
use super::icon::{IconMark, icon};
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_STRONG, BUTTON_DISABLED, ICON_DISABLED, ICON_ON_COLOR_DISABLED,
    SPACING_01, SPACING_02, SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_MUTED, TEXT_ON_ACCENT,
    TEXT_ON_COLOR, TEXT_PRIMARY, t,
};
use super::{pad, stack, swatch};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, FocusFigure, Intent, Interaction, Key, Phase, Role,
    ViewNode,
};

/// Carbon checkbox box (`_checkbox.scss` / style-page Structure): 16×16.
const CHECKBOX_BOX: f32 = 16.0;
/// Carbon radio appearance (`_radio-button.scss`): 18×18. T070 prefers SCSS
/// over the style-page 20.
const RADIO_BOX: f32 = 18.0;
/// Indeterminate dash inside the 16px box: a short bar, not a tick.
const CHECKBOX_DASH_W: f32 = 8.0;
const CHECKBOX_DASH_H: f32 = 2.0;

/// Carbon default switch (`_toggle.scss`): 48×24 track, 18 handle, 3 inset, 24 travel.
const TOGGLE_TRACK_W: f32 = 48.0;
const TOGGLE_TRACK_H: f32 = 24.0;
const TOGGLE_HANDLE: f32 = 18.0;
const TOGGLE_TRAVEL: f32 = 24.0;
const TOGGLE_INSET: f32 = 3.0;

/// Carbon small switch: 32×16 track, 10 handle, same 3 inset, 16 travel.
const TOGGLE_SM_TRACK_W: f32 = 32.0;
const TOGGLE_SM_TRACK_H: f32 = 16.0;
const TOGGLE_SM_HANDLE: f32 = 10.0;
const TOGGLE_SM_TRAVEL: f32 = 16.0;

fn pinned(w: f32, h: f32) -> Constraints {
    Constraints {
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
    }
}

enum ToggleSize {
    Default,
    Small,
}

/// What a checkbox and a toggle both declare: one boolean, flipped when the
/// press completes.
///
/// Named once because the two controls are the same gesture with two
/// pictures, and a second hand-written copy is a second thing to drift.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

/// A mark beside a label. `selected` is the semantic fact; the fill and the
/// inner child (tick, dash, or dot) are the two visual channels.
fn labelled_box(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
    box_node: ViewNode,
    intents: &[Interaction],
    figure: FocusFigure,
) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let mut label_node = text("label", label.clone());
    // Carbon: checkbox's `:disabled + label { color: $text-disabled }`
    // (`_checkbox.scss`) and radio's identical rule on
    // `.radio-button__label` (`_radio-button.scss`). `$text-disabled` and
    // `$icon-disabled` are byte-identical in every published theme
    // (`_themes.scss`), so this reuses the name the box's own outline binds
    // below rather than shipping a second token for one measured value.
    label_node
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_DISABLED));
    let mut row = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![box_node, label_node],
    );
    // Body line-height is 20; the checkbox is 16 and the radio is 18.
    // Start-align sits the mark on the top of the line; Center puts it on
    // the optical midline.
    row.props.align = Some(Align::Center);
    row.props.padding = Some(pad(SPACING_02, SPACING_01));
    // No `owning_its_text`: a checkbox's and a radio's caption is a `<label>`
    // in Carbon, and `user-select: none` is what a browser puts on the
    // control, never on the label beside it. The operator: *"check boxes need
    // their text highlightable"*. `crate::component::link` carries the full
    // argument and the price.
    let mut node = row.interactive(Role::Button, label, intents);
    // The caller's, because the two controls that share this row do not
    // have the same room under them and the operator caught the difference
    // on 2026-09-06: *"radio button needs the under bar, the other bar is
    // too close."*
    //
    // A `BarUnder` needs `FocusRing::gap` + `FocusRing::thickness` = 5 units
    // of clear run below the node. `radio_group` stacks its items on
    // `SPACING_05` = **16** and has eleven to spare; `checkbox_group` stacks
    // on `SPACING_02` = **4**, so the bar's last unit paints inside the next
    // row's box and the operator cannot tell which of the two it marks.
    // That second number was measured on 2026-09-05 — check-a's bottom at
    // 308.0 against check-b's top at 312.0 — and it is why the checkbox
    // keeps a figure that needs no run at all.
    //
    // `toggle` keeps the bar for the same reason the radio may: its rows are
    // 12 apart.
    //
    // Carbon rings the 16-unit box itself, not the label row
    // (`_checkbox.scss`), and this marks the row. That gap is open and
    // tracked.
    node.semantics.focus_figure = figure;
    node.semantics.selected = selected;
    node
}

/// Outline only: unchecked checkbox, unselected radio. The keyed `"box"` is
/// a swatch so tests.rs still reads background from that node.
fn empty_mark(size: f32, shape: &str) -> ViewNode {
    let mut node = swatch("box", size, size, None, Some(BORDER_STRONG), Some(shape));
    // Carbon: checkbox's `:disabled + label::before { border-color:
    // $icon-disabled }` (`_checkbox.scss`) and radio's identical rule on
    // `.radio-button__appearance` (`_radio-button.scss`). This module's own
    // doc header already argues an unchecked/unselected mark *is* its
    // outline, so fading that outline is the only channel a disabled empty
    // control has to lose.
    node.props
        .tokens
        .insert("border@disabled".into(), t(ICON_DISABLED));
    node
}

/// Filled mark with a second-channel child. The keyed `"box"` stack carries
/// the background token; insets centre the inner mark on the main axis.
fn marked_box(size: f32, fill: &str, shape: &str, inner: ViewNode) -> ViewNode {
    let inner_w = inner.constraints.horizontal.min.unwrap_or(0.0);
    let inset = ((size - inner_w) * 0.5).max(0.0);
    let mut node = stack(
        "box",
        Axis::Horizontal,
        None,
        vec![
            swatch("inset-start", inset, size, None, None, None),
            inner,
            swatch("inset-end", inset, size, None, None, None),
        ],
    );
    node.props.align = Some(Align::Center);
    node.props.tokens.insert("background".into(), t(fill));
    node.props.tokens.insert("border".into(), t(BORDER_STRONG));
    node.props.tokens.insert("radius".into(), t(shape));
    // Carbon: `_checkbox.scss`'s "checked:disabled ... background-color:
    // $icon-disabled" swaps the accent fill for the same faded tone the
    // outline uses when off (see `empty_mark`) — a checked-and-disabled box
    // reads as neither the live accent nor a plain outline.
    node.props
        .tokens
        .insert("background@disabled".into(), t(ICON_DISABLED));
    node.props
        .tokens
        .insert("border@disabled".into(), t(ICON_DISABLED));
    node.with_constraints(pinned(size, size))
}

/// FR-022: a checkbox's box is [`crate::token::CornerRole::BoxedMark`]'s
/// own named example, so both states resolve to `shape.corner-xs` (2) on
/// the 16-unit box rather than a literal `SHAPE_NONE`.
fn checkbox_mark(checked: bool) -> ViewNode {
    let radius = corner_for(CornerRole::BoxedMark, CHECKBOX_BOX);
    if checked {
        marked_box(
            CHECKBOX_BOX,
            ACCENT_PRIMARY,
            radius,
            icon("tick", IconMark::Check),
        )
    } else {
        empty_mark(CHECKBOX_BOX, radius)
    }
}

/// The 16×16 box alone, in any of its three states, with no label and no
/// interaction of its own: the mark a data table draws in its selection
/// column, where the **row** is the control that carries `selected` and
/// the box is its visible second channel (`data_table.rs`).
///
/// Sized off the same constants [`checkbox`] uses, so a standalone box and
/// a labelled one are one shape at one size.
pub(crate) fn checkbox_box(state: CheckState) -> ViewNode {
    match state {
        CheckState::Unchecked => checkbox_mark(false),
        CheckState::Checked => checkbox_mark(true),
        CheckState::Mixed => marked_box(
            CHECKBOX_BOX,
            ACCENT_PRIMARY,
            corner_for(CornerRole::BoxedMark, CHECKBOX_BOX),
            swatch(
                "dash",
                CHECKBOX_DASH_W,
                CHECKBOX_DASH_H,
                Some(TEXT_ON_ACCENT),
                None,
                None,
            ),
        ),
    }
}

/// A checkbox: an independent on/off choice, drawn as a sharp 16×16 square.
pub fn checkbox(key: impl Into<Key>, label: impl Into<String>, checked: bool) -> ViewNode {
    labelled_box(
        key,
        label,
        checked,
        checkbox_mark(checked),
        &[Interaction::Focus, Interaction::Click],
        // Four units to the next row. See `labelled_box`.
        FocusFigure::BarInside,
    )
    // Spec 010 FR-013: a click flips the one boolean this node carries, on
    // release. Declared here and not on `labelled_box`, so
    // `checkbox_readonly` — which shares every line of that helper — does
    // not inherit a behaviour it will not honour.
    .with_behaviour(TOGGLES_ON_RELEASE)
}

/// The three states a checkbox can declare (Carbon's `checked`,
/// unchecked, and `indeterminate`).
///
/// A value rather than two bools because `checked` and `indeterminate`
/// are not independent: Carbon's own prop pair lets a caller assert both,
/// and a `checkbox` that then has to pick one is a decision hidden in a
/// constructor. Three variants make the impossible pair unspellable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    /// Off: the outline alone.
    #[default]
    Unchecked,
    /// On: the accent fill and the tick.
    Checked,
    /// Neither: the accent fill and the dash. `Semantics.value` is
    /// `"mixed"` and `selected` stays false — see [`checkbox_indeterminate`].
    Mixed,
}

/// A checkbox in any of its three states. [`checkbox`] and
/// [`checkbox_indeterminate`] are the two-state and mixed spellings of the
/// same control; this is the one a caller that cycles all three holds its
/// state in.
pub fn checkbox_tristate(
    key: impl Into<Key>,
    label: impl Into<String>,
    state: CheckState,
) -> ViewNode {
    match state {
        CheckState::Unchecked => checkbox(key, label, false),
        CheckState::Checked => checkbox(key, label, true),
        CheckState::Mixed => checkbox_indeterminate(key, label),
    }
}

/// A checkbox in the mixed state: not checked, not empty.
///
/// `Semantics.selected` stays false — mixed is not on. `Semantics.value` is
/// `"mixed"` so the third state is declared, not faked as selected.
pub fn checkbox_indeterminate(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let mut node = labelled_box(
        key,
        label,
        false,
        checkbox_box(CheckState::Mixed),
        &[Interaction::Focus, Interaction::Click],
        FocusFigure::BarInside,
    )
    .with_behaviour(TOGGLES_ON_RELEASE);
    node.semantics.value = Some("mixed".into());
    node
}

/// A checkbox that shows its state and keeps focus, but does not take Click.
///
/// Read-only is not disabled (`contracts/interaction-state.md` §5):
/// `Semantics.read_only` is set and `Semantics.disabled` is not.
pub fn checkbox_readonly(key: impl Into<Key>, label: impl Into<String>, checked: bool) -> ViewNode {
    let mut node = labelled_box(
        key,
        label,
        checked,
        checkbox_mark(checked),
        &[Interaction::Focus],
        FocusFigure::BarInside,
    );
    node.semantics.read_only = true;
    node
}

/// A non-interactive checkbox group: a legend plus the caller's items.
///
/// No `Role` — the enum has no Group, and a container with no actions is
/// outside FR-058. Item spacing is Carbon's stacked gap (`spacing-02` = 4);
/// the legend sits `spacing-03` (8) above the first item.
pub fn checkbox_group(
    key: impl Into<Key>,
    legend: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut legend_node = text("legend", legend.into());
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let list = stack("items", Axis::Vertical, Some(SPACING_02), items);
    stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![legend_node, list],
    )
}

/// Stack a binary control over [`warning_helper`]. No container border:
/// the helper's word and glyph are the channels, and boxing the row would
/// be a second frame Carbon does not draw on checkbox or radio.
fn with_warning(key: impl Into<Key>, control: ViewNode, message: impl Into<String>) -> ViewNode {
    let mut node = stack(
        key,
        Axis::Vertical,
        Some(SPACING_02),
        vec![control, warning_helper(message)],
    );
    node.props.align = Some(Align::Stretch);
    node
}

/// A checkbox plus a warning helper stacked below it.
///
/// Colour is not the only channel: the helper already carries
/// `Warning: {message}` and a WarningFilled mark. This constructor does
/// not wrap the control in a four-sided container border.
pub fn checkbox_warning(
    key: impl Into<Key>,
    label: impl Into<String>,
    checked: bool,
    message: impl Into<String>,
) -> ViewNode {
    with_warning(key, checkbox("checkbox", label, checked), message)
}

/// A radio button: one choice among a group, drawn as an 18×18 circle.
///
/// Selected: the keyed `"box"` fills solid with [`ACCENT_PRIMARY`].
/// Unselected: fill `None`, outline [`BORDER_STRONG`] (the 2026-08-25 edge
/// contract). So the two states differ as **solid disc against empty ring**,
/// which is a shape channel and survives greyscale — it does not lean on the
/// blue at all.
///
/// # Why there is no inner dot
///
/// Carbon draws the selected radio as a ring with a concentric dot, scaling
/// the 18px circle by `0.5` to get a 9px one. Petra drew that, and it came out
/// visibly wrong: 18 minus 9 is 9, so the centring inset is **4.5**, the dot
/// lands on a half-pixel, and at 1x the painter snaps it up and to the left.
/// On a 9px mark half a pixel is an eighth of its width, and the operator read
/// it as an off-centre dot on 2026-09-04 — correctly. The layout was exact
/// (box and dot both centred on 305.0, both axes); only the rasterization was
/// not, which is why every frame-record assertion over this component passed
/// while it looked broken.
///
/// Filling the circle is the operator's call and it removes the odd inset
/// rather than rounding it away, so there is no half-pixel left to snap.
/// Carbon's own ring-plus-dot is the thing given up. Restoring it needs either
/// an even dot size, which stops being Carbon's `scale(0.5)`, or pixel
/// snapping for small marks in the painter — see
/// `.agents/notes/proposed/bug-fix/2026-09-04-a-half-pixel-inset-snaps-a-small-mark-off-centre.md`.
// FR-022 reaches the radio box through `CornerRole::Pill`. Carbon's
// `border-radius: 50%` is a proportional circle, so no *fixed* per-role
// radius can express it — 8 is the largest `corner_for` offers and an
// 18-unit box needs 9 — and `Pill` is the member that names the shape
// instead of a number. See the module doc's own paragraph on this.
pub fn radio(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let box_node = if selected {
        let mut node = swatch(
            "box",
            RADIO_BOX,
            RADIO_BOX,
            Some(ACCENT_PRIMARY),
            Some(BORDER_STRONG),
            Some(corner_for(CornerRole::Pill, RADIO_BOX)),
        );
        // Carbon: `_radio-button.scss`'s disabled selector on
        // `.radio-button__appearance` (`border-color: $icon-disabled`) plus
        // its nested `::before` dot (`background-color: $text-disabled` —
        // byte-identical to `$icon-disabled` in every published theme, see
        // `labelled_box`). Petra fills the whole disc rather than nesting a
        // dot (this module's own doc header, the half-pixel argument), so
        // the one fill here carries both halves of Carbon's rule.
        node.props
            .tokens
            .insert("background@disabled".into(), t(ICON_DISABLED));
        node.props
            .tokens
            .insert("border@disabled".into(), t(ICON_DISABLED));
        node
    } else {
        empty_mark(RADIO_BOX, corner_for(CornerRole::Pill, RADIO_BOX))
    };
    labelled_box(
        key,
        label,
        selected,
        box_node,
        &[Interaction::Focus, Interaction::Click],
        // Sixteen units to the next row, so the default bar fits with
        // eleven to spare. The operator asked for it by name.
        FocusFigure::BarUnder,
    )
    // Spec 010 FR-013. `Select`, not `Toggle`: a radio sets itself true and
    // never flips back on its own, and exclusivity among its siblings is the
    // caller's to answer, which is the whole difference between the two
    // members.
    .with_behaviour(Behaviour {
        intent: Intent::Select,
        phase: Phase::OnRelease,
    })
}

/// A radio plus a warning helper stacked below it.
///
/// Same channels as [`checkbox_warning`]: the helper's word and glyph, and
/// no four-sided container border around the row.
pub fn radio_warning(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
    message: impl Into<String>,
) -> ViewNode {
    with_warning(key, radio("radio", label, selected), message)
}

/// A vertical radio group. Mutual exclusivity is the caller's `selected`
/// bools — Petra has no native radio `name`, so this container does not
/// clear siblings. Carbon's horizontal-group item gap is [`SPACING_05`]
/// (SCSS 16; T070, not the style-page 8); this constructor stacks
/// `--vertical` and spends that same gap between items.
pub fn radio_group(
    key: impl Into<Key>,
    legend: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut legend_node = text("legend", legend.into());
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let list = stack("items", Axis::Vertical, Some(SPACING_05), items);
    stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![legend_node, list],
    )
}

/// A toggle: Carbon **default** size (48×24 track). Label sits above the
/// switch; state text ("On"/"Off") sits beside it.
pub fn toggle(key: impl Into<Key>, label: impl Into<String>, on: bool) -> ViewNode {
    toggle_sized(key, label, on, ToggleSize::Default)
}

/// A toggle: Carbon **small** size (32×16 track). Same anatomy as [`toggle`];
/// FR-058 still requires a label argument.
pub fn toggle_sm(key: impl Into<Key>, label: impl Into<String>, on: bool) -> ViewNode {
    toggle_sized(key, label, on, ToggleSize::Small)
}

fn toggle_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    on: bool,
    size: ToggleSize,
) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let (track_w, track_h, handle, travel) = match size {
        ToggleSize::Default => (TOGGLE_TRACK_W, TOGGLE_TRACK_H, TOGGLE_HANDLE, TOGGLE_TRAVEL),
        ToggleSize::Small => (
            TOGGLE_SM_TRACK_W,
            TOGGLE_SM_TRACK_H,
            TOGGLE_SM_HANDLE,
            TOGGLE_SM_TRAVEL,
        ),
    };
    // Both pads stay in the tree so the knob's identity is stable and the
    // engine can interpolate its x. Off: inset | handle | inset+travel.
    // On: inset+travel | handle | inset. Travel is 24 default / 16 small.
    let start = if on {
        TOGGLE_INSET + travel
    } else {
        TOGGLE_INSET
    };
    let end = if on {
        TOGGLE_INSET
    } else {
        TOGGLE_INSET + travel
    };
    // DEPARTURE FROM CARBON, operator decision 2026-09-05: no check mark.
    //
    // Carbon's small toggle **has** one. `Toggle.js` renders
    // `.cds--toggle__check` under `isSm && !readOnly`, and `_toggle.scss`
    // gives it `inline-size: 6px; block-size: 5px; inset-block-start: 6px;
    // inset-inline-end: 5px` in `fill: $support-success` — which, against
    // the 32x16 switch with its checked 10x10 handle at x 19..29 / y 3..13,
    // is a 6x5 tick at x 21..27 / y 6..11: centred inside the white handle
    // to within half a unit. `ignored/carbon-ref/shots/36-toggle.png` shows
    // exactly that, and `super::icon`'s [`IconMark::Check`] is already that
    // same path in a 10-unit node, so putting it back is a one-line change.
    //
    // This library shipped the tick in `pad-start` instead, which floated it
    // in the track's leading gap with nothing around it. The operator asked
    // twice for it to go rather than to move, and chose deletion over
    // Carbon's placement when both were put to him with the reference shot.
    // So the small on-toggle is the default size's anatomy at a smaller
    // scale: a pill and a plain knob, no mark.
    // The knob is a `Pill` for the same reason as the radio box: Carbon's
    // toggle handle is a `border-radius: 50%` circle at both sizes (18
    // default, 10 small), and 18 is past what any fixed per-role radius can
    // push to a stadium. Naming the shape keeps both sizes an exact circle
    // rather than a circle at one size and a rounded square at the other.
    let mut knob = swatch(
        "knob",
        handle,
        handle,
        // Carbon: `$icon-on-color`, white with no `--checked` override.
        // `TEXT_ON_ACCENT` is the theme's own `surface.base` and drew this
        // knob black in dark — see the module doc.
        Some(if on { TEXT_ON_COLOR } else { TEXT_PRIMARY }),
        None,
        Some(corner_for(CornerRole::Pill, handle)),
    )
    .with_transition(crate::anim::TOGGLE_KNOB);
    // Carbon: `.cds--toggle--disabled .cds--toggle__switch::before {
    // background-color: $icon-on-color-disabled; }` — one fill for both
    // states, because Carbon's own rule does not condition on `checked`
    // either (`_toggle.scss`).
    knob.props
        .tokens
        .insert("background@disabled".into(), t(ICON_ON_COLOR_DISABLED));

    let pad_start = swatch("pad-start", start, track_h, None, None, None);

    let mut track = stack(
        "track",
        Axis::Horizontal,
        None,
        vec![
            pad_start,
            knob,
            swatch("pad-end", end, track_h, None, None, None),
        ],
    );
    track.props.align = Some(Align::Center);
    track.props.tokens.insert(
        "background".into(),
        t(if on { ACCENT_PRIMARY } else { SURFACE_RAISED }),
    );
    track.props.tokens.insert("border".into(), t(BORDER_STRONG));
    // FR-022: the track is a stadium at both sizes. Carbon writes it as a
    // fixed 12px on the 24-tall track, which is exactly half that height,
    // and the small track keeps the same shape at its own height. That is
    // `Pill`, not a fixed radius that happens to be large: routed through
    // `Grouping` the track drew a 4-radius box around a circular knob, which
    // is what the rasterized toggle showed on 2026-09-09.
    track
        .props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Pill, track_h)));
    // Carbon: `.cds--toggle--disabled .cds--toggle__switch { background-color:
    // button.$button-disabled; }`, on and off alike (`_toggle.scss`).
    // `$button-disabled` is Carbon's own value, not a fade of the accent or
    // of `icon-primary` — see `BUTTON_DISABLED`.
    track
        .props
        .tokens
        .insert("background@disabled".into(), t(BUTTON_DISABLED));
    track = track.with_constraints(pinned(track_w, track_h));

    let mut label_node = text("label", label.clone());
    label_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    // Carbon: `.cds--toggle--disabled .cds--toggle__label-text,
    // .cds--toggle--disabled .cds--toggle__text { color: $text-disabled; }`
    // covers both the label and the On/Off word below — `$text-disabled` is
    // byte-identical to `$icon-disabled` in every theme (see `labelled_box`).
    label_node
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_DISABLED));
    let mut state = text("state", if on { "On" } else { "Off" });
    state
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_DISABLED));

    let mut appearance = stack(
        "appearance",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![track, state],
    );
    appearance.props.align = Some(Align::Center);

    let column = stack(
        key,
        Axis::Vertical,
        Some(SPACING_05),
        vec![label_node, appearance],
    );
    // Lends its text for `labelled_box`'s reason: Carbon's toggle wraps both
    // the caption and the On/Off word in the `<label>`, not in the button.
    let mut node = column
        .interactive(
            Role::Button,
            label,
            &[Interaction::Focus, Interaction::Click],
        )
        // Spec 010 FR-013: the same declaration a checkbox makes, because a
        // toggle is the same gesture drawn differently.
        .with_behaviour(TOGGLES_ON_RELEASE);
    node.semantics.selected = on;
    node
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_PRIMARY, BUTTON_DISABLED, ICON_DISABLED, ICON_ON_COLOR_DISABLED, SPACING_05,
    };
    use super::{
        CHECKBOX_BOX, RADIO_BOX, TOGGLE_SM_TRACK_H, TOGGLE_SM_TRACK_W, TOGGLE_TRACK_H,
        TOGGLE_TRACK_W, checkbox, checkbox_group, checkbox_indeterminate, checkbox_readonly,
        checkbox_warning, radio, radio_group, radio_warning, toggle, toggle_sm,
    };
    use crate::component::{IconMark, IconTone, icon_toned};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn pin(node: &ViewNode) -> (f32, f32) {
        (
            node.constraints.horizontal.min.expect("pinned width"),
            node.constraints.vertical.min.expect("pinned height"),
        )
    }

    #[test]
    fn checkbox_box_is_sixteen() {
        let off = checkbox("c", "Off", false);
        let on = checkbox("c", "On", true);
        assert_eq!(pin(named(&off, "box")), (CHECKBOX_BOX, CHECKBOX_BOX));
        assert_eq!(pin(named(&on, "box")), (CHECKBOX_BOX, CHECKBOX_BOX));
        assert_eq!(CHECKBOX_BOX, 16.0);
    }

    #[test]
    fn radio_box_is_eighteen() {
        let off = radio("r", "Off", false);
        let on = radio("r", "On", true);
        assert_eq!(pin(named(&off, "box")), (RADIO_BOX, RADIO_BOX));
        assert_eq!(pin(named(&on, "box")), (RADIO_BOX, RADIO_BOX));
        assert_eq!(RADIO_BOX, 18.0);
    }

    #[test]
    fn checkbox_indeterminate_is_mixed_not_selected() {
        let node = checkbox_indeterminate("c", "Partial");
        assert!(
            !node.semantics.selected,
            "mixed is not selected; do not fake it as on"
        );
        assert_eq!(node.semantics.value.as_deref(), Some("mixed"));
        assert_eq!(pin(named(&node, "box")), (CHECKBOX_BOX, CHECKBOX_BOX));
        let box_node = named(&node, "box");
        assert_eq!(
            box_node.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        named(&node, "dash");
    }

    #[test]
    fn a_checked_checkbox_nests_a_tick_inside_the_keyed_box() {
        let node = checkbox("c", "On", true);
        let box_node = named(&node, "box");
        assert_eq!(
            box_node.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        named(&node, "tick");
        let off = checkbox("c", "Off", false);
        assert!(
            !named(&off, "box").props.tokens.contains_key("background"),
            "unchecked box stays empty"
        );
    }

    /// A selected radio is a solid accent disc; an unselected one is an empty
    /// ring. The two states must differ by **fill**, not only by hue.
    ///
    /// The operator is red-green colour blind, so a difference carried by
    /// colour alone is a difference he cannot see. Filled against empty is a
    /// shape channel: it survives greyscale, and it is what this asserts.
    ///
    /// It also asserts the box has no children, which is the [`radio`] doc's
    /// half-pixel argument made executable. Nesting a 9px dot in the 18px
    /// circle puts the centring inset on 4.5 and the painter snaps the mark
    /// off-centre at 1x. Any future child here reintroduces that unless its
    /// size keeps `(18 - size) / 2` whole, so the assertion names the
    /// constraint rather than the one shape that happened to break it.
    #[test]
    fn a_selected_radio_is_a_solid_disc_and_an_unselected_one_is_an_empty_ring() {
        let on = radio("r", "On", true);
        let box_node = named(&on, "box");
        assert_eq!(
            box_node.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        assert!(
            box_node.children.is_empty(),
            "the selected radio nests {} child(ren). An inner mark of width w \
             centres on an inset of (18 - w) / 2, and any odd w puts that on a \
             half-pixel the 1x painter snaps away from centre — the defect the \
             operator reported on 2026-09-04.",
            box_node.children.len()
        );

        let off = radio("r", "Off", false);
        let off_box = named(&off, "box");
        assert!(
            !off_box.props.tokens.contains_key("background"),
            "an unselected radio must stay unfilled, or selection is carried \
             by hue alone and a colour blind reader loses it"
        );
        assert!(
            off_box.props.tokens.contains_key("border"),
            "an unfilled radio still has to be visible, so it keeps its ring"
        );
    }

    #[test]
    fn checkbox_readonly_is_not_disabled() {
        let node = checkbox_readonly("c", "Locked", true);
        assert!(node.semantics.read_only);
        assert!(!node.semantics.disabled);
        assert!(node.semantics.selected);
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(!node.interactions.contains(&Interaction::Click));
    }

    #[test]
    fn checkbox_group_is_a_legend_plus_items() {
        let node = checkbox_group(
            "g",
            "Options",
            vec![checkbox("a", "A", true), checkbox("b", "B", false)],
        );
        assert!(!node.is_interactive());
        assert!(node.semantics.role.is_none());
        assert_eq!(
            named(&node, "legend").props.text.as_deref(),
            Some("Options")
        );
        named(&node, "a");
        named(&node, "b");
    }

    #[test]
    fn radio_group_is_vertical_and_does_not_own_exclusivity() {
        let node = radio_group(
            "g",
            "Pick one",
            vec![radio("a", "A", true), radio("b", "B", true)],
        );
        assert!(!node.is_interactive());
        assert_eq!(node.props.axis, Some(Axis::Vertical));
        assert_eq!(
            named(&node, "items")
                .props
                .spacing
                .as_ref()
                .map(|t| t.as_str()),
            Some(SPACING_05)
        );
        // Both items may be selected: the group does not clear siblings.
        assert!(named(&node, "a").semantics.selected);
        assert!(named(&node, "b").semantics.selected);
    }

    /// Colour is not the only channel: the helper says `Warning: {message}`
    /// and sits a WarningFilled mark beside it. The wrapper is not a
    /// four-sided box; a checkbox's outline is already its own.
    fn assert_warning_helper(node: &ViewNode, message: &str) {
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Vertical));
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal Button"
        );
        assert!(
            !node.props.tokens.contains_key("border"),
            "a warning control is not a four-sided box; the helper is the channel"
        );
        let message_node = named(node, "message");
        let expected_text = format!("Warning: {message}");
        assert_eq!(
            message_node.props.text.as_deref(),
            Some(expected_text.as_str())
        );
        let mark = named(node, "mark");
        let expected = icon_toned("mark", IconMark::WarningFilled, IconTone::Primary);
        assert_eq!(mark.kind, NodeKind::Canvas);
        assert_eq!(
            mark.props.canvas, expected.props.canvas,
            "the helper's glyph is WarningFilled, not ErrorFilled and not a \
             swatch"
        );
    }

    #[test]
    fn checkbox_warning_says_so_in_words_and_a_glyph() {
        let node = checkbox_warning("c", "Agree", false, "required");
        assert_warning_helper(&node, "required");
        assert!(!named(&node, "checkbox").semantics.selected);
        named(&node, "box");
        named(&node, "label");
        let on = checkbox_warning("c", "Agree", true, "looks old");
        assert!(named(&on, "checkbox").semantics.selected);
        assert_warning_helper(&on, "looks old");
    }

    #[test]
    fn radio_warning_says_so_in_words_and_a_glyph() {
        let node = radio_warning("r", "Other", false, "pick one");
        assert_warning_helper(&node, "pick one");
        assert!(!named(&node, "radio").semantics.selected);
        named(&node, "box");
        named(&node, "label");
        let on = radio_warning("r", "Other", true, "unusual choice");
        assert!(named(&on, "radio").semantics.selected);
        assert_warning_helper(&on, "unusual choice");
    }

    #[test]
    fn toggle_track_stays_carbon_geometry() {
        let default_node = toggle("t", "On", false);
        let default = named(&default_node, "track");
        assert_eq!(pin(default), (TOGGLE_TRACK_W, TOGGLE_TRACK_H));
        assert_eq!((TOGGLE_TRACK_W, TOGGLE_TRACK_H), (48.0, 24.0));
        let small_node = toggle_sm("t", "On", false);
        let small = named(&small_node, "track");
        assert_eq!(pin(small), (TOGGLE_SM_TRACK_W, TOGGLE_SM_TRACK_H));
        assert_eq!((TOGGLE_SM_TRACK_W, TOGGLE_SM_TRACK_H), (32.0, 16.0));
        let on_node = toggle("t", "On", true);
        let keys: Vec<&str> = named(&on_node, "track")
            .children
            .iter()
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(keys, ["pad-start", "knob", "pad-end"]);
    }

    /// DEPARTURE FROM CARBON, operator decision 2026-09-05: no toggle draws
    /// a check mark, in any size or state.
    ///
    /// Carbon's small on-toggle does — `Toggle.js` renders
    /// `.cds--toggle__check` under `isSm && !readOnly`, and
    /// `ignored/carbon-ref/shots/36-toggle.png` shows the tick inside the
    /// white knob. The operator asked twice for it to go and chose deletion
    /// over Carbon's placement when both were put to him. This pins the
    /// decision so a later Carbon-conformance pass has to argue with a named
    /// test rather than quietly put the mark back.
    ///
    /// Asserted as "no canvas anywhere under the appearance", not as "no
    /// node keyed tick": the mark could come back under any key, and a
    /// `NodeKind::Canvas` is the only way a toggle can draw a glyph at all.
    #[test]
    fn no_toggle_draws_a_glyph_in_any_size_or_state() {
        fn canvases(node: &ViewNode, into: &mut Vec<String>) {
            if node.kind == NodeKind::Canvas {
                into.push(node.key.as_str().to_owned());
            }
            for child in &node.children {
                canvases(child, into);
            }
        }
        for (name, node) in [
            ("default off", toggle("t", "Toggle", false)),
            ("default on", toggle("t", "Toggle", true)),
            ("small off", toggle_sm("t", "Toggle", false)),
            ("small on", toggle_sm("t", "Toggle", true)),
        ] {
            let mut found = Vec::new();
            canvases(&node, &mut found);
            assert!(
                found.is_empty(),
                "{name} draws {found:?}; the operator asked for a bare pill                  and knob"
            );
            let knob = named(&node, "knob");
            assert!(
                knob.children.is_empty(),
                "{name}: the knob is a leaf, not a carrier"
            );
            assert_eq!(pin(knob).0, pin(knob).1, "{name}: the knob is square");
        }
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        // Toggle's knob carries `crate::anim::TOGGLE_KNOB`; unlike checkbox
        // and radio (no transitions), a lone toggle needs the shipped
        // animation registry declared or tree acceptance refuses it.
        crate::anim::shipped_registry().declare_into(&mut registry);
        registry
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D across every checkbox state: checked, unchecked,
    /// indeterminate, read-only, and disabled. Radio and toggle are out of
    /// scope for this pass (see this module's own doc header).
    #[test]
    fn checkbox_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("checked", checkbox("c", "Checked", true)),
            ("unchecked", checkbox("c", "Unchecked", false)),
            ("indeterminate", checkbox_indeterminate("c", "Partial")),
            ("readonly", checkbox_readonly("c", "Locked", true)),
            (
                "disabled",
                crate::component::disabled(checkbox("c", "Unavailable", false)),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
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
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: an enabled checkbox (checked, unchecked, indeterminate,
    /// read-only) declares `Focus` and is reachable; a disabled one is not.
    #[test]
    fn checkbox_focus_reachability_matches_disabled_state() {
        for (label, node, should_be_focusable) in [
            ("checked", checkbox("c", "Checked", true), true),
            ("readonly", checkbox_readonly("c", "Locked", true), true),
            (
                "disabled",
                crate::component::disabled(checkbox("c", "Unavailable", false)),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let root_placement = frame
                .placements
                .iter()
                .find(|p| p.id == "/root/c")
                .expect("checkbox row is placed");
            let reachable = focus.order().iter().any(|id| id == &root_placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the label text and (for checked/indeterminate) the mark
    /// inside the box, against the page ground the row sits on, in both
    /// themes, read through `Props.opacity`.
    #[test]
    fn checkbox_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [
                checkbox("c", "Checked", true),
                checkbox("c", "Unchecked", false),
                checkbox_indeterminate("c", "Partial"),
            ] {
                let label = named(&node, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "label at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }

    // Radio is out of scope for the checkbox pass above (see this module's
    // own doc header and its own comment on `checkbox_frame_geometry_has_
    // no_degenerate_or_overflowing_placements`); it is audited here.
    // Unlike checkbox, radio binds no `@hover` or other state-decorated
    // token at all (slice-d.md: "Radio button does NOT define hover as a
    // distinct rule (no `:hover` selector anywhere in the file)"), so
    // Class 1 does not apply here, and `empty_mark`/`marked_box` never nest
    // text inside a pinned box, so Class 4 does not apply either — both
    // checked here structurally by the fact that `radio`'s children are a
    // swatch and a dot swatch, never a `text` node inside a pinned box.

    /// Check C/D across every radio state: selected, unselected, and
    /// disabled.
    #[test]
    fn radio_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("selected", radio("r", "Chosen", true)),
            ("unselected", radio("r", "Not chosen", false)),
            (
                "disabled",
                crate::component::disabled(radio("r", "Unavailable", false)),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
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
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: an enabled radio declares `Focus` and is reachable; a
    /// disabled one is not.
    #[test]
    fn radio_focus_reachability_matches_disabled_state() {
        for (label, node, should_be_focusable) in [
            ("selected", radio("r", "Chosen", true), true),
            ("unselected", radio("r", "Not chosen", false), true),
            (
                "disabled",
                crate::component::disabled(radio("r", "Unavailable", false)),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let root_placement = frame
                .placements
                .iter()
                .find(|p| p.id == "/root/r")
                .expect("radio row is placed");
            let reachable = focus.order().iter().any(|id| id == &root_placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the label text against the page ground the row sits on, in
    /// both themes, read through `Props.opacity`.
    #[test]
    fn radio_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [radio("r", "Chosen", true), radio("r", "Not chosen", false)] {
                let label = named(&node, "label");
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "label at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }

    // Toggle is out of scope for the checkbox/radio passes above (this
    // module's own doc header, and the two comments before their own
    // geometry tests); it is audited here, this group's own component.
    // Unlike checkbox/radio, `toggle_sized` never nests a `text` node inside
    // a pinned box either — the track's only pinned child positions are the
    // swatch pads and the circular knob, and the label/state words sit
    // outside any pinned `Constraints` entirely — so Class 4 does not apply
    // structurally here, the same reasoning `controls.rs`'s radio comment
    // already gives. Class 1 does not apply either: slice-f.md is explicit
    // that Toggle "Does NOT have hover as a distinct documented state"
    // (docs list only on/off/focus/disabled/read-only/skeleton; SCSS has no
    // `:hover` rule on the switch itself), and `toggle_sized` binds no
    // `@hover` (or any other `@state`) token at all on the track — there is
    // no state-decorated binding to leave silent.

    /// Check C/D across every toggle state and both sizes: on, off,
    /// disabled, and the small variant on.
    #[test]
    fn toggle_frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("on", toggle("t", "Autosave", true)),
            ("off", toggle("t", "Autosave", false)),
            (
                "disabled",
                crate::component::disabled(toggle("t", "Autosave", false)),
            ),
            ("small-on", toggle_sm("t", "Compact mode", true)),
            ("small-off", toggle_sm("t", "Compact mode", false)),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
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
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: an enabled toggle (on or off, either size) declares `Focus`
    /// and is reachable; a disabled one is not.
    #[test]
    fn toggle_focus_reachability_matches_disabled_state() {
        for (label, node, should_be_focusable) in [
            ("on", toggle("t", "Autosave", true), true),
            ("off", toggle("t", "Autosave", false), true),
            (
                "disabled",
                crate::component::disabled(toggle("t", "Autosave", false)),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let root_placement = frame
                .placements
                .iter()
                .find(|p| p.id == "/root/t")
                .expect("toggle column is placed");
            let reachable = focus.order().iter().any(|id| id == &root_placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the label text and the On/Off state text against the page
    /// ground the column sits on, in both themes, read through
    /// `Props.opacity`.
    #[test]
    fn toggle_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            for node in [
                toggle("t", "Autosave", true),
                toggle("t", "Autosave", false),
            ] {
                for key in ["label", "state"] {
                    let text_node = named(&node, key);
                    let fg_name = text_node
                        .props
                        .tokens
                        .get("foreground")
                        .unwrap_or_else(|| panic!("{key} binds a foreground"));
                    let opacity = text_node.props.opacity.unwrap_or(1.0);
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{key} at {ratio:.2}:1 against page ground fails AA {MIN_TEXT_CONTRAST}:1"
                    );
                }
            }
        }
    }

    /// T0.1: `grep -c "@disabled" controls.rs` used to be 0. This reads the
    /// bindings back rather than the frame, so removing an `insert` in
    /// `empty_mark`/`marked_box`/`labelled_box` fails here even though the
    /// geometry tests above (which only check that *something* painted)
    /// would stay green.
    ///
    /// Carbon: `_checkbox.scss`'s `:disabled + label::before { border-color:
    /// $icon-disabled }`, its `checked:disabled ... { background-color:
    /// $icon-disabled }`, and its `:disabled + label { color: $text-disabled
    /// }` (byte-identical to `$icon-disabled` in every published theme).
    #[test]
    fn a_disabled_checkbox_binds_the_disabled_ink_family() {
        let off = checkbox("c", "Off", false);
        let off_box = named(&off, "box");
        assert_eq!(
            off_box
                .props
                .tokens
                .get("border@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED),
            "an unchecked checkbox is nothing but its outline, so the \
             outline must fade when disabled"
        );
        assert!(
            !off_box.props.tokens.contains_key("background@disabled"),
            "an unchecked box carries no live background, so it must not \
             gain a disabled one either"
        );

        let on = checkbox("c", "On", true);
        let on_box = named(&on, "box");
        assert_eq!(
            on_box
                .props
                .tokens
                .get("background@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED),
            "a checked-and-disabled box must swap the accent fill for the \
             faded tone, or it reads as a live, pressable checkbox"
        );
        assert_eq!(
            on_box
                .props
                .tokens
                .get("border@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED)
        );

        for (label, node) in [("off", off), ("on", on)] {
            let label_node = named(&node, "label");
            assert_eq!(
                label_node
                    .props
                    .tokens
                    .get("foreground@disabled")
                    .map(|t| t.as_str()),
                Some(ICON_DISABLED),
                "{label}: the label text must fade under disabled too"
            );
        }
    }

    /// See [`a_disabled_checkbox_binds_the_disabled_ink_family`]. Carbon:
    /// `_radio-button.scss`'s `:disabled` selectors on
    /// `.radio-button__appearance` (`border-color: $icon-disabled`) and on
    /// its label (`color: $text-disabled`).
    #[test]
    fn a_disabled_radio_binds_the_disabled_ink_family() {
        let off = radio("r", "Off", false);
        let off_box = named(&off, "box");
        assert_eq!(
            off_box
                .props
                .tokens
                .get("border@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED),
            "an unselected radio is nothing but its ring, so the ring must \
             fade when disabled"
        );

        let on = radio("r", "On", true);
        let on_box = named(&on, "box");
        assert_eq!(
            on_box
                .props
                .tokens
                .get("background@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED),
            "a selected-and-disabled radio must swap its accent fill for \
             the faded tone, or it reads as a live, pressable radio"
        );
        assert_eq!(
            on_box
                .props
                .tokens
                .get("border@disabled")
                .map(|t| t.as_str()),
            Some(ICON_DISABLED)
        );

        for (label, node) in [("off", off), ("on", on)] {
            let label_node = named(&node, "label");
            assert_eq!(
                label_node
                    .props
                    .tokens
                    .get("foreground@disabled")
                    .map(|t| t.as_str()),
                Some(ICON_DISABLED),
                "{label}: the label text must fade under disabled too"
            );
        }
    }

    /// See [`a_disabled_checkbox_binds_the_disabled_ink_family`]. Carbon:
    /// `_toggle.scss`'s `.cds--toggle--disabled .cds--toggle__switch {
    /// background-color: button.$button-disabled }` (a value Carbon
    /// publishes on its own, not a fade — see [`BUTTON_DISABLED`]'s doc),
    /// its nested `::before { background-color: $icon-on-color-disabled }`,
    /// and its `.cds--toggle__label-text`/`.cds--toggle__text { color:
    /// $text-disabled }` pair, on both on and off.
    #[test]
    fn a_disabled_toggle_binds_the_disabled_ink_family() {
        for (label, node) in [
            ("off", toggle("t", "Autosave", false)),
            ("on", toggle("t", "Autosave", true)),
        ] {
            let track = named(&node, "track");
            assert_eq!(
                track
                    .props
                    .tokens
                    .get("background@disabled")
                    .map(|t| t.as_str()),
                Some(BUTTON_DISABLED),
                "{label}: the track must swap its fill for Carbon's own \
                 disabled grey, on and off alike"
            );

            let knob = named(&node, "knob");
            assert_eq!(
                knob.props
                    .tokens
                    .get("background@disabled")
                    .map(|t| t.as_str()),
                Some(ICON_ON_COLOR_DISABLED),
                "{label}: the knob must fade to the on-colour disabled ink"
            );

            for key in ["label", "state"] {
                let text_node = named(&node, key);
                assert_eq!(
                    text_node
                        .props
                        .tokens
                        .get("foreground@disabled")
                        .map(|t| t.as_str()),
                    Some(ICON_DISABLED),
                    "{label}: {key} text must fade under disabled too"
                );
            }
        }
    }
}
