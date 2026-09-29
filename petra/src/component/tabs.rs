//! `tab` and `tab_bar` — Carbon Tabs (slice-e).
//!
//! Line is the default ([`tab`], [`tab_bar`]). Contained and Vertical are
//! extra constructors. Callers wrap a tab with [`super::disabled`] to make
//! it unavailable.
//!
//! A Line/Contained strip whose tabs overflow its width is Carbon's
//! scrollable state (see [`scrollable_row`]): it clips and is reachable by
//! wheel or Tab, never squeezes a tab under its own label. What is still
//! missing from that state is the two overflow-nav buttons (Carbon's
//! `ChevronLeft`/`ChevronRight`, `$spacing-08`/`09` hit targets) and their
//! CSS-gradient edge fade — this library has no gradient primitive, and the
//! buttons need to appear only when the strip actually overflows, which is
//! not yet knowable at the point a `ViewNode` tree is built (before
//! layout runs). Dismissible chrome (a close affordance per tab) is also
//! still omitted.

use super::text::text;
use super::tokens::{
    ACCENT_PRIMARY, BORDER_SUBTLE, ICON_DISABLED, LAYER_HOVER, SIZE_MD, SPACING_01, SPACING_03,
    SPACING_05, SURFACE_BASE, SURFACE_RAISED, TEXT_MUTED, TEXT_PRIMARY, TYPOGRAPHY_BODY,
    TYPOGRAPHY_HEADING_SM, t,
};
use super::{pad, stack};
use crate::geom::{Align, Axis};
use crate::tree::{
    AxisConstraint, Behaviour, Constraints, FocusFigure, Intent, Interaction, Key, NodeKind, Phase,
    Props, Role, Semantics, TrackSize, ViewNode,
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
        .interactive(Role::Tab, label, intents)
        // Spec 010: exclusive among siblings in the strip.
        .with_behaviour(Behaviour {
            intent: Intent::Select,
            phase: Phase::OnRelease,
        })
        .owning_its_text();
    node.semantics.selected = selected;
    // Carbon focuses a tab with the ring: `.cds--tabs__nav-link:focus` is
    // `@include focus-outline('outline')` — `outline: 2px solid $focus;
    // outline-offset: -2px`, a 2-unit stroke inside the tab's own edge
    // (`components/tabs/_tabs.scss:497-499`, and the same mixin again at
    // `:140`, `:421-423`, `:727-728` for the close button, the scroll
    // buttons and the panel).
    //
    // `mark` is a different rule. It is pinned to one of the tab's own
    // edges — 2 units at the bottom (Line), 2 at the top (Contained), 3 at
    // the left (Vertical) — from `.cds--tabs__nav-item--selected`'s
    // `border-block-end: 2px solid $border-interactive` under `// Item
    // Selected` (`_tabs.scss:593-596`). It says *which tab is current*, not
    // *where the keyboard is*.
    //
    // The two coexist even though `$focus` and `$border-interactive` are the
    // same value in the light themes (`#0f62fe`, `@carbon/themes` generated
    // `_themes.scss` `$white` and `$g10`), because a ring has four edges and
    // an indicator has one. Wherever the ring's bottom band lands on a Line
    // tab's indicator, its top, left and right bands are on pixels the
    // indicator never touches.
    // `a_tabs_ring_marks_edges_its_indicator_does_not` measures that.
    //
    // Which variant gets the ring is decided per variant, not once for the
    // strip, under the operator's rule of 2026-09-05 that a box is only for
    // where a bar will not fit:
    //
    // The operator's call of 2026-09-06 is no boxes on this row. What is
    // left is decided by geometry rather than taste: the selection indicator
    // already owns one edge of each variant, and the focus figure may not
    // share it, or the two accent marks read as one.
    //
    // * **Line** — the indicator is on the tab's own *bottom* edge, so a
    //   `BarInside` stripe would sit exactly on it and a selected tab would
    //   show no focus at all. `Sides` was the answer until 2026-09-06 and
    //   the operator photographed it clipping: the strip clips its own
    //   height, so bars standing `hug_gap` outside a tab are cut off at top
    //   and bottom. Their instruction is that an element already carrying a
    //   blue line on its bottom edge takes the bar *below* it, which is the
    //   one figure that neither collides nor clips: `BarUnder`.
    //
    //   It does put two accent lines two units apart on a selected tab.
    //   That was reported against this strip once, in the other direction —
    //   R6 wanted them further apart, not merged — and the shadow under the
    //   bar is what separates them now.
    // * **Contained** — the indicator is on the *top* edge and the panel
    //   below the strip is not a control, so the bottom edge is free.
    //   `BarUnder`, unchanged.
    // * **Vertical** — the indicator is on the *left*, so `Sides` would
    //   collide the way a bar collides on Line. The tabs also stack, and the
    //   measured gap between `tab-vert-0` and `tab-vert-1` is smaller than
    //   the five units `BarUnder` needs. Both outset figures are out, and
    //   the bottom edge is free: `BarInside`, which needs no run at all.
    node.semantics.focus_figure = match variant {
        Variant::Line => FocusFigure::BarUnder,
        Variant::Vertical => FocusFigure::BarInside,
        Variant::Contained => FocusFigure::BarUnder,
    };
    node
}

fn tab_list(
    key: impl Into<Key>,
    axis: Axis,
    spacing: Option<&str>,
    fill: Option<&str>,
    tabs: Vec<ViewNode>,
) -> ViewNode {
    let row = stack("row", axis, spacing, tabs);
    let content = match axis {
        // Carbon's scrollable state (slice-e.md, Tabs anatomy item D): see
        // `scrollable_row`'s own doc for the mechanism.
        Axis::Horizontal => scrollable_row(row),
        // Vertical tabs overflow on the other axis and are not this fix's
        // scope (`.agents/notes/proposed/bug-fix/2026-09-03-tab-strip-
        // compresses-tabs-below-their-labels.md`, Risks: "Vertical tabs
        // scroll on the other axis ... Doing only the horizontal case
        // leaves `vertical_tab_bar` with the same defect."). Left exactly
        // as it was before this fix.
        Axis::Vertical => row,
    };
    let mut node = ViewNode::new(NodeKind::Stack, key)
        .with_props(Props {
            axis: Some(axis),
            ..Props::default()
        })
        .child(content);
    if let Some(name) = fill {
        node.props.tokens.insert("background".into(), t(name));
    }
    node.semantics = Semantics {
        role: Some(Role::TabList),
        ..Semantics::default()
    };
    node
}

/// Carbon's scrollable state for a Line/Contained strip (slice-e.md, Tabs
/// anatomy item D): a real fix for the defect this module used to carry,
/// where `TrackSize::FitContent` on a tab's own column was a *preference*
/// the surrounding `Stack` could squeeze, not a floor
/// (`.agents/notes/proposed/bug-fix/2026-09-03-tab-strip-compresses-tabs-
/// below-their-labels.md`).
///
/// [`crate::layout::scroll::measure`] and `::place` always probe their
/// child with `Proposal::Unbounded` on the scrolling axis (that module's
/// own doc: "the scrolling axis always probes the content's maximum useful
/// extent, regardless of what this container was itself offered"), so
/// nothing downstream of this wrapper ever negotiates `row`'s width down to
/// what the strip has left — every tab answers its own natural
/// `FitContent` width, unconditionally. What does not fit is clipped by
/// the `Scroll`'s own placed rect (never placed outside it — `layout::
/// scroll::place`'s child is offset and clipped, not shrunk) and reachable
/// by the mouse wheel (`Host::apply_scroll`) or by Tab stepping past the
/// visible set (`FocusTree::reachable`, `Host::step_focus`) — both wired
/// by f5fd4a4, of which this is the first component-layer caller.
///
/// This is a different mechanism than the Agent Note's literal proposal
/// (a per-tab `Constraints` floor): that shape cannot work here because no
/// tab's natural width is knowable at tree-construction time — it depends
/// on font metrics `layout::measure` only has mid-pass. Wrapping in
/// `Scroll` gets the same floor as an emergent property of an `Unbounded`
/// proposal instead, and gets the "never placed outside its parent" half
/// of the Note's proposal for free from the same wrapper, since clipping
/// (not squeezing) is what a `Scroll` does to overflow by construction.
///
/// `Interaction::Scroll` is declared explicitly: a `Scroll` node gets no
/// interactions merely from its `NodeKind` (`ViewNode::new` always starts
/// `interactions: Vec::new()`), and without it `input::hit_test` finds no
/// placement at the pointer that accepts a wheel event — the same gap
/// `gallery/catalog.rs`'s `index`/`main-scroll` nodes already comment on.
fn scrollable_row(row: ViewNode) -> ViewNode {
    ViewNode::new(NodeKind::Scroll, "viewport")
        .with_props(Props {
            axis: Some(Axis::Horizontal),
            ..Props::default()
        })
        .child(row)
        .interactive(Role::Scroll, "Tab strip", &[Interaction::Scroll])
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
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::Size;
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
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
        petrify_lone_at(child, Size { w: 400.0, h: 200.0 })
    }

    /// [`petrify_lone`], parameterized on the viewport a lone child is
    /// petrified against — the one number [`scrollable_row`]'s own fix
    /// needs to control, to prove a strip narrower than its tabs clips
    /// rather than compresses them.
    fn petrify_lone_at(child: ViewNode, viewport_size: Size) -> crate::frame::PetrifiedFrame {
        let root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                ..Props::default()
            })
            .child(child);
        let mut registry = Registry::with_vocabulary(standard_vocabulary());
        crate::anim::shipped_registry().declare_into(&mut registry);
        let mut harness = Harness::new();
        let viewport = Viewport::new(viewport_size, ThemeMode::Dark);
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

    /// Why each variant's focus figure keeps off the edge its own selection
    /// indicator already owns, when the two are the same accent.
    ///
    /// The three land on three different edges, and each figure is picked to
    /// avoid it. **Line** pins its indicator to the tab's own bottom edge,
    /// which is exactly where a bar would hang and where a `BarInside`
    /// stripe would sit: two accent lines told apart only by width. Its left
    /// and right edges are free, so it brackets. **Vertical** pins its
    /// indicator to the *left*, so bracketing would collide there instead;
    /// it also stacks its tabs closer than the five units `BarUnder` needs,
    /// so it takes the contained stripe on its free bottom edge.
    /// **Contained** pins its indicator to the *top* edge and its strip has
    /// the panel below it, so it takes the default bar — this test asserts
    /// that too, at the end.
    ///
    /// Carbon's answer, and the citation this file used to get wrong:
    /// `.cds--tabs__nav-link:focus` is `focus-outline('outline')`
    /// (`_tabs.scss:497-499`). `_tabs.scss:596` is `// Item Selected`, a
    /// different rule for a different meaning.
    ///
    /// The property that makes the two legible together is **not** that
    /// their colours differ; in the white and g10 themes `$focus` and
    /// `$border-interactive` are both `#0f62fe`. It is that the ring marks
    /// three edges the indicator does not. So that is what this measures:
    /// for each variant, the ring's four edges, minus whichever one the
    /// indicator shares, must be clear of the indicator entirely.
    ///
    /// # How this goes red
    ///
    /// Trim `FocusRing::border_edges` to the one edge an indicator sits on
    /// — which is what "focus is a bar under the tab" amounted to — and the
    /// count of clear edges drops below three.
    #[test]
    fn a_tabs_ring_marks_edges_its_indicator_does_not() {
        use crate::geom::Rect;
        use crate::token::FocusRing;

        let ring = FocusRing::STANDARD;
        for (label, node, held) in [
            ("line", tab("t", "Fibers", true), "bottom"),
            ("vertical", vertical_tab("t", "Fibers", true), "left"),
        ] {
            assert_eq!(
                node.semantics.focus_figure,
                match label {
                    "line" => FocusFigure::BarUnder,
                    _ => FocusFigure::BarInside,
                },
                "{label}: the focus figure must keep off the {held} edge, \
                 which this variant's selection indicator already owns"
            );
            let frame = petrify_lone(node);
            let find = |suffix: &str| -> Rect {
                frame
                    .placements
                    .iter()
                    .find(|p| p.id.ends_with(suffix))
                    .unwrap_or_else(|| panic!("{label}: no placement ending in {suffix}"))
                    .rect
            };
            let tab_rect = find("/root/t");
            let mark = find("/root/t/indicator");
            let edges = ring.border_edges(tab_rect);
            let names = ["top", "right", "bottom", "left"];
            let area = |r: Rect| r.w * r.h;

            // Exactly one band is swallowed by the indicator: the one on the
            // edge this variant pins it to. Corners belong to two bands, so
            // the side bands do clip the indicator's ends — that is a few
            // square units out of a full-height band, not a covered band,
            // and the distinction is the whole point.
            let swallowed: Vec<&str> = names
                .iter()
                .zip(edges)
                .filter(|(_, edge)| area(edge.intersect(mark)) >= area(*edge))
                .map(|(name, _)| *name)
                .collect();
            assert_eq!(
                swallowed,
                vec![held],
                "{label}: the indicator {mark:?} swallows {swallowed:?} of \
                 the ring on {tab_rect:?}. One band may coincide with the \
                 selection mark; the rest are what focus has left to show."
            );

            // And each of the other three keeps pixels the indicator can
            // never paint, which is what tells the two marks apart on a tab
            // that is selected *and* focused. `$focus` and
            // `$border-interactive` are the same colour in the light themes,
            // so shape is the only channel carrying this, per FR-015.
            for (name, edge) in names.iter().zip(edges) {
                if *name == held {
                    continue;
                }
                assert!(
                    area(edge) > 0.0,
                    "{label}: the ring's {name} band is degenerate \
                     ({edge:?}), so the ring is not closed"
                );
                let own = area(edge) - area(edge.intersect(mark));
                assert!(
                    own > 0.0,
                    "{label}: the ring's {name} band {edge:?} is entirely \
                     inside the indicator {mark:?}, so focus paints nothing \
                     selection does not already paint"
                );
            }
        }

        // Contained is the variant that does *not* need the ring. Its
        // indicator is on the top edge, so the bottom edge — where a bar
        // hangs — is free, and the operator's rule then applies: a bar unless
        // a bar will not fit.
        let contained = contained_tab("t", "Fibers", true);
        assert_eq!(
            contained.semantics.focus_figure,
            FocusFigure::BarUnder,
            "a contained tab pins its indicator to the top edge, so nothing \
             is competing for the bottom one"
        );
        let frame = petrify_lone(contained);
        let rect = |suffix: &str| -> Rect {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("contained: no placement ending in {suffix}"))
                .rect
        };
        let bar = ring.bar(rect("/root/t"));
        let mark = rect("/root/t/indicator");
        assert_eq!(
            bar.intersect(mark).w * bar.intersect(mark).h,
            0.0,
            "the bar {bar:?} touches the contained indicator {mark:?}, which \
             is the two-lines defect this variant was chosen to avoid"
        );
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

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    fn check_geometry(frame: &PetrifiedFrame, label: &str) {
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

    /// Check C/D: `indicator` is exactly the childless-swatch shape that
    /// petrified 0px wide for the Accordion divider — `indicator_bar`
    /// builds an empty `Stack` with no children, pins only its own
    /// *thickness* axis via `Constraints`, and leaves the other axis to
    /// `Align::Stretch` on the surrounding `Grid` (this module's own doc on
    /// `indicator_bar`: "An empty stack, not a spacer ... Stretch fills the
    /// cell"). This is the frame-level check that proves it petrifies
    /// non-zero on both axes, across all three variants, selected and
    /// unselected, and the disabled form — not a substitute for reading
    /// the code.
    #[test]
    fn indicator_and_body_place_with_a_real_nonzero_rect_in_every_state() {
        let cases: Vec<(&str, ViewNode)> = vec![
            ("line-selected", tab("t", "Fibers", true)),
            ("line-unselected", tab("t", "Fibers", false)),
            (
                "line-disabled",
                crate::component::disabled(tab("t", "Fibers", false)),
            ),
            ("contained-selected", contained_tab("t", "Fibers", true)),
            ("contained-unselected", contained_tab("t", "Fibers", false)),
            ("vertical-selected", vertical_tab("t", "Fibers", true)),
            ("vertical-unselected", vertical_tab("t", "Fibers", false)),
        ];
        for (label, node) in cases {
            check_geometry(&petrify_lone(node), label);
        }
    }

    /// Check F: an enabled tab declares `Focus` and is reachable; a
    /// disabled one (wrapped by [`super::super::disabled`]) is not.
    #[test]
    fn a_disabled_tab_is_not_reachable_by_focus() {
        for (label, node, should_be_focusable) in [
            ("enabled", tab("t", "Fibers", true), true),
            (
                "disabled",
                crate::component::disabled(tab("t", "Fibers", false)),
                false,
            ),
        ] {
            let frame = petrify_lone(node);
            let focus = crate::focus::FocusTree::from_placements(
                &frame.placements,
                &std::collections::BTreeMap::new(),
            );
            let placement = frame
                .placements
                .iter()
                .find(|p| p.id.ends_with("/t"))
                .expect("the tab is placed");
            let reachable = focus.order().iter().any(|o| o == &placement.id);
            assert_eq!(
                reachable, should_be_focusable,
                "{label}: focus reachability was {reachable}, expected {should_be_focusable}"
            );
        }
    }

    /// Check E: the label against the tab's own resting fill, selected and
    /// unselected, in both themes. This is also the regression pin for the
    /// type-ramp defect this module's own doc records: a selected label
    /// bound to `TYPOGRAPHY_HEADING` (Carbon's 20px `heading-03`) instead
    /// of `TYPOGRAPHY_HEADING_SM` (`heading-compact-01`, still 14px) grew
    /// six units taller than the 40px tab has room for and overflowed —
    /// `check_geometry`'s `!p.paint.overflowed` assertion above is what
    /// would catch a repeat of it.
    #[test]
    fn label_clears_aa_contrast_against_its_own_tabs_resting_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node) in [
                ("line-selected", tab("t", "Fibers", true)),
                ("line-unselected", tab("t", "Fibers", false)),
                ("contained-selected", contained_tab("t", "Fibers", true)),
                ("contained-unselected", contained_tab("t", "Fibers", false)),
                ("vertical-selected", vertical_tab("t", "Fibers", true)),
                ("vertical-unselected", vertical_tab("t", "Fibers", false)),
            ] {
                let tab_bg_name = node
                    .props
                    .tokens
                    .get("background")
                    .unwrap_or_else(|| panic!("{label}: tab binds a resting background"));
                let tab_bg = color(&theme, tab_bg_name.as_str());
                let text_node = named(&node, "label");
                let fg_name = text_node
                    .props
                    .tokens
                    .get("foreground")
                    .unwrap_or_else(|| panic!("{label}: label binds a foreground"));
                let opacity = text_node.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(tab_bg);
                let ratio = fg.contrast_ratio(tab_bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    tab_bg_name.as_str()
                );
            }
        }
    }

    /// Check G: the fix `scrollable_row` exists for
    /// (`.agents/notes/proposed/bug-fix/2026-09-03-tab-strip-compresses-
    /// tabs-below-their-labels.md`), reproduced directly rather than only
    /// through the inspector's own shipped window.
    ///
    /// Four long labels, petrified once against a strip wide enough that
    /// nothing overflows and once against one narrow enough that the sum
    /// of their natural widths cannot fit. Falsified exactly as the Agent
    /// Note asks: shrink the strip until the tabs no longer fit and check
    /// that a clipped, unsquashed run appears rather than a compressed
    /// label.
    #[test]
    fn a_strip_narrower_than_its_tabs_clips_rather_than_compresses_them() {
        let long_tabs = || {
            vec![
                tab("t1", "Approvals pending review", false),
                tab("t2", "Recent leaks and findings", true),
                tab("t3", "History", false),
                tab("t4", "Logs", false),
            ]
        };
        let keys = ["t1", "t2", "t3", "t4"];

        let wide = petrify_lone_at(
            tab_bar("strip", long_tabs()),
            Size {
                w: 2000.0,
                h: 200.0,
            },
        );
        let narrow = petrify_lone_at(tab_bar("strip", long_tabs()), Size { w: 300.0, h: 200.0 });

        // No label draws content larger than its own rect in either
        // window: this is exactly `layout_overlap.rs`'s
        // `every_text_run_in_the_shipped_window_fits_the_box_it_was_given`,
        // reproduced at a size guaranteed to overflow rather than hoping
        // the shipped window still does.
        for (label, frame) in [("wide", &wide), ("narrow", &narrow)] {
            for p in frame.placements.iter().filter(|p| p.kind == NodeKind::Text) {
                assert!(
                    !p.paint.overflowed,
                    "{label}: {} drew content larger than its own rect",
                    p.id
                );
            }
        }

        // The floor half of the fix: every tab keeps the same width
        // whether the strip has room to spare or not. Before this fix the
        // narrow frame's two widest tabs (`t1`, `t2`) came out narrower
        // than in the wide frame — the exact squeeze the Agent Note
        // measured (188.91/158.25 down to 156.85 at 900x700).
        let width_of = |frame: &PetrifiedFrame, key: &str| -> f32 {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(&format!("/{key}")))
                .unwrap_or_else(|| panic!("tab `{key}` missing from the frame"))
                .rect
                .w
        };
        for key in keys {
            assert_eq!(
                width_of(&wide, key),
                width_of(&narrow, key),
                "tab `{key}`: width changed between the wide and narrow strip, so \
                 something between them is still squeezing it"
            );
        }

        // This is a real overflow, not a coincidence: the sum of what the
        // tabs actually took is wider than the viewport they are clipped
        // to, so the assertions above are proving something.
        let viewport = narrow
            .placements
            .iter()
            .find(|p| p.id.ends_with("/viewport"))
            .expect("the scroll viewport is placed");
        let tabs_total: f32 = keys.iter().map(|key| width_of(&narrow, key)).sum();
        assert!(
            tabs_total > viewport.rect.w,
            "the narrow window did not actually overflow (tabs total {tabs_total} <= \
             viewport {}), so this test proves nothing about the fix",
            viewport.rect.w
        );
    }
}
