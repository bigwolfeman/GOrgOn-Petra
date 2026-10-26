//! Spec 008 US4: the warning treatment, read back out of the built gallery
//! cells (T039, T041, T042).
//!
//! One question home, the seam [`Catalog::warning_cells`] is
//! (`catalog.rs`): `page_body` places those cells, `lua_parity` seeds its
//! mirror from the same call, the shots photograph them, and this module
//! asserts the treatment on them — so a page, a mirror, a picture and a
//! test cannot disagree about which component has a warning state or what
//! it contains.
//!
//! # What "the treatment" is, and why every assertion below has teeth
//!
//! Three channels (spec 008 FR-020/T039 — the operator is red-green colour
//! blind, and this is the pair most likely to fail him):
//!
//! 1. **the hue** — some node in the cell binds the
//!    [`SUPPORT_WARNING`] token, and no node in it binds
//!    [`SUPPORT_ERROR`] or the focus accent: a warning well must wear the
//!    warning outline and neither the error one nor the focused one;
//! 2. **the word** — a text node reads `Warning: …` in `text.primary`
//!    (never Carbon's asymmetric `$text-error` treatment, whose warning
//!    text stays silent in greyscale);
//! 3. **the glyph** — the helper's `mark` canvas is
//!    [`IconMark::WarningAltFilled`]'s triangle-with-bang, which is what a
//!    greyscale reader distinguishes from error's circle-with-slash.
//!
//! Delete the state dressing and channel 1 fails; rename the helper word
//! and channel 2 fails; swap the glyph back to a filled triangle or the
//! error mark and channel 3 fails. One test per warning state (T041),
//! naming each component's own anatomy.

use gorgon_petra::component::{IconMark, IconTone, icon_toned};
use gorgon_petra::tree::ViewNode;

use crate::catalog::Catalog;

/// The token the warning state dresses a well in (spec 008 FR-021, bound
/// in `component/tokens.rs`).
const SUPPORT_WARNING: &str = "support-warning";
/// The error state's token — a warning cell must never wear it.
const SUPPORT_ERROR: &str = "support-error";
/// The focus accent — a warning cell must never wear it either.
const ACCENT: &str = "border-interactive";
/// The helper word's prefix, as [`gorgon_petra::component`]'s shared
/// state helper writes it.
const WORD: &str = "Warning: ";

/// Every form-family row whose slice documents a warning state: spec 008
/// SC-005's six, plus Textarea — the seventh constructor pair FR-019 gives
/// the same treatment. The count is asserted by
/// [`every_form_family_row_exhibits_exactly_one_warning_cell`].
const ROWS: [&str; 7] = [
    "Date picker",
    "Dropdown",
    "Number input",
    "Select",
    "Slider",
    "Text input",
    "Textarea",
];

/// The page's built warning cell(s), one per state it exhibits.
///
/// # Panics
/// If the page exhibits none — a slice row with no gallery cell is a T041
/// gap, and drawing nothing in its place would hide it.
fn cells(component: &str) -> Vec<ViewNode> {
    let cells = Catalog::on_page(component).warning_cells();
    assert!(
        !cells.is_empty(),
        "{component}: the page exhibits no warning cell, so the state the \
         slice documents has no gallery cell (T041)"
    );
    cells
}

/// Assert all three channels on `root` and everything under it — the whole
/// cell is the warning state.
///
/// # Panics
/// If any channel is missing, naming the channel and the component.
fn assert_warning_state(what: &str, root: &ViewNode) {
    assert!(
        binds(root, SUPPORT_WARNING),
        "{what}: no node in the cell binds {SUPPORT_WARNING}, so the state \
         carries no hue at all"
    );
    assert!(
        !binds(root, SUPPORT_ERROR),
        "{what}: the cell wears {SUPPORT_ERROR}: a warning must not be \
         drawn with the error hue (FR-020)"
    );
    assert!(
        !binds(root, ACCENT),
        "{what}: the cell wears the focus accent: a focused well does too, \
         and the two states became indistinguishable"
    );
    let mut messages = 0usize;
    assert_word_and_glyph(what, root, &mut messages);
    assert!(
        messages > 0,
        "{what}: the cell draws no helper row, so the word and glyph \
         assertions measured nothing"
    );
}

/// The word and the glyph, on the shared helper row's two children.
fn assert_word_and_glyph(what: &str, node: &ViewNode, messages: &mut usize) {
    if node.key.as_str() == "message" {
        let text = node
            .props
            .text
            .as_deref()
            .unwrap_or_else(|| panic!("{what}: the helper's message node draws no text"));
        assert!(
            text.starts_with(WORD),
            "{what}: the message reads {text:?}, not a {WORD:?} word — in \
             greyscale the word is the channel the operator has (FR-020)"
        );
        assert_eq!(
            node.props.tokens.get("foreground").map(|t| t.as_str()),
            Some("text.primary"),
            "{what}: warning text must stay in `text.primary`. Carbon \
             leaves warning text unlabelled while error text takes \
             `$text-error`; a tint here would trade a channel the \
             operator has for one he does not (T039)"
        );
        *messages += 1;
    }
    if node.key.as_str() == "mark" {
        let expected = icon_toned("mark", IconMark::WarningAltFilled, IconTone::Primary);
        assert_eq!(
            node.props.canvas, expected.props.canvas,
            "{what}: the helper's glyph is not WarningAltFilled's \
             triangle-with-bang — the silhouette is what a greyscale \
             reader distinguishes from error's circle-with-slash (T039)"
        );
    }
    for child in &node.children {
        assert_word_and_glyph(what, child, messages);
    }
}

/// Whether any node in the subtree binds `token` in any slot.
fn binds(node: &ViewNode, token: &str) -> bool {
    node.props.tokens.values().any(|t| t.as_str() == token)
        || node.children.iter().any(|c| binds(c, token))
}

/// SC-005 verbatim: "at least six form-family components enter a warning
/// state". Every row of [`ROWS`] exhibits exactly one warning cell, and
/// the family ships seven — quoted as the count the criterion asks for.
#[test]
fn every_form_family_row_exhibits_exactly_one_warning_cell() {
    for row in ROWS {
        assert_eq!(
            cells(row).len(),
            1,
            "{row} must exhibit exactly one warning cell"
        );
    }
    assert!(
        ROWS.len() >= 6,
        "SC-005 asks for at least six form-family warning states; the \
         family carries {}",
        ROWS.len()
    );
}

#[test]
fn date_picker_warning_applies_the_treatment() {
    for cell in &cells("Date picker") {
        assert_warning_state("Date picker", cell);
    }
}

#[test]
fn dropdown_warning_applies_the_treatment() {
    for cell in &cells("Dropdown") {
        assert_warning_state("Dropdown", cell);
    }
}

#[test]
fn number_input_warning_applies_the_treatment() {
    for cell in &cells("Number input") {
        assert_warning_state("Number input", cell);
    }
}

#[test]
fn select_warning_applies_the_treatment() {
    for cell in &cells("Select") {
        assert_warning_state("Select", cell);
    }
}

#[test]
fn slider_warning_applies_the_treatment() {
    for cell in &cells("Slider") {
        assert_warning_state("Slider", cell);
    }
}

#[test]
fn text_input_warning_applies_the_treatment() {
    for cell in &cells("Text input") {
        assert_warning_state("Text input", cell);
    }
}

#[test]
fn textarea_warning_applies_the_treatment() {
    for cell in &cells("Textarea") {
        assert_warning_state("Textarea", cell);
    }
}
