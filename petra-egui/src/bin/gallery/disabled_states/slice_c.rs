//! Slice C's disabled states: manifest rows 240 (Link), 273 (Menu), 288
//! (Menu buttons), 321 (Notification).

use super::{assert_disabled_state, cells};

#[test]
fn link_disabled_applies_the_treatment() {
    let cells = cells("Link");
    assert_eq!(cells.len(), 1, "Link exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Link", cell);
    }
}

#[test]
fn menu_disabled_applies_the_treatment() {
    let cells = cells("Menu");
    assert_eq!(
        cells.len(),
        1,
        "Menu exhibits its disabled state on an item with its own menu"
    );
    for cell in &cells {
        assert_disabled_state("Menu", cell);
    }
}

#[test]
fn menu_buttons_disabled_applies_the_treatment() {
    let cells = cells("Menu buttons");
    assert_eq!(cells.len(), 1, "Menu buttons exhibit one disabled cell");
    for cell in &cells {
        assert_disabled_state("Menu buttons", cell);
    }
}

#[test]
fn notification_disabled_applies_the_treatment() {
    let cells = cells("Notification");
    assert_eq!(
        cells.len(),
        1,
        "Notification exhibits its disabled state on the actionable kind"
    );
    for cell in &cells {
        assert_disabled_state("Notification", cell);
    }
}
