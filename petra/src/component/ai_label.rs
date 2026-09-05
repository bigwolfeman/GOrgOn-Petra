//! Carbon AI label (slice-a, inventory row 2). "AI label" is the current
//! Carbon name; "Slug" is the old one kept for back-compat classes
//! (`_ai-label.scss` is an empty shim over `_slug.scss`, per slice-a) — this
//! module uses the current name throughout.
//!
//! Anatomy (docs "Anatomy" + SCSS class walk, slice-a):
//! 1. root (flex container) — `.cds--ai-label`.
//! 2. [`trigger_button`] — the AI indicator/button, the popover trigger.
//! 3. `"text"` — the localized `"AI"` glyph, inside the trigger. Only the
//!    English string ships here; the `AI`/`IA`/`KI`/`ИИ` localisation table
//!    (docs "Content") is not implemented.
//! 4. `"panel"` (present only while `open`) — the explainability popover,
//!    built on [`super::popover::popover_with`] (the shared anchored
//!    surface both Popover and Toggletip already compose on).
//! 5. `"actions"` (present only in [`ai_label_with_actions`]) — a footer
//!    row of caller-built buttons, 48px tall.
//!
//! # Variant
//! **Default** ([`ai_label`] and its sized siblings): a bordered
//! icon-button trigger. **Inline** ([`ai_label_inline`] and its sized
//! siblings): a small trigger that sits inline with text, with a leading
//! bullet dot instead of a border.
//!
//! # Sizes — two independent, discrete ramps (contract §4)
//! Default (`_slug.scss` `$sizes`, square glyph): [`ai_label_mini`] 16,
//! [`ai_label_2xs`] 20, [`ai_label_xs`] 24, [`ai_label_sm`] 32, [`ai_label`]
//! 40 (md, default), [`ai_label_lg`] 48, [`ai_label_xl`] 64.
//! Inline (paired with 12/14/16px body text this component does not set —
//! that pairing belongs to the surrounding prose, not to the trigger):
//! [`ai_label_inline_sm`] 16, [`ai_label_inline`] 18 (md, default),
//! [`ai_label_inline_lg`] 22.
//!
//! # States
//! Enabled, hover, focus. The usage page is explicit: "AI label has three
//! interactive states: enabled, hover, and focus. The AI label should
//! never be disabled" — so no `_disabled` sibling exists here, and
//! [`super::disabled`] should not be composed onto anything this module
//! returns. **Revert** ([`ai_label_revert`]) is the one state unique to
//! this component: Carbon swaps the whole trigger for an icon-only "Undo"
//! button (`.cds--ai-label--revert`), a bare curved arrow with no box at
//! all — measured off `02-ai-label.png`, a 24x21 device-pixel glyph run
//! with no border anywhere near it. [`icon`](super::icon) ships no `Undo`
//! mark (this file does not own `icon.rs`), and FR-026/FR-058 forbid an
//! icon-only control regardless, so the swapped trigger here carries the
//! visible word `"Undo"` instead of a mark — a labelled control, never an
//! icon alone — in a box that is Carbon's height and its label's width.
//!
//! # T070 (SCSS wins)
//! Slice-a's `SYNTHESIS.md` §5 eight-item docs-vs-SCSS list does not name
//! AI label. The one number here sourced from SCSS rather than the style
//! page is the popover radius (8px, `_slug.scss:330`, [`SHAPE_MD`]) —
//! slice-a marks it MEASURED, not SOURCED from prose, so there is no docs
//! figure to record beside it.
//!
//! # What this module does NOT build, and why
//! * **Gradient background/border.** `ai-popover-gradient()`,
//!   `$ai-aura-start/end`, `$ai-border-start/end` are theme tokens
//!   slice-a's own "Unverified" section could not resolve to hex, and
//!   `component::tokens` has no gradient slot at all (every fill here is a
//!   flat token). The panel fill is [`SURFACE_RAISED`] — [`popover_with`]'s
//!   own choice, reused rather than duplicated.
//! * **`border-inverse`.** Not a name in `component::tokens`. The trigger's
//!   1px edge binds [`BORDER_SUBTLE`] instead, the same stand-in
//!   [`super::tag`] and [`super::toggletip`] already use for a Carbon edge
//!   colour this library has no token for.
//! * **The mini/2xs invisible 24×24 click-target extension** (SCSS
//!   `::after`, slice-a "Key numbers"). Petra's hit-test area is a node's
//!   own box; there is no primitive for a hit-target larger than the drawn
//!   box, and adding one is outside a single component file.
//! * **`:has()`-driven auto-open and dismiss-on-outside-click wiring.**
//!   `open` is a caller-supplied fact here, exactly as it is for
//!   [`super::popover`] and [`super::toggletip`] — this file does not own
//!   the driver loop that would flip it.
//! * **A distinct popover title slot / `heading-lg` typography.** Slice-a's
//!   "Key numbers" documents a 28px title inside the explainability panel,
//!   but the anatomy list itself has no numbered title part, and neither
//!   [`super::popover`] nor [`super::toggletip`] carries one either — this
//!   stays consistent with both siblings rather than inventing a part they
//!   don't have.
//! * **Popover auto-orientation.** Reused as-is from [`popover_with`]
//!   ([`ClampRule::Flip`](crate::tree::ClampRule::Flip)), not reimplemented.

use super::pad;
use super::popover::popover_with;
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, SHAPE_FULL, SHAPE_MD, SIZE_MD, SPACING_02, SPACING_03, SPACING_05,
    SURFACE_BASE, TEXT_PRIMARY, TYPOGRAPHY_LABEL, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Justify, Key, Role, TextWrap, ViewNode,
};

/// Carbon default-variant `mini`.
const DEFAULT_MINI: f32 = 16.0;
/// Carbon default-variant `2xs`.
const DEFAULT_2XS: f32 = 20.0;
/// Carbon default-variant `xs`.
const DEFAULT_XS: f32 = 24.0;
/// Carbon default-variant `sm`.
const DEFAULT_SM: f32 = 32.0;
/// Carbon default-variant `lg`.
const DEFAULT_LG: f32 = 48.0;
/// Carbon default-variant `xl`.
const DEFAULT_XL: f32 = 64.0;

/// Carbon inline-variant `sm`.
const INLINE_SM: f32 = 16.0;
/// Carbon inline-variant `md` (default).
const INLINE_MD: f32 = 18.0;
/// Carbon inline-variant `lg`.
const INLINE_LG: f32 = 22.0;

/// Inline leading bullet at `sm`/`md`.
const BULLET_SMALL: f32 = 4.0;
/// Inline leading bullet at `lg`.
const BULLET_LARGE: f32 = 8.0;

/// Explainability popover footer height ("Explainability popover
/// structure", style page).
const ACTIONS_FOOTER_HEIGHT: f32 = 48.0;

/// Carbon `$spacing-06` (24 units): the explainability popover's container
/// padding. Not one of [`super::tokens`]'s names — accordion.rs hit the
/// same gap for its panel bottom inset — named locally per the house rule
/// that a local `const` may name any token already in the shipped
/// vocabulary (`token::shipped::SPACING_SCALE` carries `spacing-06`).
const SPACING_06: &str = "spacing-06";

/// Carbon `body-02` (16/22), the type the **inline lg** trigger is paired
/// with. Not one of [`super::tokens`]'s re-exports — named locally under the
/// same house rule as [`SPACING_06`], because `token::shipped`'s type ramp
/// carries `typography.body-lg`.
const TYPOGRAPHY_BODY_LG: &str = "typography.body-lg";

/// The default-variant size at and below which the `"AI"` glyph steps down
/// to [`TYPOGRAPHY_LABEL`].
///
/// Carbon's `$sizes` map scales the box and the docs do not record what it
/// does to `.cds--ai-label__text`; slice-a's "Key numbers" has no per-size
/// font pairing for the default variant, so there is no Carbon figure to
/// copy here. What there is, is a measurement: at `mini` (16), body type
/// (14/20) puts the `"AI"` run 2.5 points off the left border and 1.5 off
/// the right, sitting on the bottom rule — read off `02-ai-label.png` on
/// 2026-09-05, glyph run 591..610 device pixels inside a well of 586..613.
/// [`TYPOGRAPHY_LABEL`] is Carbon `label-01` (12/16) and is the smallest
/// step this library ships, so it is what the two small boxes get. The
/// **inline** ramp needs no such judgement: slice-a SOURCES its pairing
/// outright (16/18/22 boxes against 12/14/16 text).
const SMALL_GLYPH_BELOW: f32 = DEFAULT_XS;

const _: () = assert!(DEFAULT_MINI == 16.0);
const _: () = assert!(DEFAULT_2XS == 20.0);
const _: () = assert!(DEFAULT_XS == 24.0);
const _: () = assert!(DEFAULT_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(DEFAULT_LG == 48.0);
const _: () = assert!(DEFAULT_XL == 64.0);
const _: () = assert!(INLINE_SM == 16.0);
const _: () = assert!(INLINE_MD == 18.0);
const _: () = assert!(INLINE_LG == 22.0);
const _: () = assert!(BULLET_SMALL == 4.0);
const _: () = assert!(BULLET_LARGE == 8.0);
const _: () = assert!(ACTIONS_FOOTER_HEIGHT == 48.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Default-variant AI label at Carbon `md` (40px, [`SIZE_MD`]). `label` is
/// required (FR-058): it names both the trigger and the popover it opens.
/// `open` shows or hides the explainability panel; `body` is its text.
pub fn ai_label(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, SIZE_MD)
}

/// [`ai_label`] at Carbon `mini` (16px).
pub fn ai_label_mini(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_MINI)
}

/// [`ai_label`] at Carbon `2xs` (20px).
pub fn ai_label_2xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_2XS)
}

/// [`ai_label`] at Carbon `xs` (24px).
pub fn ai_label_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_XS)
}

/// [`ai_label`] at Carbon `sm` (32px).
pub fn ai_label_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_SM)
}

/// [`ai_label`] at Carbon `lg` (48px).
pub fn ai_label_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_LG)
}

/// [`ai_label`] at Carbon `xl` (64px).
pub fn ai_label_xl(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    default_sized(key, label, open, body, DEFAULT_XL)
}

/// [`ai_label`] whose panel also hosts a 48px footer row of caller-built
/// `actions` (anatomy part 5, "with actions" variant). Each action keeps
/// whatever role and label it already carries (FR-058 stays on them, the
/// same rule [`popover_with`] documents for its own children).
pub fn ai_label_with_actions(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
    actions: Vec<ViewNode>,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger_button("trigger", label.clone(), SIZE_MD);
    let mut children = vec![trigger];
    if open {
        let mut footer = stack("actions", Axis::Horizontal, Some(SPACING_03), actions);
        footer.props.align = Some(Align::Center);
        footer.constraints.vertical = AxisConstraint {
            min: Some(ACTIONS_FOOTER_HEIGHT),
            max: Some(ACTIONS_FOOTER_HEIGHT),
            priority: 0,
        };
        let rows = vec![text("body", body.into()), footer];
        children.push(explainability_panel(label.clone(), "trigger", rows));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// Revert-state trigger (`.cds--ai-label--revert`): the whole trigger
/// swapped for a labelled "Undo" control. No popover — reverting is a
/// terminal action, not a fresh trigger for the explainability panel.
///
/// # Why this one is not square
///
/// Carbon's revert is an icon-only `Undo` glyph in a 40 box, and
/// [`icon`](super::icon) ships no `Undo` mark (this file does not own
/// `icon.rs`), so what stands in its place is the **word**. Pinned square
/// at 40 the word had 3 points of air on its left and 2.5 on its right —
/// measured off `02-ai-label.png` on 2026-09-05, glyph run 592..656 device
/// pixels inside a well of 586..662 — and the operator's report on the row
/// was *"it looks bad whatever it is"*. A word crammed edge to edge inside
/// a box built for a glyph is what that looks like.
///
/// So this control keeps Carbon's **height** (40, [`SIZE_MD`], the same
/// pin every other trigger has) and takes its width from its label plus
/// [`SPACING_05`] on each side, exactly as [`super::button`] does. The
/// floor stays at 40 so it is never *narrower* than the square it replaces.
/// It is a labelled button that happens to be the revert control, which is
/// what FR-026/FR-058 leave available until an `Undo` mark exists.
pub fn ai_label_revert(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut node = stack(key, Axis::Horizontal, None, centered_caption("Undo", None));
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    // A container, not a leaf, so `Props.padding` is legal here
    // (`Violation::PaddingOnLeafKind`); the caption inside it is the leaf.
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(revert_box())
        .interactive(Role::Button, label, TRIGGER_INTENTS)
}

/// [`ai_label_revert`]'s box: Carbon's 40 height pinned, width free above a
/// 40 floor.
///
/// The mirror image of [`square`], and the two docs belong together: a
/// trigger holding a two-glyph mark must not be allowed to stretch, and a
/// control holding a word must not be forbidden to fit it.
fn revert_box() -> Constraints {
    Constraints {
        horizontal: AxisConstraint {
            min: Some(SIZE_MD),
            max: None,
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(SIZE_MD),
            max: Some(SIZE_MD),
            priority: 0,
        },
    }
}

/// Inline-variant AI label at Carbon `md` (18px, default). Leading bullet
/// dot instead of a border (slice-a "Inline structure").
pub fn ai_label_inline(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    inline_sized(key, label, open, body, INLINE_MD, BULLET_SMALL)
}

/// [`ai_label_inline`] at Carbon `sm` (16px).
pub fn ai_label_inline_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    inline_sized(key, label, open, body, INLINE_SM, BULLET_SMALL)
}

/// [`ai_label_inline`] at Carbon `lg` (22px). Carbon steps the bullet to
/// 8px here (sm/md share the 4px bullet).
pub fn ai_label_inline_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
) -> ViewNode {
    inline_sized(key, label, open, body, INLINE_LG, BULLET_LARGE)
}

fn default_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
    size: f32,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger_button("trigger", label.clone(), size);
    let mut children = vec![trigger];
    if open {
        let rows = vec![text("body", body.into())];
        children.push(explainability_panel(label.clone(), "trigger", rows));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn inline_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    body: impl Into<String>,
    height: f32,
    bullet: f32,
) -> ViewNode {
    let label = label.into();
    let trigger = inline_trigger("trigger", label.clone(), height, bullet);
    let mut children = vec![trigger];
    if open {
        let rows = vec![text("body", body.into())];
        children.push(explainability_panel(label.clone(), "trigger", rows));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// The bordered square icon-button trigger (anatomy parts 2+3: button
/// wrapping the `"AI"` text). `border-inverse` has no token; see the
/// module doc for why [`BORDER_SUBTLE`] stands in.
fn trigger_button(key: impl Into<Key>, label: String, size: f32) -> ViewNode {
    let style = (size < SMALL_GLYPH_BELOW).then_some(TYPOGRAPHY_LABEL);
    let mut node = stack(key, Axis::Horizontal, None, centered_caption("AI", style));
    node.props.align = Some(Align::Center);
    node.props.justify = Some(Justify::Center);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.with_constraints(square(size))
        .interactive(Role::Button, label, TRIGGER_INTENTS)
}

/// A short caption, centred on *both* axes inside a box the caller pins
/// square (`trigger_button`, [`ai_label_revert`]) and marks `align: Center`
/// (cross axis) plus `justify: Center` (main axis) — see
/// [`crate::tree::Props::justify`]'s doc.
///
/// Previously this built a `[Spacer, caption, Spacer]` triple and gave the
/// caption `AxisConstraint::priority = 1` to win the whole row in
/// `stack::distribute`'s first group, because `Align::Center` on the row
/// only ever resolved the cross axis and the main axis had no such knob.
/// `Props::justify` is that knob now: a lone child, centred on both axes,
/// with no spacer pair and no priority trick.
fn centered_caption(label: &str, style: Option<&str>) -> Vec<ViewNode> {
    let mut caption = text("text", label);
    // Clip, not the default Wrap: a two-word caption ("AI", "Undo") has
    // nothing to prove by wrapping, and this is the policy the label
    // actually wants — never break onto a second line.
    caption.props.wrap = Some(TextWrap::Clip);
    if let Some(style) = style {
        caption.props.style = Some(t(style));
    }
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    vec![caption]
}

/// The inline trigger: leading bullet dot, then the `"AI"` text, no border
/// (slice-a "Inline structure": 4px left padding on both the button and
/// the text, bullet 4px sm/md or 8px lg).
fn inline_trigger(key: impl Into<Key>, label: String, height: f32, bullet: f32) -> ViewNode {
    let dot = bullet_dot("dot", bullet);
    let mut caption = text("text", "AI");
    // Slice-a SOURCES the pairing outright: the 16/18/22 inline boxes go
    // with 12/14/16 text. `text()` already binds the middle one, so only
    // the ends are named here.
    if let Some(style) = inline_type(height) {
        caption.props.style = Some(t(style));
    }
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    // The 4px gap between the bullet and the text is spacing between two
    // siblings, not an inset carried by either — `Props.padding` only
    // applies to container kinds (`Violation::PaddingOnLeafKind`; see
    // `button.rs`'s `labelled` for the same rule), and `"text"` here is a
    // leaf with nothing to inset. `stack`'s own `spacing` prop is exactly
    // the "space between this row's children" primitive, so it carries the
    // same 4px Carbon draws with `margin-left` on the text, without putting
    // padding anywhere a leaf would have to own it.
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_02), vec![dot, caption]);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_02)),
        right: Some(t(SPACING_02)),
        ..InsetRefs::default()
    });
    // Resting fill matches the page it sits on ([`SURFACE_BASE`]), the same
    // fix `accordion.rs`'s header and this file's own `trigger_button`
    // already carry: an unbound `background` beside `background@hover` is
    // exactly the "declares content, resolves to nothing" shape the paint
    // accounting counts as silent, not empty (Accordion/Modal's crash cause).
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let vertical = AxisConstraint {
        min: Some(height),
        max: Some(height),
        priority: 0,
    };
    node.with_constraints(Constraints {
        vertical,
        ..Constraints::default()
    })
    .interactive(Role::Button, label, TRIGGER_INTENTS)
}

/// The type step Carbon pairs with an inline trigger of `height`
/// (slice-a "Sizes": sm 16 with 12px text, md 18 with 14px, lg 22 with
/// 16px). `None` is the md step, which [`super::text::text`] already binds.
fn inline_type(height: f32) -> Option<&'static str> {
    if height <= INLINE_SM {
        Some(TYPOGRAPHY_LABEL)
    } else if height >= INLINE_LG {
        Some(TYPOGRAPHY_BODY_LG)
    } else {
        None
    }
}

/// The leading bullet: a filled, fully-rounded square. Decorative — it
/// carries no status of its own (FR-026 governs an icon standing in for a
/// status signal; this is a fixed visual accent always paired with the
/// `"AI"` text right beside it), so [`BORDER_SUBTLE`] is enough ink,
/// matching [`super::accordion`]'s divider — a fixed-extent stack, not a
/// spacer.
fn bullet_dot(key: impl Into<Key>, size: f32) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    node.props.tokens.insert("radius".into(), t(SHAPE_FULL));
    node.constraints = square(size);
    node
}

/// The explainability popover (anatomy part 4): [`popover_with`]'s shell,
/// re-seated to this component's own key numbers — 8px radius
/// ([`SHAPE_MD`], MEASURED SCSS) and 24px container padding
/// ([`SPACING_06`]) in place of the generic popover's 16px
/// ([`super::tokens::SPACING_05`]).
fn explainability_panel(label: String, anchor: impl Into<Key>, rows: Vec<ViewNode>) -> ViewNode {
    let mut panel = popover_with("panel", label, anchor, rows);
    panel.props.tokens.insert("radius".into(), t(SHAPE_MD));
    panel.props.padding = Some(pad(SPACING_06, SPACING_06));
    panel
}

/// Carbon's square glyph box, pinned on BOTH axes.
///
/// This is the one place the "keep the Carbon number as `min`, drop `max`"
/// fix used by [`super::modal`]'s close button, [`super::number_input`]'s
/// steppers and [`super::ui_shell`]'s header actions is the WRONG fix, and
/// it was tried and reverted rather than reasoned about: with `max: None`
/// the trigger stopped being a button and stretched to the full width of
/// whatever row held it, caption flush against the far edge. The picture is
/// in this repo's history; the box went from 40px to ~460px.
///
/// Those three are controls inside a row that sizes to its own content. A
/// trigger is a lone child of a stretching container, so an unbounded
/// inline axis is an invitation, not a floor.
///
/// This is the box for the **glyph** triggers, the ones holding `"AI"`.
/// [`ai_label_revert`] holds a word and takes [`revert_box`] instead; its
/// doc has the measurement that separated the two.
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
        ACTIONS_FOOTER_HEIGHT, BULLET_LARGE, BULLET_SMALL, DEFAULT_2XS, DEFAULT_LG, DEFAULT_MINI,
        DEFAULT_SM, DEFAULT_XL, DEFAULT_XS, INLINE_LG, INLINE_MD, INLINE_SM, ai_label,
        ai_label_2xs, ai_label_inline, ai_label_inline_lg, ai_label_inline_sm, ai_label_lg,
        ai_label_mini, ai_label_revert, ai_label_sm, ai_label_with_actions, ai_label_xl,
        ai_label_xs,
    };
    use crate::component::tokens::{BORDER_SUBTLE, SHAPE_FULL, SHAPE_MD, SIZE_MD, SPACING_05};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn has_canvas(node: &ViewNode) -> bool {
        node.kind == NodeKind::Canvas || node.children.iter().any(|c| has_canvas(c))
    }

    #[test]
    fn ai_label_closed_is_a_labelled_trigger_at_default_md_size() {
        let node = ai_label("conf", "Confidence score", false, "80% confident");
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1, "closed: no panel child");

        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("Confidence score"));
        assert!(trigger.interactions.contains(&Interaction::Focus));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert!(trigger.interactions.contains(&Interaction::Hover));
        assert_eq!(trigger.constraints.horizontal.min, Some(SIZE_MD));
        assert_eq!(
            trigger.constraints.horizontal.max,
            Some(SIZE_MD),
            "pinned on purpose, and the one box in this workspace where \
             dropping `max` is wrong: see `square`'s own doc for the picture \
             that proved it"
        );
        assert_eq!(trigger.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(trigger, "border"), Some(BORDER_SUBTLE));
        assert_eq!(child(trigger, "text").props.text.as_deref(), Some("AI"));

        assert!(!node.semantics.disabled, "AI label is never disabled");
    }

    #[test]
    fn ai_label_open_hosts_an_explainability_popover() {
        let node = ai_label(
            "conf",
            "Confidence score",
            true,
            "Trained on ticket history.",
        );
        assert_eq!(node.semantics.expanded, Some(true));

        let panel = child(&node, "panel");
        assert_eq!(panel.kind, NodeKind::Surface);
        assert_eq!(panel.semantics.role, Some(Role::Overlay));
        assert_eq!(panel.semantics.label.as_deref(), Some("Confidence score"));
        match &panel.props.anchor {
            Some(crate::tree::Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected a sibling-anchored panel, got {other:?}"),
        }
        assert_eq!(token(panel, "radius"), Some(SHAPE_MD));
        let pad = panel.props.padding.as_ref().expect("panel has padding");
        assert_eq!(pad.left.as_ref().unwrap().as_str(), "spacing-06");
        assert_eq!(pad.top.as_ref().unwrap().as_str(), "spacing-06");

        let content = child(panel, "content");
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Trained on ticket history.")
        );
    }

    #[test]
    fn ai_label_default_size_ramp_matches_carbon() {
        fn size_of(node: &ViewNode) -> (Option<f32>, Option<f32>) {
            let trigger = child(node, "trigger");
            (
                trigger.constraints.horizontal.min,
                trigger.constraints.vertical.min,
            )
        }
        assert_eq!(
            size_of(&ai_label_mini("a", "Ask AI", false, "x")),
            (Some(DEFAULT_MINI), Some(DEFAULT_MINI))
        );
        assert_eq!(
            size_of(&ai_label_2xs("a", "Ask AI", false, "x")),
            (Some(DEFAULT_2XS), Some(DEFAULT_2XS))
        );
        assert_eq!(
            size_of(&ai_label_xs("a", "Ask AI", false, "x")),
            (Some(DEFAULT_XS), Some(DEFAULT_XS))
        );
        assert_eq!(
            size_of(&ai_label_sm("a", "Ask AI", false, "x")),
            (Some(DEFAULT_SM), Some(DEFAULT_SM))
        );
        assert_eq!(
            size_of(&ai_label_lg("a", "Ask AI", false, "x")),
            (Some(DEFAULT_LG), Some(DEFAULT_LG))
        );
        assert_eq!(
            size_of(&ai_label_xl("a", "Ask AI", false, "x")),
            (Some(DEFAULT_XL), Some(DEFAULT_XL))
        );
        assert_eq!(DEFAULT_MINI, 16.0);
        assert_eq!(DEFAULT_2XS, 20.0);
        assert_eq!(DEFAULT_XS, 24.0);
        assert_eq!(DEFAULT_SM, 32.0);
        assert_eq!(DEFAULT_LG, 48.0);
        assert_eq!(DEFAULT_XL, 64.0);
    }

    #[test]
    fn ai_label_inline_variant_has_a_leading_bullet_and_its_own_size_ramp() {
        let md = ai_label_inline("a", "Ask AI", false, "x");
        let trigger = child(&md, "trigger");
        assert_eq!(trigger.constraints.vertical.min, Some(INLINE_MD));
        assert_eq!(INLINE_MD, 18.0);
        let dot = child(trigger, "dot");
        assert_eq!(dot.constraints.horizontal.min, Some(BULLET_SMALL));
        assert_eq!(token(dot, "radius"), Some(SHAPE_FULL));
        assert!(
            token(trigger, "border").is_none(),
            "inline trigger has a bullet, not a border"
        );
        assert_eq!(
            token(trigger, "background@hover"),
            Some(crate::component::tokens::LAYER_HOVER)
        );
        assert_eq!(
            token(trigger, "background"),
            Some(crate::component::tokens::SURFACE_BASE),
            "a resting `background` must be bound alongside `background@hover`, \
             or the trigger paints nothing when it is not hovered -- the paint \
             pass counts that as silent, not empty"
        );

        let sm = ai_label_inline_sm("a", "Ask AI", false, "x");
        assert_eq!(
            child(&sm, "trigger").constraints.vertical.min,
            Some(INLINE_SM)
        );
        assert_eq!(INLINE_SM, 16.0);

        let lg = ai_label_inline_lg("a", "Ask AI", false, "x");
        let lg_trigger = child(&lg, "trigger");
        assert_eq!(lg_trigger.constraints.vertical.min, Some(INLINE_LG));
        assert_eq!(INLINE_LG, 22.0);
        assert_eq!(
            child(lg_trigger, "dot").constraints.horizontal.min,
            Some(BULLET_LARGE)
        );
        assert_eq!(BULLET_LARGE, 8.0);
    }

    #[test]
    fn ai_label_revert_is_a_labelled_button_never_icon_only() {
        let node = ai_label_revert("conf", "Revert to AI suggestion");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(
            node.semantics.label.as_deref(),
            Some("Revert to AI suggestion")
        );
        assert!(node.interactions.contains(&Interaction::Click));
        assert_eq!(child(&node, "text").props.text.as_deref(), Some("Undo"));
        assert!(
            !has_canvas(&node),
            "revert is the word Undo, never an icon-only mark"
        );
        assert!(!node.semantics.disabled);
    }

    /// The `"AI"` glyph steps down with the box it sits in.
    ///
    /// The inline half is Carbon's own pairing (slice-a "Sizes": 16/18/22
    /// boxes against 12/14/16 text). The default half is a measurement, not
    /// a Carbon figure — see [`super::SMALL_GLYPH_BELOW`]'s doc — and it is
    /// asserted here so the judgement is visible rather than buried.
    ///
    /// Falsify by dropping the `style` argument from `centered_caption`:
    /// mini and 2xs go back to body type and the glyph sits on its own
    /// bottom rule.
    #[test]
    fn the_ai_glyph_steps_down_with_the_box_it_sits_in() {
        fn style(node: &ViewNode) -> Option<&str> {
            named(node, "text").props.style.as_ref().map(|s| s.as_str())
        }
        const LABEL: &str = "typography.label";
        const BODY: &str = "typography.body";
        const BODY_LG: &str = "typography.body-lg";

        assert_eq!(
            style(&ai_label_mini("a", "Ask AI", false, "x")),
            Some(LABEL)
        );
        assert_eq!(style(&ai_label_2xs("a", "Ask AI", false, "x")), Some(LABEL));
        assert_eq!(style(&ai_label_xs("a", "Ask AI", false, "x")), Some(BODY));
        assert_eq!(style(&ai_label_sm("a", "Ask AI", false, "x")), Some(BODY));
        assert_eq!(style(&ai_label("a", "Ask AI", false, "x")), Some(BODY));
        assert_eq!(style(&ai_label_lg("a", "Ask AI", false, "x")), Some(BODY));
        assert_eq!(style(&ai_label_xl("a", "Ask AI", false, "x")), Some(BODY));

        assert_eq!(
            style(&ai_label_inline_sm("a", "Ask AI", false, "x")),
            Some(LABEL)
        );
        assert_eq!(
            style(&ai_label_inline("a", "Ask AI", false, "x")),
            Some(BODY)
        );
        assert_eq!(
            style(&ai_label_inline_lg("a", "Ask AI", false, "x")),
            Some(BODY_LG)
        );

        // The revert control holds a word, not a glyph, and keeps body type.
        assert_eq!(style(&ai_label_revert("a", "Undo AI edit")), Some(BODY));
    }

    /// The revert control keeps Carbon's height and gets its width from
    /// the word it has to hold.
    ///
    /// Pinned square at 40 the word `"Undo"` had 3 points of air on one
    /// side and 2.5 on the other (measured off `02-ai-label.png`,
    /// 2026-09-05), which is the "looks bad" half of the operator's report
    /// on this row. Falsify by putting `square(SIZE_MD)` back on it: the
    /// `max` and the padding assertions both fail, and the picture goes
    /// back to a word wedged against two borders.
    #[test]
    fn the_revert_control_is_as_tall_as_carbon_and_as_wide_as_its_word() {
        let node = ai_label_revert("conf", "Revert to AI suggestion");
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(
            node.constraints.vertical.max,
            Some(SIZE_MD),
            "Carbon's revert is 40 tall like every other trigger"
        );
        assert_eq!(
            node.constraints.horizontal.min,
            Some(SIZE_MD),
            "never narrower than the square it replaces"
        );
        assert_eq!(
            node.constraints.horizontal.max, None,
            "a word needs room; pinning the inline axis is what crammed it"
        );
        let pad = node
            .props
            .padding
            .as_ref()
            .expect("revert has inline padding");
        assert_eq!(pad.left.as_ref().map(|n| n.as_str()), Some(SPACING_05));
        assert_eq!(pad.right.as_ref().map(|n| n.as_str()), Some(SPACING_05));
        assert!(
            pad.top.is_none() && pad.bottom.is_none(),
            "height is the pinned constraint, as it is on `button`"
        );

        // The glyph triggers are unchanged and stay pinned on both axes.
        let trigger = child(&ai_label("a", "Ask AI", false, "x"), "trigger").clone();
        assert_eq!(trigger.constraints.horizontal.max, Some(SIZE_MD));
    }

    #[test]
    fn ai_label_never_declares_disabled_or_read_only() {
        for node in [
            ai_label("a", "Ask AI", true, "x"),
            ai_label_inline("a", "Ask AI", true, "x"),
            ai_label_revert("a", "Undo AI edit"),
        ] {
            assert!(!node.semantics.disabled);
            assert!(!node.semantics.read_only);
        }
    }

    #[test]
    fn ai_label_with_actions_places_a_48px_footer_row_inside_the_panel() {
        let cancel = ViewNode::new(NodeKind::Stack, "cancel").interactive(
            Role::Button,
            "Keep suggestion",
            &[Interaction::Click],
        );
        let node = ai_label_with_actions(
            "conf",
            "Confidence score",
            true,
            "Trained on ticket history.",
            vec![cancel],
        );
        let footer = named(&node, "actions");
        assert_eq!(footer.constraints.vertical.min, Some(ACTIONS_FOOTER_HEIGHT));
        assert_eq!(footer.constraints.vertical.max, Some(ACTIONS_FOOTER_HEIGHT));
        assert_eq!(ACTIONS_FOOTER_HEIGHT, 48.0);
        assert_eq!(
            child(footer, "cancel").semantics.label.as_deref(),
            Some("Keep suggestion")
        );
        // body still present alongside the footer
        let panel = child(&node, "panel");
        let content = child(panel, "content");
        assert_eq!(
            child(content, "body").props.text.as_deref(),
            Some("Trained on ticket history.")
        );
    }

    #[test]
    fn ai_label_does_not_invent_gradient_or_literal_colour_tokens() {
        fn walk_tokens(node: &ViewNode, check: &impl Fn(&str)) {
            for name in node.props.tokens.values() {
                check(name.as_str());
            }
            for c in &node.children {
                walk_tokens(c, check);
            }
        }
        let node = ai_label("conf", "Confidence score", true, "80% confident");
        walk_tokens(&node, &|name| {
            assert!(
                !name.contains("gradient") && !name.contains("aura") && !name.starts_with('#'),
                "found an invented colour/gradient token: {name}"
            );
        });
    }

    /// Every public constructor must produce a tree `crate::tree::validate`
    /// accepts. The eight tests above only assert on the constructed
    /// `ViewNode` directly — roles, labels, tokens, constraints — and never
    /// call `validate`, so a structurally invalid tree (e.g. padding
    /// declared on a leaf) could pass every one of them while still being
    /// refused the moment a real host tried to mount it. This is exactly
    /// how `ai_label_inline`'s bad leaf padding shipped unnoticed.
    ///
    /// Both states are built: the closed trigger (where the padding defect
    /// lived) and the open one, whose explainability panel names the
    /// trigger by bare sibling key (`Anchor::Sibling`) and so validates
    /// standalone as well as at depth
    /// (`ai_label_open_validates_when_mounted_at_catalog_depth`).
    #[test]
    fn every_constructor_produces_a_tree_validate_accepts() {
        let trees: Vec<(&str, ViewNode)> = vec![
            ("ai_label", ai_label("a", "Ask AI", false, "x")),
            ("ai_label open", ai_label("a", "Ask AI", true, "x")),
            (
                "ai_label_mini open",
                ai_label_mini("a", "Ask AI", true, "x"),
            ),
            ("ai_label_2xs open", ai_label_2xs("a", "Ask AI", true, "x")),
            ("ai_label_xs open", ai_label_xs("a", "Ask AI", true, "x")),
            ("ai_label_sm open", ai_label_sm("a", "Ask AI", true, "x")),
            ("ai_label_lg open", ai_label_lg("a", "Ask AI", true, "x")),
            ("ai_label_xl open", ai_label_xl("a", "Ask AI", true, "x")),
            (
                "ai_label_with_actions open",
                ai_label_with_actions("a", "Ask AI", true, "x", vec![]),
            ),
            (
                "ai_label_inline open",
                ai_label_inline("a", "Ask AI", true, "x"),
            ),
            (
                "ai_label_inline_sm open",
                ai_label_inline_sm("a", "Ask AI", true, "x"),
            ),
            (
                "ai_label_inline_lg open",
                ai_label_inline_lg("a", "Ask AI", true, "x"),
            ),
            ("ai_label_mini", ai_label_mini("a", "Ask AI", false, "x")),
            ("ai_label_2xs", ai_label_2xs("a", "Ask AI", false, "x")),
            ("ai_label_xs", ai_label_xs("a", "Ask AI", false, "x")),
            ("ai_label_sm", ai_label_sm("a", "Ask AI", false, "x")),
            ("ai_label_lg", ai_label_lg("a", "Ask AI", false, "x")),
            ("ai_label_xl", ai_label_xl("a", "Ask AI", false, "x")),
            (
                "ai_label_with_actions",
                ai_label_with_actions("a", "Ask AI", false, "x", vec![]),
            ),
            ("ai_label_revert", ai_label_revert("a", "Undo AI edit")),
            (
                "ai_label_inline",
                ai_label_inline("a", "Ask AI", false, "x"),
            ),
            (
                "ai_label_inline_sm",
                ai_label_inline_sm("a", "Ask AI", false, "x"),
            ),
            (
                "ai_label_inline_lg",
                ai_label_inline_lg("a", "Ask AI", false, "x"),
            ),
        ];

        for (name, node) in &trees {
            eprintln!("validating {name}");
            let _ = validated(node);
        }
    }

    /// The open form's `panel` names its `trigger` sibling by bare key
    /// (`Anchor::Sibling`), so it is accepted wherever a caller mounts it —
    /// here two containers below the root, the gallery catalog's own depth.
    #[test]
    fn ai_label_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "ai_label open",
            vec![
                ai_label(
                    "conf",
                    "Confidence score",
                    true,
                    "Trained on ticket history.",
                ),
                ai_label_inline("inline", "Ask AI", true, "Trained on ticket history."),
            ],
        );
    }

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(node: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(node);
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

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D across the closed forms of every size ramp and every
    /// variant (default, inline, revert): no degenerate rect, no child
    /// outside its parent. The open forms are not here because a surface
    /// child is placed against the window rather than inside its parent's
    /// rect, so the "inside its parent" half of this check does not apply
    /// to them; `every_constructor_produces_a_tree_validate_accepts` and
    /// `ai_label_open_validates_when_mounted_at_catalog_depth` cover their
    /// acceptance, and the gallery photographs them open.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("mini", ai_label_mini("a", "Ask AI", false, "x")),
            ("2xs", ai_label_2xs("a", "Ask AI", false, "x")),
            ("xs", ai_label_xs("a", "Ask AI", false, "x")),
            ("sm", ai_label_sm("a", "Ask AI", false, "x")),
            ("md", ai_label("a", "Ask AI", false, "x")),
            ("lg", ai_label_lg("a", "Ask AI", false, "x")),
            ("xl", ai_label_xl("a", "Ask AI", false, "x")),
            ("inline-sm", ai_label_inline_sm("a", "Ask AI", false, "x")),
            ("inline-md", ai_label_inline("a", "Ask AI", false, "x")),
            ("inline-lg", ai_label_inline_lg("a", "Ask AI", false, "x")),
            ("revert", ai_label_revert("a", "Revert to AI suggestion")),
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

    /// Check F: every trigger declares `Focus` and must be reachable;
    /// AI label is never disabled (module doc), so there is no negative
    /// case here.
    #[test]
    fn every_trigger_is_reachable_in_focus_order() {
        for (label, node) in [
            ("default", ai_label("a", "Ask AI", false, "x")),
            ("inline", ai_label_inline("a", "Ask AI", false, "x")),
            ("revert", ai_label_revert("a", "Undo AI edit")),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let trigger_id = frame
                .placements
                .iter()
                .find(|p| p.semantics.role == Some(Role::Button))
                .unwrap_or_else(|| panic!("{label}: no Button placement"))
                .id
                .clone();
            assert!(
                focus.order().iter().any(|id| id == &trigger_id),
                "{label}: trigger declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: the `"AI"` / `"Undo"` glyph and the inline `"AI"` text
    /// against each trigger's own resting fill, in both themes, read
    /// through `Props.opacity`.
    #[test]
    fn trigger_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("default", ai_label("a", "Ask AI", false, "x")),
                // The inline trigger's own resting `background` is the fix
                // under test elsewhere in this file; read it from the node
                // rather than assume it, so this measures what is bound.
                ("inline", ai_label_inline("a", "Ask AI", false, "x")),
                ("revert", ai_label_revert("a", "Undo AI edit")),
            ] {
                // `revert`'s trigger *is* the returned node (keyed by the
                // caller's key, not "trigger"); `default`/`inline` nest a
                // child keyed "trigger". `named` finds either: the caller's
                // node for `revert`, the descendant for the others.
                let trigger = if node.key.as_str() == "trigger" || label == "revert" {
                    &node
                } else {
                    named(&node, "trigger")
                };
                let bg_name = trigger
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: trigger has no resting background"));
                let bg = color(&theme, bg_name.as_str());
                let text_node = named(trigger, "text");
                let fg_name = text_node
                    .props
                    .tokens
                    .get("foreground")
                    .expect("text binds a foreground");
                let opacity = text_node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: trigger text at {ratio:.2}:1 against {bg_name} fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
