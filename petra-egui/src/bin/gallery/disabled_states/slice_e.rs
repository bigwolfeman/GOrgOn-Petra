//! Slice E's disabled states: manifest rows 431 (Select), 445 (Slider), 461
//! (Structured list), 478 (Tabs), 506 (Tag), 521 (Text input), 537 (Tile).

use super::{assert_disabled_state, cells};

#[test]
fn select_disabled_applies_the_treatment() {
    let cells = cells("Select");
    assert_eq!(cells.len(), 1, "Select exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Select", cell);
    }
}

#[test]
fn slider_disabled_applies_the_treatment() {
    let cells = cells("Slider");
    assert_eq!(cells.len(), 1, "Slider exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Slider", cell);
    }
}

#[test]
fn structured_list_disabled_applies_the_treatment() {
    let cells = cells("Structured list");
    assert_eq!(cells.len(), 1, "Structured list exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Structured list", cell);
    }
}

#[test]
fn tabs_disabled_applies_the_treatment() {
    let cells = cells("Tabs");
    assert_eq!(cells.len(), 1, "Tabs exhibit one disabled cell");
    for cell in &cells {
        assert_disabled_state("Tabs", cell);
    }
}

#[test]
fn tag_disabled_applies_the_treatment() {
    let cells = cells("Tag");
    assert_eq!(
        cells.len(),
        1,
        "Tag exhibits its disabled state as one cell — the dismissible and \
         the selectable tag inside it"
    );
    for cell in &cells {
        assert_disabled_state("Tag", cell);
    }
}

#[test]
fn text_input_disabled_applies_the_treatment() {
    let cells = cells("Text input");
    assert_eq!(
        cells.len(),
        1,
        "Text input exhibits one disabled cell — a valued field, so the \
         value text is the label under test"
    );
    for cell in &cells {
        assert_disabled_state("Text input", cell);
    }
}

#[test]
fn tile_disabled_applies_the_treatment() {
    let cells = cells("Tile");
    assert_eq!(
        cells.len(),
        1,
        "Tile exhibits its disabled state as one cell — the clickable and \
         the selectable tile inside it"
    );
    for cell in &cells {
        assert_disabled_state("Tile", cell);
    }
}
