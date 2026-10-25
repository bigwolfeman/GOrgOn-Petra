//! Slice F's disabled states: manifest rows 547 (Toggle), 571 (Tree view).

use super::{assert_disabled_state, cells};

#[test]
fn toggle_disabled_applies_the_treatment() {
    let cells = cells("Toggle");
    assert_eq!(
        cells.len(),
        1,
        "Toggle exhibits its disabled state as one cell — off and on inside \
         it"
    );
    for cell in &cells {
        assert_disabled_state("Toggle", cell);
    }
}

#[test]
fn tree_view_disabled_applies_the_treatment() {
    let cells = cells("Tree view");
    assert_eq!(cells.len(), 1, "Tree view exhibits one disabled cell");
    for cell in &cells {
        assert_disabled_state("Tree view", cell);
    }
}
