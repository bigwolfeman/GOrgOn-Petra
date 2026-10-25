//! Slice B's disabled states: manifest rows 122 (Content switcher), 149
//! (Data table), 166 (Date picker), 191 (Dropdown), 207 (File uploader),
//! 221 (Form), and 223 (Form's `disabled-fieldset`).
//!
//! Row 223 is the one disabled state in the manifest that is not a whole
//! component: a disabled field inside a live form. Its case asserts the
//! treatment on that one subtree *and* that the form around it stays live.

use super::{assert_disabled_state, assert_disabled_state_within, cells};

#[test]
fn content_switcher_disabled_applies_the_treatment() {
    let cells = cells("Content switcher");
    assert_eq!(
        cells.len(),
        1,
        "Content switcher exhibits one disabled cell"
    );
    for cell in &cells {
        assert_disabled_state("Content switcher", cell);
    }
}

#[test]
fn data_table_disabled_applies_the_treatment() {
    let cells = cells("Data table");
    assert_eq!(
        cells.len(),
        1,
        "Data table exhibits its disabled state on the batch action bar"
    );
    for cell in &cells {
        assert_disabled_state("Data table", cell);
    }
}

#[test]
fn date_picker_disabled_applies_the_treatment() {
    let cells = cells("Date picker");
    assert_eq!(cells.len(), 1, "Date picker exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Date picker", cell);
    }
}

#[test]
fn dropdown_disabled_applies_the_treatment() {
    let cells = cells("Dropdown");
    assert_eq!(cells.len(), 1, "Dropdown exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Dropdown", cell);
    }
}

#[test]
fn file_uploader_disabled_applies_the_treatment() {
    let cells = cells("File uploader");
    assert_eq!(
        cells.len(),
        1,
        "File uploader exhibits its disabled state as one cell — the drop \
         zone and a file item inside it"
    );
    for cell in &cells {
        assert_disabled_state("File uploader", cell);
    }
}

#[test]
fn form_disabled_applies_the_treatment() {
    let cells = cells("Form");
    assert_eq!(
        cells.len(),
        2,
        "Form exhibits both its rows: the disabled fieldset (223) and the \
         disabled form (221)"
    );
    assert_disabled_state("Form", &cells[1]);
}

#[test]
fn form_disabled_fieldset_applies_the_treatment() {
    let cells = cells("Form");
    assert_eq!(
        cells.len(),
        2,
        "Form exhibits both its rows: the disabled fieldset (223) and the \
         disabled form (221)"
    );
    assert_disabled_state_within("Form fieldset", &cells[0]);
}
