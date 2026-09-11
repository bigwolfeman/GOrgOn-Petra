//! Calendar as a [`Compound`]: month on show plus range or multi-select.
//!
//! The constructors in `date_picker.rs` stay put (Q9). This module lifts the
//! year/month-on-show that [`gorgon_petra::component::Calendar`] already
//! carried, adds the selection set, and calls
//! [`date_picker_showing_selection`] with the whole set so every picked day
//! is marked, not only one of them. Date maths that `update` needs
//! (`days_in_month`) is duplicated here because those helpers are
//! crate-private in the picker.
//!
//! Binding: spec 009 T032.

use std::collections::BTreeSet;

use gorgon_petra::component::{Calendar as DatePickerCalendar, date_picker_showing_selection};
use gorgon_petra::tree::ViewNode;
use serde::{Deserialize, Serialize};

use crate::Compound;

/// Namespace over the calendar triple. Never constructed as a value.
pub struct Calendar;

/// Root key of the node [`Calendar::view`] returns.
const CALENDAR_KEY: &str = "calendar";

/// One Gregorian day. Ordered year, month, day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Date {
    /// Proleptic Gregorian year.
    pub year: i32,
    /// Month, `1..=12`.
    pub month: u32,
    /// Day of month, `1..=31` as the month allows.
    pub day: u32,
}

impl Date {
    /// `YYYY-MM-DD`. The spelling [`date_picker_showing`] parses.
    #[must_use]
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// How [`Intent::Pick`] writes [`State::selected`].
///
/// Copied from [`Props`] at [`Calendar::init`] onto [`State`], because
/// `update` does not see Props (`view-fiber.md` §4.1) and Pick still has
/// to branch. The author supplies it; `update` never rewrites it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Replace the set with the picked day.
    Single,
    /// First pick is the start; the next pick fills every day in between.
    Range,
    /// Toggle the picked day in the set.
    #[default]
    Multi,
}

/// Label and selection mode. The author owns these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Props {
    /// Accessible name of the picker.
    pub label: String,
    /// How picks accumulate. Copied onto [`State::mode`] at init.
    pub mode: Mode,
}

impl Default for Props {
    fn default() -> Self {
        Self {
            label: "Date".into(),
            mode: Mode::Multi,
        }
    }
}

/// Month on show and the selected days. Round-trips through reload as a whole.
///
/// Year and month are held separately from the selection for the same reason
/// the picker's `Calendar` enum is: browsing and choosing are two things, and
/// a component that took only the selected date could not say "August is
/// selected, September is on screen".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Year the grid is browsing.
    pub year: i32,
    /// Month the grid is browsing, `1..=12`.
    pub month: u32,
    /// Selected days. Days that fall outside the month on show stay in the
    /// set; [`date_picker_showing`] only marks a day of the month on screen.
    pub selected: BTreeSet<Date>,
    /// Range mode: the first pick of a pair, while the second is pending.
    pub range_anchor: Option<Date>,
    /// [`Props::mode`] at init. Not rewritten by `update`.
    pub mode: Mode,
}

impl Default for State {
    fn default() -> Self {
        Self {
            year: 1970,
            month: 1,
            selected: BTreeSet::new(),
            range_anchor: None,
            mode: Mode::Multi,
        }
    }
}

/// Closed set of things that can happen to the calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    /// Step the month on show backward. Selection is unchanged.
    PrevMonth,
    /// Step the month on show forward. Selection is unchanged.
    NextMonth,
    /// Pick `date`. Invalid dates (month outside `1..=12`, day outside the
    /// month) are a no-op.
    Pick(Date),
}

impl Compound for Calendar {
    type Props = Props;
    type State = State;
    type Intent = Intent;
    type Request = ();

    fn init(props: &Self::Props) -> Self::State {
        State {
            mode: props.mode,
            ..State::default()
        }
    }

    fn update(state: &mut Self::State, intent: Self::Intent) -> Vec<Self::Request> {
        match intent {
            Intent::PrevMonth => {
                if state.month <= 1 {
                    state.year -= 1;
                    state.month = 12;
                } else {
                    state.month -= 1;
                }
            }
            Intent::NextMonth => {
                if state.month >= 12 {
                    state.year += 1;
                    state.month = 1;
                } else {
                    state.month += 1;
                }
            }
            Intent::Pick(date) => {
                if !valid(date) {
                    return Vec::new();
                }
                state.year = date.year;
                state.month = date.month;
                match state.mode {
                    Mode::Single => {
                        state.selected.clear();
                        state.selected.insert(date);
                        state.range_anchor = None;
                    }
                    Mode::Range => match state.range_anchor {
                        None => {
                            state.range_anchor = Some(date);
                            state.selected.clear();
                            state.selected.insert(date);
                        }
                        Some(start) => {
                            state.selected = fill_range(start, date);
                            state.range_anchor = None;
                        }
                    },
                    Mode::Multi => {
                        if !state.selected.remove(&date) {
                            state.selected.insert(date);
                        }
                    }
                }
            }
        }
        Vec::new()
    }

    fn view(state: &Self::State, props: &Self::Props) -> ViewNode {
        let month = state.month.clamp(1, 12);
        date_picker_showing_selection(
            CALENDAR_KEY,
            props.label.as_str(),
            field_value(state),
            state.selected.iter().map(|d| d.iso()),
            DatePickerCalendar::Compact {
                year: state.year,
                month,
            },
        )
    }
}

/// Days in `month` of `year`, Gregorian. Same table as `date_picker.rs`.
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn valid(date: Date) -> bool {
    (1..=12).contains(&date.month)
        && date.day >= 1
        && date.day <= days_in_month(date.year, date.month)
}

fn fill_range(a: Date, b: Date) -> BTreeSet<Date> {
    let (mut cur, end) = if a <= b { (a, b) } else { (b, a) };
    let mut out = BTreeSet::new();
    loop {
        out.insert(cur);
        if cur == end {
            break;
        }
        cur = next_day(cur);
    }
    out
}

fn next_day(date: Date) -> Date {
    let last = days_in_month(date.year, date.month);
    if date.day < last {
        Date {
            day: date.day + 1,
            ..date
        }
    } else if date.month < 12 {
        Date {
            year: date.year,
            month: date.month + 1,
            day: 1,
        }
    } else {
        Date {
            year: date.year + 1,
            month: 1,
            day: 1,
        }
    }
}

/// The closed field's own visible text: a selected day in the month on
/// show, else the earliest selected day, else empty. Only the field reads
/// this now — the grid is marked from the whole of `state.selected`, in
/// [`Calendar::view`], not from this single day.
fn field_value(state: &State) -> String {
    state
        .selected
        .iter()
        .find(|d| d.year == state.year && d.month == state.month)
        .or_else(|| state.selected.iter().next())
        .copied()
        .map(Date::iso)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use gorgon_petra::tree::ViewNode;

    use crate::Compound;

    use super::{CALENDAR_KEY, Calendar, Date, Intent, Mode, Props};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn date(year: i32, month: u32, day: u32) -> Date {
        Date { year, month, day }
    }

    fn range_props() -> Props {
        Props {
            label: "When".into(),
            mode: Mode::Range,
        }
    }

    fn multi_props() -> Props {
        Props {
            label: "When".into(),
            mode: Mode::Multi,
        }
    }

    #[test]
    fn init_copies_mode_and_starts_at_unix_epoch_month() {
        let state = Calendar::init(&range_props());
        assert_eq!(state.mode, Mode::Range);
        assert_eq!(state.year, 1970);
        assert_eq!(state.month, 1);
        assert!(state.selected.is_empty());
    }

    #[test]
    fn prev_and_next_month_wrap_the_year() {
        let mut state = Calendar::init(&multi_props());
        assert!(Calendar::update(&mut state, Intent::PrevMonth).is_empty());
        assert_eq!((state.year, state.month), (1969, 12));
        Calendar::update(&mut state, Intent::NextMonth);
        Calendar::update(&mut state, Intent::NextMonth);
        assert_eq!((state.year, state.month), (1970, 2));
    }

    #[test]
    fn multi_pick_toggles() {
        let mut state = Calendar::init(&multi_props());
        let d = date(2026, 8, 15);
        Calendar::update(&mut state, Intent::Pick(d));
        assert!(state.selected.contains(&d));
        assert_eq!((state.year, state.month), (2026, 8));
        Calendar::update(&mut state, Intent::Pick(d));
        assert!(!state.selected.contains(&d));
    }

    #[test]
    fn range_pick_fills_inclusive() {
        let mut state = Calendar::init(&range_props());
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 3)));
        assert_eq!(state.range_anchor, Some(date(2026, 8, 3)));
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 5)));
        assert!(state.range_anchor.is_none());
        assert_eq!(
            state.selected.iter().copied().collect::<Vec<_>>(),
            vec![date(2026, 8, 3), date(2026, 8, 4), date(2026, 8, 5)]
        );
    }

    #[test]
    fn range_pick_backwards_still_fills() {
        let mut state = Calendar::init(&range_props());
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 5)));
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 3)));
        assert_eq!(state.selected.len(), 3);
        assert!(state.selected.contains(&date(2026, 8, 4)));
    }

    #[test]
    fn invalid_pick_is_a_noop() {
        let mut state = Calendar::init(&multi_props());
        Calendar::update(&mut state, Intent::Pick(date(2026, 2, 31)));
        Calendar::update(&mut state, Intent::Pick(date(2026, 13, 1)));
        assert!(state.selected.is_empty());
        assert_eq!((state.year, state.month), (1970, 1));
    }

    #[test]
    fn leap_day_is_valid_only_on_a_leap_year() {
        let mut state = Calendar::init(&multi_props());
        Calendar::update(&mut state, Intent::Pick(date(2024, 2, 29)));
        assert!(state.selected.contains(&date(2024, 2, 29)));
        Calendar::update(&mut state, Intent::Pick(date(2025, 2, 29)));
        assert_eq!(state.selected.len(), 1);
    }

    #[test]
    fn view_calls_the_public_picker_grid() {
        let props = multi_props();
        let mut state = Calendar::init(&props);
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 30)));
        let node = Calendar::view(&state, &props);
        assert_eq!(node.key.as_str(), CALENDAR_KEY);
        assert_eq!(node.semantics.expanded, Some(true));
        named(&node, "field");
        named(&node, "calendar");
        named(&node, "day-30");
    }

    #[test]
    fn single_pick_replaces() {
        let props = Props {
            label: "When".into(),
            mode: Mode::Single,
        };
        let mut state = Calendar::init(&props);
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 1)));
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 2)));
        assert_eq!(state.selected.len(), 1);
        assert!(state.selected.contains(&date(2026, 8, 2)));
    }

    /// The grid marks every day the state holds, not only one of them. This
    /// is the frame-level property that would have caught the earlier
    /// `date_picker_showing(..., grid_value(state), ...)` defect (a single
    /// ISO string carrying the whole picture): the count of days the tree
    /// marks `semantics.selected` on must equal `state.selected.len()`.
    #[test]
    fn every_selected_day_is_marked_not_only_one() {
        let props = multi_props();
        let mut state = Calendar::init(&props);
        for d in [date(2026, 8, 3), date(2026, 8, 10), date(2026, 8, 20)] {
            Calendar::update(&mut state, Intent::Pick(d));
        }
        assert_eq!(state.selected.len(), 3);
        let node = Calendar::view(&state, &props);
        assert_eq!(count_marked_days(&node), state.selected.len());
    }

    /// The gallery's own Range-pane seed (`page/calendar_compound.rs`,
    /// Pick(3) then Pick(9)) fills the whole 3..=9 span and marks every one
    /// of those seven days, not just the two picks.
    #[test]
    fn the_gallery_range_seed_fills_and_marks_the_whole_span() {
        let props = range_props();
        let mut state = Calendar::init(&props);
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 3)));
        Calendar::update(&mut state, Intent::Pick(date(2026, 8, 9)));
        assert_eq!(state.selected.len(), 7);
        let node = Calendar::view(&state, &props);
        assert_eq!(count_marked_days(&node), 7);
    }

    /// Every node keyed `day-<n>` under `node` with `semantics.selected`
    /// set, counted regardless of depth.
    fn count_marked_days(node: &ViewNode) -> usize {
        let here = usize::from(node.key.as_str().starts_with("day-") && node.semantics.selected);
        here + node
            .children
            .iter()
            .map(|c| count_marked_days(c))
            .sum::<usize>()
    }
}
