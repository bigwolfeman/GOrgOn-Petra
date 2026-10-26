//! Carbon **Pagination nav** (slice-d), the second Pagination *variant*
//! (spec 008 T043, FR-028) — a distinct anatomy in its own file, the way
//! Carbon documents it on the same page as the bar but shares almost none
//! of its anatomy: "two variants documented on the same page: **Pagination**
//! (bar attached to a data table) and **Pagination nav** (freestanding
//! page-number buttons for on-page pagination). They share no anatomy
//! beyond 'previous/next button' and 'container'" (slice-d:52).
//!
//! Anatomy (slice-d:59-63, "Anatomy of pagination nav"):
//! 1. Container — the same treatment the bar's container gets, because the
//!    slice says the two variants share it: [`SURFACE_RAISED`] (`$layer`)
//!    fill over a 1-unit `border-block-start` rule ([`BORDER_SUBTLE`]),
//!    `padding-inline: 16px` ([`SPACING_05`]), the row inside pinned to
//!    the size step.
//! 2. Previous button — the shared ghost caret piece
//!    ([`super::pagination::pagination_prev_next`]'s [`nav_button`]).
//! 3. The page-number row — a compact window of page buttons (the same
//!    window derivation the bar's number row uses) with, where a run of
//!    pages collapsed, an **overflow button**: an ellipsis (the crate's
//!    `U+2026` mark, the one the bar draws a collapsed run with) that is a
//!    [`Role::Button`] and, while open, lists the pages it stands in for
//!    in a [`list_box`]. Carbon imports `OverflowMenuHorizontal` for this
//!    button (slice-d "Icons"); the crate's ellipsis mark stands in —
//!    "Icons" bullets are outside the variant manifest's mining scope, and
//!    the substitution is recorded in the note below. This is the
//!    affordance the bar's text-only `ellipsis_mark` deliberately is not.
//! 4. Next button — the same shared piece.
//!
//! The selected page (T044) is Carbon's own treatment and it already is
//! two channels: "a page button showing the current page — 4px bottom
//! border in `$border-interactive`" (slice-d:54). The border's **hue** is
//! one channel; its **geometry** — four units on one edge of one button —
//! is the other, and it reads in greyscale. That is
//! [`tabs::indicator_bar`]'s construction (the same `border-block-end`
//! rule Carbon writes on a selected tab), a fixed 4-unit row pinned to the
//! button's bottom, in [`ACCENT_PRIMARY`] (`$border-interactive`). Plus
//! [`Semantics::selected`] for the tree. What the selected page does
//! *not* get is the bar number row's `background@selected` fill and weight
//! step — those are the bar's anatomy (spec 009 T022), not this variant's.
//!
//! Sizes (slice-d "Sizes"): the shared ramp, sm 32 / md 40 / lg 48 —
//! "there is no xs/xl pagination size". [`PageBarSize`] is the same step
//! enum the bar pins to.
//!
//! Note: `.agents/notes/implemented/spec/2026-10-25-pagination-nav-variant.md`.

use super::dropdown::dropdown_option;
use super::icon::IconMark;
use super::list_box::{Dividers, list_box};
use super::pagination::{
    ACTIVATES_ON_RELEASE, NAV_INTENTS, NumberSlot, PageBarSize, TOGGLES_ON_RELEASE, nav_button,
    number_slots, pad_inline, picker_cell, pin_button, run,
};
use super::stack;
use super::tabs::indicator_bar;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, LAYER_HOVER, SPACING_05, SURFACE_RAISED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{Justify, Key, NodeKind, Props, Role, TrackSize, ViewNode};

/// Carbon's selected-page border: "Pagination nav selected-page border:
/// 4px" (slice-d:69). The non-colour half of the selection signal.
const SELECTED_BORDER: f32 = 4.0;

/// One ellipsis character, not three dots — the mark the bar draws a
/// collapsed run with, and what Carbon's overflow button reads as.
const ELLIPSIS: &str = "\u{2026}";

/// Accessible name of the overflow button and of the list it opens.
const OVERFLOW_LABEL: &str = "Show hidden pages";

/// Which of the variant's two collapsed runs is showing its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationNavOverflow {
    /// The gap after page 1 — the `…` in `1 … 4 5 6 … 20`.
    Start,
    /// The gap before the last page — the trailing `…` in the same window.
    End,
}

/// Carbon's Pagination nav variant at md (40): Previous, the page-number
/// row with its selected page and overflow buttons, Next. `page` is
/// 1-indexed; `page_count` is the last page number (0 renders the two
/// unavailable nav buttons and no row).
#[must_use]
pub fn pagination_nav(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    pagination_nav_at(key, page, page_count, PageBarSize::Md, None)
}

/// The variant at Carbon sm (32).
#[must_use]
pub fn pagination_nav_sm(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    pagination_nav_at(key, page, page_count, PageBarSize::Sm, None)
}

/// The variant at Carbon lg (48).
#[must_use]
pub fn pagination_nav_lg(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    pagination_nav_at(key, page, page_count, PageBarSize::Lg, None)
}

/// [`pagination_nav`] with `overflow`'s list mounted under its button: the
/// pages that run collapsed, as [`dropdown_option`] rows keyed `page-{n}`,
/// the same rows the bar's page picker lists, so one press-handling arm
/// drives both. A press on a listed row is the caller's to handle.
#[must_use]
pub fn pagination_nav_open(
    key: impl Into<Key>,
    page: u32,
    page_count: u32,
    overflow: PaginationNavOverflow,
) -> ViewNode {
    pagination_nav_at(key, page, page_count, PageBarSize::Md, Some(overflow))
}

fn pagination_nav_at(
    key: impl Into<Key>,
    page: u32,
    page_count: u32,
    size: PageBarSize,
    open: Option<PaginationNavOverflow>,
) -> ViewNode {
    let height = size.height();
    let page = page.clamp(1, page_count.max(1));

    // The page-number row: the bar's window derivation, but a collapsed
    // run is the overflow button here rather than a text mark. With no
    // pages there is no row at all — a 0-wide FitContent cell petrifies
    // as a degenerate rect, the reason the bar omits its column too.
    let mut row_children = Vec::new();
    if page_count > 0 {
        let mut slots = Vec::new();
        for slot in number_slots(page, page_count) {
            match slot {
                NumberSlot::Page(n) => slots.push(page_button(n, n == page, height)),
                NumberSlot::Gap {
                    first_hidden,
                    last_hidden,
                } => slots.push(overflow_cell(first_hidden, last_hidden, page, open, height)),
            }
        }
        row_children.push(run("numbers", slots, height));
    }

    let previous = nav_button(
        "previous",
        "Previous",
        IconMark::CaretLeft,
        page <= 1,
        height,
    );
    row_children.insert(0, previous);
    row_children.push(nav_button(
        "next",
        "Next",
        IconMark::CaretRight,
        page_count == 0 || page >= page_count,
        height,
    ));

    // The row carries the container's `padding-inline: 16px` (slice-d:67)
    // so the rule above it still runs the container's full width. The
    // row's own key is fixed (`bar` does the same with its grid): the
    // outer node takes the caller's key alone.
    let mut row = run("row", row_children, height);
    row.props.padding = Some(pad_inline(SPACING_05, SPACING_05));

    let mut rule = super::rule("rule", Axis::Horizontal, BORDER_SUBTLE);
    rule.props.align_self = Some(Align::Stretch);

    let mut node = stack(key, Axis::Vertical, None, vec![rule, row]);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node
}

/// One page button. Unselected is the number in [`TEXT_PRIMARY`] on the
/// container's own fill. Selected adds Carbon's 4-unit
/// `$border-interactive` border along the bottom edge — see the module
/// doc; the button's key is `num-{n}`, so a press routes to the same
/// handler arm the bar's number row does.
fn page_button(n: u32, selected: bool, height: f32) -> ViewNode {
    let label = format!("Page {n}");
    let mut caption = text("label", n.to_string());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));

    let mut node = if selected {
        let mut body = stack("body", Axis::Horizontal, None, vec![caption]);
        body.props.align = Some(Align::Center);
        body.props.justify = Some(Justify::Center);
        ViewNode::new(NodeKind::Grid, format!("num-{n}"))
            .with_props(Props {
                columns: vec![TrackSize::Weight { weight: 1.0 }],
                rows: vec![
                    TrackSize::Weight { weight: 1.0 },
                    TrackSize::Fixed {
                        value: SELECTED_BORDER,
                    },
                ],
                align: Some(Align::Stretch),
                ..Props::default()
            })
            .with_children(vec![
                body,
                indicator_bar(Axis::Horizontal, SELECTED_BORDER, Some(ACCENT_PRIMARY)),
            ])
    } else {
        let mut node = stack(format!("num-{n}"), Axis::Horizontal, None, vec![caption]);
        node.props.align = Some(Align::Center);
        node.props.justify = Some(Justify::Center);
        node.props.padding = Some(pad_inline(SPACING_05, SPACING_05));
        node
    };
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node = node
        .with_constraints(pin_button(height))
        .interactive(Role::Button, label, NAV_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text();
    node.semantics.selected = selected;
    node.semantics.value = Some(n.to_string());
    node
}

/// One collapsed run: the overflow button, and while `open`, its list.
/// The button is keyed `overflow-start`/`overflow-end` (the gap from page 2
/// is the leading collapse; anything else the trailing one — the same rule
/// the bar's `ellipsis-start`/`ellipsis-end` keys follow).
fn overflow_cell(
    first_hidden: u32,
    last_hidden: u32,
    page: u32,
    open: Option<PaginationNavOverflow>,
    height: f32,
) -> ViewNode {
    let which = if first_hidden == 2 {
        PaginationNavOverflow::Start
    } else {
        PaginationNavOverflow::End
    };
    let key = overflow_key(which);
    let mut mark = text("dots", ELLIPSIS);
    mark.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut trigger = stack(key, Axis::Horizontal, None, vec![mark]);
    trigger.props.align = Some(Align::Center);
    trigger.props.justify = Some(Justify::Center);
    trigger
        .props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    trigger
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut trigger = trigger
        .with_constraints(pin_button(height))
        .interactive(Role::Button, OVERFLOW_LABEL, NAV_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text();
    trigger.semantics.expanded = Some(open == Some(which));
    trigger.semantics.value = Some(format!("{first_hidden}\u{2013}{last_hidden}"));

    let menu = (open == Some(which)).then(|| {
        list_box(
            "menu",
            OVERFLOW_LABEL,
            key,
            (first_hidden..=last_hidden)
                .map(|n| dropdown_option(format!("page-{n}"), n.to_string(), n == page))
                .collect(),
            Dividers::Between,
        )
    });
    picker_cell(overflow_cell_key(which), trigger, menu)
}

fn overflow_key(which: PaginationNavOverflow) -> &'static str {
    match which {
        PaginationNavOverflow::Start => "overflow-start",
        PaginationNavOverflow::End => "overflow-end",
    }
}

fn overflow_cell_key(which: PaginationNavOverflow) -> &'static str {
    match which {
        PaginationNavOverflow::Start => "overflow-start-cell",
        PaginationNavOverflow::End => "overflow-end-cell",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::tests::petrify_lone_at;
    use crate::frame::PetrifiedFrame;
    use crate::geom::Size;
    use crate::token::TokenName;
    use crate::tree::{Behaviour, Intent, Phase};

    fn named<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key)
    }

    fn frame(node: ViewNode) -> PetrifiedFrame {
        petrify_lone_at(node, Size::new(800.0, 240.0))
    }

    fn placed<'a>(
        frame: &'a PetrifiedFrame,
        suffix: &str,
    ) -> &'a crate::frame::placement::Placement {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("{suffix} is not placed"))
    }

    /// FR-028, the variant half: a page-number row, a selected-page
    /// indicator, and an overflow affordance — and almost none of the
    /// bar's anatomy. No pickers, no range caption, no dividers: the row
    /// is prev | numbers | next.
    #[test]
    fn the_variant_is_the_nav_anatomy_not_the_bar() {
        let node = pagination_nav("nav", 5, 20);
        assert!(named(&node, "previous").is_some(), "no Previous button");
        assert!(named(&node, "next").is_some(), "no Next button");
        assert!(named(&node, "numbers").is_some(), "no page-number row");
        assert!(
            named(&node, "page-size-picker").is_none(),
            "the variant mounts the bar's items-per-page picker"
        );
        assert!(
            named(&node, "page-picker").is_none(),
            "the variant mounts the bar's page picker"
        );
        assert!(
            named(&node, "range-text").is_none(),
            "the variant mounts the bar's range caption"
        );
        assert!(
            named(&node, "divider-page").is_none() && named(&node, "divider-previous").is_none(),
            "the variant draws the bar's divider seams"
        );
    }

    /// T044: the selected page is a 4-unit `$border-interactive` border on
    /// its bottom edge and nothing else — no fill channel, no weight step.
    /// The border's geometry is the channel that survives greyscale, so
    /// this test asserts the geometry (a fixed 4-unit row pinned to the
    /// button's bottom edge) and the semantic flag, and asserts the
    /// *absence* of the bar's fill channel.
    #[test]
    fn the_selected_page_is_a_four_pixel_border_not_a_colour() {
        let node = pagination_nav("nav", 5, 20);
        let selected = named(&node, "num-5").expect("no button for the current page");
        assert!(
            selected.semantics.selected,
            "the current page is not declared selected"
        );
        // The bar's channel, which this variant must not borrow.
        assert!(
            !selected.props.tokens.keys().any(|k| k.contains("selected")),
            "the selected page borrows the bar's selected-fill channel: {:?}",
            selected.props.tokens
        );

        let mark = named(selected, "indicator").expect("the selected page has no border mark");
        assert_eq!(
            (mark.constraints.vertical.min, mark.constraints.vertical.max),
            (Some(SELECTED_BORDER), Some(SELECTED_BORDER)),
            "the border's thickness channel disappeared"
        );
        assert_eq!(
            mark.props.tokens.get("background"),
            Some(&t(ACCENT_PRIMARY)),
            "the border is not in $border-interactive"
        );

        // And an unselected button carries no mark at all: the mark is
        // what says *which* page is current.
        let plain = named(&node, "num-4").expect("no button for page 4");
        assert!(
            named(plain, "indicator").is_none(),
            "an unselected page carries the selected page's border"
        );
    }

    /// The selected button really places its border on its own bottom
    /// edge — a fixed 4-unit band, not a decorative child. This is the
    /// channel the greyscale PNG review reads.
    #[test]
    fn the_selected_border_sits_on_the_buttons_bottom_edge() {
        let frame = frame(pagination_nav("nav", 5, 20));
        let button = placed(&frame, "num-5");
        let mark = placed(&frame, "num-5/indicator");
        assert!(
            (mark.rect.y + mark.rect.h - (button.rect.y + button.rect.h)).abs() < 1.0,
            "the border {mark:?} is not on the button's bottom edge {button:?}"
        );
        assert!(
            (mark.rect.h - SELECTED_BORDER).abs() < 0.5,
            "the placed border is {} tall, not {SELECTED_BORDER}",
            mark.rect.h
        );
    }

    /// FR-028's third anatomy item: a collapsed run is a button, not the
    /// bar's text mark, and it opens a list of exactly the pages it stands
    /// in for.
    #[test]
    fn the_overflow_button_lists_the_hidden_pages() {
        let closed = pagination_nav("nav", 5, 20);
        let trigger = named(&closed, "overflow-start").expect("no leading overflow button");
        assert_eq!(
            trigger.semantics.role,
            Some(Role::Button),
            "the overflow affordance is not a button"
        );
        assert_eq!(
            trigger.behaviour,
            Some(Behaviour {
                intent: Intent::Toggle,
                phase: Phase::OnRelease,
            }),
            "the overflow button does not toggle its list"
        );
        assert_eq!(
            trigger.semantics.expanded,
            Some(false),
            "the closed overflow button does not say so"
        );
        assert_eq!(
            trigger.semantics.value.as_deref(),
            Some("2\u{2013}3"),
            "the overflow button does not name the run it stands in for"
        );
        assert!(
            named(&closed, "menu").is_none(),
            "the closed variant mounts an overflow list"
        );

        let open = pagination_nav_open("nav", 5, 20, PaginationNavOverflow::Start);
        let trigger = named(&open, "overflow-start").expect("no leading overflow button");
        assert_eq!(trigger.semantics.expanded, Some(true));
        let menu = named(&open, "menu").expect("the open overflow button mounts no list");
        for hidden in 2..=3 {
            assert!(
                named(menu, &format!("page-{hidden}")).is_some(),
                "hidden page {hidden} is not in the overflow list"
            );
        }
        for visible in [1, 5, 20] {
            assert!(
                named(menu, &format!("page-{visible}")).is_none(),
                "page {visible} is visible in the row and must not be listed again"
            );
        }
    }

    /// A gap of one page is that page, not an overflow button — the window
    /// rule the bar's number row shares (collapsing one number into a mark
    /// wastes a slot). Here it also keeps a one-page "menu" off the tree.
    #[test]
    fn a_one_page_gap_is_a_page_button_not_an_overflow() {
        // Page 4 of 20: `1 2 3 4 5 … 20` — the leading gap is page 2
        // alone, so it is drawn as a page button. The trailing gap
        // (6..=19) is wide and does become an overflow button.
        let node = pagination_nav("nav", 4, 20);
        assert!(
            named(&node, "overflow-start").is_none(),
            "a one-page collapsed run became an overflow button"
        );
        assert!(
            named(&node, "num-2").is_some(),
            "the one-page run is not shown as a page button"
        );
        assert!(
            named(&node, "overflow-end").is_some(),
            "the wide trailing run did not become an overflow button"
        );
    }

    /// T045: the variant builds at all three documented steps, pinned to
    /// the shared ramp (sm 32 / md 40 / lg 48). Both variants' step
    /// inventory is in the phase note.
    #[test]
    fn the_variant_builds_at_sm_md_and_lg() {
        for (node, step) in [
            (pagination_nav_sm("nav", 2, 5), 32.0),
            (pagination_nav("nav", 2, 5), 40.0),
            (pagination_nav_lg("nav", 2, 5), 48.0),
        ] {
            let frame = frame(node);
            let row = placed(&frame, "nav/row");
            assert!(
                (row.rect.h - step).abs() < 1.0,
                "the variant's row is {} tall at step {step}",
                row.rect.h
            );
            let previous = placed(&frame, "nav/row/previous");
            assert!(
                (previous.rect.h - step).abs() < 1.0,
                "the Previous button is {} tall at step {step}",
                previous.rect.h
            );
        }
    }

    /// Keyboard operation starts with reachability: every control the
    /// variant offers declares Focus and lands in the focus order once.
    /// The gallery's driver tests press them (T046).
    #[test]
    fn every_control_is_in_the_focus_order() {
        let frame = frame(pagination_nav("nav", 5, 20));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        for want in [
            "previous",
            "next",
            "num-5",
            "overflow-start",
            "overflow-end",
        ] {
            let hits = order.iter().filter(|id| id.ends_with(want)).count();
            assert_eq!(
                hits, 1,
                "{want} appears {hits} times in the focus order {order:?}"
            );
        }
    }

    /// The window derivation is shared with the bar: same slots for the
    /// same page, and the gap now carries the range the overflow lists.
    #[test]
    fn the_window_matches_the_bars_derivation() {
        assert_eq!(
            number_slots(5, 20),
            vec![
                NumberSlot::Page(1),
                NumberSlot::Gap {
                    first_hidden: 2,
                    last_hidden: 3
                },
                NumberSlot::Page(4),
                NumberSlot::Page(5),
                NumberSlot::Page(6),
                NumberSlot::Gap {
                    first_hidden: 7,
                    last_hidden: 19
                },
                NumberSlot::Page(20),
            ]
        );
    }

    /// The trigger's ink is the page's, not a hue of its own — the
    /// ellipsis is a control here, not a caption.
    #[test]
    fn the_overflow_mark_is_page_ink() {
        let node = pagination_nav("nav", 5, 20);
        let trigger = named(&node, "overflow-start").unwrap();
        let dots = named(trigger, "dots").unwrap();
        let ink: Option<&TokenName> = dots.props.tokens.get("foreground");
        assert_eq!(ink, Some(&t(TEXT_PRIMARY)));
    }
}
