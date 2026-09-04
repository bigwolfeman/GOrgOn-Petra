//! Carbon Structured list (slice-e).
//!
//! A definition-style table, not [`super::list`] (ordered/unordered
//! markers). Anatomy (`_structured-list.scss`):
//! 1. Container — `display:table` → [`Role::Table`].
//! 2. Header row — [`Role::Row`] of [`Role::Cell`]s. Not interactive.
//! 3. Data rows — [`structured_list_row`]: [`Role::Row`], selectable,
//!    `Semantics.selected` never colour alone — Carbon's own anatomy draws
//!    `RadioButtonChecked` / `RadioButton` beside the row (slice-e, Icons:
//!    "the only two icons this component ever renders"), so a row with a
//!    fill change and nothing else is not an approximation, it is a
//!    channel Carbon specifies and this file used to drop. See
//!    [`with_selection_mark`].
//!
//! Padding is `$spacing-05` (16) inline on each cell (see
//! `cell_padding_inline`), not a single inset on the row: each row is a
//! `Grid` of shared-width columns so sibling rows resolve identical pixel
//! columns (the `data_table.rs` fix, `31-structured-list.png` had the same
//! defect). Default row min-height is Carbon's 60. The 10-colour tag set
//! is unrelated; this file does not invent hues.

use super::icon::{IconMark, icon};
use super::stack;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05,
    SURFACE_BASE, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, InsetRefs, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
};

/// Carbon default structured-list row height (style page Size table).
const ROW_HEIGHT: f32 = 60.0;

const _: () = assert!(ROW_HEIGHT == 60.0);

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Structured list: header + rows under [`Role::Table`].
pub fn structured_list(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let mut children = vec![plain_row("header", header)];
    children.extend(rows.into_iter().map(ensure_row));
    let mut node = stack(key, Axis::Vertical, None, children);
    // Every row is a `Grid` (see `row_shell`) whose equal-weight column
    // tracks resolve against whatever width `place` offers it. Without
    // `Stretch` here, each row measures at its own content width and the
    // picture (`31-structured-list.png`) showed exactly `data_table.rs`'s
    // pre-fix defect one level up: "NameRole" cells touching with no gap,
    // three ragged row widths, and "kernel"/"runtime" not lining up under
    // "petra"/"layout". `Stretch` offers every row the list's own width,
    // so the Grid tracks resolve identical pixel columns row to row — the
    // same fix `data_table` made (commit 8778a83).
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Table),
        ..Semantics::default()
    };
    node
}

/// One selectable data row. Interactive, [`Role::Row`], cells stamped
/// [`Role::Cell`]. `selected` is a declared fact plus the four-fill set
/// [`super::list_row`] pioneered, plus [`with_selection_mark`]'s icon —
/// Carbon's own second channel, not [`super::list_row`]'s.
pub fn structured_list_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = row_label(key.as_str(), &cells);
    let cells = with_selection_mark(cells, selected);
    let mut node = row_shell(key, cells);
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    let mut node = node.interactive(Role::Row, label, ROW_INTENTS);
    node.semantics.selected = selected;
    node
}

/// Wraps the row's leading cell with a selection mark, Carbon's own second
/// channel for this component (slice-e Icons: `RadioButtonChecked` /
/// `RadioButton`, "the only two icons this component ever renders").
///
/// [`IconMark`] has no radio-pair glyph — it is the vocabulary's existing
/// stand-in for "this is the on state" ([`super::tile::selectable_tile`],
/// the toggle, radio, checkbox, [`super::progress_indicator`]'s complete
/// step), not a new SVG path, and reusing it here is the same move those
/// made rather than new geometry for a mark this vocabulary already has
/// one of.
///
/// The mark sits at the row's leading edge — Carbon's own
/// `enable-v12-structured-list-visible-icons` placement, the one slice-e
/// calls *visible at all times* rather than the legacy right-edge
/// placement that stays `fill: transparent` until hover/checked, which is
/// the same colour-only failure this fix exists to close.
///
/// The mark's footprint is reserved on **every** row, selected or not — a
/// same-size transparent spacer stands in when it is absent
/// ([`selection_mark`]). An icon that only appeared on the selected row
/// would make row width depend on selection, which is exactly the
/// `31-structured-list.png` raggedness [`row_shell`]'s own doc already
/// fixed once, one level deeper (inside a cell instead of across a row).
/// The header row does not call this — Carbon's icon is "(Selectable
/// only)", tbody rows alone — so the header's own leading column does not
/// reserve this width; `row_shell`'s shared `Weight` tracks still keep
/// every column's own boundary identical column-to-column, header
/// included, because track width comes from the Grid's weight split, not
/// from a cell's content.
fn with_selection_mark(cells: Vec<ViewNode>, selected: bool) -> Vec<ViewNode> {
    let mut cells = cells.into_iter();
    let Some(first) = cells.next() else {
        return Vec::new();
    };
    let mut lead = stack(
        "lead",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![selection_mark(selected), first],
    );
    lead.props.align = Some(Align::Center);
    std::iter::once(lead).chain(cells).collect()
}

/// The mark itself: [`IconMark::Check`] when selected, a same-size
/// transparent spacer when not. Sized off the icon's own constructed
/// constraints rather than a duplicated constant — the same idiom
/// [`super::progress_indicator::complete_mark`] uses to centre its own
/// check without importing [`super::icon`]'s private `SIZE`.
fn selection_mark(selected: bool) -> ViewNode {
    let mark = icon("mark", IconMark::Check);
    if selected {
        return mark;
    }
    let w = mark.constraints.horizontal.min.unwrap_or(0.0);
    let h = mark.constraints.vertical.min.unwrap_or(0.0);
    let mut spacer = stack("mark", Axis::Horizontal, None, vec![]);
    spacer.constraints.horizontal = AxisConstraint {
        min: Some(w),
        max: Some(w),
        priority: 0,
    };
    spacer.constraints.vertical = AxisConstraint {
        min: Some(h),
        max: Some(h),
        priority: 0,
    };
    spacer
}

fn plain_row(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let mut node = row_shell(key, cells);
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node
}

/// One row's cells, laid out as a `Grid` of `ncols` equal [`TrackSize::Weight`]
/// columns rather than a bare `Axis::Horizontal` stack.
///
/// The picture (`31-structured-list.png`) showed `NameRole` with no gap and
/// `kernel`/`runtime` in row 1 not lining up under `petra`/`layout` in row
/// 2 — a horizontal stack sizes each cell to its own text, so column 2's x
/// depends on how wide column 1's *own row* happened to be. A `Grid` with
/// shared column tracks fixes both at once: every cell in column *i*
/// resolves the same width, in every row, because they are the same track.
/// `structured_list` gives every row's `Grid` the identical `Stretch`-
/// offered width, so the tracks resolve to identical pixels row to row.
/// Carbon gives no per-column width for this component either, so an equal
/// split is the least-invented default — the same reasoning `data_table.rs`
/// used.
fn row_shell(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let ncols = cells.len().max(1);
    let cells = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| as_cell(i, cell))
        .collect();
    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }; ncols],
            rows: vec![TrackSize::Weight { weight: 1.0 }],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(cells);
    node.constraints.vertical.min = Some(ROW_HEIGHT);
    node
}

/// `padding-inline: $spacing-05` on both sides. MEASURED (SCSS,
/// `padding--data-structured-list` mixin, used by
/// `.cds--structured-list--selection`): "selectable-row padding is
/// `$spacing-05`(16px) on both inline sides — a different, simpler rule
/// than the plain padding-td/padding-th mixins above"
/// (`.agents/research/08-25-2026/Carbon-Component-Inventory/slice-e.md`
/// line 104). Applied per cell, matching `data_table.rs`'s
/// `cell_padding_inline`: without it the Grid's shared columns still touch
/// at the seam, which is the second half of the `NameRole` defect.
fn cell_padding_inline() -> InsetRefs {
    InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    }
}

fn as_cell(index: usize, node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Cell) {
        return node;
    }
    let mut wrap = stack(format!("c{index}"), Axis::Horizontal, None, vec![node]);
    wrap.props.padding = Some(cell_padding_inline());
    wrap.props.align = Some(Align::Center);
    wrap.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    wrap
}

fn ensure_row(node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Row) {
        node
    } else {
        plain_row(node.key.clone(), vec![node])
    }
}

fn row_label(key: &str, cells: &[ViewNode]) -> String {
    let from_cells: Vec<String> = cells
        .iter()
        .map(collect_text)
        .filter(|s| !s.is_empty())
        .collect();
    if from_cells.is_empty() {
        key.to_string()
    } else {
        from_cells.join(" ")
    }
}

fn collect_text(node: &ViewNode) -> String {
    if let Some(text) = node.props.text.as_ref().filter(|s| !s.is_empty()) {
        return text.clone();
    }
    if let Some(label) = node.semantics.label.as_ref().filter(|s| !s.is_empty()) {
        return label.clone();
    }
    node.children
        .iter()
        .map(|child| collect_text(child))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{ROW_HEIGHT, SPACING_05, structured_list, structured_list_row};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn has_canvas(node: &ViewNode) -> bool {
        node.kind == NodeKind::Canvas || node.children.iter().any(|child| has_canvas(child))
    }

    /// `31-structured-list.png` showed `NameRole` with no gap between the
    /// cells because padding lived on the whole row (a single inset around
    /// all cells at once) instead of between them. The fix moved
    /// `$spacing-05` to each cell's own inline padding (`cell_padding_inline`,
    /// mirroring `data_table.rs`'s `cell_padding_inline`), so this checks the
    /// cell, not the row.
    fn cell_padding_inline(cell: &ViewNode) -> Option<(&str, &str)> {
        let pad = cell.props.padding.as_ref()?;
        Some((pad.left.as_ref()?.as_str(), pad.right.as_ref()?.as_str()))
    }

    #[test]
    fn structured_list_is_a_table_of_rows_and_cells() {
        let node = structured_list(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![structured_list_row(
                "r0",
                vec![text("p", "Alpha"), text("c", "12")],
                false,
            )],
        );
        assert_eq!(node.semantics.role, Some(Role::Table));
        assert!(node.interactions.is_empty());
        assert_ne!(node.semantics.role, Some(Role::List));

        let header = child(&node, "header");
        assert_eq!(header.semantics.role, Some(Role::Row));
        assert!(header.interactions.is_empty());
        assert_eq!(header.constraints.vertical.min, Some(ROW_HEIGHT));
        assert_eq!(ROW_HEIGHT, 60.0);
        assert_eq!(header.kind, NodeKind::Grid, "shared columns need a Grid");
        assert_eq!(header.props.align, Some(crate::geom::Align::Stretch));
        assert_eq!(child(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(header, "c1").semantics.role, Some(Role::Cell));
        assert_eq!(
            cell_padding_inline(child(header, "c0")),
            Some((SPACING_05, SPACING_05)),
            "adjacent cells need inline padding or they touch"
        );

        let row = child(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(child(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(row, "c1").semantics.role, Some(Role::Cell));
        assert_eq!(
            cell_padding_inline(child(row, "c1")),
            Some((SPACING_05, SPACING_05))
        );
    }

    #[test]
    fn structured_list_row_declares_selected_and_is_interactive() {
        let on = structured_list_row("r0", vec![text("p", "Alpha"), text("c", "12")], true);
        assert_eq!(on.semantics.role, Some(Role::Row));
        assert!(on.semantics.selected);
        assert_eq!(on.semantics.label.as_deref(), Some("Alpha 12"));
        assert!(on.interactions.contains(&Interaction::Click));
        assert!(on.interactions.contains(&Interaction::Focus));
        assert!(on.interactions.contains(&Interaction::Hover));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "border"), Some(BORDER_SUBTLE));
        assert_eq!(on.kind, NodeKind::Grid, "shared columns need a Grid");
        assert_eq!(
            cell_padding_inline(child(&on, "c0")),
            Some((SPACING_05, SPACING_05))
        );
        assert_eq!(on.constraints.vertical.min, Some(ROW_HEIGHT));

        let off = structured_list_row("r0", vec![text("p", "Alpha")], false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }

    /// A5 (2026-09-04): Carbon's own anatomy draws `RadioButtonChecked` /
    /// `RadioButton` beside a selected row (slice-e Icons); a fill change
    /// alone is a channel this file used to drop, not an approximation of
    /// one Carbon never specified. Selected rows must carry a canvas mark;
    /// unselected rows must not — the mark is the on-state glyph, not a
    /// permanent decoration.
    #[test]
    fn structured_list_row_selection_carries_a_second_channel() {
        let on = structured_list_row("r0", vec![text("p", "Alpha"), text("c", "12")], true);
        assert!(
            has_canvas(&on),
            "Carbon's own anatomy draws RadioButtonChecked beside a \
             selected row (slice-e Icons); selection must not be fill-tone \
             alone"
        );
        let off = structured_list_row("r0", vec![text("p", "Alpha"), text("c", "12")], false);
        assert!(
            !has_canvas(&off),
            "the mark is the on-state glyph, not a permanent decoration"
        );
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

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn fixture() -> ViewNode {
        structured_list(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![
                structured_list_row("r0", vec![text("p0", "Basic"), text("c0", "$12")], false),
                structured_list_row("r1", vec![text("p1", "Pro"), text("c1", "$24")], true),
            ],
        )
    }

    /// Check C/D: the header row and its own row-divider `border`
    /// (slice-e, "`.cds--structured-list-row` gets a `1px solid
    /// $border-subtle` top divider ... the tbody's last row additionally
    /// gets a bottom divider") is not a separate childless swatch the way
    /// the Accordion divider was — the border lives directly on the row
    /// node's own `Props.tokens`, not on a zero-sized child, so there is
    /// no class-two shape here to petrify around. This is the frame-level
    /// check across the header and both data rows, one selected.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(fixture());
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
            if let Some(parent_idx) = p.parent {
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
            }
        }
    }

    /// A5 (2026-09-04): [`with_selection_mark`]'s own doc names the risk —
    /// an icon that only appeared on the selected row would make that
    /// row's leading-cell content depend on selection, the same
    /// `31-structured-list.png` raggedness [`row_shell`]'s doc already
    /// fixed once, one level deeper. The reserved spacer closes it: the
    /// leading label ("Basic" in the unselected row, "Pro" in the
    /// selected one) must land at the same x in both, because the mark's
    /// footprint — icon or spacer — is the same size either way.
    #[test]
    fn selection_mark_reserves_the_same_width_selected_or_not() {
        let frame = petrify_lone(fixture());
        let label_x = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"))
                .rect
                .x
        };
        assert_eq!(
            label_x("/r0/c0/lead/p0"),
            label_x("/r1/c0/lead/p1"),
            "the leading label must start at the same x whether its own \
             row is selected or not"
        );
    }

    /// Check F: a selectable data row declares `Focus` and is reachable;
    /// the header row declares no interactions at all (slice-e: "no
    /// interactive states because it is not operable by a mouse or
    /// keyboard") and is not.
    #[test]
    fn data_rows_are_reachable_and_the_header_row_is_not() {
        let frame = petrify_lone(fixture());
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        for (suffix, should_be_focusable) in [("/header", false), ("/r0", true), ("/r1", true)] {
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"));
            let reachable = order.iter().any(|o| o == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{suffix}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the header cells against the page ground (the header binds
    /// no `background` of its own), and the data-row cells against each
    /// row's own resting fill, in both themes.
    #[test]
    fn row_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let page_bg = color(&theme, SURFACE_BASE);
            let node = fixture();

            fn cell_text<'a>(row: &'a ViewNode, cell_key: &str) -> &'a ViewNode {
                let first = row
                    .children
                    .iter()
                    .find(|c| c.key.as_str() == cell_key)
                    .unwrap_or_else(|| panic!("missing cell {cell_key}"))
                    .children
                    .first()
                    .unwrap_or_else(|| panic!("cell {cell_key} carries no text child"));
                // A5: the leading cell now wraps its label behind the
                // selection mark (`with_selection_mark`'s own `lead` node,
                // children `[mark, label]`) — reach past it to the label.
                // Other cells are untouched, so `first` is already the
                // label there.
                if first.key.as_str() == "lead" {
                    first
                        .children
                        .last()
                        .unwrap_or_else(|| panic!("cell {cell_key}'s lead carries no label"))
                } else {
                    first
                }
            }

            let header = node
                .children
                .iter()
                .find(|c| c.key.as_str() == "header")
                .expect("header row is present");
            for cell_key in ["c0", "c1"] {
                let text_node = cell_text(header, cell_key);
                let fg_name = text_node
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("header {cell_key} binds a foreground"));
                let opacity = text_node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(page_bg);
                let ratio = fg.contrast_ratio(page_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "header {cell_key} at {ratio:.2}:1 against page ground fails AA \
                     {MIN_TEXT_CONTRAST}:1"
                );
            }

            for row_key in ["r0", "r1"] {
                let row = node
                    .children
                    .iter()
                    .find(|c| c.key.as_str() == row_key)
                    .unwrap_or_else(|| panic!("missing row {row_key}"));
                let row_bg_name = row
                    .props
                    .tokens
                    .get("background")
                    .expect("data row binds a resting background");
                let row_bg = color(&theme, row_bg_name.as_str());
                for cell_key in ["c0", "c1"] {
                    let text_node = cell_text(row, cell_key);
                    let fg_name = text_node
                        .props
                        .tokens
                        .get("foreground")
                        .unwrap_or_else(|| panic!("{row_key} {cell_key} binds a foreground"));
                    let opacity = text_node.props.opacity.unwrap_or(1.0);
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(row_bg);
                    let ratio = fg.contrast_ratio(row_bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "{row_key} {cell_key} at {ratio:.2}:1 against {} fails AA \
                         {MIN_TEXT_CONTRAST}:1",
                        row_bg_name.as_str()
                    );
                }
            }
        }
    }
}
