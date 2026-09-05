//! `icon` — a digest-visible mark a labeled control can carry.
//!
//! # FR-026: a second channel, never the only one
//!
//! An icon MUST sit beside text or shape. It MUST never be the only channel
//! a part communicates through (Constitution VII, the same rule
//! [`crate::token::StatusToken`] already enforces for status). This
//! constructor is the mark itself: non-interactive, no role, no label of
//! its own. The labeled control that contains it owns those — the toggle
//! has the label. Making [`icon`] a `Role::Button` with no extra label
//! would turn the mark into a control the audit cannot name.
//!
//! A swatch is not a check. [`super::swatch`] is a filled rectangle, and a
//! square pretending to be a tick is the thing this file exists to stop.
//! The picture is a [`crate::draw::DrawList`] on a [`NodeKind::Canvas`], so
//! the path reaches the frame digest as the path.
//!
//! # Where the shapes come from
//!
//! Every mark after [`IconMark::Check`] is traced from `@carbon/icons-react`
//! 11.72.0, the same package the IBM Carbon reference app under
//! `ignored/carbon-ref/` renders. Each variant's doc records the source path
//! and its viewBox. Carbon ships a size-specific path for some glyphs at 16
//! and 20 and a 32 master for the rest; a mark drawn at a box Carbon has no
//! path for scales the 32 master, which is what the React component does.
//!
//! [`DrawList::new`] refuses a filled path that is not convex, so a glyph
//! with a notch is either split into convex pieces — a chevron is two
//! parallelograms, an X is two bars — or built from rectangles, or drawn as
//! a stroke along the centre of Carbon's outline. Rectangles snap to the
//! device grid (`Command::Rect { snap: true }`, the one command allowed to):
//! a one-unit bar straddling a half-pixel at 1x would otherwise smear into
//! two grey rows, the same failure the radio's half-pixel inset had
//! (`.agents/notes/proposed/bug-fix/2026-09-04-a-half-pixel-inset-snaps-a-small-mark-off-centre.md`).
//!
//! # Tone
//!
//! The fill is a choice the caller makes, defaulting to `text.on-accent`
//! ([`IconTone::OnAccent`]), which is right on an accent track and invisible
//! on a layer: a selected tag's check measured 1.44:1 against its own pill
//! while that fill was hardcoded. A mark on a layer ground says so with
//! [`IconTone::Primary`] or [`IconTone::Secondary`], Carbon's `$icon-primary`
//! and `$icon-secondary`.

use std::sync::Arc;

use super::tokens::{
    ACCENT_PRIMARY, ICON_DISABLED, ICON_PRIMARY, ICON_SECONDARY, TEXT_ON_ACCENT, t,
};
use crate::draw::{
    ColorRef, Command, Corners, DrawList, Paint, PathVerb, Stroke, Width, arc_verbs,
};
use crate::geom::{Point, Rect, Size};
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, ViewNode};

/// Logical extent of the [`IconMark::Check`] node on both axes.
///
/// Matches the Carbon small-toggle handle (`convert.to-rem(10px)`, slice-f)
/// and the status marker. The Carbon tick itself is the 6×5
/// `.cds--toggle__check` path, centred in this box.
const CHECK_BOX: f32 = 10.0;

/// Carbon's glyph box for a control's icon: 16×16 (`convert.to-rem(16px)`
/// on `.cds--accordion__arrow`, `.cds--snippet__icon`,
/// `.cds--search-magnifier-icon`, `.cds--side-nav__submenu-chevron > svg`,
/// the date picker's calendar and the tag's close, each sourced in the
/// slice that owns the component).
const GLYPH_BOX: f32 = 16.0;

/// Carbon's glyph box for a UI shell header action: 20×20. Not in the
/// inventory slices — `HeaderMenuButton.tsx` renders `<Menu size={20} />` /
/// `<Close size={20} />` and the reference app's `HeaderGlobalAction`s
/// render their icons at `size={20}` (`ignored/carbon-ref/src/pages.jsx`).
const HEADER_BOX: f32 = 20.0;

/// Bézier handle length for a quarter circle of unit radius.
const KAPPA: f32 = 0.552_284_8;

/// A named mark [`icon`] can draw.
///
/// The vocabulary is exactly the set of glyphs a shipped component carries.
/// Before 2026-09-04 it was one variant long and every other Carbon icon in
/// this library was spelled as its name in body text, which is most of what
/// made the catalog read as unfinished.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconMark {
    /// Carbon small-toggle on-tick (slice-f `.cds--toggle__check`).
    ///
    /// Source path, viewBox `0 0 6 5`:
    /// `M2.2 2.7L5 0 6 1 2.2 5 0 2.7 1 1.5z`.
    ///
    /// The one mark with its own box: [`CHECK_BOX`] (10), whichever
    /// [`IconBox`] a caller names, because the tick is the toggle handle's
    /// mark and the controls that centre it depend on that number.
    Check,
    /// Carbon `Calendar` (the date picker's field icon).
    ///
    /// Source path, viewBox `0 0 32 32`:
    /// `M26,4h-4V2h-2v2h-8V2h-2v2H6C4.9,4,4,4.9,4,6v20c0,1.1,0.9,2,2,2h20c1.1,0,2-0.9,2-2V6C28,4.9,27.1,4,26,4z
    /// M26,26H6V12h20V26z M26,10H6V6h4v2h2V6h8v2h2V6h4V10z`.
    /// Built from seven snapped rectangles: the frame's four bars, the
    /// header's lower bar, and the two ring stems; the 2-unit corner radius
    /// is half a pixel at 16 and is dropped.
    Calendar,
    /// Carbon `ChevronDown` (a closed dropdown, menu button, accordion
    /// item, expandable table row or side-nav sub-menu).
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M8 11L3 6 3.7 5.3 8 9.6 12.3 5.3 13 6z`; at 32, viewBox `0 0 32 32`:
    /// `M16 22L6 12 7.4 10.6 16 19.2 24.6 10.6 26 12z`. Two parallelograms
    /// split at the vertex.
    ChevronDown,
    /// Carbon `ChevronDown` rotated 180°, which is how Carbon draws every
    /// open state above (`transform: rotate(180deg)` on `.cds--list-box__menu-icon--open`,
    /// `.cds--menu-button__trigger--open svg`, the expanded side-nav
    /// sub-menu; `rotate(-90deg)` from `ChevronRight` on the open accordion
    /// and `rotate(270deg)` on the expanded table row land on the same
    /// glyph). Carbon's own `ChevronUp` is that path mirrored:
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M8 5L13 10 12.3 10.7 8 6.4 3.7 10.7 3 10z`; at 32, viewBox `0 0 32 32`:
    /// `M16 10L26 20 24.6 21.4 16 12.8 7.4 21.4 6 20z`.
    ChevronUp,
    /// Carbon `ChevronLeft` (a calendar's previous-month control).
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M10 13L5 8 10 3 10.7 3.7 6.4 8 10.7 12.3z`; at 32, viewBox
    /// `0 0 32 32`: `M20 26L10 16 20 6 21.4 7.4 12.8 16 21.4 24.6z`. Two
    /// parallelograms split at the vertex, the same anatomy as
    /// [`IconMark::ChevronDown`].
    ChevronLeft,
    /// Carbon `ChevronRight` (a calendar's next-month control, and the
    /// glyph Carbon rotates for the collapsed accordion and table row).
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M6 13L5.3 12.3 9.6 8 5.3 3.7 6 3 11 8z`; at 32, viewBox
    /// `0 0 32 32`: `M12 26L10.6 24.6 19.2 16 10.6 7.4 12 6 22 16z`.
    ChevronRight,
    /// Carbon `Close` (a dismissible tag's remove control, the open UI
    /// shell menu trigger).
    ///
    /// Source path, viewBox `0 0 32 32`:
    /// `M17.4141 16L24 9.4141 22.5859 8 16 14.5859 9.4143 8 8 9.4141 14.5859 16 8 22.5859 9.4143 24 16 17.4141 22.5859 24 24 22.5859 17.4141 16z`.
    /// Two diagonal bars, each a convex quad, crossing at the centre.
    Close,
    /// Carbon `Copy` (the code snippet's copy button).
    ///
    /// Source paths, viewBox `0 0 32 32`:
    /// `M28,10V28H10V10H28m0-2H10a2,2,0,0,0-2,2V28a2,2,0,0,0,2,2H28a2,2,0,0,0,2-2V10a2,2,0,0,0-2-2Z`
    /// and `M4,18H2V4A2,2,0,0,1,4,2H18V4H4Z`. The front square is four
    /// snapped bars and the back sheet's L is two; the 2-unit corner radii
    /// are a pixel at 16 and are dropped.
    Copy,
    /// Carbon `Add` (the number input's increment stepper).
    ///
    /// Source path, viewBox `0 0 32 32`:
    /// `M17 15L17 8 15 8 15 15 8 15 8 17 15 17 15 24 17 24 17 17 24 17 24 15z`.
    /// Two snapped bars crossing at the centre.
    Add,
    /// Carbon `Subtract` (the number input's decrement stepper).
    ///
    /// Source path, viewBox `0 0 32 32`: `M8 15H24V17H8z`. One snapped bar.
    Subtract,
    /// Carbon `Search` (the search field's magnifier, a header action).
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M15,14.3L10.7,10c1.9-2.3,1.6-5.8-0.7-7.7S4.2,0.7,2.3,3S0.7,8.8,3,10.7c2,1.7,5,1.7,7,0l4.3,4.3L15,14.3z
    /// M2,6.5C2,4,4,2,6.5,2S11,4,11,6.5S9,11,6.5,11S2,9,2,6.5z`; at 32,
    /// viewBox `0 0 32 32`:
    /// `M29,27.5859l-7.5521-7.5521a11.0177,11.0177,0,1,0-1.4141,1.4141L27.5859,29ZM4,13a9,9,0,1,1,9,9A9.01,9.01,0,0,1,4,13Z`.
    /// The lens is a ring, drawn as a stroked ellipse down the centre of
    /// Carbon's band; the handle is one filled quad.
    Search,
    /// Carbon `Menu` (the closed UI shell menu trigger, the "hamburger").
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M2 12H14V13H2zM2 9H14V10H2zM2 6H14V7H2zM2 3H14V4H2z`; at 20, viewBox
    /// `0 0 20 20`:
    /// `M2 14.8H18V16H2zM2 11.2H18V12.4H2zM2 7.6H18V8.8H2zM2 4H18V5.2H2z`.
    /// Four snapped bars.
    Menu,
    /// Carbon `Notification` (the UI shell's notifications action).
    ///
    /// Source path at 16, viewBox `0 0 16 16`:
    /// `M14.4,10.1L13,8.8V6.5c0-2.6-1.9-4.7-4.5-5v-1h-1v1C5,1.8,3,3.9,3,6.5v2.3l-1.4,1.3c-0.1,0.1-0.2,0.2-0.1,0.4V12c0,0.3,0.2,0.5,0.4,0.5c0,0,0,0,0.1,0h3.5C5.5,13.9,6.6,15,8,15s2.5-1.1,2.5-2.5H14c0.3,0,0.5-0.2,0.5-0.4c0,0,0,0,0-0.1v-1.5C14.5,10.4,14.4,10.2,14.4,10.1z
    /// M8,14c-0.8,0-1.5-0.7-1.5-1.5h3C9.5,13.3,8.8,14,8,14z
    /// M13.5,11.5h-11v-0.8l1.3-1.4C3.9,9.3,4,9.1,4,9V6.5c0-2.2,1.8-4,4-4s4,1.8,4,4V9c0,0.1,0.1,0.3,0.1,0.4l1.4,1.3V11.5z`;
    /// at 32, viewBox `0 0 32 32`:
    /// `M28.7071,19.293,26,16.5859V13a10.0136,10.0136,0,0,0-9-9.9492V1H15V3.0508A10.0136,10.0136,0,0,0,6,13v3.5859L3.2929,19.293A1,1,0,0,0,3,20v3a1,1,0,0,0,1,1h7v.7768a5.152,5.152,0,0,0,4.5,5.1987A5.0057,5.0057,0,0,0,21,25V24h7a1,1,0,0,0,1-1V20A1,1,0,0,0,28.7071,19.293ZM19,25a3,3,0,0,1-6,0V24h6Zm8-3H5V20.4141L7.707,17.707A1,1,0,0,0,8,17V13a8,8,0,0,1,16,0v4a1,1,0,0,0,.293.707L27,20.4141Z`.
    /// An outline glyph with two holes, so it is drawn as strokes down the
    /// centre of Carbon's band: the bell body as one closed path, the
    /// clapper as one open arc, the knob as a snapped bar.
    Notification,
    /// Carbon `Switcher` (the UI shell's app switcher action).
    ///
    /// Source path, viewBox `0 0 32 32`:
    /// `M14 4H18V8H14zM4 4H8V8H4zM24 4H28V8H24zM14 14H18V18H14zM4 14H8V18H4zM24 14H28V18H24zM14 24H18V28H14zM4 24H8V28H4zM24 24H28V28H24z`.
    /// Nine snapped squares.
    Switcher,
    /// Carbon `CaretLeft` (pagination's Previous button).
    ///
    /// Source path at 16, viewBox `0 0 16 16`: `M10 12L5 8 10 4z`; at 32,
    /// viewBox `0 0 32 32`: `M20 24L10 16 20 8z`. One filled triangle
    /// pointing left, on whole units at 16 so no vertex lands on a half
    /// pixel.
    CaretLeft,
    /// Carbon `CaretRight` (pagination's Next button).
    ///
    /// Source path at 16, viewBox `0 0 16 16`: `M6 4L11 8 6 12z`; at 32,
    /// viewBox `0 0 32 32`: `M12 8L22 16 12 24z`. [`IconMark::CaretLeft`]
    /// mirrored.
    CaretRight,
    /// Carbon `CheckmarkOutline` (a complete progress-indicator step).
    ///
    /// Source paths, viewBox `0 0 32 32`:
    /// `M16,2A14,14,0,1,0,30,16,14,14,0,0,0,16,2Zm0,26A12,12,0,1,1,28,16,12,12,0,0,1,16,28Z`
    /// and `M14 21.414L9 16.413 10.413 15 14 18.586 21.585 11 23 12.415 14 21.414z`.
    /// The ring, 14 outside and 12 inside, is one stroked ellipse down the
    /// centre of the band; the check is the same thick chevron as
    /// [`IconMark::Check`], split into two convex quads.
    CheckmarkOutline,
    /// Carbon `CircleDash` (a not-started progress-indicator step).
    ///
    /// Source path, viewBox `0 0 32 32`: ten dashes, each an 18° arc of the
    /// band between radius 12 and 14, on a 36° pitch starting at three
    /// o'clock — `M27.4 19.7l1.9.6A15.47 15.47 0 0030 16H28A11.48 11.48 0
    /// 0127.4 19.7z` is the first, and the other nine follow round the
    /// circle. Each dash is one stroked arc down the centre of the band.
    CircleDash,
    /// Carbon `Incomplete` (the current progress-indicator step).
    ///
    /// Source path, viewBox `0 0 32 32`: `M16 30V2a14 14 0 000 28z` is the
    /// solid left half; `M23.7642 6.8593l1.2851-1.5315A13.976 13.976 0
    /// 0020.8672 2.887l-.6836 1.8776A11.9729 11.9729 0 0123.7642 6.8593z`
    /// and three more like it are four 20° dashes of the band on a 40°
    /// pitch, symmetric about three o'clock, so the gap sits at three
    /// o'clock. The half disc is one closed path of two quarter arcs; the
    /// dashes are stroked arcs.
    Incomplete,
    /// Carbon `Checkmark` (the selected option of a list box: dropdown,
    /// select, selectable menu item). Not [`IconMark::Check`], which is the
    /// toggle handle's own 6×5 tick in its 10-unit box; this is the 16-unit
    /// glyph a row carries.
    ///
    /// Source path, viewBox `0 0 32 32`:
    /// `M13 24L4 15 5.414 13.586 13 21.171 26.586 5.586 28 7z`. A thick
    /// chevron, split at the inner V into two convex quads the same way the
    /// toggle's tick is.
    Checkmark,
}

/// The box a mark is drawn in: which of Carbon's two glyph sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconBox {
    /// [`GLYPH_BOX`], 16: a control's icon.
    Glyph,
    /// [`HEADER_BOX`], 20: a UI shell header action's icon.
    Header,
}

/// The fill a mark is drawn in.
///
/// Three names and not a free token string, so a caller cannot invent a tone
/// (gate C1-8) and so the choice reads as what it is: which ground the mark
/// sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconTone {
    /// `text.on-accent`: the mark sits on an accent fill (the toggle's on
    /// track, a checked box). The default, and what every mark drew before
    /// the slot existed.
    OnAccent,
    /// `icon-primary`: the mark sits on a layer and is the control's own
    /// mark (a chevron, a close, a stepper).
    Primary,
    /// `icon-secondary`: the mark sits on a layer and is furniture (the
    /// search field's magnifier, a resting header action).
    Secondary,
    /// `icon-disabled`: the mark is a control's own mark and the control is
    /// unavailable (pagination's Previous on page 1). Carbon's
    /// `$icon-disabled` is `$icon-primary` at 25%, and the caret is the
    /// only thing in an icon button, so a caret drawn at the live tone would
    /// leave a disabled button indistinguishable from a live one.
    Disabled,
    /// `accent.primary`: the mark sits on a layer and **is** the accent —
    /// Carbon's `fill: $interactive` on a complete or current progress step
    /// (`_progress-indicator.scss`, `.cds--progress-step svg`).
    Accent,
}

impl IconTone {
    fn token(self) -> &'static str {
        match self {
            Self::OnAccent => TEXT_ON_ACCENT,
            Self::Primary => ICON_PRIMARY,
            Self::Secondary => ICON_SECONDARY,
            Self::Disabled => ICON_DISABLED,
            Self::Accent => ACCENT_PRIMARY,
        }
    }
}

impl IconMark {
    /// The logical extent of this mark's node on both axes at `boxed`.
    #[must_use]
    pub fn extent(self, boxed: IconBox) -> f32 {
        match (self, boxed) {
            (Self::Check, _) => CHECK_BOX,
            (_, IconBox::Glyph) => GLYPH_BOX,
            (_, IconBox::Header) => HEADER_BOX,
        }
    }
}

/// A non-interactive visual mark, used inside a labeled control.
///
/// `mark` chooses the picture, drawn at [`IconBox::Glyph`] in
/// [`IconTone::OnAccent`]. No role, no interactions: those belong to the
/// control that contains this node. [`icon_toned`] and [`icon_in`] are the
/// same node with the tone and the box chosen.
#[must_use]
pub fn icon(key: impl Into<Key>, mark: IconMark) -> ViewNode {
    icon_in(key, mark, IconBox::Glyph, IconTone::OnAccent)
}

/// [`icon`] at [`IconBox::Glyph`] in `tone`.
#[must_use]
pub fn icon_toned(key: impl Into<Key>, mark: IconMark, tone: IconTone) -> ViewNode {
    icon_in(key, mark, IconBox::Glyph, tone)
}

/// [`icon`] with both the box and the tone chosen.
#[must_use]
pub fn icon_in(key: impl Into<Key>, mark: IconMark, boxed: IconBox, tone: IconTone) -> ViewNode {
    let size = mark.extent(boxed);
    let list = draw_list(mark, boxed, tone);
    ViewNode::new(NodeKind::Canvas, key)
        .with_props(Props {
            canvas: Some(Arc::new(list)),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(size),
                max: Some(size),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(size),
                max: Some(size),
                priority: 0,
            },
        })
}

/// The mark's picture.
///
/// # Panics
/// Never in practice: every glyph is a handful of convex quads, snapped
/// rectangles, ellipses and strokes, well inside every draw-list bound. A
/// panic here means an edit broke a glyph's convexity, which is a defect and
/// not a runtime condition to handle.
fn draw_list(mark: IconMark, boxed: IconBox, tone: IconTone) -> DrawList {
    let color = ColorRef::Token(t(tone.token()).as_str().to_owned());
    let size = mark.extent(boxed);
    let commands = match mark {
        IconMark::Check => check_mark(color),
        IconMark::Calendar => calendar(size / 32.0, color),
        IconMark::ChevronDown => chevron_down(boxed, color),
        IconMark::ChevronUp => chevron_up(boxed, color),
        IconMark::ChevronLeft => chevron_left(boxed, color),
        IconMark::ChevronRight => chevron_right(boxed, color),
        IconMark::Close => close(size / 32.0, color),
        IconMark::Copy => copy(size / 32.0, color),
        IconMark::Add => add(size / 32.0, color),
        IconMark::Subtract => subtract(size / 32.0, color),
        IconMark::Search => search(boxed, color),
        IconMark::Menu => menu(boxed, color),
        IconMark::Notification => notification(boxed, color),
        IconMark::Switcher => switcher(size / 32.0, color),
        IconMark::CaretLeft => caret_left(size / 32.0, color),
        IconMark::CaretRight => caret_right(size / 32.0, color),
        IconMark::CheckmarkOutline => checkmark_outline(size / 32.0, color),
        IconMark::CircleDash => circle_dash(size / 32.0, color),
        IconMark::Incomplete => incomplete(size / 32.0, color),
        IconMark::Checkmark => checkmark(size / 32.0, color),
    };
    DrawList::new(commands).unwrap_or_else(|err| panic!("{mark:?} draw list refused: {err}"))
}

/// Carbon `.cds--toggle__check` as two convex filled quads.
///
/// The six-vertex outline is a thick chevron, and [`DrawList::new`] refuses
/// a filled non-convex path (`contracts/draw-list.md` §5). Split on the
/// inner-V to bottom-tip diagonal, the same way the cat's ears are two
/// triangles rather than two notches in the head.
///
/// The 6×5 viewBox is centred in the 10×10 node: offset `(2.0, 2.5)`.
fn check_mark(color: ColorRef) -> Vec<Command> {
    let ox = 2.0;
    let oy = 2.5;
    let p = |x: f32, y: f32| Point::new(x + ox, y + oy);
    // Vertex letters follow the Carbon path order:
    // A inner V, B inner top of the long arm, C outer top, D bottom tip,
    // E outer left of the short arm, F inner left.
    let a = p(2.2, 2.7);
    let b = p(5.0, 0.0);
    let c = p(6.0, 1.0);
    let d = p(2.2, 5.0);
    let e = p(0.0, 2.7);
    let f = p(1.0, 1.5);
    let paint = Paint::filled(color);
    vec![
        filled_quad([a, b, c, d], 1.0, paint.clone()),
        filled_quad([a, d, e, f], 1.0, paint),
    ]
}

/// Carbon `Checkmark` as two convex filled quads, split on the inner V to
/// bottom-tip diagonal exactly as [`check_mark`] is. Vertex letters follow
/// the Carbon path order: A bottom tip, B outer left end, C inner left end,
/// D inner V, E inner right end, F outer right end.
fn checkmark(s: f32, color: ColorRef) -> Vec<Command> {
    let a = pt(13.0, 24.0);
    let b = pt(4.0, 15.0);
    let c = pt(5.414, 13.586);
    let d = pt(13.0, 21.171);
    let e = pt(26.586, 5.586);
    let f = pt(28.0, 7.0);
    let paint = Paint::filled(color);
    vec![
        filled_quad([a, b, c, d], s, paint.clone()),
        filled_quad([a, d, e, f], s, paint),
    ]
}

fn calendar(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    [
        // Frame: top, left, right, bottom bars.
        (4.0, 4.0, 28.0, 6.0),
        (4.0, 4.0, 6.0, 28.0),
        (26.0, 4.0, 28.0, 28.0),
        (4.0, 26.0, 28.0, 28.0),
        // The bar under the header band, between the two holes.
        (4.0, 10.0, 28.0, 12.0),
        // Ring stems: above the frame, through the top bar, into the band.
        (10.0, 2.0, 12.0, 8.0),
        (20.0, 2.0, 22.0, 8.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect()
}

fn chevron_down(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    match boxed {
        IconBox::Glyph => vec![
            filled_quad(
                [pt(3.0, 6.0), pt(3.7, 5.3), pt(8.0, 9.6), pt(8.0, 11.0)],
                1.0,
                paint.clone(),
            ),
            filled_quad(
                [pt(8.0, 11.0), pt(8.0, 9.6), pt(12.3, 5.3), pt(13.0, 6.0)],
                1.0,
                paint,
            ),
        ],
        IconBox::Header => {
            let s = HEADER_BOX / 32.0;
            vec![
                filled_quad(
                    [pt(6.0, 12.0), pt(7.4, 10.6), pt(16.0, 19.2), pt(16.0, 22.0)],
                    s,
                    paint.clone(),
                ),
                filled_quad(
                    [
                        pt(16.0, 22.0),
                        pt(16.0, 19.2),
                        pt(24.6, 10.6),
                        pt(26.0, 12.0),
                    ],
                    s,
                    paint,
                ),
            ]
        }
    }
}

/// Carbon `ChevronLeft`, split at its vertex the way [`chevron_down`] is.
fn chevron_left(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    match boxed {
        IconBox::Glyph => vec![
            filled_quad(
                [pt(10.0, 3.0), pt(10.7, 3.7), pt(6.4, 8.0), pt(5.0, 8.0)],
                1.0,
                paint.clone(),
            ),
            filled_quad(
                [pt(5.0, 8.0), pt(6.4, 8.0), pt(10.7, 12.3), pt(10.0, 13.0)],
                1.0,
                paint,
            ),
        ],
        IconBox::Header => {
            let s = HEADER_BOX / 32.0;
            vec![
                filled_quad(
                    [pt(20.0, 6.0), pt(21.4, 7.4), pt(12.8, 16.0), pt(10.0, 16.0)],
                    s,
                    paint.clone(),
                ),
                filled_quad(
                    [
                        pt(10.0, 16.0),
                        pt(12.8, 16.0),
                        pt(21.4, 24.6),
                        pt(20.0, 26.0),
                    ],
                    s,
                    paint,
                ),
            ]
        }
    }
}

/// Carbon `ChevronRight`, split at its vertex the way [`chevron_down`] is.
fn chevron_right(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    match boxed {
        IconBox::Glyph => vec![
            filled_quad(
                [pt(6.0, 3.0), pt(11.0, 8.0), pt(9.6, 8.0), pt(5.3, 3.7)],
                1.0,
                paint.clone(),
            ),
            filled_quad(
                [pt(11.0, 8.0), pt(6.0, 13.0), pt(5.3, 12.3), pt(9.6, 8.0)],
                1.0,
                paint,
            ),
        ],
        IconBox::Header => {
            let s = HEADER_BOX / 32.0;
            vec![
                filled_quad(
                    [pt(12.0, 6.0), pt(22.0, 16.0), pt(19.2, 16.0), pt(10.6, 7.4)],
                    s,
                    paint.clone(),
                ),
                filled_quad(
                    [
                        pt(22.0, 16.0),
                        pt(12.0, 26.0),
                        pt(10.6, 24.6),
                        pt(19.2, 16.0),
                    ],
                    s,
                    paint,
                ),
            ]
        }
    }
}

fn chevron_up(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    match boxed {
        IconBox::Glyph => vec![
            filled_quad(
                [pt(8.0, 5.0), pt(13.0, 10.0), pt(12.3, 10.7), pt(8.0, 6.4)],
                1.0,
                paint.clone(),
            ),
            filled_quad(
                [pt(8.0, 6.4), pt(3.7, 10.7), pt(3.0, 10.0), pt(8.0, 5.0)],
                1.0,
                paint,
            ),
        ],
        IconBox::Header => {
            let s = HEADER_BOX / 32.0;
            vec![
                filled_quad(
                    [
                        pt(16.0, 10.0),
                        pt(26.0, 20.0),
                        pt(24.6, 21.4),
                        pt(16.0, 12.8),
                    ],
                    s,
                    paint.clone(),
                ),
                filled_quad(
                    [pt(16.0, 12.8), pt(7.4, 21.4), pt(6.0, 20.0), pt(16.0, 10.0)],
                    s,
                    paint,
                ),
            ]
        }
    }
}

fn close(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    vec![
        // The bar from top-left to bottom-right.
        filled_quad(
            [
                pt(9.4143, 8.0),
                pt(24.0, 22.5859),
                pt(22.5859, 24.0),
                pt(8.0, 9.4141),
            ],
            s,
            paint.clone(),
        ),
        // The bar from top-right to bottom-left.
        filled_quad(
            [
                pt(22.5859, 8.0),
                pt(24.0, 9.4141),
                pt(9.4143, 24.0),
                pt(8.0, 22.5859),
            ],
            s,
            paint,
        ),
    ]
}

fn copy(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    [
        // The front sheet's outline, 8..30 outside and 10..28 inside.
        (8.0, 8.0, 30.0, 10.0),
        (8.0, 28.0, 30.0, 30.0),
        (8.0, 10.0, 10.0, 28.0),
        (28.0, 10.0, 30.0, 28.0),
        // The back sheet's L.
        (2.0, 2.0, 18.0, 4.0),
        (2.0, 4.0, 4.0, 18.0),
    ]
    .into_iter()
    .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, s, paint.clone()))
    .collect()
}

fn add(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    vec![
        snapped_bar(15.0, 8.0, 17.0, 24.0, s, paint.clone()),
        snapped_bar(8.0, 15.0, 24.0, 17.0, s, paint),
    ]
}

fn subtract(s: f32, color: ColorRef) -> Vec<Command> {
    vec![snapped_bar(8.0, 15.0, 24.0, 17.0, s, Paint::filled(color))]
}

fn search(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    // The lens ring's centre, radius down the middle of Carbon's band, band
    // width, and the handle's four corners, in the source viewBox.
    let (s, center, radius, band, handle) = match boxed {
        IconBox::Glyph => (
            1.0,
            pt(6.5, 6.5),
            5.0,
            1.0,
            [
                pt(10.7, 10.0),
                pt(15.0, 14.3),
                pt(14.3, 15.0),
                pt(10.0, 10.7),
            ],
        ),
        IconBox::Header => (
            HEADER_BOX / 32.0,
            pt(13.0, 13.0),
            10.0,
            2.0,
            [
                pt(21.4479, 20.0338),
                pt(29.0, 27.5859),
                pt(27.5859, 29.0),
                pt(20.0338, 21.4479),
            ],
        ),
    };
    vec![
        Command::Ellipse {
            center: scaled(center, s),
            radii: Size::new(radius * s, radius * s),
            paint: Paint::stroked(Stroke {
                width: Width::Logical(band * s),
                color: color.clone(),
            }),
        },
        filled_quad(handle, s, Paint::filled(color)),
    ]
}

fn menu(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let bars: [(f32, f32, f32, f32); 4] = match boxed {
        IconBox::Glyph => [
            (2.0, 3.0, 14.0, 4.0),
            (2.0, 6.0, 14.0, 7.0),
            (2.0, 9.0, 14.0, 10.0),
            (2.0, 12.0, 14.0, 13.0),
        ],
        IconBox::Header => [
            (2.0, 4.0, 18.0, 5.2),
            (2.0, 7.6, 18.0, 8.8),
            (2.0, 11.2, 18.0, 12.4),
            (2.0, 14.8, 18.0, 16.0),
        ],
    };
    bars.into_iter()
        .map(|(x0, y0, x1, y1)| snapped_bar(x0, y0, x1, y1, 1.0, paint.clone()))
        .collect()
}

fn notification(boxed: IconBox, color: ColorRef) -> Vec<Command> {
    // Everything in the source viewBox, on the centreline of Carbon's band:
    // the dome's centre and radius, the skirt's corners from the dome's
    // right shoulder round to its left, the clapper's centre and radius, the
    // knob's bar, and the band width.
    let (s, dome, dome_r, skirt, clapper, clapper_r, knob, band) = match boxed {
        IconBox::Glyph => (
            1.0,
            pt(8.0, 6.5),
            4.5,
            [
                pt(12.5, 8.9),
                pt(14.0, 10.4),
                pt(14.0, 12.0),
                pt(2.0, 12.0),
                pt(2.0, 10.4),
                pt(3.5, 8.9),
            ],
            pt(8.0, 12.5),
            2.0,
            (7.5, 0.5, 8.5, 2.0),
            1.0,
        ),
        IconBox::Header => (
            HEADER_BOX / 32.0,
            pt(16.0, 13.0),
            9.0,
            [
                pt(25.0, 16.8),
                pt(28.0, 19.8),
                pt(28.0, 23.0),
                pt(4.0, 23.0),
                pt(4.0, 19.8),
                pt(7.0, 16.8),
            ],
            pt(16.0, 24.0),
            4.0,
            (15.0, 1.0, 17.0, 4.0),
            2.0,
        ),
    };
    let stroke = |closed: bool, verbs: Vec<PathVerb>| Command::Path {
        verbs,
        closed,
        paint: Paint::stroked(Stroke {
            width: Width::Logical(band * s),
            color: color.clone(),
        }),
    };
    let k = KAPPA * dome_r;
    let mut body = vec![
        PathVerb::MoveTo(scaled(pt(dome.x - dome_r, dome.y), s)),
        // Up and over the dome: two quarter arcs, left shoulder to crown to
        // right shoulder.
        PathVerb::CubicTo {
            c1: scaled(pt(dome.x - dome_r, dome.y - k), s),
            c2: scaled(pt(dome.x - k, dome.y - dome_r), s),
            to: scaled(pt(dome.x, dome.y - dome_r), s),
        },
        PathVerb::CubicTo {
            c1: scaled(pt(dome.x + k, dome.y - dome_r), s),
            c2: scaled(pt(dome.x + dome_r, dome.y - k), s),
            to: scaled(pt(dome.x + dome_r, dome.y), s),
        },
    ];
    body.extend(skirt.into_iter().map(|p| PathVerb::LineTo(scaled(p, s))));
    let k = KAPPA * clapper_r;
    let tongue = vec![
        PathVerb::MoveTo(scaled(pt(clapper.x - clapper_r, clapper.y), s)),
        PathVerb::CubicTo {
            c1: scaled(pt(clapper.x - clapper_r, clapper.y + k), s),
            c2: scaled(pt(clapper.x - k, clapper.y + clapper_r), s),
            to: scaled(pt(clapper.x, clapper.y + clapper_r), s),
        },
        PathVerb::CubicTo {
            c1: scaled(pt(clapper.x + k, clapper.y + clapper_r), s),
            c2: scaled(pt(clapper.x + clapper_r, clapper.y + k), s),
            to: scaled(pt(clapper.x + clapper_r, clapper.y), s),
        },
    ];
    let (x0, y0, x1, y1) = knob;
    vec![
        stroke(true, body),
        stroke(false, tongue),
        snapped_bar(x0, y0, x1, y1, s, Paint::filled(color)),
    ]
}

fn switcher(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color);
    let starts = [4.0, 14.0, 24.0];
    starts
        .into_iter()
        .flat_map(|y| starts.into_iter().map(move |x| (x, y)))
        .map(|(x, y)| snapped_bar(x, y, x + 4.0, y + 4.0, s, paint.clone()))
        .collect()
}

fn caret_left(s: f32, color: ColorRef) -> Vec<Command> {
    vec![filled_tri(
        [pt(20.0, 24.0), pt(10.0, 16.0), pt(20.0, 8.0)],
        s,
        Paint::filled(color),
    )]
}

fn caret_right(s: f32, color: ColorRef) -> Vec<Command> {
    vec![filled_tri(
        [pt(12.0, 8.0), pt(22.0, 16.0), pt(12.0, 24.0)],
        s,
        Paint::filled(color),
    )]
}

/// The three step glyphs share one circle: Carbon's band runs from radius
/// 12 to 14 in the 32 viewBox, so its centreline is 13 and it is 2 wide.
const STEP_RING_RADIUS: f32 = 13.0;
const STEP_RING_BAND: f32 = 2.0;
/// Where the step glyphs' circle sits: the viewBox centre.
const STEP_CENTER: Point = Point { x: 16.0, y: 16.0 };

/// One dash of a step ring: the band's centreline from `from` degrees
/// through `sweep` degrees, clockwise on screen from three o'clock.
fn ring_dash(from: f32, sweep: f32, s: f32, color: ColorRef) -> Command {
    Command::Path {
        verbs: arc_verbs(
            scaled(STEP_CENTER, s),
            STEP_RING_RADIUS * s,
            from.to_radians(),
            sweep.to_radians(),
        ),
        closed: false,
        paint: Paint::stroked(Stroke {
            width: Width::Logical(STEP_RING_BAND * s),
            color,
        }),
    }
}

fn checkmark_outline(s: f32, color: ColorRef) -> Vec<Command> {
    let paint = Paint::filled(color.clone());
    // Vertex letters as in `check_mark`: A inner V, B inner top of the
    // long arm, C outer top, D bottom tip, E outer left of the short arm,
    // F inner left.
    let a = pt(14.0, 18.586);
    let b = pt(21.585, 11.0);
    let c = pt(23.0, 12.415);
    let d = pt(14.0, 21.414);
    let e = pt(9.0, 16.413);
    let f = pt(10.413, 15.0);
    vec![
        Command::Ellipse {
            center: scaled(STEP_CENTER, s),
            radii: Size::new(STEP_RING_RADIUS * s, STEP_RING_RADIUS * s),
            paint: Paint::stroked(Stroke {
                width: Width::Logical(STEP_RING_BAND * s),
                color,
            }),
        },
        filled_quad([a, b, c, d], s, paint.clone()),
        filled_quad([a, d, e, f], s, paint),
    ]
}

fn circle_dash(s: f32, color: ColorRef) -> Vec<Command> {
    // Ten 18° dashes on a 36° pitch, the first starting at three o'clock.
    (0..10)
        .map(|k| ring_dash(36.0 * k as f32, 18.0, s, color.clone()))
        .collect()
}

fn incomplete(s: f32, color: ColorRef) -> Vec<Command> {
    // The solid left half: from twelve o'clock, anticlockwise on screen
    // through nine o'clock to six o'clock, closed on the vertical chord.
    // `arc_verbs`' control points sit 1.14 r out, so at r = 14 the box edge
    // at 16 is cleared by 0.02 of a viewBox unit.
    let half = Command::Path {
        verbs: arc_verbs(
            scaled(STEP_CENTER, s),
            14.0 * s,
            (-90.0_f32).to_radians(),
            (-180.0_f32).to_radians(),
        ),
        closed: true,
        paint: Paint::filled(color.clone()),
    };
    // Four 20° dashes on a 40° pitch, symmetric about three o'clock: the
    // gap straddles three o'clock, as Carbon's does.
    let mut commands = vec![half];
    commands.extend(
        [10.0, 50.0, -30.0, -70.0]
            .into_iter()
            .map(|from| ring_dash(from, 20.0, s, color.clone())),
    );
    commands
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn scaled(p: Point, s: f32) -> Point {
    Point::new(p.x * s, p.y * s)
}

/// A closed, filled, convex four-vertex path, its corners scaled by `s`.
fn filled_quad(corners: [Point; 4], s: f32, paint: Paint) -> Command {
    let [p0, p1, p2, p3] = corners.map(|p| scaled(p, s));
    Command::Path {
        verbs: vec![
            PathVerb::MoveTo(p0),
            PathVerb::LineTo(p1),
            PathVerb::LineTo(p2),
            PathVerb::LineTo(p3),
        ],
        closed: true,
        paint,
    }
}

/// One filled triangle, the convex figure every Carbon caret is.
fn filled_tri(corners: [Point; 3], s: f32, paint: Paint) -> Command {
    let [p0, p1, p2] = corners.map(|p| scaled(p, s));
    Command::Path {
        verbs: vec![
            PathVerb::MoveTo(p0),
            PathVerb::LineTo(p1),
            PathVerb::LineTo(p2),
        ],
        closed: true,
        paint,
    }
}

/// An axis-aligned bar from `(x0, y0)` to `(x1, y1)` in the source viewBox,
/// scaled by `s`, snapped to the device grid at paint time.
fn snapped_bar(x0: f32, y0: f32, x1: f32, y1: f32, s: f32, paint: Paint) -> Command {
    Command::Rect {
        rect: Rect::new(x0 * s, y0 * s, (x1 - x0) * s, (y1 - y0) * s),
        radius: Corners::default(),
        snap: true,
        paint,
    }
}

#[cfg(test)]
mod tests {
    use super::{CHECK_BOX, GLYPH_BOX, HEADER_BOX, IconBox, IconMark, IconTone, icon, icon_in};
    use crate::component::tokens::{
        ACCENT_PRIMARY, ICON_DISABLED, ICON_PRIMARY, ICON_SECONDARY, TEXT_ON_ACCENT,
    };
    use crate::draw::{ColorRef, Command, DrawList};
    use crate::tree::NodeKind;

    const EVERY_MARK: [IconMark; 20] = [
        IconMark::CaretLeft,
        IconMark::CaretRight,
        IconMark::CheckmarkOutline,
        IconMark::CircleDash,
        IconMark::Incomplete,
        IconMark::Check,
        IconMark::Checkmark,
        IconMark::Calendar,
        IconMark::ChevronDown,
        IconMark::ChevronUp,
        IconMark::ChevronLeft,
        IconMark::ChevronRight,
        IconMark::Close,
        IconMark::Copy,
        IconMark::Add,
        IconMark::Subtract,
        IconMark::Search,
        IconMark::Menu,
        IconMark::Notification,
        IconMark::Switcher,
    ];

    fn list(node: &crate::tree::ViewNode) -> &DrawList {
        node.props
            .canvas
            .as_ref()
            .expect("an icon is a canvas with a draw list")
    }

    /// Every colour a list paints with, fills and strokes alike.
    fn colours(list: &DrawList) -> Vec<String> {
        let mut out = Vec::new();
        for command in list.commands() {
            let paint = match command {
                Command::Rect { paint, .. }
                | Command::Ellipse { paint, .. }
                | Command::Path { paint, .. } => paint,
                _ => continue,
            };
            for reference in paint
                .fill
                .iter()
                .chain(paint.stroke.as_ref().map(|s| &s.color))
            {
                match reference {
                    ColorRef::Token(name) => out.push(name.clone()),
                    ColorRef::Rgba(_) => panic!("an icon binds a token, never a literal"),
                }
            }
        }
        out
    }

    #[test]
    fn icon_produces_a_node_with_the_given_key() {
        let node = icon("tick", IconMark::Check);
        assert_eq!(node.key.as_str(), "tick");
    }

    #[test]
    fn icon_is_not_interactive() {
        let node = icon("tick", IconMark::Check);
        assert!(
            !node.is_interactive(),
            "an icon is a visual part, not a control"
        );
        assert!(node.interactions.is_empty());
        assert!(
            node.semantics.role.is_none(),
            "FR-026: the labeled control owns the role; icon() does not take a label"
        );
        assert!(node.semantics.label.is_none());
    }

    #[test]
    fn check_mark_has_a_non_empty_canvas_draw_list() {
        let node = icon("tick", IconMark::Check);
        assert_eq!(node.kind, NodeKind::Canvas);
        assert_eq!(node.constraints.horizontal.min, Some(CHECK_BOX));
        assert_eq!(node.constraints.horizontal.max, Some(CHECK_BOX));
        assert_eq!(node.constraints.vertical.min, Some(CHECK_BOX));
        assert_eq!(node.constraints.vertical.max, Some(CHECK_BOX));
        let list = list(&node);
        assert!(
            !list.is_empty(),
            "a swatch pretending to be a tick is empty of paths"
        );
        assert!(
            list.path_verbs() > 0,
            "Check is a path, not a filled rectangle"
        );
        let mut saw_fill = false;
        for command in list.commands() {
            let Command::Path { paint, verbs, .. } = command else {
                continue;
            };
            assert!(!verbs.is_empty());
            match paint.fill.as_ref() {
                Some(ColorRef::Token(name)) => {
                    assert_eq!(name, TEXT_ON_ACCENT);
                    saw_fill = true;
                }
                other => panic!("Check fill must be token {TEXT_ON_ACCENT}, got {other:?}"),
            }
        }
        assert!(saw_fill, "Check paints with TEXT_ON_ACCENT");
    }

    /// Every mark constructs at both boxes — which is the convexity proof,
    /// since [`DrawList::new`] refuses a non-convex fill at construction —
    /// paints something, and sits in the box Carbon gives it.
    #[test]
    fn every_mark_constructs_at_both_boxes_and_paints_something() {
        for mark in EVERY_MARK {
            for boxed in [IconBox::Glyph, IconBox::Header] {
                let node = icon_in("m", mark, boxed, IconTone::Primary);
                assert_eq!(node.kind, NodeKind::Canvas, "{mark:?}");
                let expected = match (mark, boxed) {
                    (IconMark::Check, _) => CHECK_BOX,
                    (_, IconBox::Glyph) => GLYPH_BOX,
                    (_, IconBox::Header) => HEADER_BOX,
                };
                assert_eq!(node.constraints.horizontal.min, Some(expected), "{mark:?}");
                assert_eq!(node.constraints.horizontal.max, Some(expected), "{mark:?}");
                assert_eq!(node.constraints.vertical.min, Some(expected), "{mark:?}");
                assert_eq!(node.constraints.vertical.max, Some(expected), "{mark:?}");
                let list = list(&node);
                assert!(!list.is_empty(), "{mark:?} at {boxed:?} draws nothing");
                assert!(
                    !colours(list).is_empty(),
                    "{mark:?} at {boxed:?} has no paint"
                );
            }
        }
    }

    /// Every shape of every mark stays inside its box: a glyph that leaks
    /// past its own node is clipped by the parent or overdraws a neighbour.
    #[test]
    fn every_mark_stays_inside_its_box() {
        for mark in EVERY_MARK {
            for boxed in [IconBox::Glyph, IconBox::Header] {
                let node = icon_in("m", mark, boxed, IconTone::Primary);
                let size = mark.extent(boxed);
                let inside = |x: f32, y: f32| {
                    assert!(
                        (-0.001..=size + 0.001).contains(&x)
                            && (-0.001..=size + 0.001).contains(&y),
                        "{mark:?} at {boxed:?}: ({x}, {y}) is outside a {size} box"
                    );
                };
                for command in list(&node).commands() {
                    match command {
                        Command::Rect { rect, .. } => {
                            inside(rect.x, rect.y);
                            inside(rect.right(), rect.bottom());
                        }
                        Command::Ellipse { center, radii, .. } => {
                            inside(center.x - radii.w, center.y - radii.h);
                            inside(center.x + radii.w, center.y + radii.h);
                        }
                        Command::Path { verbs, .. } => {
                            for verb in verbs {
                                for p in verb.points() {
                                    inside(p.x, p.y);
                                }
                            }
                        }
                        other => panic!("{mark:?} uses an unexpected command {other:?}"),
                    }
                }
            }
        }
    }

    /// The tone slot: the default is the accent ink, and each named tone
    /// binds exactly its Carbon token on every shape of every mark.
    #[test]
    fn the_tone_binds_one_token_on_every_shape() {
        for mark in EVERY_MARK {
            let default = icon("m", mark);
            for name in colours(list(&default)) {
                assert_eq!(name, TEXT_ON_ACCENT, "{mark:?} default tone");
            }
            for (tone, token) in [
                (IconTone::OnAccent, TEXT_ON_ACCENT),
                (IconTone::Primary, ICON_PRIMARY),
                (IconTone::Secondary, ICON_SECONDARY),
                (IconTone::Disabled, ICON_DISABLED),
                (IconTone::Accent, ACCENT_PRIMARY),
            ] {
                let node = icon_in("m", mark, IconBox::Glyph, tone);
                let seen = colours(list(&node));
                assert!(!seen.is_empty(), "{mark:?} in {tone:?} paints nothing");
                for name in seen {
                    assert_eq!(name, token, "{mark:?} in {tone:?}");
                }
            }
        }
    }

    /// The marks that are built from bars snap, so a one-unit bar never
    /// straddles a half-pixel at 1x; the marks that are paths do not, since
    /// only `Rect` may (`contracts/draw-list.md` §3).
    #[test]
    fn bar_built_marks_snap_and_nothing_else_does() {
        for mark in EVERY_MARK {
            let node = icon("m", mark);
            for command in list(&node).commands() {
                if let Command::Rect { snap, .. } = command {
                    assert!(*snap, "{mark:?} draws an unsnapped bar");
                }
            }
        }
    }

    /// A glyph centred by its parent lands on a whole pixel: every box is an
    /// even number, so `(row - box) / 2` is whole for every even row height
    /// in the library (24, 32, 40, 48).
    #[test]
    fn every_box_is_even() {
        for size in [CHECK_BOX, GLYPH_BOX, HEADER_BOX] {
            assert_eq!(
                size % 2.0,
                0.0,
                "{size} would put a centred mark on a half-pixel"
            );
        }
    }
}
