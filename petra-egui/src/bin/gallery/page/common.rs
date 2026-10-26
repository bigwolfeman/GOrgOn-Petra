//! What several catalog pages share.
//!
//! The token references, the layout stacks a page body is built from, and
//! the path test a handler matches its node ids with. Pages import these
//! rather than copying them, so the drift the per-page split pulls apart
//! cannot come back a different way.

use gorgon_petra::component::text;
use gorgon_petra::geom::{Align, Axis};
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    AxisConstraint, Constraints, NodeKind, Props, TextWrap, TrackSize, ViewNode,
};

/// A spacing token reference, for the styling props that take one (FR-053).
pub fn sp(name: &str) -> Option<TokenName> {
    Some(TokenName::new(name).expect("catalog spacing tokens are well-formed"))
}

/// A colour or type token reference, for `props.tokens` values.
pub fn tok(name: &str) -> TokenName {
    TokenName::new(name).expect("catalog style tokens are well-formed")
}

/// Whether any segment of the routed node path `node` is exactly `key`.
///
/// A press on a child (knob, label, state text) still names that child in
/// the route, so a handler matches its own id anywhere in the path rather
/// than only as the last segment.
pub fn path_has(node: &str, key: &str) -> bool {
    node.split('/').any(|part| part == key)
}

/// A column divider (`div0`, `div1`, …) or a row-reorder grip. A press on
/// either sits inside a row's path, so `path_has(node, row_id)` would treat
/// a resize as a row click if the handler did not skip these first.
pub fn path_is_column_chrome(node: &str) -> bool {
    node.split('/').any(|part| {
        part == "grip"
            || part.strip_prefix("div").is_some_and(|digits| {
                !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
            })
    })
}

/// Whether any of the dismissed surface ids `ids` is the surface keyed
/// `key` — the test a page runs in [`super::Page::dismissed`].
///
/// By the id's **last** segment. A placement id ends at the node it
/// names, so the last segment *is* the surface and everything before it
/// is where the surface lives — and "where it lives" must not be matched:
/// the Menu page mounts its surface inside a section keyed `menu`, so any
/// segment match read every dismissal on the page (a press that shut one
/// of the variant menus mounted open at rest) as a dismissal of the live
/// menu and closed it the same event that opened it.
pub fn dismisses(ids: &[String], key: &str) -> bool {
    ids.iter().any(|id| id.rsplit('/').next() == Some(key))
}

/// Whether any dismissed id names the surface keyed `key` inside the
/// control keyed `owner` — [`dismisses`] plus the owning path segment.
///
/// The owner, because a page can mount more than one surface keyed
/// `key` and `key` alone cannot say which one a press shut: two date
/// pickers whose insides carry identical keys, three menus on the Menu
/// buttons page (live, combo, overflow), a column menu beside every open
/// row's menu on the data table. `owner` is a segment of the canonical id
/// that only the owning control's subtree carries (`"compact"`, `"full"`,
/// `"mb-live"`, `"mn-pair"`).
///
/// The **last** segment for `key`, by [`dismisses`]'s rule — which this
/// one needs for a second, subtler reason: a surface may be nested inside
/// another. The month chooser's canonical id is
/// `.../full/calendar/content/chooser`, which carries `calendar` as a
/// segment, so any-segment matching read a dismissed chooser as a
/// dismissed calendar and shut the whole picker every second press of the
/// month button.
pub fn dismissed_under(ids: &[String], owner: &str, key: &str) -> bool {
    ids.iter()
        .any(|id| path_has(id, owner) && id.rsplit('/').next() == Some(key))
}

/// A vertical run of blocks, each as tall as it needs to be.
///
/// A single-column `Grid`, not a `Stack`. A vertical `Stack` placed at
/// an exact height divides that height among its children by equal
/// share; a `Grid`'s implicit rows are `FitContent`. See
/// `examples/gallery.rs` `Gallery::column`.
pub fn column(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Grid, key)
        .with_props(Props {
            columns: vec![TrackSize::Weight { weight: 1.0 }],
            row_spacing: spacing,
            ..Props::default()
        })
        .with_children(children)
}

/// A horizontal run of blocks, centred on the cross axis.
pub fn row(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            spacing,
            align: Some(Align::Center),
            ..Props::default()
        })
        .with_children(children)
}

/// The body of a `section`: a column that outranks the section's title
/// when the section divides its height.
pub fn body(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    column(key, spacing, children).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: None,
            max: None,
            priority: 1,
        },
        ..Constraints::default()
    })
}

/// [`body`], with its children filled to the column's width.
///
/// A Carbon text field fills its container; ours hugged its own placeholder,
/// so row 34 showed five fields at five different widths and each one changed
/// width as the operator typed. `body`'s `Grid` places its cells by one
/// container-wide `align`, and the default leaves them at their natural size.
///
/// A separate helper rather than a change to [`body`]: every one of the 42
/// pages calls `body`, and stretching all of them is a decision about 42
/// pictures rather than about text fields. Pages that want it ask for it.
pub fn filled_body(key: &str, spacing: Option<TokenName>, children: Vec<ViewNode>) -> ViewNode {
    let mut node = body(key, spacing, children);
    node.props.align = Some(Align::Stretch);
    node
}

/// A text leaf that wraps at its width instead of overflowing it.
pub fn wrapped(key: &str, content: impl Into<String>) -> ViewNode {
    let mut node = text(key, content);
    node.props.wrap = Some(TextWrap::Wrap);
    node
}

/// The first node whose key is `key`, depth first. For page and chrome tests
/// that read a control's semantics back out of a built tree.
#[cfg(test)]
pub fn find<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
    if node.key.as_str() == key {
        return Some(node);
    }
    node.children.iter().find_map(|child| find(child, key))
}

#[cfg(test)]
mod tests {
    use super::dismissed_under;
    use super::dismisses;

    /// Every id below has the shape the placement actually reports: a
    /// placement id ends at the surface's own key (`host.rs` asserts
    /// `dismissed[0].ends_with("/menu")`) and carries every key the
    /// surface hangs under before it. The prefixes are the gallery pages'
    /// real trees: the Menu page (`section("menu")` > `body("mn")` > …)
    /// mounts its live surface under the `mn-pair` control and its
    /// variant surfaces under `mn-variants`; the Menu buttons page
    /// (`section("mb")` > `body("mb-body")` > …) mounts its live surface
    /// under `mb-live` and its combo under `mb-variants`.
    const SECTION_HOST: &str = "/page/menu/mn/mn-variants/v-border/v-menu-border";
    const PAIR_SURFACE: &str = "/page/menu/mn/mn-pair/menu";
    const LIVE_SURFACE: &str = "/page/mb/mb-body/mb-live/mb/menu";
    const COMBO_SURFACE: &str = "/page/mb/mb-body/mb-variants/v-combo/v-combo-button/menu";

    /// The claim `dismisses` makes: the surface keyed `key` is the
    /// **last** segment of its id, never a middle one. Every id on the
    /// Menu page traverses its section, which is itself keyed `menu`:
    /// matching by any segment read every press that shut a variant menu
    /// mounted open at rest as a dismissal of the live menu and closed it
    /// the same event that opened it.
    #[test]
    fn a_surface_is_its_last_segment_and_a_host_section_is_never_a_surface() {
        let ids = [SECTION_HOST.to_string()];
        assert!(
            !dismisses(&ids, "menu"),
            "an id that merely traverses the `menu` section is not the `menu` surface: {SECTION_HOST}"
        );
        let ids = [PAIR_SURFACE.to_string()];
        assert!(dismisses(&ids, "menu"), "the surface id ends in `menu`");
    }

    /// The claim `dismissed_under` makes: two surfaces may share the key
    /// `menu` and only the owner segment tells them apart. The Menu
    /// buttons page mounts three (live, combo, overflow); a press that
    /// shut the combo must not read as a press that shut the live one.
    #[test]
    fn a_press_that_shut_another_controls_surface_is_not_this_controls() {
        let ids = [COMBO_SURFACE.to_string()];
        assert!(!dismissed_under(&ids, "mb-live", "menu"));
        // ... while the key alone cannot tell them apart:
        assert!(dismisses(&ids, "menu"), "key alone matches the combo id");
        let ids = [LIVE_SURFACE.to_string()];
        assert!(dismissed_under(&ids, "mb-live", "menu"));
    }
}
