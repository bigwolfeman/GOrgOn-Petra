//! The token names the component library binds, gathered in one place.
//!
//! FR-052's vocabulary sits on top of [`crate::token::shipped::standard_vocabulary`],
//! and the two are decided by different people at different times: the ramp
//! is the design system's call, the component signatures are this module's.
//! Hardcoding a literal like `"spacing-03"` at each of a dozen call sites
//! means a ramp rename becomes a sweep; naming it once here and having every
//! component read the constant means it becomes a one-line edit — which
//! matters concretely in this build, because the ramp was still being
//! extended in a sibling leaf while this module was written.
//!
//! [`ALL`] is also the fixture [`super::tests::every_bound_token_is_in_the_standard_vocabulary`]
//! walks (gate C1-7): the test proves the constants below are real tokens,
//! not that any particular component call site used one, so it keeps
//! covering the whole library as components are added.

use crate::token::TokenName;

/// The gaps the component library spends, on Carbon's ordinal ramp.
///
/// These moved off the `spacing.2xs … spacing.lg` t-shirt names on
/// 2026-08-25, when the ramp grew from eight steps to Carbon's thirteen and
/// the t-shirt names became documented aliases
/// (`token::shipped::SPACING_ALIASES`). **No gap changed size** — the eight
/// old values are Carbon's `spacing-01 … spacing-09` verbatim, so this is a
/// rename of the name and not of the number.
///
/// This module is the reason that migration was a five-line edit rather than
/// a sweep, which is the claim its own doc comment above makes and this is
/// the first change to test it.
pub(crate) const SPACING_01: &str = "spacing-01";
/// See [`SPACING_01`]. 4 units.
pub(crate) const SPACING_02: &str = "spacing-02";
/// See [`SPACING_01`]. 8 units.
pub(crate) const SPACING_03: &str = "spacing-03";
/// See [`SPACING_01`]. 12 units.
pub(crate) const SPACING_04: &str = "spacing-04";
/// See [`SPACING_01`]. 16 units.
pub(crate) const SPACING_05: &str = "spacing-05";
/// See [`SPACING_01`]. 24 units. Carbon's structured-list cell bottom
/// padding (`padding-td`, slice-e) and the ordered list's marker column.
pub(crate) const SPACING_06: &str = "spacing-06";
/// See [`SPACING_01`]. 32 units. Carbon nested-list indent (`_list.scss`).
pub(crate) const SPACING_07: &str = "spacing-07";
/// See [`SPACING_01`]. 48 units. Carbon modal content bottom padding and
/// the header's inline-end reserve for the close control (`_modal.scss`).
pub(crate) const SPACING_09: &str = "spacing-09";

/// Control height `size-md`. Numeric because `Constraints` stay extents
/// (FR-053): the token ramp owns the number, this constant is how a
/// component cites it without spelling `40.0` at the call site.
pub(crate) const SIZE_MD: f32 = 40.0;

pub(crate) const SHAPE_NONE: &str = "shape.corner-none";
/// Carbon's popover corner (`$popover-border-radius`, 2px) — the one
/// rounded corner a floating surface has. Carbon's fields, list boxes and
/// menus are square.
pub(crate) const SHAPE_XS: &str = "shape.corner-xs";
pub(crate) const SHAPE_SM: &str = "shape.corner-sm";
pub(crate) const SHAPE_MD: &str = "shape.corner-md";
pub(crate) const SHAPE_FULL: &str = "shape.corner-full";

pub(crate) const SILHOUETTE_RECT: &str = "shape.silhouette-rect";
pub(crate) const SILHOUETTE_TRIANGLE: &str = "shape.silhouette-triangle";
pub(crate) const SILHOUETTE_DIAMOND: &str = "shape.silhouette-diamond";
pub(crate) const SILHOUETTE_OCTAGON: &str = "shape.silhouette-octagon";

pub(crate) const TYPOGRAPHY_BODY: &str = "typography.body";
pub(crate) const TYPOGRAPHY_HEADING: &str = "typography.heading";
/// Carbon `heading-compact-01`: a selected tab keeps the body size and steps
/// the weight, so it must not borrow the 20px page-heading role.
pub(crate) const TYPOGRAPHY_HEADING_SM: &str = "typography.heading-sm";
/// Carbon `body-compact-01` (14/18): the text of a field value, a list box
/// option and a menu item. [`TYPOGRAPHY_BODY`] is `body-01` (14/20), the
/// paragraph step; a 40-unit row built on it reads two units looser than
/// Carbon's.
pub(crate) const TYPOGRAPHY_BODY_COMPACT: &str = "typography.body-compact";
/// Carbon `label-01` (12/16): the label above a field.
pub(crate) const TYPOGRAPHY_LABEL: &str = "typography.label";
/// The one MONO step in the ramp (`shipped.rs`: 12/16, mono). Code is code:
/// a snippet set in the sans body face is not a code snippet, and columns
/// that do not line up are the first thing a reader notices.
pub(crate) const TYPOGRAPHY_CODE: &str = "typography.code";

/// The elevation a resting container casts. Colour only: the offset, blur
/// and spread live in `token::SHADOW_GEOMETRY`, because they are the same in
/// both themes and a theme is the wrong place to keep a fact that does not
/// change with the lights.
pub(crate) const SHADOW_RAISED: &str = "shadow.raised";

/// The deeper elevation a floating surface casts — a menu, a calendar, a
/// popover. Present in the shipped vocabulary since it was written; only
/// the re-export was missing, which is the fourth token found that way and
/// the reason the module doc now says to check `token/shipped.rs` first.
/// The elevation a surface that has left the page casts: a popover, a
/// tooltip, a list box. Carbon gives every floating panel one shadow
/// (`0 2px 6px 0 rgba(0,0,0,.2)`, slice-b, slice-c) and gives a resting
/// container none, which is the difference between these two names.
/// `token::shipped` pins `shadow.overlay` to fall further and soften more
/// than [`SHADOW_RAISED`].
pub(crate) const SHADOW_OVERLAY: &str = "shadow.overlay";

/// The scrim a modal lays over the page behind it: black at a theme-chosen
/// alpha, so the page dims rather than recolours. Bound as `background` on
/// a window-covering surface, never composited into another node's rect
/// (`crate::token::shipped::SCRIM_TOKEN`).
pub(crate) const OVERLAY_SCRIM: &str = "overlay.scrim";

pub(crate) const SURFACE_BASE: &str = "surface.base";
pub(crate) const SURFACE_RAISED: &str = "surface.raised";
/// The top of the layer ramp (`token::shipped::LAYER_TOKENS[3]`, `#444444`
/// dark). A floating note that must separate itself from *whatever* it was
/// dragged over -- the page at `#121212` or a card at `#222222` -- and
/// cannot know which, so it takes the one tone that is a step away from
/// both.
pub(crate) const SURFACE_LAYER_THREE: &str = "surface.layer-three";

/// A recessed fill: one selected-magnitude step off the layer it sits on.
/// Carbon's own `$layer-accent-01`.
///
/// **This exists because `border.subtle` cannot be both a hairline and a
/// filled rail.** Carbon spends `$border-subtle` on both — slice-d:122 gives
/// the progress track `background-color: $border-subtle` — and gets away with
/// it because Carbon's g100 `$border-subtle` is `#393939`. Petra's dark layers
/// *ramp* rather than alternate, so a hairline at that tone would vanish
/// against `surface.layer-three` (`#444444`), and `token::shipped`'s
/// `DARK_BORDER` is deliberately raised to `#9c9c9c` to hold 3.55:1 there.
///
/// That is the right call for a one-unit line and the wrong one for an
/// eight-unit trough: filled with `#9c9c9c` a progress track reads as a second
/// bar in a different colour, so a thirty-percent bar looks like two bars
/// rather than one partly filled. Measured on the rasterized page, 2026-09-04:
/// fill `#4589ff` — byte-identical to Carbon's — against a track 99 sRGB
/// levels lighter than Carbon's.
///
/// So copying Carbon's token *name* was the mistake, and it was invisible
/// while only the name was checked. Where an area is filled to recede, spend
/// this.
pub(crate) const LAYER_ACCENT: &str = "layer-accent";
/// [`LAYER_ACCENT`] under the pointer: Carbon's `$layer-accent-hover`, the
/// data table's column-header hover (slice-b, "Column header" colour table).
pub(crate) const LAYER_ACCENT_HOVER: &str = "layer-accent-hover";

/// The error hue. Carbon's `$support-error`, which `token::shipped` aliases
/// onto `status.down`.
///
/// **The third token found missing that was never missing.** Like
/// [`LAYER_ACCENT`] and unlike a real gap, the name has always been in
/// `standard_vocabulary` — `SUPPORT_ALIASES` registers it and every theme
/// assigns it. Only this re-export was absent, so a component author reading
/// `tokens.rs` concluded the library had no error colour and reached for
/// [`ACCENT_PRIMARY`] instead. That is how `field_invalid` came to draw its
/// border in the same blue a focused field uses, which made the invalid state
/// and the focused state pixel-identical.
///
/// **It is never the only channel.** The operator is red-green colour blind,
/// so a red edge alone is not a signal he can rely on; `field_invalid` pairs
/// it with the word "Invalid" in the helper text, and Carbon additionally puts
/// an error glyph in the field. `IconMark` has no warning glyph yet, so that
/// third channel is still owed.
pub(crate) const SUPPORT_ERROR: &str = "support-error";
pub(crate) const TEXT_PRIMARY: &str = "text.primary";
pub(crate) const TEXT_MUTED: &str = "text.muted";

/// The inverted polarity's ground and the ink that reads on it: Carbon's
/// `$layer-selected-inverse` and `$text-inverse`. Spent by
/// [`super::content_switcher_item`]'s selected tab, which Carbon's default
/// (high-contrast) switcher fills with the *other* theme's surface so the
/// selected tab is a luminance step nobody can miss, colour-blind or not.
///
/// Both names were in `token::shipped` before this re-export existed; the
/// content switcher's module doc used to say the inverse family was not
/// shipped, and that was true only of this file.
pub(crate) const LAYER_SELECTED_INVERSE: &str = "layer-selected-inverse";
/// See [`LAYER_SELECTED_INVERSE`].
pub(crate) const TEXT_INVERSE: &str = "text-inverse";
/// The tone a non-text glyph is drawn in when it is the control's own mark
/// on a layer ground: Carbon's `$icon-primary`, the fill SCSS names for the
/// accordion arrow, the list-box chevron, the snippet's copy, the number
/// input's steppers, the date picker's calendar and the tag's close. Spent
/// through [`super::IconTone::Primary`].
pub(crate) const ICON_PRIMARY: &str = "icon-primary";

/// The tone a non-text glyph is drawn in when it is furniture rather than
/// a message: Carbon's `$icon-secondary`, spent by the disclosure caret
/// ([`super::caret`]) that a tree branch and an expandable tile carry, and
/// through [`super::IconTone::Secondary`] by the search magnifier and a
/// resting header action.
pub(crate) const ICON_SECONDARY: &str = "icon-secondary";

/// The **decorative** rule: a divider between two rows, a panel edge, a
/// line under a header. Carbon's `$border-subtle`.
///
/// Every border in this library bound [`TEXT_MUTED`] until 2026-08-25 —
/// 10.73:1 against the card it was drawn on, as loud as the prose inside
/// it, because there was no border colour in the theme and a text one had
/// been conscripted. Most of those borders are now gone entirely.
///
/// # The 2026-09-05 split, which is the reason to read this
///
/// What survived the 2026-08-25 pass was **one** border tone doing two
/// jobs, held to WCAG SC 1.4.11's 3:1 because one of those jobs is a
/// checkbox outline. That floor set the tone of every table rule, every
/// divider and every panel edge on 42 pages, and it is the single largest
/// reason the catalog read as "not Carbon": dark's rule measured `#9c9c9c`
/// against Carbon's `#393939`, 99 sRGB levels too bright.
///
/// SC 1.4.11 covers information that identifies a *component*. A hairline
/// between two table rows identifies none. So the roles are two names now:
///
/// | | binds | floor |
/// |---|---|---|
/// | divider, row rule, panel edge | this | Carbon's own 1.3:1 |
/// | checkbox, radio, toggle track, tertiary button, drop zone, field rule | [`BORDER_STRONG`] | SC 1.4.11's 3:1 |
///
/// `component::tests::containers_take_a_tone_and_controls_take_an_edge`
/// names every node that draws an edge **and which of the two it must
/// bind**, so a new edge on the wrong side of the split fails a test rather
/// than shipping. `crate::token::shipped`'s `BORDER_TOKEN` carries the
/// measurement table and derives both tones from the layer set.
pub(crate) const BORDER_SUBTLE: &str = "border.subtle";

/// The **control boundary**: the edge that identifies an interactive
/// element, and the only border tone in this library held to WCAG SC
/// 1.4.11's 3:1.
///
/// Two shapes bind it. The first is Carbon's one-unit field rule —
/// `border-block-end: 1px solid $border-strong` on a text input, a select
/// and a list box field (slice-e, slice-b); a field's boundary in Carbon is
/// that rule and nothing else, no box and no radius. The second is every
/// control whose outline *is* the control: an unchecked checkbox, an
/// unselected radio, a toggle track, a tertiary button, the upload drop
/// zone (Carbon: `border: 1px dashed $border-strong`, MEASURED
/// `_file-uploader.scss:425`). Those four moved here from [`BORDER_SUBTLE`]
/// on 2026-09-05 — see that constant for the split and why it happened.
pub(crate) const BORDER_STRONG: &str = "border-strong";

/// Carbon's `$button-danger-primary` (`#da1e28`, the same in all four
/// published themes): the fill under a destructive button.
///
/// Spent by `super::danger_button` and nothing else. `SUPPORT_ERROR` cannot
/// do this job — no ink in this library clears the 4.5:1 AA floor on it,
/// best 4.41:1 — which is the whole reason for a second red. See
/// `crate::token::shipped`'s `DANGER_FILL_TOKEN` for the measurements and
/// the colour-blindness gate that lets a new hue into the palette at all.
pub(crate) const BUTTON_DANGER_PRIMARY: &str = "button-danger-primary";

/// Carbon's `$text-on-color` / `$icon-on-color`: **white in every theme**,
/// the ink and the mark that ride a saturated fill.
///
/// **Not [`TEXT_ON_ACCENT`], and the difference is visible.** That one is
/// the theme's own `surface.base`, so in dark it is `#121212`, which is
/// correct on the accent (white on `#4589ff` is 3.35:1, under AA) and wrong
/// everywhere else. Binding it for the toggle handle is what drew row 36's
/// on-toggle with a black knob where Carbon's is white.
pub(crate) const TEXT_ON_COLOR: &str = "text-on-color";

/// The accent fill, and the ink that goes on top of it.
///
/// Spent by [`super::primary_button`] and nothing else in this library. See
/// `crate::token::shipped`'s own comments for why the hue is blue (it is the
/// axis red-green colour blindness does not collapse) and why the pair is
/// two names rather than one (neither shipped text tone clears AA on it).
pub(crate) const ACCENT_PRIMARY: &str = "accent.primary";
/// See [`ACCENT_PRIMARY`].
pub(crate) const TEXT_ON_ACCENT: &str = "text.on-accent";

/// The interaction-state surfaces, bound through the decorated-key channel
/// `crate::token::state` resolves (`background@hover`, `background@selected`,
/// `background@selected-hover`).
///
/// Four names and not one composite rule, because Carbon **names the
/// combination**: `layer-selected-hover` is its own entry in the theme, not
/// `layer-hover` laid over `layer-selected`. A library that composited them
/// would produce a selected-hover tone no designer ever chose, and would land
/// on a different one in light and dark.
pub(crate) const LAYER_HOVER: &str = "layer-hover";
/// See [`LAYER_HOVER`]. The pressed surface: one full layer past selected, so
/// a press is legible even on a row that was already selected.
pub(crate) const LAYER_ACTIVE: &str = "layer-active";
/// See [`LAYER_HOVER`].
pub(crate) const LAYER_SELECTED: &str = "layer-selected";
/// See [`LAYER_HOVER`].
pub(crate) const LAYER_SELECTED_HOVER: &str = "layer-selected-hover";

/// The ink a disabled control's label is drawn in: the primary text tone at
/// 25% alpha, which is how Carbon expresses every disabled ink.
///
/// **This is one of two channels, never the only one.** FR-010 forbids
/// conveying disabled by colour — or by opacity — alone, and the painter
/// drops a disabled node's elevation for exactly that reason
/// (`gorgon_petra_egui::paint`). A faded label says "unavailable" to a reader
/// who can see the fade; a control lying flat beside two that are lifted says
/// it to everyone.
pub(crate) const ICON_DISABLED: &str = "icon-disabled";

/// See [`ICON_DISABLED`]. The same idea on an accent fill: the *on-colour*
/// ink at 25%, because a disabled label on a blue button faded toward the
/// page's ink would be a different hue as well as a different lightness.
pub(crate) const ICON_ON_COLOR_DISABLED: &str = "icon-on-color-disabled";

/// Carbon's `$button-disabled`: the track a disabled toggle paints, on and
/// off alike (`.cds--toggle--disabled .cds--toggle__switch`).
///
/// **Not [`ICON_DISABLED`].** That name is an existing live tone faded to
/// 25%; this one is a colour Carbon publishes as its own value
/// (`_button-tokens.scss`), an opaque mid-grey in White/G10 and a
/// translucent one in G90/G100 — measured, not derived by fading
/// `icon-primary` or the accent. Reusing `icon-disabled` here would be one
/// name doing two jobs, the exact thing `component::controls`'s own module
/// doc argues against for `border-strong`.
pub(crate) const BUTTON_DISABLED: &str = "button-disabled";

/// The ink a link is written in: Carbon's `$link-primary`, derived in
/// `token::shipped` from the accent so it clears AA as *text* on every layer
/// (the accent itself does not, on dark's deepest layers). Spent by
/// [`super::link`] on the label and on the `underline` slot, so the hue and
/// the rule are one colour.
pub(crate) const LINK_PRIMARY: &str = "link-primary";

/// Every constant above, for the completeness proof. A name added above and
/// left out of this list would silently stop being covered, so the list is
/// what the test walks rather than the component source.
///
/// `#[cfg(test)]`: nothing in the shipped library reads this back, only the
/// completeness test below does, so a non-test build correctly reports it
/// unused without this — the gate is a test's job, not runtime code's.
#[cfg(test)]
pub(crate) const ALL: &[&str] = &[
    SPACING_01,
    SPACING_02,
    SPACING_03,
    SPACING_04,
    SPACING_05,
    SPACING_07,
    SPACING_09,
    SHAPE_NONE,
    SHAPE_XS,
    SHAPE_SM,
    SHAPE_MD,
    SHAPE_FULL,
    SILHOUETTE_RECT,
    SILHOUETTE_TRIANGLE,
    SILHOUETTE_DIAMOND,
    SILHOUETTE_OCTAGON,
    TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING,
    TYPOGRAPHY_HEADING_SM,
    SHADOW_OVERLAY,
    TYPOGRAPHY_BODY_COMPACT,
    TYPOGRAPHY_LABEL,
    SHADOW_RAISED,
    SHADOW_OVERLAY,
    OVERLAY_SCRIM,
    SURFACE_BASE,
    SURFACE_RAISED,
    LAYER_ACCENT,
    SUPPORT_ERROR,
    TEXT_PRIMARY,
    TEXT_MUTED,
    LAYER_SELECTED_INVERSE,
    TEXT_INVERSE,
    ICON_PRIMARY,
    ICON_SECONDARY,
    BORDER_SUBTLE,
    BORDER_STRONG,
    ACCENT_PRIMARY,
    TEXT_ON_ACCENT,
    BUTTON_DANGER_PRIMARY,
    TEXT_ON_COLOR,
    LAYER_HOVER,
    LAYER_ACTIVE,
    LAYER_SELECTED,
    LAYER_SELECTED_HOVER,
    ICON_DISABLED,
    ICON_ON_COLOR_DISABLED,
    BORDER_STRONG,
    BUTTON_DISABLED,
    LINK_PRIMARY,
];

/// `name` as a [`TokenName`].
///
/// # Panics
/// Never, for any constant declared in this file — each one is checked
/// well-formed by `every_constant_is_a_well_formed_token_name` below. A
/// panic here means a constant above was mistyped, not that a caller passed
/// something bad: every call site in `component/` passes one of the named
/// constants, never a computed or author-supplied string (gate C1-8).
pub(crate) fn t(name: &str) -> TokenName {
    TokenName::new(name).unwrap_or_else(|err| panic!("component token name {name:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{ALL, t};
    use crate::token::standard_vocabulary;

    #[test]
    fn every_constant_is_a_well_formed_token_name() {
        for name in ALL {
            let _ = t(name);
        }
    }

    /// Gate C1-7: every token the component library binds is declared in
    /// the shipped vocabulary. Proved by walking [`ALL`] — the list the
    /// components themselves are built from — against a freshly built
    /// `standard_vocabulary()`, so this keeps being true no matter what the
    /// vocabulary looks like by the time this runs.
    #[test]
    fn every_bound_token_is_in_the_standard_vocabulary() {
        let vocab = standard_vocabulary();
        for name in ALL {
            let token = t(name);
            assert!(
                vocab.contains(&token),
                "component token `{name}` is not declared in standard_vocabulary()"
            );
        }
    }
}
