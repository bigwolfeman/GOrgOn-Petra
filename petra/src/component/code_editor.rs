//! A code view's line rows: gutter, text, colour runs, caret (spec 006, and
//! spec 016's read-only viewer).
//!
//! The view is a scroll on both axes over a virtualized [`NodeKind::Collection`]
//! of rows. Each row is a gutter cell pinned at the view's left edge (it stays
//! put while the code scrolls sideways under it) and the line as one
//! `typography.code-lg` text node with its colour runs. The caret is that text
//! node's [`Props::cursor`], so it stands at the shaped glyph edge. The caret's
//! line gets a band (`layer-selected`) on both cells: a lightness step, not a
//! hue.
//!
//! A virtualized list never measures rows it has not placed, so it cannot say
//! how wide the file is. [`code_ruler`] can: it is the widest line laid out as
//! a row with no height and no ink, and the page is as wide as it. One row,
//! however long the file. [`cell_width`] reads a monospace cell's width off
//! its placed width, which is what [`locate`] needs to turn a pointer into a
//! column.
//!
//! Rows own their text ([`ViewNode::owning_its_text`]): the application's own
//! selection, in document positions, is the one that selects here, drawn as
//! run backgrounds.

use super::stack;
use super::text::text;
use super::tokens::t;
use crate::frame::{PetrifiedFrame, Placement};
use crate::geom::{Align, Axis, Point};
use crate::tree::{InsetRefs, Interaction, NodeKind, Props, Role, TextCursor, TextRun, TextWrap, ViewNode};

/// The key of the scroll that holds the rows: the node whose scroll offset
/// sideways moves change, and the one that holds focus.
pub const CODE_KEY: &str = "code";

/// The key of the virtualized row list.
pub const LINES_KEY: &str = "lines";

/// The key of the widest-line row ([`code_ruler`]).
pub const RULER_KEY: &str = "ruler";

/// One line's height: `typography.code-lg`'s line height. The collection
/// starts its window on a grid of this pitch and places rows at their
/// measured height, so a pitch taller than a line would leave the bottom of
/// the window empty and land a go-to-line off centre.
pub const LINE_PITCH: f32 = 20.0;

/// The gutter's inset from the view's left edge.
const GUTTER_LEFT: &str = "spacing-05";

/// The gap between the numbers and the code, kept inside the gutter cell so
/// code scrolled under the pinned gutter never shows in the gap.
const GUTTER_GAP: &str = "spacing-05";

/// Room after the widest line, so its last character is not on the view's
/// edge when scrolled to the end.
const LINE_END_ROOM: &str = "spacing-05";

/// The surface the rows sit on.
const GROUND: &str = "chrome.ground";

/// The caret line's band.
const CURRENT_LINE: &str = "layer-selected";

/// The key of line `number`'s row.
#[must_use]
pub fn line_key(number: u64) -> String {
    format!("line-{number}")
}

/// What one row shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeLine {
    /// The 1-based line number.
    pub number: u64,
    /// Digits the gutter is padded to: the widest number in the file.
    pub number_width: usize,
    /// The line as drawn (tabs already expanded to spaces).
    pub text: String,
    /// Colour runs tiling `text`, or empty for plain text.
    pub runs: Vec<TextRun>,
    /// The caret's byte offset in `text`, when the caret is on this line and
    /// the view is focused.
    pub caret: Option<usize>,
    /// This is the caret's line: the band and the bright number.
    pub current: bool,
}

/// One row: the pinned gutter cell, then the code.
#[must_use]
pub fn code_line(line: CodeLine) -> ViewNode {
    let CodeLine {
        number,
        number_width,
        text: content,
        runs,
        caret,
        current,
    } = line;
    let mut gutter = code_text("gutter", format!("{number:>number_width$}")).owning_its_text();
    // The caret line's number is in the primary ink, every other one faint:
    // the gutter is furniture. A lightness step, not a hue.
    gutter.props.tokens.insert(
        "foreground".into(),
        t(if current { "text.primary" } else { "text.faint" }),
    );
    let mut cell = flush("gutter-cell", Axis::Horizontal, vec![gutter]);
    cell.props.padding = Some(InsetRefs {
        left: Some(t(GUTTER_LEFT)),
        right: Some(t(GUTTER_GAP)),
        ..InsetRefs::default()
    });
    cell.props.pinned = Some(true);
    cell.props.z = Some(1);
    cell.props
        .tokens
        .insert("background".into(), t(if current { CURRENT_LINE } else { GROUND }));
    let mut code = code_text("text", content);
    code.props.runs = runs;
    code.props.cursor = caret.map(TextCursor::at);
    let mut row = flush(line_key(number), Axis::Horizontal, vec![cell, code]).owning_its_text();
    row.props.padding = Some(InsetRefs {
        right: Some(t(LINE_END_ROOM)),
        ..InsetRefs::default()
    });
    if current {
        row.props.tokens.insert("background".into(), t(CURRENT_LINE));
    }
    row
}

/// The widest line, laid out as a row with no height and no ink, so the page
/// is as wide as the file (see the module docs). `widest` is drawn text.
#[must_use]
pub fn code_ruler(widest: String, number_width: usize) -> ViewNode {
    let mut ruler = code_line(CodeLine {
        number: 1,
        number_width,
        text: widest,
        ..CodeLine::default()
    });
    ruler.key = RULER_KEY.into();
    ruler.props.opacity = Some(0.0);
    ruler.constraints.vertical.max = Some(0.0);
    ruler
}

/// The code view: one scroll on both axes over the row list. `name` is the
/// accessible name (the file name); `source` is the row source the
/// application's `RowSource` answers; `text_scale` is the document zoom.
/// `raw_claim` makes the view take plain keys and Tab ahead of the binding
/// table, as an editor must (spec 006 FR-013); a read-only view leaves it
/// off.
#[must_use]
pub fn code_view(
    name: &str,
    total_lines: usize,
    source: String,
    text_scale: f32,
    ruler: Option<ViewNode>,
    raw_claim: bool,
) -> ViewNode {
    let collection = ViewNode::new(NodeKind::Collection, LINES_KEY).with_props(Props {
        total_count: Some(total_lines),
        source: Some(source),
        estimated_extent: Some(LINE_PITCH * text_scale),
        ..Props::default()
    });
    let mut page = Vec::with_capacity(2);
    page.extend(ruler);
    page.push(collection);
    let interactions: &[Interaction] = if raw_claim {
        &[
            Interaction::Scroll,
            Interaction::Focus,
            Interaction::Key,
            Interaction::TextEdit,
        ]
    } else {
        &[Interaction::Scroll, Interaction::Focus, Interaction::Key]
    };
    // Declares the wheel, or the wheel and a trackpad's swipe go to the tile
    // around it and the code never moves.
    let mut view = ViewNode::new(NodeKind::Scroll, CODE_KEY)
        .with_props(Props {
            axis: Some(Axis::Vertical),
            cross_scroll: Some(true),
            text_scale: (text_scale != 1.0).then_some(text_scale),
            ..Props::default()
        })
        .interactive(Role::Scroll, name.to_owned(), interactions)
        .child(flush("page", Axis::Vertical, page));
    if raw_claim {
        view = view.with_raw_claim();
    }
    view
}

/// Where a pointer is over a code view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Located {
    /// The 1-based line of the row under (or, reaching, nearest) the
    /// pointer.
    pub line: u64,
    /// The pointer's distance from the left edge of that row's text.
    pub x: f32,
}

/// The row list placed for `source` in `frame`.
fn list<'f>(frame: &'f PetrifiedFrame, source: &str) -> Option<(usize, &'f Placement)> {
    frame
        .placements
        .iter()
        .enumerate()
        .find(|(_, p)| p.semantics.row_source.as_deref() == Some(source))
}

/// The row under `pos` in the code view whose rows come from `source`. With
/// `reach`, the nearest row when the pointer is above or below every row, as
/// a drag past the edge needs; a press must land on one.
#[must_use]
pub fn locate(frame: &PetrifiedFrame, source: &str, pos: Point, reach: bool) -> Option<Located> {
    let (index, _) = list(frame, source)?;
    let mut rows = frame
        .placements
        .iter()
        .filter(|p| p.parent == Some(index) && p.rect.h > 0.0);
    let row = if reach {
        rows.min_by(|a, b| {
            let gap = |p: &&Placement| (p.rect.y - pos.y).max(pos.y - (p.rect.y + p.rect.h)).max(0.0);
            gap(a).total_cmp(&gap(b))
        })?
    } else {
        rows.find(|p| p.rect.contains(pos) && p.clip.contains(pos))?
    };
    let line = row
        .id
        .rsplit('/')
        .next()
        .and_then(|key| key.strip_prefix("line-"))
        .and_then(|n| n.parse::<u64>().ok())?;
    let text = frame.placement(&format!("{}/text", row.id))?;
    Some(Located {
        line,
        x: pos.x - text.rect.x,
    })
}

/// One monospace cell's width in `frame`: the ruler's placed text width over
/// its drawn cells. `None` until the ruler is placed or when it is empty.
#[must_use]
pub fn cell_width(frame: &PetrifiedFrame, source: &str, ruler_cells: usize) -> Option<f32> {
    if ruler_cells == 0 {
        return None;
    }
    let (_, list) = list(frame, source)?;
    let page = list.id.strip_suffix(&format!("/{LINES_KEY}"))?;
    let ruler = frame.placement(&format!("{page}/{RULER_KEY}/text"))?;
    Some(ruler.rect.w / ruler_cells as f32)
}

/// The code view's placement id in `frame`: the scroll around the rows from
/// `source`.
#[must_use]
pub fn view_id(frame: &PetrifiedFrame, source: &str) -> Option<String> {
    let (_, list) = list(frame, source)?;
    frame.scroll_ancestor(&list.id).map(|p| p.id.clone())
}

/// A single-line text in the code face, clipped at the tile edge.
fn code_text(key: &str, content: String) -> ViewNode {
    let mut node = text(key, content);
    node.props.style = Some(t("typography.code-lg"));
    node.props.wrap = Some(TextWrap::Clip);
    node
}

/// A stack with no gap whose children stretch on the cross axis.
fn flush(key: impl Into<crate::tree::Key>, axis: Axis, children: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, axis, None, children);
    node.props.align = Some(Align::Stretch);
    node
}

#[cfg(test)]
mod tests {
    use super::{CodeLine, code_line, code_ruler, code_view};
    use crate::tree::{NodeKind, TextCursor, ViewNode};

    fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|c| find(c, key))
    }

    #[test]
    fn a_row_carries_the_caret_on_its_text_and_bands_the_current_line() {
        let row = code_line(CodeLine {
            number: 7,
            number_width: 3,
            text: "let x;".into(),
            caret: Some(4),
            current: true,
            ..CodeLine::default()
        });
        assert_eq!(row.key.as_str(), "line-7");
        let text = find(&row, "text").expect("text");
        assert_eq!(text.props.cursor, Some(TextCursor::at(4)));
        let gutter = find(&row, "gutter").expect("gutter");
        assert_eq!(gutter.props.text.as_deref(), Some("  7"));
        assert_eq!(
            row.props.tokens.get("background").map(|t| t.as_str()),
            Some("layer-selected")
        );
    }

    #[test]
    fn the_view_claims_raw_keys_only_when_editable() {
        let edit = code_view("a.rs", 10, "rows".into(), 1.0, Some(code_ruler("x".into(), 2)), true);
        let read = code_view("a.rs", 10, "rows".into(), 1.0, None, false);
        assert!(edit.raw_claim);
        assert!(!read.raw_claim);
        assert_eq!(edit.kind, NodeKind::Scroll);
        let ruler = find(&edit, "ruler").expect("ruler");
        assert_eq!(ruler.props.opacity, Some(0.0));
    }
}
