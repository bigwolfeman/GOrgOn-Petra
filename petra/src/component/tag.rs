//! Carbon Tag (slice-e).
//!
//! Four Carbon forms; this file ships three. Operational needs Popover
//! (Wave 3) and is omitted. Colour variants (`$tag-background-red` …) are
//! **skipped**: those names are not in [`super::tokens`], and inventing
//! hues here would put a colour decision in a component. High-contrast /
//! outline uses [`BORDER_SUBTLE`].
//!
//! Sizes MEASURED `_tag.scss`: sm 18, md 24 (default), lg 32. Radius
//! [`SHAPE_FULL`]. `min-inline-size` 32, `max-inline-size` 208.
//!
//! Read-only [`tag`] is not interactive. [`dismissible_tag`] is a labelled
//! button `"Dismiss {label}"` with a visible `"Dismiss"` word — never an
//! icon-only close (FR-026). [`selectable_tag`] is [`Role::Button`] plus
//! `Semantics.selected`.

use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, LAYER_HOVER, LAYER_SELECTED, LAYER_SELECTED_HOVER, SHAPE_FULL, SPACING_03,
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

/// Dismissible tag. The whole pill is `"Dismiss {label}"` (FR-058); the
/// visible `"Dismiss"` word is the second channel so close is never
/// icon-only.
pub fn dismissible_tag(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let accessible = format!("Dismiss {label}");
    let title = title_text("label", label);
    let mut dismiss = text("dismiss", "Dismiss");
    dismiss
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    shell(key, HEIGHT_MD, SPACING_03, vec![title, dismiss], true).interactive(
        Role::Button,
        accessible,
        INTERACTIVE,
    )
}

/// Selectable tag. [`Role::Button`] + `Semantics.selected`. Outline is
/// [`BORDER_SUBTLE`] (high-contrast/outline stand-in). No colour set.
pub fn selectable_tag(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    let label = label.into();
    let mut node = shell(
        key,
        HEIGHT_MD,
        SPACING_03,
        vec![title_text("label", label.clone())],
        true,
    );
    node.props
        .tokens
        .insert("background@selected".into(), t(LAYER_SELECTED));
    node.props
        .tokens
        .insert("background@selected-hover".into(), t(LAYER_SELECTED_HOVER));
    let mut node = node.interactive(Role::Button, label, INTERACTIVE);
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

fn shell(
    key: impl Into<Key>,
    height: f32,
    inline_pad: &str,
    children: Vec<ViewNode>,
    interactive: bool,
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
    if interactive {
        node.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
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
        HEIGHT_LG, HEIGHT_MD, HEIGHT_SM, MAX_INLINE, MIN_INLINE, dismissible_tag, selectable_tag,
        tag, tag_lg, tag_sm,
    };
    use crate::component::tokens::{BORDER_SUBTLE, LAYER_SELECTED, SHAPE_FULL, SURFACE_RAISED};
    use crate::tree::{Interaction, Role, ViewNode};

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
        assert_eq!(
            child(&node, "dismiss").props.text.as_deref(),
            Some("Dismiss")
        );
        assert!(
            !has_canvas(&node),
            "close is the word Dismiss, not an icon-only mark"
        );
        assert_eq!(node.constraints.vertical.min, Some(HEIGHT_MD));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
    }

    #[test]
    fn selectable_tag_declares_selected() {
        let on = selectable_tag("env", "prod", true);
        assert_eq!(on.semantics.role, Some(Role::Button));
        assert_eq!(on.semantics.label.as_deref(), Some("prod"));
        assert!(on.semantics.selected);
        assert!(on.interactions.contains(&Interaction::Click));
        assert_eq!(token(&on, "background@selected"), Some(LAYER_SELECTED));
        assert_eq!(token(&on, "border"), Some(BORDER_SUBTLE));
        assert_eq!(on.constraints.vertical.min, Some(HEIGHT_MD));

        let off = selectable_tag("env", "prod", false);
        assert!(!off.semantics.selected);
        assert!(off.is_interactive());
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
}
