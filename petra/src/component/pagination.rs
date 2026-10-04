//! Carbon Pagination (slice-d), as a kit of pieces (spec 009 T022, D-092).
//!
//! MynaUI's page is composable: a caller picks Previous/Next, a number row,
//! a caption. Ours was a sealed bar. The bar remains ([`pagination`] and
//! [`pagination_items`] still build it) but it is a composition of the
//! same pieces a caller can pick on their own, not a second anatomy. We
//! keep both jobs: the items-per-page picker **and** a page-number row.
//! There is no four-sided boxed-border variant. First/Last are omitted:
//! [`IconMark`] has no PageFirst / PageLast, and a doubled caret is not
//! a cheap stand-in.
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
//!    `border-inline-end` divider. [`pagination_page_size`] is the piece;
//!    [`pagination_items`] composes it, [`pagination`] does not (it is
//!    not told a page size and does not invent one).
//! 3. Range text — "1–10 of 50 items" — filling the middle so the right
//!    group sits at the bar's end. [`pagination_range`] is the piece;
//!    same math as [`derive`]. [`pagination_items`] only.
//! 4. Page-number row — [`pagination_numbers`]: a compact window of page
//!    buttons plus "…" (`U+2026`) for collapsed runs, e.g. 1 … 4 5 6 … 20.
//!    The current page is [`Semantics.selected`] plus a
//!    `background@selected` fill plus [`TYPOGRAPHY_HEADING_SM`] weight.
//!    Fill is not enough on its own: `layer-selected` is a measured
//!    2-of-255 sRGB step off `layer.raised` in the dark theme.
//! 5. Right group — a `border-inline-start` divider, the **page picker**
//!    and "of N pages", then [`pagination_nav`]: Previous and Next, each
//!    behind its own divider (slice-d:58). They are Carbon's ghost icon
//!    buttons: 40×40, a 16 `CaretLeft` / `CaretRight` glyph ([`IconMark`])
//!    centred, labelled "Previous" / "Next" in [`Semantics`] so the word
//!    is still there for a reader who is not looking. Page 1 disables
//!    Previous via [`super::disabled`] and draws its caret in
//!    [`IconTone::Disabled`]; the last page disables Next the same way.
//!
//! Carbon's two pickers are native `<select>`s, whose popup the browser
//! draws. Here a picker is a [`Role::Button`] that opens a
//! [`super::list_box`] under itself: [`pagination_items_open`] mounts the
//! list for whichever picker is open, with the page sizes the caller
//! offers or the pages 1..=N, as [`super::dropdown_option`] rows. The
//! closed form ([`pagination_items`]) needs no option list, so it takes
//! none.

use super::disabled;
use super::dropdown::dropdown_option;
use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::{Dividers, list_box};
use super::pin_block;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SIZE_MD, SPACING_03,
    SPACING_05, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT,
    TYPOGRAPHY_HEADING_SM, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, InsetRefs, Intent, Interaction, Justify, Key, NodeKind,
    Phase, Props, Role, TrackSize, ViewNode,
};

/// Spec 010: page / prev / next — one shot navigation.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// Spec 010: page-size / page pickers open a list.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

const _: () = assert!(SIZE_MD == 40.0);

const NAV_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// What both of this bar's rules measure across themselves: the rule
/// material's own thickness at scale 1, which is the scale every test in
/// this module petrifies at.
///
/// Carbon writes `border-inline-start: 1px solid $border-subtle` on each nav
/// button and on the right group, and `border-inline-end` on the left group
/// (SOURCED `slice-d.md:57-58`). Both are separator elements here rather
/// than edge-slot bindings on the buttons: the buttons carry an opaque fill
/// and sit flush in a `Stretch` row, which is the compositing shape an edge
/// slot on a container cannot survive (see the module doc's item 1), and a
/// sibling element draws the same line either way.
///
/// **Not `1.0`.** These were one logical unit until 2026-09-09, which
/// reserved a quarter of the room the two-stroke groove paints; the seam
/// between the nav buttons is also the first vertical rule in this library
/// to reach the material. Neither number is written at a call site any
/// more — [`super::rule`] builds a `Separator` and the layout measures it.
/// Which is why this is test-only: production code here has no number left
/// to hold, and the tests still have placed geometry to check.
#[cfg(test)]
const RULE_UNITS: f32 = crate::token::rule::total_units();

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

/// Pagination bar at Carbon md (40) with the compact cluster: the
/// page-number row, the page picker, and Previous/Next. No items-per-page
/// group, because this constructor is not told a page size and does not
/// invent one. `page` is 1-indexed.
///
/// `page_count` is the last page number. Previous is unavailable on page
/// 1; Next is unavailable on the last page (and when there are no pages).
#[must_use]
pub fn pagination(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    bar(key, None, page, page_count, None)
}

/// Carbon's full bar, pickers closed: items per page, the range of items
/// on this page, the page-number row, and the page controls. `page` is
/// 1-indexed; `page_size` is the number of items per page; `total_items`
/// is the whole set.
///
/// The page count is `total_items / page_size` rounded up, and the range
/// text is "first–last of total items" for this page. A `page_size` of 0
/// is treated as 1 rather than dividing by it. Composes
/// [`pagination_page_size`], [`pagination_range`], [`pagination_numbers`],
/// and [`pagination_nav`] rather than a second sealed anatomy.
#[must_use]
pub fn pagination_items(
    key: impl Into<Key>,
    page: u32,
    page_size: u32,
    total_items: u32,
) -> ViewNode {
    let page_size = page_size.max(1);
    let (page_count, _) = derive(page, page_size, total_items);
    bar(key, Some((page_size, total_items)), page, page_count, None)
}

/// [`pagination_items`] with `picker` open: its list box hangs under it,
/// listing `page_sizes` (the caller's offer, as Carbon's `pageSizes` prop)
/// or the pages `1..=page_count`. A press on a listed row is the caller's
/// to handle; rows are keyed `size-{n}` and `page-{n}`.
#[must_use]
pub fn pagination_items_open(
    key: impl Into<Key>,
    page: u32,
    page_size: u32,
    page_sizes: &[u32],
    total_items: u32,
    picker: PaginationPicker,
) -> ViewNode {
    let page_size = page_size.max(1);
    let (page_count, _) = derive(page, page_size, total_items);
    bar(
        key,
        Some((page_size, total_items)),
        page,
        page_count,
        Some(Open {
            picker,
            page_sizes: page_sizes.to_vec(),
        }),
    )
}

/// Items-per-page group: the "Items per page:" caption, the page-size
/// picker, and the group's closing divider. `page_size` is the current
/// value. `open_sizes` `Some` mounts the list of offered sizes under the
/// picker, the same open form [`pagination_items_open`] uses for
/// [`PaginationPicker::PageSize`].
#[must_use]
pub fn pagination_page_size(
    key: impl Into<Key>,
    page_size: u32,
    open_sizes: Option<&[u32]>,
) -> ViewNode {
    let page_size = page_size.max(1);
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
            Dividers::Between,
        )
    });
    run(
        key,
        vec![
            caption,
            picker_cell("page-size-cell", trigger, menu),
            nav_divider("divider-items"),
        ],
    )
}

/// Range caption for this page of a set: "first–last of total items".
/// Same math as the bar ([`derive`]). A `page_size` of 0 is treated as 1.
#[must_use]
pub fn pagination_range(
    key: impl Into<Key>,
    page: u32,
    page_size: u32,
    total_items: u32,
) -> ViewNode {
    let page_size = page_size.max(1);
    let (_, range) = derive(page, page_size, total_items);
    cell(
        key,
        InsetRefs {
            left: Some(t(SPACING_05)),
            ..InsetRefs::default()
        },
        muted("range-text", range),
    )
}

/// Compact page-number row: a window around `page`, first and last always
/// shown when they fall outside it, and "…" (`U+2026`) where a run of two
/// or more pages collapsed. A gap of one page is shown as that page
/// rather than an ellipsis (collapsing one number into a mark wastes a
/// slot). `page` is 1-indexed. A `page_count` of 0 is an empty row.
///
/// The current page is distinguishable without hue: [`Semantics.selected`],
/// a `background@selected` fill, and [`TYPOGRAPHY_HEADING_SM`] (same size
/// as the unselected [`TYPOGRAPHY_BODY_COMPACT`], heavier weight). Number
/// buttons are keyed `num-{n}` so they do not collide with the page
/// picker's `page-{n}` option rows.
#[must_use]
pub fn pagination_numbers(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    if page_count == 0 {
        return stack(key, Axis::Horizontal, None, vec![]);
    }
    let page = page.clamp(1, page_count);
    let mut children = Vec::new();
    let mut prev: Option<NumberSlot> = None;
    for slot in number_slots(page, page_count) {
        match slot {
            NumberSlot::Page(n) => {
                children.push(page_number_button(n, n == page));
            }
            NumberSlot::Ellipsis => {
                // After page 1 it is the leading collapse; otherwise the
                // trailing one. The two cannot share a key.
                let slot_key = match prev {
                    Some(NumberSlot::Page(1)) => "ellipsis-start",
                    _ => "ellipsis-end",
                };
                children.push(ellipsis_mark(slot_key));
            }
        }
        prev = Some(slot);
    }
    run(key, children)
}

/// Previous and Next. Page 1 disables Previous; the last page (and a
/// count of zero) disables Next. Two channels on an unavailable button:
/// [`super::disabled`] (no click, not in focus order) and the caret in
/// [`IconTone::Disabled`]. Each button sits behind its own divider, the
/// same seams the bar already drew.
#[must_use]
pub fn pagination_nav(key: impl Into<Key>, page: u32, page_count: u32) -> ViewNode {
    let previous = nav_button("previous", "Previous", IconMark::CaretLeft, page <= 1);
    let next = nav_button(
        "next",
        "Next",
        IconMark::CaretRight,
        page_count == 0 || page >= page_count,
    );
    run(
        key,
        vec![
            nav_divider("divider-previous"),
            previous,
            nav_divider("divider-next"),
            next,
        ],
    )
}

/// One slot in the compact page-number window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumberSlot {
    Page(u32),
    Ellipsis,
}

/// Show-all budget: first + ellipsis + a 3-wide window + ellipsis + last.
/// At or under this, every page is a button and no mark is needed.
const NUMBER_BUDGET: u32 = 7;

/// Compact window: always first and last; a run of three around `page`;
/// an ellipsis where a gap of two or more pages was collapsed. `page` is
/// already clamped to `1..=page_count`; `page_count` is at least 1.
fn number_slots(page: u32, page_count: u32) -> Vec<NumberSlot> {
    if page_count <= NUMBER_BUDGET {
        return (1..=page_count).map(NumberSlot::Page).collect();
    }

    let mut start = page.saturating_sub(1).max(1);
    let mut end = page.saturating_add(1).min(page_count);
    if end.saturating_sub(start) < 2 {
        if start == 1 {
            end = 3.min(page_count);
        } else {
            start = page_count.saturating_sub(2).max(1);
        }
    }

    let mut slots = Vec::with_capacity(NUMBER_BUDGET as usize);
    if start > 1 {
        slots.push(NumberSlot::Page(1));
        if start == 3 {
            slots.push(NumberSlot::Page(2));
        } else if start > 3 {
            slots.push(NumberSlot::Ellipsis);
        }
    }
    for n in start..=end {
        slots.push(NumberSlot::Page(n));
    }
    if end < page_count {
        if end == page_count.saturating_sub(2) {
            slots.push(NumberSlot::Page(page_count - 1));
        } else if end < page_count.saturating_sub(2) {
            slots.push(NumberSlot::Ellipsis);
        }
        slots.push(NumberSlot::Page(page_count));
    }
    slots
}

/// One page button. Selected uses weight + selected semantics + fill, never
/// hue alone. Keyed `num-{n}` (see [`pagination_numbers`]).
fn page_number_button(n: u32, selected: bool) -> ViewNode {
    let label = format!("Page {n}");
    let mut caption = text("label", n.to_string());
    caption.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING_SM
    } else {
        TYPOGRAPHY_BODY_COMPACT
    }));
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(format!("num-{n}"), Axis::Horizontal, None, vec![caption]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props.padding = Some(pad_inline(SPACING_03, SPACING_03));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    let mut node = node
        .with_constraints(pin_at_least_md())
        .interactive(Role::Button, label, NAV_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text();
    node.semantics.selected = selected;
    node.semantics.value = Some(n.to_string());
    node
}

/// The collapsed-run mark. Not a button: this leaf does not mount an
/// overflow menu of the hidden pages (Carbon's overflow button). Text is
/// the single ellipsis character, not three dots.
fn ellipsis_mark(key: &'static str) -> ViewNode {
    cell(
        key,
        pad_inline(SPACING_03, SPACING_03),
        muted("dots", "\u{2026}"),
    )
}

/// Height pinned at md; width at least md so a one-digit page is a square
/// and a three-digit page can grow.
fn pin_at_least_md() -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(SIZE_MD),
            max: None,
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(SIZE_MD),
            max: Some(SIZE_MD),
            priority: 0,
        },
    }
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

/// The bar: a 1-unit top rule over a 40-tall grid whose weight column is
/// the range (or an empty spacer) that pushes the number row and the page
/// controls to the end. Pieces, not a second anatomy.
fn bar(
    key: impl Into<Key>,
    left: Option<(u32, u32)>,
    page: u32,
    page_count: u32,
    open: Option<Open>,
) -> ViewNode {
    let mut columns = Vec::with_capacity(4);
    let mut cells = Vec::with_capacity(4);
    match left {
        Some((page_size, total_items)) => {
            let sizes = open
                .as_ref()
                .filter(|o| o.picker == PaginationPicker::PageSize)
                .map(|o| o.page_sizes.as_slice());
            columns.push(TrackSize::FitContent);
            cells.push(pagination_page_size("items-per-page", page_size, sizes));
            columns.push(TrackSize::Weight { weight: 1.0 });
            cells.push(pagination_range("range", page, page_size, total_items));
        }
        None => {
            columns.push(TrackSize::Weight { weight: 1.0 });
            cells.push(stack("range", Axis::Horizontal, None, vec![]));
        }
    }
    let pages_open = open
        .as_ref()
        .is_some_and(|o| o.picker == PaginationPicker::Page);
    if page_count > 0 {
        columns.push(TrackSize::FitContent);
        cells.push(pagination_numbers("numbers", page, page_count));
    }
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

    let mut rule = super::rule("rule", Axis::Horizontal, BORDER_SUBTLE);
    rule.props.align_self = Some(Align::Stretch);

    let mut node = stack(key, Axis::Vertical, None, vec![rule, row]);
    node.props.align = Some(Align::Stretch);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node
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
fn run(key: impl Into<Key>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::FitContent; children.len()],
            rows: vec![TrackSize::Fixed { value: SIZE_MD }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(children)
}

/// One padded, vertically centred text cell. The padding lives here and
/// not on the text because a `Text` leaf has nothing to inset and the
/// tree validator refuses the declaration.
fn cell(key: impl Into<Key>, padding: InsetRefs, child: ViewNode) -> ViewNode {
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
    // A column, the same shape `dropdown` and `select` give their own pair,
    // and for the same reason: the list is a floating `Surface` that
    // `Anchor::Sibling` lifts out, but before the overlay pass runs it is
    // still a child of this stack. Laid out along a *horizontal* stack it
    // took a share of the row's width, and `Fit::Anchor` then widened it
    // from 0 rather than from the trigger — the panel came out 32 wide and
    // its dividers had no interior left to draw in.
    let mut node = stack(key, Axis::Vertical, None, parts);
    node.props.align = Some(Align::Start);
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
    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label, NAV_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text();
    node.semantics.value = Some(value);
    node.semantics.expanded = Some(open);
    node
}

/// The right group: divider, page picker, "of N pages", then
/// [`pagination_nav`]. Page-select padding 16 start / 8 end (slice-d:67).
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
            Dividers::Between,
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

    run(
        "controls",
        vec![
            nav_divider("divider-page"),
            picker_cell("page-cell", trigger, menu),
            count,
            pagination_nav("nav", page, page_count),
        ],
    )
}

/// Carbon's `$text-secondary` captions.
fn muted(key: &'static str, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
    node
}

/// The vertical seam spanning the row's own height, standing in for the
/// `border-inline-start` Carbon puts on the button itself. A sibling element
/// rather than an edge slot because the buttons carry an opaque fill and sit
/// flush in a `Stretch` row, which is the compositing shape an edge slot on
/// a container cannot survive; see the module doc's item 1. It is also the
/// first vertical rule in this library to reach the material
/// ([`crate::token::rule`]).
///
/// The [`run`] grid's `Align::Stretch` grows it to the row's placed
/// height. A `Separator`, not a `Spacer`: a separator measures zero along
/// its own axis under an unspecified offer, so leaving the vertical axis
/// unconstrained costs nothing at measure time — only `Stretch`, read at
/// *place* time once the row's real height is already settled, ever grows
/// it. A `Spacer` answers "whatever is offered" on an unconstrained axis
/// and reported a 900-unit bar once.
fn nav_divider(key: &'static str) -> ViewNode {
    super::rule(key, Axis::Vertical, BORDER_SUBTLE)
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
    node.props.justify = Some(Justify::Center);
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
    let node = node
        .with_constraints(pin_square(SIZE_MD))
        .interactive(Role::Button, label, NAV_INTENTS)
        .with_behaviour(ACTIVATES_ON_RELEASE)
        .owning_its_text();
    if unavailable { disabled(node) } else { node }
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
        PaginationPicker, RULE_UNITS, SIZE_MD, derive, pagination, pagination_items,
        pagination_items_open, pagination_nav, pagination_numbers, pagination_page_size,
        pagination_range,
    };
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{
        BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, SURFACE_RAISED, TYPOGRAPHY_BODY_COMPACT,
        TYPOGRAPHY_HEADING_SM,
    };
    use crate::frame::PetrifiedFrame;
    use crate::geom::{Axis, Rect, Size};

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Behaviour, Intent, Interaction, NodeKind, Phase, Role, ViewNode};

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
            child_keys(named(&node, "bar")),
            ["range", "numbers", "controls"],
            "compact cluster: number row then the page controls"
        );
        assert_eq!(
            child_keys(named(&node, "controls")),
            ["divider-page", "page-cell", "page-count-cell", "nav"],
            "Carbon's right group: divider, page picker, `of N pages`, then nav"
        );
        assert_eq!(
            child_keys(named(&node, "nav")),
            ["divider-previous", "previous", "divider-next", "next"],
            "each nav button behind its own divider"
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
            ["items-per-page", "range", "numbers", "controls"],
            "table bar composes items-per-page, range, numbers, nav"
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
        // A picker is a select's list box, so `Dividers::Between` rules
        // between each pair. This asserted the three options alone until
        // the wave that built `list_box` landed; that wave read Carbon's
        // `.cds--list-box__menu-item` boundary and this one shares it.
        let rows = &named(menu, "content").children;
        assert_eq!(
            rows.iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["size-10", "div-1", "size-20", "div-2", "size-30"]
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
            [
                "page-1", "div-1", "page-2", "div-2", "page-3", "div-3", "page-4", "div-4",
                "page-5"
            ]
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
        for key in [
            "previous",
            "next",
            "page-picker",
            "page-size-picker",
            "num-1",
            "num-2",
        ] {
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
            assert_eq!(rule.h, RULE_UNITS, "{label}");
            assert_eq!(bar.h, SIZE_MD, "{label}: the row is exactly md");
            assert_eq!(
                outer.h,
                SIZE_MD + RULE_UNITS,
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
                assert_eq!(d.w, RULE_UNITS, "{label} {divider}");
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
                    assert_fits_parent(label, p, &frame.placements[parent_idx]);
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
        for key in ["/next", "/page-picker", "/page-size-picker", "/num-2"] {
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

    fn caption_style<'a>(node: &'a ViewNode, key: &str) -> Option<&'a str> {
        named(named(node, key), "label")
            .props
            .style
            .as_ref()
            .map(|t| t.as_str())
    }

    /// The documented compact window: page 5 of 20 is `1 … 4 5 6 … 20`.
    #[test]
    fn pagination_numbers_collapses_runs_with_ellipsis() {
        let node = pagination_numbers("numbers", 5, 20);
        assert_eq!(
            child_keys(&node),
            [
                "num-1",
                "ellipsis-start",
                "num-4",
                "num-5",
                "num-6",
                "ellipsis-end",
                "num-20"
            ]
        );
        assert_eq!(
            named(named(&node, "ellipsis-start"), "dots")
                .props
                .text
                .as_deref(),
            Some("\u{2026}"),
            "one ellipsis character, not three dots"
        );
        assert_eq!(
            named(named(&node, "ellipsis-end"), "dots")
                .props
                .text
                .as_deref(),
            Some("\u{2026}")
        );
        assert!(
            !named(&node, "ellipsis-start")
                .interactions
                .contains(&Interaction::Click)
        );
        assert_eq!(
            named(&node, "num-5").semantics.label.as_deref(),
            Some("Page 5")
        );
        assert_eq!(named(&node, "num-5").semantics.value.as_deref(), Some("5"));
        assert_eq!(named(&node, "num-4").semantics.role, Some(Role::Button));
    }

    #[test]
    fn pagination_numbers_shows_all_when_the_run_fits() {
        let node = pagination_numbers("numbers", 3, 7);
        assert_eq!(
            child_keys(&node),
            [
                "num-1", "num-2", "num-3", "num-4", "num-5", "num-6", "num-7"
            ]
        );
        assert!(!has_key(&node, "ellipsis-start"));
        assert!(!has_key(&node, "ellipsis-end"));
    }

    #[test]
    fn pagination_numbers_keeps_first_and_last_at_the_edges() {
        let first = pagination_numbers("numbers", 1, 20);
        assert_eq!(
            child_keys(&first),
            ["num-1", "num-2", "num-3", "ellipsis-end", "num-20"]
        );
        let last = pagination_numbers("numbers", 20, 20);
        assert_eq!(
            child_keys(&last),
            ["num-1", "ellipsis-start", "num-18", "num-19", "num-20"]
        );
        let gap_one_leading = pagination_numbers("numbers", 4, 20);
        assert_eq!(
            child_keys(&gap_one_leading),
            [
                "num-1",
                "num-2",
                "num-3",
                "num-4",
                "num-5",
                "ellipsis-end",
                "num-20"
            ],
            "a gap of one page is that page, not an ellipsis"
        );
        let gap_one_trailing = pagination_numbers("numbers", 17, 20);
        assert_eq!(
            child_keys(&gap_one_trailing),
            [
                "num-1",
                "ellipsis-start",
                "num-16",
                "num-17",
                "num-18",
                "num-19",
                "num-20"
            ]
        );
        let empty = pagination_numbers("numbers", 1, 0);
        assert!(child_keys(&empty).is_empty());
        let clamped_low = pagination_numbers("numbers", 0, 20);
        assert!(named(&clamped_low, "num-1").semantics.selected);
        let clamped_high = pagination_numbers("numbers", 99, 20);
        assert!(named(&clamped_high, "num-20").semantics.selected);
    }

    /// Current page is selected semantics + fill + weight. Colour is never
    /// the only channel: heading-sm is the same size as body-compact with
    /// a heavier weight, and `layer-selected` is a measured 2-of-255 step
    /// off the resting fill in the dark theme.
    #[test]
    fn current_page_is_selected_by_weight_and_semantics_not_hue_alone() {
        let node = pagination_numbers("numbers", 5, 20);
        let current = named(&node, "num-5");
        let other = named(&node, "num-4");
        assert!(current.semantics.selected);
        assert!(!other.semantics.selected);
        assert_eq!(
            current
                .props
                .tokens
                .get("background@selected")
                .map(|t| t.as_str()),
            Some(LAYER_SELECTED)
        );
        assert_eq!(caption_style(&node, "num-5"), Some(TYPOGRAPHY_HEADING_SM));
        assert_eq!(caption_style(&node, "num-4"), Some(TYPOGRAPHY_BODY_COMPACT));
        assert_eq!(caption_style(&node, "num-1"), Some(TYPOGRAPHY_BODY_COMPACT));
        assert_eq!(
            caption_style(&node, "num-20"),
            Some(TYPOGRAPHY_BODY_COMPACT)
        );
    }

    #[test]
    fn pagination_range_uses_the_same_math_as_the_bar() {
        let range = pagination_range("range", 1, 10, 50);
        assert_eq!(
            named(&range, "range-text").props.text.as_deref(),
            Some("1\u{2013}10 of 50 items")
        );
        let last = pagination_range("range", 5, 10, 47);
        assert_eq!(
            named(&last, "range-text").props.text.as_deref(),
            Some("41\u{2013}47 of 47 items")
        );
        let zero = pagination_range("range", 1, 0, 10);
        assert_eq!(
            named(&zero, "range-text").props.text.as_deref(),
            Some("1\u{2013}1 of 10 items"),
            "page_size 0 is treated as 1, same as the bar"
        );
    }

    #[test]
    fn pagination_page_size_is_the_items_per_page_picker() {
        let node = pagination_page_size("items-per-page", 10, None);
        let picker = named(&node, "page-size-picker");
        assert_eq!(picker.semantics.role, Some(Role::Button));
        assert_eq!(picker.semantics.label.as_deref(), Some("Items per page"));
        assert_eq!(picker.semantics.value.as_deref(), Some("10"));
        assert_eq!(picker.semantics.expanded, Some(false));
        assert!(!has_key(&node, "menu"));
        assert!(has_key(&node, "divider-items"));

        let open = pagination_page_size("items-per-page", 20, Some(&[10, 20, 30]));
        assert_eq!(
            named(&open, "page-size-picker").semantics.expanded,
            Some(true)
        );
        assert!(named(&open, "size-20").semantics.selected);
        assert!(!named(&open, "size-10").semantics.selected);
    }

    #[test]
    fn pagination_nav_disables_on_two_channels() {
        let first = pagination_nav("nav", 1, 4);
        let previous = named(&first, "previous");
        assert!(previous.semantics.disabled);
        assert!(!previous.interactions.contains(&Interaction::Click));
        assert_eq!(
            named(previous, "caret").props.canvas,
            icon_toned("caret", IconMark::CaretLeft, IconTone::Disabled)
                .props
                .canvas
        );
        assert!(!named(&first, "next").semantics.disabled);

        let last = pagination_nav("nav", 4, 4);
        assert!(named(&last, "next").semantics.disabled);
        assert_eq!(
            named(named(&last, "next"), "caret").props.canvas,
            icon_toned("caret", IconMark::CaretRight, IconTone::Disabled)
                .props
                .canvas
        );
        assert!(!named(&last, "previous").semantics.disabled);

        let none = pagination_nav("nav", 1, 0);
        assert!(named(&none, "previous").semantics.disabled);
        assert!(named(&none, "next").semantics.disabled);
    }

    /// A table bar is items + range + numbers + nav. A compact cluster is
    /// numbers + nav. The constructors remain; a caller can also compose
    /// the pieces without them.
    #[test]
    fn a_caller_composes_items_range_numbers_and_nav() {
        let table = super::stack(
            "table",
            Axis::Horizontal,
            None,
            vec![
                pagination_page_size("items-per-page", 10, None),
                pagination_range("range", 5, 10, 200),
                pagination_numbers("numbers", 5, 20),
                pagination_nav("nav", 5, 20),
            ],
        );
        assert_eq!(
            child_keys(&table),
            ["items-per-page", "range", "numbers", "nav"]
        );
        assert_eq!(
            named(&table, "range-text").props.text.as_deref(),
            Some("41\u{2013}50 of 200 items")
        );
        assert!(named(&table, "num-5").semantics.selected);
        assert!(!named(&table, "previous").semantics.disabled);
        assert!(!named(&table, "next").semantics.disabled);

        let compact = super::stack(
            "compact",
            Axis::Horizontal,
            None,
            vec![
                pagination_numbers("numbers", 1, 20),
                pagination_nav("nav", 1, 20),
            ],
        );
        assert_eq!(child_keys(&compact), ["numbers", "nav"]);
        assert!(!has_key(&compact, "items-per-page"));
        assert!(named(&compact, "previous").semantics.disabled);
        assert!(named(&compact, "num-1").semantics.selected);
    }

    #[test]
    fn the_bar_composes_the_number_row() {
        let node = pagination("pages", 5, 20);
        assert_eq!(
            child_keys(named(&node, "numbers")),
            [
                "num-1",
                "ellipsis-start",
                "num-4",
                "num-5",
                "num-6",
                "ellipsis-end",
                "num-20"
            ]
        );
        assert!(named(&node, "num-5").semantics.selected);
        let items = pagination_items("pages", 5, 10, 200);
        assert!(has_key(&items, "items-per-page"));
        assert!(has_key(&items, "numbers"));
        assert!(has_key(&items, "nav"));
        assert_eq!(
            named(&items, "range-text").props.text.as_deref(),
            Some("41\u{2013}50 of 200 items")
        );
        let none = pagination("pages", 1, 0);
        assert!(
            !has_key(&none, "numbers"),
            "no page-number column when there are no pages (a 0-wide FitContent \
             cell would petrify as a degenerate rect)"
        );
        assert!(named(&none, "previous").semantics.disabled);
        assert!(named(&none, "next").semantics.disabled);
    }

    /// T023 / spec R5: the items-per-page picker is a capability. Dropping
    /// [`pagination_page_size`], or dropping it from [`pagination_items`],
    /// is the loss this fails on.
    #[test]
    fn items_per_page_picker_exists() {
        let piece = pagination_page_size("items-per-page", 10, None);
        let picker = named(&piece, "page-size-picker");
        assert_eq!(picker.semantics.role, Some(Role::Button));
        assert_eq!(picker.semantics.label.as_deref(), Some("Items per page"));
        assert_eq!(picker.semantics.value.as_deref(), Some("10"));
        assert_eq!(named(picker, "page-size").props.text.as_deref(), Some("10"));

        let bar = pagination_items("pages", 1, 10, 50);
        assert!(
            has_key(&bar, "items-per-page"),
            "pagination_items still composes the items-per-page group"
        );
        let bar_picker = named(&bar, "page-size-picker");
        assert_eq!(bar_picker.semantics.role, Some(Role::Button));
        assert_eq!(
            bar_picker.semantics.label.as_deref(),
            Some("Items per page")
        );
        assert_eq!(bar_picker.semantics.value.as_deref(), Some("10"));
    }

    /// T023 / spec R5: [`derive`] still computes the range caption and the
    /// page count. Hardcoding "1–10 of 50 items", or truncating 47/10 to 4
    /// pages, is the loss this fails on.
    #[test]
    fn derive_computes_range_and_page_count() {
        for (page, page_size, total, pages, caption) in [
            (1, 10, 50, 5, "1\u{2013}10 of 50 items"),
            (2, 10, 50, 5, "11\u{2013}20 of 50 items"),
            (5, 10, 47, 5, "41\u{2013}47 of 47 items"),
            (1, 10, 10, 1, "1\u{2013}10 of 10 items"),
        ] {
            let (page_count, range) = derive(page, page_size, total);
            assert_eq!(
                page_count, pages,
                "{total} items at {page_size} per page is {pages} pages, not truncated"
            );
            assert_eq!(range, caption, "page {page} of {total} at {page_size}");
        }

        let first = pagination_items("pages", 1, 10, 50);
        assert_eq!(
            named(&first, "range-text").props.text.as_deref(),
            Some("1\u{2013}10 of 50 items")
        );
        assert_eq!(
            named(&first, "page-count").props.text.as_deref(),
            Some("of 5 pages")
        );
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
        assert_eq!(
            named(&pagination_range("range", 2, 10, 50), "range-text")
                .props
                .text
                .as_deref(),
            Some("11\u{2013}20 of 50 items"),
            "the range piece uses the same math as derive"
        );
    }

    /// T023 / spec R5: page 1 Previous is unavailable on two channels —
    /// [`super::disabled`] (no click) and the caret in [`IconTone::Disabled`].
    /// Keeping one and dropping the other is the loss this fails on.
    #[test]
    fn page_one_previous_is_disabled_on_two_channels() {
        for (label, node) in [
            ("nav", pagination_nav("nav", 1, 4)),
            ("bar", pagination("pages", 1, 4)),
            ("items", pagination_items("pages", 1, 10, 50)),
        ] {
            let previous = named(&node, "previous");
            assert!(
                previous.semantics.disabled,
                "{label}: Previous on page 1 is disabled()"
            );
            assert!(
                !previous.interactions.contains(&Interaction::Click),
                "{label}: disabled() clears Click"
            );
            assert_eq!(
                named(previous, "caret").props.canvas,
                icon_toned("caret", IconMark::CaretLeft, IconTone::Disabled)
                    .props
                    .canvas,
                "{label}: caret is IconTone::Disabled"
            );
        }
    }

    /// Spec 010: page / nav buttons declare Activate / OnRelease.
    ///
    /// Falsified by dropping `.with_behaviour(...)` from page / nav builders:
    ///
    /// ```text
    /// pagination button declared behaviour None
    /// ```
    #[test]
    fn pagination_nav_declares_activate_on_release() {
        let node = pagination_nav("nav", 2, 5);
        let want = Some(Behaviour {
            intent: Intent::Activate,
            phase: Phase::OnRelease,
        });
        assert_eq!(
            named(&node, "previous").behaviour,
            want,
            "pagination previous declared behaviour {:?}",
            named(&node, "previous").behaviour
        );
        assert_eq!(
            named(&node, "next").behaviour,
            want,
            "pagination next declared behaviour {:?}",
            named(&node, "next").behaviour
        );
    }
}
