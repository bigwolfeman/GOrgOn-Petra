//! Carbon Date picker (slice-b). Closed field plus a one-month calendar
//! popover. No date library.
//!
//! Anatomy (`_date-picker.scss` + `_flatpickr.scss`):
//! 1. Field — the shared field well ([`super::field::bind_field_chrome`]:
//!    a fill and a bottom rule, no box, no radius), height md 40.
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

use super::icon::{IconMark, IconTone, icon_toned};
use super::pad;
use super::stack;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, LAYER_HOVER, SHADOW_OVERLAY, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED,
    TEXT_MUTED, TEXT_ON_ACCENT, TEXT_PRIMARY, TYPOGRAPHY_HEADING_SM, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    Align as PropAlign, Anchor, AxisConstraint, ClampRule, Constraints, Edge, FocusFigure,
    FocusShownOn, InputPolicy, Interaction, Justify, Key, Layer, NodeKind, Props, Role, Tip,
    TrackSize, ViewNode,
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
    let field = closed_field("field", label, value, false);
    column(key, field, None)
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
pub fn date_picker_showing(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    calendar: Calendar,
) -> ViewNode {
    let label = label.into();
    let value = value.into();
    let (year, month) = calendar.view();
    // The selected day belongs to the month it was picked in. Browsing away
    // from that month must not carry the accent fill with it onto whatever
    // day happens to share the number.
    let selected = match parse_date(&value) {
        Some((y, m, day)) if y == year && m == month => day,
        _ => 0,
    };
    let field = closed_field("field", label.clone(), value, true);
    column(
        key,
        field,
        Some(calendar_surface(label, year, month, selected, calendar)),
    )
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

fn closed_field(
    key: impl Into<Key>,
    label: impl Into<String>,
    value: impl Into<String>,
    expanded: bool,
) -> ViewNode {
    let label = label.into();
    let mut value_node = text("value", value.into());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut mark = icon_toned("calendar-mark", IconMark::Calendar, IconTone::Primary);
    mark.semantics.label = Some("calendar".to_owned());

    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, mark],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
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
        .with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, FIELD_INTENTS)
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
fn calendar_surface(
    label: String,
    year: i32,
    month: u32,
    selected: u32,
    calendar: Calendar,
) -> ViewNode {
    let mut children = vec![
        month_header(year, month, calendar),
        month_grid(year, month, selected),
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
                key: Key::new("field"),
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
    row.constraints = pin_height(SIZE_MD);
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
    node.with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label.to_owned(), FIELD_INTENTS)
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
        .with_constraints(pin_height(SIZE_MD))
        .interactive(
            Role::Button,
            format!("{name}, choose month and year"),
            FIELD_INTENTS,
        )
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
    row.constraints = pin_height(SIZE_MD);
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
        .with_constraints(pin_height(MONTH_CELL_H))
        .interactive(Role::Button, name.to_owned(), FIELD_INTENTS)
        .owning_its_text()
        // `Border`, for the day cell's second reason: a selected month fills
        // `ACCENT_PRIMARY`, and a stripe on that fill cannot be seen.
        .with_focus_figure(FocusFigure::Border);
    node.semantics.selected = selected;
    node
}

/// The weekday row plus six week rows of the real month.
fn month_grid(year: i32, month: u32, selected: u32) -> ViewNode {
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
            children.push(day_button(day, day == selected));
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

/// A day of the month on show. `selected` fills it with the accent, which
/// is Carbon's `.cds--date-picker__day--selected`.
fn day_button(day: u32, selected: bool) -> ViewNode {
    let label = day.to_string();
    let mut caption = text("label", label.clone());
    caption.props.tokens.insert(
        "foreground".into(),
        t(if selected {
            TEXT_ON_ACCENT
        } else {
            TEXT_PRIMARY
        }),
    );
    let mut node = stack(format!("day-{day}"), Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    // Resting background: the calendar's own content fill, the ground every
    // day cell sits on. Without this, `background@hover` has no resting
    // `background` beneath it and resolves to nothing at rest — the
    // Accordion/Modal/AI-label defect, generalised (see
    // `a_state_decorated_token_always_has_a_resting_binding`).
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
        .with_constraints(pin_height(SIZE_MD))
        .interactive(Role::Button, label, FIELD_INTENTS)
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
    node.semantics.selected = selected;
    node
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
    cell.with_constraints(pin_height(SIZE_MD))
}

/// `child` in a seat that centres it on both axes inside its grid track.
fn centred(key: String, child: ViewNode) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![child]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CALENDAR_H, CALENDAR_W, CHOOSER_H, Calendar, MONTH_CELL_H, MONTH_COLS, MONTH_ROWS,
        PropAlign, SIZE_MD, WEEK_ROWS, WEEKDAYS, date_picker, date_picker_open,
        date_picker_showing, days_in_month, first_weekday, parse_date,
    };
    use crate::component::tokens::{ACCENT_PRIMARY, BORDER_STRONG, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{
        Anchor, ClampRule, FocusFigure, FocusShownOn, Interaction, NodeKind, Props, Registry, Role,
        Tip, ViewNode,
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

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
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
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
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
}
