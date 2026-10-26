//! Inventory row 10, Date picker.
//!
//! Two of them, side by side, because the operator asked for two: *"we need
//! 2 date pickers 'compact' and 'full' where compact is what we have now,
//! and full lets me click the month to get another menu that lets me pick
//! month and year, and in the full view the month is a button, not just a
//! label."* Side by side and not stacked so both calendars hang from the
//! same line and neither has to flip above its field to stay on the window.
//!
//! Each picker owns a [`Pane`]: what is open, which month the grid is
//! browsing, and which date is picked. Browsing and picking are two
//! different pieces of state and were one until 2026-09-05, which is the
//! whole of the other defect this file answers — see
//! `.agents/notes/implemented/bug-fix/2026-09-05-a-month-arrow-is-not-a-press-on-the-field.md`.

use gorgon_petra::component::{Calendar, date_picker, date_picker_showing, heading, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::{NodeKind, ViewNode};

use super::Page;
use super::common::{body, column, dismissed_under, path_has, row, sp};

/// The compact picker's key: Carbon's calendar, month name as a caption.
const COMPACT: &str = "compact";
/// The full picker's key: the month is a button that raises a chooser.
const FULL: &str = "full";
/// The calendar popover's key inside the open date picker.
const CALENDAR: &str = "calendar";
/// The month/year chooser's key inside the full form's calendar.
const CHOOSER: &str = "chooser";
/// The full form's month control.
const MONTH_BUTTON: &str = "month-button";
/// The calendar header's back arrow.
const PREV_MONTH: &str = "prev-month";
/// The calendar header's forward arrow.
const NEXT_MONTH: &str = "next-month";
/// The chooser's back arrow.
const PREV_YEAR: &str = "prev-year";
/// The chooser's forward arrow.
const NEXT_YEAR: &str = "next-year";
/// A day button's key prefix inside the calendar (`day-30`).
const DAY: &str = "day-";
/// A month cell's key prefix inside the chooser (`mon-9`).
const MON: &str = "mon-";

/// One picker's live state.
///
/// `year`/`month` is what the grid is **browsing** and `picked` is what is
/// **selected**. Carbon's arrows turn the first and leave the second alone,
/// so a single field cannot serve both: stepping the month would drag the
/// selection along with it.
struct Pane {
    open: bool,
    /// Full form only: the month/year chooser is over the day grid.
    choosing: bool,
    /// Year the grid is browsing.
    year: i32,
    /// Month the grid is browsing, `1..=12`.
    month: u32,
    /// The selected date, `(year, month, day)`, in the shape
    /// `gorgon_petra::component`'s date parser reads back.
    picked: (i32, u32, u32),
}

impl Pane {
    fn new() -> Self {
        Self {
            open: false,
            choosing: false,
            year: 2026,
            month: 8,
            picked: (2026, 8, 30),
        }
    }

    /// The field's visible text, `YYYY-MM-DD`.
    fn value(&self) -> String {
        let (y, m, d) = self.picked;
        format!("{y}-{m:02}-{d:02}")
    }

    /// Step the browsed month one on, carrying the year at December.
    ///
    /// Spelled out rather than done in modular arithmetic on a signed
    /// offset: `month` is a `u32` in `1..=12`, and every arithmetic form of
    /// this needs a fallible `i32` conversion whose failure arm would have
    /// to invent a month. There is no arm here to get wrong.
    fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
    }

    /// Step the browsed month one back, carrying the year at January.
    fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
    }

    /// The tree for this picker, closed or open.
    fn view(&self, key: &'static str, label: &str, full: bool) -> ViewNode {
        if !self.open {
            return date_picker(key, label, self.value());
        }
        let (year, month) = (self.year, self.month);
        let calendar = match (full, self.choosing) {
            (false, _) => Calendar::Compact { year, month },
            (true, false) => Calendar::Full { year, month },
            (true, true) => Calendar::Choosing { year, month },
        };
        date_picker_showing(key, label, self.value(), calendar)
    }

    /// React to an activation inside this picker's subtree. Always consumes:
    /// the path named this pane, so nothing under it belongs to the chrome.
    ///
    /// The order of the arms is the fix for *"I click an arrow and it goes
    /// away"*. Every node in the open calendar carries this pane's key in
    /// its path, so a handler that asked "is this pane named?" first read a
    /// press on the next-month arrow as a press on the field and toggled the
    /// whole picker shut. The field is now the **last** arm, reached only
    /// when the press was not on any control inside the calendar.
    fn handle(&mut self, node: &str) -> bool {
        if let Some(day) = segment_number(node, DAY) {
            self.picked = (self.year, self.month, day);
            self.open = false;
            self.choosing = false;
        } else if let Some(month) = segment_number(node, MON) {
            self.month = month;
            self.choosing = false;
        } else if path_has(node, PREV_MONTH) {
            self.prev_month();
        } else if path_has(node, NEXT_MONTH) {
            self.next_month();
        } else if path_has(node, PREV_YEAR) {
            self.year -= 1;
        } else if path_has(node, NEXT_YEAR) {
            self.year += 1;
        } else if path_has(node, MONTH_BUTTON) {
            self.choosing = !self.choosing;
        } else if path_has(node, CALENDAR) {
            // Calendar chrome with no control on it: a weekday initial, an
            // adjacent-month cell, the panel's own ground. Consumed and
            // ignored, so a press that lands between the buttons leaves the
            // calendar exactly as it was rather than falling through to the
            // field arm below and shutting it.
        } else {
            self.open = !self.open;
            self.choosing = false;
        }
        true
    }
}

/// The number in the first path segment of `node` spelled `<prefix><n>`.
///
/// Whole segments, so `mon-9` is a month and `month-header` is not a
/// malformed one.
fn segment_number(node: &str, prefix: &str) -> Option<u32> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix)?.parse::<u32>().ok())
}

/// Live state of the Date picker page: one [`Pane`] per form.
pub struct DatePicker {
    compact: Pane,
    full: Pane,
}

impl Default for DatePicker {
    fn default() -> Self {
        Self {
            compact: Pane::new(),
            full: Pane::new(),
        }
    }
}

impl Page for DatePicker {
    fn row(&self) -> &'static str {
        "Date picker"
    }

    /// The warning cell (spec 008 US4, FR-019): the picker's own
    /// `date_picker_warning`, one cell — the state the slice documents.
    fn warning(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::date_picker_warning;

        vec![date_picker_warning(
            "dp-warn",
            "Due date",
            "2026-08-30",
            "date is in the past",
        )]
    }

    /// Carbon's date-picker skeleton (`_date-picker-next.scss`): the input
    /// skeleton is a 14×75 block; the range variant puts two at 143.5 wide.
    fn skeleton(&self, now: f64) -> Option<ViewNode> {
        use gorgon_petra::component::{SizeStep, SkeletonAnatomy, skeleton};

        use super::common::{row, sp};
        Some(row(
            "skeleton-blocks",
            sp("spacing-05"),
            vec![
                skeleton(
                    "skeleton-input",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    75.0,
                    now,
                ),
                skeleton(
                    "skeleton-range-a",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    143.5,
                    now,
                ),
                skeleton(
                    "skeleton-range-b",
                    SkeletonAnatomy::Caption,
                    SizeStep::Md,
                    143.5,
                    now,
                ),
            ],
        ))
    }

    /// The documented disabled state (spec 008 T025, register row 166): the
    /// field reads in `text.muted`, its edge in the `border@disabled` fade
    /// (slice-b States; the field is `component::field`'s anatomy).
    fn disabled(&self) -> Vec<ViewNode> {
        use gorgon_petra::component::disabled;

        vec![disabled(row(
            "disabled",
            sp("spacing.md"),
            vec![date_picker("disabled-picker", "Date", "2026-10-24")],
        ))]
    }

    fn body(&self) -> ViewNode {
        use gorgon_petra::component::{
            date_picker_fluid, date_picker_input, date_picker_invalid, date_picker_range,
            date_picker_range_showing, text, time_picker,
        };

        section(
            "date",
            "Date picker",
            vec![body(
                "dp",
                // `spacing.sm`, not `spacing.md`: the raised range
                // calendar drops 336 units flush below its fields, so they
                // must end by y=564 to fit the 900-unit viewport. The row
                // gaps give up 4 each and land them at 518; at `spacing.md`
                // they sat at 530, the placement flipped the calendar up
                // over `full/field`, and the month-button drive test
                // pressed the calendar instead of the field.
                sp("spacing.sm"),
                vec![
                    row(
                        "dp-forms",
                        sp("spacing.lg"),
                        vec![
                            column(
                                "compact-form",
                                sp("spacing.sm"),
                                vec![
                                    heading("compact-title", "Compact"),
                                    self.compact.view(COMPACT, "Date", false),
                                ],
                            ),
                            // `forms-gap`: a spacer, not a third form. A
                            // `Stack` divides its width in equal shares,
                            // so this buys the middle third of the row for
                            // open calendars to drop through. Without it
                            // `full/calendar` (x=734..1022 when open)
                            // shares that x-range with
                            // `v-range-open/calendar` (x=586..874) below,
                            // and `clear_run` — which measures each open
                            // calendar's tallest leftmost bar against the
                            // next lower pixel — fails the page however the
                            // drives layer the two.
                            ViewNode::new(NodeKind::Spacer, "forms-gap"),
                            column(
                                "full-form",
                                sp("spacing.sm"),
                                vec![
                                    heading("full-title", "Full"),
                                    self.full.view(FULL, "Date", true),
                                ],
                            ),
                        ],
                    ),
                    heading("variants-title", "The documented forms"),
                    row(
                        "dp-variants",
                        sp("spacing.lg"),
                        vec![
                            // T049's simple input: the text field and
                            // nothing else — no calendar, no mark.
                            column(
                                "v-input",
                                sp("spacing.sm"),
                                vec![
                                    text("v-input-title", "Simple input"),
                                    date_picker_input("var-input", "Date", "1989-04-12"),
                                ],
                            ),
                            // T049's fluid form: the 64-unit well with the
                            // label stacked inside it.
                            column(
                                "v-fluid",
                                sp("spacing.sm"),
                                vec![
                                    text("v-fluid-title", "Fluid"),
                                    date_picker_fluid("var-fluid", "Date", "2026-08-30"),
                                ],
                            ),
                            // T049's time picker: the hh:mm field and its
                            // AM/PM select, flush beside a hairline rule.
                            column(
                                "v-time",
                                sp("spacing.sm"),
                                vec![
                                    text("v-time-title", "Time picker"),
                                    time_picker("var-time", "Time", "3:45 PM", Some("PM"), None),
                                ],
                            ),
                        ],
                    ),
                    row(
                        "dp-variants-2",
                        sp("spacing.lg"),
                        vec![
                            // T049's calendar range, closed: the pair of
                            // dated wells sharing one calendar menu.
                            column(
                                "v-range",
                                sp("spacing.sm"),
                                vec![
                                    text("v-range-title", "Calendar range"),
                                    date_picker_range(
                                        "var-range",
                                        "Start date",
                                        "2026-10-01",
                                        "End date",
                                        "2026-10-25",
                                    ),
                                ],
                            ),
                            // The same range with its shared calendar
                            // raised, the marked run's interior one tone
                            // step up between its accented ends.
                            column(
                                "v-range-open",
                                sp("spacing.sm"),
                                vec![
                                    text("v-range-open-title", "Calendar range, raised"),
                                    date_picker_range_showing(
                                        "var-range-open",
                                        "Start date",
                                        "2026-10-01",
                                        "End date",
                                        "2026-10-25",
                                        ["2026-10-05", "2026-10-06", "2026-10-07"],
                                        Calendar::Compact {
                                            year: 2026,
                                            month: 10,
                                        },
                                    ),
                                ],
                            ),
                            // T049's validation states: the invalid form
                            // here, the warning form in the page's own
                            // Warning cell below.
                            column(
                                "v-invalid",
                                sp("spacing.sm"),
                                vec![
                                    text("v-invalid-title", "Invalid"),
                                    date_picker_invalid(
                                        "var-invalid",
                                        "Date",
                                        "2026-13-45",
                                        "date does not exist",
                                    ),
                                ],
                            ),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // Which picker the press landed in is settled first and once. Both
        // pickers key their insides identically -- two `calendar`s, two
        // `day-12`s -- so every arm below would otherwise be ambiguous.
        if path_has(node, COMPACT) {
            self.compact.handle(node)
        } else if path_has(node, FULL) {
            self.full.handle(node)
        } else {
            false
        }
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismissed_under(ids, COMPACT, CALENDAR) {
            self.compact.open = false;
            self.compact.choosing = false;
        }
        if dismissed_under(ids, FULL, CHOOSER) {
            self.full.choosing = false;
        }
        if dismissed_under(ids, FULL, CALENDAR) {
            self.full.open = false;
            self.full.choosing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{COMPACT, DatePicker, FULL};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};
    use gorgon_petra::tree::ViewNode;

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    /// The subtree of one picker, so a key both pickers carry resolves to
    /// the one being asked about.
    fn pane<'a>(tree: &'a ViewNode, owner: &str) -> &'a ViewNode {
        find(tree, owner).unwrap_or_else(|| panic!("no picker keyed {owner}"))
    }

    fn text_in(page: &DatePicker, owner: &str, key: &str) -> Option<String> {
        let tree = page.body();
        find(pane(&tree, owner), key).and_then(|node| node.props.text.clone())
    }

    fn has(page: &DatePicker, owner: &str, key: &str) -> bool {
        let tree = page.body();
        find(pane(&tree, owner), key).is_some()
    }

    /// The field opens the calendar, a day picks and closes it, and a press
    /// outside the calendar closes it through the dismissal path.
    #[test]
    fn the_field_opens_a_day_picks_and_an_outside_press_dismisses() {
        let mut page = DatePicker::default();
        assert!(!has(&page, COMPACT, "calendar"));
        assert!(page.handle(&press(), "/page/dp/compact/field/value"));
        assert!(has(&page, COMPACT, "calendar"), "the field opened it");
        assert!(
            !has(&page, FULL, "calendar"),
            "opening one picker opened the other"
        );
        assert!(page.handle(
            &press(),
            "/page/dp/compact/calendar/content/days/day-12/label"
        ));
        assert!(!has(&page, COMPACT, "calendar"), "a day closed it");
        assert_eq!(
            text_in(&page, COMPACT, "value").as_deref(),
            Some("2026-08-12")
        );

        assert!(page.handle(&press(), "/page/dp/compact/field"));
        assert!(has(&page, COMPACT, "calendar"));
        page.dismissed(&["/page/dp/compact/calendar".to_owned()]);
        assert!(!has(&page, COMPACT, "calendar"), "dismissed");
        page.dismissed(&["/page/other/menu".to_owned()]);
        assert!(!has(&page, COMPACT, "calendar"));
    }

    /// R1. A month arrow turns the month on show and leaves the calendar
    /// open and the selected date alone. December steps into the next
    /// January and back.
    #[test]
    fn a_month_arrow_turns_the_month_and_never_closes_the_calendar() {
        let mut page = DatePicker::default();
        page.handle(&press(), "/page/dp/compact/field");
        let arrow = |page: &mut DatePicker, key: &str| {
            let node = format!("/page/dp/compact/calendar/content/month-header/{key}/glyph");
            assert!(page.handle(&press(), &node));
        };
        arrow(&mut page, "next-month");
        assert!(
            has(&page, COMPACT, "calendar"),
            "the arrow closed the calendar"
        );
        assert_eq!(
            text_in(&page, COMPACT, "month").as_deref(),
            Some("September 2026")
        );
        assert_eq!(
            text_in(&page, COMPACT, "value").as_deref(),
            Some("2026-08-30"),
            "browsing moved the selected date"
        );
        for _ in 0..4 {
            arrow(&mut page, "next-month");
        }
        assert_eq!(
            text_in(&page, COMPACT, "month").as_deref(),
            Some("January 2027"),
            "December did not carry into the next year"
        );
        arrow(&mut page, "prev-month");
        assert_eq!(
            text_in(&page, COMPACT, "month").as_deref(),
            Some("December 2026"),
            "January did not carry back"
        );
        assert!(has(&page, COMPACT, "calendar"));
    }

    /// R2. The compact form's month is a caption; the full form's is a
    /// button that raises a chooser, and a month cell in that chooser moves
    /// the grid without moving the selection.
    #[test]
    fn only_the_full_form_has_a_month_button_and_it_opens_the_chooser() {
        let mut page = DatePicker::default();
        page.handle(&press(), "/page/dp/compact/field");
        page.handle(&press(), "/page/dp/full/field");
        assert!(
            !has(&page, COMPACT, "month-button"),
            "the compact form grew a month button"
        );
        assert!(
            has(&page, FULL, "month-button"),
            "the full form's month is not a button"
        );
        assert!(!has(&page, FULL, "chooser"), "the chooser is up at rest");

        let full = |page: &mut DatePicker, tail: &str| {
            assert!(page.handle(&press(), &format!("/page/dp/full/calendar/{tail}")));
        };
        full(&mut page, "content/month-header/month-button/glyph");
        assert!(
            has(&page, FULL, "chooser"),
            "the month button opened nothing"
        );
        assert!(
            !has(&page, COMPACT, "chooser"),
            "the full form's button reached the compact picker"
        );
        assert_eq!(text_in(&page, FULL, "year").as_deref(), Some("2026"));

        full(
            &mut page,
            "chooser/chooser-content/year-header/next-year/glyph",
        );
        assert_eq!(text_in(&page, FULL, "year").as_deref(), Some("2027"));
        assert!(has(&page, FULL, "chooser"), "the year arrow closed it");

        full(&mut page, "chooser/chooser-content/months/mon-3/label");
        assert!(!has(&page, FULL, "chooser"), "picking a month left it up");
        assert_eq!(
            text_in(&page, FULL, "month").as_deref(),
            Some("March 2027"),
            "the chooser did not move the grid"
        );
        assert_eq!(
            text_in(&page, FULL, "value").as_deref(),
            Some("2026-08-30"),
            "choosing a month moved the selected date"
        );
        assert!(has(&page, FULL, "calendar"), "the calendar shut");
    }

    /// The month button toggles: pressing it while the chooser is up shuts
    /// it, and the dismissal that arrives after the handler finds it
    /// already shut rather than reopening it.
    #[test]
    fn the_month_button_shuts_the_chooser_it_opened() {
        let mut page = DatePicker::default();
        page.handle(&press(), "/page/dp/full/field");
        let button = "/page/dp/full/calendar/content/month-header/month-button";
        page.handle(&press(), button);
        assert!(has(&page, FULL, "chooser"));
        page.handle(&press(), button);
        page.dismissed(&["/page/dp/full/calendar/content/chooser".to_owned()]);
        assert!(!has(&page, FULL, "chooser"));
        assert!(
            has(&page, FULL, "calendar"),
            "shutting the chooser shut the calendar with it"
        );
    }

    /// A month arrow pressed while the chooser is up steps the month and
    /// shuts the chooser, because the arrow sits in the header and the
    /// chooser hangs below it: the press is inside the calendar and outside
    /// the chooser, so both the handler and the dismissal have something to
    /// say. Stated here so it stays a decision rather than an accident.
    #[test]
    fn a_month_arrow_pressed_over_the_chooser_steps_and_shuts_it() {
        let mut page = DatePicker::default();
        page.handle(&press(), "/page/dp/full/field");
        page.handle(
            &press(),
            "/page/dp/full/calendar/content/month-header/month-button",
        );
        assert!(has(&page, FULL, "chooser"));
        page.handle(
            &press(),
            "/page/dp/full/calendar/content/month-header/next-month/glyph",
        );
        page.dismissed(&["/page/dp/full/calendar/content/chooser".to_owned()]);
        assert!(!has(&page, FULL, "chooser"), "the chooser survived");
        assert!(has(&page, FULL, "calendar"), "the calendar went with it");
        assert_eq!(
            text_in(&page, FULL, "month").as_deref(),
            Some("September 2026")
        );
    }

    /// A press outside the compact picker's calendar leaves the full
    /// picker's open. Both surfaces are keyed `calendar`, so a matcher that
    /// looked only at the key would shut both.
    #[test]
    fn dismissing_one_pickers_calendar_leaves_the_others_open() {
        let mut page = DatePicker::default();
        page.handle(&press(), "/page/dp/compact/field");
        page.handle(&press(), "/page/dp/full/field");
        assert!(has(&page, COMPACT, "calendar") && has(&page, FULL, "calendar"));
        page.dismissed(&["/page/dp/compact/calendar".to_owned()]);
        assert!(!has(&page, COMPACT, "calendar"));
        assert!(has(&page, FULL, "calendar"), "the wrong calendar shut");
    }

    /// T049's simple-input cell: the text field and nothing else — no
    /// calendar behind it, the whole of `datePickerType="simple"`.
    #[test]
    fn the_page_carries_the_simple_input_form() {
        let tree = DatePicker::default().body();
        let cell = find(&tree, "var-input").expect("the simple input is on the page");
        assert!(find(cell, "input").is_some(), "its well is there");
        assert!(
            find(cell, "calendar").is_none(),
            "a simple input has no calendar"
        );
    }

    /// T049's fluid cell: the 64-unit well with the label stacked inside.
    #[test]
    fn the_page_carries_the_fluid_form() {
        let tree = DatePicker::default().body();
        let cell = find(&tree, "var-fluid").expect("the fluid picker is on the page");
        assert!(find(cell, "field").is_some(), "its fluid well is there");
    }

    /// T049's time-picker cell: the hh:mm field and its AM/PM select in
    /// one group.
    #[test]
    fn the_page_carries_the_time_picker_form() {
        let tree = DatePicker::default().body();
        let cell = find(&tree, "var-time").expect("the time picker is on the page");
        assert!(find(cell, "field").is_some(), "the hh:mm well is there");
        assert!(
            find(cell, "meridiem").is_some(),
            "the AM/PM select is beside it"
        );
    }

    /// T049's calendar-range cell: the closed pair — two dated wells under
    /// one key — and the raised form beside it carrying the shared
    /// calendar with the marked run.
    #[test]
    fn the_page_carries_both_calendar_range_forms() {
        let tree = DatePicker::default().body();
        let closed = find(&tree, "var-range").expect("the closed range is on the page");
        assert!(
            find(closed, "fields").is_some(),
            "the pair of wells is there"
        );
        assert!(
            find(closed, "calendar").is_none(),
            "the closed range shows no calendar"
        );
        let open = find(&tree, "var-range-open").expect("the raised range is on the page");
        assert!(
            find(open, "calendar").is_some(),
            "the raised range carries the shared calendar"
        );
    }

    /// T049's validation states: the invalid form in the body, the
    /// warning form in the page's own Warning cell — each wearing its
    /// helper line, the word beside the glyph (FR-018/FR-019).
    #[test]
    fn the_page_carries_both_validation_forms_with_their_helpers() {
        let tree = DatePicker::default().body();
        let invalid = find(&tree, "var-invalid").expect("the invalid form is on the page");
        let helper = find(invalid, "helper").expect("the invalid form wears its helper");
        assert!(
            find(helper, "message").is_some_and(|m| {
                m.props
                    .text
                    .as_deref()
                    .is_some_and(|t| t.starts_with("Invalid:"))
            }),
            "the helper names the word Invalid beside its glyph"
        );
        let warning = DatePicker::default().warning();
        assert_eq!(warning.len(), 1, "one warning cell");
        assert!(
            find(&warning[0], "helper").is_some(),
            "the warning form wears its helper"
        );
    }
}
