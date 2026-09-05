//! Inventory row 10, Date picker.

use gorgon_petra::component::{date_picker, date_picker_open, section};
use gorgon_petra::input::InputEvent;
use gorgon_petra::tree::ViewNode;

use super::Page;
use super::common::{body, dismisses, path_has, sp};

const WHEN: &str = "when";
/// The calendar popover's key inside `date_picker_open`.
const CALENDAR: &str = "calendar";
/// A day button's key prefix inside the calendar (`day-30`).
const DAY: &str = "day-";

/// Live state of the Date picker page: whether the calendar is open, and
/// the chosen day of the one month the calendar shows.
pub struct DatePicker {
    open: bool,
    day: u32,
}

impl Default for DatePicker {
    fn default() -> Self {
        Self {
            open: false,
            day: 30,
        }
    }
}

impl DatePicker {
    fn value(&self) -> String {
        format!("2026-08-{:02}", self.day)
    }
}

impl Page for DatePicker {
    fn row(&self) -> &'static str {
        "Date picker"
    }

    fn body(&self) -> ViewNode {
        let field = if self.open {
            date_picker_open(WHEN, "Date", self.value())
        } else {
            date_picker(WHEN, "Date", self.value())
        };
        section(
            "date",
            "Date picker",
            vec![body("dp", sp("spacing.md"), vec![field])],
        )
    }

    fn handle(&mut self, _event: &InputEvent, node: &str) -> bool {
        // A day is inside the field's own subtree, so it is matched first.
        if let Some(day) = node
            .split('/')
            .find_map(|part| part.strip_prefix(DAY)?.parse::<u32>().ok())
        {
            self.day = day;
            self.open = false;
        } else if path_has(node, WHEN) {
            self.open = !self.open;
        } else {
            return false;
        }
        true
    }

    fn dismissed(&mut self, ids: &[String]) {
        if dismisses(ids, CALENDAR) {
            self.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DatePicker, WHEN};
    use crate::page::Page;
    use crate::page::common::find;
    use gorgon_petra::input::{InputEvent, Modifiers, PointerButton};

    fn press() -> InputEvent {
        InputEvent::PointerPressed {
            pos: gorgon_petra::geom::Point::new(0.0, 0.0),
            button: PointerButton::Primary,
            modifiers: Modifiers::NONE,
        }
    }

    /// The field opens the calendar, a day closes it and becomes the
    /// value, and a press outside the calendar closes it through the
    /// dismissal path.
    #[test]
    fn the_field_opens_a_day_picks_and_an_outside_press_dismisses() {
        let mut page = DatePicker::default();
        let value = |page: &DatePicker| {
            find(&page.body(), "value")
                .unwrap()
                .props
                .text
                .clone()
                .unwrap()
        };
        assert!(find(&page.body(), "calendar").is_none());
        assert!(page.handle(&press(), "/page/dp/when/value"));
        assert!(
            find(&page.body(), "calendar").is_some(),
            "the field opened it"
        );
        assert!(page.handle(&press(), "/page/dp/when/calendar/content/days/day-12/label"));
        assert!(find(&page.body(), "calendar").is_none(), "a day closed it");
        assert_eq!(value(&page), "2026-08-12");

        assert!(page.handle(&press(), &format!("/page/dp/{WHEN}")));
        assert!(find(&page.body(), "calendar").is_some());
        page.dismissed(&["/page/dp/when/calendar".to_owned()]);
        assert!(find(&page.body(), "calendar").is_none(), "dismissed");
        page.dismissed(&["/page/other/menu".to_owned()]);
        assert!(find(&page.body(), "calendar").is_none());
    }
}
