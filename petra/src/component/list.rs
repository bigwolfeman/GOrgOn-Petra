//! Carbon List: ordered, unordered, and nested static markers (slice-c).
//!
//! The list container is not interactive (FR-058). List items that are only
//! text are not interactive either. Markers are generated here because Petra
//! has no CSS counters and no `::before` content.
//!
//! # Bullets are drawn, and there is a set of them
//!
//! Carbon writes both of its unordered markers as `::before` *text* — an en
//! dash at level 1 and a black small square at level 2 (`_list.scss:85,91`)
//! — and ships no third. The operator asked for the Word / org-mode set on
//! 2026-09-05: *"List: we need several types of bullet points like word or
//! emacs org mode has"* (`.agents/carbon-waves/ROUND4-DEFECTS.md` R7).
//!
//! [`Bullet`] is that set — dash, disc, ring, square — and [`BulletScheme`]
//! says how a list picks one per nesting level: [`BulletScheme::Carbon`]
//! (the default, and Carbon's own two), [`BulletScheme::Rotating`] (Word's
//! list gallery and org-mode's `org-superstar` cycle), or
//! [`BulletScheme::Fixed`]. A scheme reaches every level below the list it
//! is named on, which is the only way a rotation can be a rotation.
//!
//! Each bullet is a drawn [`IconMark`] on a canvas, never a character in a
//! string, following [`super::icon`]'s convention rather than starting a
//! second one. A typed marker cannot be sized, centred or snapped apart from
//! the label's font; the row centres the 16-unit mark against the 20-unit
//! line box instead. An ordered list's counter stays text, because a counter
//! *is* text.
//!
//! Nesting has no depth limit. It used to: `Level` had two values, so a
//! level-3 list drew level 2's marker and so did every level under it.
//!
//! # The hanging indent, measured
//!
//! Carbon hangs the marker in a gutter to the *left* of the label's edge:
//! the unordered container carries `margin-inline-start: $spacing-05` and
//! its level-1 marker sits at `inset-inline-start: calc(-1 * $spacing-05)`
//! (slice-c, `_list.scss:34,87`), so the marker lands on the list's own
//! left edge and every label starts 16 in. Read off `16-list.png`
//! (Carbon, 2x): "–" at the block's edge, "Inbox" 16 in; the nested "▪" 36
//! in and "2025" 52 in (32 nested indent + 4 item padding + 16); "1." 24
//! *outside* the block's edge and "Clone" on it (the ordered container has
//! no margin and its counter hangs at `-$spacing-06`).
//!
//! Petra's first version put the marker *inside* the indent: a 16 inset,
//! then the marker, then an 8 gap, then the label — so "–" sat 16 in and
//! "Inbox" 31 in, and the nested pair 58 / 74. Every line was the right
//! height and the block read as typed markdown, which is what the operator
//! called it twice (`.agents/carbon-waves/ROUND2-DEFECTS.md` row 16). Now
//! each item is one row: a **marker column** of fixed width — 16 for an
//! unordered marker, 24 for an ordered one (`$spacing-06`, the counter's
//! own hang) — with the marker at its start and the label at its end, no
//! gap and no inset. Labels form one left edge at 16 (unordered) or 24
//! (ordered) and markers hang in the column beside it. The one thing a
//! retained layout cannot copy is Carbon's ordered counter overflowing
//! *outside* its own list box; here the ordered list's box includes its
//! 24 gutter, so an ordered list's labels sit 24 further in than Carbon's
//! relative to the block, with the same marker-to-label distance.

use std::sync::Arc;

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::text;
use super::tokens::{SPACING_02, SPACING_07, t};
// `SPACING_06` (24) is the ordered marker column, cited by value in
// `MARKER_COLUMN_ORDERED` because a constraint is an extent, not a token ref.
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, InsetRefs, Key, Role, Semantics, ViewNode};

/// The unordered marker column: Carbon's `$spacing-05` hang (`_list.scss:87`).
const MARKER_COLUMN_UNORDERED: f32 = 16.0;
/// The ordered marker column: the counter's `$spacing-06` hang, room for
/// two digits and the full stop.
const MARKER_COLUMN_ORDERED: f32 = 24.0;
/// The ordered marker column from item 100 on: three digits need the next
/// step of the ramp (`$spacing-07`).
const MARKER_COLUMN_ORDERED_WIDE: f32 = 32.0;
/// The first zero-based index whose ordered marker has three digits.
const THREE_DIGITS_FROM: usize = 99;

const _: () = assert!(MARKER_COLUMN_UNORDERED == 16.0);
const _: () = assert!(MARKER_COLUMN_ORDERED == 24.0);
const _: () = assert!(MARKER_COLUMN_ORDERED_WIDE == 32.0);

/// One unordered marker, as a picture.
///
/// The set the operator asked for on 2026-09-05: *"we need several types of
/// bullet points like word or emacs org mode has"*
/// (`.agents/carbon-waves/ROUND4-DEFECTS.md` R7). Carbon ships two of these
/// four and writes both as `::before` text — `content: '\002013'` at level 1
/// and `'\0025AA\00FE0E'` at level 2, MEASURED
/// `ignored/carbon-ref/node_modules/@carbon/styles/scss/components/list/
/// _list.scss:85,91`. It has no disc and no ring; those are the Word /
/// org-mode half, and CSS's own `list-style-type: disc` / `circle`.
///
/// Every one is a drawn [`IconMark`], never a character in the label's
/// string, for the reason this module's own history gives: a typed marker
/// cannot be sized, centred or snapped apart from the label's font, and the
/// operator called that "badly formatted markdown", twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Bullet {
    /// An en dash. Carbon's level-1 marker (`_list.scss:85`), org-mode's
    /// plain `-`, and this library's default so a caller who names nothing
    /// still gets Carbon.
    #[default]
    Dash,
    /// A filled disc. `list-style-type: disc`, Word's first level.
    Disc,
    /// A hollow ring. `list-style-type: circle`, Word's second level.
    Circle,
    /// A filled square. Carbon's level-2 marker (`_list.scss:91`) drawn,
    /// `list-style-type: square`, Word's third level.
    Square,
}

impl Bullet {
    /// The mark this bullet draws.
    #[must_use]
    pub fn mark(self) -> IconMark {
        match self {
            Self::Dash => IconMark::BulletDash,
            Self::Disc => IconMark::BulletDisc,
            Self::Circle => IconMark::BulletCircle,
            Self::Square => IconMark::BulletSquare,
        }
    }
}

/// How an unordered list picks a [`Bullet`] for each nesting level.
///
/// Depth is 1-based: the outermost list is level 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BulletScheme {
    /// Carbon's own two: [`Bullet::Dash`] at level 1 and [`Bullet::Square`]
    /// at every level under it, which is what `_list.scss:80-93` writes
    /// (one rule for `--unordered` and one for `--unordered.--nested`, with
    /// no third). The default, so conformance is what a caller gets by
    /// saying nothing.
    #[default]
    Carbon,
    /// A different mark at every level, walking [`Bullet::Disc`],
    /// [`Bullet::Circle`], [`Bullet::Square`], [`Bullet::Dash`] and
    /// repeating — Word's list gallery and org-mode's `org-superstar`
    /// cycle. Off the conformance target rather than against it: Carbon
    /// writes no rule for a third level at all.
    Rotating,
    /// One named mark, at every level.
    Fixed(Bullet),
}

/// [`BulletScheme::Rotating`]'s cycle, in order.
const ROTATION: [Bullet; 4] = [Bullet::Disc, Bullet::Circle, Bullet::Square, Bullet::Dash];

impl BulletScheme {
    /// The bullet this scheme draws at 1-based nesting `depth`.
    fn bullet(self, depth: usize) -> Bullet {
        match self {
            Self::Carbon if depth <= 1 => Bullet::Dash,
            Self::Carbon => Bullet::Square,
            Self::Rotating => ROTATION[depth.saturating_sub(1) % ROTATION.len()],
            Self::Fixed(bullet) => bullet,
        }
    }
}

/// What one item hangs in its marker column: a counter, or a bullet.
#[derive(Clone, Debug)]
enum Marker {
    /// An ordered list's generated counter, `"1."` / `"a."` / `"i."`.
    Counter(String),
    /// An unordered list's drawn mark.
    Glyph(Bullet),
}

#[derive(Clone, Copy)]
enum Kind {
    Unordered,
    Ordered,
}

/// An unordered list under [`BulletScheme::Carbon`]. `Role::List`, no
/// interactions, no container inset: the marker column on each item is the
/// whole hanging indent (module doc).
pub fn unordered_list(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    unordered_list_with(key, BulletScheme::default(), items)
}

/// [`unordered_list`] with the bullet scheme named.
///
/// The scheme reaches every level below this list too, so one call decides
/// the whole nested block — which is how Word and org-mode both work, and is
/// the only way a rotation can be a rotation.
pub fn unordered_list_with(
    key: impl Into<Key>,
    scheme: BulletScheme,
    items: Vec<ViewNode>,
) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, items);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    restamp(&mut node, Kind::Unordered, 1, scheme);
    node
}

/// An ordered list. Level-1 markers are `1.` `2.` … generated here, level 2
/// is lower-latin and every level below it lower-roman.
/// `Role::List`, no interactions.
pub fn ordered_list(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, items);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    restamp(&mut node, Kind::Ordered, 1, BulletScheme::default());
    node
}

/// One list row: a fixed-width marker column then the label, which is the
/// hanging indent (module doc). Default marker is Carbon's level-1 en dash
/// in a 16 column; the list that adopts this item restamps both. A
/// standalone item carries no inset of its own: a bare `list_item` is
/// `Role::ListItem` with its marker on its own left edge. No interactions.
pub fn list_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    list_item_with(key, label, None)
}

/// [`list_item`] plus an optional nested list.
///
/// Nested indent is Carbon's 32px (`spacing-07`) on the nested container and
/// [`SPACING_02`] on each nested item (T070: SCSS wins over the style-page
/// `$spacing-05`), which with the 16 marker column puts a nested label 52
/// in from the parent item's edge — the figure `16-list.png` measures.
/// Nesting is not capped: each level takes the enclosing list's scheme one
/// step further, and an ordered list walks decimal, lower-latin, lower-roman.
pub fn list_item_with(
    key: impl Into<Key>,
    label: impl Into<String>,
    nested: Option<ViewNode>,
) -> ViewNode {
    let label = label.into();
    match nested {
        None => with_list_item_role(item_row(key, &label), label),
        Some(nested) => {
            let row = item_row("row", &label);
            let mut item = stack(key, Axis::Vertical, None, vec![row, nested]);
            // A standalone `list_item_with` is a complete picture on its
            // own: nothing has adopted it yet, so nobody else will stamp
            // its nested block. The list that does adopt it restamps from
            // its own depth, and every step here is an assignment, so
            // re-stamping is idempotent.
            restamp_nested(&mut item, 1, BulletScheme::default());
            with_list_item_role(item, label)
        }
    }
}

fn with_list_item_role(mut node: ViewNode, label: String) -> ViewNode {
    node.semantics = Semantics {
        role: Some(Role::ListItem),
        label: Some(label),
        ..Semantics::default()
    };
    node
}

fn item_row(key: impl Into<Key>, label: &str) -> ViewNode {
    let marker = marker_node(&Marker::Glyph(Bullet::default()), MARKER_COLUMN_UNORDERED);
    let mut row = stack(
        key,
        Axis::Horizontal,
        None,
        vec![marker, text("label", label)],
    );
    // A drawn bullet is a 16-unit square and the label's line box is 20, so
    // the two only share an optical centre line if the row centres them. A
    // typed marker rode the label's own baseline and needed nothing; this
    // is the one thing the picture costs.
    row.props.align = Some(Align::Center);
    row
}

/// The node an item hangs in its marker column, pinned to `column` so the
/// label after it starts at the column's end whatever the marker is.
fn marker_node(marker: &Marker, column: f32) -> ViewNode {
    let mut node = match marker {
        Marker::Counter(text_) => text("marker", text_.clone()),
        // `IconTone::Primary` is `$icon-primary`, which every theme assigns
        // from `text.primary` (`token/shipped.rs`'s `ICON_TOKENS` doc), so
        // the bullet is the same ink as the label Carbon gives `$text-primary`
        // (`_list.scss:44`) rather than a second grey.
        Marker::Glyph(bullet) => icon_toned("marker", bullet.mark(), IconTone::Primary),
    };
    set_column(&mut node, column);
    node
}

/// Pin a marker node to its column width, so the label after it starts at
/// the column's end whatever the marker is.
fn set_column(marker: &mut ViewNode, width: f32) {
    marker.constraints.horizontal = AxisConstraint {
        min: Some(width),
        max: Some(width),
        priority: 0,
    };
}

/// The marker column an item at `index` takes in a list of `kind`.
fn marker_column(kind: Kind, index: usize) -> f32 {
    match kind {
        Kind::Unordered => MARKER_COLUMN_UNORDERED,
        Kind::Ordered if index >= THREE_DIGITS_FROM => MARKER_COLUMN_ORDERED_WIDE,
        Kind::Ordered => MARKER_COLUMN_ORDERED,
    }
}

/// Stamp every item of `list` for 1-based `depth`, then recurse into each
/// item's own nested list one level deeper.
///
/// Depth used to be a two-valued `Level`, so level 3 drew level 2's marker
/// and every level under it drew the same one again — a list could not
/// nest past two in any way a reader could see. The operator asked for the
/// Word / org-mode rotation on 2026-09-05, and a rotation with two stops is
/// not one.
fn restamp(list: &mut ViewNode, kind: Kind, depth: usize, scheme: BulletScheme) {
    if list.semantics.role != Some(Role::List) {
        return;
    }
    for (index, item) in list.children.iter_mut().enumerate() {
        let item = Arc::make_mut(item);
        set_marker(
            item,
            &marker_for(kind, scheme, depth, index),
            marker_column(kind, index),
        );
        if depth > 1 {
            pad_inline_start(item, SPACING_02);
        }
        restamp_nested(item, depth, scheme);
    }
}

/// [`restamp`] for a list this module did not build the call for: read its
/// kind off the marker its own constructor already stamped.
///
/// An ordered list's marker is a generated counter and therefore text; every
/// unordered marker is a drawn canvas. The two never look alike, so one look
/// at the first item settles it. A list nested inside another arrives here
/// already stamped at level 1 by its own `ordered_list` / `unordered_list`
/// call, which is what makes that look reliable.
fn restamp_sniffed(list: &mut ViewNode, depth: usize, scheme: BulletScheme) {
    let kind = if list
        .children
        .first()
        .is_some_and(|item| marker_is_ordered(item))
    {
        Kind::Ordered
    } else {
        Kind::Unordered
    };
    restamp(list, kind, depth, scheme);
}

/// Carbon's nested indent on any list `item` holds, and that list's own
/// items one level deeper.
fn restamp_nested(item: &mut ViewNode, depth: usize, scheme: BulletScheme) {
    for child in &mut item.children {
        let child = Arc::make_mut(child);
        if child.semantics.role == Some(Role::List) {
            restamp_sniffed(child, depth + 1, scheme);
            pad_inline_start(child, SPACING_07);
        }
    }
}

fn marker_for(kind: Kind, scheme: BulletScheme, depth: usize, index: usize) -> Marker {
    match kind {
        Kind::Unordered => Marker::Glyph(scheme.bullet(depth)),
        Kind::Ordered => Marker::Counter(counter(depth, index)),
    }
}

/// An ordered list's counter at 1-based `depth`: decimal, then lower-latin,
/// then lower-roman. Carbon writes the first two — `list-style-type:
/// lower-latin` on `--ordered.--nested` (`_list.scss:74-77`) — and says
/// nothing at all about a third, so the third is HTML's own next step and
/// Word's default, not a Carbon claim.
fn counter(depth: usize, index: usize) -> String {
    match depth {
        0 | 1 => format!("{}.", index + 1),
        2 => latin_marker(index),
        _ => roman_marker(index),
    }
}

fn latin_marker(index: usize) -> String {
    let mut chars = Vec::new();
    let mut n = index;
    loop {
        chars.push(char::from(b'a' + (n % 26) as u8));
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    chars.reverse();
    let mut out: String = chars.into_iter().collect();
    out.push('.');
    out
}

/// Lower-roman for a 0-based `index`, subtractive, capped at the largest
/// number HTML's own `lower-roman` renders before it falls back to decimal.
fn roman_marker(index: usize) -> String {
    const PLACES: [(usize, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut n = index + 1;
    // Past 3999 there is no roman numeral, and CSS says a counter style
    // that cannot render a value falls back to decimal. Say so rather than
    // loop forever or draw an `mmmm`.
    if n > 3999 {
        return format!("{n}.");
    }
    let mut out = String::new();
    for (value, glyph) in PLACES {
        while n >= value {
            out.push_str(glyph);
            n -= value;
        }
    }
    out.push('.');
    out
}

/// Whether `item` hangs a generated counter rather than a drawn bullet.
///
/// The counter is the only marker in this module that is text; a bullet is
/// always a canvas. That is the whole test, and it is exact rather than a
/// guess at the counter's shape — the earlier version parsed the string for
/// a trailing full stop and alphanumerics, which a label like `"a."` in the
/// marker slot could have satisfied by accident.
fn marker_is_ordered(item: &ViewNode) -> bool {
    marker_text(item).is_some()
}

fn marker_text(node: &ViewNode) -> Option<&str> {
    for child in &node.children {
        match child.key.as_str() {
            "marker" => return child.props.text.as_deref(),
            "row" => return marker_text(child),
            _ => {}
        }
    }
    None
}

fn set_marker(item: &mut ViewNode, marker: &Marker, column: f32) {
    for child in &mut item.children {
        let child = Arc::make_mut(child);
        match child.key.as_str() {
            "marker" => {
                *child = marker_node(marker, column);
                return;
            }
            "row" => {
                set_marker(child, marker, column);
                return;
            }
            _ => {}
        }
    }
}

fn pad_inline_start(node: &mut ViewNode, token: &str) {
    node.props.padding = Some(InsetRefs {
        left: Some(t(token)),
        ..InsetRefs::default()
    });
}

#[cfg(test)]
mod tests {
    use super::{
        Bullet, BulletScheme, MARKER_COLUMN_ORDERED, MARKER_COLUMN_ORDERED_WIDE,
        MARKER_COLUMN_UNORDERED, SPACING_02, SPACING_07, list_item, list_item_with, marker_text,
        ordered_list, unordered_list, unordered_list_with,
    };
    use crate::component::icon::{IconTone, icon_toned};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, Role, ViewNode};
    use std::sync::Arc;

    fn padding_left(node: &ViewNode) -> Option<&str> {
        node.props
            .padding
            .as_ref()
            .and_then(|pad| pad.left.as_ref())
            .map(|name| name.as_str())
    }

    fn uses_padding_token(node: &ViewNode, token: &str) -> bool {
        padding_left(node) == Some(token)
            || node
                .children
                .iter()
                .any(|child| uses_padding_token(child, token))
    }

    fn nested_list(item: &ViewNode) -> &ViewNode {
        item.children
            .iter()
            .find(|child| child.semantics.role == Some(Role::List))
            .expect("list_item_with nests a Role::List child")
    }

    #[test]
    fn unordered_list_sets_role_list() {
        let node = super::unordered_list("u", vec![super::list_item("a", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }

    #[test]
    fn ordered_list_sets_role_list() {
        let node = super::ordered_list("o", vec![super::list_item("a", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }

    #[test]
    fn list_item_sets_role_list_item() {
        let node = super::list_item("a", "Alpha");
        assert_eq!(node.semantics.role, Some(Role::ListItem));
        assert_eq!(node.semantics.label.as_deref(), Some("Alpha"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(
            marker_text(&node),
            None,
            "an unordered marker is a drawn mark, not a character in a string"
        );
        assert_eq!(bullet_of(&node), Bullet::Dash, "Carbon's level-1 marker");
    }

    /// The one place a test reads a drawn bullet back: compare the marker's
    /// own draw list against the mark the [`Bullet`] names. Everything about
    /// an unordered marker is in that list — there is no text to read, and
    /// the operator cannot use hue, so the picture is the whole channel.
    fn bullet_of(item: &ViewNode) -> Bullet {
        let drawn = markers_by_level(&super::stack(
            "probe",
            Axis::Vertical,
            None,
            vec![item.clone()],
        ))
        .into_iter()
        .next()
        .expect("the item hangs a marker");
        for bullet in [Bullet::Dash, Bullet::Disc, Bullet::Circle, Bullet::Square] {
            let want = icon_toned("m", bullet.mark(), IconTone::Primary)
                .props
                .canvas
                .expect("a bullet is a canvas");
            if drawn.commands() == want.commands() {
                return bullet;
            }
        }
        panic!("the marker draws no bullet this module knows");
    }

    /// V3 visual audit, `07-contained-list.png`: `contained_list`'s header
    /// carries `padding-left: SPACING_05` (`contained_list.rs`), but a row
    /// built from a bare `list_item` (as the catalog does, not wrapped in
    /// `unordered_list`) had none, so "Recent" sat 17px right of "Trace" /
    /// "Store". SOURCED `slice-a.md` Contained list: "List item:
    /// `padding-left`/`right` 16px (`$spacing-05`)". A standalone item now
    /// carries that inset itself so it lines up with the header without
    /// needing a list wrapper.
    /// W8 audit, row 16 ("two adjacent lists run together"): measured on
    /// `16-list.png` against Carbon's own `16-list.png`, the item pitch is
    /// 20 logical units in both — Carbon's `$body-01` line box, with
    /// `margin-bottom: 0` on every item at both levels (slice-c:51, SOURCED
    /// style page). The extra air between Carbon's two demo lists is the
    /// reference page's own `.ref-body > * + * { margin-top: 1.5rem }`, not
    /// `_list.scss`. So the list binds no row gap, and this pins that: a
    /// gap added here to "fix" the picture would put a number in the
    /// component that Carbon does not have. The gap between two lists on a
    /// page belongs to the page.
    #[test]
    fn items_abut_with_no_row_gap_as_carbon_gives_them_zero_margin() {
        let unordered = unordered_list("u", vec![list_item("a", "A"), list_item("b", "B")]);
        let ordered = ordered_list("o", vec![list_item("a", "A"), list_item("b", "B")]);
        assert_eq!(unordered.props.spacing, None);
        assert_eq!(ordered.props.spacing, None);
        let item = list_item_with(
            "a",
            "A",
            Some(unordered_list("n", vec![list_item("c", "C")])),
        );
        assert_eq!(nested_list(&item).props.spacing, None);
    }

    fn marker_column(item: &ViewNode) -> Option<f32> {
        let marker = item
            .children
            .iter()
            .find(|c| c.key.as_str() == "marker" || c.key.as_str() == "row")?;
        if marker.key.as_str() == "row" {
            return marker_column(marker);
        }
        let h = &marker.constraints.horizontal;
        assert_eq!(h.min, h.max, "the marker column is pinned");
        h.min
    }

    /// Row 16, round 2 ("still badly formatted markdown"): the marker
    /// hangs in a fixed column on the item's own left edge and the label
    /// starts at the column's end — no item inset, no gap. 16 for an
    /// unordered marker, 24 for an ordered one, 32 from the hundredth.
    #[test]
    fn the_marker_hangs_in_a_fixed_column_and_the_item_has_no_inset() {
        let item = list_item("a", "Alpha");
        assert_eq!(
            padding_left(&item),
            None,
            "no inset: the column is the indent"
        );
        assert_eq!(item.props.spacing, None, "no gap: the column is the gap");
        assert_eq!(marker_column(&item), Some(MARKER_COLUMN_UNORDERED));
        assert_eq!(MARKER_COLUMN_UNORDERED, 16.0);

        let items: Vec<ViewNode> = (0..100).map(|i| list_item(format!("i{i}"), "x")).collect();
        let ordered = ordered_list("o", items);
        assert_eq!(
            marker_column(&ordered.children[0]),
            Some(MARKER_COLUMN_ORDERED)
        );
        assert_eq!(
            marker_column(&ordered.children[98]),
            Some(MARKER_COLUMN_ORDERED)
        );
        assert_eq!(MARKER_COLUMN_ORDERED, 24.0);
        assert_eq!(
            marker_column(&ordered.children[99]),
            Some(MARKER_COLUMN_ORDERED_WIDE),
            "\"100.\" needs the wider column"
        );
        assert_eq!(marker_text(&ordered.children[99]), Some("100."));
        assert_eq!(padding_left(&ordered.children[0]), None);
        assert_eq!(
            padding_left(&ordered),
            None,
            "the ordered container has no margin"
        );
    }

    /// The whole block, placed: labels form one left edge 16 in from the
    /// list's edge, markers sit on the edge, a nested label is 52 in and an
    /// ordered label 24 in — the figures measured off Carbon's own
    /// `16-list.png` (module doc), bar the ordered counter's overflow.
    #[test]
    fn placed_labels_form_carbons_hanging_indent() {
        let frame = petrify_lone(sample_lists());
        let x_of = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is not placed"))
                .rect
                .x
        };
        let ul = x_of("/lists/ul");
        assert_eq!(
            x_of("/ul/ul-0/marker"),
            ul,
            "the marker hangs on the list's edge"
        );
        assert_eq!(
            x_of("/ul/ul-0/label"),
            ul + 16.0,
            "unordered labels start 16 in"
        );
        assert_eq!(x_of("/ul/ul-1/label"), ul + 16.0, "and form one left edge");
        let ol = x_of("/lists/ol");
        assert_eq!(
            x_of("/ol/ol-0/marker"),
            ol,
            "the counter hangs on the list's edge"
        );
        assert_eq!(
            x_of("/ol/ol-0/label"),
            ol + 24.0,
            "ordered labels start 24 in"
        );
        let parent = x_of("/lists/with-nested");
        assert_eq!(x_of("/with-nested/row/label"), parent + 16.0);
        assert_eq!(
            x_of("/nested/n0/label"),
            parent + 52.0,
            "32 nested indent + 4 item padding + 16 marker column"
        );
        assert_eq!(x_of("/nested/n0/marker"), parent + 36.0);
    }

    #[test]
    fn ordered_list_generates_numbered_markers() {
        let node = super::ordered_list(
            "o",
            vec![
                super::list_item("a", "Alpha"),
                super::list_item("b", "Bravo"),
            ],
        );
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_eq!(marker_text(&node.children[0]), Some("1."));
        assert_eq!(marker_text(&node.children[1]), Some("2."));
        assert_eq!(node.children[0].semantics.role, Some(Role::ListItem));
        assert_eq!(node.children[1].semantics.role, Some(Role::ListItem));
    }

    #[test]
    fn nested_indent_uses_spacing_02() {
        let nested = super::unordered_list("inner", vec![super::list_item("n", "Nested")]);
        let item = super::list_item_with("outer-item", "Outer", Some(nested));
        assert_eq!(item.semantics.role, Some(Role::ListItem));
        let nested = nested_list(&item);
        assert_eq!(nested.semantics.role, Some(Role::List));
        assert_eq!(padding_left(nested), Some(SPACING_07));
        assert_eq!(padding_left(&nested.children[0]), Some(SPACING_02));
        assert!(
            uses_padding_token(&item, SPACING_02),
            "nested indent must bind SPACING_02"
        );
    }

    #[test]
    fn nested_unordered_uses_square_marker() {
        let nested = unordered_list("inner", vec![list_item("n", "Nested")]);
        let item = list_item_with("outer-item", "Outer", Some(nested));
        let nested = nested_list(&item);
        assert_eq!(
            bullet_of(&nested.children[0]),
            Bullet::Square,
            "Carbon's nested marker is the small square (`_list.scss:91`)"
        );
    }

    #[test]
    fn nested_ordered_uses_latin_markers() {
        let nested = ordered_list(
            "inner",
            vec![list_item("a", "Alpha"), list_item("b", "Bravo")],
        );
        let item = list_item_with("outer-item", "Outer", Some(nested));
        let nested = nested_list(&item);
        assert_eq!(marker_text(&nested.children[0]), Some("a."));
        assert_eq!(marker_text(&nested.children[1]), Some("b."));
        assert_eq!(padding_left(&nested.children[0]), Some(SPACING_02));
    }

    /// A four-level unordered list under `scheme`, for reading one bullet
    /// per level back out.
    fn four_levels(scheme: BulletScheme) -> ViewNode {
        unordered_list_with(
            "r",
            scheme,
            vec![list_item_with(
                "l1",
                "One",
                Some(unordered_list(
                    "l2",
                    vec![list_item_with(
                        "l2i",
                        "Two",
                        Some(unordered_list(
                            "l3",
                            vec![list_item_with(
                                "l3i",
                                "Three",
                                Some(unordered_list("l4", vec![list_item("l4i", "Four")])),
                            )],
                        )),
                    )],
                )),
            )],
        )
    }

    /// The marker canvases down one chain of nested lists, outermost first.
    fn markers_by_level(list: &ViewNode) -> Vec<Arc<crate::draw::DrawList>> {
        let mut out = Vec::new();
        let mut here = list;
        loop {
            let item = here.children.first().expect("a level with no item");
            let row = item
                .children
                .iter()
                .find(|c| c.key.as_str() == "row")
                .map_or(item.as_ref(), |c| c.as_ref());
            let marker = row
                .children
                .iter()
                .find(|c| c.key.as_str() == "marker")
                .expect("every row hangs a marker");
            out.push(
                marker
                    .props
                    .canvas
                    .clone()
                    .unwrap_or_else(|| panic!("{:?} draws no bullet", marker.key)),
            );
            match item
                .children
                .iter()
                .find(|c| c.semantics.role == Some(Role::List))
            {
                Some(next) => here = next,
                None => return out,
            }
        }
    }

    /// R7, the operator's round-4 line: *"List: we need several types of
    /// bullet points like word or emacs org mode has"*
    /// (`.agents/carbon-waves/ROUND4-DEFECTS.md`).
    ///
    /// [`BulletScheme::Rotating`] is Word's list gallery and org-mode's
    /// `org-superstar` cycle: a **different mark at every level**, walking
    /// disc, ring, square, dash and repeating. Read off the marker's own
    /// draw list, because the mark is the whole channel — this operator
    /// cannot use hue and a bullet carries no text.
    ///
    /// It also proves nesting is real past level 2. Until 2026-09-05 the
    /// depth was a two-valued `Level`, so a level-3 list drew a level-2
    /// marker and every level below it drew the same one again.
    #[test]
    fn a_rotating_scheme_draws_a_different_bullet_at_every_nesting_level() {
        let marks = markers_by_level(&four_levels(BulletScheme::Rotating));
        assert_eq!(marks.len(), 4, "four levels were built");
        let want = [Bullet::Disc, Bullet::Circle, Bullet::Square, Bullet::Dash];
        for (level, (drawn, bullet)) in marks.iter().zip(want).enumerate() {
            let expected = icon_toned("marker", bullet.mark(), IconTone::Primary)
                .props
                .canvas
                .expect("a bullet is a canvas");
            assert_eq!(
                drawn.commands(),
                expected.commands(),
                "level {} draws {:?}, not {bullet:?}",
                level + 1,
                drawn.commands()
            );
        }
        for (i, a) in marks.iter().enumerate() {
            for (j, b) in marks.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a.commands(),
                    b.commands(),
                    "levels {} and {} draw the same mark",
                    i + 1,
                    j + 1
                );
            }
        }
    }

    /// [`BulletScheme::Fixed`] answers the plain half of the operator's ask:
    /// one named mark, at every level, the way Word's "define new bullet"
    /// does.
    #[test]
    fn a_fixed_scheme_draws_one_named_bullet_at_every_level() {
        for bullet in [Bullet::Dash, Bullet::Disc, Bullet::Circle, Bullet::Square] {
            let marks = markers_by_level(&four_levels(BulletScheme::Fixed(bullet)));
            let expected = icon_toned("marker", bullet.mark(), IconTone::Primary)
                .props
                .canvas
                .expect("a bullet is a canvas");
            for (level, drawn) in marks.iter().enumerate() {
                assert_eq!(
                    drawn.commands(),
                    expected.commands(),
                    "{bullet:?}: level {} drew something else",
                    level + 1
                );
            }
        }
    }

    /// The default scheme is still Carbon's own two marks — an en dash at
    /// level 1 and the small square at every level under it
    /// (`_list.scss:85,91`) — so nothing that did not ask for the Word set
    /// moved. Drawn now rather than typed, which is the only change.
    #[test]
    fn the_default_scheme_is_carbons_own_dash_then_square() {
        let marks = markers_by_level(&four_levels(BulletScheme::default()));
        let want = [Bullet::Dash, Bullet::Square, Bullet::Square, Bullet::Square];
        for (level, (drawn, bullet)) in marks.iter().zip(want).enumerate() {
            let expected = icon_toned("marker", bullet.mark(), IconTone::Primary)
                .props
                .canvas
                .expect("a bullet is a canvas");
            assert_eq!(
                drawn.commands(),
                expected.commands(),
                "level {} is not Carbon's {bullet:?}",
                level + 1
            );
        }
        assert_eq!(
            unordered_list("u", vec![list_item("a", "A")]).children[0]
                .children
                .iter()
                .find(|c| c.key.as_str() == "marker")
                .and_then(|m| m.props.canvas.clone())
                .map(|c| c.commands().to_vec()),
            icon_toned("m", Bullet::Dash.mark(), IconTone::Primary)
                .props
                .canvas
                .map(|c| c.commands().to_vec()),
            "`unordered_list` without a scheme is Carbon"
        );
    }

    /// An ordered list nests too, and past level 2: decimal, lower-latin,
    /// then lower-roman, which is Word's own default and HTML's
    /// `list-style-type` ladder. Carbon writes only the first two
    /// (`_list.scss:74-77`, `list-style-type: lower-latin` on a nested
    /// ordered list) and says nothing about a third.
    #[test]
    fn an_ordered_list_walks_decimal_then_latin_then_roman() {
        let list = ordered_list(
            "o",
            vec![list_item_with(
                "l1",
                "One",
                Some(ordered_list(
                    "l2",
                    vec![list_item_with(
                        "l2i",
                        "Two",
                        Some(ordered_list("l3", vec![list_item("l3i", "Three")])),
                    )],
                )),
            )],
        );
        let mut here = &list;
        for want in ["1.", "a.", "i."] {
            let item = here.children.first().expect("a level with no item");
            assert_eq!(marker_text(item), Some(want));
            match item
                .children
                .iter()
                .find(|c| c.semantics.role == Some(Role::List))
            {
                Some(next) => here = next,
                None => break,
            }
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

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn sample_lists() -> ViewNode {
        let nested = unordered_list("nested", vec![list_item("n0", "Nested")]);
        super::stack(
            "lists",
            Axis::Vertical,
            None,
            vec![
                unordered_list(
                    "ul",
                    vec![list_item("ul-0", "Alpha"), list_item("ul-1", "Bravo")],
                ),
                ordered_list(
                    "ol",
                    vec![list_item("ol-0", "First"), list_item("ol-1", "Second")],
                ),
                list_item_with("with-nested", "Parent", Some(nested)),
            ],
        )
    }

    /// Check C/D: unordered, ordered, and a nested list all place with real
    /// rects, none of them outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(sample_lists());
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

    /// Check F is vacuous here: `_list.scss` defines no interaction state
    /// at all (slice-c), so `list_item` declares no `Interaction::Focus`
    /// and there is nothing for a focus tree to reach. Confirmed rather
    /// than assumed:
    #[test]
    fn list_items_declare_no_interaction() {
        assert!(!list_item("a", "Alpha").is_interactive());
        assert!(!unordered_list("u", vec![]).is_interactive());
        assert!(!ordered_list("o", vec![]).is_interactive());
    }

    /// Check E: marker and label ink against the page ground
    /// (`surface.base`, matching how `text()` itself is styled — `list.rs`
    /// binds no `background` of its own anywhere), read through
    /// `Props.opacity`.
    ///
    /// **Both kinds of marker.** A label and an ordered counter are text and
    /// bind `foreground`; an unordered bullet is a canvas and names its ink
    /// inside its draw list. Walking `Props.text` alone was the whole check
    /// until 2026-09-05, and the day the bullets became pictures is the day
    /// that walk would have stopped seeing them —
    /// [`crate::draw::DrawList::token_colours`] is why it still does.
    #[test]
    fn marker_and_label_ink_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            fn check(node: &ViewNode, name: &str, bg: ColorValue, theme: &Theme, min: f32) {
                let opacity = node.props.opacity.unwrap_or(1.0);
                let fg = color(theme, name).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= min,
                    "{:?} at {ratio:.2}:1 against {name} fails AA {min}:1",
                    node.key
                );
            }
            fn walk(node: &ViewNode, bg: ColorValue, theme: &Theme, min: f32) {
                if node.props.text.is_some()
                    && let Some(fg_name) = node.props.tokens.get("foreground")
                {
                    check(node, fg_name.as_str(), bg, theme, min);
                }
                if let Some(canvas) = &node.props.canvas {
                    let inks = canvas.token_colours();
                    assert!(
                        !inks.is_empty(),
                        "{:?} draws a mark that names no token",
                        node.key
                    );
                    for ink in inks {
                        check(node, ink, bg, theme, min);
                    }
                }
                for child in &node.children {
                    walk(child, bg, theme, min);
                }
            }
            let block = sample_lists();
            let mut seen_canvas = false;
            fn any_canvas(node: &ViewNode, seen: &mut bool) {
                *seen |= node.props.canvas.is_some();
                for child in &node.children {
                    any_canvas(child, seen);
                }
            }
            any_canvas(&block, &mut seen_canvas);
            assert!(
                seen_canvas,
                "no bullet in the sample is a canvas, so the canvas arm of \
                 this check proves nothing"
            );
            walk(&block, bg, &theme, MIN_TEXT_CONTRAST);
        }
    }
}
