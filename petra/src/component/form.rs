//! `form` — Carbon Form (slice-b): a labelled vertical stack of fields.
//!
//! Carbon documents Form as a spacing/label shell, not a control. There is
//! no `Role::Group`, so this constructor sets no role and no interactions.
//! Legend colour is `$text-secondary` → [`TEXT_MUTED`].
//!
//! Gap is [`SPACING_05`] (16). Carbon default form-item `margin-bottom` is
//! `$spacing-07` (32); the constructor contract pins spacing-05.

use super::stack;
use super::text::text;
use super::tokens::{SPACING_05, TEXT_MUTED, t};
use crate::geom::Axis;
use crate::tree::{Key, ViewNode};

/// A vertical field group with a muted legend and no role.
///
/// `children` are the fields. The legend is a `text` child keyed `"legend"`.
/// Not interactive: Form does not submit, focus, or click.
pub fn form(key: impl Into<Key>, legend: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    let mut legend_node = text("legend", legend);
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    let mut rows = Vec::with_capacity(children.len() + 1);
    rows.push(legend_node);
    rows.extend(children);
    stack(key, Axis::Vertical, Some(SPACING_05), rows)
}

#[cfg(test)]
mod tests {
    use super::form;
    use crate::component::field::field;
    use crate::component::tokens::{SPACING_05, TEXT_MUTED};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{NodeKind, Props, Registry, ViewNode};

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
    fn form_is_a_vertical_stack_with_spacing_05() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(crate::geom::Axis::Vertical));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_05)
        );
    }

    #[test]
    fn form_has_no_role_and_is_not_interactive() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        assert!(node.semantics.role.is_none());
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(node.semantics.label.is_none());
    }

    #[test]
    fn legend_is_muted_text() {
        let node = form("signup", "Account", vec![field("name", "Name")]);
        let legend = child(&node, "legend");
        assert_eq!(legend.kind, NodeKind::Text);
        assert_eq!(legend.props.text.as_deref(), Some("Account"));
        assert_eq!(token(legend, "foreground"), Some(TEXT_MUTED));
        let _ = child(&node, "name");
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

    /// Check C/D: the legend and both fields place with a real rect, none
    /// of them outside the form.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
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

    /// Check F: `form` declares no interaction of its own, but its field
    /// children keep theirs — the shell does not steal focus reachability
    /// from what it groups.
    #[test]
    fn fields_stay_reachable_inside_a_form() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        for suffix in ["/name", "/email"] {
            let p = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the petrified frame"));
            assert!(
                focus.order().iter().any(|o| o == &p.id),
                "{suffix} declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: the legend against the page ground it is read on
    /// (`surface.base`, matching how `text()` itself is styled to sit on
    /// the base layer — `form` sets no fill of its own), read through
    /// `Props.opacity`.
    #[test]
    fn legend_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            let node = form("signup", "Account", vec![field("name", "Name")]);
            let legend = child(&node, "legend");
            let fg_name = legend
                .props
                .tokens
                .get("foreground")
                .expect("legend binds a foreground");
            let opacity = legend.props.opacity.unwrap_or(1.0);
            let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
            let ratio = fg.contrast_ratio(bg);
            assert!(
                ratio >= MIN_TEXT_CONTRAST,
                "legend at {ratio:.2}:1 against {SURFACE_BASE} fails AA {MIN_TEXT_CONTRAST}:1"
            );
        }
    }
}
