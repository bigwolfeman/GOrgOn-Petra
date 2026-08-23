//! `section` — a titled, padded group: the shape every labelled block on a
//! surface has (measured consumer: `gallery.rs`, every section on the
//! page).

use super::text::heading;
use super::tokens::{SHAPE_MD, SPACING_LG, SPACING_MD, SPACING_SM, SURFACE_RAISED, TEXT_MUTED, t};
use super::{pad, stack};
use crate::geom::Axis;
use crate::tree::{Key, ViewNode};

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

    let mut node = stack(key, Axis::Vertical, Some(SPACING_SM), rows);
    node.props.padding = Some(pad(SPACING_LG, SPACING_MD));
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("border".into(), t(TEXT_MUTED));
    node.props.tokens.insert("radius".into(), t(SHAPE_MD));
    node
}
