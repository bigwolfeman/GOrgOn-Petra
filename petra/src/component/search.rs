//! Carbon Search (slice-d).
//!
//! Anatomy (`_search.scss`): field + decorative magnifier + typed value.
//! The close ("x") button is a filled-state affordance and is omitted
//! until a Clear control is wired; expandable is omitted (width animation
//! plus a collapsed icon-button).
//!
//! The magnifier is **not** the only channel: the field has a label
//! (FR-026). The magnifier itself has no role and no interactions.
//! `NodeKind::Input` is a leaf, so the magnifier sits as a sibling in the
//! well the way Number input's steppers do.
//!
//! Sizes: sm 32, md 40 (default), lg 48. Fill/edge is Petra's field pair.

use super::stack;
use super::text::text;
use super::tokens::{
    BORDER_SUBTLE, SHAPE_SM, SIZE_MD, SPACING_03, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY,
    TYPOGRAPHY_BODY, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, ViewNode};

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
    let mut magnifier = text("magnifier", "Search");
    magnifier
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));

    let mut well = stack(
        key,
        Axis::Horizontal,
        Some(SPACING_03),
        vec![magnifier, search_field("input", label, height)],
    );
    well.props.align = Some(Align::Center);
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
        .interactive(
            Role::TextInput,
            label,
            &[Interaction::Focus, Interaction::Key, Interaction::TextEdit],
        )
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
    use crate::tree::{Interaction, NodeKind, Role, ViewNode};

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
        assert_eq!(magnifier.props.text.as_deref(), Some("Search"));
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
}
