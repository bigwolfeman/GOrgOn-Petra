//! `button` — Carbon Button (slice-a): one activatable label.
//!
//! # Anatomy
//!
//! A padded, rounded `Stack` around a `text` label. Carbon's four-part
//! skeleton is container + label (+ optional icon + icon wrapper). This
//! rebuild ships the labeled-without-icon case: the container is the
//! clickable region, the label is a child `Text` node, and icons stay
//! caller-supplied later (Carbon itself imports none). The label is
//! **left-aligned on the main axis** (Carbon: never centered except
//! icon-only). Cross-axis [`Align::Center`] sits the glyphs on the optical
//! midline of the pinned height; that is vertical centering, not a
//! centered caption.
//!
//! Role, label, and `Focus`+`Click`+`Hover` are set inside every
//! constructor (FR-058). There is no `button(key)` that compiles.
//!
//! # Height: Petra md, not Carbon lg
//!
//! Carbon's unmodified `.cds--btn` is **lg 48** (`layout.use('size',
//! $default: 'lg')`, `_button.scss`). Gate G1 pins [`button`] and
//! [`primary_button`] at Carbon **md 40** ([`SIZE_MD`]) at scale 1.0.
//! `button_lg` is the Carbon unmodified height. `xs`/`sm`/`md`/`lg` are
//! explicit constructors; `xl`/`2xl` are ambient layout-size in Carbon
//! (no `.cds--btn--xl` class) and are not shipped here.
//!
//! # Variants
//!
//! Carbon's seven, MEASURED from `.cds--btn` modifier classes: `primary`
//! (also the bare `.cds--btn` default), `secondary`, `tertiary`, `ghost`,
//! `danger`, `danger-tertiary`, `danger-ghost`.
//!
//! Petra's [`button`] is Carbon **secondary** (quiet filled), because
//! [`primary_button`] already spends the accent. Carbon's bare `.cds--btn`
//! is primary; we split that default so reaching for the generic name
//! cannot put two accents on a page.
//!
//! # Padding
//!
//! Carbon without icon: `padding-left` 16 (`$spacing-05`), `padding-right`
//! 64 (`$spacing-10`) — the large end pad reserves room for an icon so the
//! label never collides with the edge. `$spacing-10` is not in
//! [`super::tokens`] (the component ramp stops at [`SPACING_05`]), and
//! [`InsetRefs`] only takes token names. Both inline edges therefore bind
//! [`SPACING_05`]. Block padding is none: height is the pinned constraint,
//! matching Carbon's `min-height` + `padding-block: 0` shape.
//!
//! # Danger: Carbon's red fill, and the octagon stays
//!
//! Carbon's `.cds--btn--danger` is `background-color: $button-danger-primary`
//! (`#da1e28` in every theme) with `color: $text-on-color` (`#ffffff`), and
//! its tertiary and ghost forms are `$button-danger-secondary` (`#fa4d56`
//! at g100) as an outline and as ink. All three values are MEASURED from
//! `@carbon/themes/scss/generated/_button-tokens.scss`.
//!
//! ## The two names this comment used to ask for now exist
//!
//! The paragraph that follows is wave E's measurement and it is kept
//! because it is why the token layer owed two names rather than one alias.
//! **Petra ships exactly one red and it is neither of Carbon's.**
//! [`SUPPORT_ERROR`] resolves to `#f21c0d` in the dark theme, tuned as a
//! *status marker* — it has to clear 3:1 against a surface, so it was
//! pushed bright. Measured against every ink this library owns, no label
//! clears the 4.5:1 AA floor on it:
//!
//! | ink on the dark `support-error` fill | measured |
//! |---|---|
//! | `text.on-accent` `#121212` | **4.41:1** |
//! | `text-inverse` `#1a1a1a` | 4.10:1 |
//! | `text.primary` `#f2f2f2` | 3.79:1 |
//! | `layer-selected-inverse` `#ffffff` | 4.24:1 |
//!
//! Carbon's own `#da1e28` clears it at 5.00:1 against white, which is the
//! whole difference. So the ask was *"a button-danger fill and the
//! always-white ink that goes on it"*, and on 2026-09-05
//! [`BUTTON_DANGER_PRIMARY`] and [`TEXT_ON_COLOR`] shipped in
//! `crate::token::shipped`, each with its own gate. [`danger_button`] binds
//! both.
//!
//! ## The octagon stays, and on the fill it turns white
//!
//! Carbon has no such mark. This library keeps it because **FR-015 says
//! colour is never the only channel** and the operator is red-green colour
//! blind: a red button that differs from a grey one only in hue does not
//! differ from it for him. So the [`SILHOUETTE_OCTAGON`] leads all three
//! danger variants, the same figure `status.down` paints, and it carries
//! the kind in a channel that survives greyscale.
//!
//! What changed with the fill is the mark's **tone**, and it changed
//! because of a measurement rather than a preference: `support-error`
//! `#f21c0d` on `#da1e28` is **1.18:1**. Two reds of nearly equal
//! luminance is an invisible mark, for everybody, colour vision or not. On
//! the filled variant the octagon therefore takes [`TEXT_ON_COLOR`], which
//! is 5.00:1 on the fill; on the tertiary and ghost variants, whose ground
//! is a grey surface, it keeps [`SUPPORT_ERROR`]. Both tones are pinned by
//! [`tests::the_danger_mark_is_visible_against_whatever_the_variant_fills_with`].
//!
//! On the two grey grounds the mark still carries hue as a second channel:
//! `#f21c0d` against the default button's `#222222` measures ΔE\*ab **47.2**
//! under Viénot-Brettel-Mollon protanope simulation and **76.3** under
//! deuteranope, against this repo's own `MIN_STATUS_SEPARATION` floor of
//! 30, at 3.75:1 — over the 3:1 SC 1.4.11 floor for a graphical object.
//!
//! ## What tertiary and ghost do *not* get, and why
//!
//! Carbon's danger-tertiary and danger-ghost are `$button-danger-secondary`
//! at rest and fill with `$button-danger-primary` under `$text-on-color` on
//! hover. Neither half is portable here yet:
//!
//! * `$button-danger-secondary` has no name in this library, and inventing
//!   a second red in a component is the decision `crate::token::shipped`'s
//!   own comments exist to prevent.
//! * The hover fill *is* reachable — the wrapper declares `Hover` and
//!   `background@hover` resolves — but the **ink cannot follow it**. The
//!   label is a child `Text` node and the interaction state belongs to the
//!   node it was declared on, so a `foreground@hover` on the label would
//!   never fire. A danger-tertiary that filled red on hover would carry
//!   [`TEXT_PRIMARY`] `#f2f2f2` on `#da1e28`, which is **4.47:1** — missing
//!   AA by 0.03, the same 0.03 the accent's own floor argument turns on.
//!
//! So both keep [`TEXT_PRIMARY`] and lean on the octagon, and the hover
//! fill is left for whoever gives a child node a way to read its parent's
//! state. **This is an open departure from Carbon, not a closed one.**
//!
//! The mark leads rather than trails. Carbon's icon slot is the trailing
//! one (`.cds--btn { justify-content: space-between }`) and is held open by
//! `$spacing-10` of end padding — [`PAD_END_NO_ICON`], the number this file
//! deliberately does not bind — so a trailing mark here would sit tight
//! against the edge. Leading also puts the warning before the word.
//!
//! `Semantics.value` still names the kind, for the reader who has neither.
//!
//! # Ghost is a link, not a label
//!
//! MEASURED `_button.scss:196`: `.cds--btn--ghost` is
//! `button-theme(transparent, transparent, $link-primary, ...)`. Its ink is
//! the link colour, which is what makes a fill-less, edge-less control read
//! as a control at all — and it is why the reference draws "Ghost" in blue
//! where ours drew it in the same white as the label beside it. Carbon's
//! danger-ghost and danger-tertiary want `$button-danger-secondary` there
//! instead; that ink does not exist here (see above), so those two keep
//! [`TEXT_PRIMARY`] and lean on the octagon.
//!
//! # A filled button sits *on* the surface
//!
//! Filled variants ([`button`], [`primary_button`], [`danger_button`])
//! cast [`SHADOW_RAISED`] and draw no outline. Ghost and tertiary do not
//! shout a fill; tertiary (and danger-tertiary) take [`BORDER_STRONG`] —
//! `border.subtle` until 2026-09-05, when that name became the *decorative*
//! tone and every edge that identifies a control moved to the one still
//! held at SC 1.4.11's 3:1. A tertiary button with no fill is exactly the
//! case the split's control side is for.
//! Ghost (and danger-ghost) bind [`SURFACE_BASE`] and no shadow, so
//! [`super::on_layer`] can disappear them into their ground.
//!
//! A button was outlined in `border.subtle` for part of 2026-08-25. That
//! outline was the only thing here reaching WCAG 2.1 SC 1.4.11's 3:1 for
//! the visual information identifying a component:
//!
//! | separating a filled button from the card it sits on | light | dark |
//! |---|---|---|
//! | one layer of tonal step | 1.12:1 | 1.26:1 |
//! | `shadow.raised` at its darkest pixel | ~1.6:1 | ~1.4:1 |
//! | `border.subtle` (removed from filled) | **3.34:1** | **5.80:1** |
//!
//! So filled-without-outline is a deliberate step *down* in measured
//! boundary contrast, taken on the operator's instruction after seeing
//! both rendered. Tertiary keeps the edge because without a fill the
//! outline *is* the control, the same reason a checkbox does.

use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_STRONG, BUTTON_DANGER_PRIMARY, ICON_DISABLED, ICON_ON_COLOR_DISABLED,
    LAYER_ACTIVE, LAYER_HOVER, LINK_PRIMARY, SHADOW_RAISED, SHAPE_MD, SILHOUETTE_OCTAGON, SIZE_MD,
    SPACING_03, SPACING_05, SUPPORT_ERROR, SURFACE_BASE, SURFACE_RAISED, TEXT_ON_ACCENT,
    TEXT_ON_COLOR, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Interaction, Key, Role, ViewNode};

/// Carbon `.cds--btn--xs` height. Numeric because `Constraints` stay extents
/// (FR-053); [`super::tokens`] only ships [`SIZE_MD`].
const HEIGHT_XS: f32 = 24.0;
/// Carbon `.cds--btn--sm` height. See [`HEIGHT_XS`].
const HEIGHT_SM: f32 = 32.0;
/// Carbon unmodified `.cds--btn` height (`lg`). [`button`] is md, not this.
const HEIGHT_LG: f32 = 48.0;

/// Carbon `$spacing-10` = 64, the no-icon `padding-right`. Not bound: see
/// the module comment. Named so a later tokens.rs extension has a number
/// to meet, and so tests can petrify the fact we did not invent the name.
const PAD_END_NO_ICON: f32 = 64.0;

const _: () = assert!(HEIGHT_XS == 24.0);
const _: () = assert!(HEIGHT_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(HEIGHT_LG == 48.0);
const _: () = assert!(PAD_END_NO_ICON == 64.0);

/// The danger mark's extent, Carbon's own button-icon box.
const DANGER_MARK: f32 = 16.0;

const _: () = assert!(DANGER_MARK == 16.0);

/// The paint slot naming a node's outline family — the painter's
/// `SILHOUETTE_SLOT`. Spelled here for the reason `status.rs` spells it: a
/// slot key is a plain string by design (`Props::tokens`), not a token name
/// this module could import.
const SILHOUETTE_SLOT: &str = "silhouette";

#[derive(Clone, Copy)]
enum Size {
    Xs,
    Sm,
    Md,
    Lg,
}

impl Size {
    fn height(self) -> f32 {
        match self {
            Self::Xs => HEIGHT_XS,
            Self::Sm => HEIGHT_SM,
            Self::Md => SIZE_MD,
            Self::Lg => HEIGHT_LG,
        }
    }
}

#[derive(Clone, Copy)]
enum Variant {
    Primary,
    Secondary,
    Tertiary,
    Ghost,
    Danger,
    DangerTertiary,
    DangerGhost,
}

impl Variant {
    /// Status-red stand-in: a value string, never a hue. `None` for the
    /// four variants whose chrome is already unique without it.
    fn danger_kind(self) -> Option<&'static str> {
        match self {
            Self::Danger => Some("danger"),
            Self::DangerTertiary => Some("danger-tertiary"),
            Self::DangerGhost => Some("danger-ghost"),
            Self::Primary | Self::Secondary | Self::Tertiary | Self::Ghost => None,
        }
    }

    /// The ink the leading octagon is filled with, which follows the
    /// variant's *ground* rather than its kind. See [`danger_mark`] for the
    /// two measurements; the short version is that a red mark on a red fill
    /// is 1.18:1 and invisible.
    ///
    /// Total over the enum on purpose: the four non-danger variants never
    /// reach [`danger_mark`], and returning their tone here anyway means a
    /// new variant has to say which ground its mark lands on rather than
    /// falling through a wildcard onto whichever tone was listed last.
    fn mark_tone(self) -> &'static str {
        match self {
            Self::Danger => TEXT_ON_COLOR,
            Self::DangerTertiary
            | Self::DangerGhost
            | Self::Primary
            | Self::Secondary
            | Self::Tertiary
            | Self::Ghost => SUPPORT_ERROR,
        }
    }
}

/// The chrome one variant paints. A struct rather than a pile of
/// positional `&str`s: every name is a token, so every transposition
/// compiles and paints something plausible-but-wrong.
#[derive(Clone, Copy)]
struct Chrome {
    background: &'static str,
    foreground: &'static str,
    hover: Option<&'static str>,
    active: Option<&'static str>,
    disabled_ink: &'static str,
    border: Option<&'static str>,
    shadow: Option<&'static str>,
}

fn chrome(variant: Variant) -> Chrome {
    match variant {
        Variant::Primary => Chrome {
            background: ACCENT_PRIMARY,
            foreground: TEXT_ON_ACCENT,
            // **No hover or pressed surface, deliberately.** The two state
            // tones a secondary button uses step off the *layer* ramp, and
            // binding them here would turn the one accent control on the
            // page grey the moment a pointer touched it. The vocabulary
            // has no `accent.primary-hover`; inventing the tone here would
            // put a colour decision in a component. Recorded so the gap
            // is a known one.
            hover: None,
            active: None,
            disabled_ink: ICON_ON_COLOR_DISABLED,
            border: None,
            shadow: Some(SHADOW_RAISED),
        },
        Variant::Secondary => Chrome {
            background: SURFACE_RAISED,
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: None,
            shadow: Some(SHADOW_RAISED),
        },
        // Carbon's `.cds--btn--danger`, at last: `$button-danger-primary`
        // under `$text-on-color`. It shared `Secondary`'s grey chrome until
        // 2026-09-05, when the token layer grew the two names — see the
        // module doc for the 4.41:1 that ruled `support-error` out as a fill
        // and the 1.18:1 that turns the octagon white on this one.
        //
        // **No hover or pressed surface**, for the reason `Primary` states:
        // the two state tones step off the *layer* ramp, so binding them
        // would turn the one red control on the page grey the moment a
        // pointer touched it. Carbon has `$button-danger-hover`; this
        // library has no name for it, and inventing a second red here is
        // the decision `token::shipped` exists to hold.
        Variant::Danger => Chrome {
            background: BUTTON_DANGER_PRIMARY,
            foreground: TEXT_ON_COLOR,
            hover: None,
            active: None,
            disabled_ink: ICON_ON_COLOR_DISABLED,
            border: None,
            shadow: Some(SHADOW_RAISED),
        },
        Variant::Tertiary | Variant::DangerTertiary => Chrome {
            background: SURFACE_BASE,
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            // The control boundary, not the decorative rule: a tertiary
            // button has no fill, so this edge is the only thing that says
            // a control is here. See `tokens::BORDER_SUBTLE` for the split.
            border: Some(BORDER_STRONG),
            shadow: None,
        },
        Variant::Ghost => Chrome {
            background: SURFACE_BASE,
            // MEASURED `_button.scss:196`: `color: $link-primary`.
            foreground: LINK_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: None,
            shadow: None,
        },
        Variant::DangerGhost => Chrome {
            background: SURFACE_BASE,
            // Carbon wants `$button-danger-secondary` here; this library has
            // no such ink (module doc), and `link-primary` on a *danger*
            // control would say "safe". The octagon carries the kind.
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: None,
            shadow: None,
        },
    }
}

/// A [`DANGER_MARK`]-unit octagon in `tone`: the same figure `status.down`
/// paints, at the button-icon box.
///
/// **`tone` is a parameter because the ground moved.** On the two grey
/// variants the mark is [`SUPPORT_ERROR`], 3.75:1 on the button fill and
/// ΔE\*ab 47.2/76.3 apart from it under the two red-green simulations. On
/// the filled danger button that same red is **1.18:1** against
/// [`BUTTON_DANGER_PRIMARY`] — two reds of nearly equal luminance, which is
/// no mark at all — so there it takes [`TEXT_ON_COLOR`] at 5.00:1. The
/// shape is the channel either way;
/// [`tests::the_danger_mark_is_visible_against_whatever_the_variant_fills_with`]
/// holds both numbers.
///
/// No fill token on a text node and no `border` slot, so neither the AA
/// floor nor
/// [`super::tests::containers_take_a_tone_and_controls_take_an_edge`]'s
/// border-tone rule has anything to say about it.
fn danger_mark(tone: &str) -> ViewNode {
    let mut mark = swatch("mark", DANGER_MARK, DANGER_MARK, Some(tone), None, None);
    mark.props
        .tokens
        .insert(SILHOUETTE_SLOT.into(), t(SILHOUETTE_OCTAGON));
    mark
}

fn pin_height(h: f32) -> Constraints {
    Constraints {
        vertical: AxisConstraint {
            min: Some(h),
            max: Some(h),
            priority: 0,
        },
        ..Constraints::default()
    }
}

/// Carbon no-icon `padding-inline: $spacing-05`. Block is none: height is
/// pinned. `$spacing-10` on the end is [`PAD_END_NO_ICON`], not bound.
fn pad_inline() -> InsetRefs {
    InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    }
}

/// A focusable, clickable control with a label.
///
/// Carbon **secondary** at **md 40**, not Carbon's unmodified primary+lg.
/// See the module comment for why those two facts are split. `label` is a
/// required positional parameter — there is no `button(key)` that compiles.
/// Role and interactions are set inside this function (FR-058).
pub fn button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Secondary, Size::Md)
}

/// Carbon secondary at xs 24.
pub fn button_xs(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Secondary, Size::Xs)
}

/// Carbon secondary at sm 32.
pub fn button_sm(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Secondary, Size::Sm)
}

/// Carbon secondary at lg 48 — Carbon's unmodified `.cds--btn` height.
/// Petra's [`button`] is md; this is the explicit lg constructor.
pub fn button_lg(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Secondary, Size::Lg)
}

/// The page's one loudest action, filled with the accent instead of a grey.
///
/// Same default height as [`button`]: Carbon md 40, not Carbon lg 48.
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
    labelled(key, label, Variant::Primary, Size::Md)
}

/// Carbon tertiary: no fill shouting, [`BORDER_STRONG`] edge, no shadow.
pub fn tertiary_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Tertiary, Size::Md)
}

/// Carbon ghost: [`SURFACE_BASE`] fill, no shadow, no border.
pub fn ghost_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Ghost, Size::Md)
}

/// Carbon danger (filled). Same structure as [`button`]; the kind lives in
/// `Semantics.value`, not a painted red — see the module comment.
pub fn danger_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::Danger, Size::Md)
}

/// Carbon danger-tertiary: outlined, no shadow. Kind in `Semantics.value`.
pub fn danger_tertiary_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::DangerTertiary, Size::Md)
}

/// Carbon danger-ghost: base fill, no shadow, no border. Kind in
/// `Semantics.value`.
pub fn danger_ghost_button(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    labelled(key, label, Variant::DangerGhost, Size::Md)
}

/// Shared body for every public constructor: variant and size are the
/// only knobs. Role and interactions live here so FR-058 is a fact about
/// this function, not a convention its caller is trusted to follow.
///
/// A `button` is a padded, rounded `Stack` around a `text` label rather
/// than a bare styled `Text` leaf, because `Props.padding` only applies to
/// container kinds (`crate::tree::validate::Violation::PaddingOnLeafKind`)
/// — a `Text` node has no children to inset. The label lives in the child;
/// the role, the interactions, and the chrome live on the wrapper.
fn labelled(
    key: impl Into<Key>,
    label: impl Into<String>,
    variant: Variant,
    size: Size,
) -> ViewNode {
    let key = key.into();
    let label = label.into();
    let chrome = chrome(variant);

    // Carbon's label is body-compact-01 (14/18). `text()` binds
    // `typography.body` (14/20) — the closest name `tokens.rs` ships.
    // `typography.body-compact` is in the vocab; spelling it here would
    // invent a library constant.
    let mut inner = text(format!("{}-label", key.as_str()), label.clone());
    inner
        .props
        .tokens
        .insert("foreground".into(), t(chrome.foreground));
    // The disabled family, declared here rather than patched on afterwards.
    // `super::disabled` sets one flag; which token that flag reaches for is
    // the component's own decision, and putting it here is what stops a
    // caller from having to reach into a button's label child to express
    // "unavailable" (`contracts/interaction-state.md` §6).
    inner
        .props
        .tokens
        .insert("foreground@disabled".into(), t(chrome.disabled_ink));

    // The danger triple leads with the library's own `status.down` figure
    // in `support-error`. See the module doc for the four contrast
    // measurements that rule a red fill out and the two CVD separations
    // that make this mark legible to the operator.
    let (children, spacing) = match variant.danger_kind() {
        Some(_) => (
            vec![danger_mark(variant.mark_tone()), inner],
            Some(SPACING_03),
        ),
        None => (vec![inner], None),
    };
    let mut node = stack(key, Axis::Horizontal, spacing, children);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(pad_inline());
    node.props
        .tokens
        .insert("background".into(), t(chrome.background));
    // Hover and pressed, as *named* surfaces rather than as a lightening
    // rule. The painter resolves which one is in force through
    // `crate::token::state`'s precedence chain, so nothing here branches on a
    // hover bool and nothing here has to be told when the pointer moves.
    //
    // Both are optional, and an unbound one is not a gap in the chain: the
    // lookup falls through to `background`, so a button with no hover tone
    // simply keeps its resting fill. See [`primary_button`] for the one that
    // does.
    for (slot, token) in [
        ("background@hover", chrome.hover),
        ("background@active", chrome.active),
    ] {
        if let Some(token) = token {
            node.props.tokens.insert(slot.into(), t(token));
        }
    }
    node.props.tokens.insert("radius".into(), t(SHAPE_MD));
    if let Some(shadow) = chrome.shadow {
        node.props.tokens.insert("shadow".into(), t(shadow));
    }
    if let Some(border) = chrome.border {
        node.props.tokens.insert("border".into(), t(border));
    }

    let mut node = node
        .with_constraints(pin_height(size.height()))
        .interactive(
            Role::Button,
            label,
            // `Hover` is not decoration. A node that does not declare it is
            // not a hit-test candidate for hover, so the engine never lights
            // it and the `background@hover` binding above would be a token
            // nothing ever reads — the "declared name nothing reads" defect
            // `crate::token::shipped` counts, aimed at the slot channel.
            &[Interaction::Focus, Interaction::Click, Interaction::Hover],
        );
    if let Some(kind) = variant.danger_kind() {
        node.semantics.value = Some(kind.to_owned());
    }
    node
}

#[cfg(test)]
mod tests {
    use super::{
        ACCENT_PRIMARY, BORDER_STRONG, BUTTON_DANGER_PRIMARY, HEIGHT_LG, HEIGHT_SM, HEIGHT_XS,
        LINK_PRIMARY, PAD_END_NO_ICON, SHADOW_RAISED, SIZE_MD, SPACING_05, SUPPORT_ERROR,
        SURFACE_BASE, TEXT_ON_ACCENT, TEXT_ON_COLOR, TEXT_PRIMARY, button, button_lg, button_sm,
        button_xs, danger_button, danger_ghost_button, danger_tertiary_button, ghost_button,
        primary_button, tertiary_button,
    };
    use crate::frame::{TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        crate::anim::shipped_registry().declare_into(&mut registry);
        registry
    }

    fn petrify_lone(child: ViewNode) -> crate::frame::PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(child);
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

    fn placed_height(frame: &crate::frame::PetrifiedFrame, key: &str) -> f32 {
        let suffix = format!("/root/{key}");
        frame
            .placements
            .iter()
            .find(|p| p.id.ends_with(&suffix))
            .unwrap_or_else(|| panic!("missing placement ending {suffix}"))
            .rect
            .h
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(TokenName::as_str)
    }

    fn assert_role_label_and_intents(node: &ViewNode, label: &str) {
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some(label));
        assert!(node.is_interactive());
        for intent in [Interaction::Focus, Interaction::Click, Interaction::Hover] {
            assert!(
                node.interactions.contains(&intent),
                "{:?} missing {intent:?}",
                node.key
            );
        }
    }

    /// G1: default `button()` is Carbon md 40 at scale 1.0, not Carbon lg 48.
    #[test]
    fn default_button_height_is_carbon_md_40_at_scale_1() {
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(HEIGHT_LG, 48.0);
        let theme = crate::token::light();
        let value = theme
            .value(&TokenName::new("size-md").unwrap())
            .expect("size-md is in the vocabulary");
        match value {
            crate::token::TokenValue::Spacing(units) => assert_eq!(*units, SIZE_MD),
            other => panic!("size-md should be a spacing value, got {other:?}"),
        }
        let node = button("b", "Save");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        let frame = petrify_lone(node);
        assert_eq!(placed_height(&frame, "b"), SIZE_MD);
        let primary = primary_button("p", "Save");
        assert_eq!(primary.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(primary.constraints.vertical.max, Some(SIZE_MD));
        let frame = petrify_lone(primary);
        assert_eq!(placed_height(&frame, "p"), SIZE_MD);
    }

    #[test]
    fn size_constructors_pin_carbon_xs_sm_lg() {
        let theme = crate::token::light();
        for (name, expected) in [
            ("size-xs", HEIGHT_XS),
            ("size-sm", HEIGHT_SM),
            ("size-lg", HEIGHT_LG),
        ] {
            match theme.value(&TokenName::new(name).unwrap()) {
                Some(crate::token::TokenValue::Spacing(units)) => {
                    assert_eq!(*units, expected, "{name}")
                }
                other => panic!("{name} should be a spacing value, got {other:?}"),
            }
        }

        let xs = button_xs("xs", "Save");
        assert_eq!(xs.constraints.vertical.min, Some(HEIGHT_XS));
        assert_eq!(xs.constraints.vertical.max, Some(HEIGHT_XS));
        assert_eq!(placed_height(&petrify_lone(xs), "xs"), HEIGHT_XS);

        let sm = button_sm("sm", "Save");
        assert_eq!(sm.constraints.vertical.min, Some(HEIGHT_SM));
        assert_eq!(sm.constraints.vertical.max, Some(HEIGHT_SM));
        assert_eq!(placed_height(&petrify_lone(sm), "sm"), HEIGHT_SM);

        let lg = button_lg("lg", "Save");
        assert_eq!(lg.constraints.vertical.min, Some(HEIGHT_LG));
        assert_eq!(lg.constraints.vertical.max, Some(HEIGHT_LG));
        assert_eq!(placed_height(&petrify_lone(lg), "lg"), HEIGHT_LG);
    }

    #[test]
    fn every_constructor_sets_role_label_and_intents() {
        let nodes = [
            button("b", "Save"),
            button_xs("xs", "Save"),
            button_sm("sm", "Save"),
            button_lg("lg", "Save"),
            primary_button("p", "Save"),
            tertiary_button("t", "Save"),
            ghost_button("g", "Save"),
            danger_button("d", "Delete"),
            danger_tertiary_button("dt", "Delete"),
            danger_ghost_button("dg", "Delete"),
        ];
        for node in &nodes {
            let label = node.semantics.label.clone().expect("label");
            assert_role_label_and_intents(node, &label);
            assert_eq!(node.kind, NodeKind::Stack);
            assert_eq!(node.props.align, Some(Align::Center));
        }
    }

    #[test]
    fn inline_padding_is_spacing_05_not_an_invented_spacing_10() {
        assert_eq!(PAD_END_NO_ICON, 64.0);
        let pad = button("b", "Save").props.padding.expect("padding");
        assert_eq!(pad.left.as_ref().map(TokenName::as_str), Some(SPACING_05));
        assert_eq!(pad.right.as_ref().map(TokenName::as_str), Some(SPACING_05));
        assert!(pad.top.is_none());
        assert!(pad.bottom.is_none());
    }

    #[test]
    fn filled_buttons_cast_shadow_raised_and_draw_no_edge() {
        for node in [
            button("b", "Save"),
            primary_button("p", "Save"),
            danger_button("d", "Delete"),
        ] {
            assert_eq!(token(&node, "shadow"), Some(SHADOW_RAISED));
            assert!(token(&node, "border").is_none());
        }
    }

    #[test]
    fn primary_button_spends_the_accent_and_on_accent_ink() {
        let node = primary_button("p", "Save");
        assert_eq!(token(&node, "background"), Some(ACCENT_PRIMARY));
        let label = node.children.first().expect("label child");
        assert_eq!(token(label, "foreground"), Some(TEXT_ON_ACCENT));
        assert_ne!(
            token(&button("b", "Cancel"), "background"),
            Some(ACCENT_PRIMARY)
        );
    }

    #[test]
    fn tertiary_has_a_border_ghost_does_not_shout() {
        let tertiary = tertiary_button("t", "Save");
        assert_eq!(token(&tertiary, "border"), Some(BORDER_STRONG));
        assert!(token(&tertiary, "shadow").is_none());
        assert_eq!(token(&tertiary, "background"), Some(SURFACE_BASE));

        let ghost = ghost_button("g", "Save");
        assert!(token(&ghost, "border").is_none());
        assert!(token(&ghost, "shadow").is_none());
        assert_eq!(token(&ghost, "background"), Some(SURFACE_BASE));
    }

    #[test]
    fn danger_is_named_in_semantics_not_painted_accent_red() {
        let danger = danger_button("d", "Delete");
        assert_eq!(danger.semantics.value.as_deref(), Some("danger"));
        assert_ne!(token(&danger, "background"), Some(ACCENT_PRIMARY));
        assert_eq!(token(&danger, "background"), Some(BUTTON_DANGER_PRIMARY));
        assert_eq!(token(&danger, "shadow"), Some(SHADOW_RAISED));

        let dt = danger_tertiary_button("dt", "Delete");
        assert_eq!(dt.semantics.value.as_deref(), Some("danger-tertiary"));
        assert_eq!(token(&dt, "border"), Some(BORDER_STRONG));
        assert!(token(&dt, "shadow").is_none());

        let dg = danger_ghost_button("dg", "Delete");
        assert_eq!(dg.semantics.value.as_deref(), Some("danger-ghost"));
        assert!(token(&dg, "border").is_none());
        assert!(token(&dg, "shadow").is_none());

        assert!(button("b", "Save").semantics.value.is_none());
        assert!(primary_button("p", "Save").semantics.value.is_none());
    }

    #[test]
    fn label_is_a_text_child_not_a_leaf_button() {
        let node = button("b", "Save");
        assert_eq!(node.kind, NodeKind::Stack);
        let label = node.children.first().expect("label");
        assert_eq!(label.kind, NodeKind::Text);
        assert_eq!(label.props.text.as_deref(), Some("Save"));
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// The leading octagon is **findable on the fill it is drawn on**, in
    /// both themes, for all three danger variants.
    ///
    /// # The defect this was written after
    ///
    /// Giving [`danger_button`] Carbon's red fill on 2026-09-05 made the
    /// mark and the ground the same colour family. `support-error`
    /// `#f21c0d` on `button-danger-primary` `#da1e28` is **1.18:1**: two
    /// reds of nearly equal luminance, so the octagon disappears into the
    /// button while every structural assertion about it — it exists, it is
    /// first, it is 16 units, it carries a silhouette — still passes. That
    /// is the exact shape of failure this repo's rules are written against,
    /// and no test in this file could have caught it, because none of them
    /// read a colour against another colour.
    ///
    /// So the floor is WCAG 2.1 SC 1.4.11's 3:1 for a graphical object, and
    /// the ground is whatever the variant's own `chrome` fills with — read
    /// out of the built node rather than named here, so a chrome edit that
    /// changes the ground and forgets the mark fails.
    ///
    /// **Falsify it** by making `Variant::mark_tone` return `SUPPORT_ERROR`
    /// for `Danger`, which is what it did before the fill landed.
    #[test]
    fn the_danger_mark_is_visible_against_whatever_the_variant_fills_with() {
        /// WCAG 2.1 SC 1.4.11 *Non-text Contrast*, Level AA. The octagon is
        /// a graphical object that conveys information, not body text.
        const MIN_MARK_CONTRAST: f32 = 3.0;

        for (theme_name, theme) in [
            ("light", crate::token::shipped::light()),
            ("dark", crate::token::shipped::dark()),
        ] {
            for (label, node) in [
                ("danger", danger_button("b", "Delete")),
                ("danger-tertiary", danger_tertiary_button("b", "Delete")),
                ("danger-ghost", danger_ghost_button("b", "Delete")),
            ] {
                let ground = color(
                    &theme,
                    node.props
                        .tokens
                        .get("background")
                        .expect("every danger variant fills with something")
                        .as_str(),
                );
                let mark = node
                    .children
                    .iter()
                    .find(|c| c.key.as_str() == "mark")
                    .expect("every danger variant leads with a mark");
                let tone = mark
                    .props
                    .tokens
                    .get("background")
                    .expect("the mark is a filled swatch");
                let ratio = color(&theme, tone.as_str()).contrast_ratio(ground);
                assert!(
                    ratio >= MIN_MARK_CONTRAST,
                    "{theme_name}/{label}: the octagon in `{}` measures \
                     {ratio:.2}:1 against the fill this variant paints, under \
                     the {MIN_MARK_CONTRAST}:1 SC 1.4.11 floor. A mark nobody \
                     can see is not a second channel, and the whole reason \
                     this library keeps a mark Carbon does not have is that \
                     the operator cannot rely on the hue.",
                    tone.as_str()
                );
            }
        }
    }

    /// Check C/D across every variant, every size, and a disabled state:
    /// no degenerate rect, no child placed outside its parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("secondary-xs", button_xs("b", "Save")),
            ("secondary-sm", button_sm("b", "Save")),
            ("secondary-md", button("b", "Save")),
            ("secondary-lg", button_lg("b", "Save")),
            ("primary", primary_button("b", "Save")),
            ("tertiary", tertiary_button("b", "Save")),
            ("ghost", ghost_button("b", "Save")),
            ("danger", danger_button("b", "Delete")),
            ("danger-tertiary", danger_tertiary_button("b", "Delete")),
            ("danger-ghost", danger_ghost_button("b", "Delete")),
            (
                "disabled",
                crate::component::disabled(button("b", "Unavailable")),
            ),
        ];
        for (label, node) in cases {
            let frame = petrify_lone(node);
            assert!(!frame.placements.is_empty(), "{label}: nothing placed");
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{label}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
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
                        "{label}: {} (rect {:?}) extends outside its parent {} (rect {:?})",
                        p.id, p.rect, parent.id, parent.rect
                    );
                }
            }
        }
    }

    /// Check F: an enabled button is reachable; a disabled one is not.
    #[test]
    fn focus_reachability_matches_disabled_state() {
        for (label, node, should_be_focusable) in [
            ("enabled", button("b", "Save"), true),
            (
                "disabled",
                crate::component::disabled(button("b", "Unavailable")),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let root_placement = frame
                .placements
                .iter()
                .find(|p| p.id == "/root/b")
                .expect("button is placed");
            let reachable = focus.order().iter().any(|id| id == &root_placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: every variant's label against its own resting fill, in both
    /// themes, read through `Props.opacity`.
    #[test]
    fn label_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("secondary", button("b", "Save")),
                ("primary", primary_button("b", "Save")),
                ("tertiary", tertiary_button("b", "Save")),
                ("ghost", ghost_button("b", "Save")),
                ("danger", danger_button("b", "Delete")),
                ("danger-tertiary", danger_tertiary_button("b", "Delete")),
                ("danger-ghost", danger_ghost_button("b", "Delete")),
            ] {
                let bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: button has no resting background"));
                let bg = color(&theme, bg_name.as_str());
                let text = node
                    .children
                    .iter()
                    .find(|c| c.key.as_str() == "b-label")
                    .unwrap_or_else(|| panic!("{label}: no label child"));
                let fg_name = text
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label binds a foreground");
                let opacity = text.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: label at {ratio:.2}:1 against {bg_name} fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }

    /// The danger triple's mark: a leading octagon on all three, and on
    /// none of the other four, in the tone that suits the ground each one
    /// fills with.
    ///
    /// The measurements this asserts the *consequences* of live in the
    /// module doc and in
    /// [`the_danger_mark_is_visible_against_whatever_the_variant_fills_with`].
    /// The mark is a shape as well as a hue, which is what makes it legible
    /// to a red-green colour blind reader; the hue changes with the ground
    /// because `support-error` on Carbon's danger fill is 1.18:1. Falsify by
    /// returning `None` from `Variant::danger_kind` for one variant, by
    /// deleting the `silhouette` insert in `danger_mark`, or by collapsing
    /// `Variant::mark_tone` back to one tone.
    #[test]
    fn every_danger_variant_leads_with_an_octagon_toned_for_its_ground() {
        use crate::component::tokens::SILHOUETTE_OCTAGON;
        let mark = |node: &ViewNode| -> Option<(String, String, String)> {
            let m = node.children.iter().find(|c| c.key.as_str() == "mark")?;
            Some((
                m.props.tokens.get("background")?.as_str().to_owned(),
                m.props.tokens.get("silhouette")?.as_str().to_owned(),
                format!("{:?}", m.kind),
            ))
        };
        for (label, node, tone) in [
            // The filled variant's ground is Carbon's red, so the mark
            // cannot also be red: 1.18:1. It takes the on-colour white.
            ("danger", danger_button("b", "Delete"), TEXT_ON_COLOR),
            (
                "danger-tertiary",
                danger_tertiary_button("b", "Delete"),
                SUPPORT_ERROR,
            ),
            (
                "danger-ghost",
                danger_ghost_button("b", "Delete"),
                SUPPORT_ERROR,
            ),
        ] {
            let found = mark(&node)
                .unwrap_or_else(|| panic!("{label} draws no mark: danger is a colour-only kind"));
            assert_eq!(
                found,
                (
                    tone.to_owned(),
                    SILHOUETTE_OCTAGON.to_owned(),
                    "Spacer".to_owned()
                ),
                "{label}'s mark is not the `status.down` figure in the tone \
                 its own ground can carry"
            );
            assert_eq!(
                node.children.first().map(|c| c.key.as_str()),
                Some("mark"),
                "{label}'s mark leads the label; Carbon's trailing icon slot                  is held open by `$spacing-10`, which this file does not bind"
            );
            assert_eq!(
                node.children.first().unwrap().constraints.horizontal.min,
                Some(super::DANGER_MARK)
            );
        }
        for (label, node) in [
            ("secondary", button("b", "Save")),
            ("primary", primary_button("b", "Save")),
            ("tertiary", tertiary_button("b", "Save")),
            ("ghost", ghost_button("b", "Save")),
        ] {
            assert!(
                mark(&node).is_none(),
                "{label} grew a danger mark; the mark is what tells the two                  apart and it has to be absent from the safe four"
            );
        }
    }

    /// MEASURED `_button.scss:196`: a ghost button's ink is `$link-primary`.
    /// Ours was `text.primary`, so "Ghost" rendered in the same white as
    /// "Default" beside it while the Carbon reference draws it blue. The
    /// underline is not added here: Carbon's ghost is not underlined, and
    /// the button's own fill-and-shadow-less shape is its second channel.
    #[test]
    fn a_ghost_button_is_written_in_the_link_ink() {
        assert_eq!(
            token(&ghost_button("g", "Cancel"), "background"),
            Some(SURFACE_BASE)
        );
        let ink = |node: &ViewNode| -> Option<String> {
            node.children
                .iter()
                .find(|c| c.key.as_str() == "g-label")?
                .props
                .tokens
                .get("foreground")
                .map(|t| t.as_str().to_owned())
        };
        assert_eq!(
            ink(&ghost_button("g", "Cancel")).as_deref(),
            Some(LINK_PRIMARY)
        );
        assert_eq!(
            ink(&tertiary_button("g", "Cancel")).as_deref(),
            Some(TEXT_PRIMARY),
            "only ghost takes the link ink; a tertiary button has an edge"
        );
        assert_eq!(
            ink(&danger_ghost_button("g", "Delete")).as_deref(),
            Some(TEXT_PRIMARY),
            "Carbon wants `$button-danger-secondary` here and this library              has no such ink; `link-primary` on a danger control would say              the opposite of what it means"
        );
    }
}
