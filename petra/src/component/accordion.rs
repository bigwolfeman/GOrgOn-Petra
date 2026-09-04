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
//!
//! The item stretches every child to its own resolved width
//! ([`Align::Stretch`]). The divider is a childless swatch with no
//! intrinsic content, so without stretch it petrifies at a **0px width** —
//! declared paint content covering zero pixels, invisible even though every
//! `ViewNode`-level check (which never looks at a placed rect) passes. See
//! `the_divider_spans_the_full_item_width_not_a_zero_intrinsic_one` in this
//! module's own tests.

use super::stack;
use super::text::text;
use super::tokens::{BORDER_SUBTLE, LAYER_HOVER, SIZE_MD, SPACING_03, SPACING_05, SURFACE_BASE, t};
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
    // Resting fill matches the page it sits on ([`SURFACE_BASE`]) rather than
    // binding no `background` at all: an unbound slot with only
    // `background@hover` beside it declares content the paint pass cannot
    // resolve at rest, which the accounting counts as silent
    // (`gorgon_petra_egui::paint::PaintReport::silent`) — the same "no fill
    // fallthrough" every `chrome()` variant in [`super::button`] already
    // avoids by binding `SURFACE_BASE` under `Variant::Ghost`.
    header
        .props
        .tokens
        .insert("background".into(), t(SURFACE_BASE));
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
    item.props.align = Some(Align::Stretch);
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
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

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
        assert_eq!(
            token(header, "background"),
            Some(super::SURFACE_BASE),
            "a resting `background` must be bound alongside `background@hover`, \
             or the header paints nothing when it is not hovered — the paint \
             pass counts that as silent, not empty"
        );
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

    const VIEWPORT: Size = Size { w: 900.0, h: 700.0 };

    fn accepting_registry() -> Registry {
        Registry::with_vocabulary(standard_vocabulary())
    }

    fn petrify_lone(child: ViewNode) -> PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(child);
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

    /// Carbon's `_accordion.scss` `border-top: 1px solid $border-subtle`
    /// divider spans the full row. A childless swatch has no intrinsic
    /// content of its own to size against, so without cross-axis stretch on
    /// the item it resolves to a **0px-wide placement** — declares
    /// `background: border.subtle` (paint content) and covers zero pixels,
    /// the geometry-layer sibling of the token-layer "silent paint" defect
    /// the header's own `background@hover` fix guards against above. This is
    /// invisible at the `ViewNode`/constraints level (nothing here pins a
    /// width), which is why only a petrified frame's placement rects catch
    /// it — [`accordion_item_owns_a_one_px_border_subtle_divider`] above
    /// passed the whole time this shipped broken.
    #[test]
    fn the_divider_spans_the_full_item_width_not_a_zero_intrinsic_one() {
        let item = accordion_item("a", "Section A", true, "body text");
        let frame = petrify_lone(item);
        let find = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("no placement ending {suffix}"))
        };
        let header = find("/header");
        let divider = find("/divider");
        assert!(
            divider.rect.w > 0.0,
            "divider placed with a degenerate (zero) width: {:?}",
            divider.rect
        );
        assert_eq!(
            divider.rect.w, header.rect.w,
            "the divider should span the same width as the row above it, \
             not shrink to its own (empty) intrinsic content"
        );
    }

    /// Check C/D at the frame level, across every size and both expansion
    /// states: no placement collapses to a degenerate rect, and no child
    /// extends past the parent that placed it.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            (
                "md-open",
                accordion_item("a", "Section A", true, "The panel body."),
            ),
            (
                "md-closed",
                accordion_item("a", "Section A", false, "hidden"),
            ),
            (
                "sm-open",
                accordion_item_sm("a", "Small section", true, "body"),
            ),
            (
                "lg-open",
                accordion_item_lg("a", "Large section", true, "body"),
            ),
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

    /// Check F: the header declares `Focus` and must be reachable when
    /// enabled; accordion never disables a header, so this is the positive
    /// half only (there is no negative case to check).
    #[test]
    fn header_is_reachable_in_focus_order() {
        let item = accordion_item("a", "Section A", false, "hidden");
        let frame = petrify_lone(item);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let header = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/header"))
            .expect("header is placed");
        assert!(
            focus.order().iter().any(|id| id == &header.id),
            "header declares Focus but is not in focus order"
        );
    }

    /// Check E: title and chevron ink against the header's own resting fill
    /// (`surface.base`), read through `Props.opacity` (always 1.0 here, but
    /// composited rather than assumed so a future faded label is caught).
    #[test]
    fn header_text_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            let item = accordion_item("a", "Section A", false, "hidden");
            let header = named(&item, "header");
            let bg_name = header
                .props
                .tokens
                .get("background")
                .expect("header binds a resting background");
            let bg = color(&theme, bg_name.as_str());
            for label_key in ["title", "chevron"] {
                let label = named(header, label_key);
                let fg_name = label
                    .props
                    .tokens
                    .get("foreground")
                    .expect("label text binds a foreground");
                let opacity = label.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label_key} at {ratio:.2}:1 against {bg_name} fails AA {MIN_TEXT_CONTRAST}:1"
                );
            }
        }
    }
}
