//! `button` — the one activatable label component.

use super::pad;
use super::tokens::{
    ACCENT_PRIMARY, SHADOW_RAISED, SHAPE_MD, SPACING_03, SPACING_04, SURFACE_RAISED,
    TEXT_ON_ACCENT, TEXT_PRIMARY, TYPOGRAPHY_BODY, t,
};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, NodeKind, Props, Role, ViewNode};

/// A focusable, clickable control with a label.
///
/// `label` is a required positional parameter, not an `Option<String>` and
/// not a setter a caller could skip calling — there is no `button(key)`
/// that compiles. The role and interactions are set inside this function,
/// not left to [`crate::tree::ViewNode::interactive`] being called
/// separately, so an author cannot construct a `button` node and forget the
/// step that makes it accessible: FR-058 is a fact about this function's
/// signature, not a convention its caller is trusted to follow.
///
/// A `button` is a padded, rounded `Stack` around a `text` label rather
/// than a bare styled `Text` leaf, because `Props.padding` only applies to
/// container kinds (`crate::tree::validate::Violation::PaddingOnLeafKind`)
/// — a `Text` node has no children to inset. The label lives in the child;
/// the role, the interactions, and the chrome live on the wrapper.
pub fn button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, SURFACE_RAISED, TEXT_PRIMARY)
}

/// The page's one loudest action, filled with the accent instead of a grey.
///
/// # Why this exists rather than a `border` on [`button`]
///
/// A default button is a tonal fill one layer ahead of its ground (see
/// [`super::on_layer`]), which separates it from the card without shouting.
/// That is right for Save-and-Cancel-and-six-others and wrong for the one
/// control a reader should find first: on a page where every control is a
/// grey step, nothing is primary. The previous answer was to give the loud
/// control a `text.muted` outline, which made it the loudest thing on the
/// page in the least useful way — an edge carries no meaning, it just
/// vibrates.
///
/// The accent carries meaning. `accent.primary` and `text.on-accent` have
/// been in the shipped themes since 2026-08-25 with **nothing painting
/// them**, which is precisely the defect `crate::token::shipped`'s own
/// comments count ("a declared name nothing reads"). This is the binding
/// that closes it.
///
/// # Two constraints a caller has to respect
///
/// - **One per view.** An accent that appears twice is not an accent. The
///   library cannot enforce this — it sees one node at a time — so it is
///   stated here and left to the page.
/// - **Not on the deepest dark layer.** `crate::token::shipped`'s
///   `DEEPEST_ACCENT_LAYER` pins dark's accent as legible down to
///   `surface.layer-two` and **not** on `surface.layer-three`, where
///   `#4589ff` measures 2.91:1 and misses the 3:1 SC 1.4.11 fill floor.
///   [`super::on_layer`] never rewrites an accent fill, so a primary button
///   placed that deep keeps a colour its ground cannot carry.
///   [`super::MAX_LAYER_DEPTH`] is why that seat is hard to reach by
///   accident rather than why it is impossible.
///
/// The label is `text.on-accent`, not `text.primary`. Neither shipped text
/// tone clears AA on either accent — the four measurements are in
/// `ON_ACCENT_TOKEN`'s own doc comment — so this is not a stylistic choice
/// and a caller must not "simplify" it back.
pub fn primary_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, ACCENT_PRIMARY, TEXT_ON_ACCENT)
}

/// The shape both entries above share: a padded, rounded, **elevated**
/// `Stack`, around a `Text` child carrying the label.
///
/// One body rather than two because the only difference between a default
/// and a primary button is its two colours. Written twice they would drift,
/// and the thing that would drift first is the role and interaction block
/// below — the one part FR-058 says a caller must never be able to skip.
///
/// A `Chrome` enum briefly lived here to tell the two apart. It went when the
/// border did, because both arms then produced the same node, and two
/// variants with identical bodies is a switch that decides nothing.
///
/// # A button sits *on* the surface, and depth is what says so
///
/// Every button casts [`SHADOW_RAISED`] and none of them draws an outline.
/// The hierarchy between them is carried by the **fill**: the accent measures
/// 4.75:1 (dark) and 4.47:1 (light) against the card, so the primary is the
/// loudest thing in the row whatever shadow either casts. Giving the primary
/// a deeper shadow *as well* was considered and refused — two elevations on
/// one row of buttons reads as fog, and the accent is already unambiguous.
///
/// ## The trade this makes, stated rather than buried
///
/// A button was outlined in `border.subtle` for part of 2026-08-25. That
/// outline was the only thing here reaching WCAG 2.1 SC 1.4.11's 3:1 for the
/// visual information identifying a component, and the numbers belong in
/// front of a reader:
///
/// | separating a button from the card it sits on | light | dark |
/// |---|---|---|
/// | one layer of tonal step | 1.12:1 | 1.26:1 |
/// | `shadow.raised` at its darkest pixel | ~1.6:1 | ~1.4:1 |
/// | `border.subtle` (removed) | **3.34:1** | **5.80:1** |
///
/// So this is a deliberate step *down* in measured boundary contrast, taken
/// on the operator's instruction after seeing both rendered, and it is the
/// call Material 3 and Apple's HIG both make for a filled button: the label
/// and the fill identify the control, and the edge is not carrying that load
/// alone. It is recorded so nobody later reads the absent border as an
/// oversight, and so anyone re-opening it starts from the measurement rather
/// than from taste.
///
/// [`super::field`] keeps its edge. A field is a place to *put* something
/// rather than a thing to press, it carries no label of its own until
/// somebody types one, and an empty well with no boundary does not read as an
/// input at all.
fn labelled(
    key: impl Into<Key>,
    label: impl Into<String>,
    background: &str,
    foreground: &str,
) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut label_props = Props {
        text: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    label_props
        .tokens
        .insert("foreground".into(), t(foreground));
    let inner =
        ViewNode::new(NodeKind::Text, format!("{}-label", key.as_str())).with_props(label_props);

    let mut props = Props {
        axis: Some(Axis::Horizontal),
        padding: Some(pad(SPACING_04, SPACING_03)),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(background));
    props.tokens.insert("radius".into(), t(SHAPE_MD));
    props.tokens.insert("shadow".into(), t(SHADOW_RAISED));

    ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .child(inner)
        .interactive(
            Role::Button,
            label,
            &[Interaction::Focus, Interaction::Click],
        )
}
