//! `form` — Carbon Form (slice-b): a labelled vertical stack of fields.
//!
//! Carbon documents Form as a spacing/label shell, not a control. There is
//! no `Role::Group`, so this constructor sets no role and no interactions.
//! Legend colour is `$text-secondary` → [`TEXT_MUTED`].
//!
//! # Two gaps, and a `stack` has one
//!
//! Carbon's form has **two** vertical gaps and they are not the same size:
//!
//! - Legend to the first item: the legend is a `.cds--label`, whose
//!   `margin-block-end` is `$spacing-03` (8) — slice-b:227 anatomy 2a.
//! - Item to item: `margin-bottom` `$spacing-07` (32) — slice-b:230.
//!
//! Measured on `ignored/carbon-ref/shots/13-form.png`, that is 9 units
//! between the legend's line box and the label's, and 27 between the
//! field's bottom rule and the checkbox's box. This file shipped one
//! uniform `$spacing-05` (16) for both, under a comment that called 16 "the
//! constructor contract" — a contract with nobody: no spec, no gate and no
//! other test ever named the number, and Carbon is the conformance target
//! (spec 005, open decision 0).
//!
//! A `Stack` carries exactly one gap, so this builds the small one and
//! stands a [`ITEM_FILLER`]-tall [`NodeKind::Spacer`] between consecutive
//! items to make up the large one: `8 + 16 + 8 = 32`.
//!
//! **A nesting level would be tidier and is deliberately not used.** The
//! obvious shape is `legend` beside an `items` stack of its own, which is
//! what Carbon's `<fieldset>` is. It would move every field one segment
//! deeper, and `component/tests.rs:1424` pins
//! `root/carbon2/fm-signup/fm-name` as a **direct** child of the form. That
//! file belongs to another wave this round, so the spacer is the shape that
//! fixes the geometry without reaching into it. Collapsing the two into an
//! `items` stack is a one-line change here plus that one path there, and
//! whoever does it should.
//!
//! # What this file does *not* do
//!
//! It sets no cross-axis `align`. Round 2 recorded row 13's field as hugging
//! its own content; measured on 2026-09-05 it does not, at either level —
//! `a_labelled_form_item_fills_the_form_it_sits_in` holds with the align
//! declared and with it absent, because a `stack` measures an `Input` leaf
//! at the cross extent it was offered. The line was written, falsified, and
//! taken out again rather than left in as a claim nothing tests.
//!
//! # The two variants
//!
//! [`form_fluid`] and [`form_two_column`] keep this shell's legend slot and
//! change only the item rhythm (flush against one shared rule; row-major in
//! two 32-guttered columns). Their measurements live in their own tests,
//! not in the summary above, which describes [`form`].

use super::tokens::{BORDER_SUBTLE, SPACING_03, SPACING_07, TEXT_MUTED, t};
use super::{rule, stack, text::text};
use crate::geom::Axis;
use crate::tree::{AxisConstraint, Constraints, Key, NodeKind, Props, TrackSize, ViewNode};

/// What stands between two consecutive form items, so that the `Stack`'s
/// own `$spacing-03` on each side of it adds up to Carbon's `$spacing-07`
/// item gap: `32 - 2 x 8`. See the module doc for why a spacer and not a
/// nesting level.
const ITEM_FILLER: f32 = 16.0;

const _: () = assert!(ITEM_FILLER == 32.0 - 2.0 * 8.0);

/// A vertical field group with a muted legend and no role.
///
/// `children` are the fields. The legend is a `text` child keyed `"legend"`.
/// Not interactive: Form does not submit, focus, or click.
pub fn form(key: impl Into<Key>, legend: impl Into<String>, children: Vec<ViewNode>) -> ViewNode {
    let mut rows = Vec::with_capacity(children.len() * 2 + 1);
    rows.push(legend_text(legend));
    for (i, child) in children.into_iter().enumerate() {
        if i > 0 {
            rows.push(item_filler(i - 1));
        }
        rows.push(child);
    }
    stack(key, Axis::Vertical, Some(SPACING_03), rows)
}

/// The blank between form item `index` and the one after it.
fn item_filler(index: usize) -> ViewNode {
    ViewNode::new(NodeKind::Spacer, format!("gap{index}")).with_constraints(Constraints {
        vertical: AxisConstraint {
            min: Some(ITEM_FILLER),
            max: Some(ITEM_FILLER),
            priority: 0,
        },
        ..Constraints::default()
    })
}

/// Carbon's fluid form (`Form / fluid`) — the item rhythm of
/// `.cds--form--fluid`, whose `.cds--form-item { margin-bottom: 0 }`
/// (`_form.scss:142`) stacks items flush so their shared 1px borders are the
/// gutter: consecutive items meet at one hairline rule, not 32px of air.
///
/// This shell therefore owns exactly the two things that difference asks
/// for: the flush stack (gap 0, spelled as no spacing token, which
/// [`crate::tree::props::DEFAULT_SPACING`] resolves to) and the one
/// [`BORDER_SUBTLE`] rule
/// between consecutive items — a drawn separator, because two siblings in a
/// view tree cannot literally share a CSS border edge, and folding one rule
/// in keeps the gutter visible in the frame record instead of implicit in
/// two boxes happening to touch. That rule is the border material's groove
/// (2 shadow + 2 highlight device pixels — [`crate::token::rule`]'s
/// rendering of Carbon's 1px shared border), so the gutter it makes is
/// exactly one rule thick and nothing more, measured in
/// `fluid_form_items_meet_at_one_shared_rule`.
///
/// The 64px input height is a fluid *field's* business (`field_fluid`,
/// `date_picker_fluid`); this shell deliberately does not re-pin child
/// extents the way `menu_at` re-pins its rows, because form items
/// carry their own validated stacks (`field_with`, `date_picker_with`) that
/// a blanket extent would flatten.
///
/// The fieldset legend keeps its `spacing-03` (8px) above the first item,
/// as in [`form`]: the fluid step is in the items' rhythm, not the title's.
pub fn form_fluid(
    key: impl Into<Key>,
    legend: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let key = key.into();
    let n = items.len();
    let mut flush = Vec::with_capacity(n * 2);
    for (i, item) in items.into_iter().enumerate() {
        flush.push(item);
        if i + 1 < n {
            flush.push(rule(format!("gap{i}"), Axis::Horizontal, BORDER_SUBTLE));
        }
    }
    let rows = vec![
        legend_text(legend),
        stack(format!("{}-items", key), Axis::Vertical, None, flush),
    ];
    stack(key, Axis::Vertical, Some(SPACING_03), rows)
}

/// Carbon's two-column form (`Form / two-column`) — fields flowing row-major
/// over two equal-weight columns with Carbon's form margins on both axes:
/// `$spacing-07` (32px) down the rows (`margin-block-end` on `.cds--form-item`,
/// `_form.scss:85`) and the same 32px as the column gutter (a form item's
/// trailing half-gutter pattern, `_form.scss:89`).
///
/// Children land row-major, exactly as `pagination_nav`'s `page_grid` seats
/// the pagination nav: item 0 top-left, item 1 top-right, item 2 on the
/// second row's left. An odd item count leaves the last row's right cell
/// empty — a grid hole is honest; stretching the last item across both
/// columns would make its 50% field width a claim the author never typed.
///
/// The fieldset legend sits above the grid, whole-width, at `spacing-03`;
/// the 32s live between the items, not under the title.
pub fn form_two_column(
    key: impl Into<Key>,
    legend: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    let key = key.into();
    let grid = ViewNode::new(NodeKind::Grid, format!("{}-items", key)).with_props(Props {
        columns: vec![
            TrackSize::Weight { weight: 1.0 },
            TrackSize::Weight { weight: 1.0 },
        ],
        column_spacing: Some(t(SPACING_07)),
        row_spacing: Some(t(SPACING_07)),
        ..Props::default()
    });
    let grid = grid.with_children(items);
    stack(
        key,
        Axis::Vertical,
        Some(SPACING_03),
        vec![legend_text(legend), grid],
    )
}

/// The muted fieldset legend both variants share with [`form`].
fn legend_text(legend: impl Into<String>) -> ViewNode {
    let mut legend_node = text("legend", legend);
    legend_node
        .props
        .tokens
        .insert("foreground".into(), t(TEXT_MUTED));
    legend_node
}

#[cfg(test)]
mod tests {
    use super::form;
    use crate::component::field::{field, field_labeled};
    use crate::component::tests::{assert_fits_parent, petrify_lone};
    use crate::component::tokens::{SPACING_03, TEXT_MUTED};

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{NodeKind, ViewNode};

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

    /// Carbon's two gaps, measured in the frame rather than read off the
    /// props: the legend sits `$spacing-03` (8) above the first item and
    /// consecutive items sit `$spacing-07` (32) apart (slice-b:227, :230).
    /// This asserted one uniform `spacing-05` until 2026-09-05 and cited a
    /// "constructor contract" that no spec or gate ever wrote down.
    #[test]
    fn form_puts_carbons_two_gaps_between_its_legend_and_its_items() {
        let node = form(
            "signup",
            "Account",
            vec![field("name", "Name"), field("email", "Email")],
        );
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.props.axis, Some(crate::geom::Axis::Vertical));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_03)
        );

        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let legend = rect("/signup/legend");
        let first = rect("/signup/name");
        let second = rect("/signup/email");
        assert_eq!(
            first.y - legend.bottom(),
            8.0,
            "the legend's gap is `$spacing-03`, not the item gap"
        );
        assert_eq!(
            second.y - first.bottom(),
            32.0,
            "consecutive items are `$spacing-07` apart"
        );
    }

    /// Row 13, round 3. The guard for the claim round 2 made and this
    /// round refuted: that this row's field hugs its own content.
    ///
    /// It does not, and it did not — a `stack` measures an `Input` leaf at
    /// the cross extent it was offered, so a labelled item and a bare one
    /// both come out the form's own width. Kept as a regression guard
    /// rather than deleted: the claim will be made again, and this is the
    /// measurement that answers it.
    #[test]
    fn a_labelled_form_item_fills_the_form_it_sits_in() {
        let node = form(
            "signup",
            "Account",
            vec![field_labeled("name", "Name"), field("email", "Email")],
        );
        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let form_rect = rect("/signup");
        let labelled = rect("/signup/name");
        let bare = rect("/signup/email");
        assert_eq!(
            labelled.w, form_rect.w,
            "the labelled item hugged its own label instead of filling the form"
        );
        assert_eq!(bare.w, form_rect.w, "the bare field stopped filling too");
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
                assert_fits_parent("", p, &frame.placements[parent_idx]);
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

    /// The fluid variant's whole difference from [`form`], measured in the
    /// frame: `.cds--form-item { margin-bottom: 0 }` (`_form.scss:142`)
    /// makes consecutive items meet at their shared border, so what sits
    /// between two items is exactly one rule — the `border.subtle` groove,
    /// 2 shadow + 2 highlight device pixels — and no slack either side of
    /// it, against the 32 of the standard rhythm. The legend keeps its
    /// `spacing-03` above the first item.
    #[test]
    fn fluid_form_items_meet_at_one_shared_rule() {
        use super::form_fluid;
        let node = form_fluid(
            "fluid",
            "Fluid",
            vec![
                field("name", "Name"),
                field("email", "Email"),
                field("org", "Org"),
            ],
        );
        assert_eq!(node.kind, NodeKind::Stack);
        // Structural half of the claim: the rules between items are drawn
        // separators bound to `border.subtle`, and the flush stack carries no
        // spacing token at all (absence = 0 gap).
        let items = node.children[1].as_ref();
        assert_eq!(items.children.len(), 5, "three items and two hairlines");
        for i in [1, 3] {
            let rule = items.children[i].as_ref();
            assert_eq!(rule.kind, NodeKind::Separator, "the gutter is a drawn rule");
            assert_eq!(
                token(rule, "background"),
                Some(crate::component::tokens::BORDER_SUBTLE),
                "the divider is the `border.subtle` material"
            );
        }
        assert!(
            items.props.spacing.is_none(),
            "the flush stack asks for no gap; absence resolves to 0"
        );

        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let legend = rect("/legend");
        let first = rect("/name");
        let second = rect("/email");
        let third = rect("/org");
        let rule0 = rect("/gap0");
        assert_eq!(
            first.y - legend.bottom(),
            8.0,
            "the legend's gap is `$spacing-03`, as in `form`"
        );
        assert_eq!(
            rule0.h,
            crate::token::rule::total_units(),
            "the divider is one groove (2 shadow + 2 highlight device px), \
             this library's border.subtle rendering of Carbon's 1px shared border"
        );
        assert_eq!(
            second.y - first.bottom(),
            rule0.h,
            "flush items are separated by exactly the rule and nothing else"
        );
        assert_eq!(
            third.y - second.bottom(),
            rect("/gap1").h,
            "every consecutive pair is flush, not just the first"
        );
    }

    /// The two-column variant's layout, measured in the frame: items flow
    /// row-major over two equal columns with `$spacing-07` on both axes —
    /// 32 across the gutter and 32 down the rows (`_form.scss:85,89`) — and
    /// the third item wraps back under the first instead of flowing on to a
    /// third column.
    #[test]
    fn two_column_form_flows_row_major_with_32_on_both_axes() {
        use super::form_two_column;
        let node = form_two_column(
            "two",
            "Two",
            vec![
                field("a", "A"),
                field("b", "B"),
                field("c", "C"),
                field("d", "D"),
            ],
        );
        let frame = petrify_lone(node);
        let rect = |suffix: &str| {
            frame
                .placements
                .iter()
                .find(|p| p.id.ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix} is missing from the frame"))
                .rect
        };
        let (a, b, c, d) = (rect("/a"), rect("/b"), rect("/c"), rect("/d"));
        assert_eq!(a.w, b.w, "the columns are equal-weight");
        assert_eq!(a.y, b.y, "the first two items share a row");
        assert_eq!(b.x - a.right(), 32.0, "the column gutter is `$spacing-07`");
        assert_eq!(c.y - a.bottom(), 32.0, "the row gap is `$spacing-07`");
        assert_eq!(c.x, a.x, "the third item wraps back under the first");
        assert_eq!(d.y, c.y, "the second row is also a pair");
        assert_eq!(d.x, b.x, "and its right item sits in the right column");
    }
}
