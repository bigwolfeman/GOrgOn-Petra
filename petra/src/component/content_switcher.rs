//! Carbon Content switcher (slice-b).
//!
//! Anatomy (docs + `_content-switcher.scss`), high-contrast default:
//! 1. [`content_switcher`] — the group: one bordered row, radius 4
//!    (`shape.corner-sm`, via `CornerRole::Grouping`), 1px [`BORDER_SUBTLE`] outline, every tab the same
//!    width (Carbon's `.cds--content-switcher-btn` is `inline-size: 100%`
//!    inside a flex row, so N tabs share the row N ways — `08-content-
//!    switcher.png` shows two tabs at exactly half each).
//! 2. [`content_switcher_item`] — one tab button, height 40 ([`SIZE_MD`]),
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
//! Icon-only tabs are omitted: the icon vocabulary has no glyph for them.
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

use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED_INVERSE, SIZE_MD, SPACING_01, SPACING_03,
    SPACING_05, SURFACE_BASE, TEXT_INVERSE, TEXT_MUTED, TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING_SM, t,
};
use super::{pad, pin_block, stack, swatch};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Behaviour, Intent, Interaction, Justify, Key, NodeKind, Phase, Props, Role, Semantics,
    TrackSize, ViewNode,
};

const _: () = assert!(SIZE_MD == 40.0);

const ITEM_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Divider between two tabs: 1 wide, 16 tall (`_content-switcher.scss`
/// `::before`, `block-size: convert.to-rem(16px)`).
const DIVIDER_WIDTH: f32 = 1.0;
const DIVIDER_HEIGHT: f32 = 16.0;

/// The switcher group. `Role::TabList`, no interactions of its own.
///
/// A single-row `Grid`: every tab sits in a `Weight { 1.0 }` column so
/// all of them share the row equally, and a `FitContent` column between
/// each adjacent pair holds the divider. `align: Stretch` is what makes a
/// tab fill its column; a `Stack` would let each tab hug its own label,
/// which is how `List` and `Grid` came to be two different widths.
pub fn content_switcher(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut columns = Vec::with_capacity(items.len() * 2);
    let mut children = Vec::with_capacity(items.len() * 2);
    let mut previous_selected = false;
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            let hidden = previous_selected || item.semantics.selected;
            columns.push(TrackSize::FitContent);
            children.push(divider(i, hidden));
        }
        previous_selected = item.semantics.selected;
        columns.push(TrackSize::Weight { weight: 1.0 });
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
    // `CornerRole::Grouping`. The group's real height is `SIZE_MD` (40) plus
    // two units of padding on each edge (44 total, per the module doc); `SIZE_MD`
    // alone is already well clear of Grouping's 8-unit half-edge threshold, so
    // it stands in for the padded total without inventing a new constant.
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(CornerRole::Grouping, SIZE_MD)),
    );
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
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
/// Height is Carbon md 40. Label padding is `$spacing-05` inline. The box
/// is the same in both states — only the fill, the ink and the type weight
/// change, so the pair never reflows when the selection moves.
pub fn content_switcher_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
) -> ViewNode {
    let label = label.into();
    let mut label_node = super::text::text("label", label.clone());
    label_node.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING_SM
    } else {
        TYPOGRAPHY_BODY
    }));
    label_node.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_INVERSE } else { TEXT_MUTED }),
    );

    let mut node = stack(key, Axis::Horizontal, None, vec![label_node]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad(SPACING_05, SPACING_03));
    node.props.tokens.insert(
        "background".into(),
        t(if selected {
            LAYER_SELECTED_INVERSE
        } else {
            SURFACE_BASE
        }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    // Carbon keeps the selected tab's inverse fill under the pointer.
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED_INVERSE));
    node.props.tokens.insert(
        "background@selected-hover".into(),
        t(LAYER_SELECTED_INVERSE),
    );

    let mut node = node
        .with_constraints(pin_block(SIZE_MD))
        .interactive(Role::Button, label, ITEM_INTENTS)
        // Spec 010: exclusive among siblings in the switcher.
        .with_behaviour(Behaviour {
            intent: Intent::Select,
            phase: Phase::OnRelease,
        })
        .owning_its_text();
    node.semantics.selected = selected;
    node
}

#[cfg(test)]
mod tests {
    use super::{
        BORDER_SUBTLE, DIVIDER_HEIGHT, DIVIDER_WIDTH, LAYER_SELECTED_INVERSE, SIZE_MD,
        content_switcher, content_switcher_item,
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
    /// none of them outside the row, whichever item is selected.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(three());
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
    /// themes, at both selection states — selected swaps to the inverse
    /// ground and the inverse ink together.
    #[test]
    fn item_label_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for selected in [true, false] {
                let item = content_switcher_item("a", "List", selected);
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
                    "selected={selected} at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    bg_name.as_str()
                );
            }
        }
    }
}
