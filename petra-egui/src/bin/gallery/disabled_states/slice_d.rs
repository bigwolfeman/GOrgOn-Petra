//! Slice D's disabled states: manifest rows 340 (Number input), 352
//! (Pagination), 384 (Progress indicator), 397 (Radio button), 413 (Search).

use super::{assert_disabled_state, cells};

#[test]
fn number_input_disabled_applies_the_treatment() {
    let cells = cells("Number input");
    assert_eq!(cells.len(), 1, "Number input exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Number input", cell);
    }
}

#[test]
fn pagination_disabled_applies_the_treatment() {
    let cells = cells("Pagination");
    assert_eq!(
        cells.len(),
        1,
        "Pagination exhibits its disabled state as one cell — both sealed \
         variants inside it"
    );
    for cell in &cells {
        assert_disabled_state("Pagination", cell);
    }
}

#[test]
fn progress_indicator_disabled_applies_the_treatment() {
    let cells = cells("Progress indicator");
    assert_eq!(
        cells.len(),
        1,
        "Progress indicator exhibits one disabled cell"
    );
    for cell in &cells {
        assert_disabled_state("Progress indicator", cell);
    }
}

#[test]
fn radio_button_disabled_applies_the_treatment() {
    let cells = cells("Radio button");
    assert_eq!(
        cells.len(),
        1,
        "Radio button exhibits its disabled state as one cell — unselected \
         and selected inside it"
    );
    for cell in &cells {
        assert_disabled_state("Radio button", cell);
    }
}

#[test]
fn search_disabled_applies_the_treatment() {
    let cells = cells("Search");
    assert_eq!(cells.len(), 1, "Search exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Search", cell);
    }
}
