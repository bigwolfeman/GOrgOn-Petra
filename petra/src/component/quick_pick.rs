//! Quick pick: the workbench's one-line chooser, docked at the top centre of
//! the window.
//!
//! A workbench editor asks "which one?" the same way every time: a narrow
//! panel hangs from the top of the window, a title says what is being
//! chosen and for what, a field filters, and the rows under it are the
//! choices, the best match first. Enter takes the chosen row and Escape
//! leaves. The panel sits over the work, not in the middle of it, so the
//! thing being acted on stays in view below.
//!
//! Anatomy:
//! 1. Surface — [`NodeKind::Surface`] on [`Layer::Modal`], docked inside the
//!    window's top edge at its centre ([`Anchor::ViewportEdge`]), held
//!    [`SPACING_03`] off the edge so it does not read as part of whatever
//!    strip runs along the top. Pinned to [`QUICK_PICK_WIDTH`] and shrunk to
//!    a narrower window. Fill [`SURFACE_RAISED`], [`SHADOW_OVERLAY`], the
//!    floating radius, [`SPACING_02`] of padding. A press outside closes it
//!    ([`InputPolicy::DismissOutside`]).
//! 2. Title — optional, one line of [`TEXT_MUTED`] at the top, centred:
//!    what is being chosen, for what (`Move Cargo.lock to…`).
//! 3. Field — a [`QUICK_PICK_FIELD_HEIGHT`] well holding the query, its
//!    placeholder the prompt, its edge the accent: the keys go here.
//! 4. Rows — [`QUICK_PICK_ROW_HEIGHT`] each: an optional leading glyph, the
//!    label with every matched character underlined (a shape, not a hue),
//!    and an optional detail in [`TEXT_FAINT`] after it. The chosen row
//!    takes the selected fill **and** a one-unit accent outline, so the
//!    choice reads by lightness and by shape, never by hue alone.
//! 5. Note — when nothing matches, one muted line says so.
//!
//! The panel owns no state. The application keeps the query, the cursor,
//! the ranking and which row is chosen, and draws them here each pass; the
//! field is built with the editable intents and an application that hears
//! the keys itself clears them, as it does for any field it drives.

use super::icon::{IconMark, IconTone, icon_toned};
use super::pin_block;
use super::stack;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, CHROME_SELECTED, SHADOW_OVERLAY, SPACING_02, SPACING_03, SURFACE_BASE,
    SURFACE_RAISED, TEXT_FAINT, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::{Align as CrossAlign, Axis};
use crate::token::{CornerRole, corner_for};
use crate::tree::{
    Align, Anchor, AxisConstraint, ClampRule, Constraints, Edge, InputPolicy, InsetRefs,
    Interaction, Justify, Key, Layer, NodeKind, Props, Role, Semantics, TextCursor, TextRun,
    TextWrap, ViewNode,
};

/// The panel's width: the workbench quick input's.
pub const QUICK_PICK_WIDTH: f32 = 600.0;
/// The filter well's height.
pub const QUICK_PICK_FIELD_HEIGHT: f32 = 26.0;
/// One row's height: the workbench list pitch.
pub const QUICK_PICK_ROW_HEIGHT: f32 = 22.0;

/// What an editable well declares (`field.rs` keeps the same list private).
const EDITABLE: &[Interaction] = &[
    Interaction::Focus,
    Interaction::Click,
    Interaction::Key,
    Interaction::TextEdit,
];

/// One choice in a [`quick_pick`], described before it is built.
#[derive(Clone, Debug)]
pub struct QuickPickItem {
    key: Key,
    label: String,
    detail: Option<String>,
    icon: Option<IconMark>,
    matched: Vec<usize>,
    chosen: bool,
}

impl QuickPickItem {
    /// A row keyed `key` that reads `label`, with nothing else.
    #[must_use]
    pub fn new(key: impl Into<Key>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            detail: None,
            icon: None,
            matched: Vec::new(),
            chosen: false,
        }
    }

    /// Words after the label in a fainter ink, such as where the choice
    /// lives. They give way first when the panel is narrow.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A glyph before the label, so a kind of choice is a shape.
    #[must_use]
    pub fn icon(mut self, icon: IconMark) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The characters of the label the query matched, as `char` indexes.
    /// Each is underlined.
    #[must_use]
    pub fn matched(mut self, chars: Vec<usize>) -> Self {
        self.matched = chars;
        self
    }

    /// Whether Enter takes this row.
    #[must_use]
    pub fn chosen(mut self, chosen: bool) -> Self {
        self.chosen = chosen;
        self
    }

    /// The row.
    #[must_use]
    pub fn build(self) -> ViewNode {
        let Self {
            key,
            label,
            detail,
            icon,
            matched,
            chosen,
        } = self;
        let mut name = text("label", label.clone());
        name.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
        name.props.wrap = Some(TextWrap::Ellipsis);
        name.props
            .tokens
            .insert("foreground".into(), t(TEXT_PRIMARY));
        name.props.runs = underline_runs(&label, &matched);
        // The label keeps its width; the detail gives way first.
        name.constraints.horizontal.priority = 1;
        let mut parts = Vec::with_capacity(3);
        if let Some(mark) = icon {
            parts.push(icon_toned("icon", mark, IconTone::Secondary));
        }
        parts.push(name);
        if let Some(detail) = detail {
            let mut words = text("detail", detail);
            words.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
            words.props.wrap = Some(TextWrap::Ellipsis);
            words
                .props
                .tokens
                .insert("foreground".into(), t(TEXT_FAINT));
            parts.push(words);
        }
        let mut row = stack(key, Axis::Horizontal, Some(SPACING_03), parts);
        row.props.align = Some(CrossAlign::Center);
        row.props.justify = Some(Justify::Start);
        row.props.padding = Some(InsetRefs {
            left: Some(t(SPACING_03)),
            right: Some(t(SPACING_03)),
            ..InsetRefs::default()
        });
        row.constraints = pin_block(QUICK_PICK_ROW_HEIGHT);
        if chosen {
            row.props
                .tokens
                .insert("background".into(), t(CHROME_SELECTED));
            row.props.tokens.insert("border".into(), t(ACCENT_PRIMARY));
        }
        row.semantics = Semantics {
            role: Some(Role::ListItem),
            label: Some(label),
            selected: chosen,
            ..Semantics::default()
        };
        row
    }
}

/// Runs over `label` that underline each matched `char` index, in bytes.
fn underline_runs(label: &str, matched: &[usize]) -> Vec<TextRun> {
    if matched.is_empty() {
        return Vec::new();
    }
    let mut runs: Vec<TextRun> = Vec::new();
    let mut hits = matched.iter().copied().peekable();
    for (index, ch) in label.chars().enumerate() {
        let hit = hits.peek() == Some(&index);
        if hit {
            hits.next();
        }
        match runs.last_mut() {
            Some(run) if run.underline == hit => run.len += ch.len_utf8(),
            _ => runs.push(TextRun {
                len: ch.len_utf8(),
                underline: hit,
                ..TextRun::default()
            }),
        }
    }
    runs
}

/// A quick pick keyed `key`.
///
/// `title` heads the panel when given. `query` is the filter typed so far,
/// drawn with `cursor`; `placeholder` shows while it is empty and names the
/// field. `items` are the rows in order, and `empty` is the line shown when
/// there are none.
#[must_use]
pub fn quick_pick(
    key: impl Into<Key>,
    title: Option<&str>,
    query: &str,
    placeholder: &str,
    cursor: TextCursor,
    items: Vec<QuickPickItem>,
    empty: &str,
) -> ViewNode {
    let mut children = Vec::with_capacity(items.len() + 2);
    if let Some(title) = title {
        let mut words = text("title", title);
        words.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
        words.props.wrap = Some(TextWrap::Ellipsis);
        words
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_MUTED));
        let mut head = stack("head", Axis::Horizontal, None, vec![words]);
        head.props.justify = Some(Justify::Center);
        head.props.align = Some(CrossAlign::Center);
        head.constraints = pin_block(QUICK_PICK_ROW_HEIGHT);
        children.push(head);
    }
    children.push(field(query, placeholder, cursor));
    if items.is_empty() {
        let mut words = text("empty", empty);
        words.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
        words
            .props
            .tokens
            .insert("foreground".into(), t(TEXT_MUTED));
        let mut note = stack("note", Axis::Horizontal, None, vec![words]);
        note.props.align = Some(CrossAlign::Center);
        note.props.padding = Some(InsetRefs {
            left: Some(t(SPACING_03)),
            ..InsetRefs::default()
        });
        note.constraints = pin_block(QUICK_PICK_ROW_HEIGHT);
        children.push(note);
    } else {
        let rows: Vec<ViewNode> = items.into_iter().map(QuickPickItem::build).collect();
        let mut list = stack("rows", Axis::Vertical, None, rows);
        list.props.align = Some(CrossAlign::Stretch);
        list.semantics = Semantics {
            role: Some(Role::List),
            label: Some(placeholder.to_owned()),
            ..Semantics::default()
        };
        children.push(list);
    }
    let mut body = stack("body", Axis::Vertical, Some(SPACING_02), children);
    body.props.align = Some(CrossAlign::Stretch);

    let pinned = AxisConstraint {
        min: Some(QUICK_PICK_WIDTH),
        max: Some(QUICK_PICK_WIDTH),
        priority: 0,
    };
    let mut node = ViewNode::new(NodeKind::Surface, key)
        .with_props(Props {
            layer: Some(Layer::Modal),
            anchor: Some(Anchor::ViewportEdge {
                edge: Edge::Top,
                align: Align::Center,
                offset: Some(t(SPACING_03)),
            }),
            clamp: Some(ClampRule::Shrink),
            input_policy: Some(InputPolicy::DismissOutside),
            padding: Some(InsetRefs {
                left: Some(t(SPACING_02)),
                right: Some(t(SPACING_02)),
                top: Some(t(SPACING_02)),
                bottom: Some(t(SPACING_02)),
            }),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: pinned,
            ..Constraints::default()
        })
        .child(body);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("shadow".into(), t(SHADOW_OVERLAY));
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(
            CornerRole::Floating,
            QUICK_PICK_FIELD_HEIGHT * 4.0,
        )),
    );
    node.semantics = Semantics {
        role: Some(Role::Dialog),
        label: Some(title.unwrap_or(placeholder).to_owned()),
        ..Semantics::default()
    };
    node
}

/// The filter well.
fn field(query: &str, placeholder: &str, cursor: TextCursor) -> ViewNode {
    let mut props = Props {
        placeholder: Some(placeholder.to_owned()),
        text: Some(query.to_owned()),
        cursor: Some(cursor),
        style: Some(t(TYPOGRAPHY_BODY_COMPACT)),
        ..Props::default()
    };
    // An empty well paints its placeholder: a hint, in the muted ink.
    let ink = if query.is_empty() {
        TEXT_MUTED
    } else {
        TEXT_PRIMARY
    };
    props.tokens.insert("foreground".into(), t(ink));
    props.tokens.insert("background".into(), t(SURFACE_BASE));
    props.tokens.insert("border".into(), t(ACCENT_PRIMARY));
    ViewNode::new(NodeKind::Input, "query")
        .with_props(props)
        .interactive(Role::TextInput, placeholder.to_owned(), EDITABLE)
        .with_constraints(pin_block(QUICK_PICK_FIELD_HEIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::tests::petrify_lone_at;
    use crate::geom::{Rect, Size};

    fn rect_of(frame: &crate::frame::PetrifiedFrame, tail: &str) -> Rect {
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(tail))
            .unwrap_or_else(|| panic!("no placement ends with {tail}"))
            .rect
    }

    fn pick(items: Vec<QuickPickItem>) -> ViewNode {
        quick_pick(
            "pick",
            Some("Move Cargo.lock to…"),
            "src",
            "Folder",
            TextCursor::at(3),
            items,
            "No matching folder",
        )
    }

    /// The panel hangs from the top centre, held off the edge, at its width;
    /// the title, the field and the rows stack in that order.
    #[test]
    fn the_panel_docks_at_the_top_centre_with_title_field_and_rows() {
        let items = vec![
            QuickPickItem::new("a", "src")
                .icon(IconMark::Folder)
                .detail("gorgon")
                .matched(vec![0, 1, 2])
                .chosen(true),
            QuickPickItem::new("b", "src/files").icon(IconMark::Folder),
        ];
        let frame = petrify_lone_at(pick(items), Size::new(1400.0, 900.0));
        let panel = rect_of(&frame, "/pick");
        assert_eq!(panel.w, QUICK_PICK_WIDTH);
        assert!(
            (panel.x + panel.w / 2.0 - 700.0).abs() < 0.5,
            "centred: {panel:?}"
        );
        assert!(
            panel.y > 0.0 && panel.y < 16.0,
            "near the top, off the edge: {panel:?}"
        );
        let title = rect_of(&frame, "/pick/body/head");
        let query = rect_of(&frame, "/pick/body/query");
        let first = rect_of(&frame, "/pick/body/rows/a");
        assert!(title.y < query.y && query.y < first.y);
        assert_eq!(query.h, QUICK_PICK_FIELD_HEIGHT);
        assert_eq!(first.h, QUICK_PICK_ROW_HEIGHT);
        assert!(
            first.w > QUICK_PICK_WIDTH - 20.0,
            "rows span the panel: {first:?}"
        );
    }

    /// The chosen row is marked by fill and outline and the `selected` fact;
    /// the matched characters are underlined runs, not a colour.
    #[test]
    fn the_chosen_row_is_a_shape_and_matches_are_underlined() {
        let chosen = QuickPickItem::new("a", "src/ab")
            .matched(vec![0, 4])
            .chosen(true)
            .build();
        assert!(chosen.semantics.selected);
        assert_eq!(
            chosen.props.tokens.get("border").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        let label = chosen
            .children
            .iter()
            .find(|c| c.key.as_str() == "label")
            .expect("a label");
        let underlined: Vec<(usize, bool)> = label
            .props
            .runs
            .iter()
            .map(|run| (run.len, run.underline))
            .collect();
        assert_eq!(
            underlined,
            [(1, true), (3, false), (1, true), (1, false)],
            "s / rc/ / a / b"
        );
        let plain = QuickPickItem::new("b", "x").build();
        assert!(!plain.semantics.selected);
        assert!(!plain.props.tokens.contains_key("border"));
    }

    #[test]
    fn no_rows_shows_the_empty_line() {
        let node = pick(Vec::new());
        fn texts(node: &ViewNode, out: &mut Vec<String>) {
            if let Some(text) = &node.props.text
                && node.kind == NodeKind::Text
            {
                out.push(text.clone());
            }
            for child in &node.children {
                texts(child, out);
            }
        }
        let mut words = Vec::new();
        texts(&node, &mut words);
        assert!(words.iter().any(|w| w == "No matching folder"), "{words:?}");
        assert!(words.iter().any(|w| w == "Move Cargo.lock to…"));
    }
}
