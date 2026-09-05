//! Carbon Structured list (slice-e).
//!
//! A definition-style table, not [`super::list`] (ordered/unordered
//! markers). Anatomy (`_structured-list.scss` + `31-structured-list.png`):
//! 1. Container — `display:table` → [`Role::Table`]. No box: Carbon draws
//!    nothing around a structured list.
//! 2. Header row — [`Role::Row`] of [`Role::Cell`]s. Not interactive, no
//!    fill, **no rule of its own**; text `$text-primary` in
//!    `heading-compact-01` (`TYPOGRAPHY_HEADING_SM`). Cell padding is the
//!    `padding-th` mixin: 16 top, 8 bottom.
//! 3. Data rows — [`structured_list_row`]: [`Role::Row`], selectable,
//!    `Semantics.selected` never colour alone — Carbon's own anatomy draws
//!    `RadioButtonChecked` / `RadioButton` beside the row (slice-e, Icons:
//!    "the only two icons this component ever renders"); see
//!    [`with_selection_mark`]. Cell padding is the `padding-td` mixin: 16
//!    top, 24 bottom, so a one-line row comes out at Carbon's 60 without a
//!    pinned height, and the text sits in the upper part of the row the
//!    way the reference shows it, not centred.
//! 4. Rules — every data row binds `border-top` ([`BORDER_SUBTLE`]):
//!    slice-e:98, "`.cds--structured-list-row` gets a `1px solid
//!    $border-subtle` top divider". The tbody's **last** row also binds
//!    `border-bottom`, which [`structured_list`] adds because only the
//!    container knows which row is last. One line per boundary; the old
//!    four-sided `border` on every row is the grid-of-boxes defect
//!    (`.agents/carbon-waves/ROUND2-DEFECTS.md` row 31).
//!
//! Inline padding is `$spacing-05` (16) on each cell (see
//! [`cell_padding`]), not a single inset on the row: each row is a `Grid`
//! of shared-width columns so sibling rows resolve identical pixel columns
//! (the `data_table.rs` fix). The 10-colour tag set is unrelated; this file
//! does not invent hues.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::as_compact_heading;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05,
    SPACING_06, SURFACE_BASE, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, InsetRefs, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
};

/// Carbon default structured-list row height (style page Size table). Not
/// pinned on the row: it is what `padding-td` (16 + 24) plus one 20-unit
/// line of `body-01` adds up to, and the row grows with a second line.
const ROW_HEIGHT: f32 = 60.0;

const _: () = assert!(ROW_HEIGHT == 60.0);

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Which padding mixin a cell takes.
#[derive(Clone, Copy)]
enum CellKind {
    /// `padding-th`: 16 top, 8 bottom.
    Header,
    /// `padding-td`: 16 top, 24 bottom.
    Data,
}

/// Structured list: header + rows under [`Role::Table`]. The last data row
/// gets the tbody's closing rule.
pub fn structured_list(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let mut rows: Vec<ViewNode> = rows.into_iter().map(ensure_row).collect();
    if let Some(last) = rows.last_mut() {
        last.props
            .tokens
            .insert("border-bottom".into(), t(BORDER_SUBTLE));
    }
    let mut children = vec![header_row("header", header)];
    children.extend(rows);
    let mut node = stack(key, Axis::Vertical, None, children);
    // Every row is a `Grid` (see `row_shell`) whose equal-weight column
    // tracks resolve against whatever width `place` offers it. `Stretch`
    // offers every row the list's own width, so the Grid tracks resolve
    // identical pixel columns row to row.
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
/// Carbon's own second channel.
pub fn structured_list_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = row_label(key.as_str(), &cells);
    let cells = with_selection_mark(cells, selected);
    let mut node = row_shell(key, cells, CellKind::Data);
    for (slot, token) in [
        ("background", SURFACE_BASE),
        ("background@hover", LAYER_HOVER),
        ("background@selected", LAYER_SELECTED),
        ("background@selected-hover", LAYER_SELECTED_HOVER),
    ] {
        node.props.tokens.insert(slot.into(), t(token));
    }
    node.props
        .tokens
        .insert("border-top".into(), t(BORDER_SUBTLE));
    let mut node = node.interactive(Role::Row, label, ROW_INTENTS);
    node.semantics.selected = selected;
    node
}

/// Wraps the row's leading cell with a selection mark, Carbon's own second
/// channel for this component (slice-e Icons: `RadioButtonChecked` /
/// `RadioButton`, "the only two icons this component ever renders").
///
/// [`IconMark`] has no radio-pair glyph — [`IconMark::Check`] is the
/// vocabulary's existing stand-in for "this is the on state", drawn in
/// [`IconTone::Primary`] (Carbon's `$icon-primary`, the fill its SCSS gives
/// the checked icon). The first version drew it in the default on-accent
/// tone, which on the row's own layer measured 1.44:1 and was a mark
/// nobody could see.
///
/// The mark sits at the row's leading edge — Carbon's own
/// `enable-v12-structured-list-visible-icons` placement, the one slice-e
/// calls *visible at all times*.
///
/// The mark's footprint is reserved on **every** row, selected or not — a
/// same-size transparent spacer stands in when it is absent
/// ([`selection_mark`]), so row width does not depend on selection. The
/// header row does not call this — Carbon's icon is "(Selectable only)".
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

/// The mark itself: [`IconMark::Check`] in the primary icon tone when
/// selected, a same-size transparent spacer when not.
fn selection_mark(selected: bool) -> ViewNode {
    let mark = icon_toned("mark", IconMark::Check, IconTone::Primary);
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

/// The column header row: compact-heading text, no fill, no rule.
fn header_row(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let cells = cells.into_iter().map(as_compact_heading).collect();
    let mut node = row_shell(key, cells, CellKind::Header);
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node
}

/// One row's cells, laid out as a `Grid` of `ncols` equal [`TrackSize::Weight`]
/// columns rather than a bare `Axis::Horizontal` stack, so every cell in
/// column *i* resolves the same width in every row.
fn row_shell(key: impl Into<Key>, cells: Vec<ViewNode>, kind: CellKind) -> ViewNode {
    let ncols = cells.len().max(1);
    let cells = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| as_cell(i, cell, kind))
        .collect();
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }; ncols],
            rows: vec![TrackSize::FitContent],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(cells)
}

/// Inline `$spacing-05` on both sides (MEASURED, `padding--data-structured-list`
/// mixin, slice-e:104: "selectable-row padding is `$spacing-05`(16px) on
/// both inline sides"); block from the `padding-th` / `padding-td` mixins
/// (slice-e:100-101): header 16 top / 8 bottom, data 16 top / 24 bottom.
fn cell_padding(kind: CellKind) -> InsetRefs {
    let bottom = match kind {
        CellKind::Header => SPACING_03,
        CellKind::Data => SPACING_06,
    };
    InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        top: Some(t(SPACING_05)),
        bottom: Some(t(bottom)),
    }
}

fn as_cell(index: usize, node: ViewNode, kind: CellKind) -> ViewNode {
    if node.semantics.role == Some(Role::Cell) {
        return node;
    }
    let mut wrap = stack(format!("c{index}"), Axis::Horizontal, None, vec![node]);
    wrap.props.padding = Some(cell_padding(kind));
    // Top-aligned: Carbon's cell is padded, not centred, and the asymmetric
    // 16/24 block padding is what puts the text where the reference has it.
    wrap.props.align = Some(Align::Start);
    wrap.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    wrap
}

/// A data row handed in as a bare node: a non-selectable row with the data
/// padding and its top rule.
fn ensure_row(node: ViewNode) -> ViewNode {
    if node.semantics.role == Some(Role::Row) {
        return node;
    }
    let mut row = row_shell(node.key.clone(), vec![node], CellKind::Data);
    row.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    row.props
        .tokens
        .insert("border-top".into(), t(BORDER_SUBTLE));
    row
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
    use super::{
        ROW_HEIGHT, SPACING_03, SPACING_05, SPACING_06, structured_list, structured_list_row,
    };
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED, TYPOGRAPHY_HEADING_SM};
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

    /// The four edges of a cell's padding, as token names.
    fn cell_padding(cell: &ViewNode) -> Option<[&str; 4]> {
        let pad = cell.props.padding.as_ref()?;
        Some([
            pad.left.as_ref()?.as_str(),
            pad.top.as_ref()?.as_str(),
            pad.right.as_ref()?.as_str(),
            pad.bottom.as_ref()?.as_str(),
        ])
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
        assert_eq!(header.kind, NodeKind::Grid, "shared columns need a Grid");
        assert_eq!(header.props.align, Some(crate::geom::Align::Stretch));
        assert_eq!(child(header, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(header, "c1").semantics.role, Some(Role::Cell));
        assert_eq!(
            cell_padding(child(header, "c0")),
            Some([SPACING_05, SPACING_05, SPACING_05, SPACING_03]),
            "header cells take `padding-th`: 16 top, 8 bottom, 16 inline"
        );

        let row = child(&node, "r0");
        assert_eq!(row.semantics.role, Some(Role::Row));
        assert_eq!(child(row, "c0").semantics.role, Some(Role::Cell));
        assert_eq!(child(row, "c1").semantics.role, Some(Role::Cell));
        assert_eq!(
            cell_padding(child(row, "c1")),
            Some([SPACING_05, SPACING_05, SPACING_05, SPACING_06]),
            "data cells take `padding-td`: 16 top, 24 bottom, 16 inline"
        );
    }

    /// Row 31, round 2 ("not in carbon style"): the header is compact
    /// heading text with no fill and no rule; each data row draws one rule
    /// above itself and the last one closes the body with a rule below;
    /// nothing draws a four-sided box.
    #[test]
    fn rows_draw_one_top_rule_the_last_closes_and_the_header_is_bare() {
        let node = structured_list(
            "plans",
            vec![text("h0", "Plan")],
            vec![
                structured_list_row("r0", vec![text("p0", "Basic")], false),
                structured_list_row("r1", vec![text("p1", "Pro")], false),
            ],
        );
        let header = child(&node, "header");
        assert_eq!(token(header, "background"), None, "no header fill");
        assert_eq!(token(header, "border-top"), None);
        assert_eq!(token(header, "border-bottom"), None);
        assert_eq!(
            child(child(header, "c0"), "h0")
                .props
                .style
                .as_ref()
                .map(|s| s.as_str()),
            Some(TYPOGRAPHY_HEADING_SM)
        );
        let r0 = child(&node, "r0");
        assert_eq!(token(r0, "border-top"), Some(BORDER_SUBTLE));
        assert_eq!(token(r0, "border-bottom"), None, "only the last row closes");
        let r1 = child(&node, "r1");
        assert_eq!(token(r1, "border-top"), Some(BORDER_SUBTLE));
        assert_eq!(token(r1, "border-bottom"), Some(BORDER_SUBTLE));
        for key in ["header", "r0", "r1"] {
            assert_eq!(
                token(child(&node, key), "border"),
                None,
                "{key}: a four-sided border is the grid-of-boxes defect"
            );
        }
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
        assert_eq!(token(&on, "border-top"), Some(BORDER_SUBTLE));
        assert_eq!(on.kind, NodeKind::Grid, "shared columns need a Grid");

        let off = structured_list_row("r0", vec![text("p", "Alpha")], false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }

    /// Selected rows carry a canvas mark in the primary icon tone;
    /// unselected rows carry none — the mark is the on-state glyph, and
    /// the tone is the one that reads on a layer (the on-accent default
    /// measured 1.44:1 there).
    #[test]
    fn structured_list_row_selection_carries_a_visible_second_channel() {
        let on = structured_list_row("r0", vec![text("p", "Alpha"), text("c", "12")], true);
        assert!(has_canvas(&on));
        let mark = child(child(child(&on, "c0"), "lead"), "mark");
        assert_eq!(
            mark.props.canvas,
            icon_toned("mark", IconMark::Check, IconTone::Primary)
                .props
                .canvas,
            "the mark is drawn in `icon-primary`, not the on-accent ink"
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

    fn rect_of(frame: &PetrifiedFrame, suffix: &str) -> crate::geom::Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ending {suffix}"))
            .rect
    }

    /// Check C/D across the header and both data rows, one selected.
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

    /// A one-line data row is its padding plus its one line and nothing
    /// else — Carbon's 60 with a real 20-unit `body-01` line box (the test
    /// harness shapes a shorter line, so the sum is asserted rather than
    /// the constant) — with no pinned height; the header is shorter by the
    /// 16 that `padding-th` gives up at the bottom; and the text sits 16
    /// under the row's top, not centred in it.
    #[test]
    fn a_one_line_row_measures_carbons_default_height_from_its_padding() {
        let frame = petrify_lone(fixture());
        let row = rect_of(&frame, "/plans/r0");
        let label = rect_of(&frame, "/r0/c0/lead/p0");
        assert_eq!(
            row.h,
            16.0 + label.h + 24.0,
            "padding-td (16 + 24) plus one line; with a 20 line box that is {ROW_HEIGHT}"
        );
        assert_eq!(
            label.y - row.y,
            16.0,
            "the text sits 16 under the row's top, not centred in it"
        );
        let header = rect_of(&frame, "/plans/header");
        assert_eq!(
            row.h - header.h,
            16.0,
            "the header takes padding-th (16 + 8) and is 16 shorter than a data row"
        );
    }

    /// The reserved spacer: the leading label lands at the same x whether
    /// its own row is selected or not.
    #[test]
    fn selection_mark_reserves_the_same_width_selected_or_not() {
        let frame = petrify_lone(fixture());
        assert_eq!(
            rect_of(&frame, "/r0/c0/lead/p0").x,
            rect_of(&frame, "/r1/c0/lead/p1").x,
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
