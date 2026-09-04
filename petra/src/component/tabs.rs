//! `tab` and `tab_bar` — Carbon Tabs (slice-e).
//!
//! Line is the default ([`tab`], [`tab_bar`]). Contained and Vertical are
//! extra constructors. Dismissible chrome and overflow-nav are omitted:
//! they need a close overlay and scroll buttons this library does not ship.
//! Callers wrap a tab with [`super::disabled`] to make it unavailable.

use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, ICON_DISABLED, LAYER_HOVER, SIZE_MD, SPACING_01, SPACING_03,
    SPACING_05, SURFACE_BASE, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING_SM, t,
};
use super::{pad, stack};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Constraints, Interaction, Key, NodeKind, Props, Role, Semantics, TrackSize,
    ViewNode,
};

/// Carbon line/contained tab height (`2.5rem`). [`SIZE_MD`] is that number.
const LINE_INDICATOR: f32 = 2.0;
/// Carbon vertical tab height (`4rem`). No size token for 64; local const.
const VERTICAL_HEIGHT: f32 = 64.0;
/// Carbon selected vertical indicator (`border-left: 3px`).
const VERTICAL_INDICATOR: f32 = 3.0;
/// A selected tab's label style: Carbon's `$heading-compact-01` — 14px
/// Medium, the *same size* as the unselected `$body-compact-01` label, with
/// only the weight stepping up (slice-e.md T101, MEASURED from SCSS: "14px
/// SemiBold selected vs 14px Regular unselected").
///
/// This is `"typography.heading-sm"` in [`super::tokens::TYPOGRAPHY_RAMP`]'s
/// own mapping table (`heading-sm` → Carbon's `heading-compact-01`), not
/// [`super::tokens::TYPOGRAPHY_HEADING`] — that constant is Carbon's
/// `heading-03` (20px), a page-heading role three sizes up from a tab label.
/// Binding it here is what made a selected tab's own label overflow its own
/// budget: nothing shrank, the label just grew 14px → 20px, six units taller
/// than the 40px tab has room for and wide enough to blow past its column.
/// No shipped component binds `heading-sm` yet, so `component::tokens` does
/// not export a constant for it; this file does not own that module, so the
/// name is declared locally, at the exact string `component::tokens` would
/// use if it did.

#[derive(Clone, Copy)]
enum Variant {
    Line,
    Contained,
    Vertical,
}

/// One Line tab. `selected` is declared in `Semantics` and shown as a 2px
/// underline keyed `"indicator"` — never colour alone.
///
/// Height is [`SIZE_MD`] (40). Padding is `$spacing-05` inline / `$spacing-03`
/// block. Unselected label is [`TEXT_MUTED`]; selected is [`TEXT_PRIMARY`]
/// plus [`TYPOGRAPHY_HEADING_SM`]. Fill stays [`SURFACE_BASE`] so
/// [`super::on_layer`] can seat an unselected tab flush with its ground.
///
/// Wrap with [`super::disabled`] for the unavailable state. The label already
/// binds `foreground@disabled`.
pub fn tab(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    tab_variant(key, label, selected, Variant::Line)
}

/// One Contained tab: filled strip, 2px *top* indicator, same 40px height.
///
/// Unselected fill is [`SURFACE_RAISED`]; selected is [`SURFACE_BASE`] (the
/// panel the tab sits on). Hover is [`LAYER_HOVER`].
pub fn contained_tab(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    tab_variant(key, label, selected, Variant::Contained)
}

/// One Vertical tab: 64px tall, 3px *left* indicator.
pub fn vertical_tab(key: impl Into<Key>, label: impl Into<String>, selected: bool) -> ViewNode {
    tab_variant(key, label, selected, Variant::Vertical)
}

/// The Line strip: already-built [`tab`] children under `Role::TabList`.
///
/// Carries no label and no interactions of its own. FR-058's role-and-label
/// obligation is about actionable nodes; each [`tab`] inside already carries
/// its own. Inter-tab gap is [`SPACING_01`] — Carbon's 1px margin has no
/// matching token, and this is the smallest shipped step rather than an
/// invented name.
pub fn tab_bar(key: impl Into<Key>, tabs: Vec<ViewNode>) -> ViewNode {
    tab_list(key, Axis::Horizontal, Some(SPACING_01), None, tabs)
}

/// Contained strip: flush tabs, [`SURFACE_RAISED`] fill, `Role::TabList`.
pub fn contained_tab_bar(key: impl Into<Key>, tabs: Vec<ViewNode>) -> ViewNode {
    tab_list(key, Axis::Horizontal, None, Some(SURFACE_RAISED), tabs)
}

/// Vertical strip: [`Axis::Vertical`], `Role::TabList`, no interactions.
pub fn vertical_tab_bar(key: impl Into<Key>, tabs: Vec<ViewNode>) -> ViewNode {
    tab_list(key, Axis::Vertical, None, Some(SURFACE_RAISED), tabs)
}

fn tab_variant(
    key: impl Into<Key>,
    label: impl Into<String>,
    selected: bool,
    variant: Variant,
) -> ViewNode {
    let key = key.into();
    let label = label.into();

    let mut label_node = text("label", label.clone());
    label_node.props.style = Some(t(if selected {
        TYPOGRAPHY_HEADING_SM
    } else {
        TYPOGRAPHY_BODY
    }));
    label_node.props.tokens.insert(
        "foreground".into(),
        t(if selected { TEXT_PRIMARY } else { TEXT_MUTED }),
    );
    label_node
        .props
        .tokens
        .insert("foreground@disabled".into(), t(ICON_DISABLED));

    let mut body = stack("body", Axis::Horizontal, None, vec![label_node]);
    body.props.align = Some(Align::Center);
    body.props.padding = Some(pad(SPACING_05, SPACING_03));

    let (along, thickness) = match variant {
        Variant::Vertical => (Axis::Vertical, VERTICAL_INDICATOR),
        Variant::Line | Variant::Contained => (Axis::Horizontal, LINE_INDICATOR),
    };
    let fill = match (variant, selected) {
        (_, true) => Some(ACCENT_PRIMARY),
        (Variant::Line, false) => Some(BORDER_SUBTLE),
        (Variant::Contained | Variant::Vertical, false) => None,
    };
    let mark = indicator_bar(along, thickness, fill);

    let (columns, rows, children, height) = match variant {
        Variant::Line => (
            vec![TrackSize::FitContent],
            vec![
                TrackSize::Weight { weight: 1.0 },
                TrackSize::Fixed {
                    value: LINE_INDICATOR,
                },
            ],
            vec![body, mark],
            SIZE_MD,
        ),
        Variant::Contained => (
            vec![TrackSize::FitContent],
            vec![
                TrackSize::Fixed {
                    value: LINE_INDICATOR,
                },
                TrackSize::Weight { weight: 1.0 },
            ],
            vec![mark, body],
            SIZE_MD,
        ),
        Variant::Vertical => (
            vec![
                TrackSize::Fixed {
                    value: VERTICAL_INDICATOR,
                },
                TrackSize::Weight { weight: 1.0 },
            ],
            vec![TrackSize::Weight { weight: 1.0 }],
            vec![mark, body],
            VERTICAL_HEIGHT,
        ),
    };

    let mut props = Props {
        columns,
        rows,
        align: Some(Align::Stretch),
        ..Props::default()
    };
    match variant {
        Variant::Line => {
            props.tokens.insert("background".into(), t(SURFACE_BASE));
        }
        Variant::Contained | Variant::Vertical => {
            props.tokens.insert("background".into(), t(SURFACE_RAISED));
            props
                .tokens
                .insert("background@hover".into(), t(LAYER_HOVER));
            props
                .tokens
                .insert("background@selected".into(), t(SURFACE_BASE));
            props
                .tokens
                .insert("background@selected-hover".into(), t(SURFACE_BASE));
        }
    }

    let intents: &[Interaction] = match variant {
        Variant::Line => &[Interaction::Focus, Interaction::Click],
        Variant::Contained | Variant::Vertical => {
            &[Interaction::Focus, Interaction::Click, Interaction::Hover]
        }
    };

    let mut node = ViewNode::new(NodeKind::Grid, key)
        .with_props(props)
        .with_children(children)
        .with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(height),
                max: Some(height),
                priority: 0,
            },
            ..Constraints::default()
        })
        .interactive(Role::Tab, label, intents);
    node.semantics.selected = selected;
    node
}

fn tab_list(
    key: impl Into<Key>,
    axis: Axis,
    spacing: Option<&str>,
    fill: Option<&str>,
    tabs: Vec<ViewNode>,
) -> ViewNode {
    let mut node = stack(key, axis, spacing, tabs);
    if let Some(name) = fill {
        node.props.tokens.insert("background".into(), t(name));
    }
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}

/// Selected (or Line-unselected) indicator. An empty stack, not a spacer:
/// a spacer answers Unbounded with 65535 and would blow a `FitContent`
/// column to viewport-width. An empty stack measures zero and Stretch
/// fills the cell, so the bar is as wide as the label and as thick as
/// `thickness`.
fn indicator_bar(along: Axis, thickness: f32, fill: Option<&str>) -> ViewNode {
    let mut node = stack("indicator", along, None, vec![]);
    if let Some(name) = fill {
        node.props.tokens.insert("background".into(), t(name));
    }
    match along {
        Axis::Horizontal => {
            node.constraints.vertical = AxisConstraint {
                min: Some(thickness),
                max: Some(thickness),
                priority: 0,
            };
        }
        Axis::Vertical => {
            node.constraints.horizontal = AxisConstraint {
                min: Some(thickness),
                max: Some(thickness),
                priority: 0,
            };
        }
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{TransitionActivity, Viewport, petrify};
    use crate::geom::Size;
    use crate::testing::{Harness, validated_with};
    use crate::token::{ThemeMode, standard_vocabulary};
    use crate::tree::{NodeKind, Registry};

    fn named<'a>(node: &'a ViewNode, key: &str) -> &'a ViewNode {
        fn walk<'a>(node: &'a ViewNode, key: &str) -> Option<&'a ViewNode> {
            if node.key.as_str() == key {
                return Some(node);
            }
            node.children.iter().find_map(|child| walk(child, key))
        }
        walk(node, key).unwrap_or_else(|| panic!("no descendant keyed `{key}`"))
    }

    fn child_keys(node: &ViewNode) -> Vec<&str> {
        node.children.iter().map(|c| c.key.as_str()).collect()
    }

    fn petrify_lone(child: ViewNode) -> crate::frame::PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(child);
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        crate::anim::shipped_registry().declare_into(&mut registry);
        let mut harness = Harness::new();
        let viewport = Viewport::new(Size { w: 400.0, h: 200.0 }, ThemeMode::Dark);
        harness.scale = viewport.scale;
        petrify(
            1,
            validated_with(&root, &registry),
            &mut harness.ctx(),
            viewport,
            TransitionActivity::default(),
        )
    }

    #[test]
    fn tab_sets_role_tab_label_and_actions() {
        let node = tab("t", "Fibers", false);
        assert_eq!(node.semantics.role, Some(Role::Tab));
        assert_eq!(node.semantics.label.as_deref(), Some("Fibers"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(node.is_interactive());
    }

    #[test]
    fn tab_bar_sets_role_tablist_and_has_no_interactions() {
        let node = tab_bar("strip", vec![tab("a", "A", true), tab("b", "B", false)]);
        assert_eq!(node.semantics.role, Some(Role::TabList));
        assert!(node.interactions.is_empty());
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
    }

    #[test]
    fn tab_declares_selected_in_semantics() {
        assert!(tab("t", "Fibers", true).semantics.selected);
        assert!(!tab("t", "Fibers", false).semantics.selected);
        assert!(contained_tab("t", "A", true).semantics.selected);
        assert!(!contained_tab("t", "A", false).semantics.selected);
        assert!(vertical_tab("t", "A", true).semantics.selected);
    }

    #[test]
    fn line_tab_height_is_size_md() {
        let node = tab("t", "Fibers", true);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.constraints.vertical.max, Some(SIZE_MD));
        assert_eq!(SIZE_MD, 40.0);

        let frame = petrify_lone(tab("t", "Fibers", true));
        let placed = frame
            .placements
            .iter()
            .find(|p| p.id.ends_with("/root/t"))
            .expect("the tab is missing from the petrified frame");
        assert_eq!(placed.rect.h, SIZE_MD);
    }

    #[test]
    fn selected_tab_has_an_indicator_child() {
        let selected = tab("t", "Fibers", true);
        let mark = named(&selected, "indicator");
        assert_eq!(
            mark.props.tokens.get("background").map(|t| t.as_str()),
            Some(ACCENT_PRIMARY)
        );
        assert!(child_keys(&selected).contains(&"indicator"));
        assert!(child_keys(&contained_tab("c", "A", true)).contains(&"indicator"));
        assert!(child_keys(&vertical_tab("v", "A", true)).contains(&"indicator"));
    }

    #[test]
    fn line_tab_puts_the_indicator_under_the_label() {
        assert_eq!(child_keys(&tab("t", "A", true)), ["body", "indicator"]);
        assert_eq!(
            named(&tab("t", "A", true), "label")
                .props
                .style
                .as_ref()
                .map(|t| t.as_str()),
            Some(TYPOGRAPHY_HEADING_SM)
        );
        assert_eq!(
            named(&tab("t", "A", false), "label")
                .props
                .style
                .as_ref()
                .map(|t| t.as_str()),
            Some(TYPOGRAPHY_BODY)
        );
        assert_eq!(
            tab("t", "A", false)
                .props
                .tokens
                .get("background")
                .map(|t| t.as_str()),
            Some(SURFACE_BASE)
        );
    }

    #[test]
    fn contained_tab_puts_the_indicator_on_top() {
        let node = contained_tab("t", "A", true);
        assert_eq!(child_keys(&node), ["indicator", "body"]);
        assert_eq!(node.constraints.vertical.min, Some(SIZE_MD));
        assert_eq!(node.semantics.role, Some(Role::Tab));
        assert!(node.interactions.contains(&Interaction::Hover));
        let bar = contained_tab_bar("strip", vec![node]);
        assert_eq!(bar.semantics.role, Some(Role::TabList));
        assert!(bar.interactions.is_empty());
        assert_eq!(bar.props.axis, Some(Axis::Horizontal));
    }

    #[test]
    fn vertical_tab_bar_is_a_vertical_tablist() {
        let node = vertical_tab("t", "A", true);
        assert_eq!(node.constraints.vertical.min, Some(VERTICAL_HEIGHT));
        assert_eq!(node.constraints.vertical.max, Some(VERTICAL_HEIGHT));
        assert_eq!(child_keys(&node), ["indicator", "body"]);
        assert_eq!(
            named(&node, "indicator").constraints.horizontal.min,
            Some(VERTICAL_INDICATOR)
        );
        let bar = vertical_tab_bar("strip", vec![node]);
        assert_eq!(bar.semantics.role, Some(Role::TabList));
        assert_eq!(bar.props.axis, Some(Axis::Vertical));
        assert!(bar.interactions.is_empty());
    }
}
