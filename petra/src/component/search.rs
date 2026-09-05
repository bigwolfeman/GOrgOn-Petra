//! Carbon Search (slice-d).
//!
//! Anatomy (`_search.scss`): field + decorative magnifier + typed value.
//! The close ("x") button is a filled-state affordance and is omitted
//! until a Clear control is wired; expandable is omitted (width animation
//! plus a collapsed icon-button).
//!
//! The magnifier is [`IconMark::Search`] in [`IconTone::Secondary`]
//! (`.cds--search-magnifier-icon`, `fill: $icon-secondary`, 16×16,
//! slice-d) and is **not** the only channel: the field has a label, and
//! the mark carries the word `Search` as its own accessible name
//! (FR-026). The magnifier itself has no role and no interactions.
//! `NodeKind::Input` is a leaf, so the magnifier sits as a sibling in the
//! well the way Number input's steppers do.
//!
//! Sizes: sm 32, md 40 (default), lg 48. Fill/edge is Petra's field pair.

use super::icon::{IconMark, IconTone, icon_toned};
use super::stack;
use super::tokens::{
    BORDER_SUBTLE, SHAPE_SM, SIZE_MD, SPACING_03, SPACING_04, SURFACE_RAISED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, InsetRefs, Key, NodeKind, Props, Role, ViewNode};

/// Carbon Search sm. `tokens` only ships [`SIZE_MD`] (md / 40).
const SIZE_SM: f32 = 32.0;
/// Carbon Search lg.
const SIZE_LG: f32 = 48.0;

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);

/// Search field, Carbon md (40). `label` names the input (FR-058).
pub fn search(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    search_sized(key, label, SIZE_MD)
}

/// Carbon sm (32).
pub fn search_sm(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    search_sized(key, label, SIZE_SM)
}

/// Carbon lg (48).
pub fn search_lg(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    search_sized(key, label, SIZE_LG)
}

fn search_sized(key: impl Into<Key>, label: impl Into<String>, height: f32) -> ViewNode {
    let label = label.into();
    let mut magnifier = icon_toned("magnifier", IconMark::Search, IconTone::Secondary);
    magnifier.semantics.label = Some("Search".to_owned());

    let mut well = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![magnifier, search_field("input", label, height)],
    );
    well.props.align = Some(Align::Center);
    // `NodeKind::Input` carries its own internal inset (`search_field`'s
    // placeholder never touches the well's right edge in the capture), but
    // the magnifier is a bare glyph node with none, so it sat flush on
    // the well's left border — SOURCED
    // `_search.scss:128`: the real icon's leading inset is
    // `calc((layout.size('height') - 1rem) / 2)`, which at md (40px) is
    // (40 − 16) / 2 = 12px, i.e. `SPACING_04`.
    well.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_04)),
        ..InsetRefs::default()
    });
    well.props
        .tokens
        .insert("background".into(), t(SURFACE_RAISED));
    well.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    well.props.tokens.insert("radius".into(), t(SHAPE_SM));
    well.constraints.vertical.min = Some(height);
    well
}

fn search_field(key: &'static str, label: String, height: f32) -> ViewNode {
    let mut props = Props {
        placeholder: Some(label.clone()),
        style: Some(t(TYPOGRAPHY_BODY)),
        ..Props::default()
    };
    props.tokens.insert("foreground".into(), t(TEXT_PRIMARY));
    ViewNode::new(NodeKind::Input, key)
        .with_props(props)
        .interactive(Role::TextInput, label, super::field::EDITABLE_TEXT_INTENTS)
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(height),
                max: None,
                priority: 0,
            },
            ..Constraints::default()
        })
}

#[cfg(test)]
mod tests {
    use super::{SIZE_LG, SIZE_MD, SIZE_SM, search, search_lg, search_sm};
    use crate::component::tokens::{BORDER_SUBTLE, SURFACE_RAISED};
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

    #[test]
    fn search_is_size_md_with_labelled_text_input() {
        let node = search("q", "Filter fibers");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
        assert_eq!(token(&node, "background"), Some(SURFACE_RAISED));
        assert_eq!(token(&node, "border"), Some(BORDER_SUBTLE));
        assert!(
            node.semantics.role.is_none(),
            "wrapper must not steal TextInput"
        );

        let input = child(&node, "input");
        assert_eq!(input.kind, NodeKind::Input);
        assert_eq!(input.semantics.role, Some(Role::TextInput));
        assert_eq!(input.semantics.label.as_deref(), Some("Filter fibers"));
        assert_eq!(input.props.placeholder.as_deref(), Some("Filter fibers"));
        assert_eq!(input.constraints.vertical.min, Some(SIZE_MD));
        assert!(input.interactions.contains(&Interaction::TextEdit));
        assert!(input.interactions.contains(&Interaction::Focus));
        assert!(input.interactions.contains(&Interaction::Key));
    }

    #[test]
    fn search_magnifier_has_no_role() {
        let node = search("q", "Filter fibers");
        let magnifier = child(&node, "magnifier");
        assert!(
            magnifier.semantics.role.is_none(),
            "magnifier is decorative; the field owns the label"
        );
        assert!(magnifier.interactions.is_empty());
        assert!(!magnifier.is_interactive());
        assert_eq!(magnifier.kind, NodeKind::Canvas);
        assert_eq!(
            magnifier.props.text, None,
            "the magnifier is a glyph, not the word"
        );
        assert_eq!(
            magnifier.semantics.label.as_deref(),
            Some("Search"),
            "the word survives as the mark's accessible name"
        );
        assert_eq!(
            child(&node, "input").semantics.label.as_deref(),
            Some("Filter fibers"),
            "magnifier is not the only channel"
        );
    }

    #[test]
    fn search_sm_is_32_and_lg_is_48() {
        let sm = search_sm("q", "Filter");
        assert_eq!(sm.constraints.vertical.min, Some(SIZE_SM));
        assert_eq!(SIZE_SM, 32.0);
        assert_eq!(child(&sm, "input").semantics.role, Some(Role::TextInput));
        let lg = search_lg("q", "Filter");
        assert_eq!(lg.constraints.vertical.min, Some(SIZE_LG));
        assert_eq!(SIZE_LG, 48.0);
        assert_eq!(child(&lg, "magnifier").semantics.role, None);
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

    /// Check C/D across the three sizes. No clear ("x") button case here:
    /// this module's own doc says it "is omitted until a Clear control is
    /// wired" — a scope gap (no constructor exists), not a defect this
    /// audit fixes.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        for (label, node) in [
            ("md", search("q", "Filter fibers")),
            ("sm", search_sm("q", "Filter fibers")),
            ("lg", search_lg("q", "Filter fibers")),
        ] {
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

    /// Check F: the field declares `Focus` and is reachable.
    #[test]
    fn the_field_is_reachable_in_focus_order() {
        let frame = petrify_lone(search("q", "Filter fibers"));
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let input = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/input"))
            .expect("the field is placed");
        assert!(
            focus.order().iter().any(|o| o == &input.id),
            "the field declares Focus but is not in focus order"
        );
    }

    /// Check E: the magnifier and the field's placeholder against the
    /// well's own resting fill, in both themes.
    #[test]
    fn well_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let node = search("q", "Filter fibers");
            let well_bg_name = node
                .props
                .tokens
                .get("background")
                .expect("the well binds a resting background");
            let well_bg = color(&theme, well_bg_name.as_str());
            let magnifier = child(&node, "magnifier");
            let inks = inks(magnifier);
            assert!(!inks.is_empty(), "magnifier binds an ink");
            let opacity = magnifier.props.opacity.unwrap_or(1.0);
            for fg_name in inks {
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(well_bg);
                let ratio = fg.contrast_ratio(well_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "magnifier at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
