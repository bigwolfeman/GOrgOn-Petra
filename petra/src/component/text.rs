//! `text` and `heading` — the two non-interactive type components.

use std::sync::Arc;

use super::tokens::{
    TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY, TYPOGRAPHY_HEADING, TYPOGRAPHY_HEADING_SM, t,
};
use crate::tree::{Key, NodeKind, Props, TextWrap, ViewNode};

/// The disabled ink every text leaf in this crate binds, on one home.
///
/// Carbon's `$text-disabled` is `$text-primary` at 25% alpha, which measures
/// **1.31:1** against a light-theme layer — nowhere near the 4.5:1 a label
/// owes (spec 008 FR-023, closing spec 005 FR-010's named hole: no gate ever
/// measured label contrast *through* the disabled treatment). The treatment
/// was therefore fixed at the ink rather than the threshold: a disabled label
/// is drawn at [`TEXT_MUTED`], which measures **5.70:1 (light) / 5.26:1
/// (dark)** against `button-disabled` and 9.74:1 / 12.64:1 against ground.
/// The measurements and the per-label verdicts live in
/// `specs/008-petra-carbon-variants/contracts/disabled.md`.
///
/// Bound on every text leaf here rather than per component, because the rule
/// is "a disabled label is legible", not "this label is legible" — the
/// alternative is forty-two copies of the same binding waiting to disagree.
/// Non-text marks do **not** take this binding: they keep Carbon's 25% fades
/// (`icon-disabled`, `icon-on-color-disabled`), which are decoration, not
/// reading matter
/// ([`super::icon`]).
/// The disabled partner every label-ink binding takes (SC-014's fix,
/// spec 008 FR-023): `foreground@disabled → text.muted`.
///
/// `pub(crate)` because the rule is bigger than [`text`]: an [`Input`]'s
/// value ink ([`super::field::valued`], `number_input`'s well, `search`'s)
/// and every other resting `foreground` binding needs the same partner, and
/// each duplicating the pair by hand is each a chance to drift from the
/// measured value.
pub(crate) fn disabled_label_ink(props: &mut Props) {
    props
        .tokens
        .insert("foreground@disabled".into(), t(TEXT_MUTED));
}

/// A run of body text.
///
/// Carries no role and no interactions: `crate::layout::default_role` gives
/// a plain text node none, and a node with no actions is outside
/// `ActionableNeedsRoleAndLabel`'s reach, so FR-058 has nothing to say about
/// it — the obligation is about what an *interactive* component cannot
/// omit, and `text` is not one.
pub fn text(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    let mut props = Props {
        text: Some(content.into()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    disabled_label_ink(&mut props);
    ViewNode::new(NodeKind::Text, key).with_props(props)
}

/// A section or page title.
///
/// `text`'s sibling in every way but one: the typography step it binds is
/// [`TYPOGRAPHY_HEADING`] instead of [`TYPOGRAPHY_BODY`]. Kept as two
/// functions rather than one with a `size` argument, so an author reaching
/// for "the big text" and "the normal text" finds two names rather than a
/// style value to pick — which is the whole point of a component library
/// over the primitives (FR-052).
pub fn heading(key: impl Into<Key>, content: impl Into<String>) -> ViewNode {
    let mut props = Props {
        text: Some(content.into()),
        style: Some(t(TYPOGRAPHY_HEADING)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    disabled_label_ink(&mut props);
    ViewNode::new(NodeKind::Text, key).with_props(props)
}

/// Every text leaf under `node` restyled as `heading-compact-01`
/// ([`TYPOGRAPHY_HEADING_SM`]) in [`TEXT_PRIMARY`]: the type a table's
/// column header is set in (data table, slice-b:75; structured list,
/// slice-e anatomy 2). Shared by both tables so a caller can hand either
/// a bare [`text`] as a header cell and get the header's type without
/// knowing the token.
pub(crate) fn as_compact_heading(mut node: ViewNode) -> ViewNode {
    fn walk(node: &mut ViewNode) {
        if node.kind == NodeKind::Text {
            node.props.style = Some(t(TYPOGRAPHY_HEADING_SM));
            node.props
                .tokens
                .insert("foreground".into(), t(TEXT_PRIMARY));
            disabled_label_ink(&mut node.props);
            node.props.wrap = Some(TextWrap::Ellipsis);
        }
        for child in &mut node.children {
            walk(Arc::make_mut(child));
        }
    }
    walk(&mut node);
    node
}

/// Single-line truncation on every text leaf. A table cell that Wraps at
/// a dragged-narrow column stacks one glyph per line; Ellipsis keeps the
/// row's height.
pub(crate) fn ellipsis_text(node: &mut ViewNode) {
    if node.kind == NodeKind::Text {
        node.props.wrap = Some(TextWrap::Ellipsis);
    }
    for child in &mut node.children {
        ellipsis_text(Arc::make_mut(child));
    }
}
