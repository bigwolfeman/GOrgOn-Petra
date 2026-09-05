//! Carbon Pagination (slice-d), the bar variant.
//!
//! Anatomy (usage page + `_pagination.scss` + `23-pagination.png`):
//! 1. Container — `$layer` fill ([`SURFACE_RAISED`]), `border-block-start:
//!    1px solid $border-subtle`, height md 40, as wide as what it sits in.
//!    The top edge is a real 1-unit `rule` element rather than a
//!    `border-top` binding, because the bar's own children fill its whole
//!    height with opaque fills and would paint over a rule the container
//!    drew first — the compositing shape
//!    `.agents/notes/implemented/architecture/2026-09-04-per-edge-border-slots.md`
//!    names as the one case an edge slot does not cover.
//! 2. Left group — "Items per page:" plus the **page-size picker**
//!    ([`picker`]: the value, a chevron, `Semantics.value`), closed by a
//!    `border-inline-end` divider. Built by [`pagination_items`], which
//!    knows the page size; [`pagination`] does not and omits the group
//!    rather than invent one.
//! 3. Range text — "1–10 of 50 items" — filling the middle so the right
//!    group sits at the bar's end. [`pagination_items`] only.
//! 4. Right group — a `border-inline-start` divider, the **page picker**
//!    and "of N pages", then Previous and Next, each behind its own divider
//!    (slice-d:58). They are Carbon's ghost icon buttons: 40×40, a 16
//!    `CaretLeft` / `CaretRight` glyph ([`IconMark`]) centred, labelled
//!    "Previous" / "Next" in [`Semantics`] so the word is still there for a
//!    reader who is not looking. Page 1 disables Previous via
//!    [`super::disabled`] and draws its caret in [`IconTone::Disabled`];
//!    the last page disables Next the same way.
//!
//! Carbon's two pickers are native `<select>`s, whose popup the browser
//! draws. Here a picker is a [`Role::Button`] that opens a
//! [`super::list_box`] under itself: [`pagination_items_open`] mounts the
//! list for whichever picker is open, with the page sizes the caller
//! offers or the pages 1..=N, as [`super::dropdown_option`] rows. The
//! closed form ([`pagination_items`]) needs no option list, so it takes
//! none. Pagination nav (page-number buttons) is a second Carbon variant
//! and is omitted.

use super::disabled;
use super::dropdown::dropdown_option;
use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::list_box;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_RAISED, TEXT_MUTED,
    TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Key, NodeKind, Props, Role, TrackSize,
    ViewNode,
};

const _: () = assert!(SIZE_MD == 40.0);

const NAV_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// `border-inline-start: 1px solid $border-subtle` on each nav button and
/// on the right group, `border-inline-end` on the left group, SOURCED
/// `slice-d.md:57-58`. A real divider element rather than a `border-left`
/// binding on the button: the buttons carry an opaque fill and sit flush
/// in a `Stretch` row, which is the compositing shape an edge slot on a
/// container cannot survive (see the module doc's item 1), and a sibling
/// element draws the same one line either way.
const DIVIDER_WIDTH: f32 = 1.0;
/// The container's `border-block-start`.
const RULE_HEIGHT: f32 = 1.0;

/// The page-size picker's key: the trigger a caller matches a press on,
/// and the sibling its list box anchors to.
const PAGE_SIZE_PICKER: &str = "page-size-picker";
/// The page picker's key; see [`PAGE_SIZE_PICKER`].
const PAGE_PICKER: &str = "page-picker";
/// The open list's key under either picker.
const PICKER_MENU: &str = "menu";
/// Accessible names for the two pickers.
const PAGE_SIZE_LABEL: &str = "Items per page";
const PAGE_LABEL: &str = "Page number";

/// Which of the bar's two pickers is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaginationPicker {
    /// The items-per-page select in the left group.
    PageSize,
    /// The page-number select in the right group.
    Page,
}

/// The open picker and what it lists.
struct Open {
    picker: PaginationPicker,
    page_sizes: Vec<u32>,
}

/// Pagination bar at Carbon md (40) with only the page controls: the
/// right group of Carbon's anatomy, and no items-per-page group, because
/// this constructor is not told a page size and does not invent one.
/// `page` is 1-indexed.
///
/// `page_count` is the last page number. Previous is unavailable on page
/// 1; Next is unavailable on the last page (and when there are no pages).
pub fn pagination(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    bar(key, None, page, page_count, None)
}

/// Carbon's full bar, pickers closed: items per page, the range of items
/// on this page, and the page controls. `page` is 1-indexed; `page_size`
/// is the number of items per page; `total_items` is the whole set.
///
/// The page count is `total_items / page_size` rounded up, and the range
/// text is "first–last of total items" for this page. A `page_size` of 0
/// is treated as 1 rather than dividing by it.
pub fn pagination_items(
    key: impl Into<Key>,
    page: u32,
    page_size: u32,
    total_items: u32,
) -> ViewNode {
    let page_size = page_size.max(1);
    let (page_count, range) = derive(page, page_size, total_items);
    bar(key, Some((page_size, range)), page, page_count, None)
}

/// [`pagination_items`] with `picker` open: its list box hangs under it,
/// listing `page_sizes` (the caller's offer, as Carbon's `pageSizes` prop)
/// or the pages `1..=page_count`. A press on a listed row is the caller's
/// to handle; rows are keyed `size-{n}` and `page-{n}`.
pub fn pagination_items_open(
    key: impl Into<Key>,
    page: u32,
    page_size: u32,
    page_sizes: &[u32],
    total_items: u32,
    picker: PaginationPicker,
) -> ViewNode {
    let page_size = page_size.max(1);
    let (page_count, range) = derive(page, page_size, total_items);
    bar(
        key,
        Some((page_size, range)),
        page,
        page_count,
        Some(Open {
            picker,
            page_sizes: page_sizes.to_vec(),
        }),
    )
}

/// The page count and the range caption for one page of a set.
fn derive(page: u32, page_size: u32, total_items: u32) -> (u32, String) {
    let page_count = total_items.div_ceil(page_size);
    let first = page
        .saturating_sub(1)
        .saturating_mul(page_size)
        .saturating_add(1);
    let last = page.saturating_mul(page_size).min(total_items);
    (
        page_count,
        format!("{first}\u{2013}{last} of {total_items} items"),
    )
}

/// The bar: a 1-unit top rule over a 40-tall grid whose middle column is
/// the `Weight` track that pushes the page controls to the end.
fn bar(
    key: impl Into<Key>,
    left: Option<(u32, String)>,
    page: u32,
    page_count: u32,
    open: Option<Open>,
) -> ViewNode {
    let mut columns = Vec::with_capacity(3);
    let mut cells = Vec::with_capacity(3);
    match left {
        Some((page_size, range)) => {
            let sizes = open
                .as_ref()
                .filter(|o| o.picker == PaginationPicker::PageSize)
                .map(|o| o.page_sizes.as_slice());
            columns.push(TrackSize::FitContent);
            cells.push(items_per_page(page_size, sizes));
            columns.push(TrackSize::Weight { weight: 1.0 });
            cells.push(range_cell(range));
        }
        None => {
            columns.push(TrackSize::Weight { weight: 1.0 });
            cells.push(stack("range", Axis::Horizontal, None, vec![]));
        }
    }
    let pages_open = open
        .as_ref()
        .is_some_and(|o| o.picker == PaginationPicker::Page);
    columns.push(TrackSize::FitContent);
    cells.push(page_controls(page, page_count, pages_open));

    let mut row = ViewNode::new(NodeKind::Grid, "bar")
        .with_props(Props {
            columns,
            rows: vec![TrackSize::FitContent],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(cells);
    row.constraints.vertical = AxisConstraint {
        min: Some(SIZE_MD),
        max: Some(SIZE_MD),
        priority: 0,
    };

    let mut rule = stack("rule", Axis::Horizontal, None, vec![]);
    rule.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    rule.props.align_self = Some(Align::Stretch);
    rule.constraints.vertical = AxisConstraint {
        min: Some(RULE_HEIGHT),
        max: Some(RULE_HEIGHT),
        priority: 0,
    };

    let mut node = stack(key, Axis::Vertical, None, vec![rule, row]);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node
}

/// "Items per page:" plus the page-size picker, then the group's closing
/// divider. Label padding 16 start (the container's `padding-inline`);
/// the picker's own padding is 8 start / 16 end (slice-d:66).
fn items_per_page(page_size: u32, open_sizes: Option<&[u32]>) -> ViewNode {
    let caption = cell(
        "label-cell",
        InsetRefs {
            left: Some(t(SPACING_05)),
            ..InsetRefs::default()
        },
        muted("label", "Items per page:"),
    );
    let trigger = picker(
        PAGE_SIZE_PICKER,
        "page-size",
        PAGE_SIZE_LABEL,
        page_size.to_string(),
        pad_inline(SPACING_03, SPACING_05),
        open_sizes.is_some(),
    );
    let menu = open_sizes.map(|sizes| {
        list_box(
            PICKER_MENU,
            PAGE_SIZE_LABEL,
            PAGE_SIZE_PICKER,
            sizes
                .iter()
                .map(|n| dropdown_option(format!("size-{n}"), n.to_string(), *n == page_size))
                .collect(),
        )
    });
    run(
        "items-per-page",
        vec![
            caption,
            picker_cell("page-size-cell", trigger, menu),
            nav_divider("divider-items"),
        ],
    )
}

/// A run of cells, each exactly as wide as it asks to be: a one-row grid
/// of `FitContent` columns on a row fixed at [`SIZE_MD`], stretched to it.
/// The row is fixed rather than `FitContent` because a group with no
/// 40-tall child would otherwise size its row to the captions and leave
/// its divider short of the bar.
///
/// Not a `Stack`. A stack probes itself once for its natural width and
/// then distributes that budget to its children, and the two do not
/// agree to the unit — measured with real text metrics the `controls`
/// stack came out about two units short of the sum of its children, and a
/// stack settles a shortfall by squeezing its most flexible child, which
/// for a text label means wrapping. A grid probes each column on its own
/// and hands each child exactly that width.
fn run(key: &'static str, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::FitContent; children.len()],
            rows: vec![TrackSize::Fixed { value: SIZE_MD }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(children)
}

/// The range text, centred on the bar, 16 off the divider before it.
fn range_cell(range: String) -> ViewNode {
    cell(
        "range",
        InsetRefs {
            left: Some(t(SPACING_05)),
            ..InsetRefs::default()
        },
        muted("range-text", range),
    )
}

/// One padded, vertically centred text cell. The padding lives here and
/// not on the text because a `Text` leaf has nothing to inset and the
/// tree validator refuses the declaration.
fn cell(key: &'static str, padding: InsetRefs, child: ViewNode) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![child]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(padding);
    node
}

fn pad_inline(start: &str, end: &str) -> InsetRefs {
    InsetRefs {
        left: Some(t(start)),
        right: Some(t(end)),
        ..InsetRefs::default()
    }
}

/// A picker and, while open, its list box — siblings in one child list so
/// the list's [`crate::tree::Anchor::Sibling`] resolves against the picker.
fn picker_cell(key: &'static str, trigger: ViewNode, menu: Option<ViewNode>) -> ViewNode {
    let mut parts = vec![trigger];
    parts.extend(menu);
    let mut node = stack(key, Axis::Horizontal, None, parts);
    node.props.align = Some(Align::Center);
    node
}

/// A closed-or-open inline select: the current value, then a 16 chevron
/// pointing down shut and up open (Carbon's `.cds--select__arrow`, and the
/// rotation every open list box gives it). A [`Role::Button`] with
/// `Semantics.value` and `Semantics.expanded`, resting on the bar's own
/// fill so `background@hover` has a resting binding.
fn picker(
    key: &'static str,
    value_key: &'static str,
    label: &'static str,
    value: String,
    padding: InsetRefs,
    open: bool,
) -> ViewNode {
    let mut value_node = text(value_key, value.clone());
    value_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let chevron = icon_toned(
        "chevron",
        if open {
            IconMark::ChevronUp
        } else {
            IconMark::ChevronDown
        },
        IconTone::Primary,
    );
    let mut node = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![value_node, chevron],
    );
    node.props.align = Some(Align::Center);
    node.props.padding = Some(padding);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut node =
        node.with_constraints(pin_height(SIZE_MD))
            .interactive(Role::Button, label, NAV_INTENTS);
    node.semantics.value = Some(value);
    node.semantics.expanded = Some(open);
    node
}

/// The right group: divider, page picker, "of N pages", divider,
/// Previous, divider, Next. Page-select padding 16 start / 8 end
/// (slice-d:67).
fn page_controls(page: u32, page_count: u32, pages_open: bool) -> ViewNode {
    let trigger = picker(
        PAGE_PICKER,
        "page",
        PAGE_LABEL,
        page.to_string(),
        pad_inline(SPACING_05, SPACING_03),
        pages_open,
    );
    let menu = pages_open.then(|| {
        list_box(
            PICKER_MENU,
            PAGE_LABEL,
            PAGE_PICKER,
            (1..=page_count.max(1))
                .map(|n| dropdown_option(format!("page-{n}"), n.to_string(), n == page))
                .collect(),
        )
    });
    let count = cell(
        "page-count-cell",
        InsetRefs {
            right: Some(t(SPACING_05)),
            ..InsetRefs::default()
        },
        muted("page-count", format!("of {page_count} pages")),
    );

    let previous = nav_button("previous", "Previous", IconMark::CaretLeft, page <= 1);
    let next = nav_button(
        "next",
        "Next",
        IconMark::CaretRight,
        page_count == 0 || page >= page_count,
    );

    run(
        "controls",
        vec![
            nav_divider("divider-page"),
            picker_cell("page-cell", trigger, menu),
            count,
            nav_divider("divider-previous"),
            previous,
            nav_divider("divider-next"),
            next,
        ],
    )
}

/// Carbon's `$text-secondary` captions.
fn muted(key: &'static str, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    node
}

/// A 1px vertical line spanning the row's own height, standing in for the
/// `border-inline-start` Carbon puts on the button itself (see
/// [`DIVIDER_WIDTH`]'s doc for why this is a sibling element).
///
/// The [`run`] grid's `Align::Stretch` grows it to the row's placed
/// height. A **childless `Stack`**, not a `Spacer`: `stack::measure`
/// returns `Size::ZERO` for a childless stack before it ever looks at what
/// was offered, so leaving the vertical axis unconstrained costs nothing at
/// measure time — only `Stretch`, read at *place* time once the row's real
/// height is already settled, ever grows it. A `Spacer` answers "whatever
/// is offered" on an unconstrained axis and reported a 900-unit bar once.
fn nav_divider(key: &'static str) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    node.constraints.horizontal = AxisConstraint {
        min: Some(DIVIDER_WIDTH),
        max: Some(DIVIDER_WIDTH),
        priority: 0,
    };
    node
}

/// A ghost icon button, 40×40: one 16 caret centred, the label in
/// [`Semantics`] only. Unavailable: [`super::disabled`], and the caret in
/// [`IconTone::Disabled`] — Carbon's `$icon-disabled` — because the caret
/// is the whole visible button and a live-toned caret on a dead button is
/// a button that lies.
fn nav_button(
    key: &'static str,
    label: &'static str,
    mark: IconMark,
    unavailable: bool,
) -> ViewNode {
    let tone = if unavailable {
        IconTone::Disabled
    } else {
        IconTone::Primary
    };
    let mut node = stack(
        key,
        Axis::Horizontal,
        None,
        vec![icon_toned("caret", mark, tone)],
    );
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Align::Center);
    // Resting background: the bar it sits on. Without this,
    // `background@hover` has no resting `background` beneath it and
    // resolves to nothing at rest (see
    // `a_state_decorated_token_always_has_a_resting_binding`).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let node =
        node.with_constraints(pin_square(SIZE_MD))
            .interactive(Role::Button, label, NAV_INTENTS);
    if unavailable { disabled(node) } else { node }
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

fn pin_square(side: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(side),
            max: Some(side),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(side),
            max: Some(side),
            priority: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DIVIDER_WIDTH, PaginationPicker, RULE_HEIGHT, SIZE_MD, pagination, pagination_items,
        pagination_items_open,
    };
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_HOVER, SURFACE_RAISED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Rect, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn has_key(node: &ViewNode, key: &str) -> bool {
        if node.key.as_str() == key {
            return true;
        }
        node.children.iter().any(|child| has_key(child, key))
    }

    fn child_keys(node: &ViewNode) -> Vec<&str> {
        node.children.iter().map(|c| c.key.as_str()).collect()
    }

    #[test]
    fn pagination_is_size_md_with_caret_prev_next() {
        let node = pagination("pages", 2, 5);
        let bar = named(&node, "bar");
        assert_eq!(bar.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(bar.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(
            child_keys(&node),
            ["rule", "bar"],
            "a 1-unit top rule over the 40-tall bar"
        );
        assert_eq!(
            child_keys(named(&node, "controls")),
            [
                "divider-page",
                "page-cell",
                "page-count-cell",
                "divider-previous",
                "previous",
                "divider-next",
                "next"
            ],
            "Carbon's right group: divider, page picker, `of N pages`, then \
             each nav button behind its own divider"
        );

        for (key, label, mark) in [
            ("previous", "Previous", IconMark::CaretLeft),
            ("next", "Next", IconMark::CaretRight),
        ] {
            let button = named(&node, key);
            assert_eq!(button.semantics.role, Some(Role::Button));
            assert_eq!(button.semantics.label.as_deref(), Some(label));
            assert!(button.interactions.contains(&Interaction::Click));
            assert!(!button.semantics.disabled);
            assert_eq!(
                button.constraints.horizontal.min,
                Some(SIZE_MD),
                "{key}: square"
            );
            assert_eq!(button.constraints.horizontal.max, Some(SIZE_MD));
            assert_eq!(button.constraints.vertical.max, Some(SIZE_MD));
            assert!(
                button.children.iter().all(|c| c.props.text.is_none()),
                "{key}: the button is a caret, not a word"
            );
            assert_eq!(
                named(button, "caret").props.canvas,
                icon_toned("caret", mark, IconTone::Primary).props.canvas,
                "{key}: Carbon's caret glyph in `icon-primary`"
            );
        }
    }

    /// The page number is a picker: a button carrying the value, a chevron
    /// pointing down while shut, and no list until it is opened.
    #[test]
    fn pagination_current_page_is_a_closed_picker() {
        let node = pagination("pages", 3, 10);
        let picker = named(&node, "page-picker");
        assert_eq!(picker.semantics.role, Some(Role::Button));
        assert_eq!(picker.semantics.label.as_deref(), Some("Page number"));
        assert_eq!(picker.semantics.value.as_deref(), Some("3"));
        assert_eq!(picker.semantics.expanded, Some(false));
        assert!(picker.interactions.contains(&Interaction::Click));
        assert_eq!(named(picker, "page").props.text.as_deref(), Some("3"));
        assert_eq!(
            named(picker, "chevron").props.canvas,
            icon_toned("chevron", IconMark::ChevronDown, IconTone::Primary)
                .props
                .canvas
        );
        assert_eq!(
            named(&node, "page-count").props.text.as_deref(),
            Some("of 10 pages")
        );
        assert!(!has_key(&node, "menu"), "shut: no list mounted");
        assert!(
            !has_key(&node, "items-per-page"),
            "without a page size there is no items-per-page group to invent"
        );
    }

    /// [`pagination_items`] carries Carbon's whole anatomy: the page size
    /// as a picker with `Semantics.value`, the range text for this page,
    /// and the page count derived from the total.
    #[test]
    fn pagination_items_derives_the_range_and_the_page_count() {
        let node = pagination_items("pages", 1, 10, 50);
        assert_eq!(
            child_keys(named(&node, "bar")),
            ["items-per-page", "range", "controls"]
        );
        let size = named(&node, "page-size-picker");
        assert_eq!(size.semantics.role, Some(Role::Button));
        assert_eq!(size.semantics.label.as_deref(), Some("Items per page"));
        assert_eq!(size.semantics.value.as_deref(), Some("10"));
        assert_eq!(named(size, "page-size").props.text.as_deref(), Some("10"));
        assert_eq!(
            named(&node, "range-text").props.text.as_deref(),
            Some("1\u{2013}10 of 50 items")
        );
        assert_eq!(
            named(&node, "page-count").props.text.as_deref(),
            Some("of 5 pages")
        );
        assert!(named(&node, "previous").semantics.disabled);
        assert!(!named(&node, "next").semantics.disabled);

        let last = pagination_items("pages", 5, 10, 47);
        assert_eq!(
            named(&last, "range-text").props.text.as_deref(),
            Some("41\u{2013}47 of 47 items"),
            "the last page's range stops at the total"
        );
        assert_eq!(
            named(&last, "page-count").props.text.as_deref(),
            Some("of 5 pages"),
            "47 items at 10 per page is 5 pages"
        );
        assert!(named(&last, "next").semantics.disabled);
    }

    /// An open picker mounts a beakless list box as its sibling, listing
    /// the caller's page sizes or the pages, with the current one marked;
    /// its chevron turns up; the other picker stays shut.
    #[test]
    fn an_open_picker_lists_its_options_under_itself() {
        let sizes = pagination_items_open(
            "pages",
            2,
            10,
            &[10, 20, 30],
            50,
            PaginationPicker::PageSize,
        );
        let cell = named(&sizes, "page-size-cell");
        assert_eq!(
            child_keys(cell),
            ["page-size-picker", "menu"],
            "trigger then its list"
        );
        let picker = named(cell, "page-size-picker");
        assert_eq!(picker.semantics.expanded, Some(true));
        assert_eq!(
            named(picker, "chevron").props.canvas,
            icon_toned("chevron", IconMark::ChevronUp, IconTone::Primary)
                .props
                .canvas
        );
        let menu = named(cell, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        let rows = &named(menu, "content").children;
        assert_eq!(
            rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["size-10", "size-20", "size-30"]
        );
        assert!(
            named(menu, "size-10").semantics.selected,
            "the current size is marked"
        );
        assert!(!named(menu, "size-20").semantics.selected);
        assert_eq!(
            named(&sizes, "page-picker").semantics.expanded,
            Some(false),
            "the other picker stays shut"
        );
        assert!(!has_key(named(&sizes, "controls"), "menu"));

        let pages =
            pagination_items_open("pages", 2, 10, &[10, 20, 30], 50, PaginationPicker::Page);
        let cell = named(&pages, "page-cell");
        assert_eq!(child_keys(cell), ["page-picker", "menu"]);
        let rows = &named(named(cell, "menu"), "content").children;
        assert_eq!(
            rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["page-1", "page-2", "page-3", "page-4", "page-5"]
        );
        assert!(named(cell, "page-2").semantics.selected);
        assert!(!has_key(named(&pages, "items-per-page"), "menu"));
    }

    #[test]
    fn pagination_disables_prev_on_page_one_and_dims_its_caret() {
        let node = pagination("pages", 1, 4);
        let previous = named(&node, "previous");
        assert!(previous.semantics.disabled);
        assert!(!previous.interactions.contains(&Interaction::Click));
        assert_eq!(
            named(previous, "caret").props.canvas,
            icon_toned("caret", IconMark::CaretLeft, IconTone::Disabled)
                .props
                .canvas,
            "a disabled caret is drawn in `icon-disabled`"
        );
        let next = named(&node, "next");
        assert!(!next.semantics.disabled);
        assert!(next.interactions.contains(&Interaction::Click));
    }

    #[test]
    fn pagination_disables_next_on_the_last_page() {
        let node = pagination("pages", 4, 4);
        let next = named(&node, "next");
        assert!(next.semantics.disabled);
        assert!(!next.interactions.contains(&Interaction::Click));
        assert_eq!(
            named(next, "caret").props.canvas,
            icon_toned("caret", IconMark::CaretRight, IconTone::Disabled)
                .props
                .canvas
        );
        let previous = named(&node, "previous");
        assert!(!previous.semantics.disabled);
        assert!(previous.interactions.contains(&Interaction::Click));
    }

    #[test]
    fn nav_buttons_and_pickers_carry_a_resting_background_under_their_hover_state() {
        let node = pagination_items("pages", 2, 10, 50);
        for key in ["previous", "next", "page-picker", "page-size-picker"] {
            let button = named(&node, key);
            assert_eq!(
                button.props.tokens.get("background").map(|t| t.as_str()),
                Some(SURFACE_RAISED),
                "{key}: resting background"
            );
            assert_eq!(
                button
                    .props
                    .tokens
                    .get("background@hover")
                    .map(|t| t.as_str()),
                Some(LAYER_HOVER),
                "{key}: hover background"
            );
        }
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
        let registry = accepting_registry();
        let mut harness = Harness::new();
        let viewport = Viewport::new(VIEWPORT, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    fn rect_of(frame: &PetrifiedFrame, suffix: &str) -> Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ends with {suffix:?}"))
            .rect
    }

    /// Row 23's geometry, pinned on the placed frame: the bar's one
    /// horizontal edge is a rule that spans the bar's whole width, the bar
    /// is as wide as its column, the page controls end at the bar's end,
    /// every divider spans the 40-tall row, the nav buttons are 40 squares
    /// with their caret centred, and the bar does not balloon past 40 +
    /// the rule.
    #[test]
    fn the_top_rule_spans_the_bar_and_the_dividers_span_the_row() {
        for (label, node) in [
            ("page-only", pagination("pages", 2, 5)),
            ("full", pagination_items("pages", 2, 10, 50)),
        ] {
            let frame = petrify_lone(node);
            let outer = rect_of(&frame, "/pages");
            let rule = rect_of(&frame, "/pages/rule");
            let bar = rect_of(&frame, "/pages/bar");
            assert_eq!(
                outer.w, VIEWPORT.w,
                "{label}: the bar is as wide as its column"
            );
            assert_eq!(
                rule.x, outer.x,
                "{label}: rule starts at the bar's left edge"
            );
            assert_eq!(rule.w, outer.w, "{label}: rule spans the bar's whole width");
            assert_eq!(rule.h, RULE_HEIGHT, "{label}");
            assert_eq!(bar.h, SIZE_MD, "{label}: the row is exactly md");
            assert_eq!(
                outer.h,
                SIZE_MD + RULE_HEIGHT,
                "{label}: the bar's own height must not balloon"
            );
            let controls = rect_of(&frame, "/bar/controls");
            assert_eq!(
                controls.x + controls.w,
                bar.x + bar.w,
                "{label}: the page controls sit at the bar's end"
            );
            for divider in ["/divider-page", "/divider-previous", "/divider-next"] {
                let d = rect_of(&frame, divider);
                assert_eq!(d.w, DIVIDER_WIDTH, "{label} {divider}");
                assert_eq!(d.y, bar.y, "{label} {divider}: flush with the row's top");
                assert_eq!(d.h, bar.h, "{label} {divider}: spans the row's full height");
            }
            for key in ["/previous", "/next"] {
                let button = rect_of(&frame, key);
                assert_eq!(button.w, SIZE_MD, "{label} {key}: a 40 square");
                assert_eq!(button.h, SIZE_MD, "{label} {key}: a 40 square");
                let caret = rect_of(&frame, &format!("{key}/caret"));
                assert_eq!(
                    caret.x - button.x,
                    (button.w - caret.w) / 2.0,
                    "{label} {key}: the caret is centred across"
                );
                assert_eq!(
                    caret.y - button.y,
                    (button.h - caret.h) / 2.0,
                    "{label} {key}: the caret is centred down"
                );
            }
            let page = rect_of(&frame, "/page");
            let centred = (bar.h - page.h) / 2.0;
            assert_eq!(
                page.y - bar.y,
                centred,
                "{label}: the page number stays vertically centred, not stretched"
            );
        }
        let frame = petrify_lone(pagination_items("pages", 2, 10, 50));
        let items = rect_of(&frame, "/bar/items-per-page");
        let bar = rect_of(&frame, "/pages/bar");
        assert_eq!(
            items.x, bar.x,
            "the items-per-page group starts at the bar's start"
        );
        let d = rect_of(&frame, "/divider-items");
        assert_eq!(d.h, bar.h, "the left group's closing divider spans the row");
        assert_eq!(
            d.x + d.w,
            items.x + items.w,
            "the left group's divider is its trailing edge"
        );
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: page 1 (previous disabled), the last page (next
    /// disabled), a mid-run page (both enabled), the full bar and both
    /// open forms all place with real rects, none of them outside their
    /// parent. The page-only bar's empty `range` cell is the one placement
    /// that may legitimately have no content; it still has a real rect.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for (label, node) in [
            ("first", pagination("pages", 1, 4)),
            ("last", pagination("pages", 4, 4)),
            ("mid", pagination("pages", 2, 5)),
            ("full", pagination_items("pages", 3, 10, 50)),
            (
                "sizes-open",
                pagination_items_open("pages", 3, 10, &[10, 20], 50, PaginationPicker::PageSize),
            ),
            (
                "pages-open",
                pagination_items_open("pages", 3, 10, &[10, 20], 50, PaginationPicker::Page),
            ),
        ] {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
                assert!(
                    !p.paint.truncated,
                    "{label}: {} was truncated to fit its parent",
                    p.id
                );
                // An anchored surface floats over the page by design; its
                // own children must still sit inside it.
                if let Some(parent_idx) = p.parent
                    && frame.placements[parent_idx].kind != NodeKind::Surface
                    && p.kind != NodeKind::Surface
                {
                    let parent = &frame.placements[parent_idx];
                    let fits = p.rect.x >= parent.rect.x - 0.01
                        && p.rect.y >= parent.rect.y - 0.01
                        && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                        && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                    assert!(
                        fits,
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: an enabled nav button and both pickers are reachable; a
    /// disabled nav button is not.
    #[test]
    fn a_disabled_nav_button_is_not_reachable() {
        let frame = petrify_lone(pagination_items("pages", 1, 10, 40));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let placed = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is placed"))
                .id
                .clone()
        };
        assert!(
            !order.contains(&placed("/previous")),
            "a disabled previous button must not be reachable"
        );
        for key in ["/next", "/page-picker", "/page-size-picker"] {
            assert!(
                order.contains(&placed(key)),
                "{key} declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: every caption and both picker values against the bar's own
    /// resting fill, in both themes, for both constructors.
    #[test]
    fn bar_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for node in [
                pagination("pages", 2, 5),
                pagination_items("pages", 2, 10, 50),
            ] {
                let bar_bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .expect("pagination binds a resting background");
                let bar_bg = color(&theme, bar_bg_name.as_str());
                fn walk_text(
                    node: &ViewNode,
                    inherited_bg: ColorValue,
                    theme: &Theme,
                    min: f32,
                    get_color: &impl Fn(&Theme, &str) -> ColorValue,
                ) {
                    let bg = match node.props.tokens.get("background") {
                        Some(name) => get_color(theme, name.as_str()),
                        None => inherited_bg,
                    };
                    if node.props.text.is_some()
                        && let Some(fg_name) = node.props.tokens.get("foreground")
                    {
                        let opacity = node.props.opacity.unwrap_or(1.0);
                        let fg = get_color(theme, fg_name.as_str()).faded(opacity).over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= min,
                            "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                            node.key,
                            fg_name.as_str()
                        );
                    }
                    for child in &node.children {
                        walk_text(child, bg, theme, min, get_color);
                    }
                }
                walk_text(&node, bar_bg, &theme, MIN_TEXT_CONTRAST, &color);
            }
        }
        let _ = BORDER_SUBTLE;
    }
}
