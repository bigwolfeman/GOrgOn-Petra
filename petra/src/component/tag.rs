//! Carbon Tag (slice-e).
//!
//! Four Carbon forms; this file ships three. Operational needs Popover
//! (Wave 3) and is omitted. Colour variants (`$tag-background-red` …) are
//! **skipped**: those names are not in [`super::tokens`], and inventing
//! hues here would put a colour decision in a component. High-contrast /
//! outline uses [`BORDER_STRONG`].
//!
//! Sizes MEASURED `_tag.scss`: sm 18, md 24 (default), lg 32. Radius
//! [`SHAPE_FULL`]. `min-inline-size` 32, `max-inline-size` 208.
//!
//! Read-only [`tag`] is not interactive. [`dismissible_tag`] is a labelled
//! button `"Dismiss {label}"` drawing [`IconMark::Close`] (Carbon's
//! `.cds--tag__close-icon`, `color: $icon-primary`, 16px glyph, slice-e);
//! the label is what makes the close never icon-only (FR-026).
//! [`selectable_tag`] is [`Role::Button`] plus
//! `Semantics.selected`, plus [`IconMark::Check`] when selected — Carbon
//! gives selectable tags no icon spec (unlike Structured list's own
//! `RadioButtonChecked`, `.agents/research/08-25-2026/Carbon-Component-Inventory/slice-e.md`),
//! but a measured render still failed the same test a spec would have
//! caught: selectable tags carry only core tokens
//! (`$layer`/`$border-inverse`/`$text-primary`, slice-e:150), and stepping
//! `layer-selected` off the same `$layer` a selectable tag already rests
//! on (`layer.raised`) is the *smallest* step this vocabulary has — a
//! captured render measured the selected and unselected fills only 2 of
//! 255 sRGB steps apart in the dark theme, not merely "hard for a
//! colour-blind reader" but indistinguishable for any reader. See
//! [`selectable_tag`]'s own doc.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_STRONG, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_FULL, SPACING_03,
    SPACING_04, SURFACE_RAISED, TEXT_PRIMARY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Key, Role, TextWrap, ViewNode,
};

/// Carbon sm tag height (`1.125rem`).
const HEIGHT_SM: f32 = 18.0;
/// Carbon md tag height (`1.5rem`). Default.
const HEIGHT_MD: f32 = 24.0;
/// Carbon lg tag height (`2rem`).
const HEIGHT_LG: f32 = 32.0;
/// Carbon `min-inline-size` on every tag.
const MIN_INLINE: f32 = 32.0;
/// Carbon `max-inline-size` before the title truncates.
const MAX_INLINE: f32 = 208.0;

const _: () = assert!(HEIGHT_SM == 18.0);
const _: () = assert!(HEIGHT_MD == 24.0);
const _: () = assert!(HEIGHT_LG == 32.0);
const _: () = assert!(MIN_INLINE == 32.0);
const _: () = assert!(MAX_INLINE == 208.0);

const INTERACTIVE: &[Interaction] = &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// Read-only tag, Carbon md (24). Not interactive.
pub fn tag(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    read_only_tag(key, label, HEIGHT_MD, SPACING_03)
}

/// Carbon sm (18).
pub fn tag_sm(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    read_only_tag(key, label, HEIGHT_SM, SPACING_03)
}

/// Carbon lg (32).
pub fn tag_lg(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    read_only_tag(key, label, HEIGHT_LG, SPACING_04)
}

/// Dismissible tag. The whole pill is `"Dismiss {label}"` (FR-058); that
/// name is the second channel, so the [`IconMark::Close`] glyph is never
/// icon-only.
///
/// No `border`: slice-e's anatomy line names dismissible tags explicitly
/// among the variants that do *not* draw one — see [`shell`]'s own doc.
pub fn dismissible_tag(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let accessible = format!("Dismiss {label}");
    let title = title_text("label", label);
    let dismiss = icon_toned("dismiss", IconMark::Close, IconTone::Primary);
    shell(
        key,
        HEIGHT_MD,
        SPACING_03,
        vec![title, dismiss],
        true,
        false,
    )
    .interactive(Role::Button, accessible, INTERACTIVE)
    .owning_its_text()
}

/// Selectable tag. [`Role::Button`] + `Semantics.selected`. Outline is
/// [`BORDER_STRONG`] (high-contrast/outline stand-in). No colour set.
///
/// The control-boundary tone rather than the decorative one (2026-09-05
/// split, see `tokens::BORDER_SUBTLE`): a selectable tag's fill is
/// `SURFACE_RAISED`, which on a card is the card's own tone, so the pill
/// edge is the only thing that says a pill is there.
///
/// Selected additionally draws [`IconMark::Check`] ahead of the title, in
/// [`IconTone::Primary`]: the pill is a layer, not an accent track, and the
/// default `text.on-accent` fill measured 1.44:1 against it — the only
/// thing marking selection on this page, and invisible.
/// Carbon's own anatomy names no icon for this variant — the module doc
/// explains why this is built anyway: `layer-selected` is a measured 2-of-
/// 255 sRGB step off the resting `layer.raised` fill in the dark theme, a
/// render no reader recovers reliably. [`IconMark::Check`] is the
/// vocabulary's existing "this is on" glyph
/// ([`super::tile::selectable_tile`], the toggle, radio, checkbox), reused
/// rather than invented.
pub fn selectable_tag(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut parts = Vec::new();
    if selected {
        parts.push(icon_toned("mark", IconMark::Check, IconTone::Primary));
    }
    parts.push(title_text("label", label.clone()));
    let mut node = shell(key, HEIGHT_MD, SPACING_03, parts, true, true);
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    let mut node = node
        .interactive(Role::Button, label, INTERACTIVE)
        .owning_its_text();
    node.semantics.selected = selected;
    node
}

fn read_only_tag(
    key: impl Into<Key>,
    label: impl Into<String>,
    height: f32,
    inline_pad: &str,
) -> ViewNode {
    let label = label.into();
    shell(
        key,
        height,
        inline_pad,
        vec![title_text("label", label)],
        false,
        false,
    )
}

fn title_text(key: &'static str, content: String) -> ViewNode {
    let mut node = text(key, content);
    node.props.wrap = Some(TextWrap::Ellipsis);
    node.props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    node
}

/// `hover` and `border` are independent, not one `interactive` flag: slice-e's
/// own anatomy line splits them by variant — *"Selectable and Operational
/// additionally have a Border (E) that read-only/dismissible tags do not
/// have"* (SOURCED, usage page "Formatting"). Dismissible tags get hover
/// (they are in the "enabled, hover, focus, on-click, disabled, skeleton"
/// state list, slice-e "States") but not the border; Selectable/Operational
/// get both.
fn shell(
    key: impl Into<Key>,
    height: f32,
    inline_pad: &str,
    children: Vec<ViewNode>,
    hover: bool,
    border: bool,
) -> ViewNode {
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), children);
    node.props.align = Some(Align::Center);
    node.props.padding = Some(InsetRefs {
        left: Some(t(inline_pad)),
        right: Some(t(inline_pad)),
        ..InsetRefs::default()
    });
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    node.props.tokens.insert("radius".into(), t(SHAPE_FULL));
    if border {
        node.props.tokens.insert("border".into(), t(BORDER_STRONG));
    }
    if hover {
        node.props
            .tokens
            .insert("background@hover".into(), t(LAYER_HOVER));
    }
    node.with_constraints(Constraints {
        horizontal: AxisConstraint {
            min: Some(MIN_INLINE),
            max: Some(MAX_INLINE),
            priority: 0,
        },
        vertical: AxisConstraint {
            min: Some(height),
            max: Some(height),
            priority: 0,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{
        HEIGHT_LG, HEIGHT_MD, HEIGHT_SM, IconMark, IconTone, MAX_INLINE, MIN_INLINE,
        dismissible_tag, icon_toned, selectable_tag, tag, tag_lg, tag_sm,
    };
    use crate::component::tokens::{
        BORDER_STRONG, LAYER_HOVER, LAYER_SELECTED, SHAPE_FULL, SURFACE_RAISED,
    };
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, inks, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn child<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        node.children
            .iter()
            .find(|c| c.key.as_str() == key)
            .map(|c| c.as_ref())
            .unwrap_or_else(|| panic!("missing child {key}"))
    }

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

    fn has_canvas(node: &ViewNode) -> bool {
        node.kind == crate::tree::NodeKind::Canvas
            || node.children.iter().any(|child| has_canvas(child))
    }

    #[test]
    fn tag_is_read_only_at_height_24() {
        let node = tag("env", "prod");
        assert_eq!(node.constraints.vertical.min, Some(HEIGHT_MD));
        assert_eq!(node.constraints.vertical.max, Some(HEIGHT_MD));
        assert_eq!(HEIGHT_MD, 24.0);
        assert_eq!(node.constraints.horizontal.min, Some(MIN_INLINE));
        assert_eq!(node.constraints.horizontal.max, Some(MAX_INLINE));
        assert_eq!(MIN_INLINE, 32.0);
        assert_eq!(MAX_INLINE, 208.0);
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(node.semantics.role.is_none());
        assert_eq!(token(&node, "radius"), Some(SHAPE_FULL));
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(
            token(&node, "border"),
            None,
            "read-only tags have no interactive edge"
        );
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("prod"));
    }

    #[test]
    fn tag_sm_is_18_and_lg_is_32() {
        let sm = tag_sm("env", "prod");
        assert_eq!(sm.constraints.vertical.min, Some(HEIGHT_SM));
        assert_eq!(HEIGHT_SM, 18.0);
        assert!(!sm.is_interactive());
        let lg = tag_lg("env", "prod");
        assert_eq!(lg.constraints.vertical.min, Some(HEIGHT_LG));
        assert_eq!(HEIGHT_LG, 32.0);
        assert_eq!(token(&lg, "radius"), Some(SHAPE_FULL));
    }

    #[test]
    fn dismissible_tag_is_a_labelled_button_never_icon_only() {
        let node = dismissible_tag("env", "prod");
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Dismiss prod"));
        assert!(node.interactions.contains(&Interaction::Click));
        assert_eq!(child(&node, "label").props.text.as_deref(), Some("prod"));
        let dismiss = child(&node, "dismiss");
        assert_eq!(dismiss.kind, crate::tree::NodeKind::Canvas);
        assert_eq!(
            dismiss.props.text, None,
            "close is a glyph, not the word Dismiss"
        );
        assert_eq!(
            dismiss.props.canvas,
            icon_toned("dismiss", IconMark::Close, IconTone::Primary)
                .props
                .canvas,
            "close draws Carbon's Close in the layer's icon tone"
        );
        assert!(
            has_canvas(&node),
            "the close glyph is drawn; the pill's own label keeps it from being icon-only"
        );
        assert_eq!(node.constraints.vertical.min, Some(HEIGHT_MD));
        assert_eq!(
            token(&node, "border"),
            None,
            "dismissible tags have no border — slice-e's anatomy line names \
             them explicitly among the variants that do not draw one, only \
             Selectable and Operational do"
        );
        assert_eq!(
            token(&node, "background@hover"),
            Some(LAYER_HOVER),
            "dismissible tags still get hover (slice-e States: \"enabled, \
             hover, focus, on-click, disabled, skeleton\"), just not the edge"
        );
    }

    #[test]
    fn selectable_tag_declares_selected() {
        let on = selectable_tag("env", "prod", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("prod"));
        assert!(on.semantics.selected);
        assert!(on.interactions.contains(&Interaction::Click));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "border"), Some(BORDER_STRONG));
        assert_eq!(on.constraints.vertical.min, Some(HEIGHT_MD));

        let off = selectable_tag("env", "prod", false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
    }

    /// A5 (2026-09-04): a captured render measured `layer-selected` only 2
    /// of 255 sRGB steps off the resting `layer.raised` fill in the dark
    /// theme — fill tone alone does not carry this state for any reader.
    /// Selected must carry a canvas mark; unselected must not.
    #[test]
    fn selectable_tag_selection_carries_a_second_channel() {
        let on = selectable_tag("env", "prod", true);
        assert!(
            has_canvas(&on),
            "layer-selected measured 2 of 255 sRGB steps off the resting \
             fill; selection must not be fill-tone alone"
        );
        let off = selectable_tag("env", "prod", false);
        assert!(
            !has_canvas(&off),
            "the mark is the on-state glyph, not a permanent decoration"
        );
    }

    #[test]
    fn tag_does_not_invent_colour_tokens() {
        let node = tag("env", "prod");
        for (slot, name) in &node.props.tokens {
            assert!(
                !name.as_str().contains("red")
                    && !name.as_str().contains("magenta")
                    && !name.as_str().contains("tag-background"),
                "skipped the 10-colour set, found {slot}={name}"
            );
        }
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

    /// Check C/D across every shipped constructor. `dismissible_tag`'s
    /// close was the class-4 suspect named in this group's brief while it
    /// was the word `"Dismiss"` (an icon-only hit box pinned around a
    /// word); it is a 16px glyph now and sits inside the same fluid
    /// horizontal stack as the title — this is the frame-level proof that
    /// it does not overflow, not a substitute for reading the code.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("read-only", tag("env", "prod")),
            ("sm", tag_sm("env", "prod")),
            ("lg", tag_lg("env", "prod")),
            ("dismissible", dismissible_tag("env", "prod")),
            ("selectable-on", selectable_tag("env", "prod", true)),
            ("selectable-off", selectable_tag("env", "prod", false)),
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

    /// Check F: read-only tags declare no interactions at all and are
    /// never reachable; dismissible and selectable tags declare `Focus`
    /// on the whole pill and are.
    #[test]
    fn interactive_forms_are_reachable_and_read_only_is_not() {
        for (label, node, should_be_focusable) in [
            ("read-only", tag("env", "prod"), false),
            ("dismissible", dismissible_tag("env", "prod"), true),
            ("selectable", selectable_tag("env", "prod", false), true),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/env"))
                .expect("the tag is placed");
            let reachable = focus.order().iter().any(|o| o == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the title text (and the dismiss word) against the pill's
    /// own resting fill, in both themes.
    #[test]
    fn title_clears_aa_contrast_against_its_own_pill_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node, keys) in [
                ("read-only", tag("env", "prod"), vec!["label"]),
                (
                    "dismissible",
                    dismissible_tag("env", "prod"),
                    vec!["label", "dismiss"],
                ),
                (
                    "selectable",
                    selectable_tag("env", "prod", false),
                    vec!["label"],
                ),
            ] {
                let pill_bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: pill binds a resting background"));
                let pill_bg = color(&theme, pill_bg_name.as_str());
                for key in keys {
                    let text_node = node
                        .children
                        .iter()
                        .find(|c| c.key.as_str() == key)
                        .unwrap_or_else(|| panic!("{label}: missing child {key}"));
                    let inks = inks(text_node);
                    assert!(!inks.is_empty(), "{label}: {key} binds an ink");
                    let opacity = text_node.props.opacity.unwrap_or(1.0);
                    for fg_name in inks {
                        let fg = color(&theme, fg_name.as_str()).faded(opacity).over(pill_bg);
                        let ratio = fg.contrast_ratio(pill_bg);
                        assert!(
                            ratio >= MIN_TEXT_CONTRAST,
                            "{label} {key} at {ratio:.2}:1 against {} fails AA \
                             {MIN_TEXT_CONTRAST}:1",
                            pill_bg_name.as_str()
                        );
                    }
                }
            }
        }
    }
}
