//! Spec 008 US2: the Pagination nav **variant**, driven (T043–T048).
//!
//! One question home, the seam [`Pagination::body`] is
//! (`page/pagination.rs`): the variant cell is on the gallery's Pagination
//! page beside the two bars, and this module drives it and asserts what
//! came out — so the page, the drivers and the picture cannot disagree
//! about the variant's anatomy.
//!
//! The bar variant's own driving test is
//! `shots.rs::the_pagination_bar_pages_forward_and_its_pickers_pick`; this
//! module is the variant's peer of it (T047).
//!
//! # What has teeth here
//!
//! * **T046, keyboard as a peer of the pointer** — every control is
//!   reached twice, once by `click` and once by `focus` + an activation
//!   keystroke, and both routes must land the same observable state.
//!   A handler arm that only fires for one route fails its half.
//! * **T044, the selection channel** — the selected page carries the
//!   placed 4-unit `$border-interactive` band on its own bottom edge
//!   (`cam.rect`), declares `Semantics::selected` (`cam.selected`), and
//!   its resting fill is the *same token* as an unselected page's
//!   (`cam.token`): the distinction is geometry plus semantics, never a
//!   fill. Delete the border and the rect assertion fails; move it off
//!   the bottom edge and the placement assertion fails; swap the channel
//!   for a selected fill and the token equality fails.
//! * **T045, the step inventory** — the three built steps are cells on
//!   this page and their rows' placed heights are 32 / 40 / 48.
//!
//! Greyscale is read by eye from `PETRA_SHOT_DIR`'s PNGs (T048); the
//! channel tests above are what make that reading a foregone conclusion
//! rather than the only evidence.

use gorgon_petra::input::KeyCode;

use super::shots::Camera;

/// Where the variant's controls live, as a `Camera` tail prefix. The row
/// is the fixed-height grid inside the container; `numbers` is the
/// page-number row inside it.
const NAV: &str = "nav-variant/row";
const NUMS: &str = "nav-variant/row/numbers";

/// The page the variant currently selects. Only placed buttons are
/// asked: page 5 of 20 places num-1/4/5/6/20 and the two overflow
/// buttons, and `Camera::selected` resolves ids the hard way (it panics
/// on an unplaced tail, listing every id) — scanning all 20 would turn
/// every collapsed run into a probe failure.
fn selected_page(cam: &Camera) -> u32 {
    (1..=20)
        .filter(|n| cam.has(&format!("{NUMS}/num-{n}")))
        .find(|n| cam.selected(&format!("{NUMS}/num-{n}")))
        .expect("no placed page button declares Semantics::selected")
}

/// T047: the variant drives — page buttons, Previous and Next move the
/// selected page, and the bar's driving test does the same for the other
/// variant on the same page.
#[test]
fn the_variant_drives_by_pointer() {
    let mut cam = Camera::on("Pagination");
    assert!(cam.has(&format!("{NUMS}/num-5")), "no page-number row");
    assert_eq!(selected_page(&cam), 5, "page 5 is not the resting page");

    // num-6 is one of the seven nodes page 5 of 20 actually places
    // (num-1, num-4, num-5, num-6, num-20 and the two overflow buttons):
    // a probe names a placed id or it panics listing every one of them.
    cam.click(&format!("{NUMS}/num-6"));
    assert_eq!(
        selected_page(&cam),
        6,
        "a page button did not move selection"
    );
    // The band follows the drive. Page 5's window places num-5 and num-6
    // both, so a band left behind on the wrong row is caught here the way
    // CLAUDE.md describes: the frame record is green, the pixels are not.
    assert!(
        cam.has(&format!("{NUMS}/num-6/indicator")),
        "the selection band did not follow the drive to page 6"
    );
    assert!(
        !cam.has(&format!("{NUMS}/num-5/indicator")),
        "the selection band stayed on page 5 after page 6 selected"
    );
    cam.click(&format!("{NAV}/previous"));
    assert_eq!(selected_page(&cam), 5, "Previous did not step back");
    cam.click(&format!("{NAV}/next"));
    assert_eq!(selected_page(&cam), 6, "Next did not step forward");
    cam.click(&format!("{NUMS}/num-1"));
    assert_eq!(selected_page(&cam), 1, "page 1 did not select");
    // And the band lands on the driven page's own bottom edge — the same
    // geometry T044 pins at rest, now proved after a drive.
    let button = cam.rect(&format!("{NUMS}/num-1"));
    let band = cam.rect(&format!("{NUMS}/num-1/indicator"));
    assert!(
        (band.y + band.h - (button.y + button.h)).abs() < 1.0
            && (band.x - button.x).abs() < 1.0
            && (band.w - button.w).abs() < 1.0,
        "after driving, the band {band:?} is not on page 1's bottom edge {button:?}"
    );
    assert!(
        !cam.has(&format!("{NUMS}/num-20/indicator")),
        "an unselected page kept the band"
    );
    cam.shoot("23-pagination-nav-driven");
}

/// T046: every action the pointer can take, the keyboard takes too — and
/// lands the same state. Two routes, one handler arm.
#[test]
fn the_keyboard_is_a_peer_of_the_pointer() {
    let mut cam = Camera::on("Pagination");

    // Every target below is placed in every window state this drive can
    // land in (the first page, the last page, the pair of nav buttons):
    // the test moves the selection, so a probe pinned to one window's
    // neighbours would break on its own first step.

    // Page jump by pointer, then the same arm from the keyboard.
    cam.click(&format!("{NUMS}/num-6"));
    assert_eq!(
        selected_page(&cam),
        6,
        "pointer did not select a page button"
    );
    cam.focus(&format!("{NUMS}/num-20"));
    cam.key(KeyCode::Enter);
    assert_eq!(
        selected_page(&cam),
        20,
        "Enter on a focused page button did not select it"
    );
    cam.focus(&format!("{NUMS}/num-1"));
    cam.key(KeyCode::Space);
    assert_eq!(
        selected_page(&cam),
        1,
        "Space on a focused page button did not select it"
    );

    // Stepping: keyboard Next, pointer Next, keyboard Previous, pointer
    // Previous — both routes must land the same pages.
    cam.focus(&format!("{NAV}/next"));
    cam.key(KeyCode::Enter);
    assert_eq!(selected_page(&cam), 2, "keyboard Next did not step forward");
    cam.click(&format!("{NAV}/next"));
    assert_eq!(selected_page(&cam), 3, "pointer Next did not step forward");
    cam.focus(&format!("{NAV}/previous"));
    cam.key(KeyCode::Enter);
    assert_eq!(
        selected_page(&cam),
        2,
        "keyboard Previous did not step back"
    );
    cam.click(&format!("{NAV}/previous"));
    assert_eq!(selected_page(&cam), 1, "pointer Previous did not step back");

    // The unavailable end is unavailable to both routes: at page 1 a
    // press on Previous leaves the selection put. (That the keyboard
    // cannot reach it at all is `the_unavailable_end_is_not_even_focusable`
    // below.)
    cam.click(&format!("{NAV}/previous"));
    assert_eq!(selected_page(&cam), 1, "Previous at page 1 moved");
}

/// The unavailable end is not a keyboard target: at page 1 the host
/// refuses to focus Previous at all (`Focus refused: NotFocusable(...)`),
/// so no keystroke can reach it. The refusal is the assertion — a test
/// that merely skipped the button would pass on a wrongly-enabled end too.
#[test]
#[should_panic(expected = "NotFocusable")]
fn the_unavailable_end_is_not_even_focusable() {
    let mut cam = Camera::on("Pagination");
    cam.click(&format!("{NUMS}/num-1"));
    cam.focus(&format!("{NAV}/previous"));
}

/// T046: the keyboard walks the variant's controls in row order and stops
/// on each exactly once — full operation, not one lucky button.
#[test]
fn tab_walks_every_variant_control_once() {
    let mut cam = Camera::on("Pagination");
    cam.focus(&format!("{NAV}/previous"));
    let expected = [
        "previous",
        "num-1",
        "overflow-start",
        "num-4",
        "num-5",
        "num-6",
        "overflow-end",
        "num-20",
        "next",
    ];
    let mut walked = Vec::new();
    for _ in 0..expected.len() {
        let id = cam
            .focused()
            .unwrap_or_else(|| panic!("focus is nowhere after {} stops", walked.len()));
        walked.push(
            id.rsplit('/')
                .next()
                .expect("a placement id has a last segment")
                .to_owned(),
        );
        if walked.len() < expected.len() {
            cam.key(KeyCode::Tab);
        }
    }
    assert_eq!(
        walked, expected,
        "Tab did not walk the variant's controls in row order"
    );
}

/// FR-028's affordance is a button that lists the run it collapsed —
/// and a press on one of its pages selects it. The list's rows sit under
/// `list_box`'s `content` stack, so the probe names the id the layout
/// actually places (`menu/content/page-{n}`).
#[test]
fn the_overflow_affordance_lists_the_hidden_pages() {
    let mut cam = Camera::on("Pagination");
    let trigger = format!("{NUMS}/overflow-start-cell/overflow-start");
    assert!(!cam.has("menu"), "the variant mounts a list at rest");

    cam.click(&trigger);
    cam.shoot("23-pagination-nav-overflow-open");
    assert!(cam.has("menu"), "the overflow button opened no list");
    for hidden in [2, 3] {
        assert!(
            cam.has(&format!("overflow-start-cell/menu/content/page-{hidden}")),
            "hidden page {hidden} is missing from the overflow list"
        );
    }
    assert!(
        !cam.has("overflow-start-cell/menu/content/page-5"),
        "a visible page is listed in the overflow list"
    );
    cam.click("overflow-start-cell/menu/content/page-2");
    assert_eq!(selected_page(&cam), 2, "a hidden page did not select");
    assert!(!cam.has("menu"), "choosing a page left the list open");
}

/// The same open-and-choose by keyboard, from a fresh resting page (page
/// 2 has no leading run to collapse, so the pointer half above cannot
/// come back here and this route is proved from page 5 like the first):
/// the trigger toggles on Enter as on release, and a focused row takes
/// the same select arm a press does.
#[test]
fn the_overflow_affordance_opens_by_keyboard() {
    let mut cam = Camera::on("Pagination");
    let trigger = format!("{NUMS}/overflow-start-cell/overflow-start");
    cam.focus(&trigger);
    cam.key(KeyCode::Enter);
    assert!(
        cam.has("menu"),
        "Enter on the overflow button opened no list"
    );
    assert!(
        cam.has("overflow-start-cell/menu/content/page-3"),
        "the keyboard-opened list lost its hidden pages"
    );
    cam.focus("overflow-start-cell/menu/content/page-3");
    cam.key(KeyCode::Enter);
    assert_eq!(
        selected_page(&cam),
        3,
        "keyboard-choosing a hidden page did not select it"
    );
    assert!(!cam.has("menu"));
}

/// T044: the selected page is the bordered one — a 4-unit band on the
/// button's own bottom edge, `Semantics::selected`, and the same resting
/// fill as every other page button. The fill equality is the assertion
/// with teeth against a colour-only treatment.
#[test]
fn the_selected_page_is_geometry_and_semantics_not_a_fill() {
    let cam = Camera::on("Pagination");
    let selected = format!("{NUMS}/num-5");
    let plain = format!("{NUMS}/num-4");
    assert!(cam.selected(&selected), "page 5 is not declared selected");
    assert!(!cam.selected(&plain), "page 4 is declared selected too");
    assert_eq!(
        cam.token(&selected, "background"),
        cam.token(&plain, "background"),
        "the selected page wears a fill the others do not — that is the bar's \
         channel, and it vanishes in greyscale"
    );

    let button = cam.rect(&selected);
    let band = cam.rect(&format!("{selected}/indicator"));
    assert!(
        (band.h - 4.0).abs() < 0.5,
        "the selected page's border is {} tall, not 4",
        band.h
    );
    assert!(
        (band.y + band.h - (button.y + button.h)).abs() < 1.0,
        "the border {band:?} is not on the button's bottom edge {button:?}"
    );
    assert!(
        (band.x - button.x).abs() < 1.0 && (band.w - button.w).abs() < 1.0,
        "the border {band:?} does not span the button {button:?}"
    );
}

/// T045: the variant's three built steps are on this page, at 32 / 40 /
/// 48. No step is missing, so no step carries an exclusion. The step is
/// the row's height (the same node `pagination_nav.rs`'s ramp pin
/// measures): the variant's root is `rule` + `row`, so the root is the
/// step plus a hairline and probing it would measure the wrong thing.
#[test]
fn the_variant_builds_at_three_steps_on_the_page() {
    let cam = Camera::on("Pagination");
    for (tail, want) in [
        ("size-navvar-sm-cell/row", 32.0),
        ("size-navvar-md-cell/row", 40.0),
        ("size-navvar-lg-cell/row", 48.0),
    ] {
        assert!(cam.has(tail), "no size cell for {tail}");
        let row = cam.rect(tail);
        assert!(
            (row.h - want).abs() < 1.0,
            "{tail} is {} tall, not {want}",
            row.h
        );
    }
}
