//! Carbon Contained list (slice-a).
//!
//! Not [`super::list`]. This is a header + row list for related content
//! inside a container (card, sidebar, popover).
//!
//! Anatomy (docs + `_contained-list.scss`):
//! 1. [`contained_list`] — outer container, `Role::List`.
//! 2. Header — on-page height follows the size step (32/40/48 at
//!    sm/md/lg, 48 at xl); the disclosed header is its own fixed 32
//!    ([`contained_list_disclosed`]).
//! 3. Title — `heading` on-page, muted `text` when disclosed.
//! 4. Rows — caller-supplied children, stacked under the header, re-pinned
//!    to the step's row height because the step sizes rows too.
//!
//! On-page header has no fill. Disclosed header fills [`SURFACE_RAISED`].

use super::pin_block;
use super::stack;
use super::text::{heading, text};
use super::tokens::{
    BORDER_SUBTLE, SIZE_LG, SIZE_MD, SIZE_SM, SIZE_XL, SPACING_05, SURFACE_RAISED, TEXT_MUTED, t,
};
use crate::geom::{Align, Axis};
use crate::tree::{InsetRefs, Key, Role, Semantics, ViewNode};

/// Carbon's on-page contained-list ramp — slice-a "Sizes": `sm` 32, `md`
/// 40, `lg` 48, with **header and row matching at each of those**, and
/// `xl` header 48 / row 64 (the only step where the two diverge).
/// Closed on purpose, the same trade [`super::content_switcher`]'s
/// `SwitcherSize` records: a step nobody wired is a compile error, not a
/// silently missing row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListSize {
    Sm,
    Md,
    Lg,
    Xl,
}

impl ListSize {
    /// The on-page header height for this step.
    fn header(self) -> f32 {
        match self {
            Self::Sm => SIZE_SM,
            Self::Md => SIZE_MD,
            Self::Lg | Self::Xl => SIZE_LG,
        }
    }

    /// The row height for this step.
    fn row(self) -> f32 {
        match self {
            Self::Sm => SIZE_SM,
            Self::Md => SIZE_MD,
            Self::Lg => SIZE_LG,
            Self::Xl => SIZE_XL,
        }
    }
}

/// Disclosed header: 32 (slice-a "Disclosed: single named step, `lg`,
/// header 32px / row 48px"). The named step is the ROW's height — the
/// pre-T033 tree read `lg` as the header's and pinned the header at 48,
/// which is the row figure. `lg` 32 = Carbon's `$spacing-07`.
const HEADER_DISCLOSED: f32 = 32.0;
/// Disclosed row height: 48 (`lg`).
const ROW_DISCLOSED: f32 = 48.0;

const _: () = assert!(SIZE_SM == 32.0);
const _: () = assert!(SIZE_MD == 40.0);
const _: () = assert!(SIZE_LG == 48.0);
const _: () = assert!(SIZE_XL == 64.0);
const _: () = assert!(HEADER_DISCLOSED == 32.0);
const _: () = assert!(ROW_DISCLOSED == 48.0);

/// On-page contained list at Carbon md (40): header 40, rows re-pinned to
/// 40, no header fill, `Role::List`.
pub fn contained_list(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    at(key, title, items, ListSize::Md)
}

/// On-page contained list at Carbon sm (32).
pub fn contained_list_sm(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    at(key, title, items, ListSize::Sm)
}

/// On-page contained list at Carbon lg (48).
pub fn contained_list_lg(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    at(key, title, items, ListSize::Lg)
}

/// On-page contained list at Carbon xl: header 48, rows 64 — the one step
/// where the two heights diverge.
pub fn contained_list_xl(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    at(key, title, items, ListSize::Xl)
}

/// Disclosed contained list: header 32, rows 48, [`SURFACE_RAISED`] header
/// fill. Slice-a's one disclosed step, `lg`.
pub fn contained_list_disclosed(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
) -> ViewNode {
    contained(key, title, items, HEADER_DISCLOSED, ROW_DISCLOSED, true)
}

/// The list owns its size step: whatever height a caller's row arrives
/// with, the list re-pins it to the step's row height (slice-a "Sizes":
/// "header AND row all match"), the same rule [`super::content_switcher`]
/// applies to its tabs.
fn at(
    key: impl Into<Key>,
    title: impl Into<String>,
    items: Vec<ViewNode>,
    size: ListSize,
) -> ViewNode {
    contained(key, title, items, size.header(), size.row(), false)
}

fn contained(
    key: impl Into<Key>,
    title: impl Into<String>,
    mut items: Vec<ViewNode>,
    header_h: f32,
    row_h: f32,
    disclosed: bool,
) -> ViewNode {
    for item in &mut items {
        item.constraints = pin_block(row_h);
    }
    let title = title.into();
    let title_node = if disclosed {
        let mut node = text("title", title.clone());
        node.props.tokens.insert("foreground".into(), t(TEXT_MUTED));
        node
    } else {
        heading("title", title.clone())
    };

    let mut header = stack("header", Axis::Horizontal, None, vec![title_node]);
    header.props.align = Some(Align::Center);
    // Height is pinned; only the documented 16px inline pad is bound.
    header.props.padding = Some(InsetRefs {
        left: Some(t(SPACING_05)),
        right: Some(t(SPACING_05)),
        ..InsetRefs::default()
    });
    if disclosed {
        header
            .props
            .tokens
            .insert("background".into(), t(SURFACE_RAISED));
    }
    let header = header.with_constraints(pin_block(header_h));

    // Carbon separates the header from the first row and each row from the
    // next with `1px solid $border-subtle`
    // (`.cds--contained-list-item { border-block-end }`). Without them the
    // list is three lines of prose in a box, which is what "this is just
    // markdown, wtf" was about — the round 1 fix put each entry on its own
    // line and stopped there.
    //
    // Between, not after: a rule under the last row would draw a boundary
    // with nothing on the far side of it.
    let mut children = Vec::with_capacity(items.len() * 2 + 1);
    children.push(header);
    for (i, item) in items.into_iter().enumerate() {
        children.push(super::rule(
            format!("rule-{i}"),
            Axis::Horizontal,
            BORDER_SUBTLE,
        ));
        children.push(item);
    }

    let mut node = stack(key, Axis::Vertical, None, children);
    // `Align::Stretch`: the same stair-step fix as `accordion` (see that
    // module's doc). Without it the header (sized to its own title text) and
    // each caller-supplied row measure to their own intrinsic widths and the
    // list's edges do not line up.
    node.props.align = Some(Align::Stretch);
    node.semantics = Semantics {
        role: Some(Role::List),
        label: Some(title),
        ..Semantics::default()
    };
    node
}

#[cfg(test)]
mod tests {
    use super::super::text::text;
    use super::{
        HEADER_DISCLOSED, ROW_DISCLOSED, SURFACE_RAISED, contained_list, contained_list_disclosed,
        contained_list_lg, contained_list_sm, contained_list_xl,
    };
    use crate::component::tests::{assert_fits_parent, petrify_lone};

    use crate::token::{ColorValue, Theme, TokenName, TokenValue};
    use crate::tree::{Role, ViewNode};

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

    fn pinned(node: &ViewNode, key: &str) -> f32 {
        let v = &named(node, key).constraints.vertical;
        assert_eq!(v.min, v.max, "{key} is not pinned to one height");
        v.min.expect("pinned")
    }

    #[test]
    fn contained_list_sets_role_list_and_the_default_is_md_40() {
        let node = contained_list(
            "cl",
            "Related",
            vec![text("r0", "Alpha"), text("r1", "Bravo")],
        );
        assert_eq!(node.key.as_str(), "cl");
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_eq!(node.semantics.label.as_deref(), Some("Related"));
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert_eq!(pinned(&node, "header"), 40.0);
        assert_eq!(token(named(&node, "header"), "background"), None);
        // Slice-a "Sizes": on-page "header AND row all match" — the list
        // re-pins every caller row to the step (FR-027).
        assert_eq!(pinned(&node, "r0"), 40.0);
        assert_eq!(pinned(&node, "r1"), 40.0);
    }

    /// Slice-a "Sizes": on-page sm 32 / md 40 / lg 48 (header and rows
    /// matching), xl header 48 / row 64 (the one divergent step).
    #[test]
    fn on_page_ramp_is_32_40_48_with_xl_at_header_48_row_64() {
        for (label, node, header, row) in [
            (
                "sm",
                contained_list_sm("cl", "S", vec![text("r0", "A")]),
                32.0,
                32.0,
            ),
            (
                "md",
                contained_list("cl", "M", vec![text("r0", "A")]),
                40.0,
                40.0,
            ),
            (
                "lg",
                contained_list_lg("cl", "L", vec![text("r0", "A")]),
                48.0,
                48.0,
            ),
            (
                "xl",
                contained_list_xl("cl", "X", vec![text("r0", "A")]),
                48.0,
                64.0,
            ),
        ] {
            assert_eq!(pinned(&node, "header"), header, "{label} header");
            assert_eq!(pinned(&node, "r0"), row, "{label} row");
        }
    }

    #[test]
    fn contained_list_disclosed_header_is_32_rows_48_and_raised() {
        let node = contained_list_disclosed("cl", "Menu", vec![text("r0", "Alpha")]);
        assert_eq!(node.semantics.role, Some(Role::List));
        assert_eq!(pinned(&node, "header"), HEADER_DISCLOSED);
        assert_eq!(pinned(&node, "r0"), ROW_DISCLOSED);
        assert_eq!(HEADER_DISCLOSED, 32.0);
        assert_eq!(ROW_DISCLOSED, 48.0);
        assert_eq!(
            token(named(&node, "header"), "background"),
            Some(SURFACE_RAISED)
        );
    }

    #[test]
    fn contained_list_title_is_visible() {
        let node = contained_list("cl", "Inbox", vec![]);
        assert_eq!(named(&node, "title").props.text.as_deref(), Some("Inbox"));
        let disclosed = contained_list_disclosed("cl", "Inbox", vec![]);
        assert_eq!(
            named(&disclosed, "title").props.text.as_deref(),
            Some("Inbox")
        );
    }

    fn color(theme: &Theme, name: &str) -> ColorValue {
        match theme.value(&TokenName::new(name).unwrap()).unwrap() {
            TokenValue::Color(c) => *c,
            other => panic!("{name} is not a colour: {other:?}"),
        }
    }

    /// Check C/D across both header heights: no degenerate rect, no child
    /// placed outside its parent. Neither variant declares an interaction
    /// of its own (`Role::List` on the container, caller-supplied rows), so
    /// there is no Check F for this file — nothing here declares `Focus`.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let cases: Vec<(&str, ViewNode)> = vec![
            (
                "on-page",
                contained_list(
                    "cl",
                    "Related",
                    vec![text("r0", "Alpha"), text("r1", "Bravo")],
                ),
            ),
            (
                "disclosed",
                contained_list_disclosed("cl", "Menu", vec![text("r0", "Charlie")]),
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
                    assert_fits_parent(label, p, &frame.placements[parent_idx]);
                }
            }
        }
    }

    /// Check E: the on-page title (`heading`) and the disclosed title
    /// (muted `text`) against their own header fill -- the on-page header
    /// has none, so it inherits the page ground (`surface.base`); the
    /// disclosed header binds `surface.raised`. Both in both themes, read
    /// through `Props.opacity`.
    #[test]
    fn header_title_clears_aa_contrast_against_its_own_fill() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use super::super::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            for (label, node, page_ground) in [
                ("on-page", contained_list("cl", "Related", vec![]), true),
                (
                    "disclosed",
                    contained_list_disclosed("cl", "Menu", vec![]),
                    false,
                ),
            ] {
                let header = named(&node, "header");
                let bg = if page_ground {
                    color(&theme, SURFACE_BASE)
                } else {
                    let bg_name = header
                        .props
                        .tokens
                        .get("background")
                        .expect("disclosed header binds a background");
                    color(&theme, bg_name.as_str())
                };
                let title = named(header, "title");
                let fg_name = title
                    .props
                    .tokens
                    .get("foreground")
                    .expect("title binds a foreground");
                let opacity = title.props.opacity.unwrap_or(1.0);
                let fg = color(&theme, fg_name.as_str()).faded(opacity).over(bg);
                let ratio = fg.contrast_ratio(bg);
                assert!(
                    ratio >= MIN_TEXT_CONTRAST,
                    "{label}: title at {ratio:.2}:1 against {} fails AA {MIN_TEXT_CONTRAST}:1",
                    fg_name.as_str()
                );
            }
        }
    }
}
