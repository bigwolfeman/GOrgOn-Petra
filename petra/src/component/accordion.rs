//! Carbon Accordion (slice-a).
//!
//! Anatomy (docs + `_accordion.scss`; T070 prefers SCSS):
//! 1. [`accordion`] — the list container (`Role::List`).
//! 2. [`accordion_item`] — one `<li>`; owns the 1px [`BORDER_SUBTLE`]
//!    divider.
//! 3. Header button — the whole click/focus target (`Role::Button`).
//! 4. Chevron as text (`"expanded"` / `"collapsed"`), not an icon-only
//!    mark (FR-026).
//! 5. Title — the label, `body` type.
//! 6. Body — present only while expanded.
//!
//! Header height is the shared layout scale, clamped sm..lg: 32 / 40
//! (default, [`SIZE_MD`]) / 48. Hover fill is [`LAYER_HOVER`]; `Hover`
//! is declared so that slot is reachable.

use super::stack;
use super::text::text;
use super::tokens::{BORDER_SUBTLE, LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, t};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, InsetRefs, Interaction, Key, Role, Semantics, ViewNode,
};

/// Carbon accordion header `sm` (`layout.use` min).
const HEIGHT_SM: f32 = 32.0;
/// Carbon accordion header `lg` (`layout.use` max).
const HEIGHT_LG: f32 = 48.0;
/// Item divider: `border-top: 1px solid $border-subtle`.
const DIVIDER: f32 = 1.0;

const _: () = assert!(HEIGHT_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(HEIGHT_LG == 48.0);
const _: () = assert!(DIVIDER == 1.0);

const HEADER_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A vertical stack of accordion items. `Role::List`, no interactions.
pub fn accordion(key: impl Into<Key>, items: Vec<ViewNode>) -> ViewNode {
    let mut node = stack(key, Axis::Vertical, None, items);
    node.semantics = Semantics {
        role: Some(Role::List),
        ..Semantics::default()
    };
    node
}

/// One accordion item at the default header height ([`SIZE_MD`] / 40).
///
/// `label` is the header's accessible name (FR-058). `body` is shown only
/// when `expanded` is true. Expansion is a declared fact
/// (`Semantics.expanded`) plus the word `"expanded"` / `"collapsed"`.
pub fn accordion_item(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    accordion_item_sized(key, label, expanded, body, SIZE_MD)
}

/// [`accordion_item`] at Carbon `sm` (header 32).
pub fn accordion_item_sm(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    accordion_item_sized(key, label, expanded, body, HEIGHT_SM)
}

/// [`accordion_item`] at Carbon `lg` (header 48).
pub fn accordion_item_lg(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
) -> ViewNode {
    accordion_item_sized(key, label, expanded, body, HEIGHT_LG)
}

fn accordion_item_sized(
    key: impl Into<Key>,
    label: impl Into<String>,
    expanded: bool,
    body: impl Into<String>,
    header_h: f32,
) -> ViewNode {
    let label = label.into();
    let disclosure = if expanded { "expanded" } else { "collapsed" };

    let mut header = stack(
        "header",
        Axis::Horizontal,
        Some(SPACING_03),
        vec![text("title", label.clone()), text("chevron", disclosure)],
    );
    header.props.align = Some(Align::Center);
    header.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    header
        .props
        .tokens
        .insert("background@hover".into(), t(LAYER_HOVER));
    let mut header = header.with_constraints(pin_height(header_h)).interactive(
        Role::Button,
        label.clone(),
        HEADER_INTENTS,
    );
    header.semantics.expanded = Some(expanded);

    let mut children = vec![header];
    if expanded {
        let mut panel = stack(
            "body",
            Axis::Vertical,
            None,
            vec![text("body-text", body.into())],
        );
        // Carbon panel: padding-top 8 (`$spacing-03`), padding-inline 16
        // (`$spacing-05`). `$spacing-06` (24) bottom is not in `tokens`.
        panel.props.padding = Some(InsetRefs {
            top: Some(t(SPACING_03)),
            bottom: Some(t(SPACING_05)),
            left: Some(t(SPACING_05)),
            right: Some(t(SPACING_05)),
        });
        children.push(panel);
    }
    children.push(divider());

    let mut item = stack(key, Axis::Vertical, None, children);
    item.semantics = Semantics {
        role: Some(Role::ListItem),
        label: Some(label),
        ..Semantics::default()
    };
    item.props.tokens.insert("border".into(), t(BORDER_SUBTLE));
    item
}

/// 1px hairline. An empty stack, not a spacer: a spacer answers Unbounded
/// with 65535 and would blow the item to viewport-width.
fn divider() -> ViewNode {
    let mut node = stack("divider", Axis::Horizontal, None, vec![]);
    node.props
        .tokens
        .insert("background".into(), t(BORDER_SUBTLE));
    node.constraints.vertical = AxisConstraint {
        min: Some(DIVIDER),
        max: Some(DIVIDER),
        priority: 0,
    };
    node
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

#[cfg(test)]
mod tests {
    use super::{BORDER_SUBTLE, LAYER_HOVER};
    use super::{
        DIVIDER, HEIGHT_LG, HEIGHT_SM, SIZE_MD, accordion, accordion_item, accordion_item_lg,
        accordion_item_sm,
    };
    use crate::tree::{Interaction, Role, ViewNode};

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

    #[test]
    fn accordion_sets_role_list_and_is_not_interactive() {
        let node = accordion(
            "acc",
            vec![accordion_item("a", "Section A", false, "hidden")],
        );
        assert_eq!(node.semantics.role, Some(Role::List));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(node.key.as_str(), "acc");
    }

    #[test]
    fn accordion_item_header_is_a_labelled_button() {
        let item = accordion_item("a", "Section A", false, "hidden");
        assert_eq!(item.semantics.role, Some(Role::ListItem));
        assert_eq!(item.semantics.label.as_deref(), Some("Section A"));
        assert!(!item.is_interactive());

        let header = named(&item, "header");
        assert_eq!(header.semantics.role, Some(Role::Button));
        assert_eq!(header.semantics.label.as_deref(), Some("Section A"));
        assert!(header.interactions.contains(&Interaction::Focus));
        assert!(header.interactions.contains(&Interaction::Click));
        assert!(
            header.interactions.contains(&Interaction::Hover),
            "Hover is what makes background@hover reachable"
        );
        assert_eq!(token(header, "background@hover"), Some(LAYER_HOVER));
    }

    #[test]
    fn accordion_item_header_height_is_size_md() {
        let item = accordion_item("a", "Section A", false, "hidden");
        let header = named(&item, "header");
        assert_eq!(header.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(header.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);
    }

    #[test]
    fn accordion_item_sm_and_lg_pin_carbon_header_heights() {
        let sm = accordion_item_sm("a", "Small", false, "x");
        assert_eq!(
            named(&sm, "header").constraints.vertical.min,
            Some(HEIGHT_SM)
        );
        assert_eq!(HEIGHT_SM, 32.0);
        let lg = accordion_item_lg("a", "Large", false, "x");
        assert_eq!(
            named(&lg, "header").constraints.vertical.min,
            Some(HEIGHT_LG)
        );
        assert_eq!(HEIGHT_LG, 48.0);
    }

    #[test]
    fn accordion_item_declares_expanded_and_keeps_a_text_chevron() {
        let open = accordion_item("a", "Section A", true, "the rest");
        let header = named(&open, "header");
        assert_eq!(header.semantics.expanded, Some(true));
        assert_eq!(
            named(&open, "chevron").props.text.as_deref(),
            Some("expanded")
        );
        named(&open, "body");
        assert!(
            open.children.len() > 2,
            "expanded item keeps header, body, and divider"
        );

        let shut = accordion_item("a", "Section A", false, "the rest");
        assert_eq!(named(&shut, "header").semantics.expanded, Some(false));
        assert_eq!(
            named(&shut, "chevron").props.text.as_deref(),
            Some("collapsed")
        );
        assert!(
            shut.children
                .iter()
                .all(|child| child.key.as_str() != "body"),
            "collapsed item drops the body child"
        );
    }

    #[test]
    fn accordion_item_owns_a_one_px_border_subtle_divider() {
        let item = accordion_item("a", "Section A", false, "hidden");
        assert_eq!(token(&item, "border"), Some(BORDER_SUBTLE));
        let line = named(&item, "divider");
        assert_eq!(token(line, "background"), Some(BORDER_SUBTLE));
        assert_eq!(line.constraints.vertical.min, Some(DIVIDER));
        assert_eq!(line.constraints.vertical.max, Some(DIVIDER));
        assert_eq!(DIVIDER, 1.0);
    }
}
