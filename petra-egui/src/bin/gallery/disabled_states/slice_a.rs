//! Slice A's disabled states: manifest rows 11 (Accordion), 63 (Button), 74
//! (Checkbox), 87 (Code snippet), 107 (Contained list).
//!
//! Each test asserts the treatment on every cell the page exhibits, pins
//! how many there are (a state silently losing its gallery cell is a T025
//! gap), and names the component in every failure.

use super::{assert_disabled_state, cells};

#[test]
fn accordion_disabled_applies_the_treatment() {
    let cells = cells("Accordion");
    assert_eq!(cells.len(), 1, "Accordion exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Accordion", cell);
    }
}

#[test]
fn button_disabled_applies_the_treatment() {
    let cells = cells("Button");
    assert_eq!(
        cells.len(),
        1,
        "Button exhibits its disabled state as one cell — the filled pair \
         and the tertiary inside it"
    );
    for cell in &cells {
        assert_disabled_state("Button", cell);
    }
}

#[test]
fn checkbox_disabled_applies_the_treatment() {
    let cells = cells("Checkbox");
    assert_eq!(
        cells.len(),
        1,
        "Checkbox exhibits its disabled state as one cell — unchecked and \
         checked inside it"
    );
    for cell in &cells {
        assert_disabled_state("Checkbox", cell);
    }
}

#[test]
fn code_snippet_disabled_applies_the_treatment() {
    let cells = cells("Code snippet");
    assert_eq!(cells.len(), 1, "Code snippet exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Code snippet", cell);
    }
}

#[test]
fn contained_list_disabled_applies_the_treatment() {
    let cells = cells("Contained list");
    assert_eq!(cells.len(), 1, "Contained list exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Contained list", cell);
    }
}
