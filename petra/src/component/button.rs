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
//! # Danger without a red
//!
//! `tokens.rs` has no `$support-error`. Painting [`ACCENT_PRIMARY`] as
//! danger would be a blue lie. The three danger constructors keep Carbon's
//! *structure* (filled / outlined / ghost) with the tokens those shapes
//! already spend, and write `Semantics.value` so the kind is not colour
//! alone. Status-red waits for a support-error name in `tokens.rs`.
//!
//! # A filled button sits *on* the surface
//!
//! Filled variants ([`button`], [`primary_button`], [`danger_button`])
//! cast [`SHADOW_RAISED`] and draw no outline. Ghost and tertiary do not
//! shout a fill; tertiary (and danger-tertiary) take [`BORDER_SUBTLE`].
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
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, ICON_DISABLED, ICON_ON_COLOR_DISABLED, LAYER_ACTIVE,
    LAYER_HOVER, SHADOW_RAISED, SHAPE_MD, SIZE_MD, SPACING_05, SURFACE_BASE, SURFACE_RAISED,
    TEXT_ON_ACCENT, TEXT_PRIMARY, t,
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
        Variant::Secondary | Variant::Danger => Chrome {
            background: SURFACE_RAISED,
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: None,
            shadow: Some(SHADOW_RAISED),
        },
        Variant::Tertiary | Variant::DangerTertiary => Chrome {
            background: SURFACE_BASE,
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: Some(BORDER_SUBTLE),
            shadow: None,
        },
        Variant::Ghost | Variant::DangerGhost => Chrome {
            background: SURFACE_BASE,
            foreground: TEXT_PRIMARY,
            hover: Some(LAYER_HOVER),
            active: Some(LAYER_ACTIVE),
            disabled_ink: ICON_DISABLED,
            border: None,
            shadow: None,
        },
    }
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

/// Carbon tertiary: no fill shouting, [`BORDER_SUBTLE`] edge, no shadow.
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

    let mut node = stack(key, Axis::Horizontal, None, vec![inner]);
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
        ACCENT_PRIMARY, BORDER_SUBTLE, HEIGHT_LG, HEIGHT_SM, HEIGHT_XS, PAD_END_NO_ICON,
        SHADOW_RAISED, SIZE_MD, SPACING_05, SURFACE_BASE, TEXT_ON_ACCENT, button, button_lg,
        button_sm, button_xs, danger_button, danger_ghost_button, danger_tertiary_button,
        ghost_button, primary_button, tertiary_button,
    };
    use crate::frame::{TransitionActivity, Viewport, petrify};
    use crate::geom::{Align, Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ThemeMode, TokenName, standard_vocabulary};
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
        assert_eq!(token(&tertiary, "border"), Some(BORDER_SUBTLE));
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
        assert_eq!(token(&danger, "shadow"), Some(SHADOW_RAISED));

        let dt = danger_tertiary_button("dt", "Delete");
        assert_eq!(dt.semantics.value.as_deref(), Some("danger-tertiary"));
        assert_eq!(token(&dt, "border"), Some(BORDER_SUBTLE));
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
}
