//! Spec 009's Calendar compound, hosted directly through the [`Compound`]
//! triple, in all three [`Mode`]s side by side.
//!
//! Row 10 (Date picker) is the pure, argument-driven atomic with its own
//! open/shut field; this compound has no closed form at all — `view`
//! always draws the grid — and adds the selection set [`Mode`] governs:
//! Single replaces the pick, Range fills inclusive between two picks,
//! Multi toggles each day on its own. One `Calendar::Props`/`State` pair
//! per mode, so all three are on screen and driven independently.

use gorgon_petra::component::{heading, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;
use gorgon_petra_compound::Compound;
use gorgon_petra_compound::calendar::{
    Calendar as CalendarCompound, Date, Intent, Mode, Props, State,
};

use super::Page;
use super::common::{body, column, path_has, row, sp, wrapped};

/// One pane's owner key, wrapping [`CalendarCompound::view`]'s own
/// `"calendar"` root so all three panes can nest that key as a sibling
/// without a name clash — the same arrangement `page/date_picker.rs`'s
/// `COMPACT`/`FULL` use for the same reason.
const SINGLE: &str = "single";
const RANGE: &str = "range";
const MULTI: &str = "multi";

/// One mode's live props and state.
struct Pane {
    props: Props,
    state: State,
}

impl Pane {
    /// Build a pane in `mode`, seeded by replaying `seed` through real
    /// [`Intent::Pick`] calls rather than hand-built state, so the resting
    /// picture is what driving the compound actually produces.
    fn new(label: &str, mode: Mode, seed: &[Date]) -> Self {
        let props = Props {
            label: label.into(),
            mode,
        };
        let mut state = CalendarCompound::init(&props);
        for date in seed {
            CalendarCompound::update(&mut state, Intent::Pick(*date));
        }
        Self { props, state }
    }

    fn view(&self, owner: &'static str, title: &'static str) -> ViewNode {
        column(
            owner,
            sp("spacing.sm"),
            vec![
                heading(format!("{owner}-title"), title),
                CalendarCompound::view(&self.state, &self.props),
            ],
        )
    }

    /// React to a press inside this pane's own subtree. Always consumes:
    /// the caller only reaches this once `owner`'s key is confirmed present
    /// in `node`.
    fn handle(&mut self, node: &str) -> bool {
        if let Some(day) = segment_number(node, "day-") {
            let date = Date {
                year: self.state.year,
                month: self.state.month,
                day,
            };
            CalendarCompound::update(&mut self.state, Intent::Pick(date));
        } else if path_has(node, "prev-month") {
            CalendarCompound::update(&mut self.state, Intent::PrevMonth);
        } else if path_has(node, "next-month") {
            CalendarCompound::update(&mut self.state, Intent::NextMonth);
        } else {
            return false;
        }
        true
    }
}

/// Live state of the Calendar (compound) page: one [`Pane`] per [`Mode`].
pub struct CalendarCompoundPage {
    single: Pane,
    range: Pane,
    multi: Pane,
}

impl Default for CalendarCompoundPage {
    fn default() -> Self {
        Self {
            single: Pane::new(
                "Single",
                Mode::Single,
                &[Date {
                    year: 2026,
                    month: 8,
                    day: 15,
                }],
            ),
            range: Pane::new(
                "Range",
                Mode::Range,
                &[
                    Date {
                        year: 2026,
                        month: 8,
                        day: 3,
                    },
                    Date {
                        year: 2026,
                        month: 8,
                        day: 9,
                    },
                ],
            ),
            multi: Pane::new(
                "Multi",
                Mode::Multi,
                &[
                    Date {
                        year: 2026,
                        month: 8,
                        day: 4,
                    },
                    Date {
                        year: 2026,
                        month: 8,
                        day: 18,
                    },
                    Date {
                        year: 2026,
                        month: 8,
                        day: 27,
                    },
                ],
            ),
        }
    }
}

impl Page for CalendarCompoundPage {
    fn row(&self) -> &'static str {
        "Calendar (compound)"
    }

    fn body(&self) -> ViewNode {
        section(
            "calendar-compound",
            "Calendar (compound)",
            vec![body(
                "calc-body",
                sp("spacing.md"),
                vec![
                    wrapped(
                        "calc-note",
                        "The same triple as spec 009 T032, one pane per \
                         `Mode`: Single replaces the pick, Range fills \
                         inclusive between two picks, Multi toggles each \
                         day on its own.",
                    ),
                    row(
                        "calc-row",
                        sp("spacing.lg"),
                        vec![
                            self.single.view(SINGLE, "Single"),
                            self.range.view(RANGE, "Range"),
                            self.multi.view(MULTI, "Multi"),
                        ],
                    ),
                ],
            )],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        if path_has(node, SINGLE) {
            self.single.handle(node)
        } else if path_has(node, RANGE) {
            self.range.handle(node)
        } else if path_has(node, MULTI) {
            self.multi.handle(node)
        } else {
            false
        }
    }
}

/// The number in the first path segment of `node` spelled `<prefix><n>`.
/// Copied from `page/date_picker.rs`: whole segments, so `day-30` is a day
/// and `day-header` is not a malformed one.
fn segment_number(node: &str, prefix: &str) -> Option<u32> {
    node.split('/')
        .find_map(|part| part.strip_prefix(prefix)?.parse::<u32>().ok())
}
