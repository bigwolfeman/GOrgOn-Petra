//! `section` — a titled, padded group: the shape every labelled block on a
//! surface has (measured consumer: `gallery.rs`, every section on the
//! page).

use super::text::heading;
use super::tokens::{SHADOW_RAISED, SPACING_03, SPACING_04, SPACING_05, SURFACE_RAISED, t};
use super::{pad, stack};
use crate::geom::Axis;
use crate::token::{CornerRole, corner_for};
use crate::tree::{Key, ViewNode};

/// `section` has no declared minimum height: it wraps a heading plus
/// whatever `children` the caller passes, and neither is pinned. This is a
/// conservative floor, not a measured one — a single heading line already
/// clears it — chosen only to stand well above `CornerRole::Grouping`'s
/// 8-unit half-edge threshold so the half-edge clause never fires on a
/// real section.
const HEIGHT_FLOOR: f32 = 40.0;

/// A titled block: a [`heading`] above `children`, inside a padded,
/// rounded, bordered card.
///
/// Carries no role. `crate::tree::Role` has no "group" entry, and a
/// container with no declared actions is outside
/// `ActionableNeedsRoleAndLabel`'s reach, so FR-058 imposes no obligation
/// here — what this component guarantees instead is the padding and corner
/// radius an author would otherwise have to choose by hand for every
/// section on a page. The design build's own note on this: `button`,
/// `field` and `section` cannot look coherent without a corner ramp, and
/// `section` is the one of the three assembled from more than a single
/// token.
pub fn section(key: impl Into<Key>, title: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    let key = key.into();
    let mut rows = vec![heading("title", title)];
    rows.extend(children);

    let mut node = stack(key, Axis::Vertical, Some(SPACING_03), rows);
    node.props.padding = Some(pad(SPACING_05, SPACING_04));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    // Depth, not an outline. This bound `border` to `text.muted` until
    // 2026-08-25. That drew every card on every page in a *text* colour --
    // 10.7:1 against the fill it was separating, as loud as the prose inside
    // it, and carrying nothing the fill was not already carrying. There is no
    // border token in the theme, which is why a text one had been conscripted.
    node.props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
    // FR-022: `section` groups a heading and its children the way a tile or
    // a content switcher groups controls — `CornerRole::Grouping`. This
    // moves the card's corner from `shape.corner-md` (8) to `shape.corner-sm`
    // (4); see `HEIGHT_FLOOR`'s own doc for why the edge passed is a floor,
    // not a measurement.
    node.props.tokens.insert(
        "radius".into(),
        t(corner_for(CornerRole::Grouping, HEIGHT_FLOOR)),
    );
    node
}
