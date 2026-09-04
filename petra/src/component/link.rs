//! `link` — Carbon Link (slice-c): an activatable text control.
//!
//! There is no `Role::Link`. The activatable control is [`Role::Button`].
//! Colour is not the only channel: role + label name it. No underline token
//! exists in `tokens.rs`, so none is bound. Quiet fill is [`SURFACE_BASE`]
//! so [`super::on_layer`] can disappear the control into its ground.
//!
//! Carbon `$link-primary` is not in the component token list; ink is
//! [`TEXT_PRIMARY`].

use super::stack;
use super::text::text;
use super::tokens::{SURFACE_BASE, TEXT_PRIMARY, t};
use crate::geom::Axis;
use crate::tree::{Interaction, Key, Role, ViewNode};

/// A navigational text control. `label` is required (FR-058).
///
/// Focus and Click only. Carbon underline-on-hover has no token here.
pub fn link(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut caption = text("label", label.clone());
    caption
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_PRIMARY));
    let mut node = stack(key, Axis::Horizontal, None, vec![caption]);
    node.props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
    node.interactive(
        Role::Button,
        label,
        &[Interaction::Focus, Interaction::Click],
    )
}

#[cfg(test)]
mod tests {
    use super::link;
    use crate::component::disabled;
    use crate::component::tokens::{SURFACE_BASE, TEXT_PRIMARY};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
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

    #[test]
    fn link_is_a_labelled_button() {
        let node = link("docs", "Open docs");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Open docs"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(!node.interactions.contains(&Interaction::Drag));
        assert_eq!(token(&node, "background"), Some(SURFACE_BASE));
        assert!(node.props.tokens.get("border").is_none());
        let caption = child(&node, "label");
        assert_eq!(caption.props.text.as_deref(), Some("Open docs"));
        assert_eq!(token(caption, "foreground"), Some(TEXT_PRIMARY));
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

    /// Check C/D: the link places with a real rect, none of its parts
    /// outside it.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let frame = petrify_lone(link("docs", "Open docs"));
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
                let parent = &frame.placements[parent_idx];
                let fits = p.rect.x >= parent.rect.x - 0.01
                    && p.rect.y >= parent.rect.y - 0.01
                    && p.rect.x + p.rect.w <= parent.rect.x + parent.rect.w + 0.01
                    && p.rect.y + p.rect.h <= parent.rect.y + parent.rect.h + 0.01;
                assert!(
                    fits,
                    "{} (rect {:?}) extends outside its parent {} (rect {:?})",
                    p.id, p.rect, parent.id, parent.rect
                );
            }
        }
    }

    /// Check F: an enabled link is reachable; a disabled one is not.
    #[test]
    fn a_link_is_reachable_unless_disabled() {
        let enabled = petrify_lone(link("docs", "Open docs"));
        let focus = crate::focus::FocusTree::from_placements(
            &enabled.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        let node = enabled
            .placements
            .iter()
            .find(|p| p.id.ends_with("/docs"))
            .expect("the link is placed");
        assert!(
            order.iter().any(|o| o == &node.id),
            "the link declares Focus but is not in focus order"
        );

        let disabled_frame = petrify_lone(disabled(link("docs", "Open docs")));
        let disabled_focus = crate::focus::FocusTree::from_placements(
            &disabled_frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let disabled_order = disabled_focus.order();
        let disabled_node = disabled_frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/docs"))
            .expect("the disabled link is placed");
        assert!(
            !disabled_order.iter().any(|o| o == &disabled_node.id),
            "a disabled link must not be reachable"
        );
    }

    /// Check E: the label's text against the link's own resting fill, in
    /// both themes.
    #[test]
    fn label_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = link("docs", "Open docs");
            let bg_name = node
                .props
                .tokens
                .get("background")
                .expect("link binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            let caption = child(&node, "label");
            let fg_name = caption
                .props
                .tokens
                .get("foreground")
                .expect("label binds a foreground");
            let opacity = caption.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "link label at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                fg_name.as_str()
            );
        }
    }
}
