//! Avatar atomic (spec 009 T008). Image or initials, stadium, presence that
//! is never hue alone.
//!
//! Carbon has no Avatar in the 42-row inventory. The anatomy is MynaUI's
//! (`Avatar` / `AvatarImage` / `AvatarFallback`, default `size-8`) plus the
//! presence-dot trap that page never names and this operator cannot ignore:
//! a corner pip that is only green / amber / red does not exist for a
//! red-green colourblind reader. The second channel is the same one
//! [`super::status`] already ships — a [`StatusShape`] silhouette plus the
//! [`StatusToken`]'s text in `Semantics` — not a new glyph and not an
//! [`super::icon::IconMark`]. That enum has no user / presence mark, and
//! inventing one here would put a picture in a file that does not own
//! `icon.rs`.
//!
//! # Sizes
//!
//! Logical units, square. Chat's turn (MynaUI `size-8`) is the default.
//!
//! | constructor | size |
//! |---|---|
//! | [`avatar_xs`] | 24 ([`super::tokens::SIZE_XS`]) |
//! | [`avatar`] | 32 (default) |
//! | [`avatar_md`] | 40 ([`super::tokens::SIZE_MD`]) |
//! | [`avatar_lg`] | 48 ([`super::tokens::SIZE_LG`]) |
//!
//! # Groups
//!
//! [`avatar_group`] is the overlapping row (spec 009 T030). Callers pass
//! already-built avatars; this file does not restyle them. `overflow > 0`
//! appends a trailing disc whose letters are `+N` and whose label is
//! `"N more"`. The count is text, never a hue.
//!
//! A stack cannot take a negative gap: `layout/stack.rs` floors spacing at
//! 0. So the row is a [`NodeKind::Overlay`] of pin seats. Each later disc
//! follows a leading spacer of `i * (size - overlap)` so it paints on
//! top of the one before it. True negative spacing still waits on a
//! stack that accepts it.
//!
//! # Radius
//!
//! FR-022: a disc at every size is [`crate::token::CornerRole::Pill`],
//! reached only through [`crate::token::corner_for`]. A fixed role would
//! also land on `shape.corner-full` at 24 (`Floating`'s 8 clears 24 / 2)
//! and would quietly stop the day a size grew past 16. No radius literal.
//!
//! # No border
//!
//! The stadium fill is [`SURFACE_RAISED`]. A four-sided edge is the
//! container-border pattern the contract retired, and the painter cannot
//! round a border anyway. The group does not grow a ring: the fill step
//! is what separates stacked discs from the page.
//!
//! # Initials fallback
//!
//! [`avatar`] draws the letters. [`avatar_with_image`] still builds that
//! face and lays the [`NodeKind::Image`] over it, so a source the painter
//! cannot resolve leaves the letters showing rather than a silent hole.
//! The letters are what the caller passed, not a name this file derives.

use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    SILHOUETTE_DIAMOND, SILHOUETTE_OCTAGON, SILHOUETTE_RECT, SILHOUETTE_TRIANGLE, SIZE_LG, SIZE_MD,
    SIZE_XS, SURFACE_RAISED, TEXT_PRIMARY, TYPOGRAPHY_BODY_COMPACT, TYPOGRAPHY_LABEL, t,
};
use crate::geom::{Align, Axis};
use crate::token::{CornerRole, StatusShape, StatusToken, corner_for};
use crate::tree::{AxisConstraint, Constraints, Justify, Key, NodeKind, Props, Role, ViewNode};

/// Chat's `size-8`. Default.
const SIZE_DEFAULT: f32 = 32.0;
/// Presence mark. Smaller than [`super::status`]'s 10-unit readout dot so
/// it seats on [`avatar_xs`] without eating the face.
const PRESENCE: f32 = 8.0;
/// How far each later disc covers the one before it. Stack spacing
/// floors at 0, so this is a pin offset, not a negative gap.
///
/// 12 of 32 is enough that the row reads as a stack, not a tight
/// spaced list. 8 was a hairline overlap that photographed as a gap.
const OVERLAP: f32 = 12.0;

const _: () = assert!(SIZE_DEFAULT == 32.0);
const _: () = assert!(SIZE_XS == 24.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);
const _: () = assert!(PRESENCE == 8.0);
const _: () = assert!(OVERLAP == 12.0);
const _: () = assert!(OVERLAP < SIZE_XS);

/// The paint slot naming a node's outline family. Same string
/// [`super::status`] binds; a slot key is a plain string by design.
const SILHOUETTE_SLOT: &str = "silhouette";

/// Initials-only avatar at 32. Not interactive.
#[must_use]
pub fn avatar(key: impl Into<Key>, initials: impl Into<String>) -> ViewNode {
    build(key, initials, SIZE_DEFAULT, None, None)
}

/// 24.
#[must_use]
pub fn avatar_xs(key: impl Into<Key>, initials: impl Into<String>) -> ViewNode {
    build(key, initials, SIZE_XS, None, None)
}

/// 40.
#[must_use]
pub fn avatar_md(key: impl Into<Key>, initials: impl Into<String>) -> ViewNode {
    build(key, initials, SIZE_MD, None, None)
}

/// 48.
#[must_use]
pub fn avatar_lg(key: impl Into<Key>, initials: impl Into<String>) -> ViewNode {
    build(key, initials, SIZE_LG, None, None)
}

/// Image over the initials fallback, at the default size.
///
/// `src` is the [`Props::image`] source string the painter's registry
/// resolves. The letters stay in the tree underneath: an unresolved source
/// paints nothing, and the fallback is what remains.
#[must_use]
pub fn avatar_with_image(
    key: impl Into<Key>,
    initials: impl Into<String>,
    src: impl Into<String>,
) -> ViewNode {
    build(key, initials, SIZE_DEFAULT, Some(src.into()), None)
}

/// Initials plus a presence mark. The mark is not hue alone: it binds the
/// same silhouette [`super::status`] would, and the token's text is the
/// [`Role::Status`] label.
#[must_use]
pub fn avatar_with_status(
    key: impl Into<Key>,
    initials: impl Into<String>,
    status: &StatusToken,
) -> ViewNode {
    build(key, initials, SIZE_DEFAULT, None, Some(status))
}

/// Default-size avatar with optional image and optional presence.
///
/// Presence, when present, is a [`StatusToken`] so a colour-only pip cannot
/// reach this function. Composing [`super::status`] itself is the wrong
/// shape: that constructor is a labelled row (dot plus visible text) and
/// would grow the overlay past the face. The badge here is the marker half
/// of that pairing, with the text in `Semantics`.
#[must_use]
pub fn avatar_with(
    key: impl Into<Key>,
    initials: impl Into<String>,
    image: Option<&str>,
    status: Option<&StatusToken>,
) -> ViewNode {
    build(
        key,
        initials,
        SIZE_DEFAULT,
        image.map(str::to_owned),
        status,
    )
}

/// Horizontal overlapping stack of already-built avatars.
///
/// `avatars` are `avatar()` / `avatar_xs()` / etc. nodes. This constructor
/// does not inspect them and does not restyle them. `overflow > 0` appends
/// a trailing disc the same size as the first face (or 32 when the row
/// is empty), with visible letters `+N` and [`Role::Image`] label
/// `"N more"`. The count is text, not colour.
///
/// Stack spacing floors at 0 (`layout/stack.rs`), so overlap cannot be a
/// negative gap. Each disc is a pin seat inside an overlay: a leading
/// spacer of `i * (size - overlap)` plus the face, later children on top.
/// The seat is pinned to that width. An overlay offers every child the
/// group's full rect; without the pin, a horizontal stack spends the extra
/// width as a gap and the overlap disappears.
/// No four-sided ring. The stadium fill is the step that separates discs;
/// the painter cannot round a border.
#[must_use]
pub fn avatar_group(key: impl Into<Key>, avatars: Vec<ViewNode>, overflow: u32) -> ViewNode {
    let size = avatars.first().map(face_size).unwrap_or(SIZE_DEFAULT);
    let mut faces = avatars;
    if overflow > 0 {
        faces.push(overflow_pill(overflow, size));
    }
    let n = faces.len();
    let step = (size - OVERLAP).max(0.0);
    let children = if n > 1 {
        faces
            .into_iter()
            .enumerate()
            .map(|(i, face)| pin_at(i, i as f32 * step, face))
            .collect()
    } else {
        faces
    };
    ViewNode::new(NodeKind::Overlay, key).with_children(children)
}

fn overflow_pill(n: u32, size: f32) -> ViewNode {
    let mut node = build("overflow", format!("+{n}"), size, None, None);
    node.semantics.label = Some(format!("{n} more"));
    node
}

fn pin_at(index: usize, offset: f32, child: ViewNode) -> ViewNode {
    let face = face_size(&child);
    let mut parts = Vec::with_capacity(2);
    if offset > 0.0 {
        parts.push(lead_spacer(offset));
    }
    parts.push(child);
    let mut seat = stack(format!("pin-{index}"), Axis::Horizontal, None, parts);
    seat.props.align = Some(Align::Center);
    seat.props.justify = Some(Justify::Start);
    // Pin the seat to spacer + face. Overlay places every child in the
    // group's full rect; a free horizontal stack would spend the extra
    // width between the discs and the overlap would vanish.
    let width = offset + face;
    seat.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(width),
            max: Some(width),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(face),
            max: Some(face),
            priority: 0,
        },
    })
}

fn lead_spacer(offset: f32) -> ViewNode {
    ViewNode::new(NodeKind::Spacer, "lead").with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(offset),
            max: Some(offset),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(0.0),
            max: Some(0.0),
            priority: 0,
        },
    })
}

fn face_size(node: &ViewNode) -> f32 {
    node.constraints.horizontal.min.unwrap_or(SIZE_DEFAULT)
}

fn build(
    key: impl Into<Key>,
    initials: impl Into<String>,
    size: f32,
    image: Option<String>,
    status: Option<&StatusToken>,
) -> ViewNode {
    let key = key.into();
    let initials = initials.into();
    if image.is_none() && status.is_none() {
        return labelled_image(face(key, &initials, size), &initials, None);
    }

    let mut children = vec![face(Key::new("face"), &initials, size)];
    if let Some(src) = image {
        children.push(photo("photo", src, size, &initials));
    }
    if let Some(token) = status {
        children.push(presence_seat(token));
    }
    let mut node = ViewNode::new(NodeKind::Overlay, key)
        .with_children(children)
        .with_constraints(square(size));
    node.props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Pill, size)));
    labelled_image(node, &initials, status)
}

fn face(key: impl Into<Key>, initials: &str, size: f32) -> ViewNode {
    let mut letters = text("initials", initials.to_owned());
    letters.props.style = Some(t(type_for(size)));
    letters
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![letters]);
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // FR-022: the face is a disc at every shipped size. `CornerRole::Pill`
    // says that; a fixed role would also land on `shape.corner-full` at 24
    // and 32 and would stop the day a size grew past 16.
    node.props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Pill, size)));
    node.with_constraints(square(size))
}

fn photo(key: impl Into<Key>, src: String, size: f32, initials: &str) -> ViewNode {
    let mut node = ViewNode::new(NodeKind::Image, key).with_props(Props {
        image: Some(src),
        ..Props::default()
    });
    node.props
        .tokens
        .insert("radius".into(), t(corner_for(CornerRole::Pill, size)));
    node.semantics.label = Some(accessible_name(initials));
    node.with_constraints(square(size))
}

/// Presence pip in the trailing-end corner of the overlay.
///
/// Overlay hands every child the face's rect (`layout/overlay.rs`), so this
/// stack is a seat: `Justify::End` + `Align::End` parks the mark, the same
/// way the left-panel accent sits on its row. The mark is a
/// colour-plus-silhouette swatch; the token's text is the [`Role::Status`]
/// label, never a visible caption that would grow the avatar.
fn presence_seat(status: &StatusToken) -> ViewNode {
    let (radius, silhouette) = marker_for(status.shape());
    let mut dot = swatch(
        "dot",
        PRESENCE,
        PRESENCE,
        Some(status.name().as_str()),
        None,
        radius,
    );
    dot.props
        .tokens
        .insert(SILHOUETTE_SLOT.into(), t(silhouette));
    dot.semantics.role = Some(Role::Status);
    dot.semantics.label = Some(status.text().to_owned());
    let mut seat = stack("presence", Axis::Vertical, None, vec![dot]);
    seat.props.align = Some(Align::End);
    seat.props.justify = Some(Justify::End);
    seat
}

fn labelled_image(mut node: ViewNode, initials: &str, status: Option<&StatusToken>) -> ViewNode {
    let mut label = accessible_name(initials);
    if let Some(token) = status {
        label = format!("{label}, {}", token.text());
    }
    node.semantics.role = Some(Role::Image);
    node.semantics.label = Some(label);
    node
}

fn accessible_name(initials: &str) -> String {
    let trimmed = initials.trim();
    if trimmed.is_empty() {
        "avatar".into()
    } else {
        trimmed.to_owned()
    }
}

/// The `(radius, silhouette)` pair one [`StatusShape`] paints as.
///
/// Copied from [`super::status`]'s `marker_for`: that helper is private and
/// this file does not own it. Total over the enum, no catch-all, so a fifth
/// variant will not compile until somebody decides what figure it draws.
fn marker_for(shape: StatusShape) -> (Option<&'static str>, &'static str) {
    match shape {
        StatusShape::Circle => (
            Some(corner_for(CornerRole::Pill, PRESENCE)),
            SILHOUETTE_RECT,
        ),
        StatusShape::Square => (
            Some(corner_for(CornerRole::Tiled, PRESENCE)),
            SILHOUETTE_RECT,
        ),
        StatusShape::Triangle => (None, SILHOUETTE_TRIANGLE),
        StatusShape::Diamond => (None, SILHOUETTE_DIAMOND),
        StatusShape::Octagon => (None, SILHOUETTE_OCTAGON),
    }
}

fn type_for(size: f32) -> &'static str {
    if size >= SIZE_MD {
        TYPOGRAPHY_BODY_COMPACT
    } else {
        TYPOGRAPHY_LABEL
    }
}

fn square(size: f32) -> Constraints {
    Constraints {
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
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PRESENCE, SIZE_DEFAULT, SIZE_LG, SIZE_MD, SIZE_XS, avatar, avatar_group, avatar_lg,
        avatar_md, avatar_with, avatar_with_image, avatar_with_status, avatar_xs,
    };
    use crate::component::tokens::{
        SILHOUETTE_DIAMOND, SILHOUETTE_OCTAGON, SILHOUETTE_RECT, SILHOUETTE_TRIANGLE,
        SURFACE_RAISED,
    };
    use crate::token::{CornerRole, StatusShape, StatusToken, TokenName, corner_for};
    use crate::tree::{NodeKind, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn descendant<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        descendant_opt(node, key).unwrap_or_else(|| panic!("missing descendant {key}"))
    }

    fn descendant_opt<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
        if node.key.as_str() == key {
            return Some(node);
        }
        node.children.iter().find_map(|c| descendant_opt(c, key))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn ok_status() -> StatusToken {
        StatusToken::new(
            TokenName::new("status.ok").unwrap(),
            StatusShape::Circle,
            "online",
        )
        .unwrap()
    }

    #[test]
    fn avatar_is_a_32_stadium_with_initials_and_no_border() {
        let node = avatar("me", "JD");
        assert_eq!(node.constraints.horizontal.min, Some(SIZE_DEFAULT));
        assert_eq!(node.constraints.horizontal.max, Some(SIZE_DEFAULT));
        assert_eq!(node.constraints.vertical.min, Some(SIZE_DEFAULT));
        assert_eq!(SIZE_DEFAULT, 32.0);
        assert_eq!(
            token(&node, "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_DEFAULT))
        );
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(&node, "border"),
            None,
            "the atomic carries no four-sided edge"
        );
        assert_eq!(child(&node, "initials").props.text.as_deref(), Some("JD"));
        assert_eq!(node.semantics.role, Some(Role::Image));
        assert_eq!(node.semantics.label.as_deref(), Some("JD"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
    }

    #[test]
    fn size_ramp_is_24_32_40_48() {
        assert_eq!(
            avatar_xs("me", "JD").constraints.horizontal.min,
            Some(SIZE_XS)
        );
        assert_eq!(SIZE_XS, 24.0);
        assert_eq!(
            token(&avatar_xs("me", "JD"), "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_XS))
        );
        assert_eq!(avatar("me", "JD").constraints.horizontal.min, Some(32.0));
        assert_eq!(
            avatar_md("me", "JD").constraints.horizontal.min,
            Some(SIZE_MD)
        );
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(
            avatar_lg("me", "JD").constraints.horizontal.min,
            Some(SIZE_LG)
        );
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(
            token(&avatar_lg("me", "JD"), "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_LG))
        );
    }

    #[test]
    fn image_sits_on_the_initials_fallback() {
        let node = avatar_with_image("me", "JD", "faces/jd.png");
        assert_eq!(node.kind, NodeKind::Overlay);
        assert_eq!(
            child(&node, "photo").props.image.as_deref(),
            Some("faces/jd.png")
        );
        assert_eq!(child(&node, "photo").kind, NodeKind::Image);
        assert_eq!(
            descendant(&node, "initials").props.text.as_deref(),
            Some("JD"),
            "initials stay in the tree so an unresolved source still has a face"
        );
        assert_eq!(
            token(child(&node, "face"), "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_DEFAULT))
        );
        assert_eq!(
            token(child(&node, "photo"), "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_DEFAULT))
        );
        assert_eq!(token(&node, "border"), None);
        assert_eq!(
            token(&node, "radius"),
            Some(corner_for(CornerRole::Pill, SIZE_DEFAULT))
        );
        assert_eq!(node.semantics.role, Some(Role::Image));
        assert_eq!(child(&node, "photo").semantics.label.as_deref(), Some("JD"));
    }

    #[test]
    fn presence_is_silhouette_plus_text_never_hue_alone() {
        let node = avatar_with_status("me", "JD", &ok_status());
        let dot = descendant(&node, "dot");
        assert_eq!(dot.semantics.role, Some(Role::Status));
        assert_eq!(
            dot.semantics.label.as_deref(),
            Some("online"),
            "FR-015 text channel lives on the mark; a colour pip has no label"
        );
        assert_eq!(
            token(dot, "silhouette"),
            Some(SILHOUETTE_RECT),
            "the second visual channel is the figure, not the hue"
        );
        assert_eq!(
            token(dot, "radius"),
            Some(corner_for(CornerRole::Pill, PRESENCE))
        );
        assert_eq!(token(dot, "background"), Some("status.ok"));
        assert_eq!(token(dot, "border"), None);
        assert_eq!(node.semantics.label.as_deref(), Some("JD, online"));
        assert_eq!(node.semantics.role, Some(Role::Image));
    }

    #[test]
    fn presence_shapes_are_five_different_figures() {
        let cases = [
            (
                StatusShape::Circle,
                Some(corner_for(CornerRole::Pill, PRESENCE)),
                SILHOUETTE_RECT,
            ),
            (
                StatusShape::Square,
                Some(corner_for(CornerRole::Tiled, PRESENCE)),
                SILHOUETTE_RECT,
            ),
            (StatusShape::Triangle, None, SILHOUETTE_TRIANGLE),
            (StatusShape::Diamond, None, SILHOUETTE_DIAMOND),
            (StatusShape::Octagon, None, SILHOUETTE_OCTAGON),
        ];
        for (shape, radius, silhouette) in cases {
            let status =
                StatusToken::new(TokenName::new("status.ok").unwrap(), shape, "here").unwrap();
            let node = avatar_with_status("me", "JD", &status);
            let dot = descendant(&node, "dot");
            assert_eq!(token(dot, "silhouette"), Some(silhouette), "{shape:?}");
            assert_eq!(token(dot, "radius"), radius, "{shape:?}");
        }
    }

    #[test]
    fn avatar_with_composes_image_and_status() {
        let node = avatar_with("me", "JD", Some("faces/jd.png"), Some(&ok_status()));
        assert_eq!(node.kind, NodeKind::Overlay);
        assert!(child(&node, "photo").props.image.is_some());
        assert_eq!(descendant(&node, "dot").semantics.role, Some(Role::Status));
        assert_eq!(
            descendant(&node, "initials").props.text.as_deref(),
            Some("JD")
        );
    }

    #[test]
    fn empty_initials_still_name_the_image() {
        let node = avatar("me", "  ");
        assert_eq!(node.semantics.label.as_deref(), Some("avatar"));
        assert_eq!(node.semantics.role, Some(Role::Image));
    }

    #[test]
    fn group_child_count_is_faces_plus_overflow_pill() {
        let two = avatar_group("who", vec![avatar("a", "A"), avatar("b", "B")], 3);
        assert_eq!(two.children.len(), 3, "two faces and the +3 pill");
        assert_eq!(two.kind, NodeKind::Overlay);
        assert_eq!(token(&two, "border"), None);

        let none = avatar_group("who", vec![avatar("a", "A"), avatar("b", "B")], 0);
        assert_eq!(none.children.len(), 2, "overflow 0 adds no pill");
        assert!(
            descendant_opt(&none, "overflow").is_none(),
            "no overflow node when the count is 0"
        );

        let only_n = avatar_group("who", vec![], 4);
        assert_eq!(only_n.children.len(), 1);
    }

    #[test]
    fn overflow_pill_is_plus_n_text_and_n_more_label() {
        let node = avatar_group("who", vec![avatar("a", "A"), avatar("b", "B")], 3);
        let pill = descendant(&node, "overflow");
        assert_eq!(
            descendant(pill, "initials").props.text.as_deref(),
            Some("+3"),
            "the count is letters on the disc, not a hue"
        );
        assert_eq!(pill.semantics.label.as_deref(), Some("3 more"));
        assert_eq!(pill.semantics.role, Some(Role::Image));
        assert_eq!(token(pill, "border"), None);
        assert_eq!(token(pill, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            pill.constraints.horizontal.min,
            Some(SIZE_DEFAULT),
            "the pill is avatar-sized"
        );
    }

    #[test]
    fn group_pin_seats_are_capped_to_offset_plus_face() {
        let node = avatar_group("who", vec![avatar("a", "A"), avatar("b", "B")], 0);
        let step = SIZE_DEFAULT - super::OVERLAP;
        let pin1 = child(&node, "pin-1");
        assert_eq!(
            pin1.constraints.horizontal.max,
            Some(step + SIZE_DEFAULT),
            "overlay offers the full group width; the seat must not spend it as a gap"
        );
        assert_eq!(pin1.props.justify, Some(crate::tree::Justify::Start));
    }
}
