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
//!    [`selection_cell`]. Cell padding is the `padding-td` mixin: 16
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
//!
//! # The selection mark is a cell of its own, at the row's trailing edge
//!
//! Round 3, the operator: *"the padding on the structured list is wack."*
//! Measured against `ignored/carbon-ref/shots/31-structured-list.png`, both
//! halves of that were one defect. Carbon puts its header caption and its
//! row text at the **same** inset — 98 device pixels at dpr 2, so 16 units
//! plus the glyph's own bearing. Ours put the header at 17 and the row text
//! at 35, because the mark was prepended *inside* the leading data cell
//! (an extra `lead` stack) and the header, which has no mark, had no such
//! stack. Two columns that claimed to be one.
//!
//! The mark is now its own [`Role::Cell`] in its own grid track, present
//! and empty on the header, which is how Carbon composes it: the icon lives
//! in a `StructuredListCell` and the head row carries a matching empty one.
//! It sits at the **trailing** edge, which is Carbon's documented default
//! placement — slice-e:97, *"Positioned on the right of the row content by
//! default, or the left when the v12 visible-icons flag is enabled"* — and
//! is what puts the first column's text back at Carbon's 16.
//!
//! # Draggable column dividers, and the departure they are
//!
//! **Carbon does not ship column resize.** slice-b:59 verified it absent
//! from `@carbon/react`'s DataTable and says a from-scratch port *"should
//! not build column-resize ... unless it is deliberately adding a feature
//! Carbon v11 does not ship"*. The operator asked for it by name in round 3
//! — *"we should probably let the column spacers always we draggable
//! (toggled off in code)"* — so this is that deliberate addition, and
//! [`structured_list_sized`]'s `dividers` flag is the "toggled off in code"
//! half. [`structured_list`] passes `true`.
//!
//! A divider is a [`DIVIDER`]-wide grid track carrying a [`DIVIDER_RULE`]-wide
//! rule down its centre. **The hit area is eight times the mark on purpose**:
//! a one-unit drag target is not a thing a hand can hit, and the engine has
//! no way to widen a node's hit rect past its own placement, so the target
//! *is* the track and the rule is what the eye gets. It declares
//! [`Interaction::Drag`] and nothing else — not `Hover`, for the reason
//! `slider.rs` gives on its rail: a hovered node reports every pointer move
//! to the page as a position the page never asked for, which is how round 2
//! shipped a slider that moved when the pointer merely passed it.
//!
//! It declares no `Focus` either, so it never enters Tab order. That is a
//! real gap and it is named rather than hidden: a keyboard has no way to
//! resize a column here. Carbon has no keyboard model to copy, because
//! Carbon has no such control.
//!
//! [`structured_list_weights_at`] is the arithmetic, the peer of
//! [`super::slider_value_at`]: it takes the frame, the divider a gesture is
//! on, and the pointer, and answers the column weights that put the
//! boundary under the pointer. The weights are the *caller's* state, exactly
//! as a slider's value is, because a component builds a tree and holds
//! nothing.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::as_compact_heading;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SPACING_03, SPACING_05,
    SPACING_06, SURFACE_BASE, t,
};
use std::sync::Arc;

use crate::frame::PetrifiedFrame;
use crate::geom::{Align, Axis, Point};
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Justify, Key, NodeKind, Props, Role,
    Semantics, TrackSize, ViewNode,
};

/// Carbon default structured-list row height (style page Size table). Not
/// pinned on the row: it is what `padding-td` (16 + 24) plus one 20-unit
/// line of `body-01` adds up to, and the row grows with a second line.
const ROW_HEIGHT: f32 = 60.0;

const _: () = assert!(ROW_HEIGHT == 60.0);

/// The width of a column divider's grid track, and so of its drag target.
///
/// Not a Carbon number — Carbon has no such control (slice-b:59). It is
/// `$spacing-03` (8) expressed as a length rather than a token, because a
/// track size is a number in this engine and not a token reference, and 8
/// is the smallest step in Carbon's own spacing set that is still a target
/// a pointer can land on. The rule the eye sees is [`DIVIDER_RULE`], so the
/// target is eight times the mark.
const DIVIDER: f32 = 8.0;

/// The drawn width of the rule inside a divider: one unit, the same rule
/// weight `border-top` gives a row, so a vertical seam and a horizontal one
/// read as the same line.
const DIVIDER_RULE: f32 = 1.0;

/// The narrowest a column may be dragged.
///
/// A cell carries `$spacing-05` (16) of inline padding on each side
/// ([`cell_padding`]), so 32 is the width at which a column is all padding
/// and no content. Dragging past that would hide text behind the next
/// column rather than narrow it, so the drag stops there.
const MIN_COLUMN: f32 = 32.0;

const _: () = assert!(MIN_COLUMN == 2.0 * 16.0);

/// The key prefix every divider carries: `div0` is the boundary between
/// column 0 and column 1.
const DIVIDER_KEY: &str = "div";

/// The key of every row's selection cell, and of the mark inside it.
const SELECT_CELL: &str = "sel";
/// The mark itself, inside [`SELECT_CELL`].
const MARK: &str = "mark";

const ROW_INTENTS: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// What a column divider declares. `Drag` alone: see the module doc.
const DIVIDER_INTENTS: &[Interaction] = &[Interaction::Drag];

/// Which padding mixin a cell takes.
#[derive(Clone, Copy)]
enum CellKind {
    /// `padding-th`: 16 top, 8 bottom.
    Header,
    /// `padding-td`: 16 top, 24 bottom.
    Data,
}

/// Structured list: header + rows under [`Role::Table`], equal columns,
/// draggable dividers between them.
///
/// The last data row gets the tbody's closing rule. For a list whose
/// columns a caller sizes — which is what a drag needs, since the weights
/// are the caller's state — see [`structured_list_sized`].
pub fn structured_list(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
) -> ViewNode {
    let ncols = header.len().max(1);
    structured_list_sized(key, header, rows, &vec![1.0; ncols], true)
}

/// [`structured_list`] with the column weights and the divider policy the
/// caller chooses.
///
/// `weights` is one entry per data column; a shorter or longer vector is
/// padded or truncated to the header's column count, because a mismatch is
/// a caller bug that must not silently drop a column. `dividers` is the
/// "toggled off in code" half of the operator's round-3 request: `false`
/// builds the same table with no divider tracks and nothing draggable.
pub fn structured_list_sized(
    key: impl Into<Key>,
    header: Vec<ViewNode>,
    rows: Vec<ViewNode>,
    weights: &[f32],
    dividers: bool,
) -> ViewNode {
    let ncols = header.len().max(1);
    let names: Vec<String> = header.iter().map(collect_text).collect();
    let weights = normalise_weights(weights, ncols);

    let mut rows: Vec<ViewNode> = rows.into_iter().map(ensure_row).collect();
    if let Some(last) = rows.last_mut() {
        last.props
            .tokens
            .insert("border-bottom".into(), t(BORDER_SUBTLE));
    }
    let mut children = vec![header_row("header", header)];
    children.extend(rows);
    for row in &mut children {
        lay_out_columns(row, &weights, dividers, &names);
    }

    let mut node = stack(key, Axis::Vertical, None, children);
    // Every row is a `Grid` (see `row_shell`) whose column tracks resolve
    // against whatever width `place` offers it. `Stretch` offers every row
    // the list's own width, so the Grid tracks resolve identical pixel
    // columns row to row.
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::Table),
        ..Semantics::default()
    };
    node
}

/// One selectable data row. Interactive, [`Role::Row`], cells stamped
/// [`Role::Cell`]. `selected` is a declared fact plus the four-fill set
/// [`super::list_row`] pioneered, plus the trailing [`selection_cell`]'s
/// icon — Carbon's own second channel.
pub fn structured_list_row(key: impl Into<Key>, cells: Vec<ViewNode>, selected: bool) -> ViewNode {
    let key = key.into();
    let label = row_label(key.as_str(), &cells);
    let mut node = row_shell(
        key,
        cells,
        CellKind::Data,
        Some(selection_cell(selected, CellKind::Data)),
    );
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

/// The column weights a drag on the divider `node` to `pos` asks for.
///
/// The peer of [`super::slider_value_at`], and the same division of labour:
/// the component owns the geometry, the page owns the state. `node` is any
/// placement id inside a divider this module built — the divider or its
/// rule. `weights` is what the caller is drawing with now.
///
/// Answers `None` when `node` names no divider, when the frame has not
/// placed the two columns the divider sits between, or when those two
/// columns have less room between them than two [`MIN_COLUMN`]s. A `None`
/// is "this drag changes nothing", never a silent zero.
///
/// Only the two columns either side of the divider move, and their weights
/// sum to what they summed to before, so every other column keeps the pixel
/// width it had.
#[must_use]
pub fn structured_list_weights_at(
    frame: &PetrifiedFrame,
    node: &str,
    pos: Point,
    weights: &[f32],
) -> Option<Vec<f32>> {
    let (row, index) = divider_of(node)?;
    if index + 1 >= weights.len() {
        return None;
    }
    let left = frame.placement(&format!("{row}/c{index}"))?.rect;
    let right = frame.placement(&format!("{row}/c{}", index + 1))?.rect;
    // The two column tracks with the divider track between them.
    let travel = right.right() - left.x - DIVIDER;
    if travel <= 2.0 * MIN_COLUMN {
        return None;
    }
    // `pos.x` is where the *middle* of the divider should land.
    let want = (pos.x - DIVIDER / 2.0 - left.x).clamp(MIN_COLUMN, travel - MIN_COLUMN);
    let pair = weights[index] + weights[index + 1];
    let mut out = weights.to_vec();
    out[index] = pair * want / travel;
    out[index + 1] = pair - out[index];
    Some(out)
}

/// The row id and the column index of the divider `node` names, or `None`.
///
/// By segment rather than by suffix, because a gesture can be routed to the
/// divider itself or to the `rule` leaf inside it, and both have to answer
/// the same boundary.
fn divider_of(node: &str) -> Option<(&str, usize)> {
    let mut offset = 0usize;
    for segment in node.split('/') {
        let start = offset;
        offset += segment.len() + 1;
        if let Some(index) = segment
            .strip_prefix(DIVIDER_KEY)
            .and_then(|digits| digits.parse::<usize>().ok())
        {
            return Some((&node[..start.saturating_sub(1)], index));
        }
    }
    None
}

/// `weights` at exactly `ncols` entries, every one strictly positive.
///
/// A non-positive weight is a tree-acceptance error
/// (`TrackSize::Weight`'s own contract), so a caller handing one in would
/// get a refused tree rather than a narrow column; 1.0 is the equal-columns
/// default the same caller would have got from [`structured_list`].
fn normalise_weights(weights: &[f32], ncols: usize) -> Vec<f32> {
    (0..ncols)
        .map(|i| match weights.get(i) {
            Some(w) if *w > 0.0 && w.is_finite() => *w,
            _ => 1.0,
        })
        .collect()
}

/// Rewrite one row's grid tracks and interleave the dividers.
///
/// The container does this rather than [`structured_list_row`], because
/// only the container knows how many columns the table has, what the
/// columns are called, and whether the caller wanted dividers. A row built
/// on its own is still a legal grid — [`row_shell`] gives it equal columns
/// — and this re-seats it into the table's.
fn lay_out_columns(row: &mut ViewNode, weights: &[f32], dividers: bool, names: &[String]) {
    // A row's children are the data cells in column order, then the
    // trailing selection cell. Anything that is not a `Grid` this module
    // built is a row a caller composed itself, and is left alone.
    if row.kind != NodeKind::Grid {
        return;
    }
    let trailing = row
        .children
        .last()
        .is_some_and(|c| c.key.as_str() == SELECT_CELL);
    let ndata = row.children.len() - usize::from(trailing);
    if ndata == 0 {
        return;
    }
    let old = std::mem::take(&mut row.children);
    let mut children: Vec<Arc<ViewNode>> = Vec::with_capacity(old.len() + ndata);
    let mut columns: Vec<TrackSize> = Vec::with_capacity(old.len() + ndata);
    for (i, cell) in old.into_iter().enumerate() {
        if i == ndata {
            children.push(cell);
            columns.push(TrackSize::FitContent);
            break;
        }
        if i > 0 && dividers {
            children.push(Arc::new(column_divider(i - 1, names.get(i - 1))));
            columns.push(TrackSize::Fixed { value: DIVIDER });
        }
        children.push(cell);
        columns.push(TrackSize::Weight {
            weight: weights.get(i).copied().unwrap_or(1.0),
        });
    }
    row.props.columns = columns;
    row.children = children;
}

/// One draggable column boundary: a [`DIVIDER`]-wide target with a
/// [`DIVIDER_RULE`]-wide rule down its centre.
///
/// `name` is the caption of the column to its left, so the accessible name
/// says which boundary this is. A divider with no name to its left still
/// gets a non-empty label, because an interactive node without one is an
/// audit violation (`ActionableNeedsRoleAndLabel`) and a blank string is
/// not a name.
fn column_divider(index: usize, name: Option<&String>) -> ViewNode {
    let mut rule = ViewNode::new(NodeKind::Spacer, "rule").with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(DIVIDER_RULE),
            max: Some(DIVIDER_RULE),
            priority: 0,
        },
        ..Constraints::default()
    });
    rule.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));

    let label = match name {
        Some(name) if !name.trim().is_empty() => format!("Resize column {name}"),
        _ => format!("Resize column {}", index + 1),
    };
    let mut node = stack(
        format!("{DIVIDER_KEY}{index}"),
        Axis::Horizontal,
        None,
        vec![rule],
    );
    // Cross axis is the height: the rule runs the full height of the row.
    node.props.align = Some(Align::Stretch);
    // Main axis is the width: the one-unit rule sits in the middle of the
    // eight-unit target rather than against its leading edge.
    node.props.justify = Some(Justify::Center);
    node.interactive(Role::Separator, label, DIVIDER_INTENTS)
}

/// The trailing selection cell: the mark when the row is selected, a
/// same-size blank when it is not.
///
/// [`IconMark`] has no radio-pair glyph — [`IconMark::Check`] is the
/// vocabulary's existing stand-in for "this is the on state", drawn in
/// [`IconTone::Primary`] (Carbon's `$icon-primary`, the fill its SCSS gives
/// the checked icon). The first version drew it in the default on-accent
/// tone, which on the row's own layer measured 1.44:1 and was a mark
/// nobody could see.
///
/// The blank keeps the column one width down the whole table, so a row's
/// text does not shift when its selection changes.
fn selection_cell(selected: bool, kind: CellKind) -> ViewNode {
    let mark = icon_toned(MARK, IconMark::Check, IconTone::Primary);
    let mark = if selected {
        mark
    } else {
        let w = mark.constraints.horizontal.min.unwrap_or(0.0);
        let h = mark.constraints.vertical.min.unwrap_or(0.0);
        let mut blank = stack(MARK, Axis::Horizontal, None, vec![]);
        blank.constraints.horizontal = AxisConstraint {
            min: Some(w),
            max: Some(w),
            priority: 0,
        };
        blank.constraints.vertical = AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        };
        blank
    };
    let mut cell = stack(SELECT_CELL, Axis::Horizontal, None, vec![mark]);
    // The same padding mixin its row's data cells take, so the mark sits on
    // the row's own text baseline band rather than floating in a taller box.
    cell.props.padding = Some(cell_padding(kind));
    cell.props.align = Some(Align::Start);
    cell.semantics = Semantics {
        role: Some(Role::Cell),
        ..Semantics::default()
    };
    cell
}

/// The column header row: compact-heading text, no fill, no rule, and an
/// empty selection cell so its columns are the data rows' columns.
fn header_row(key: impl Into<Key>, cells: Vec<ViewNode>) -> ViewNode {
    let cells = cells.into_iter().map(as_compact_heading).collect();
    let mut node = row_shell(
        key,
        cells,
        CellKind::Header,
        Some(selection_cell(false, CellKind::Header)),
    );
    node.semantics = Semantics {
        role: Some(Role::Row),
        ..Semantics::default()
    };
    node
}

/// One row's cells, laid out as a `Grid` of `ncols` equal [`TrackSize::Weight`]
/// columns rather than a bare `Axis::Horizontal` stack, so every cell in
/// column *i* resolves the same width in every row.
///
/// `trailing` is the selection cell, in its own `FitContent` track at the
/// end. [`lay_out_columns`] rewrites both lists once the container knows
/// the table's real weights and divider policy.
fn row_shell(
    key: impl Into<Key>,
    cells: Vec<ViewNode>,
    kind: CellKind,
    trailing: Option<ViewNode>,
) -> ViewNode {
    let ncols = cells.len().max(1);
    let mut children: Vec<ViewNode> = cells
        .into_iter()
        .enumerate()
        .map(|(i, cell)| as_cell(i, cell, kind))
        .collect();
    let mut columns = vec![TrackSize::Weight { weight: 1.0 }; ncols];
    if let Some(trailing) = trailing {
        children.push(trailing);
        columns.push(TrackSize::FitContent);
    }
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns,
            rows: vec![TrackSize::FitContent],
            align: Some(Align::Stretch),
            ..Props::default()
        })
        .with_children(children)
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
    let mut row = row_shell(
        node.key.clone(),
        vec![node],
        CellKind::Data,
        Some(selection_cell(false, CellKind::Data)),
    );
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
        DIVIDER, DIVIDER_KEY, MARK, MIN_COLUMN, ROW_HEIGHT, SELECT_CELL, SPACING_03, SPACING_05,
        SPACING_06, structured_list, structured_list_row, structured_list_sized,
        structured_list_weights_at,
    };
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::text::text;
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED, TYPOGRAPHY_HEADING_SM};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Point, Size};
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
        let mark = child(child(&on, SELECT_CELL), MARK);
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
        let label = rect_of(&frame, "/r0/c0/p0");
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

    /// The reserved blank: the selection column is one width down the
    /// whole table, so no row's text shifts when its selection changes.
    #[test]
    fn selection_mark_reserves_the_same_width_selected_or_not() {
        let frame = petrify_lone(fixture());
        assert_eq!(
            rect_of(&frame, "/r0/c0/p0").x,
            rect_of(&frame, "/r1/c0/p1").x,
            "the leading label must start at the same x whether its own \
             row is selected or not"
        );
        assert_eq!(
            rect_of(&frame, "/r0/sel").w,
            rect_of(&frame, "/r1/sel").w,
            "the unselected row's blank is not the selected row's mark width"
        );
    }

    /// Row 31, round 3 ("the padding is wack"). Carbon puts the header
    /// caption and the row text at the **same** inset —
    /// `31-structured-list.png`, both at x=98 device at dpr 2. Ours put the
    /// header at 17 and the rows at 35 because the mark rode inside the
    /// leading data cell. It is a cell of its own now, at the trailing edge
    /// where slice-e:97 puts Carbon's default.
    #[test]
    fn the_header_caption_and_the_row_text_share_one_left_inset() {
        let frame = petrify_lone(fixture());
        let header = rect_of(&frame, "/header/c0/h0").x;
        for label in ["/r0/c0/p0", "/r1/c0/p1"] {
            assert_eq!(
                rect_of(&frame, label).x,
                header,
                "{label} does not start where the header caption does"
            );
        }
        // And the second column agrees too, which is what the shared grid
        // tracks are for.
        let header1 = rect_of(&frame, "/header/c1/h1").x;
        assert_eq!(rect_of(&frame, "/r0/c1/c0").x, header1);
        assert_eq!(rect_of(&frame, "/r1/c1/c1").x, header1);
    }

    /// The mark is at the row's **trailing** edge, not its leading one:
    /// slice-e:97, "positioned on the right of the row content by default".
    #[test]
    fn the_selection_mark_sits_after_the_last_data_column() {
        let frame = petrify_lone(fixture());
        let last_column = rect_of(&frame, "/r1/c1");
        let mark_cell = rect_of(&frame, "/r1/sel");
        assert!(
            mark_cell.x >= last_column.right() - 0.01,
            "the mark cell ({mark_cell:?}) is not after the last data \
             column ({last_column:?})"
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

    /// Round 3, the operator: *"we should probably let the column spacers
    /// always we draggable (toggled off in code)."* A divider is a real
    /// control, so it declares `Drag`, a role and a name — and **not**
    /// `Hover`, which would report every pointer move over it to the page
    /// as a position the page never asked for (the round-2 slider bug), and
    /// not `Focus`, which would put it in Tab order with no keyboard model
    /// behind it.
    #[test]
    fn a_column_divider_declares_drag_and_nothing_that_reports_a_stray_move() {
        let node = structured_list(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![structured_list_row(
                "r0",
                vec![text("p0", "Basic"), text("c0", "$12")],
                false,
            )],
        );
        for row in ["header", "r0"] {
            let div = child(child(&node, row), &format!("{DIVIDER_KEY}0"));
            assert_eq!(div.semantics.role, Some(Role::Separator));
            assert_eq!(
                div.semantics.label.as_deref(),
                Some("Resize column Plan"),
                "the divider names the column to its left"
            );
            assert!(div.interactions.contains(&Interaction::Drag));
            assert!(
                !div.interactions.contains(&Interaction::Hover),
                "a hovered divider hands the page a position it never asked for"
            );
            assert!(!div.interactions.contains(&Interaction::Focus));
            assert_eq!(
                token(child(div, "rule"), "background"),
                Some(BORDER_SUBTLE),
                "the rule is the same tone as a row's own"
            );
        }
        // One divider per boundary, never one per column.
        assert!(
            child(&node, "r0")
                .children
                .iter()
                .filter(|c| c.key.as_str().starts_with(DIVIDER_KEY))
                .count()
                == 1
        );
    }

    /// The hit area is the whole track and the drawn rule is one unit
    /// inside it, centred. A one-unit drag target is not a thing a hand can
    /// hit; this is the number that makes the control usable and it is
    /// measured in the frame, not asserted off the constant.
    #[test]
    fn the_divider_target_is_eight_times_the_rule_it_draws() {
        let frame = petrify_lone(fixture());
        let target = rect_of(&frame, "/r0/div0");
        let rule = rect_of(&frame, "/r0/div0/rule");
        assert_eq!(target.w, DIVIDER);
        assert_eq!(rule.w, 1.0);
        assert_eq!(target.w, 8.0 * rule.w);
        assert!(
            (rule.x - (target.x + (target.w - rule.w) / 2.0)).abs() < 0.51,
            "the rule ({rule:?}) is not centred in its target ({target:?})"
        );
        assert_eq!(rule.h, target.h, "the rule runs the row's full height");
    }

    /// "Toggled off in code": the same table with `dividers: false` builds
    /// no divider at all, and nothing in it declares `Drag`.
    #[test]
    fn dividers_can_be_turned_off_and_then_nothing_drags() {
        fn any_drag(node: &ViewNode) -> bool {
            node.interactions.contains(&Interaction::Drag)
                || node.children.iter().any(|c| any_drag(c))
        }
        let off = structured_list_sized(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![structured_list_row(
                "r0",
                vec![text("p0", "Basic"), text("c0", "$12")],
                false,
            )],
            &[1.0, 1.0],
            false,
        );
        assert!(!any_drag(&off), "a divider survived `dividers: false`");
        assert!(
            !child(&off, "r0")
                .children
                .iter()
                .any(|c| c.key.as_str().starts_with(DIVIDER_KEY))
        );
        // And on by default, which is the other half of the operator's ask.
        let on = structured_list(
            "plans",
            vec![text("h0", "Plan"), text("h1", "Price")],
            vec![structured_list_row(
                "r0",
                vec![text("p0", "Basic"), text("c0", "$12")],
                false,
            )],
        );
        assert!(any_drag(&on), "dividers are meant to be on by default");
    }

    /// The arithmetic: a drag to `x` puts the divider's **middle** at `x`,
    /// moves only the two columns either side, and stops at
    /// [`MIN_COLUMN`] rather than letting a column vanish.
    #[test]
    fn dragging_a_divider_moves_the_boundary_and_stops_at_the_minimum() {
        let frame = petrify_lone(fixture());
        let node = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/r0/div0"))
            .expect("the divider is placed")
            .id
            .clone();
        let left = rect_of(&frame, "/r0/c0");
        let right = rect_of(&frame, "/r0/c1");
        let travel = right.right() - left.x - DIVIDER;
        let weights = [1.0f32, 1.0];

        let at = |x: f32| {
            structured_list_weights_at(&frame, &node, Point::new(x, left.y + 1.0), &weights)
                .expect("the divider resolves")
        };

        // Equal columns to start with: the boundary is at the middle.
        let quarter = at(left.x + travel * 0.25 + DIVIDER / 2.0);
        assert!(
            (quarter[0] / (quarter[0] + quarter[1]) - 0.25).abs() < 1e-3,
            "a drag to the quarter mark gave {quarter:?}"
        );
        assert!(
            (quarter[0] + quarter[1] - 2.0).abs() < 1e-4,
            "the pair's total weight changed, so the other columns moved"
        );
        let three_quarters = at(left.x + travel * 0.75 + DIVIDER / 2.0);
        assert!(
            three_quarters[0] > quarter[0],
            "further right is a wider first column"
        );

        // Clamped: dragging off the left end leaves MIN_COLUMN behind.
        let squashed = at(left.x - 1000.0);
        let width = 2.0 * squashed[0] / (squashed[0] + squashed[1]) * travel / 2.0;
        assert!(
            (width - MIN_COLUMN).abs() < 1e-2,
            "the first column was dragged to {width}, past the {MIN_COLUMN} floor"
        );

        // A node that is not a divider resolves to nothing at all.
        assert!(
            structured_list_weights_at(
                &frame,
                &rect_id(&frame, "/r0/c0"),
                Point::new(left.x, left.y),
                &weights,
            )
            .is_none()
        );
    }

    /// The rule reaches the gesture from the leaf as well as from the
    /// target: a press can land on either and both name the same boundary.
    #[test]
    fn a_press_on_the_rule_resolves_the_same_boundary_as_the_target() {
        let frame = petrify_lone(fixture());
        let weights = [1.0f32, 1.0];
        let left = rect_of(&frame, "/r0/c0");
        let pos = Point::new(left.x + 100.0, left.y + 1.0);
        let by_target =
            structured_list_weights_at(&frame, &rect_id(&frame, "/r0/div0"), pos, &weights);
        let by_rule =
            structured_list_weights_at(&frame, &rect_id(&frame, "/r0/div0/rule"), pos, &weights);
        assert!(by_target.is_some());
        assert_eq!(by_target, by_rule);
    }

    /// The id of the placement whose id ends `suffix`.
    fn rect_id(frame: &PetrifiedFrame, suffix: &str) -> String {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(suffix))
            .unwrap_or_else(|| panic!("no placement ending {suffix}"))
            .id
            .clone()
    }
}
