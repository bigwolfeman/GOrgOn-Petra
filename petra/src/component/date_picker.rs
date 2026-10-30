//! Carbon Date picker (slice-b). Closed field plus a one-month calendar
//! popover. No date library.
//!
//! Anatomy (`_date-picker.scss` + `_flatpickr.scss`):
//! 1. Field — the shared field well ([`super::field::bind_field_chrome`]:
//!    a fill and a bottom rule, no box, no radius), height md 40 on the
//!    field ramp ([`date_picker_sm`] / [`date_picker_lg`] take 32 / 48).
//! 2. Current value (visible text).
//! 3. Calendar mark — [`IconMark::Calendar`] in [`IconTone::Primary`]
//!    (`.cds--date-picker__icon`, `fill: $icon-primary`, 16×16, slice-b),
//!    carrying the word `calendar` as its accessible name so the glyph is
//!    never the only channel (FR-026).
//! 4. Open calendar — a `Layer::Popup` surface flush under the field:
//!    a month header (`< August 2026 >`), a 7-column weekday-initial row,
//!    and six week rows of the real Gregorian month.
//!
//! Calendar menu is a fixed 288 wide (SCSS), independent of field size.
//! Day cells are 40x40 ([`SIZE_MD`]).
//!
//! # What this used to be, and why none of it survived
//!
//! Day buttons `1..=28`, on the reasoning that "28 days is enough to prove
//! the anatomy". Measured on the capture, it proved something else. The
//! grid declared seven 40px columns and the cells inside them were **8px
//! wide** — a `Grid` leaves a cell at its natural size unless told to
//! stretch, so every day was a hit box the width of its own digits and the
//! four columns of whitespace between them answered nothing. The panel hung
//! 92px to the inline-start of the field, because it borrowed
//! [`super::popover::popover_with`], which centres on its anchor; Carbon's
//! calendar is flush under the field's leading edge. And that popover put
//! its `"^"` caret — a *text glyph* — where Carbon puts the month and year.
//! Four weeks of 1..28 also matches no month that has ever existed, so the
//! weekday initials sat above the wrong columns.
//!
//! The month now comes from `value`, which is already `YYYY-MM-DD`: the
//! component reads the month it is being asked to show rather than being
//! told twice.
//!
//! # One shape, closed and open
//!
//! Both forms are a column keyed `key` holding `field`, plus `calendar`
//! while open — the same arrangement [`super::select`] and
//! [`super::dropdown`] settled on. Until 2026-09-05 the closed form *was*
//! the field (`/due`) and the open form wrapped it (`/due/field`), so the
//! id keyboard focus sat on stopped being focusable the moment the
//! calendar opened, and the vanished-focus rule sent focus to the next
//! reachable node on the page: the operator's "when I click it the cursor
//! flies away". Carbon keeps focus on the input while the calendar is
//! open. The field declares [`FocusFigure::Hug`], so that focus is two
//! bars beside the field and never an underline across the calendar's top.

use std::collections::BTreeSet;

use super::field::{
    SIZE_FLUID, field, field_labeled, hinted, invalid_helper, labeled, muted_label,
    validated_column, valued, warning_helper,
};
use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::list_box_field;
use super::pad;
use super::pin_block;
use super::rule;
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_STRONG, LAYER_HOVER, SHADOW_OVERLAY, SIZE_LG, SIZE_MD, SIZE_SM,
    SPACING_02, SPACING_03, SPACING_04, SPACING_05, SUPPORT_ERROR, SUPPORT_WARNING,
    SURFACE_LAYER_TWO, SURFACE_RAISED, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY,
    TYPOGRAPHY_HEADING_SM, t,
};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Align as PropAlign, Anchor, AxisConstraint, Behaviour, ClampRule, Constraints, Edge,
    FocusFigure, FocusShownOn, InputPolicy, Intent, Interaction, Justify, Key, Layer, NodeKind,
    Phase, Props, Role, Tip, TrackSize, ViewNode,
};

/// Spec 010: field / month-chooser open-close.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

/// Spec 010: day / month cells — exclusive among siblings.
const SELECTS_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Select,
    phase: Phase::OnRelease,
};

/// Spec 010: month/year step arrows — one shot.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// Carbon calendar menu width (`18rem`). Independent of field size.
const CALENDAR_W: f32 = 288.0;
/// Carbon calendar menu height (`21rem`). Style-page and SCSS agree.
const CALENDAR_H: f32 = 336.0;
/// Week rows Carbon always draws, whatever the month's length. Six is what
/// it takes for a 31-day month that starts on a Saturday, and a calendar
/// whose height changed with the month would move the ground under the
/// pointer.
const WEEK_ROWS: usize = 6;

const WEEKDAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The month/year chooser fills the calendar's body exactly: everything
/// below the header it hangs from. Anything shorter would leave a band of
/// the day grid showing under an opaque panel, which reads as two grids
/// stacked rather than as one panel over one grid.
const CHOOSER_H: f32 = CALENDAR_H - SIZE_MD;
/// Rows of months in the chooser: twelve months, three to a row.
const MONTH_ROWS: usize = 4;
/// Columns of months in the chooser.
const MONTH_COLS: usize = 3;
/// One month cell's height: the chooser's body divided by its rows, so the
/// grid fills the panel rather than floating in the top of it.
const MONTH_CELL_H: f32 = (CHOOSER_H - SIZE_MD) / MONTH_ROWS as f32;

const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(CALENDAR_W == 288.0);
const _: () = assert!(CALENDAR_H == 336.0);
const _: () = assert!(WEEK_ROWS == 6);
const _: () = assert!(CHOOSER_H == 296.0);
const _: () = assert!(MONTH_ROWS * MONTH_COLS == 12);
const _: () = assert!(MONTH_CELL_H == 64.0);

const FIELD_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Closed date picker at Carbon md (40): a column keyed `key` holding the
/// field, keyed `"field"`. `label` is the field's accessible name; `value`
/// is the visible date text.
pub fn date_picker(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field("field", label, value, false, FieldForm::Default);
    column(key, field, None)
}

/// [`date_picker`] at Carbon sm: the well on the field ramp's `field_sm`
/// step (32). Slice-b "Sizes" gives the date-picker input the same three
/// heights as every field-shaped control — sm 32 / md 40 / lg 48 — and the
/// calendar is fixed at every one of them ("the calendar menu itself is a
/// fixed height and width regardless of input size"), so the step is the
/// well's alone.
#[must_use]
pub fn date_picker_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field_sized("field", label, value, false, FieldForm::Default, SIZE_SM);
    column(key, field, None)
}

/// [`date_picker`] at Carbon lg (48). See [`date_picker_sm`].
#[must_use]
pub fn date_picker_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let field = closed_field_sized("field", label, value, false, FieldForm::Default, SIZE_LG);
    column(key, field, None)
}

/// Read-only date picker: [`date_picker`] under [`super::readonly::readonly`].
///
/// The state's fact and verb drop are shared with the family's other
/// read-only markers (`Semantics.read_only` set, edit verbs and intent
/// dropped, `Focus` kept). The well itself keeps its resting fill here: the
/// no-fill read-only chrome [`super::field::FieldChrome::ReadOnly`] gives
/// Text input is not yet routed through this file's private well builder,
/// and this doc is where that boundary is recorded rather than left silent.
pub fn date_picker_readonly(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    super::readonly::readonly(date_picker(key, label, value))
}

/// Invalid date picker: the field wearing [`SUPPORT_ERROR`] on all four
/// sides plus the shared [`invalid_helper`] row (FR-019).
///
/// The field keeps the key and depth [`date_picker`] gives it
/// (`{key}/field`), so a value that turns legal does not re-key the node
/// under the cursor, and the outline replaces the field's resting rule —
/// the one shared dressing
/// ([`super::field::bind_validation_chrome`]).
pub fn date_picker_invalid(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    validated_column(
        date_picker(key, label, value),
        SUPPORT_ERROR,
        invalid_helper(message),
    )
}

/// Warning date picker: the field wearing [`SUPPORT_WARNING`] plus the
/// shared [`warning_helper`] row (FR-018/FR-020).
pub fn date_picker_warning(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    message: impl Into<String>,
) -> ViewNode {
    validated_column(
        date_picker(key, label, value),
        SUPPORT_WARNING,
        warning_helper(message),
    )
}

/// Simple date input: the text field and nothing else — no calendar, and
/// no calendar mark (slice-b: the mark is "present only on calendar-backed
/// variants").
///
/// Carbon's `datePickerType="simple"` is the plain text input
/// (`.cds--date-picker__input` **is** `.cds--text-input`), for memorable or
/// approximate dates — a date of birth typed `1989-04-12` rather than
/// picked off a grid. So this is the library's filled, visible-labelled
/// field under the date picker's own name: [`field_labeled`] for the
/// outside label + well anatomy (also what "Default vs Fluid" calls the
/// default style), [`valued`] for the date text and its content ink. The
/// constructor exists so the wire surface and the manifest row have one
/// name for the unit, not because the well differs from a text input's —
/// it does not, and Carbon's does not either.
pub fn date_picker_input(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    valued(field_labeled(key, label), value)
}

/// Fluid date picker: the closed, calendar-backed field wearing Carbon
/// Fluid's dressing — 64 tall, the muted label stacked inside the well
/// over the value and the calendar mark.
///
/// One date picker, two dressings (slice-b "Default vs Fluid"): the
/// difference is the label's place and the well's step, not the control,
/// so this shares [`closed_field`] with [`date_picker`] and takes only the
/// [`FieldForm::Fluid`] arm — same mark, same toggle, same focus figure.
pub fn date_picker_fluid(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    column(
        key,
        closed_field("field", label, value, false, FieldForm::Fluid),
        None,
    )
}

/// Carbon's time picker (slice-b): the hour:minute field in a group with
/// the two optional selects the format calls for — `clock` carries the
/// AM/PM value, `zone` the timezone; `None` drops that part and widens
/// what is left, per the slice's measured width shares (`time-input` 25%
/// or 50%, `clock-select` 25% or 50%, `timezone-select` 50%).
///
/// The one label above the group is the group's and the field's accessible
/// name (Carbon's `TimePickerSelect` hides its own labels); the two
/// selects are bare wells ([`list_box_field`], the same one `select` wraps
/// in a caption) carrying their words as accessible names — `AM/PM` and
/// `timezone` — so the parts are never named by position alone. Parts sit
/// flush against a hairline [`rule`] apiece ("divider width 1px"), the
/// widths being grid weights rather than px, because the shares are
/// fractions of the group and the group is whatever width the page gives
/// it.
pub fn time_picker(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    clock: Option<&str>,
    zone: Option<&str>,
) -> ViewNode {
    let label = label.into();
    let field = hinted(field("field", label.clone()), "hh:mm");
    let mut columns = vec![TrackSize::Weight {
        weight: if clock.is_some() { 1.0 } else { 2.0 },
    }];
    let mut children = vec![valued(field, value)];
    if let Some(clock) = clock {
        columns.push(TrackSize::FitContent);
        children.push(rule("div-meridiem", Axis::Vertical, BORDER_STRONG));
        columns.push(TrackSize::Weight { weight: 1.0 });
        children.push(list_box_field(
            "meridiem",
            "AM/PM",
            clock,
            SIZE_MD,
            IconMark::ChevronDown,
            false,
        ));
    }
    if let Some(zone) = zone {
        columns.push(TrackSize::FitContent);
        children.push(rule("div-timezone", Axis::Vertical, BORDER_STRONG));
        columns.push(TrackSize::Weight { weight: 2.0 });
        children.push(list_box_field(
            "timezone",
            "timezone",
            zone,
            SIZE_MD,
            IconMark::ChevronDown,
            false,
        ));
    }
    let group = ViewNode::new(NodeKind::Grid, "group").with_props(Props {
        columns,
        ..Props::default()
    });
    labeled(key, label, group.with_children(children))
}

/// What an open calendar is showing: which month the grid is browsing, and
/// which of the two forms draws its header.
///
/// The month is carried here rather than read off `value` because browsing
/// and choosing are two different things. Carbon's arrows turn the month on
/// show and leave the selected date alone; a component that took only
/// `value` could not say "August is selected, September is on screen", so
/// the only way to step a month was to move the selection with it.
///
/// Two forms, because the operator asked for two: *"we need 2 date pickers
/// 'compact' and 'full' where compact is what we have now, and full lets me
/// click the month to get another menu that lets me pick month and year,
/// and in the full view the month is a button, not just a label."*
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Calendar {
    /// Carbon's own header: the month name is a caption between the two
    /// arrows (`.flatpickr-current-month`, a `<span class="cur-month">`),
    /// browsing `year`-`month`.
    Compact {
        /// Year the grid is browsing.
        year: i32,
        /// Month the grid is browsing, `1..=12`.
        month: u32,
    },
    /// The month name is a button carrying a [`IconMark::ChevronDown`],
    /// keyed `month-button`. Pressing it is the caller's to turn into
    /// [`Calendar::Choosing`].
    Full {
        /// Year the grid is browsing.
        year: i32,
        /// Month the grid is browsing, `1..=12`.
        month: u32,
    },
    /// [`Calendar::Full`] with the month/year chooser raised over the day
    /// grid: a year stepper keyed `prev-year`/`next-year` over twelve month
    /// cells keyed `mon-1`..`mon-12`.
    Choosing {
        /// Year the chooser is on, which is also the year the grid beneath
        /// it is browsing.
        year: i32,
        /// Month the grid beneath is browsing, `1..=12`, and the cell the
        /// chooser marks selected.
        month: u32,
    },
}

impl Calendar {
    /// The month on show, whatever the form.
    fn view(self) -> (i32, u32) {
        match self {
            Self::Compact { year, month }
            | Self::Full { year, month }
            | Self::Choosing { year, month } => (year, month),
        }
    }

    /// Whether the header's month is a button rather than a caption.
    fn month_is_a_button(self) -> bool {
        !matches!(self, Self::Compact { .. })
    }

    /// Whether the chooser is over the day grid.
    fn choosing(self) -> bool {
        matches!(self, Self::Choosing { .. })
    }
}

/// Open calendar: the same column, its field expanded, plus the month
/// `calendar` names, flush under the field's leading edge.
///
/// The field child is keyed `"field"` in every form; the calendar is keyed
/// `"calendar"` and anchored to `"field"`. A day cell is keyed `day-<n>`
/// and an adjacent-month cell `adj-<i>`, so a caller matching `day-` picks
/// up exactly the days of the month on show.
///
/// `value` is read as `YYYY-MM-DD` for the *selected* day, which is marked
/// only when it falls in the month `calendar` is browsing. A value that
/// does not parse marks nothing and still draws the month asked for, which
/// is visibly wrong in the field and never wrong in the grid.
///
/// A one-day wrapper over [`date_picker_showing_selection`], kept as its
/// own public constructor because its callers — the registry row that
/// crosses the Lua wire among them — already depend on this exact
/// signature.
pub fn date_picker_showing(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    calendar: Calendar,
) -> ViewNode {
    let value = value.into();
    date_picker_showing_selection(key, label, value.clone(), std::iter::once(value), calendar)
}

/// [`date_picker_showing`] widened to mark every day in `selected`, not
/// only the single day `value` names. `value` still drives the field's own
/// visible text, unchanged from [`date_picker_showing`]; `selected` is the
/// full set the grid marks, each entry read as `YYYY-MM-DD`. An entry that
/// does not parse, or that falls outside `calendar`'s browsed month, marks
/// nothing for that entry — the same "wrong value marks nothing" rule
/// [`date_picker_showing`] keeps for its own single day.
///
/// A marked day whose immediate calendar neighbour on either side is absent
/// from `selected` is a **run's edge** and renders in the full accent fill
/// ([`ACCENT_PRIMARY`]). A marked day held on both sides renders in
/// [`SURFACE_LAYER_TWO`] instead — one step past whatever [`SURFACE_RAISED`]
/// (the unmarked day's own fill) resolves to here, a luminance step rather
/// than a second hue, so the distinction survives red-green colour
/// blindness. This is [`SURFACE_LAYER_TWO`] and not the library's own
/// `layer-selected` on purpose: `layer-selected` is a fixed step off
/// `layer-one` with no notion of how deep this control is actually mounted
/// ([`super::on_layer`]'s own doc), and a host that re-seats the calendar
/// one layer up (every card-mounted component does) moves the unmarked
/// day's fill up to meet it — `layer-selected` and the re-seated resting
/// fill measured one L* apart on capture, not the seven the name promised.
/// `SURFACE_LAYER_TWO` is one of [`super::on_layer`]'s four recognised
/// names, so it re-seats by the same table `surface.raised` does and stays
/// one real step ahead of it at every depth. A day picked alone
/// (`Mode::Multi`, or `Mode::Range` before its second pick) is a run of one
/// and is therefore always an edge — every `Mode::Multi` pick reads fully
/// accented, and a completed range reads as an accented cap at each end
/// with the lighter band between them, which is Carbon's own range-picker
/// reading. Every marked day, edge or interior, still reports
/// `Semantics.selected = true`: the band is a fill choice, not a change in
/// what is selected.
pub fn date_picker_showing_selection(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    selected: impl IntoIterator<Item = impl AsRef<str>>,
    calendar: Calendar,
) -> ViewNode {
    showing_selection_with(key, label, value, selected, calendar, None)
}

/// Register row #170 (`today`): [`date_picker_showing`] with Carbon's
/// "Today" indicator dot on the day `today` names (`YYYY-MM-DD`) — the
/// calendar-day `today` state, slice-b "States > Calendar day".
///
/// The mark is drawn only when `today` falls in the month `calendar` is
/// browsing; a `today` outside it marks nothing, because the day is not on
/// screen (the same "wrong value marks nothing" rule
/// [`date_picker_showing`] keeps). A `today` that does not parse panics:
/// unlike `value`, it has no field to be visibly wrong in, and a silently
/// unmarked today is exactly the quiet failure this library does not ship.
///
/// Residual, named rather than papered over: the range forms
/// ([`date_picker_range_showing`]) do not carry a `today` yet — the day
/// engine below does, so a `date_picker_range_showing_today` is one wrapper
/// away when a caller needs it.
pub fn date_picker_showing_today(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    calendar: Calendar,
    today: impl AsRef<str>,
) -> ViewNode {
    let today = today.as_ref();
    let (ty, tm, td) = parse_date(today).unwrap_or_else(|| {
        panic!("date_picker_showing_today: `today` must be YYYY-MM-DD, got {today:?}")
    });
    let (year, month) = calendar.view();
    let today = (ty == year && tm == month).then_some(td);
    let value = value.into();
    showing_selection_with(
        key,
        label,
        value.clone(),
        std::iter::once(value),
        calendar,
        today,
    )
}

/// The one engine both single-day forms build through: `selected` is the
/// full set the grid marks and `today` is the day-of-month wearing the
/// "Today" dot, if it is in the browsed month at all.
fn showing_selection_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    selected: impl IntoIterator<Item = impl AsRef<str>>,
    calendar: Calendar,
    today: Option<u32>,
) -> ViewNode {
    let label = label.into();
    let value = value.into();
    let (year, month) = calendar.view();
    let all: BTreeSet<(i32, u32, u32)> = selected
        .into_iter()
        .filter_map(|s| parse_date(s.as_ref()))
        .collect();
    let days = SelectedDays::in_month(&all, year, month);
    let field = closed_field("field", label.clone(), value, true, FieldForm::Default);
    column(
        key,
        field,
        Some(calendar_surface(
            label, year, month, &days, calendar, "field", today,
        )),
    )
}

/// Closed range picker: two dated wells side by side, `start` and `end`,
/// sharing one calendar menu between them (slice-b "Calendar picker — Date
/// range": "two text fields (start/end) + calendar menu").
///
/// Each field keeps its own accessible name and its own calendar mark
/// (the mark is part of the documented date-input anatomy, and both
/// inputs here are calendar-backed); the pair sits 50/50, keyed `"fields"`
/// with the wells under it at `"start"` and `"end"`, so a caller routes to
/// the field it means and [`date_picker_range_showing`]'s calendar hangs
/// under the pair's leading edge — Carbon's flush-under-the-input rule
/// applied to the start of the group.
pub fn date_picker_range(
    key: impl Into<Key>,
    start_label: impl Into<String>,
    start: impl Into<String>,
    end_label: impl Into<String>,
    end: impl Into<String>,
) -> ViewNode {
    column(
        key,
        range_fields(start_label, start, end_label, end, false),
        None,
    )
}

/// [`date_picker_range`] with its shared calendar raised, marking every
/// day in `selected` the way [`date_picker_showing_selection`] marks them:
/// a run's ends accented, its interior one tone step up. `start` and `end`
/// drive the two fields' visible text and are not required to be the run's
/// ends — the marks come from `selected` alone, so the fields may show
/// what is typed while the grid shows what a drag or a two-click pick has
/// marked. The calendar's accessible name is `start_label`, the name of
/// the field the group is anchored under.
pub fn date_picker_range_showing(
    key: impl Into<Key>,
    start_label: impl Into<String>,
    start: impl Into<String>,
    end_label: impl Into<String>,
    end: impl Into<String>,
    selected: impl IntoIterator<Item = impl AsRef<str>>,
    calendar: Calendar,
) -> ViewNode {
    let start_label = start_label.into();
    let (year, month) = calendar.view();
    let all: BTreeSet<(i32, u32, u32)> = selected
        .into_iter()
        .filter_map(|s| parse_date(s.as_ref()))
        .collect();
    let days = SelectedDays::in_month(&all, year, month);
    column(
        key,
        range_fields(start_label.clone(), start, end_label, end, true),
        Some(calendar_surface(
            start_label,
            year,
            month,
            &days,
            calendar,
            "fields",
            None,
        )),
    )
}

/// The `"fields"` grid the two range forms share: 50/50, wells keyed
/// `"start"`/`"end"`, both wearing [`FieldForm::Default`].
fn range_fields(
    start_label: impl Into<String>,
    start: impl Into<String>,
    end_label: impl Into<String>,
    end: impl Into<String>,
    expanded: bool,
) -> ViewNode {
    let grid = ViewNode::new(NodeKind::Grid, "fields").with_props(Props {
        columns: vec![
            TrackSize::Weight { weight: 1.0 },
            TrackSize::Weight { weight: 1.0 },
        ],
        ..Props::default()
    });
    grid.with_children(vec![
        closed_field("start", start_label, start, expanded, FieldForm::Default),
        closed_field("end", end_label, end, expanded, FieldForm::Default),
    ])
}

/// Which days of the month [`calendar_surface`] is browsing carry a mark,
/// and which of those sit at the edge of a run of consecutive calendar days
/// rather than in its interior. See [`date_picker_showing_selection`] for
/// what the two draw as.
struct SelectedDays {
    /// Days of the browsed month present in the full selection.
    days: BTreeSet<u32>,
    /// The subset of [`Self::days`] with at least one calendar-adjacent
    /// neighbour (possibly in an adjoining month) missing from the full
    /// selection.
    edges: BTreeSet<u32>,
}

impl SelectedDays {
    /// `all` is the full selection, every calendar (year, month, day) it
    /// named that parsed; `year`/`month` is the month the grid is browsing.
    fn in_month(all: &BTreeSet<(i32, u32, u32)>, year: i32, month: u32) -> Self {
        let days: BTreeSet<u32> = all
            .iter()
            .filter(|&&(y, m, _)| y == year && m == month)
            .map(|&(_, _, d)| d)
            .collect();
        let edges = days
            .iter()
            .copied()
            .filter(|&d| {
                !all.contains(&prev_date(year, month, d))
                    || !all.contains(&next_date(year, month, d))
            })
            .collect();
        Self { days, edges }
    }

    fn fill(&self, day: u32) -> DayFill {
        if self.edges.contains(&day) {
            DayFill::Edge
        } else if self.days.contains(&day) {
            DayFill::Interior
        } else {
            DayFill::None
        }
    }
}

/// How a day in the grid is filled. [`date_picker_showing_selection`] names
/// what each spends and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DayFill {
    /// Not in the selection.
    None,
    /// In the selection, with both calendar-adjacent neighbours also in it.
    Interior,
    /// In the selection, and at least one calendar-adjacent neighbour is
    /// not.
    Edge,
}

impl DayFill {
    fn is_selected(self) -> bool {
        self != Self::None
    }
}

/// The calendar day immediately before `(year, month, day)`, crossing a
/// month or year boundary as Gregorian arithmetic requires.
fn prev_date(year: i32, month: u32, day: u32) -> (i32, u32, u32) {
    if day > 1 {
        (year, month, day - 1)
    } else if month > 1 {
        let prev_month = month - 1;
        (year, prev_month, days_in_month(year, prev_month))
    } else {
        (year - 1, 12, days_in_month(year - 1, 12))
    }
}

/// See [`prev_date`]; the calendar day immediately after.
fn next_date(year: i32, month: u32, day: u32) -> (i32, u32, u32) {
    let last = days_in_month(year, month);
    if day < last {
        (year, month, day + 1)
    } else if month < 12 {
        (year, month + 1, 1)
    } else {
        (year + 1, 1, 1)
    }
}

/// [`date_picker_showing`] browsing the month `value` names, in the compact
/// form: Carbon's calendar exactly.
///
/// `value` is read as `YYYY-MM-DD`. A value that does not parse cannot say
/// what month to show, so this falls back to January 1970 with nothing
/// selected — visibly wrong rather than quietly wrong.
pub fn date_picker_open(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
) -> ViewNode {
    let value = value.into();
    let (year, month, _) = parse_date(&value).unwrap_or((1970, 1, 1));
    date_picker_showing(key, label, value, Calendar::Compact { year, month })
}

/// The column both forms share: `field`, plus `calendar` while open. One
/// builder so the field's id cannot differ between the two.
fn column(key: impl Into<Key>, field: ViewNode, calendar: Option<ViewNode>) -> ViewNode {
    let open = calendar.is_some();
    let mut children = vec![field];
    children.extend(calendar);
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// Which Carbon dressing the dated well wears (slice-b "Default vs Fluid").
#[derive(Clone, Copy)]
enum FieldForm {
    /// Default: 40 tall, `padding` 16/8, and the label is the accessible
    /// name only — the outside/above label is a caller's wrapper business
    /// ([`labeled`]), as on [`date_picker`].
    Default,
    /// Fluid: 64 tall ([`SIZE_FLUID`, the same pin [`super::field::field_fluid`]
    /// takes), `padding` 16/12, and the muted label stacked *inside* the
    /// well at a 4-unit gap ([`SPACING_02`]) — Carbon Fluid's `label
    /// padding-bottom: 4px`. The 13px vertical inset the slice measures is
    /// not a step on the token ramp, so this takes the nearest one (12) and
    /// says so rather than inventing a token.
    Fluid,
}

impl FieldForm {
    fn height(self) -> f32 {
        match self {
            Self::Default => SIZE_MD,
            Self::Fluid => SIZE_FLUID,
        }
    }

    /// Vertical inset token: the default well's 8, the fluid well's 12.
    fn inset_v(self) -> &'static str {
        match self {
            Self::Default => SPACING_03,
            Self::Fluid => SPACING_04,
        }
    }
}

/// The dated well both dressings share: the value text and the calendar
/// mark, wearing the field chrome, the toggle behaviour and the pick-into
/// focus figure. `form` decides the dressing's inset and where the label
/// goes; the height is the form's own step ([`FieldForm::height`]).
fn closed_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    expanded: bool,
    form: FieldForm,
) -> ViewNode {
    closed_field_sized(key, label, value, expanded, form, form.height())
}

/// [`closed_field`] with the well pinned to a step of the field ramp
/// ([`date_picker_sm`] / [`date_picker_lg`]) rather than the form's default.
fn closed_field_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    expanded: bool,
    form: FieldForm,
    height: f32,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut mark = icon_toned("calendar-mark", IconMark::Calendar, IconTone::Primary);
    mark.semantics.label = Some("calendar".to_owned());

    let row_children = vec![value_node, mark];

    let mut node = match form {
        FieldForm::Default => {
            let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), row_children);
            node.props.align = Some(Align::Center);
            node
        }
        FieldForm::Fluid => {
            let mut row = stack("row", Axis::Horizontal, Some(SPACING_03), row_children);
            row.props.align = Some(Align::Center);
            stack(
                key,
                Axis::Vertical,
                Some(SPACING_02),
                vec![muted_label("label", label.clone()), row],
            )
        }
    };
    node.props.padding = Some(pad(SPACING_05, form.inset_v()));
    // The same well every text-shaped control in this library wears: a fill
    // and a bottom rule, and nothing on the other three sides.
    //
    // It drew a full `BORDER_SUBTLE` box with a 2-unit radius until
    // 2026-09-05, which is what the operator meant by *"there is still a
    // border on the date picker itself, remove it"* -- round 2 took the box
    // off the text input, the search well and the number well and left this
    // one behind. Carbon's `.cds--date-picker__input` **is**
    // `.cds--text-input` (slice-c), so this calls the same binder rather
    // than restating its two lines and drifting from them a second time.
    super::field::bind_field_chrome(&mut node.props);
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_block(height))
        .interactive(Role::Button, label, FIELD_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text();
    node.semantics.expanded = Some(expanded);
    // A well a person picks into: focus brackets its sides, as on a text
    // input, and never underlines into the calendar flush beneath it.
    node.semantics.focus_figure = FocusFigure::Sides;
    node.semantics.focus_shown_on = FocusShownOn::Well;
    node
}

/// `YYYY-MM-DD` as a triple, or `None` if it is not that.
///
/// Deliberately not a date library and deliberately not lenient: the only
/// producer is a caller that already formats the field's own text, so a
/// string this refuses is a caller bug and should look like one.
fn parse_date(value: &str) -> Option<(i32, u32, u32)> {
    let mut parts = value.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || parts.next().is_some() {
        return None;
    }
    Some((year, month, day))
}

/// Days in `month` of `year`, Gregorian.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
    }
}

/// The weekday of `year-month-01`, `0` for Sunday — Sakamoto's method.
///
/// Chosen over Zeller because it is a single table lookup and needs no
/// month/year shifting beyond January and February, which is the whole of
/// the correction below. Checked against known dates in this module's
/// tests rather than trusted.
fn first_weekday(year: i32, month: u32) -> usize {
    const OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let dow = (y + y / 4 - y / 100 + y / 400 + OFFSETS[(month - 1) as usize] + 1).rem_euclid(7);
    // `rem_euclid` is in `0..7`, so this is total rather than a cast that
    // happens to work.
    usize::try_from(dow).unwrap_or(0)
}

/// The whole calendar: a `Layer::Popup` surface under the field.
///
/// Built here rather than through [`super::popover::popover_with`], which
/// centres on its anchor and draws a caret. Carbon's calendar is neither:
/// `.cds--date-picker__calendar` is flush under the field's leading edge
/// and has no beak, because it is a panel belonging to the field rather
/// than a bubble pointing at it.
/// `anchor` names the sibling the calendar hangs under — `"field"` for the
/// single forms, `"fields"` for the range pair, whose leading edge is the
/// start field's.
fn calendar_surface(
    label: String,
    year: i32,
    month: u32,
    selected: &SelectedDays,
    calendar: Calendar,
    anchor: &str,
    today: Option<u32>,
) -> ViewNode {
    let mut children = vec![
        month_header(year, month, calendar),
        month_grid(year, month, selected, today),
    ];
    if calendar.choosing() {
        children.push(chooser_surface(year, month));
    }
    let mut content = stack("content", Axis::Vertical, None, children);
    content.props.align = Some(Align::Stretch);

    let mut node = ViewNode::new(NodeKind::Surface, "calendar")
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: Key::new(anchor),
                edge: Edge::Bottom,
                align: PropAlign::Start,
                offset: None,
            }),
            clamp: Some(ClampRule::Flip),
            input_policy: Some(InputPolicy::DismissOutside),
            align: Some(Align::Stretch),
            // Flush, and said out loud: `Tip::Caret` is the *default*, so a
            // surface that names no tip grows a beak. This one did, and the
            // beak poked up through the field's bottom rule and broke it in
            // two. Carbon's `.cds--date-picker__calendar` is a plain panel
            // under the input -- a beak belongs to a tooltip or a toggletip,
            // which point at something, not to a menu that hangs off an edge
            // it already touches.
            tip: Some(Tip::Flush),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // No edge. A surface floating over the page is lifted by its shadow —
    // Carbon's `.cds--date-picker__calendar` has `box-shadow` and no border,
    // and `containers_take_a_tone_and_controls_take_an_edge` is where that
    // rule is kept.
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    node.semantics.role = Some(Role::Overlay);
    node.semantics.label = Some(label);
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(CALENDAR_W),
            max: Some(CALENDAR_W),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(CALENDAR_H),
            max: Some(CALENDAR_H),
            priority: 0,
        },
    })
}

/// `< August 2026 >` — Carbon's `.flatpickr-months`: a prev arrow, the
/// current month, a next arrow, each 40 tall.
///
/// A `calendar` whose month is a button swaps the caption for a real
/// control. Carbon's own header is
/// the caption (`.cur-month` is a `<span>`), so the compact form is the
/// conformance target and the full form is a stated extension: Flatpickr
/// ships a `monthSelectorType: "dropdown"` variant and Carbon does not wire
/// it up, so there is no Carbon markup for the button or for the panel it
/// raises. What Carbon does give is the affordance — `.cur-month:hover`
/// takes `$layer-hover`, which is a hover tone on a span nothing can press.
fn month_header(year: i32, month: u32, calendar: Calendar) -> ViewNode {
    let caption_text = format!("{} {year}", MONTHS[(month - 1) as usize % 12]);
    let mut caption = text("month", caption_text.clone());
    caption.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let seat = if calendar.month_is_a_button() {
        month_button(caption, caption_text, calendar.choosing())
    } else {
        let mut seat = stack("month-seat", Axis::Horizontal, None, vec![caption]);
        seat.props.align = Some(Align::Center);
        seat.props.justify = Some(Justify::Center);
        seat
    };

    let mut row = ViewNode::new(NodeKind::Grid, "month-header")
        .with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: SIZE_MD },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Fixed { value: SIZE_MD },
            ],
            rows: vec![TrackSize::Fixed { value: SIZE_MD }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![
            step_control("prev-month", "Previous month", IconMark::ChevronLeft),
            seat,
            step_control("next-month", "Next month", IconMark::ChevronRight),
        ]);
    row.constraints = pin_block(SIZE_MD);
    row
}

/// One step control: a 40x40 icon button. Carbon's
/// `.flatpickr-prev-month`/`.flatpickr-next-month` — 40x40, `$icon-primary`,
/// `$layer-hover` on hover — and the same shape the chooser's year stepper
/// wears, because a year arrow and a month arrow are the same control
/// pointed at a different number.
fn step_control(key: &str, label: &str, mark: IconMark) -> ViewNode {
    let glyph = icon_toned("glyph", mark, IconTone::Primary);
    let mut node = stack(key, Axis::Horizontal, None, vec![glyph]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label.to_owned(), FIELD_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text()
}

/// The full form's month control: the same caption, plus a chevron saying
/// it raises something, in a button 40 tall.
///
/// `name` is the caption's own text, so the accessible name reads
/// "August 2026, choose month and year" rather than leaving the glyph to
/// carry the meaning (FR-026). The chevron turns over when the chooser is
/// up, which is Carbon's rule for every open state it draws
/// (`.cds--list-box__menu-icon--open { transform: rotate(180deg) }`).
fn month_button(caption: ViewNode, name: String, choosing: bool) -> ViewNode {
    let mark = if choosing {
        IconMark::ChevronUp
    } else {
        IconMark::ChevronDown
    };
    let glyph = icon_toned("glyph", mark, IconTone::Primary);
    let mut node = stack(
        "month-button",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![caption, glyph],
    );
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(
            Role::Button,
            format!("{name}, choose month and year"),
            FIELD_INTENTS,
        )
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text();
    node.semantics.expanded = Some(choosing);
    node
}

/// The month/year chooser: a year stepper over twelve month cells, filling
/// the calendar's body.
///
/// A `Layer::Popup` surface *inside* the calendar surface, anchored to the
/// header it hangs from. Three consequences, each of them the reason it is
/// built this way rather than as a sibling of the calendar:
///
/// * z. A surface nested in a surface takes its parent's z plus its own
///   layer base (`layout::Slot::above`), so the chooser paints over the day
///   grid without any component naming a number.
/// * Dismissal. `input::dismiss_requests` reports **every**
///   `DismissOutside` surface a press landed outside of. A chooser hanging
///   outside the calendar's own rect would make every press inside itself a
///   press outside the calendar, and picking a month would shut the
///   calendar it was picked in. Sized to the calendar's body, no press on
///   the chooser is ever outside the calendar.
/// * Anchoring. A component knows the keys it just wrote and not its own
///   mount path, so `Anchor::Sibling` is the only anchor it can spell
///   ([`Anchor::Sibling`]'s doc). Putting the chooser in the same child list
///   as `month-header` is what makes that spelling reach the header.
///
/// Carbon ships no markup for this panel; see [`month_header`].
fn chooser_surface(year: i32, month: u32) -> ViewNode {
    let mut content = stack(
        "chooser-content",
        Axis::Vertical,
        None,
        vec![year_header(year), month_cells(month)],
    );
    content.props.align = Some(Align::Stretch);

    let mut node = ViewNode::new(NodeKind::Surface, "chooser")
        .with_props(Props {
            layer: Some(Layer::Popup),
            anchor: Some(Anchor::Sibling {
                key: Key::new("month-header"),
                edge: Edge::Bottom,
                align: PropAlign::Start,
                offset: None,
            }),
            // `Shrink` and not the `Flip` the calendar takes. Flip's escape
            // hatch is the opposite edge, which for this surface is *above*
            // the header and therefore outside the calendar -- and a chooser
            // outside the calendar makes every press on itself a press
            // outside the calendar, which is the one thing the containment
            // below exists to prevent. Shrink keeps the edge and gives up
            // extent instead. In practice neither fires: the calendar is
            // already clamped onto the window and the chooser is sized to
            // its body, so there is nothing left to clamp.
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::DismissOutside),
            align: Some(Align::Stretch),
            tip: Some(Tip::Flush),
            ..Props::default()
        })
        .child(content);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    node.semantics.role = Some(Role::Overlay);
    node.semantics.label = Some("Choose month and year".to_owned());
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(CALENDAR_W),
            max: Some(CALENDAR_W),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(CHOOSER_H),
            max: Some(CHOOSER_H),
            priority: 0,
        },
    })
}

/// `< 2026 >` — the chooser's year stepper, built to the same three-track
/// plan as [`month_header`] so the two rows line up arrow over arrow.
fn year_header(year: i32) -> ViewNode {
    let mut caption = text("year", year.to_string());
    caption.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut seat = stack("year-seat", Axis::Horizontal, None, vec![caption]);
    seat.props.align = Some(Align::Center);
    seat.props.justify = Some(Justify::Center);

    let mut row = ViewNode::new(NodeKind::Grid, "year-header")
        .with_props(Props {
            columns: vec![
                TrackSize::Fixed { value: SIZE_MD },
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Fixed { value: SIZE_MD },
            ],
            rows: vec![TrackSize::Fixed { value: SIZE_MD }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(vec![
            step_control("prev-year", "Previous year", IconMark::ChevronLeft),
            seat,
            step_control("next-year", "Next year", IconMark::ChevronRight),
        ]);
    row.constraints = pin_block(SIZE_MD);
    row
}

/// Twelve month cells, three to a row, `selected` filled with the accent.
fn month_cells(selected: u32) -> ViewNode {
    let children: Vec<ViewNode> = (1..=12).map(|m| month_cell(m, m == selected)).collect();
    let mut grid = ViewNode::new(NodeKind::Grid, "months").with_props(Props {
        columns: vec![TrackSize::Weight { weight: 1.0 }; MONTH_COLS],
        rows: vec![
            TrackSize::Fixed {
                value: MONTH_CELL_H
            };
            MONTH_ROWS
        ],
        // Same reason `month_grid` names it: without it a cell keeps its
        // natural size and the hit box is the width of the word, not of the
        // column it sits in.
        align: Some(Align::Stretch),
        ..Props::default()
    });
    grid.constraints = Constraints {
        horizontal: AxisConstraint {
            min: Some(CALENDAR_W),
            max: Some(CALENDAR_W),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: None,
            max: None,
            priority: 1,
        },
    };
    grid.with_children(children)
}

/// One month of the chooser, keyed `mon-<n>` with `n` in `1..=12`.
///
/// The visible text is the three-letter abbreviation, because "September"
/// does not fit a 96-wide column at heading-sm and a label that overflows
/// is a defect this catalog has already shipped once. The accessible name
/// is the whole month, so the abbreviation is never the only channel
/// (FR-026).
fn month_cell(month: u32, selected: bool) -> ViewNode {
    let name = MONTHS[(month - 1) as usize % 12];
    let short = name.get(..3).unwrap_or(name);
    let mut caption = text("label", short.to_owned());
    caption.props.tokens.insert(
        "foreground".into(),
        t(if selected {
            TEXT_ON_ACCENT
        } else {
            TEXT_PRIMARY
        }),
    );
    let mut node = stack(
        format!("mon-{month}"),
        Axis::Horizontal,
        None,
        vec![caption],
    );
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            ACCENT_PRIMARY
        } else {
            SURFACE_RAISED
        }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_block(MONTH_CELL_H))
        .interactive(Role::Button, name.to_owned(), FIELD_INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE)
        .owning_its_text()
        // `Border`, for the day cell's second reason: a selected month fills
        // `ACCENT_PRIMARY`, and a stripe on that fill cannot be seen.
        .with_focus_figure(FocusFigure::Border);
    node.semantics.selected = selected;
    node
}

/// The weekday row plus six week rows of the real month.
fn month_grid(year: i32, month: u32, selected: &SelectedDays, today: Option<u32>) -> ViewNode {
    let lead = first_weekday(year, month);
    let count = days_in_month(year, month);
    let (prev_year, prev_month) = if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    };
    let prev_count = days_in_month(prev_year, prev_month);
    let mut children = Vec::with_capacity(7 + WEEK_ROWS * 7);
    for (i, name) in WEEKDAYS.iter().enumerate() {
        let mut caption = text("label", (*name).to_owned());
        caption
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_MUTED));
        // Centred by a seat, not by `align` on the text itself: the grid
        // stretches a cell to its track, and a bare text node then draws
        // from the track's leading edge. The initials sat 30px to the
        // inline-start of the column they name.
        children.push(centred(format!("dow-{i}"), caption));
    }
    for slot in 0..WEEK_ROWS * 7 {
        // `slot` counts cells from the first weekday column; `lead` is how
        // many of them belong to the month before this one.
        let day = (slot as i64) - (lead as i64) + 1;
        if day >= 1 && day <= i64::from(count) {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "bounded by the branch: 1 <= day <= 31"
            )]
            let day = day as u32;
            children.push(day_button(day, selected.fill(day), today == Some(day)));
        } else {
            children.push(adjacent_cell(slot, lead, count, prev_count));
        }
    }
    let mut grid = ViewNode::new(NodeKind::Grid, "days").with_props(Props {
        columns: vec![TrackSize::Fixed { value: SIZE_MD }; 7],
        rows: vec![TrackSize::Fixed { value: SIZE_MD }; WEEK_ROWS + 1],
        // Without this a cell keeps its natural size, so a day was a hit
        // box the width of its own digits sitting in a 40px track.
        align: Some(Align::Stretch),
        ..Props::default()
    });
    grid.constraints = Constraints {
        horizontal: AxisConstraint {
            min: Some(CALENDAR_W),
            max: Some(CALENDAR_W),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: None,
            max: None,
            priority: 1,
        },
    };
    grid.with_children(children)
}

/// A day of the month on show. `fill` says whether, and how, it carries the
/// selection: see [`date_picker_showing_selection`] for what
/// [`DayFill::Edge`] and [`DayFill::Interior`] each draw as, and why.
///
/// `today` (register row #170, slice-b's calendar-day `today` state) adds
/// Carbon's **"Today" indicator dot, 4×4px** (slice-b "Key numbers") under
/// the number — a second channel beside the digits themselves, never a
/// recoloured number alone.
fn day_button(day: u32, fill: DayFill, today: bool) -> ViewNode {
    let label = day.to_string();
    let mut caption = text("label", label.clone());
    caption.props.tokens.insert(
        "foreground".into(),
        t(match fill {
            // Both selected fills below are light-to-mid tones; only the
            // full accent needs the on-accent ink to clear AA.
            DayFill::Edge => TEXT_ON_ACCENT,
            DayFill::Interior | DayFill::None => TEXT_PRIMARY,
        }),
    );
    let parts = if today {
        vec![caption, today_dot(fill)]
    } else {
        vec![caption]
    };
    // The dot sits under the number, so today's cell stacks and the rest
    // stay one row; both centre the same way.
    let axis = if today {
        Axis::Vertical
    } else {
        Axis::Horizontal
    };
    let mut node = stack(format!("day-{day}"), axis, None, parts);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    // Resting background: the calendar's own content fill, the ground every
    // day cell sits on. Without this, `background@hover` has no resting
    // `background` beneath it and resolves to nothing at rest — the
    // Accordion/Modal/AI-label defect, generalised (see
    // `a_state_decorated_token_always_has_a_resting_binding`).
    node.props.tokens.insert(
        "background".into(),
        t(match fill {
            DayFill::Edge => ACCENT_PRIMARY,
            DayFill::Interior => SURFACE_LAYER_TWO,
            DayFill::None => SURFACE_RAISED,
        }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label, FIELD_INTENTS)
        .with_behaviour(SELECTS_ON_RELEASE)
        .owning_its_text()
        // `Border`, and the day grid rules out both alternatives.
        //
        // A bar *under* a day is out: the grid is 40-tall cells on a 40
        // pitch — week rows at y 452, 492, 532, 572, 612 — so there is no
        // gap at all and the bar paints inside the day below.
        //
        // `BarInside` was the answer to that and is out for a second reason,
        // found on 2026-09-06 while tracing the operator's *"it disappears
        // when in the colored one"* on the modal's primary button. The
        // **selected** day fills `ACCENT_PRIMARY`, `focus.ring` is
        // byte-identical to it, and a stripe seated on that cell's own
        // bottom edge is therefore invisible. Measured on the focused,
        // selected day-30 cell: `(15, 98, 254)` from the fill straight
        // through the stripe's rows, the only trace being two rows the
        // stripe's *shadow* darkened to `(15, 97, 251)`.
        //
        // `Border` is contained, so the grid's zero pitch is fine, and it
        // carries the ground-coloured halo (`focus::HALO_TOKEN`) that exists
        // for this exact collision. It is also Carbon's own figure for a day
        // cell. `no_bar_is_painted_on_a_fill_it_cannot_be_seen_against`
        // holds the rule.
        //
        // The calendar's nav strip sits 40 units clear of the first week,
        // never fills with the accent, and keeps the default bar.
        .with_focus_figure(FocusFigure::Border);
    node.semantics.selected = fill.is_selected();
    node
}

/// Carbon's "Today" indicator: a 4×4 dot under the number (slice-b "Key
/// numbers", MEASURED). The ink follows the number's own rule — on-accent
/// over the selected fill, primary elsewhere — because a dot that vanished
/// on a selected today would leave the state with one channel instead of
/// two.
fn today_dot(fill: DayFill) -> ViewNode {
    swatch(
        "today-dot",
        4.0,
        4.0,
        Some(match fill {
            DayFill::Edge => TEXT_ON_ACCENT,
            DayFill::Interior | DayFill::None => TEXT_PRIMARY,
        }),
        None,
        // `Pill` at 4 resolves to the half-edge: the dot is a circle, not
        // a rounded square.
        Some(corner_for(CornerRole::Pill, 4.0)),
    )
}

/// A cell belonging to the month either side of the one on show.
///
/// Carries its real number, in [`TEXT_MUTED`], because Carbon draws them
/// and a six-row grid with holes in it reads as a broken grid rather than
/// as a month. Not interactive: Carbon lets a press here step the month,
/// and this component has no month state to step. That is a stated gap.
///
/// `slot` counts cells from the first weekday column, `lead` is how many of
/// them belong to the month before, and `count` is this month's length —
/// the same three numbers [`month_grid`] placed the day by.
fn adjacent_cell(slot: usize, lead: usize, count: u32, prev_count: u32) -> ViewNode {
    let number = if slot < lead {
        // Trailing days of the month before: the last `lead` of them.
        prev_count - (lead - slot - 1) as u32
    } else {
        // Leading days of the month after.
        (slot - lead) as u32 - count + 1
    };
    let mut caption = text("label", number.to_string());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut cell = centred(format!("adj-{slot}"), caption);
    cell.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    cell.with_constraints(pin_block(SIZE_MD))
}

/// `child` in a seat that centres it on both axes inside its grid track.
fn centred(key: String, child: ViewNode) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![child]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node
}

#[cfg(test)]
mod tests {
    use super::{
        CALENDAR_H, CALENDAR_W, CHOOSER_H, Calendar, MONTH_CELL_H, MONTH_COLS, MONTH_ROWS,
        PropAlign, SIZE_FLUID, SIZE_LG, SIZE_MD, SIZE_SM, WEEK_ROWS, WEEKDAYS, date_picker,
        date_picker_fluid, date_picker_input, date_picker_invalid, date_picker_lg,
        date_picker_open, date_picker_range, date_picker_range_showing, date_picker_showing,
        date_picker_showing_selection, date_picker_showing_today, date_picker_sm,
        date_picker_warning, days_in_month, first_weekday, parse_date, time_picker,
    };
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{
        ACCENT_PRIMARY, BORDER_STRONG, SURFACE_LAYER_TWO, SURFACE_RAISED,
    };

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{
        Anchor, Behaviour, ClampRule, Edge, FocusFigure, FocusShownOn, Intent, Interaction,
        NodeKind, Phase, Role, Tip, TrackSize, ViewNode,
    };

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::tokens::{
        SUPPORT_ERROR, SUPPORT_WARNING, TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY,
    };
    use crate::token::{CornerRole, corner_for};

    /// Spec 008 register rows #157/#159: the dated well carries the field
    /// ramp's three heights (sm 32 / md 40 / lg 48 — slice-b "Sizes", the
    /// same table as every field-shaped control). The probe is the well
    /// (`{key}/field`), not the column around it, and the calendar is not
    /// probed because it is fixed at every step (slice-b: "the calendar
    /// menu itself is a fixed height and width regardless of input size").
    #[test]
    fn the_well_builds_at_three_steps_on_the_field_ramp() {
        for (node, step) in [
            (date_picker_sm("due", "Due date", "2026-08-30"), SIZE_SM),
            (date_picker("due", "Due date", "2026-08-30"), SIZE_MD),
            (date_picker_lg("due", "Due date", "2026-08-30"), SIZE_LG),
        ] {
            let well = child(&node, "field");
            assert_eq!(
                (well.constraints.vertical.min, well.constraints.vertical.max),
                (Some(step), Some(step)),
                "the well is not pinned to {step}"
            );
            let frame = petrify_lone(node);
            let placed = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/field"))
                .expect("the well is not placed");
            assert!(
                (placed.rect.h - step).abs() < 1.0,
                "the well is {} tall at step {step}",
                placed.rect.h
            );
        }
    }

    /// The invalid state dresses the field in the shared form-family
    /// chrome and says so in words and a glyph (spec 008 T039).
    #[test]
    fn date_picker_invalid_pairs_an_error_outline_with_helper_text() {
        let node = date_picker_invalid("due", "Due date", "2026-08-30", "required");
        let field = child(&node, "field");
        assert_eq!(token(field, "border"), Some(SUPPORT_ERROR));
        assert_ne!(
            token(field, "border"),
            Some(ACCENT_PRIMARY),
            "an invalid field must not wear the accent: a focused field wears \
             it too"
        );
        assert_eq!(
            token(field, "border-bottom"),
            None,
            "the outline replaces the resting rule rather than stacking on it"
        );
        let helper = child(&node, "helper");
        let message = child(helper, "message");
        assert_eq!(message.props.text.as_deref(), Some("Invalid: required"));
        assert_eq!(token(message, "foreground"), Some(TEXT_PRIMARY));
        let expected = icon_toned("mark", IconMark::ErrorFilled, IconTone::Primary);
        assert_eq!(
            child(helper, "mark").props.canvas,
            expected.props.canvas,
            "the helper's glyph is the paired error mark (spec 008 T039)"
        );
    }

    /// The warning state: the warning hue, and the word and the
    /// triangle-with-bang beside it — the channels a greyscale reader has.
    #[test]
    fn date_picker_warning_draws_the_warning_hue_and_says_so_in_words_and_a_glyph() {
        let node = date_picker_warning("due", "Due date", "2026-08-30", "check the value");
        let field = child(&node, "field");
        assert_eq!(token(field, "border"), Some(SUPPORT_WARNING));
        assert_ne!(
            token(field, "border"),
            Some(SUPPORT_ERROR),
            "a warning field must not wear the error outline"
        );
        assert_ne!(
            token(field, "border"),
            Some(ACCENT_PRIMARY),
            "a warning field must not wear the accent: a focused field wears it too"
        );
        assert_eq!(
            token(field, "border-bottom"),
            None,
            "the outline replaces the resting rule rather than stacking on it"
        );
        let helper = child(&node, "helper");
        let message = child(helper, "message");
        assert_eq!(
            message.props.text.as_deref(),
            Some("Warning: check the value")
        );
        assert_eq!(token(message, "foreground"), Some(TEXT_PRIMARY));
        let expected = icon_toned("mark", IconMark::WarningAltFilled, IconTone::Primary);
        assert_eq!(
            child(helper, "mark").props.canvas,
            expected.props.canvas,
            "the helper's glyph is the triangle-with-bang, whose silhouette is \
             what a greyscale reader distinguishes from error's circle-with-slash"
        );
    }

    /// The closed form is a column whose `field` is the button, the same
    /// shape as the open form less the calendar.
    #[test]
    fn date_picker_is_a_column_over_a_closed_field_at_height_40() {
        let node = date_picker("due", "Due date", "2026-08-30");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.semantics.role, None, "the column is not the control");
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1, "closed is the field alone");
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(
            field.semantics.focus_figure,
            FocusFigure::Sides,
            "a well a person picks into: focus brackets its sides"
        );
        assert_eq!(
            field.semantics.focus_shown_on,
            FocusShownOn::Well,
            "and the field is the hull, so it is shown on the field itself"
        );
        assert_eq!(field.semantics.label.as_deref(), Some("Due date"));
        assert_eq!(field.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(field.constraints.vertical.max, Some(SIZE_MD));
        assert!(field.interactions.contains(&Interaction::Click));
        assert_eq!(token(field, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(field, "border"),
            None,
            "Carbon's date input is `.cds--text-input`, and that is a fill \
             with one rule under it, not a box"
        );
        assert_eq!(
            token(field, "radius"),
            None,
            "and a flat one: `.cds--text-input` has no `border-radius`, and \
             the rounded box is the other half of what the operator saw"
        );
        assert_eq!(token(field, "border-bottom"), Some(BORDER_STRONG));
        assert_eq!(
            child(field, "value").props.text.as_deref(),
            Some("2026-08-30")
        );
        let mark = child(field, "calendar-mark");
        assert_eq!(mark.kind, NodeKind::Canvas);
        assert_eq!(mark.props.text, None, "the calendar is a glyph, not a word");
        assert_eq!(
            mark.semantics.label.as_deref(),
            Some("calendar"),
            "the word survives as the mark's accessible name"
        );
        assert!(mark.semantics.role.is_none());
        assert_eq!(field.semantics.expanded, Some(false));
        assert_ne!(field.semantics.role, Some(Role::Overlay));
    }

    /// The field's id is the same in both forms, so keyboard focus seated
    /// on it survives the calendar opening. Falsify by keying the closed
    /// column's field `"input"`: `/due/field` then exists only while open,
    /// and the host's vanished-focus rule sends focus elsewhere.
    #[test]
    fn the_field_keeps_its_key_across_open_and_closed() {
        let closed = date_picker("due", "Due date", "2026-08-30");
        let open = date_picker_open("due", "Due date", "2026-08-30");
        assert_eq!(closed.key, open.key);
        assert_eq!(child(&closed, "field").key, child(&open, "field").key);
        for (label, node) in [("closed", &closed), ("open", &open)] {
            let frame = petrify_lone(node.clone());
            assert!(
                frame.placements.iter().any(|p| p.id == "/root/due/field"),
                "{label}: the field is placed at the same id"
            );
        }
    }

    #[test]
    fn the_open_calendar_shows_the_real_month_its_value_names() {
        // August 2026 starts on a Saturday and has 31 days, so the grid
        // leads with six adjacent cells and the 1st lands in the last
        // column. That is the arrangement being asserted, not "some grid".
        let node = date_picker_open("due", "Due date", "2026-08-30");
        assert_eq!(node.semantics.expanded, Some(true));
        let field = child(&node, "field");
        assert_eq!(field.semantics.role, Some(Role::Button));
        assert_eq!(field.semantics.expanded, Some(true));

        let calendar = child(&node, "calendar");
        assert_eq!(calendar.kind, NodeKind::Surface);
        assert_eq!(calendar.semantics.role, Some(Role::Overlay));
        // `Tip::Caret` is the default, so a surface that names no tip grows a
        // beak. This one did, and it poked up through the field's bottom rule
        // and broke it in two. A beak points at something; a menu hanging off
        // an edge it already touches has nothing to point at.
        assert_eq!(
            calendar.props.tip,
            Some(Tip::Flush),
            "Carbon's date picker calendar is a plain panel under the input"
        );
        match &calendar.props.anchor {
            Some(Anchor::Sibling { key, align, .. }) => {
                assert_eq!(key.as_str(), "field");
                assert_eq!(
                    *align,
                    PropAlign::Start,
                    "Carbon's calendar is flush under the field's leading \
                     edge. Centred, it hung 92px off the side of it"
                );
            }
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(calendar.constraints.horizontal.max, Some(CALENDAR_W));
        assert_eq!(calendar.constraints.vertical.max, Some(CALENDAR_H));
        assert_eq!(CALENDAR_W, 288.0);
        assert_eq!(CALENDAR_H, 336.0);

        let content = child(calendar, "content");
        assert!(
            content.children.iter().all(|c| c.key.as_str() != "caret"),
            "Carbon's calendar is a panel belonging to the field, not a \
             bubble pointing at it, and has no beak. The one that was here \
             was the text glyph `^`, sitting where the month goes"
        );
        let header = child(content, "month-header");
        assert_eq!(
            child(child(header, "month-seat"), "month")
                .props
                .text
                .as_deref(),
            Some("August 2026")
        );
        for step in ["prev-month", "next-month"] {
            assert_eq!(child(header, step).semantics.role, Some(Role::Button));
        }

        let days = child(content, "days");
        assert_eq!(days.kind, NodeKind::Grid);
        assert_eq!(days.props.columns.len(), 7);
        assert_eq!(days.props.rows.len(), WEEK_ROWS + 1);
        assert_eq!(
            days.props.align,
            Some(crate::geom::Align::Stretch),
            "a Grid leaves a cell at its natural size otherwise, and every \
             day was a hit box the width of its own digits in a 40px track"
        );

        for (i, name) in WEEKDAYS.iter().enumerate() {
            let cell = child(days, &format!("dow-{i}"));
            assert_eq!(child(cell, "label").props.text.as_deref(), Some(*name));
            assert!(cell.semantics.role.is_none());
            assert!(cell.interactions.is_empty());
        }

        // The whole month, and nothing past it.
        for day in 1..=31_u32 {
            let cell = child(days, &format!("day-{day}"));
            assert_eq!(cell.semantics.role, Some(Role::Button));
            assert!(cell.interactions.contains(&Interaction::Click));
        }
        assert!(
            days.children.iter().all(|c| c.key.as_str() != "day-32"),
            "August has 31 days"
        );

        // The value's own day is the selected one, in the accent.
        let thirtieth = child(days, "day-30");
        assert!(thirtieth.semantics.selected);
        assert_eq!(token(thirtieth, "background"), Some(ACCENT_PRIMARY));
        let other = child(days, "day-12");
        assert!(!other.semantics.selected);
        assert_eq!(
            token(other, "background"),
            Some(SURFACE_RAISED),
            "an unselected day needs a resting `background` under its \
             `background@hover`, or the hover binding resolves to nothing \
             and paints silent"
        );

        // Six adjacent-month cells lead, because 2026-08-01 is a Saturday,
        // and they carry July's last six days rather than being holes.
        for (slot, day) in (0..6).zip(26..=31) {
            assert_eq!(
                child(child(days, &format!("adj-{slot}")), "label")
                    .props
                    .text
                    .as_deref(),
                Some(day.to_string().as_str()),
                "July 2026 has 31 days, so the lead-in runs 26..31"
            );
        }
        // And the trailing cells are September's first days. Six rows of
        // seven is 42 cells; 6 lead plus 31 of August leaves 5.
        assert_eq!(
            child(child(days, "adj-37"), "label").props.text.as_deref(),
            Some("1"),
            "the cell after the 31st is the 1st of September"
        );
        assert!(
            days.children.iter().all(|c| c.key.as_str() != "adj-6"),
            "slot 6 is the 1st of August"
        );
    }

    /// The compact form's month is a caption in a seat; the full form's is
    /// a real control that says what it opens and whether it is open.
    ///
    /// The operator: *"in the full view the month is a button, not just a
    /// label."* A `Text` node someone made clickable would pass a
    /// screenshot and fail a keyboard, so what is asserted is the role, the
    /// interactions and the accessible name, not the paint.
    #[test]
    fn only_the_full_form_makes_the_month_a_control() {
        let view = Calendar::Compact {
            year: 2026,
            month: 8,
        };
        let compact = date_picker_showing("due", "Due date", "2026-08-30", view);
        let header = child(
            child(child(&compact, "calendar"), "content"),
            "month-header",
        );
        let seat = child(header, "month-seat");
        assert_eq!(seat.semantics.role, None, "Carbon's `.cur-month` is a span");
        assert!(seat.interactions.is_empty());
        assert!(
            header
                .children
                .iter()
                .all(|c| c.key.as_str() != "month-button"),
            "the compact form grew a month button"
        );

        for (calendar, open) in [
            (
                Calendar::Full {
                    year: 2026,
                    month: 8,
                },
                false,
            ),
            (
                Calendar::Choosing {
                    year: 2026,
                    month: 8,
                },
                true,
            ),
        ] {
            let node = date_picker_showing("due", "Due date", "2026-08-30", calendar);
            let header = child(child(child(&node, "calendar"), "content"), "month-header");
            let button = child(header, "month-button");
            assert_eq!(button.semantics.role, Some(Role::Button));
            assert!(button.interactions.contains(&Interaction::Click));
            assert!(button.interactions.contains(&Interaction::Focus));
            assert_eq!(
                button.semantics.label.as_deref(),
                Some("August 2026, choose month and year"),
                "the chevron must not be the only channel"
            );
            assert_eq!(button.semantics.expanded, Some(open));
            assert_eq!(
                child(button, "month").props.text.as_deref(),
                Some("August 2026")
            );
            assert!(
                header
                    .children
                    .iter()
                    .all(|c| c.key.as_str() != "month-seat"),
                "the button replaces the seat rather than joining it"
            );
        }
    }

    /// The chooser: twelve named month cells over a year stepper, sized to
    /// the calendar's body and anchored to the header it hangs from.
    #[test]
    fn the_chooser_offers_every_month_and_a_year_stepper() {
        let node = date_picker_showing(
            "due",
            "Due date",
            "2026-08-30",
            Calendar::Choosing {
                year: 2026,
                month: 8,
            },
        );
        let content = child(child(&node, "calendar"), "content");
        let chooser = child(content, "chooser");
        assert_eq!(chooser.kind, NodeKind::Surface);
        assert_eq!(chooser.semantics.role, Some(Role::Overlay));
        assert_eq!(chooser.props.tip, Some(Tip::Flush));
        assert_eq!(
            chooser.props.clamp,
            Some(ClampRule::Shrink),
            "Flip's opposite edge is above the header, which is outside the \
             calendar, and a chooser outside the calendar dismisses it"
        );
        match &chooser.props.anchor {
            Some(Anchor::Sibling { key, align, .. }) => {
                assert_eq!(key.as_str(), "month-header");
                assert_eq!(*align, PropAlign::Start);
            }
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        assert_eq!(chooser.constraints.horizontal.max, Some(CALENDAR_W));
        assert_eq!(chooser.constraints.vertical.max, Some(CHOOSER_H));
        assert_eq!(CHOOSER_H, CALENDAR_H - SIZE_MD);

        let body = child(chooser, "chooser-content");
        let years = child(body, "year-header");
        assert_eq!(
            child(child(years, "year-seat"), "year")
                .props
                .text
                .as_deref(),
            Some("2026")
        );
        for step in ["prev-year", "next-year"] {
            let control = child(years, step);
            assert_eq!(control.semantics.role, Some(Role::Button));
            assert!(control.interactions.contains(&Interaction::Click));
        }

        let months = child(body, "months");
        assert_eq!(months.props.columns.len(), MONTH_COLS);
        assert_eq!(months.props.rows.len(), MONTH_ROWS);
        assert_eq!(
            months.props.align,
            Some(crate::geom::Align::Stretch),
            "an unstretched cell is a hit box the width of its own word"
        );
        assert_eq!(months.children.len(), 12);
        let names = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        for (i, name) in names.iter().enumerate() {
            let cell = child(months, &format!("mon-{}", i + 1));
            assert_eq!(cell.semantics.role, Some(Role::Button));
            assert!(cell.interactions.contains(&Interaction::Click));
            assert_eq!(
                cell.semantics.label.as_deref(),
                Some(*name),
                "the abbreviation must not be the only channel"
            );
            assert_eq!(
                child(cell, "label").props.text.as_deref(),
                Some(&name[..3]),
                "the visible text is the abbreviation that fits the column"
            );
            assert_eq!(cell.constraints.vertical.max, Some(MONTH_CELL_H));
        }
        let august = child(months, "mon-8");
        assert!(august.semantics.selected);
        assert_eq!(token(august, "background"), Some(ACCENT_PRIMARY));
        let other = child(months, "mon-3");
        assert!(!other.semantics.selected);
        assert_eq!(
            token(other, "background"),
            Some(SURFACE_RAISED),
            "an unselected cell needs a resting fill under its hover binding"
        );
    }

    /// Browsing to another month leaves the selected date alone and marks
    /// no day in the month on show.
    ///
    /// The alternative — carrying the accent onto whatever day shares the
    /// number — would have made "the 30th" look selected in every month
    /// that has one.
    #[test]
    fn browsing_off_the_selected_month_marks_no_day() {
        let node = date_picker_showing(
            "due",
            "Due date",
            "2026-08-30",
            Calendar::Compact {
                year: 2026,
                month: 9,
            },
        );
        assert_eq!(
            child(child(&node, "field"), "value").props.text.as_deref(),
            Some("2026-08-30"),
            "browsing moved the field's own value"
        );
        let content = child(child(&node, "calendar"), "content");
        assert_eq!(
            child(child(child(content, "month-header"), "month-seat"), "month")
                .props
                .text
                .as_deref(),
            Some("September 2026")
        );
        let days = child(content, "days");
        assert!(
            days.children.iter().all(|c| !c.semantics.selected),
            "the 30th of August was marked in September"
        );
        // And back on the month it belongs to, it is marked again.
        let home = date_picker_showing(
            "due",
            "Due date",
            "2026-08-30",
            Calendar::Compact {
                year: 2026,
                month: 8,
            },
        );
        let days = child(child(child(&home, "calendar"), "content"), "days");
        assert!(child(days, "day-30").semantics.selected);
    }

    /// [`date_picker_showing_selection`] marks every day in the selection,
    /// not only one. A range (the 3rd through the 6th) reads as an accented
    /// cap at each end with a lighter interior between them; a scattered
    /// pick (three days with gaps) reads as three independently accented
    /// squares, because each is a run of one and every run's own ends are
    /// edges.
    #[test]
    fn a_selection_marks_every_day_edges_full_accent_interior_lighter() {
        let view = Calendar::Compact {
            year: 2026,
            month: 8,
        };
        let range = date_picker_showing_selection(
            "due",
            "Due date",
            "2026-08-03",
            ["2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06"],
            view,
        );
        let days = child(child(child(&range, "calendar"), "content"), "days");
        for day in [3, 6] {
            let cell = child(days, &format!("day-{day}"));
            assert!(cell.semantics.selected);
            assert_eq!(
                token(cell, "background"),
                Some(ACCENT_PRIMARY),
                "day {day} is a range edge"
            );
        }
        for day in [4, 5] {
            let cell = child(days, &format!("day-{day}"));
            assert!(cell.semantics.selected);
            assert_eq!(
                token(cell, "background"),
                Some(SURFACE_LAYER_TWO),
                "day {day} is a range interior"
            );
        }
        for day in [2, 7] {
            assert!(!child(days, &format!("day-{day}")).semantics.selected);
        }

        let multi = date_picker_showing_selection(
            "due",
            "Due date",
            "2026-08-03",
            ["2026-08-03", "2026-08-10", "2026-08-20"],
            view,
        );
        let days = child(child(child(&multi, "calendar"), "content"), "days");
        for day in [3, 10, 20] {
            let cell = child(days, &format!("day-{day}"));
            assert!(cell.semantics.selected);
            assert_eq!(
                token(cell, "background"),
                Some(ACCENT_PRIMARY),
                "day {day} is picked alone, a run of one, always an edge"
            );
        }

        // Cross-month awareness: the edge/interior read depends on the
        // whole selection, not only the days visible in the browsed month.
        let crossing = date_picker_showing_selection(
            "due",
            "Due date",
            "2026-07-31",
            ["2026-07-30", "2026-07-31", "2026-08-01", "2026-08-02"],
            view,
        );
        let days = child(child(child(&crossing, "calendar"), "content"), "days");
        assert_eq!(
            token(child(days, "day-1"), "background"),
            Some(SURFACE_LAYER_TWO),
            "August 1st is held on both sides by the selection, even though \
             its July neighbour is off screen"
        );
        assert_eq!(
            token(child(days, "day-2"), "background"),
            Some(ACCENT_PRIMARY),
            "August 2nd is the range's far end"
        );
    }

    /// Register row #170 (`today`): Carbon's "Today" indicator — a 4×4 dot
    /// under the number of the day `today` names (slice-b "Key numbers"),
    /// and on no other day. The dot's ink follows the number's own rule, so
    /// the state survives the selected fill as well as the resting one.
    #[test]
    fn the_today_day_carries_the_four_unit_dot_and_no_other_day_does() {
        let view = Calendar::Compact {
            year: 2026,
            month: 8,
        };
        let node = date_picker_showing_today("due", "Due date", "2026-08-20", view, "2026-08-24");
        let days = child(child(child(&node, "calendar"), "content"), "days");
        let dot = child(child(days, "day-24"), "today-dot");
        assert_eq!(dot.constraints.horizontal.min, Some(4.0));
        assert_eq!(dot.constraints.horizontal.max, Some(4.0));
        assert_eq!(dot.constraints.vertical.min, Some(4.0));
        assert_eq!(
            dot.constraints.vertical.max,
            Some(4.0),
            "4×4, slice-b's figure"
        );
        assert_eq!(token(dot, "background"), Some(TEXT_PRIMARY));
        assert_eq!(
            token(dot, "radius"),
            Some(corner_for(CornerRole::Pill, 4.0)),
            "a dot, not a rounded square"
        );
        for other in ["day-23", "day-25"] {
            assert!(
                child(days, other)
                    .children
                    .iter()
                    .all(|c| c.key.as_str() != "today-dot"),
                "{other} wore the today dot"
            );
        }

        // On a selected today the dot swaps to the on-accent ink rather
        // than disappearing into the fill.
        let selected =
            date_picker_showing_today("due", "Due date", "2026-08-24", view, "2026-08-24");
        let days = child(child(child(&selected, "calendar"), "content"), "days");
        let dot = child(child(days, "day-24"), "today-dot");
        assert_eq!(token(dot, "background"), Some(TEXT_ON_ACCENT));

        // A today outside the browsed month is not on screen, so it marks
        // nothing — the same rule the selected day keeps.
        let away = date_picker_showing_today("due", "Due date", "2026-08-20", view, "2026-09-24");
        let days = child(child(child(&away, "calendar"), "content"), "days");
        assert!(
            days.children
                .iter()
                .all(|row| row.children.iter().all(|c| c.key.as_str() != "today-dot")),
            "a September today dotted an August grid"
        );
    }

    /// The chooser is placed **inside** the calendar and above its day
    /// grid, which is what makes it safe to declare `DismissOutside`.
    ///
    /// `input::dismiss_requests` reports every `DismissOutside` surface a
    /// press landed outside of. A chooser hanging outside the calendar's own
    /// rect would make every press inside itself a press outside the
    /// calendar, so picking a month would shut the calendar it was picked
    /// in. This is that containment, measured on a real frame rather than
    /// argued from the constraints.
    #[test]
    fn the_chooser_lands_inside_the_calendar_and_over_the_day_grid() {
        let frame = petrify_lone(date_picker_showing(
            "due",
            "Due date",
            "2026-08-30",
            Calendar::Choosing {
                year: 2026,
                month: 8,
            },
        ));
        let at = |id: &str| {
            frame
                .placement(id)
                .unwrap_or_else(|| panic!("{id} was not placed"))
        };
        let calendar = at("/root/due/calendar");
        let chooser = at("/root/due/calendar/content/chooser");
        let days = at("/root/due/calendar/content/days");
        assert!(
            chooser.rect.x >= calendar.rect.x - 0.5
                && chooser.rect.y >= calendar.rect.y - 0.5
                && chooser.rect.x + chooser.rect.w <= calendar.rect.x + calendar.rect.w + 0.5
                && chooser.rect.y + chooser.rect.h <= calendar.rect.y + calendar.rect.h + 0.5,
            "the chooser at {:?} escapes the calendar at {:?}",
            chooser.rect,
            calendar.rect
        );
        assert!(
            chooser.z > days.z,
            "the chooser at z {} is under the day grid at z {}",
            chooser.z,
            days.z
        );
        assert_eq!(chooser.rect.w, CALENDAR_W);
        assert_eq!(chooser.rect.h, CHOOSER_H);
        assert!(
            (chooser.rect.h + SIZE_MD - calendar.rect.h).abs() < 0.5,
            "the chooser leaves a band of the day grid showing under it:              chooser {:?}, calendar {:?}",
            chooser.rect,
            calendar.rect
        );
    }

    /// The full form's nested surface is accepted at the depth the catalog
    /// mounts it, which is where the anchor-cycle scan has the most to say:
    /// the chooser depends on the calendar, and the calendar on the field.
    #[test]
    fn the_full_form_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "date_picker_showing",
            vec![date_picker_showing(
                "due",
                "Due date",
                "2026-08-30",
                Calendar::Choosing {
                    year: 2026,
                    month: 8,
                },
            )],
        );
    }

    /// The two calendar facts, against dates whose weekday is known.
    #[test]
    fn the_calendar_arithmetic_agrees_with_dates_anyone_can_check() {
        // Sundays: 2026-08-02, 2000-01-02, 1970-01-04. `first_weekday`
        // answers for the 1st, so these are the offsets that follow.
        assert_eq!(first_weekday(2026, 8), 6, "2026-08-01 is a Saturday");
        assert_eq!(first_weekday(2000, 1), 6, "2000-01-01 is a Saturday");
        assert_eq!(first_weekday(1970, 1), 4, "1970-01-01 is a Thursday");
        assert_eq!(first_weekday(2024, 2), 4, "2024-02-01 is a Thursday");

        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29, "2024 is a leap year");
        assert_eq!(days_in_month(1900, 2), 28, "1900 is not: divisible by 100");
        assert_eq!(days_in_month(2000, 2), 29, "2000 is: divisible by 400");
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn a_value_that_is_not_a_date_is_refused_rather_than_guessed_at() {
        assert_eq!(parse_date("2026-08-30"), Some((2026, 8, 30)));
        assert_eq!(parse_date("2026-8-3"), Some((2026, 8, 3)));
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("2026-08"), None);
        assert_eq!(parse_date("2026-13-01"), None, "no thirteenth month");
        assert_eq!(parse_date("2026-00-01"), None);
        assert_eq!(parse_date("2026-08-32"), None);
        assert_eq!(parse_date("2026-08-30-01"), None);
        assert_eq!(parse_date("today"), None);
    }

    /// `date_picker_open`'s `calendar` names its `field` sibling by bare
    /// key (`Anchor::Sibling`), so the open form is accepted wherever a
    /// caller mounts it — here two containers below the root, the gallery
    /// catalog's own depth.
    #[test]
    fn date_picker_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "date_picker_open",
            vec![date_picker_open("due", "Due date", "2026-08-30")],
        );
    }

    // The frame-level checks below audit the CLOSED field.

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: the closed field places with a real rect, none of its
    /// parts outside it.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = date_picker("due", "Due date", "2026-08-30");
        let frame = petrify_lone(node);
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    /// Check F: the closed field declares `Focus` and is reachable; the
    /// column above it is never a stop.
    #[test]
    fn the_closed_field_is_reachable_in_focus_order() {
        let node = date_picker("due", "Due date", "2026-08-30");
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        assert!(
            focus.order().iter().any(|o| o == "/root/due/field"),
            "the closed field declares Focus but is not in focus order"
        );
        assert!(
            !focus.order().iter().any(|o| o == "/root/due"),
            "the column is not a stop"
        );
    }

    /// Check E: the value text (AA, 4.5:1) and the calendar glyph (WCAG
    /// non-text, 3:1) against the field's own resting fill, in both themes.
    #[test]
    fn closed_field_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        const MIN_GLYPH_CONTRAST: f32 = 3.0;
        for theme in [crate::token::light(), crate::token::dark()] {
            let column = date_picker("due", "Due date", "2026-08-30");
            let node = child(&column, "field");
            let bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the field binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let value = child(node, "value");
            let fg_name = value
                .props
                .tokens
                .get("foreground")
                .expect("value text binds a foreground");
            let opacity = value.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "value at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                bg_name.as_str()
            );

            let mark = child(node, "calendar-mark");
            let list = mark.props.canvas.as_ref().expect("the calendar is drawn");
            let mut fills = 0;
            for command in list.commands() {
                let crate::draw::Command::Rect { paint, .. } = command else {
                    continue;
                };
                let Some(crate::draw::ColorRef::Token(name)) = paint.fill.as_ref() else {
                    panic!("the calendar's bars bind a token fill");
                };
                let ratio = color(&theme, name).over(bg).contrast_ratio(bg);
                assert!(
                    ratio >= MIN_GLYPH_CONTRAST,
                    "calendar-mark ({name}) at {ratio:.2}:1 against {} fails \
                     non-text {MIN_GLYPH_CONTRAST}:1",
                    bg_name.as_str()
                );
                fills += 1;
            }
            assert!(fills > 0, "the calendar draws at least one bar");
        }
    }

    /// Spec 010: the field trigger toggles the calendar on release.
    ///
    /// Falsified by dropping `.with_behaviour(...)` from the closed field:
    ///
    /// ```text
    /// date_picker field declared behaviour None
    /// ```
    #[test]
    fn date_picker_field_declares_toggle_on_release() {
        let node = date_picker("when", "Date", "2026-09-25");
        let field = child(&node, "field");
        assert_eq!(
            field.behaviour,
            Some(Behaviour {
                intent: Intent::Toggle,
                phase: Phase::OnRelease,
            }),
            "date_picker field declared behaviour {:?}",
            field.behaviour
        );
    }

    /// The simple date input is the filled text field and nothing else: no
    /// calendar surface, and no calendar mark, because the slice documents
    /// the mark as "present only on calendar-backed variants". Its value is
    /// content ink ([`TEXT_PRIMARY`]), not placeholder ink.
    #[test]
    fn the_simple_date_input_is_a_filled_field_and_carries_no_calendar() {
        let node = date_picker_input("born", "Date of birth", "1989-04-12");
        let label = child(&node, "label");
        assert_eq!(token(label, "foreground"), Some(TEXT_MUTED));
        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.props.text.as_deref(), Some("1989-04-12"));
        assert_eq!(token(input, "foreground"), Some(TEXT_PRIMARY));
        assert!(
            input.children.is_empty(),
            "the simple input is a bare text field — no mark hangs off it"
        );
        assert!(
            node.children.iter().all(|c| c.key.as_str() != "calendar"),
            "the simple input raises no calendar"
        );
    }

    /// Carbon Fluid's date field: 64 tall, the muted label stacked inside
    /// the well at the 4-unit gap over the value and the mark — and still
    /// the same toggle the default dressing wears.
    #[test]
    fn the_fluid_field_stacks_its_label_inside_the_64_unit_well() {
        let node = date_picker_fluid("born", "Date of birth", "1989-04-12");
        let field = child(&node, "field");
        assert_eq!(
            field.constraints.vertical.min,
            Some(SIZE_FLUID),
            "the fluid well takes the one fluid height"
        );
        assert_eq!(SIZE_FLUID, 64.0);
        assert_eq!(
            token(child(field, "label"), "foreground"),
            Some(TEXT_MUTED),
            "the label is inside the well and muted"
        );
        let row = child(field, "row");
        assert_eq!(
            child(row, "value").props.text.as_deref(),
            Some("1989-04-12")
        );
        assert_eq!(
            child(row, "calendar-mark").semantics.label.as_deref(),
            Some("calendar"),
            "the mark carries its word, never the glyph alone"
        );
        assert_eq!(
            field.behaviour,
            Some(Behaviour {
                intent: Intent::Toggle,
                phase: Phase::OnRelease,
            }),
            "fluid is a dressing, not a different control"
        );
    }

    /// The range form is two dated wells sharing one calendar, the
    /// calendar hanging under the pair's leading edge, and the marks the
    /// shared grid draws are the run the two fields stand for.
    #[test]
    fn the_range_pair_is_two_fields_sharing_one_calendar_under_the_pair() {
        let node = date_picker_range_showing(
            "trip",
            "Start date",
            "2026-08-03",
            "End date",
            "2026-08-06",
            ["2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06"],
            Calendar::Compact {
                year: 2026,
                month: 8,
            },
        );
        let fields = child(&node, "fields");
        assert_eq!(fields.kind, NodeKind::Grid);
        for key in ["start", "end"] {
            let field = child(fields, key);
            assert_eq!(
                field.behaviour,
                Some(Behaviour {
                    intent: Intent::Toggle,
                    phase: Phase::OnRelease,
                }),
                "{key} is a dated well like the single form's"
            );
            assert_eq!(field.semantics.expanded, Some(true));
        }
        let calendar = child(&node, "calendar");
        match &calendar.props.anchor {
            Some(Anchor::Sibling { key, edge, .. }) => {
                assert_eq!(key.as_str(), "fields");
                assert_eq!(*edge, Edge::Bottom);
            }
            other => panic!("the calendar hangs off a sibling, got {other:?}"),
        }
        let days = child(child(child(&node, "calendar"), "content"), "days");
        assert_eq!(
            token(child(days, "day-3"), "background"),
            Some(ACCENT_PRIMARY),
            "the run's near end is an edge"
        );
        assert_eq!(
            token(child(days, "day-4"), "background"),
            Some(SURFACE_LAYER_TWO),
            "the run's interior is the lighter step"
        );
    }

    /// The time picker is one labelled group: the hour:minute field beside
    /// the selects the format calls for, each part carrying its own word
    /// as its accessible name, and the widths the slice measures (25/25/50
    /// with both selects) as grid weights.
    #[test]
    fn the_time_picker_groups_the_field_with_the_selects_the_format_calls_for() {
        let node = time_picker("t", "Time", "10:30", Some("AM"), Some("GMT-5"));
        assert_eq!(
            child(&node, "label").props.text.as_deref(),
            Some("Time"),
            "the one label names the group and the field"
        );
        let group = child(&node, "group");
        assert_eq!(group.kind, NodeKind::Grid);
        assert_eq!(
            group.props.columns.len(),
            5,
            "field, divider, clock, divider, zone"
        );
        assert_eq!(
            group.props.columns,
            vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 2.0 },
            ],
            "the slice's 25%/25%/50% are weights over the group"
        );
        assert_eq!(child(group, "field").props.text.as_deref(), Some("10:30"));
        let meridiem = child(group, "meridiem");
        assert_eq!(meridiem.semantics.label.as_deref(), Some("AM/PM"));
        assert_eq!(
            child(child(meridiem, "row"), "value").props.text.as_deref(),
            Some("AM"),
            "the bare well displays the chosen meridiem on its value text, \
             the way a select's field does"
        );
        let timezone = child(group, "timezone");
        assert_eq!(timezone.semantics.label.as_deref(), Some("timezone"));
        assert_eq!(
            child(child(timezone, "row"), "value").props.text.as_deref(),
            Some("GMT-5")
        );
        for divider in ["div-meridiem", "div-timezone"] {
            assert_eq!(
                child(group, divider).kind,
                NodeKind::Separator,
                "{divider} is the 1px divider, not a padded gap"
            );
        }
    }

    /// A dropped part widens what is left, per the same shares: 50/50
    /// with one select, and the field alone when the format needs neither.
    #[test]
    fn dropping_a_time_picker_part_widens_what_is_left() {
        let one = time_picker("t", "Time", "10:30", Some("AM"), None);
        let group = child(&one, "group");
        assert_eq!(
            group.props.columns,
            vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 1.0 },
            ],
            "field and clock split the group 50/50"
        );
        assert!(
            group.children.iter().all(|c| c.key.as_str() != "timezone"),
            "a None part is dropped, not left empty"
        );
        let bare = time_picker("t", "Time", "10:30", None, None);
        let group = child(&bare, "group");
        assert_eq!(
            group.props.columns,
            vec![TrackSize::Weight { weight: 2.0 }],
            "the field takes the whole group when there is nothing beside it"
        );
    }

    /// The closed range form is the pair alone: both wells resting, no
    /// calendar, 50/50 across the group.
    #[test]
    fn the_closed_range_form_is_the_pair_alone() {
        let node = date_picker_range("trip", "Start date", "2026-08-03", "End date", "2026-08-06");
        assert_eq!(node.children.len(), 1, "closed is the pair alone");
        let fields = child(&node, "fields");
        assert_eq!(fields.props.columns.len(), 2, "start and end, 50/50");
        for key in ["start", "end"] {
            assert_eq!(child(fields, key).semantics.expanded, Some(false));
        }
    }
}
