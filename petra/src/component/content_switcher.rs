//! Carbon Content switcher (slice-b).
//!
//! Anatomy (docs + `_content-switcher.scss`), high-contrast default:
//! 1. [`content_switcher`] — the group: one bordered row, radius 4
//!    (`shape.corner-sm`, via `CornerRole::Grouping`), 1px [`BORDER_SUBTLE`] outline, every tab the same
//!    width (Carbon's `.cds--content-switcher-btn` is `inline-size: 100%`
//!    inside a flex row, so N tabs share the row N ways — `08-content-
//!    switcher.png` shows two tabs at exactly half each).
//! 2. [`content_switcher_item`] — one tab button, height 40 at md
//!    ([`SIZE_MD`]; `content_switcher_sm` 32 / `content_switcher_lg` 48,
//!    slice-b "Sizes"),
//!    the same box whichever state it is in. Selected is never colour
//!    alone: [`LAYER_SELECTED_INVERSE`] plus [`TEXT_INVERSE`] ink plus
//!    `Semantics.selected` plus [`TYPOGRAPHY_HEADING_SM`] weight — Carbon
//!    fills the selected tab with the *other* polarity's surface, which is
//!    a luminance step and not a hue.
//! 3. Divider — a 1×16 [`BORDER_SUBTLE`] line between two adjacent tabs,
//!    hidden beside the selected tab (slice-b:26, :32). Carbon also hides
//!    it beside a *focused* tab; focus is a frame-time fact this
//!    constructor cannot see, so that half is not expressed.
//!
//! # Variants (T053) — the same anatomy on two grounds and two item shapes
//!
//! - **Low contrast** (`content_switcher_low_contrast` /
//!   `content_switcher_item_low_contrast`): the `.cds--content-switcher--low-contrast`
//!   ground. The group sits on [`SURFACE_LAYER_TWO`] (Carbon
//!   `$content-switcher-background`, which defaults to `$layer-01`) under a
//!   [`BORDER_STRONG`] outline (slice-b Anatomy: `$border-inverse` high
//!   contrast, `$border-strong` low), and the selected tab is one surface
//!   step — [`LAYER_SELECTED`] (Carbon `$background-selected`, the
//!   `tab--border--selected` knob's ground) — plus [`BORDER_STRONG`],
//!   `Semantics.selected`, and the same weight bump. No inverse polarity
//!   appears anywhere in this ground: `layer-selected-inverse` and
//!   `text-inverse` belong to the high-contrast tab fill alone.
//! - **Icon only** (`content_switcher_icon_only` /
//!   `content_switcher_item_icon[_with_size]`): square tabs, "each tab's
//!   width equals its height" (slice-b, Formatting) — 32 / 40 / 48 per
//!   T032's step — carrying a centred glyph and no label text (the label
//!   is the button's accessible name in `Semantics`). The glyph is
//!   Carbon's "sm/md icon 16px, lg icon 20px": [`IconBox::Glyph`] at
//!   sm/md, [`IconBox::Header`] at lg.
//!
//! # Why the group carries two units of padding
//!
//! Children paint after their parent, so a tab whose fill spans the whole
//! group covers the group's own inset border and the outline vanishes —
//! the defect `08-content-switcher.png` showed before this pass (no edge
//! anywhere, `List` a lone filled block, `Grid` bare text). Accordion,
//! Pagination and the UI shell header all met this and answered it by
//! dropping the parent's `border` for a real divider element; a four-sided
//! rounded outline cannot be built from divider elements, so the group
//! keeps its border and insets its tabs by [`SPACING_01`] instead. The
//! smallest token on the ramp is two units, one more than the stroke, so a
//! one-unit seam of the group's ground shows between border and a selected
//! tab's light fill. The whole group is 44 tall against Carbon's 42 (a 40
//! box plus a 1px CSS `outline` drawn outside it). Both are the cost of an
//! engine that cannot paint a parent's edge over its children.

use super::icon::{IconBox, IconMark, IconTone, icon_in};
use super::tokens::{
    BORDER_STRONG, BORDER_SUBTLE, BUTTON_DISABLED, LAYER_HOVER, LAYER_SELECTED,
    LAYER_SELECTED_INVERSE, SIZE_LG, SIZE_MD, SIZE_SM, SPACING_01, SPACING_03, SPACING_04,
    SPACING_05, SURFACE_BASE, SURFACE_LAYER_TWO, TEXT_INVERSE, TEXT_MUTED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING_SM, t,
};
use super::{pad, pin_block, stack, swatch};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, InsetRefs, Intent, Interaction, Justify, Key, NodeKind,
    Phase, Props, Role, Semantics, TrackSize, ViewNode,
};

/// Carbon's content-switcher height ramp — slice-b "Sizes": sm 32, md 40,
/// lg 48 (`_content-switcher.scss:23` `layout.use('size', $default: 'md')`).
/// Closed on purpose: sm/md/lg is the whole documented scale, so a `match`
/// here turns a step nobody wired into a compile error rather than a
/// silently missing row — the trade [`super::list_box::ListBoxSize`]'s doc
/// records.
///
/// Public because the icon-only item sizes its glyph from the step
/// (`content_switcher_item_icon_with_size`): slice-b's Size table makes the
/// icon 16 at sm/md and 20 at lg, so an item that did not know its step
/// would put a 16px glyph in the lg square.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitcherSize {
    /// sm: the 32-tall step of the ramp above.
    Sm,
    /// md: 40 tall, the ramp's default step.
    Md,
    /// lg: the 48-tall step, where the icon glyph grows to 20.
    Lg,
}

/// Which documented container ground the group sits on. Slice-b
/// "Variants": **High contrast** (default — dark/inverse container) and
/// **Low contrast** (`.cds--content-switcher--low-contrast`, "lighter/less
/// visually disruptive, for use in cards/modals near primary buttons").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ground {
    /// The default: the selected tab is the opposite polarity's surface
    /// ([`LAYER_SELECTED_INVERSE`] on [`SURFACE_BASE`]).
    Inverse,
    /// The low-contrast ground: standard surface tokens throughout, the
    /// selection one step up the surface ladder ([`LAYER_SELECTED`]) — no
    /// inverse polarity anywhere.
    LowContrast,
}

impl SwitcherSize {
    /// Carbon's tab height for this step, in layout units.
    fn height(self) -> f32 {
        match self {
            Self::Sm => SIZE_SM,
            Self::Md => SIZE_MD,
            Self::Lg => SIZE_LG,
        }
    }
}

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Divider between two tabs: 1 wide, 16 tall (`_content-switcher.scss`
/// `::before`, `block-size: convert.to-rem(16px)`).
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_HEIGHT: f32 = 16.0;

/// The switcher group at Carbon md (40). `Role::TabList`, no interactions
/// of its own.
///
/// A single-row `Grid`: every tab sits in a `Weight { 1.0 }` column so
/// all of them share the row equally, and a `FitContent` column between
/// each adjacent pair holds the divider. `align: Stretch` is what makes a
/// tab fill its column; a `Stack` would let each tab hug its own label,
/// which is how `List` and `Grid` came to be two different widths.
///
/// The group owns the size step: whatever height a tab arrives with, the
/// group re-pins it to its own step, because Carbon's switcher is one
/// sized box and every tab inherits it (`_content-switcher.scss:23` is one
/// `layout.use('size')` on the container).
pub fn content_switcher(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Md,
        Ground::Inverse,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The switcher group at Carbon sm (32).
pub fn content_switcher_sm(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Sm,
        Ground::Inverse,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The switcher group at Carbon lg (48).
pub fn content_switcher_lg(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Lg,
        Ground::Inverse,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The low-contrast switcher group at Carbon md (40) — `.cds--content-
/// switcher--low-contrast`, text tabs (slice-b "Variants").
pub fn content_switcher_low_contrast(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Md,
        Ground::LowContrast,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The low-contrast switcher group at Carbon sm (32).
pub fn content_switcher_low_contrast_sm(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Sm,
        Ground::LowContrast,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The low-contrast switcher group at Carbon lg (48).
pub fn content_switcher_low_contrast_lg(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Lg,
        Ground::LowContrast,
        TrackSize::Weight { weight: 1.0 },
    )
}

/// The icon-only switcher group at Carbon md (40) — `.cds--content-
/// switcher--icon-only`, square tabs (slice-b "Formatting"). Tabs sit in
/// `FitContent` columns rather than sharing the row: "each tab's width
/// equals its height", so a shared row would stretch the squares into
/// rectangles.
pub fn content_switcher_icon_only(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Md,
        Ground::Inverse,
        TrackSize::FitContent,
    )
}

/// The icon-only switcher group at Carbon sm (32).
pub fn content_switcher_icon_only_sm(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Sm,
        Ground::Inverse,
        TrackSize::FitContent,
    )
}

/// The icon-only switcher group at Carbon lg (48).
pub fn content_switcher_icon_only_lg(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    at(
        key,
        items,
        SwitcherSize::Lg,
        Ground::Inverse,
        TrackSize::FitContent,
    )
}

fn at(
    key: impl Into<Key>,
    items: Vec<ViewNode>,
    size: SwitcherSize,
    ground: Ground,
    tab_column: TrackSize,
) -> ViewNode {
    let height = size.height();
    let square = tab_column == TrackSize::FitContent;
    let mut columns = Vec::with_capacity(items.len() * 2);
    let mut children = Vec::with_capacity(items.len() * 2);
    let mut previous_selected = false;
    for (i, mut item) in items.into_iter().enumerate() {
        // The group owns the size step: whatever height a tab arrives
        // with, the group re-pins it to its own step (module doc). A
        // square tab keeps its width pinned too, so a `FitContent` column
        // reads its square and not its label.
        item.constraints = if square {
            pin_square(height)
        } else {
            pin_block(height)
        };
        if i > 0 {
            let hidden = previous_selected || item.semantics.selected;
            columns.push(TrackSize::FitContent);
            children.push(divider(i, hidden));
        }
        previous_selected = item.semantics.selected;
        columns.push(tab_column);
        children.push(item);
    }
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns,
            rows: vec![TrackSize::FitContent],
            align: Some(Align::Stretch),
            padding: Some(pad(SPACING_01, SPACING_01)),
            ..Props::default()
        })
        .with_children(children);
    // FR-022: the enum's own doc names "content switcher" under
    // `CornerRole::Grouping`. The group's real height is the tab step (32/
    // 40/48) plus two units of padding on each edge (36/44/52 total, per
    // the module doc); the step alone is already well clear of Grouping's
    // 8-unit half-edge threshold at every step, so it stands in for the
    // padded total without inventing a new constant.
    node.props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Grouping, height)));
    // Ground: the high-contrast group is a bordered box on the page, the
    // low-contrast one a `$layer-01` box under a `$border-strong` outline
    // (slice-b Anatomy: "outline: 1px solid $border-inverse (high-contrast)
    // or $border-strong (low-contrast)"). The low ground also paints its
    // own fill — `$content-switcher-background`, Carbon's `$layer-01`.
    node.props.tokens.insert(
        "border".into(),
        t(match ground {
            Ground::Inverse => BORDER_SUBTLE,
            Ground::LowContrast => BORDER_STRONG,
        }),
    );
    if ground == Ground::LowContrast {
        node.props
            .tokens
            .insert("background".into(), t(SURFACE_LAYER_TWO));
    }
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}

/// Pin `h` as both the minimum and the maximum on both axes: a square.
/// Same four lines as `date_picker`'s private helper — `kit::pin_block`'s
/// doc records why those copies are not yet unified.
fn pin_square(h: f32) -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
    }
}

/// The 1×16 line between two tabs, centred on the row. `hidden` keeps the
/// column (so the tabs do not shift when the selection moves) and drops
/// the fill.
///
/// A `Stack` with `justify: Center` rather than the swatch alone: the
/// group's `Align::Stretch` stretches this wrapper to the row's full
/// height, and `justify` then seats the fixed-height swatch in the middle
/// of it. A bare swatch under `Stretch` would clamp to its own 16 and sit
/// at the top of the row.
fn divider(index: usize, hidden: bool) -> ViewNode {
    let fill = if hidden { None } else { Some(BORDER_SUBTLE) };
    let line = swatch("line", DIVIDER_WIDTH, DIVIDER_HEIGHT, fill, None, None);
    let mut node = stack(format!("divider-{index}"), Axis::Vertical, None, vec![line]);
    node.props.justify = Some(Justify::Center);
    node
}

/// One switcher tab. `Role::Button`, selected declared in `Semantics`.
///
/// Height is Carbon md 40 on its own; inside a group the group's size
/// step owns it (see [`content_switcher`]). Label padding is
/// `$spacing-05` inline. The box is the same in both states — only the
/// fill, the ink and the type weight change, so the pair never reflows
/// when the selection moves.
pub fn content_switcher_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let ink = if selected { TEXT_INVERSE } else { TEXT_MUTED };
    tab(
        key,
        label.clone(),
        vec![label_node(label, selected, ink)],
        selected,
        Ground::Inverse,
        pad(SPACING_05, SPACING_03),
    )
}

/// One tab on the low-contrast ground (`.cds--content-switcher--low-contrast`).
/// The selection never inverts: [`LAYER_SELECTED`] fill, [`TEXT_PRIMARY`]
/// ink, the same weight bump and `Semantics.selected`.
pub fn content_switcher_item_low_contrast(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let ink = if selected { TEXT_PRIMARY } else { TEXT_MUTED };
    tab(
        key,
        label.clone(),
        vec![label_node(label, selected, ink)],
        selected,
        Ground::LowContrast,
        pad(SPACING_05, SPACING_03),
    )
}

/// One icon-only tab at Carbon md: a 40×40 square with a 16px glyph
/// ([`IconBox::Glyph`], slice-b "sm/md icon 16px") centred in it. `label`
/// is the button's accessible name — the picture carries no text.
pub fn content_switcher_item_icon(
    key: impl Into<Key>,
    label: impl Into<String>,
    mark: IconMark,
    selected: bool,
) -> ViewNode {
    content_switcher_item_icon_with_size(key, label, mark, selected, SwitcherSize::Md)
}

/// One icon-only tab at `size`: a square the step's height wide with that
/// step's glyph (16 at sm/md, 20 at lg — [`IconBox::Glyph`] /
/// [`IconBox::Header`]). Carbon's per-step icon-only inline padding is
/// exactly `(height − icon) / 2` (sm 8 / md 12 / lg 14, slice-b "Key
/// numbers"); lg's 14 is `convert.to-rem(14px)`, not a spacing token, and
/// [`InsetRefs`] is token-only by FR-053, so the lg square pads
/// `spacing-04` and lets the square pin do the centring — every step's
/// padding is inert here anyway (the fixed glyph is centred, and the
/// measured pad is precisely the centre offset it replaces).
pub fn content_switcher_item_icon_with_size(
    key: impl Into<Key>,
    label: impl Into<String>,
    mark: IconMark,
    selected: bool,
    size: SwitcherSize,
) -> ViewNode {
    let (boxed, pad_in) = match size {
        SwitcherSize::Sm => (IconBox::Glyph, SPACING_03),
        SwitcherSize::Md => (IconBox::Glyph, SPACING_04),
        SwitcherSize::Lg => (IconBox::Header, SPACING_04),
    };
    let glyph = icon_in(
        "glyph",
        mark,
        boxed,
        if selected {
            IconTone::Inverse
        } else {
            IconTone::Secondary
        },
    );
    let mut node = tab(
        key,
        label.into(),
        vec![glyph],
        selected,
        Ground::Inverse,
        pad(pad_in, pad_in),
    );
    // The tab is its own square — "each tab's width equals its height"
    // (slice-b, Formatting) — whichever group (or none) it lands in.
    node.constraints = pin_square(size.height());
    node
}

/// The tab's label node: text, weight bumped on selection.
fn label_node(label: String, selected: bool, ink: &str) -> ViewNode {
    let mut node = super::text::text("label", label);
    node.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING_SM
    } else {
        TYPOGRAPHY_BODY
    }));
    node.props.tokens.insert("foreground".into(), t(ink));
    node
}

/// The shell every tab shares: a centred row, the ground's fill family,
/// the selected pair's disabled partner, and the select behaviour.
fn tab(
    key: impl Into<Key>,
    label: String,
    content: Vec<ViewNode>,
    selected: bool,
    ground: Ground,
    padding: InsetRefs,
) -> ViewNode {
    let (rest, on_selected) = match ground {
        Ground::Inverse => (SURFACE_BASE, LAYER_SELECTED_INVERSE),
        Ground::LowContrast => (SURFACE_LAYER_TWO, LAYER_SELECTED),
    };
    let mut node = stack(key, Axis::Horizontal, None, content);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props.padding = Some(padding);
    node.props.tokens.insert(
        "background".into(),
        t(if selected { on_selected } else { rest }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    // Carbon keeps the selected tab's fill under the pointer.
    node.props
        .tokens
        .insert("background@selected".into(), t(on_selected));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(on_selected));
    // The disabled pair for the selected segment's fill. The
    // precedence chain is `slot@selected-R → slot@selected → slot@R → slot`,
    // so `background@disabled` alone loses to `background@selected` here and
    // the muted label stays on the selected ground at 1.92:1 light / 1.48:1
    // dark — the one label T027's probe caught under the floor. The ground
    // swaps to Carbon's `$button-disabled`, which `text.muted` clears 4.5:1
    // on (5.70:1 light / 5.26:1 dark, `contracts/disabled.md`).
    node.props
        .tokens
        .insert("background@selected-disabled".into(), t(BUTTON_DISABLED));

    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label, ITEM_INTENTS)
        // No selection anchors inside a tab, label or glyph alike.
        .owning_its_text()
        // Spec 010: exclusive among siblings in the switcher.
        .with_behaviour(Behaviour {
            intent: Intent::Select,
            phase: Phase::OnRelease,
        });
    node.semantics.selected = selected;
    node
}

#[cfg(test)]
mod tests {
    use super::{
        BORDER_STRONG, BORDER_SUBTLE, DIVIDER_HEIGHT, DIVIDER_WIDTH, LAYER_SELECTED,
        LAYER_SELECTED_INVERSE, SIZE_LG, SIZE_MD, SIZE_SM, SURFACE_LAYER_TWO, TEXT_INVERSE,
        TEXT_MUTED, TEXT_PRIMARY, content_switcher, content_switcher_icon_only,
        content_switcher_icon_only_lg, content_switcher_icon_only_sm, content_switcher_item,
        content_switcher_item_icon, content_switcher_item_icon_with_size,
        content_switcher_item_low_contrast, content_switcher_lg, content_switcher_low_contrast,
        content_switcher_low_contrast_lg, content_switcher_low_contrast_sm, content_switcher_sm,
    };
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::frame::PetrifiedFrame;
    use crate::geom::Rect;

    use crate::token::{ColorValue, CornerRole, Theme, TokenName, TokenValue, corner_for};
    use crate::tree::{Interaction, NodeKind, Role, TrackSize, ViewNode};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn three() -> ViewNode {
        content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                content_switcher_item("b", "Grid", false),
                content_switcher_item("c", "Table", false),
            ],
        )
    }

    /// The same three tabs on the low-contrast ground.
    fn low_three() -> ViewNode {
        content_switcher_low_contrast(
            "sw",
            vec![
                content_switcher_item_low_contrast("a", "List", true),
                content_switcher_item_low_contrast("b", "Grid", false),
                content_switcher_item_low_contrast("c", "Table", false),
            ],
        )
    }

    /// Three icon-only tabs — the same selection geometry, squares.
    fn icon_three() -> ViewNode {
        content_switcher_icon_only(
            "sw",
            vec![
                content_switcher_item_icon("a", "List", super::IconMark::Menu, true),
                content_switcher_item_icon("b", "Grid", super::IconMark::Search, false),
                content_switcher_item_icon("c", "Table", super::IconMark::Close, false),
            ],
        )
    }

    #[test]
    fn content_switcher_sets_role_tablist() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                content_switcher_item("b", "Grid", false),
            ],
        );
        assert_eq!(node.key.as_str(), "sw");
        assert_eq!(node.semantics.role, Some(Role::TabList));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.kind, NodeKind::Grid);
        assert_eq!(
            token(&node, "radius"),
            Some(corner_for(CornerRole::Grouping, SIZE_MD))
        );
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn content_switcher_item_is_a_labelled_button_at_height_40() {
        let node = content_switcher_item("a", "List", false);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("List"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.interactions.contains(&Interaction::Hover));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert!(!node.semantics.selected);
    }

    /// Row 8's defect: the selected label took `TYPOGRAPHY_HEADING`, the
    /// 20px page-heading role, so the selected tab visibly grew and the
    /// pair reflowed on every click. Carbon steps the weight and keeps the
    /// size (`heading-compact-01`), which is `TYPOGRAPHY_HEADING_SM`.
    #[test]
    fn content_switcher_item_declares_selected_not_colour_alone() {
        let on = content_switcher_item("a", "List", true);
        assert!(on.semantics.selected);
        assert_eq!(token(&on, "background"), Some(LAYER_SELECTED_INVERSE));
        assert_eq!(
            token(&on, "background@selected"),
            Some(LAYER_SELECTED_INVERSE)
        );
        assert_eq!(
            named(&on, "label").props.style.as_ref().map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_HEADING_SM),
            "a selected tab keeps the body size and steps the weight; the \
             page-heading role would make the tab grow"
        );

        let off = content_switcher_item("a", "List", false);
        assert!(!off.semantics.selected);
        assert_eq!(
            named(&off, "label")
                .props
                .style
                .as_ref()
                .map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_BODY)
        );
    }

    /// Slice-b:26, :32: a 1×16 `$border-subtle` line between adjacent
    /// tabs, hidden beside the selected one. With `a` selected, `a|b`
    /// hides its line and `b|c` draws one; every divider keeps its column
    /// so the tabs do not shift when the selection moves.
    #[test]
    fn a_divider_sits_between_adjacent_tabs_and_hides_beside_the_selected_one() {
        let node = three();
        let keys: Vec<&str> = node.children.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys, ["a", "divider-1", "b", "divider-2", "c"]);
        assert_eq!(
            node.props.columns,
            vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 1.0 },
                TrackSize::FitContent,
                TrackSize::Weight { weight: 1.0 },
            ],
            "tabs share the row equally; dividers take their own width"
        );

        let beside_selected = named(&node, "divider-1");
        let line = named(beside_selected, "line");
        assert_eq!(
            token(line, "background"),
            None,
            "the divider next to the selected tab is hidden"
        );
        assert_eq!(line.constraints.horizontal.min, Some(DIVIDER_WIDTH));
        assert_eq!(line.constraints.vertical.min, Some(DIVIDER_HEIGHT));
        assert_eq!(DIVIDER_WIDTH, 1.0);
        assert_eq!(DIVIDER_HEIGHT, 16.0);

        let between_unselected = named(&node, "divider-2");
        assert_eq!(
            token(named(between_unselected, "line"), "background"),
            Some(BORDER_SUBTLE),
            "the divider between two unselected tabs is drawn"
        );
    }

    fn rect_of(frame: &PetrifiedFrame, suffix: &str) -> Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ending in {suffix}"))
            .rect
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: the switcher row and every item place with a real rect,
    /// none of them outside the row, whichever item is selected — on both
    /// T053 grounds and both tab shapes as well as the original.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for shape in [three(), low_three(), icon_three()] {
            assert_sound_geometry(&petrify_lone(shape));
        }
    }

    fn assert_sound_geometry(frame: &PetrifiedFrame) {
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            assert!(
                !p.paint.truncated,
                "{} was truncated to fit its parent",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
    }

    /// The picture the operator saw: `List` a filled block and `Grid` bare
    /// text, two different widths, and the pair reflowing on every click.
    /// Every tab now places at the same width and the same height, the
    /// tabs fill the group, the divider is centred on the row, and the
    /// group's own edge is not covered by a tab's fill.
    #[test]
    fn every_tab_places_at_the_same_width_and_the_tabs_fill_the_group() {
        let frame = petrify_lone(three());
        let group = rect_of(&frame, "/sw");
        let a = rect_of(&frame, "/sw/a");
        let b = rect_of(&frame, "/sw/b");
        let c = rect_of(&frame, "/sw/c");
        assert!(
            (a.w - b.w).abs() < 0.01 && (b.w - c.w).abs() < 0.01,
            "tabs must share the row equally: {} / {} / {}",
            a.w,
            b.w,
            c.w
        );
        assert!(a.w > 100.0, "tabs fill the group, they do not hug a label");
        assert_eq!(a.h, SIZE_MD);
        assert_eq!(b.h, SIZE_MD);
        assert_eq!(a.y, b.y, "a selected tab sits on the same baseline");
        // The tabs are inset from the group's edge by more than the
        // one-unit stroke, so the group's border is not painted over.
        assert!(
            a.x - group.x > 1.0,
            "first tab covers the group's left edge"
        );
        assert!(a.y - group.y > 1.0, "tabs cover the group's top edge");
        assert!(
            group.x + group.w - (c.x + c.w) > 1.0,
            "last tab covers the group's right edge"
        );
        let line = rect_of(&frame, "/sw/divider-2/line");
        assert_eq!(line.w, DIVIDER_WIDTH);
        assert_eq!(line.h, DIVIDER_HEIGHT);
        let row_mid = b.y + b.h / 2.0;
        let line_mid = line.y + line.h / 2.0;
        assert!(
            (row_mid - line_mid).abs() < 0.01,
            "divider is centred on the row: row mid {row_mid}, line mid {line_mid}"
        );
        assert!(
            b.x + b.w <= line.x + 0.01 && line.x + line.w <= c.x + 0.01,
            "divider sits between its two tabs"
        );
    }

    /// Check F: an enabled item is reachable in focus order; a disabled one
    /// is not.
    #[test]
    fn enabled_items_are_reachable_and_disabled_ones_are_not() {
        let node = content_switcher(
            "sw",
            vec![
                content_switcher_item("a", "List", true),
                crate::component::disabled(content_switcher_item("b", "Grid", false)),
            ],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let a = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/a"))
            .expect("item a is placed");
        let b = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/b"))
            .expect("item b is placed");
        assert!(
            focus.order().iter().any(|o| o == &a.id),
            "enabled item a declares Focus but is not in focus order"
        );
        assert!(
            !focus.order().iter().any(|o| o == &b.id),
            "disabled item b must not be reachable"
        );
    }

    /// Check E: label ink against the item's own resting fill, in both
    /// themes, at both selection states and on both grounds — selected
    /// swaps fill and ink together, so the pair always clears.
    #[test]
    fn item_label_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        type ItemBuild = (fn(&str, &str, bool) -> ViewNode, &'static str);
        let builds: [ItemBuild; 2] = [
            (|k, l, s| content_switcher_item(k, l, s), "high-contrast"),
            (
                |k, l, s| content_switcher_item_low_contrast(k, l, s),
                "low-contrast",
            ),
        ];
        for theme in [crate::token::light(), crate::token::dark()] {
            for selected in [true, false] {
                for (build, ground) in builds {
                    let item = build("a", "List", selected);
                    let bg_name = item
                        .props
                        .tokens
                        .get("background")
                        .expect("item binds a resting background");
                    let bg = color(&theme, bg_name.as_str());
                    let label = named(&item, "label");
                    let fg_name = label
                        .props
                        .tokens
                        .get("foreground")
                        .expect("label binds a foreground");
                    let opacity = label.props.opacity.unwrap_or(1.0);
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{ground} selected={selected} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                        bg_name.as_str()
                    );
                }
            }
        }
    }

    /// Spec 008 FR-027 / slice-b "Sizes": the switcher scales sm 32 / md 40 /
    /// lg 48, and the group's step owns every item's height (the group
    /// re-pins its children), so a mixed-height row cannot happen.
    #[test]
    fn switcher_sizes_scale_and_the_step_re_pins_every_item() {
        /// One case: the step's builder and the height it must land on.
        type Case = (fn(&str, Vec<ViewNode>) -> ViewNode, f32);
        let cases: [Case; 3] = [
            (|key, items| content_switcher_sm(key, items), SIZE_SM),
            (|key, items| content_switcher(key, items), SIZE_MD),
            (|key, items| content_switcher_lg(key, items), SIZE_LG),
        ];
        for (build, step) in cases {
            let node = build(
                "sw",
                vec![
                    content_switcher_item("a", "List", true),
                    content_switcher_item("b", "Grid", false),
                ],
            );
            let frame = petrify_lone(node);
            for key in ["/sw/a", "/sw/b"] {
                let rect = rect_of(&frame, key);
                assert_eq!(
                    rect.h, step,
                    "{key} is not at the switcher's step height {step}"
                );
            }
            // The group is the step plus two units of padding on each
            // edge (`pad(SPACING_01, SPACING_01)`), 36/44/52 total — the
            // geometry the module doc names and
            // `the_group_draws_one_2px_corner_and_the_tab_radius_is_zero`
            // already measures.
            let group = rect_of(&frame, "/sw");
            assert_eq!(
                group.h,
                step + 4.0,
                "/sw is not at the padded step height {step} + 4"
            );
        }
    }

    /// T053's low-contrast ground (slice-b "Variants" and "Anatomy"): the
    /// group sits on `$layer-01` under a `$border-strong` outline, and the
    /// selection is one surface step — the inverse polarity the
    /// high-contrast tab fills with never appears in this ground. Walking
    /// every token of every node is the claim with teeth: re-point one
    /// binding at `layer-selected-inverse` or `text-inverse` and this goes
    /// red.
    #[test]
    fn the_low_contrast_ground_keeps_the_surface_ramp_and_never_inverts() {
        let node = content_switcher_low_contrast(
            "sw",
            vec![
                content_switcher_item_low_contrast("a", "List", true),
                content_switcher_item_low_contrast("b", "Grid", false),
            ],
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_LAYER_TWO));
        assert_eq!(token(&node, "border"), Some(BORDER_STRONG));

        fn walk(node: &ViewNode) {
            for (slot, name) in &node.props.tokens {
                assert_ne!(
                    name.as_str(),
                    LAYER_SELECTED_INVERSE,
                    "{slot} on `{}` flips the polarity in the low-contrast ground",
                    node.key
                );
                assert_ne!(
                    name.as_str(),
                    TEXT_INVERSE,
                    "{slot} on `{}` flips the ink in the low-contrast ground",
                    node.key
                );
            }
            for child in &node.children {
                walk(child);
            }
        }
        walk(&node);
        assert_eq!(
            token(named(&node, "a"), "background"),
            Some(LAYER_SELECTED),
            "the selected tab is the one-step-up surface"
        );
    }

    /// The low-contrast selection carries three channels and none of them
    /// is hue: a surface step against the unselected fill,
    /// `Semantics.selected`, and the same weight bump the high-contrast tab
    /// uses. Delete any one and the state is colour alone.
    #[test]
    fn the_low_contrast_selection_is_a_step_a_word_and_a_weight_not_hue() {
        let on = content_switcher_item_low_contrast("a", "List", true);
        let off = content_switcher_item_low_contrast("b", "Grid", false);
        assert!(on.semantics.selected);
        assert!(!off.semantics.selected);
        assert_eq!(token(&on, "background"), Some(LAYER_SELECTED));
        assert_eq!(token(&off, "background"), Some(SURFACE_LAYER_TWO));
        assert_eq!(
            named(&on, "label").props.style.as_ref().map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_HEADING_SM)
        );
        assert_eq!(
            named(&off, "label")
                .props
                .style
                .as_ref()
                .map(|n| n.as_str()),
            Some(super::TYPOGRAPHY_BODY)
        );
        // The ink stays dark-side in both states: the ground never inverts.
        assert_eq!(token(named(&on, "label"), "foreground"), Some(TEXT_PRIMARY));
        assert_eq!(token(named(&off, "label"), "foreground"), Some(TEXT_MUTED));
    }

    /// Slice-b "Formatting": the icon-only tab is a square — "each tab's
    /// width equals its height" — carrying a glyph where the label goes.
    /// The picture is not the name: the accessible name is in
    /// `Semantics`, and no text node exists in the tab.
    #[test]
    fn icon_only_tabs_are_squares_carrying_a_glyph_and_no_text() {
        let node = content_switcher_item_icon("a", "List", super::IconMark::Search, true);
        for axis in [&node.constraints.horizontal, &node.constraints.vertical] {
            assert_eq!(axis.min, Some(SIZE_MD));
            assert_eq!(axis.max, Some(SIZE_MD));
        }
        assert_eq!(node.semantics.label.as_deref(), Some("List"));
        assert!(node.semantics.selected);
        assert_eq!(named(&node, "glyph").kind, NodeKind::Canvas);

        fn any_text(node: &ViewNode) -> bool {
            node.kind == NodeKind::Text || node.children.iter().any(|child| any_text(child))
        }
        assert!(!any_text(&node), "an icon tab draws no label text");
    }

    /// Slice-b's Size table: "sm/md icon 16px, lg icon 20px" — the glyph
    /// box steps with the tab (16 is `IconBox::Glyph`, 20
    /// `IconBox::Header`).
    #[test]
    fn the_icon_glyph_scales_with_the_step() {
        for (size, extent) in [
            (super::SwitcherSize::Sm, 16.0),
            (super::SwitcherSize::Md, 16.0),
            (super::SwitcherSize::Lg, 20.0),
        ] {
            let node = content_switcher_item_icon_with_size(
                "a",
                "List",
                super::IconMark::Search,
                true,
                size,
            );
            let glyph = named(&node, "glyph");
            assert_eq!(
                glyph.constraints.horizontal.min,
                Some(extent),
                "glyph width at {size:?}"
            );
            assert_eq!(
                glyph.constraints.vertical.min,
                Some(extent),
                "glyph height at {size:?}"
            );
        }
    }

    /// T032's ramp carries both new shapes: sm 32 / md 40 / lg 48, the
    /// group re-pinning every tab to its own step — and an icon row of
    /// squares stays a row of squares at every step.
    #[test]
    fn the_new_shapes_place_at_three_steps_and_squares_stay_square() {
        /// One case: the step's builder and the height it must land on.
        type Group = (fn(&str, Vec<ViewNode>) -> ViewNode, f32);
        // The low-contrast ground, text tabs: height only.
        let text_groups: [Group; 3] = [
            (
                |k, items| content_switcher_low_contrast_sm(k, items),
                SIZE_SM,
            ),
            (|k, items| content_switcher_low_contrast(k, items), SIZE_MD),
            (
                |k, items| content_switcher_low_contrast_lg(k, items),
                SIZE_LG,
            ),
        ];
        for (build, step) in text_groups {
            let node = build(
                "sw",
                vec![
                    content_switcher_item_low_contrast("a", "List", true),
                    content_switcher_item_low_contrast("b", "Grid", false),
                ],
            );
            let frame = petrify_lone(node);
            for key in ["/sw/a", "/sw/b"] {
                assert_eq!(rect_of(&frame, key).h, step, "{key} misses step {step}");
            }
        }
        // The icon ground, square tabs: height *and* width.
        /// The builder, the item step, and the height both must land on.
        type IconGroup = (
            fn(&str, Vec<ViewNode>) -> ViewNode,
            super::SwitcherSize,
            f32,
        );
        let icon_groups: [IconGroup; 3] = [
            (
                |k, items| content_switcher_icon_only_sm(k, items),
                super::SwitcherSize::Sm,
                SIZE_SM,
            ),
            (
                |k, items| content_switcher_icon_only(k, items),
                super::SwitcherSize::Md,
                SIZE_MD,
            ),
            (
                |k, items| content_switcher_icon_only_lg(k, items),
                super::SwitcherSize::Lg,
                SIZE_LG,
            ),
        ];
        for (build, size, step) in icon_groups {
            let node = build(
                "sw",
                vec![
                    content_switcher_item_icon_with_size(
                        "a",
                        "List",
                        super::IconMark::Menu,
                        true,
                        size,
                    ),
                    content_switcher_item_icon_with_size(
                        "b",
                        "Grid",
                        super::IconMark::Search,
                        false,
                        size,
                    ),
                ],
            );
            let frame = petrify_lone(node);
            for key in ["/sw/a", "/sw/b"] {
                let rect = rect_of(&frame, key);
                assert_eq!(rect.h, step, "{key} misses step {step}");
                assert_eq!(rect.w, rect.h, "{key} is not square at step {step}");
            }
        }
    }
}
