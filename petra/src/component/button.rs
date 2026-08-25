//! `button` — the one activatable label component.

use super::pad;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, SHADOW_RAISED, SHAPE_MD, SPACING_MD, SPACING_SM, SURFACE_RAISED,
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
    labelled(key, label, SURFACE_RAISED, TEXT_PRIMARY, Chrome::Edged)
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
    labelled(key, label, ACCENT_PRIMARY, TEXT_ON_ACCENT, Chrome::Raised)
}

/// The shape both entries above share: a padded, rounded `Stack` carrying
/// the chrome, around a `Text` child carrying the label.
///
/// One body rather than two because the *only* differences between a default
/// and a primary button are the two colours and the elevation. Written twice
/// they would drift, and the thing that would drift first is the role and
/// interaction block below — the one part FR-058 says a caller must never be
/// able to skip.
/// What separates a button from the surface under it.
///
/// The two entries are not a style menu; each is the answer to a
/// measurement, and the measurement is in [`Chrome::Edged`]'s doc.
#[derive(Clone, Copy)]
enum Chrome {
    /// A quiet outline in [`BORDER_SUBTLE`], no elevation.
    ///
    /// **Why a default button keeps an edge when the card lost one.**
    /// [`super::on_layer`] seats a button one layer ahead of its ground,
    /// which is the depth cue and is *not* enough on its own to identify a
    /// control. Measured against the card a button actually sits on:
    ///
    /// | | one layer ahead | `border.subtle` |
    /// |---|---|---|
    /// | dark, on `#222222` | **1.26:1** | 5.80:1 |
    /// | light, on `#f2f2f2` | **1.12:1** | 3.34:1 |
    ///
    /// WCAG 2.1 SC 1.4.11 *Non-text Contrast* asks 3:1 for the visual
    /// information required to identify a user-interface component. A tonal
    /// step of 1.12:1 does not come close, and light is the worse of the two
    /// because its layer set *alternates* rather than ramps -- the step there
    /// is `#f2f2f2` to `#ffffff` and there is nowhere further to go.
    ///
    /// So tone carries the depth and a quiet edge carries the boundary. That
    /// pairing is not invented here: `crate::token::shipped`'s `LAYER_TOKENS`
    /// doc already records it as M-Carbon's own rule, taken from Carbon --
    /// *"Borders pair with their same number."*
    ///
    /// The 2026-08-25 pass deleted the outline from the card, the well, the
    /// progress rail and the image frame, all of which had it for decoration
    /// over a shape that already had a fill, and repainted the rest from a
    /// text tone at 10.73:1 to this one. That is the reduction. It is not the
    /// same claim as "no component draws an edge", and this table is here so
    /// that a later reader who wants to finish the job can see what it would
    /// cost before doing it.
    Edged,
    /// An accent fill and an elevation shadow, no outline.
    ///
    /// The accent needs no edge and must not have one: it measures 4.75:1
    /// (dark) and 4.47:1 (light) against the card on its own, well past the
    /// 3:1 floor [`Chrome::Edged`] exists to reach. Drawing a border on top
    /// of that would be the wireframe again, on the one control that least
    /// needs it -- and having exactly one button on the page carry *no*
    /// outline is a second, structural channel saying which one is primary,
    /// for a reader who cannot separate the hue.
    Raised,
}

fn labelled(
    key: impl Into<Key>,
    label: impl Into<String>,
    background: &str,
    foreground: &str,
    chrome: Chrome,
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
        padding: Some(pad(SPACING_MD, SPACING_SM)),
        ..Props::default()
    };
    props.tokens.insert("background".into(), t(background));
    props.tokens.insert("radius".into(), t(SHAPE_MD));
    match chrome {
        Chrome::Edged => {
            props.tokens.insert("border".into(), t(BORDER_SUBTLE));
        }
        // Elevation, and only on the primary. A page where every button
        // casts a shadow has no hierarchy, it just has fog.
        Chrome::Raised => {
            props.tokens.insert("shadow".into(), t(SHADOW_RAISED));
        }
    }

    ViewNode::new(NodeKind::Stack, key)
        .with_props(props)
        .child(inner)
        .interactive(
            Role::Button,
            label,
            &[Interaction::Focus, Interaction::Click],
        )
}
