//! Carbon Menu buttons (slice-c). Buttons that open a Menu.
//!
//! Anatomy (`_menu-button.scss`, a thin composition over `button` +
//! `menu`): trigger button + caret + Menu. Slice-c documents three trigger
//! shapes and all three are here: Menu button (single button + caret,
//! [`menu_button`]), Combo button (split trigger, [`combo_button`]), and
//! Overflow menu (icon-only, [`overflow_menu`]).
//!
//! The trigger is Carbon's **primary** button (`kind="primary"` is the
//! default): [`ACCENT_PRIMARY`] fill, [`TEXT_ON_ACCENT`] label at the
//! leading edge, the chevron at the trailing edge, `min-inline-size` 160 —
//! the same 160 as the menu's own minimum, which is why Carbon's reference
//! shot has the two exactly one width. Square corners, as every Carbon
//! button is. Height md 40. No hover fill, for the reason
//! [`super::primary_button`] has none: the vocabulary has no
//! `accent.primary-hover`, and inventing the tone here would put a colour
//! decision in a component.
//!
//! The caret is [`IconMark::ChevronDown`] shut and [`IconMark::ChevronUp`]
//! open (Carbon turns `.cds--menu-button__trigger--open svg` 180°), in
//! [`IconTone::OnAccent`]. Never the only channel: `Semantics.expanded` is
//! declared and the menu is mounted only while open (FR-026).
//!
//! The menu hangs flush under the trigger at its leading edge and is at
//! least the trigger's width ([`super::list_box`]'s `Fit::Anchor`). Until
//! 2026-09-04 it was a centred, padded popover with a beak, and the
//! operator could not tell the page from the Popover page.
//!
//! # Phase 8 (spec 008 T051): the two trigger shapes this adds
//!
//! * **Combo button** ([`combo_button`]) — slice-c's split trigger: the
//!   primary action half beside the chevron toggle at the MEASURED
//!   `column-gap: 1px`, the action at its MEASURED 111/239 bounds
//!   (`_combo-button.scss` lines 39–40) and the toggle half at 48 (the
//!   16-unit glyph in `spacing-05` padding either side). The halves add up
//!   to exactly the family's 160/288 — `111 + 1 + 48 = 10rem` is the same
//!   arithmetic `_combo-button.scss` line 33 writes down — which is why
//!   the menu under the pair is one width with it again. The action acts
//!   ([`Intent::Activate`]); the toggle opens. SCSS's `$triggerSizes` map
//!   carries sm/md/lg only — **no xs** — so this shape has no xs step and
//!   SCSS wins (`component-anatomy.md` §6); the style page's docs table
//!   lists all four steps uniformly and is recorded here as the
//!   disagreement (slice-c flags it "Unverified").
//! * **Overflow menu** ([`overflow_menu`]) — the icon-only ghost trigger:
//!   `--open` swaps its ground to `$layer` to merge with its open menu
//!   (MEASURED `.overflow-menu--open`), and the menu is `overflow-menu-options`
//!   at fixed `$spacing-13` width — 10rem/160 in the crate's own Carbon
//!   ramp (`token/shipped.rs`) — wearing [`Tip::Caret`], the engine's
//!   stand-in for the `::after` connector tail. The tail's *direction*
//!   detection is anchored placement's (spec 005); the part exists, its
//!   flip is the engine's business. The trigger glyph is Carbon's
//!   `OverflowMenuVertical` (a kebab); this icon set ships no kebab mark,
//!   so it reuses [`IconMark::Menu`] — the closest shape available and the
//!   substitution [`super::data_table::data_table_row_menu_trigger`] made
//!   first and recorded. Never the only channel: `label` is the name.

use super::icon::{IconMark, IconTone, icon_toned};
use super::list_box::{ListBoxSize, edge_row};
use super::menu::{MAX_INLINE, MIN_INLINE, menu_at};
use super::stack;
use super::swatch;
use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BUTTON_DISABLED, LAYER_ACTIVE, LAYER_HOVER, SIZE_MD, SURFACE_BASE,
    SURFACE_RAISED, TEXT_MUTED, TEXT_ON_ACCENT, TYPOGRAPHY_BODY_COMPACT, t,
};
use crate::geom::{Align as CrossAlign, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, Fit, FocusFigure, Intent, Interaction, Key, Phase,
    Role, TextWrap, Tip, ViewNode,
};

/// Spec 010: trigger opens/closes the menu.
const TOGGLES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Toggle,
    phase: Phase::OnRelease,
};

/// The trigger's height comes from [`ListBoxSize`]'s ramp (xs 24 / sm 32 /
/// md 40 / lg 48 — the ramp `list_box.rs` asserts), so the wave-1–4
/// placeholder pin (`65c3c43`) that froze this trigger at `SIZE_MD` is
/// gone; only the trigger's md-default assert remains, as a ramp point.
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(MIN_INLINE == 160.0);

const TRIGGER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Spec 010: the combo's primary half is a button's whole of `Activate`.
const ACTIVATES_ON_RELEASE: Behaviour = Behaviour {
    intent: Intent::Activate,
    phase: Phase::OnRelease,
};

/// MEASURED `_combo-button.scss` lines 39–40: the primary action half's
/// `min-inline-size` / `max-inline-size`.
const COMBO_ACTION_MIN: f32 = 111.0;
const COMBO_ACTION_MAX: f32 = 239.0;
/// MEASURED `_combo-button.scss` (the container's `column-gap: 1px`). A
/// literal, not a token: the ramp's smallest name is `spacing-01`
/// (0.125rem/2 units), so no token plays 1px.
const COMBO_GAP: f32 = 1.0;
/// The icon half's width: the 16-unit glyph box in `spacing-05` padding
/// either side (48) — the `triggerWidth` the measured `111 =
/// 10rem - 1px - triggerWidth` (`_combo-button.scss` line 33) leaves for
/// it, at every step.
const TRIGGER_HALF_W: f32 = 48.0;
/// `overflow-menu-options`' fixed width: `$spacing-13`, which is 10rem in
/// the crate's Carbon ramp (`token/shipped.rs`) — the family minimum,
/// fixed.
const OVERFLOW_MENU_W: f32 = 160.0;

const _: () = assert!(COMBO_ACTION_MIN + COMBO_GAP + TRIGGER_HALF_W == MIN_INLINE);
const _: () = assert!(COMBO_ACTION_MAX + COMBO_GAP + TRIGGER_HALF_W == MAX_INLINE);
const _: () = assert!(OVERFLOW_MENU_W == MIN_INLINE);

/// A labelled menu trigger at Carbon md (40). When `open` is true the node
/// also carries a [`super::menu`] list box of `items` at the same step.
/// `label` is required (FR-058).
pub fn menu_button(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_button_at(key, label, open, items, ListBoxSize::Md)
}

/// The trigger at Carbon xs (24), its open menu's rows with it.
pub fn menu_button_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_button_at(key, label, open, items, ListBoxSize::Xs)
}

/// The trigger at Carbon sm (32), its open menu's rows with it.
pub fn menu_button_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_button_at(key, label, open, items, ListBoxSize::Sm)
}

/// The trigger at Carbon lg (48), its open menu's rows with it.
pub fn menu_button_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    menu_button_at(key, label, open, items, ListBoxSize::Lg)
}

/// Slice-c "Sizes": the menu-buttons ramp is "the same four-step ramp as
/// Menu, keyed to the trigger button's size" (xs 24 / sm 32 / md 40 /
/// lg 48), and the open menu's item heights follow the trigger — so the
/// step threads trigger → menu. This replaces the wave-1–4 placeholder
/// pin (`65c3c43`) that froze the trigger at `SIZE_MD` and refused the
/// other three documented steps (FR-026).
fn menu_button_at(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
    size: ListBoxSize,
) -> ViewNode {
    let label = label.into();
    let trigger = trigger("trigger", label.clone(), open, size);
    let mut children = vec![trigger];
    if open {
        children.push(menu_at("menu", label, items, size));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

fn trigger(key: impl Into<Key>, label: String, open: bool, size: ListBoxSize) -> ViewNode {
    let chevron = chevron_glyph(open);
    // Label at the leading edge, chevron at the trailing one, whatever the
    // trigger's width (`edge_row`).
    let mut node = edge_row(key, on_accent_caption(&label), Some(chevron));
    primary_ground(&mut node);
    let mut node = node
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(MIN_INLINE),
                max: None,
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(size.height()),
                max: Some(size.height()),
                priority: 0,
            },
        })
        .interactive(Role::Button, label, TRIGGER_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .owning_its_text()
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.expanded = Some(open);
    node
}

/// The chevron every caret half wears: [`IconMark::ChevronDown`] shut and
/// [`IconMark::ChevronUp`] open (Carbon turns
/// `.cds--menu-button__trigger--open svg` 180°), in [`IconTone::OnAccent`]
/// on the primary fill. Never the only channel: `Semantics.expanded` is
/// declared beside it.
fn chevron_glyph(open: bool) -> ViewNode {
    icon_toned(
        "caret",
        if open {
            IconMark::ChevronUp
        } else {
            IconMark::ChevronDown
        },
        IconTone::OnAccent,
    )
}

/// The on-accent caption a filled half shows: `typography.body-compact`
/// in [`TEXT_ON_ACCENT`], and [`TEXT_MUTED`] under `@disabled` — the label
/// rule's `text.muted` (SC-014), not Carbon's `$text-disabled`, whose 25 %
/// fade on a live accent fill measured 2.00:1 on light (a label nobody can
/// read). The chevron keeps `icon_on-color-disabled`, which is the mark
/// family's fade, not a label's.
fn on_accent_caption(label: &str) -> ViewNode {
    let mut caption = text("label", label.to_owned());
    caption.props.style = Some(t(TYPOGRAPHY_BODY_COMPACT));
    caption.props.wrap = Some(TextWrap::Clip);
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_ON_ACCENT));
    caption
        .props
        .tokens
        .insert("foreground@disabled".into(), t(TEXT_MUTED));
    caption
}

/// The filled family's ground, the two bindings every filled half shares:
/// [`ACCENT_PRIMARY`] at rest, Carbon's `$button-disabled` when disabled,
/// so the muted label above lands on a ground it clears 4.5:1 in (5.70:1
/// light / 5.26:1 dark, `contracts/disabled.md`'s measurements).
fn primary_ground(node: &mut ViewNode) {
    node.props
        .tokens
        .insert("background".into(), t(ACCENT_PRIMARY));
    node.props
        .tokens
        .insert("background@disabled".into(), t(BUTTON_DISABLED));
}

/// The combo's primary half: the default action as a filled button at its
/// MEASURED 111/239 bounds (slice-c: `min-inline-size: 111px`,
/// `max-inline-size: 239px`). One shot on release — a button's whole of
/// [`Intent::Activate`].
fn action_half(key: impl Into<Key>, label: String, height: f32) -> ViewNode {
    let mut node = edge_row(key, on_accent_caption(&label), None);
    primary_ground(&mut node);
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(COMBO_ACTION_MIN),
            max: Some(COMBO_ACTION_MAX),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
    })
    .interactive(Role::Button, label, TRIGGER_INTENTS)
    .with_behaviour(ACTIVATES_ON_RELEASE)
    .owning_its_text()
    .with_focus_figure(FocusFigure::Sides)
}

/// The combo's toggle half: the chevron in the icon half's 48-wide box
/// ([`TRIGGER_HALF_W`]), opening and closing the menu. Same ground, intents
/// and focus figure as [`trigger`]; `Toggle`, not `Activate`.
fn toggle_half(key: impl Into<Key>, label: String, open: bool, height: f32) -> ViewNode {
    let chevron = chevron_glyph(open);
    let mut node = edge_row(key, chevron, None);
    primary_ground(&mut node);
    let mut node = node
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(TRIGGER_HALF_W),
                max: Some(TRIGGER_HALF_W),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(height),
                max: Some(height),
                priority: 0,
            },
        })
        .interactive(Role::Button, label, TRIGGER_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.expanded = Some(open);
    node
}

/// A combo button at Carbon md (40): the primary action half beside the
/// chevron toggle at the measured 1px gap, and — while `open` — the menu
/// under the whole split. `action` is the primary action's own label (the
/// default/most-common action, slice-c); the toggle and the menu share its
/// derived `"{action} options"` name, so the two halves of the control
/// never announce one name for two different acts.
pub fn combo_button(
    key: impl Into<Key>,
    action: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    combo_button_at(key, action, open, items, ListBoxSize::Md)
}

/// The combo at Carbon sm (32). No xs step: SCSS's `$triggerSizes` map has
/// none and SCSS wins — see [`combo_button`].
pub fn combo_button_sm(
    key: impl Into<Key>,
    action: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    combo_button_at(key, action, open, items, ListBoxSize::Sm)
}

/// The combo at Carbon lg (48). No xs step — see [`combo_button`].
pub fn combo_button_lg(
    key: impl Into<Key>,
    action: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    combo_button_at(key, action, open, items, ListBoxSize::Lg)
}

fn combo_button_at(
    key: impl Into<Key>,
    action: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
    size: ListBoxSize,
) -> ViewNode {
    let action = action.into();
    let options = format!("{action} options");
    let height = size.height();
    // The pair is the menu's anchor — `trigger` is the whole split, which
    // is why the open menu lands under both halves at the family width
    // (`Fit::Anchor`) rather than under the chevron alone.
    let mut pair = stack(
        "trigger",
        Axis::Horizontal,
        None,
        vec![
            action_half("action", action, height),
            swatch("gap", COMBO_GAP, height, None, None, None),
            toggle_half("toggle", options.clone(), open, height),
        ],
    );
    pair.props.align = Some(CrossAlign::Center);
    let mut children = vec![pair];
    if open {
        children.push(menu_at("menu", options, items, size));
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// An overflow menu at Carbon md (40): the icon-only ghost trigger over
/// `overflow-menu-options`. `label` is the trigger's required accessible
/// name (FR-058) and the menu's own.
pub fn overflow_menu(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    overflow_menu_at(key, label, open, items, ListBoxSize::Md)
}

/// The overflow menu at Carbon xs (24) — `layout.use('size')` carries the
/// whole ramp, unlike the combo.
pub fn overflow_menu_xs(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    overflow_menu_at(key, label, open, items, ListBoxSize::Xs)
}

/// The overflow menu at Carbon sm (32).
pub fn overflow_menu_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    overflow_menu_at(key, label, open, items, ListBoxSize::Sm)
}

/// The overflow menu at Carbon lg (48).
pub fn overflow_menu_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
) -> ViewNode {
    overflow_menu_at(key, label, open, items, ListBoxSize::Lg)
}

fn overflow_menu_at(
    key: impl Into<Key>,
    label: impl Into<String>,
    open: bool,
    items: Vec<ViewNode>,
    size: ListBoxSize,
) -> ViewNode {
    let label = label.into();
    let mut children = vec![overflow_trigger(
        "trigger",
        label.clone(),
        open,
        size.height(),
    )];
    if open {
        let mut menu = menu_at("menu", label, items, size);
        // `overflow-menu-options` is `$spacing-13` fixed, not fitted to the
        // 48-wide trigger, and it carries the `::after` connector tail —
        // [`Tip::Caret`] is the engine's version of that tail.
        menu.props.fit = Some(Fit::Content);
        menu.props.tip = Some(Tip::Caret);
        menu.constraints.horizontal = AxisConstraint {
            min: Some(OVERFLOW_MENU_W),
            max: Some(OVERFLOW_MENU_W),
            priority: 0,
        };
        children.push(menu);
    }
    let mut node = stack(key, Axis::Vertical, None, children);
    node.semantics.expanded = Some(open);
    node
}

/// The overflow trigger: an icon-only ghost button in the icon half's
/// 48-wide box (slice-c: "Icon button (ghost …)"). Its `--open` state swaps
/// the ground to `$layer` so it merges with its open menu (MEASURED
/// `.overflow-menu--open`); hover and active are the ghost button's own
/// `layer-hover`/`layer-active`, and a disabled one needs no ground swap —
/// the unfilled family binds none (`button.rs`'s `Chrome`) and the glyph's
/// own `@disabled` fade is the mark family's.
fn overflow_trigger(key: impl Into<Key>, label: String, open: bool, height: f32) -> ViewNode {
    // Carbon's glyph is `OverflowMenuVertical` (a kebab); this icon set
    // ships no kebab mark, so this reuses [`IconMark::Menu`] — the recorded
    // substitution [`super::data_table::data_table_row_menu_trigger`] made
    // first. Never the only channel: `label` is the real name.
    let glyph = icon_toned("glyph", IconMark::Menu, IconTone::Primary);
    let mut node = edge_row(key, glyph, None);
    node.props.tokens.insert(
        "background".into(),
        t(if open { SURFACE_RAISED } else { SURFACE_BASE }),
    );
    node.props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    node.props
        .tokens
        .insert("background@active".into(), t(LAYER_ACTIVE));
    let mut node = node
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(TRIGGER_HALF_W),
                max: Some(TRIGGER_HALF_W),
                priority: 0,
            },
            vertical: AxisConstraint {
                min: Some(height),
                max: Some(height),
                priority: 0,
            },
        })
        .interactive(Role::Button, label, TRIGGER_INTENTS)
        .with_behaviour(TOGGLES_ON_RELEASE)
        .with_focus_figure(FocusFigure::Sides);
    node.semantics.expanded = Some(open);
    node
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVATES_ON_RELEASE, COMBO_ACTION_MAX, COMBO_ACTION_MIN, COMBO_GAP, IconMark, IconTone,
        MAX_INLINE, MIN_INLINE, OVERFLOW_MENU_W, SIZE_MD, TOGGLES_ON_RELEASE, TRIGGER_HALF_W,
        chevron_glyph, combo_button, combo_button_lg, icon_toned, menu_button, overflow_menu,
        overflow_menu_xs,
    };
    use crate::component::disabled;
    use crate::component::menu::menu_item;
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{ACCENT_PRIMARY, SURFACE_BASE, SURFACE_RAISED, TEXT_ON_ACCENT};

    use crate::testing::inks;
    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{
        Anchor, Behaviour, Fit, Intent, Interaction, Justify, NodeKind, Phase, Role, Tip, ViewNode,
    };

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn items() -> Vec<ViewNode> {
        vec![menu_item("rename", "Rename")]
    }

    #[test]
    fn menu_button_closed_is_a_labelled_trigger_without_a_menu() {
        let node = menu_button("more", "More", false, items());
        assert_eq!(node.semantics.expanded, Some(false));
        assert_eq!(node.children.len(), 1, "closed trigger has no list box");
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some("More"));
        assert_eq!(trigger.semantics.expanded, Some(false));
        assert_eq!(trigger.constraints.vertical.min, Some(SIZE_MD));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert_eq!(child(trigger, "label").props.text.as_deref(), Some("More"));
        let caret = child(trigger, "caret");
        assert_eq!(caret.kind, NodeKind::Canvas);
        assert_eq!(caret.props.text, None, "the caret is a glyph, not a word");
        assert_eq!(
            caret.props.canvas,
            icon_toned("caret", IconMark::ChevronDown, IconTone::OnAccent)
                .props
                .canvas,
            "a closed trigger points its caret down"
        );
        assert!(
            node.children
                .iter()
                .all(|c| c.semantics.role != Some(Role::Overlay))
        );
    }

    /// The trigger is Carbon's primary button: accent fill, on-accent
    /// label and caret, at least 160 wide, square. Falsify by handing the
    /// trigger `SURFACE_RAISED` and a radius again.
    #[test]
    fn the_trigger_is_a_primary_button_at_least_160_wide() {
        let node = menu_button("more", "More", false, items());
        let trigger = child(&node, "trigger");
        assert_eq!(
            trigger.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        assert_eq!(
            child(trigger, "label")
                .props
                .tokens
                .get("foreground")
                .map(|t| t.as_str()),
            Some(TEXT_ON_ACCENT)
        );
        assert_eq!(trigger.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(MIN_INLINE, 160.0);
        assert!(
            !trigger.props.tokens.contains_key("radius"),
            "Carbon buttons are square"
        );
        assert_eq!(
            trigger.props.justify,
            Some(Justify::SpaceBetween),
            "an `edge_row`: the slack goes between label and caret, so the \
             caret sits at the trailing edge"
        );
    }

    #[test]
    fn menu_button_open_includes_a_flush_menu() {
        let node = menu_button("more", "More", true, items());
        assert_eq!(node.semantics.expanded, Some(true));
        let trigger = child(&node, "trigger");
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.expanded, Some(true));
        assert_eq!(
            child(trigger, "caret").props.canvas,
            icon_toned("caret", IconMark::ChevronUp, IconTone::OnAccent)
                .props
                .canvas,
            "an open trigger points its caret up"
        );

        let menu = child(&node, "menu");
        assert_eq!(menu.kind, NodeKind::Surface);
        assert_eq!(menu.semantics.role, Some(Role::Overlay));
        assert_eq!(menu.semantics.label.as_deref(), Some("More"));
        assert_eq!(menu.props.tip, Some(Tip::Flush), "a menu has no beak");
        match &menu.props.anchor {
            Some(Anchor::Sibling { key, .. }) => assert_eq!(key.as_str(), "trigger"),
            other => panic!("expected Anchor::Sibling, got {other:?}"),
        }
        let content = child(menu, "content");
        assert!(content.children.iter().all(|c| c.key.as_str() != "caret"));
        assert_eq!(
            child(content, "rename").semantics.label.as_deref(),
            Some("Rename")
        );
    }

    /// The open form's `menu` names its `trigger` sibling by bare key
    /// (`Anchor::Sibling`), so it is accepted wherever a caller mounts it —
    /// here two containers below the root, the gallery catalog's own depth.
    #[test]
    fn menu_button_open_validates_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "menu_button open",
            vec![menu_button("more", "More", true, items())],
        );
    }

    // The frame-level checks below audit the CLOSED trigger.

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D: the closed trigger places with a real rect, none of its
    /// parts outside it, and the caret ends where the trigger's inline
    /// padding begins.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
        let frame = petrify_lone(node);
        assert!(!frame.placements.is_empty(), "nothing placed");
        for p in &frame.placements {
            assert!(
                p.rect.w > 0.0 && p.rect.h > 0.0,
                "{} placed with a degenerate rect {:?}",
                p.id,
                p.rect
            );
            assert!(
                !p.paint.overflowed,
                "{} drew content larger than its own rect",
                p.id
            );
            if let Some(parent_idx) = p.parent {
                assert_fits_parent("", p, &frame.placements[parent_idx]);
            }
        }
        let trigger = frame.placement("/root/more/trigger").expect("trigger");
        let caret = frame.placement("/root/more/trigger/caret").expect("caret");
        assert!(
            (trigger.rect.w - MIN_INLINE).abs() < 0.5,
            "a short label leaves the trigger at its 160 minimum: {:?}",
            trigger.rect
        );
        assert!(
            ((trigger.rect.x + trigger.rect.w - 16.0) - (caret.rect.x + caret.rect.w)).abs() < 0.5,
            "the caret's trailing edge is 16 in from the trigger's: trigger {:?}, caret {:?}",
            trigger.rect,
            caret.rect
        );
    }

    /// Check F: an enabled trigger is reachable; a disabled one is not.
    #[test]
    fn the_closed_trigger_is_reachable_unless_disabled() {
        let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let trigger = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the trigger is placed");
        assert!(
            order.iter().any(|o| o == &trigger.id),
            "the trigger declares Focus but is not in focus order"
        );

        let disabled_node = disabled(menu_button(
            "more",
            "More",
            false,
            vec![menu_item("rename", "Rename")],
        ));
        let disabled_frame = petrify_lone(disabled_node);
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_trigger = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/trigger"))
            .expect("the disabled trigger is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_trigger.id),
            "a disabled trigger must not be reachable"
        );
    }

    /// Check E: the trigger's label and caret against the trigger's own
    /// resting fill — the accent — in both themes.
    #[test]
    fn trigger_text_clears_aa_contrast_against_its_own_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = menu_button("more", "More", false, vec![menu_item("rename", "Rename")]);
            let trigger = child(&node, "trigger");
            let bg_name = trigger
                .props
                .tokens
                .get("background")
                .expect("trigger binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            for key in ["label", "caret"] {
                let part = child(trigger, key);
                let inks = inks(part);
                assert!(!inks.is_empty(), "trigger/{key} binds an ink");
                let opacity = part.props.opacity.unwrap_or(1.0);
                for fg_name in inks {
                    let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= MIN_TEXT_CONTRAST,
                        "trigger/{key} at {ratio:.2}:1 against {} fails AA \
                         {MIN_TEXT_CONTRAST}:1",
                        fg_name.as_str()
                    );
                }
            }
        }
    }

    /// Spec 010: the trigger toggles the menu on release.
    ///
    /// Falsified by dropping `.with_behaviour(...)` from the trigger:
    ///
    /// ```text
    /// menu_button trigger declared behaviour None
    /// ```
    #[test]
    fn menu_button_declares_toggle_on_release() {
        let node = menu_button("actions", "Actions", false, Vec::new());
        let trigger = child(&node, "trigger");
        assert_eq!(
            trigger.behaviour,
            Some(Behaviour {
                intent: Intent::Toggle,
                phase: Phase::OnRelease,
            }),
            "menu_button trigger declared behaviour {:?}",
            trigger.behaviour
        );
    }

    fn three_items() -> Vec<ViewNode> {
        vec![
            menu_item("rename", "Rename"),
            menu_item("duplicate", "Duplicate"),
            menu_item("delete", "Delete"),
        ]
    }

    /// Spec 008 T051, the combo button: slice-c's split trigger — the
    /// action half beside the chevron toggle at the MEASURED 1px gap, and
    /// the three columns sum to the menu family's own 160/288.
    #[test]
    fn the_combo_is_a_primary_action_beside_a_toggle_at_one_pixel() {
        let node = combo_button("combo", "Delete", false, three_items());
        let trigger = child(&node, "trigger");
        assert_eq!(
            trigger.kind,
            NodeKind::Stack,
            "carbon `--custom-trigger` is a flex PAIR"
        );
        assert_eq!(
            trigger.props.axis,
            Some(crate::geom::Axis::Horizontal),
            "the split is a row: the action beside the chevron"
        );
        let action = child(trigger, "action");
        let gap = child(trigger, "gap");
        let toggle = child(trigger, "toggle");
        assert_eq!(
            action.semantics.role,
            Some(Role::Button),
            "carbon `md--menu-button__trigger[0]`: the action half is the trigger button"
        );
        assert_eq!(
            (
                action.constraints.horizontal.min,
                action.constraints.horizontal.max
            ),
            (Some(COMBO_ACTION_MIN), Some(COMBO_ACTION_MAX)),
            "carbon `_combo-button.scss` lines 39–40: `min-inline-size: 111px`, \
             `max-inline-size: 239px`"
        );
        assert_eq!(
            gap.kind,
            NodeKind::Spacer,
            "carbon `--custom-trigger-geometry {{ column-gap: $spacing-05 }}`: the 1px gap"
        );
        assert_eq!(
            (
                gap.constraints.horizontal.min,
                gap.constraints.horizontal.max
            ),
            (Some(COMBO_GAP), Some(COMBO_GAP)),
            "$spacing-05 is 1px — the measured gap, not a guess"
        );
        assert_eq!(
            (
                toggle.constraints.horizontal.min,
                toggle.constraints.horizontal.max
            ),
            (Some(TRIGGER_HALF_W), Some(TRIGGER_HALF_W)),
            "carbon `md--menu-button__trigger[1] {{ flex: 0 0 48px }}` — $spacing-13, \
             the 16-unit glyph in $spacing-05 padding either side"
        );
        assert_eq!(
            COMBO_ACTION_MIN + COMBO_GAP + TRIGGER_HALF_W,
            MIN_INLINE,
            "111 + 1 + 48 = 10rem is the family's floor (`_combo-button.scss` line 33's arithmetic)"
        );
        assert_eq!(
            COMBO_ACTION_MAX + COMBO_GAP + TRIGGER_HALF_W,
            MAX_INLINE,
            "239 + 1 + 48 = 18rem is the family's ceiling"
        );
        assert_eq!(
            action.props.tokens.get("background").map(|s| s.as_str()),
            Some(ACCENT_PRIMARY),
            "carbon `--btn--primary`: the action half is the button-colored action"
        );
        assert_eq!(
            toggle.props.tokens.get("background").map(|s| s.as_str()),
            Some(ACCENT_PRIMARY),
            "carbon `.menu-button__trigger:hover` repaints the WHOLE `--custom-trigger`, halves alike"
        );
    }

    /// The halves mean two different acts — the action fires once, the
    /// toggle opens the menu — and the caret's state is never the only
    /// channel for open/closed (FR-026).
    #[test]
    fn the_action_acts_while_the_toggle_opens_and_says_so_by_glyph() {
        let node = combo_button("combo", "Delete", false, three_items());
        let trigger = child(&node, "trigger");
        assert_eq!(
            child(trigger, "action").behaviour,
            Some(ACTIVATES_ON_RELEASE),
            "carbon `md--menu-button__trigger[0]` is a button: one shot on release"
        );
        assert_eq!(
            child(trigger, "toggle").behaviour,
            Some(TOGGLES_ON_RELEASE),
            "carbon `md--menu-button__trigger[1]` opens/closes: spec 010's toggle-on-release rule"
        );
        for open in [false, true] {
            let node = combo_button("combo", "Delete", open, three_items());
            let toggle = child(child(&node, "trigger"), "toggle");
            assert_eq!(
                child(toggle, "caret").props.canvas,
                chevron_glyph(open).props.canvas,
                "carbon `.cds--menu-button__trigger--open svg {{ transform: rotate(180deg) }}`: \
                 the chevron points {} when open={open}",
                if open { "up" } else { "down" }
            );
            assert_eq!(
                toggle.semantics.expanded,
                Some(open),
                "the glyph is never the only channel (FR-026)"
            );
            assert_eq!(
                toggle.constraints.horizontal.min,
                Some(TRIGGER_HALF_W),
                "the caret half keeps its 48 box in both states"
            );
        }
    }

    /// The menu hangs under the WHOLE split — Carbon's sibling selector
    /// `.menu-button__trigger:hover + .menu-button__menu` — at the family
    /// width, so split and menu read as one object.
    #[test]
    fn the_combo_menu_hangs_under_the_split_trigger_at_the_family_width() {
        for open in [false, true] {
            let node = combo_button("combo", "Delete", open, three_items());
            if !open {
                assert!(
                    node.children.iter().all(|c| c.key.as_str() != "menu"),
                    "closed: the overlay is mounted only while open (FR-026)"
                );
                continue;
            }
            let trigger = child(&node, "trigger");
            let menu = child(&node, "menu");
            assert!(
                trigger.children.iter().all(|c| c.key.as_str() != "menu"),
                "carbon `--custom-trigger`: the menu is the split's SIBLING \
                 (`md--menu-button__menu`), not its child"
            );
            let Some(Anchor::Sibling { key, .. }) = &menu.props.anchor else {
                panic!("the combo's menu does not hang on a sibling anchor");
            };
            assert_eq!(
                key.as_str(),
                "trigger",
                "carbon `.menu-button__trigger:hover + .menu-button__menu`: the menu hangs on its split"
            );
            assert_eq!(
                (
                    menu.constraints.horizontal.min,
                    menu.constraints.horizontal.max
                ),
                (Some(MIN_INLINE), Some(MAX_INLINE)),
                "carbon `$menu-options-width`: the open menu takes the family's 160 floor and 288 ceiling"
            );
            assert_eq!(
                menu.props.fit,
                Some(Fit::Anchor),
                "at least its anchor's width: the menu and the split are one width"
            );
            assert_eq!(
                menu.semantics.label.as_deref(),
                Some("Delete options"),
                "the menu's name derives from the action half: `{{action}} options`"
            );
        }
    }

    /// Spec 008 T051's overflow trigger: the icon-only ghost in the
    /// `$spacing-13` box, whose `--open` state swaps its ground to `$layer`
    /// so it merges with its menu.
    #[test]
    fn the_overflow_trigger_is_an_icon_only_ghost_that_merges_with_its_menu() {
        for open in [false, true] {
            let node = overflow_menu("more", "More actions", open, three_items());
            let trigger = child(&node, "trigger");
            assert_eq!(
                (
                    trigger.constraints.horizontal.min,
                    trigger.constraints.horizontal.max
                ),
                (Some(TRIGGER_HALF_W), Some(TRIGGER_HALF_W)),
                "carbon `overflow-menu__icon`: the $spacing-13 icon box"
            );
            assert_eq!(
                trigger.constraints.vertical.min,
                Some(SIZE_MD),
                "carbon `overflow-menu__icon {{ height: $button-height }}`: the md step"
            );
            assert_eq!(
                trigger.semantics.label.as_deref(),
                Some("More actions"),
                "the label is the trigger's real name (FR-058); the glyph substitutes a kebab, never the name"
            );
            let glyph = child(trigger, "glyph");
            assert_eq!(glyph.kind, NodeKind::Canvas, "the glyph is a drawn mark");
            assert_eq!(
                glyph.props.canvas,
                icon_toned("glyph", IconMark::Menu, IconTone::Primary)
                    .props
                    .canvas,
                "carbon `OverflowMenuVertical` (kebab); this icon set ships none, so the recorded \
                 Menu-mark substitution stands (the one `data_table.rs` made first)"
            );
            assert_eq!(
                trigger.props.tokens.get("background").map(|s| s.as_str()),
                Some(if open { SURFACE_RAISED } else { SURFACE_BASE }),
                "carbon `.overflow-menu--open {{ background-color: $layer }}`: the open trigger's \
                 ground becomes the menu's and the two merge"
            );
            assert_eq!(
                trigger.behaviour,
                Some(TOGGLES_ON_RELEASE),
                "the trigger is a toggle, not a one-shot action"
            );
        }
    }

    /// The open menu is `overflow-menu-options`: fixed `$spacing-13` (160 —
    /// the crate's own Carbon ramp, `token/shipped.rs`), not fitted to the
    /// 48-wide trigger, and wearing the `::after` connector tail as
    /// [`Tip::Caret`].
    #[test]
    fn the_overflow_menu_is_fixed_at_spacing_13_and_carries_the_connector_tail() {
        for open in [false, true] {
            let node = overflow_menu("more", "More actions", open, three_items());
            if !open {
                assert!(
                    node.children.iter().all(|c| c.key.as_str() != "menu"),
                    "closed: the overlay is mounted only while open (FR-026)"
                );
                continue;
            }
            let menu = child(&node, "menu");
            assert_eq!(
                (
                    menu.constraints.horizontal.min,
                    menu.constraints.horizontal.max
                ),
                (Some(OVERFLOW_MENU_W), Some(OVERFLOW_MENU_W)),
                "carbon `overflow-menu-options {{ width: $spacing-13; min-width: $spacing-13 }}`: \
                 160 fixed"
            );
            assert_eq!(
                menu.props.fit,
                Some(Fit::Content),
                "the items bound the menu; `Fit::Anchor` would squash it to the trigger's 48"
            );
            assert_eq!(
                menu.props.tip,
                Some(Tip::Caret),
                "carbon `@include caret`: the pseudo-element connector tail"
            );
            let Some(Anchor::Sibling { key, .. }) = &menu.props.anchor else {
                panic!("the overflow menu does not hang on a sibling anchor");
            };
            assert_eq!(
                key.as_str(),
                "trigger",
                "the menu hangs on its trigger, its flip the engine's business"
            );
            assert_eq!(
                menu.semantics.label.as_deref(),
                Some("More actions"),
                "the menu carries its trigger's name"
            );
        }
    }

    /// The size ramp threads trigger → menu: `layout.use('size')` steps the
    /// halves and the rows together (xs 24 / sm 32 / md 40 / lg 48).
    #[test]
    fn the_size_step_threads_the_halves_and_the_menu_rows() {
        let lg = combo_button_lg("combo", "Delete", true, three_items());
        let trigger = child(&lg, "trigger");
        assert_eq!(
            child(trigger, "action").constraints.vertical.min,
            Some(48.0),
            "carbon `$trigger-sizes` `--lg`: the action half steps to 48"
        );
        assert_eq!(
            child(trigger, "toggle").constraints.vertical.min,
            Some(48.0),
            "the halves step together"
        );
        let xs = overflow_menu_xs("more", "More actions", true, three_items());
        assert_eq!(
            child(&xs, "trigger").constraints.vertical.min,
            Some(24.0),
            "carbon `overflow-menu--sm .overflow-menu__icon {{ height: $spacing-06 }}` (24) and friends"
        );
        let content = child(child(&xs, "menu"), "content");
        assert_eq!(
            child(content, "rename").constraints.vertical.min,
            Some(24.0),
            "carbon `overflow-menu-options__option {{ height: $spacing-06 }}`: the open menu's rows \
             take the trigger's step"
        );
    }

    /// The open forms' menus name their `trigger` sibling by bare key
    /// (`Anchor::Sibling`), so they are accepted wherever a caller mounts
    /// them — the gallery catalog's own depth.
    #[test]
    fn the_new_open_shapes_validate_when_mounted_at_catalog_depth() {
        crate::component::tests::assert_mounts_at_catalog_depth(
            "combo open",
            vec![combo_button("combo", "Delete", true, three_items())],
        );
        crate::component::tests::assert_mounts_at_catalog_depth(
            "combo-lg open",
            vec![combo_button_lg("combo", "Delete", true, three_items())],
        );
        crate::component::tests::assert_mounts_at_catalog_depth(
            "overflow open",
            vec![overflow_menu("more", "More actions", true, three_items())],
        );
        crate::component::tests::assert_mounts_at_catalog_depth(
            "overflow-xs open",
            vec![overflow_menu_xs(
                "more",
                "More actions",
                true,
                three_items(),
            )],
        );
    }

    /// The frame-level geometry check audits the CLOSED shapes: an open
    /// menu is an anchored overlay and hangs outside its parent's rect by
    /// design, so the parent-bounds rule does not speak to it.
    #[test]
    fn the_new_closed_shapes_frame_clean_at_catalog_depth() {
        for (name, node) in [
            (
                "combo",
                combo_button("combo", "Delete", false, three_items()),
            ),
            (
                "combo-lg",
                combo_button_lg("combo", "Delete", false, three_items()),
            ),
            (
                "overflow",
                overflow_menu("more", "More actions", false, three_items()),
            ),
            (
                "overflow-xs",
                overflow_menu_xs("more", "More actions", false, three_items()),
            ),
        ] {
            let frame = petrify_lone(node);
            for p in &frame.placements {
                assert!(
                    p.rect.w > 0.0 && p.rect.h > 0.0,
                    "{name}: {} placed with a degenerate rect {:?}",
                    p.id,
                    p.rect
                );
                assert!(
                    !p.paint.overflowed,
                    "{name}: {} drew content larger than its own rect",
                    p.id
                );
                if let Some(parent_idx) = p.parent {
                    assert_fits_parent(name, p, &frame.placements[parent_idx]);
                }
            }
        }
    }

    /// The split's two acts and the overflow trigger land in the focus
    /// order, and a disabled one does not (the reachability rule).
    #[test]
    fn the_new_triggers_are_reachable_unless_disabled() {
        for (name, suffix, enabled, disabled_node) in [
            (
                "combo-action",
                "/action",
                combo_button("combo", "Delete", false, three_items()),
                disabled(combo_button("combo", "Delete", false, three_items())),
            ),
            (
                "combo-toggle",
                "/toggle",
                combo_button("combo", "Delete", false, three_items()),
                disabled(combo_button("combo", "Delete", false, three_items())),
            ),
            (
                "overflow",
                "/trigger",
                overflow_menu("more", "More actions", false, three_items()),
                disabled(overflow_menu("more", "More actions", false, three_items())),
            ),
        ] {
            let frame = petrify_lone(enabled);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let order = focus.order();
            let hit = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{name}: no placement ends with {suffix}"));
            assert!(
                order.iter().any(|o| o == &hit.id),
                "{name}: {} declares Focus but is not in focus order",
                hit.id
            );

            let frame = petrify_lone(disabled_node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let order = focus.order();
            for p in frame.placements.iter().filter(|p| p.id.ends_with(suffix)) {
                assert!(
                    !order.iter().any(|o| o == &p.id),
                    "{name}: disabled {} is still in focus order",
                    p.id
                );
            }
        }
    }

    /// The new triggers' text and glyphs clear AA against their own
    /// resting fills, both themes (the label rule, SC-014).
    #[test]
    fn the_new_triggers_clear_aa_contrast_against_their_own_resting_fills() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let combo = combo_button("combo", "Delete", false, three_items());
            let trigger = child(&combo, "trigger");
            let overflow = overflow_menu("more", "More actions", false, three_items());
            for (where_, part) in [
                ("combo/action", child(trigger, "action")),
                ("combo/toggle", child(trigger, "toggle")),
                ("overflow/trigger", child(&overflow, "trigger")),
            ] {
                let bg_name = part
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{where_} binds a resting background"));
                let bg = color(&theme, bg_name.as_str());
                for key in ["label", "caret", "glyph"] {
                    let Some(sub) = part.children.iter().find(|c| c.key.as_str() == key) else {
                        continue;
                    };
                    let inks = inks(sub);
                    let opacity = sub.props.opacity.unwrap_or(1.0);
                    for fg_name in inks {
                        let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                        let ratio = fg.contrast_ratio(bg);
                        assert!(
                            ratio >= MIN_TEXT_CONTRAST,
                            "{where_}/{key} at {ratio:.2}:1 against {} fails AA \
                             {MIN_TEXT_CONTRAST}:1",
                            fg_name.as_str()
                        );
                    }
                }
            }
        }
    }
}
