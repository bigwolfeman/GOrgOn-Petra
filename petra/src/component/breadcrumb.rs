//! Carbon Breadcrumb (slice-a).
//!
//! Anatomy (docs + `_breadcrumb.scss`):
//! 1. [`breadcrumb`] — horizontal row, not interactive. [`breadcrumb_overflow`]
//!    is the same row after a `max_visible` collapse.
//! 2. [`breadcrumb_item`] — one hierarchy level you can go back to. Carbon
//!    nests a `.cds--link` inside the item, so this **is** a
//!    [`super::link`]: same ink, same hover rule, same role.
//! 3. [`breadcrumb_item_current`] — the level you are standing on.
//!    Carbon marks it `[aria-current='page']` / `.cds--breadcrumb-item--current`,
//!    drops the link, and drops every interaction with it. Slice-a is
//!    explicit that a non-DOM renderer needs an own "current" boolean in
//!    place of the ARIA attribute; here that boolean is a second
//!    constructor, so a trail cannot forget to say where it is.
//! 4. Separator — text generated between items, never by the item. Carbon
//!    uses `::after { content: '/' }`; Petra has no generated content, so
//!    the separator is a first-class child. [`breadcrumb`] writes `"/"`.
//!    [`breadcrumb_with_separator`] takes any other mark.
//! 5. [`breadcrumb_item_icon`] — a leading [`super::icon::icon_toned`]
//!    beside the same [`super::link::link`]. The label is still required
//!    (FR-026 / FR-058): the mark is never the only channel.
//!
//! Sizes are type tokens, not a row height: sm 16 / md 18 line-height.
//! This constructor ships the md/default type (`typography.body`).
//!
//! # Why the item is a link and not a styled label
//!
//! Until 2026-09-05 a crumb bound no ink of its own, so all three crumbs
//! and both separators resolved to `text.primary` and the row read as one
//! sentence with slashes in it. The operator's report was *"i dont
//! understand this one"*, which is the correct reaction to five identical
//! glyph runs. Carbon paints the leading crumbs `$link-primary` and the
//! last one `$text-primary`, and in one look that says *two places I can
//! go back to, one place I am*.
//!
//! Building the crumb out of [`super::link::link`] rather than copying its
//! two token inserts is deliberate: link ink and the hover rule are one
//! fact, and two copies of one fact drift.
//!
//! **Three channels, not one.** The operator is red-green colour blind, so
//! the blue is never allowed to be the whole signal: a crumb also
//! underlines under the pointer (Carbon's own `:hover` rule, carried by
//! [`super::link::link`]) where the current page never does, and a crumb is
//! in focus order where the current page is not.
//!
//! # Overflow is derived in view
//!
//! Collapse is derived in view from `max_visible`, not fiber State. The
//! operator cannot expand collapsed crumbs back into the trail. Hidden
//! crumbs stay reachable as a menu of their labels under an ellipsis
//! keyed `"overflow"`. First crumb stays. Last crumb stays (the current
//! page). The middle is what `max_visible` trims, from the left.
//!
//! # T070 divergence, still open
//!
//! Carbon's default keeps a separator **after** the last item and the
//! reference capture (`39`-geometry `03-breadcrumb.png`, read 2026-09-05)
//! shows it. [`breadcrumb`] drops it, matching `--no-trailing-slash`. That
//! is round 3's operator question Q4 and is deliberately not answered here.

use super::icon::{IconMark, IconTone, icon_toned};
use super::menu::{menu, menu_item};
use super::stack;
use super::text::text;
use super::tokens::SPACING_03;
use crate::geom::{Align, Axis};
use crate::tree::{FocusFigure, Interaction, Key, Role, Semantics, ViewNode};

/// Default separator Carbon writes with `::after { content: '/' }`.
const DEFAULT_SEPARATOR: &str = "/";

/// One ellipsis character, not three dots. Same mark pagination uses for
/// a collapsed run.
const ELLIPSIS: &str = "\u{2026}";

/// Accessible name of the overflow trigger and of the menu it opens.
const OVERFLOW_LABEL: &str = "Show hidden breadcrumbs";

const OVERFLOW_INTENTS: &[Interaction] =
    &[Interaction::Focus, Interaction::Click, Interaction::Hover];

/// A location/path trail. Horizontal stack, no role, no interactions.
///
/// Separators are injected here: one `"/"` text node between adjacent
/// items, none after the last (Carbon `--no-trailing-slash` is the
/// portable default for a retained-mode tree). For a different mark, see
/// [`breadcrumb_with_separator`].
pub fn breadcrumb(key: impl Into<Key>, crumbs: Vec<ViewNode>) -> ViewNode {
    breadcrumb_with_separator(key, DEFAULT_SEPARATOR, crumbs)
}

/// [`breadcrumb`] with the separator mark chosen by the caller.
///
/// `sep` is the text of every separator node. The row still injects them
/// between items and still drops a trailing one. An empty `sep` still
/// inserts the separator nodes; they just have no glyphs.
pub fn breadcrumb_with_separator(
    key: impl Into<Key>,
    sep: &str,
    crumbs: Vec<ViewNode>,
) -> ViewNode {
    let mut children = Vec::with_capacity(crumbs.len().saturating_mul(2).saturating_sub(1));
    for (i, crumb) in crumbs.into_iter().enumerate() {
        if i > 0 {
            let mut node = text(format!("sep-{i}"), sep);
            node.semantics = Semantics {
                role: Some(Role::Separator),
                ..Semantics::default()
            };
            children.push(node);
        }
        children.push(crumb);
    }
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), children);
    node.props.align = Some(Align::Center);
    node
}

/// [`breadcrumb`] after collapsing the middle of `crumbs` to `max_visible`
/// items.
///
/// Collapse is derived in view from `max_visible`, not fiber State. The
/// first crumb stays, the last crumb stays (the current page), and the
/// overflow ellipsis is a menu of the hidden labels. The operator cannot
/// expand those crumbs back into the trail. `max_visible` below 2 is
/// treated as 2, because first and last always remain.
pub fn breadcrumb_overflow(
    key: impl Into<Key>,
    max_visible: usize,
    crumbs: Vec<ViewNode>,
) -> ViewNode {
    breadcrumb_with_separator(key, DEFAULT_SEPARATOR, collapse(max_visible, crumbs))
}

/// One breadcrumb link: a level above the current page, which you can go
/// back to. `label` is required (FR-058).
///
/// This is [`super::link::link`], not a lookalike. Carbon nests
/// `.cds--link` inside `.cds--breadcrumb-item`, and a link is its words —
/// one `Text` leaf carrying the role, the interactions, `link-primary` ink
/// and the `underline@hover` rule. Wrapping it in a stack would put the
/// interactions on the wrapper and the hover rule on the child, and the
/// child is never the hovered node, so the rule would never resolve; that
/// is [`super::link`]'s own measured reason for the same shape.
pub fn breadcrumb_item(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let mut node = super::link::link(key, label);
    // The one place a crumb departs from the link it is built from.
    //
    // [`super::link::link`] brackets, because a standalone inline link's
    // own underline sits exactly where a focus bar would hang. A crumb is
    // not standalone: crumbs run in a horizontal row separated by `"/"`, so
    // a bracket stands in the gap between two crumbs and reads as marking
    // the separator rather than either word. The row has clear space under
    // it, so the bar goes back under. The operator's call of 2026-09-06.
    node.semantics.focus_figure = FocusFigure::BarUnder;
    node
}

/// The crumb for the page you are on: Carbon's `[aria-current='page']`.
///
/// Not a link, not focusable, not clickable, and in the page's own ink
/// rather than link ink — all four are Carbon's rules for this item, and
/// all four are the channels that say *you are here* to someone who does
/// not read the hue.
///
/// [`Semantics::selected`] carries the boolean, because Petra's semantics
/// have no `current` and `selected` is what "this is the one of the set
/// that is in force" means everywhere else in this library
/// (`selection_is_declared_state_not_only_a_fill_colour`).
pub fn breadcrumb_item_current(key: impl Into<Key>, label: impl Into<String>) -> ViewNode {
    let label = label.into();
    let mut node = text(key, label.clone());
    node.semantics = Semantics {
        label: Some(label),
        selected: true,
        ..Semantics::default()
    };
    node
}

/// A breadcrumb link with a leading mark. `label` is required (FR-058).
///
/// The row is [`super::icon::icon_toned`] plus [`super::link::link`], not a
/// restyled caption. The words stay the control so the hover underline
/// still resolves on them: wrapping the link in an interactive stack would
/// put Hover on the wrapper and the `underline@hover` rule on the child,
/// and the child would never be the hovered node — the same measured
/// reason [`breadcrumb_item`] is a link and not a stack around one.
///
/// [`IconTone::Primary`]: the mark sits on the page ground beside link
/// ink, not on an accent fill.
pub fn breadcrumb_item_icon(
    key: impl Into<Key>,
    mark: IconMark,
    label: impl Into<String>,
) -> ViewNode {
    let label = label.into();
    let glyph = icon_toned("mark", mark, IconTone::Primary);
    let mut words = super::link::link("label", label.clone());
    words.semantics.focus_figure = FocusFigure::BarUnder;
    let mut node = stack(key, Axis::Horizontal, Some(SPACING_03), vec![glyph, words]);
    node.props.align = Some(Align::Center);
    node.semantics.label = Some(label);
    node
}

/// First and last always remain, so a budget below 2 is 2.
fn collapse(max_visible: usize, mut crumbs: Vec<ViewNode>) -> Vec<ViewNode> {
    let cap = max_visible.max(2);
    if crumbs.len() <= cap {
        return crumbs;
    }
    let last = crumbs.pop().expect("len > cap >= 2");
    let first = crumbs.remove(0);
    let keep_middle = cap - 2;
    let hidden_count = crumbs.len() - keep_middle;
    let hidden: Vec<ViewNode> = crumbs.drain(..hidden_count).collect();
    let mut out = Vec::with_capacity(3 + crumbs.len());
    out.push(first);
    out.push(overflow_ellipsis(hidden));
    out.append(&mut crumbs);
    out.push(last);
    out
}

/// Ellipsis trigger plus a [`menu`] of the hidden crumbs.
///
/// The menu is in the tree whenever crumbs overflow, so those destinations
/// stay reachable without an `open` flag on the fiber. Which crumbs sit
/// in it is still derived from `max_visible`.
fn overflow_ellipsis(hidden: Vec<ViewNode>) -> ViewNode {
    let items: Vec<ViewNode> = hidden.into_iter().map(as_hidden_item).collect();
    let trigger = text("trigger", ELLIPSIS)
        .interactive(Role::Button, OVERFLOW_LABEL, OVERFLOW_INTENTS)
        .with_focus_figure(FocusFigure::BarUnder);
    let overlay = menu("hidden", OVERFLOW_LABEL, items);
    let mut node = stack("overflow", Axis::Horizontal, None, vec![trigger, overlay]);
    node.props.align = Some(Align::Center);
    node
}

fn as_hidden_item(crumb: ViewNode) -> ViewNode {
    let label = crumb_label(&crumb);
    menu_item(crumb.key, label)
}

fn crumb_label(node: &ViewNode) -> String {
    if let Some(label) = &node.semantics.label {
        return label.clone();
    }
    if let Some(text) = &node.props.text {
        return text.clone();
    }
    for child in &node.children {
        let nested = crumb_label(child);
        if !nested.is_empty() {
            return nested;
        }
    }
    node.key.as_str().to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        ELLIPSIS, OVERFLOW_LABEL, SPACING_03, breadcrumb, breadcrumb_item, breadcrumb_item_current,
        breadcrumb_item_icon, breadcrumb_overflow, breadcrumb_with_separator,
    };
    use crate::component::icon::{IconMark, IconTone, icon_toned};
    use crate::component::tokens::{LINK_PRIMARY, TEXT_PRIMARY};
    use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
    use crate::geom::{Axis, Size};
    use crate::testing::{Harness, validated_with};
    use crate::token::{ColorValue, Theme, ThemeMode, TokenName, TokenValue, standard_vocabulary};
    use crate::tree::{Interaction, NodeKind, Props, Registry, Role, ViewNode};

    fn token<'a>(node: &'a ViewNode, slot: &str) -> Option<&'a str> {
        node.props.tokens.get(slot).map(|name| name.as_str())
    }

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

    fn has_key(node: &ViewNode, key: &str) -> bool {
        fn walk(node: &ViewNode, key: &str) -> bool {
            node.key.as_str() == key || node.children.iter().any(|child| walk(child, key))
        }
        walk(node, key)
    }

    #[test]
    fn breadcrumb_exists_and_is_not_interactive() {
        let node = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("home", "Home"),
                breadcrumb_item("here", "Here"),
            ],
        );
        assert_eq!(node.key.as_str(), "trail");
        assert!(node.interactions.is_empty());
        assert!(!node.is_interactive());
        assert!(node.semantics.role.is_none());
        assert_eq!(node.props.axis, Some(Axis::Horizontal));
        assert_eq!(
            node.props.spacing.as_ref().map(|n| n.as_str()),
            Some(SPACING_03)
        );
    }

    /// A crumb is a link: the role, the label, the intents, the ink and the
    /// hover rule Carbon's `.cds--link` inside `.cds--breadcrumb-item` has.
    ///
    /// The ink is asserted **not** to be `text.primary` as well as to be
    /// `link-primary`, for the reason `link.rs` gives about row 15: the
    /// defect was a control that resolved to the same tone as the prose
    /// beside it, and one name for two tones would pass the positive half
    /// on its own. Falsify by replacing the body of `breadcrumb_item` with
    /// `text(key, label)`: the ink assertions fail first.
    #[test]
    fn breadcrumb_item_is_a_link_in_link_ink() {
        let node = breadcrumb_item("home", "Home");
        assert_eq!(node.kind, NodeKind::Text, "a crumb is its words");
        assert_eq!(node.props.text.as_deref(), Some("Home"));
        assert_eq!(node.semantics.role, Some(Role::Button));
        assert_eq!(node.semantics.label.as_deref(), Some("Home"));
        assert!(node.interactions.contains(&Interaction::Focus));
        assert!(node.interactions.contains(&Interaction::Click));
        assert!(
            node.interactions.contains(&Interaction::Hover),
            "without Hover the crumb is never hovered and `underline@hover` \
             never resolves, so the second channel is gone"
        );
        assert!(node.is_interactive());
        assert_eq!(token(&node, "foreground"), Some(LINK_PRIMARY));
        assert_ne!(token(&node, "foreground"), Some(TEXT_PRIMARY));
        assert_eq!(token(&node, "underline@hover"), Some(LINK_PRIMARY));
    }

    /// T025: the module doc (`:38-44`) names three channels that keep the
    /// current page from reading as just another link — colour (never
    /// [`LINK_PRIMARY`]), underline-on-hover (links only, Carbon's own
    /// `:hover` rule), and focus order (links only) — because the operator
    /// is red-green colour blind and colour alone is never a channel he
    /// can read. This is one capability test asserting all three at once,
    /// with a single failure point, replacing two unlabelled tests that
    /// each covered one axis and could stay green while a regression broke
    /// another: `the_current_crumb_is_not_a_link_and_declares_itself_selected`
    /// and `the_current_crumb_is_not_in_focus_order`, both folded in here.
    ///
    /// Falsify by returning `breadcrumb_item(key, label)` from
    /// `breadcrumb_item_current`: colour and hover both resolve to link
    /// tone and `here` lands in focus order — every assertion below fails.
    #[test]
    fn the_current_crumb_differs_from_a_link_on_colour_hover_and_focus_order() {
        let home = breadcrumb_item("home", "Home");
        let here = breadcrumb_item_current("here", "Rebuild");

        // Channel 1: colour. The current page never takes link ink.
        assert_eq!(token(&home, "foreground"), Some(LINK_PRIMARY));
        assert_eq!(token(&here, "foreground"), Some(TEXT_PRIMARY));
        assert_ne!(token(&here, "foreground"), Some(LINK_PRIMARY));

        // Channel 2: underline on hover. Links only.
        assert_eq!(token(&home, "underline@hover"), Some(LINK_PRIMARY));
        assert_eq!(token(&here, "underline@hover"), None);

        // The current page also carries no role and no interactions, and
        // declares `selected` as a fact rather than only a tone.
        assert_eq!(here.kind, NodeKind::Text);
        assert_eq!(here.props.text.as_deref(), Some("Rebuild"));
        assert_eq!(here.semantics.label.as_deref(), Some("Rebuild"));
        assert_eq!(here.semantics.role, None, "the current page is not a link");
        assert!(here.interactions.is_empty());
        assert!(!here.is_interactive());
        assert!(
            here.semantics.selected,
            "Carbon's `[aria-current='page']` has to survive into the tree"
        );

        // Channel 3: focus order. A trail is where this is observable — a
        // lone node has no order to be excluded from.
        let trail = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("home", "Home"),
                breadcrumb_item_current("here", "Rebuild"),
            ],
        );
        let frame = petrify_lone(trail);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let order = focus.order();
        assert!(
            order.iter().any(|id| id.ends_with("/home")),
            "the leading crumb stays in focus order"
        );
        assert!(
            !order.iter().any(|id| id.ends_with("/here")),
            "the current page is not a link and is not in focus order"
        );
    }

    /// A trail says both things at once, and the separator belongs to
    /// neither crumb.
    #[test]
    fn a_trail_ending_in_a_current_page_has_links_before_it_and_none_after() {
        let node = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("home", "Workspace"),
                breadcrumb_item("docs", "Fibers"),
                breadcrumb_item_current("here", "Rebuild"),
            ],
        );
        assert_eq!(
            child_keys(&node),
            ["home", "sep-1", "docs", "sep-2", "here"]
        );
        let links: Vec<&str> = node
            .children
            .iter()
            .filter(|c| c.semantics.role == Some(Role::Button))
            .map(|c| c.key.as_str())
            .collect();
        assert_eq!(links, ["home", "docs"], "only the leading crumbs are links");
        assert!(named(&node, "here").semantics.selected);
    }

    #[test]
    fn breadcrumb_inserts_slash_separators_between_items() {
        let node = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("a", "Alpha"),
                breadcrumb_item("b", "Bravo"),
                breadcrumb_item("c", "Charlie"),
            ],
        );
        assert_eq!(child_keys(&node), ["a", "sep-1", "b", "sep-2", "c"]);
        assert_eq!(named(&node, "sep-1").props.text.as_deref(), Some("/"));
        assert_eq!(named(&node, "sep-2").props.text.as_deref(), Some("/"));
        assert_eq!(named(&node, "sep-1").semantics.role, Some(Role::Separator));
        assert!(
            child_keys(&node)
                .iter()
                .filter(|k| k.starts_with("sep-"))
                .count()
                == 2,
            "no trailing separator after the last item"
        );
    }

    #[test]
    fn breadcrumb_with_one_item_has_no_separator() {
        let node = breadcrumb("trail", vec![breadcrumb_item("only", "Only")]);
        assert_eq!(child_keys(&node), ["only"]);
    }

    #[test]
    fn breadcrumb_with_separator_writes_the_given_mark() {
        let node = breadcrumb_with_separator(
            "trail",
            ">",
            vec![
                breadcrumb_item("a", "Alpha"),
                breadcrumb_item("b", "Bravo"),
                breadcrumb_item_current("c", "Charlie"),
            ],
        );
        assert_eq!(child_keys(&node), ["a", "sep-1", "b", "sep-2", "c"]);
        assert_eq!(named(&node, "sep-1").props.text.as_deref(), Some(">"));
        assert_eq!(named(&node, "sep-2").props.text.as_deref(), Some(">"));
        assert_eq!(named(&node, "sep-1").semantics.role, Some(Role::Separator));
        assert_eq!(
            named(&node, "sep-1").semantics.role,
            named(&node, "sep-2").semantics.role
        );
        assert_eq!(
            breadcrumb(
                "slash",
                vec![breadcrumb_item("a", "A"), breadcrumb_item("b", "B")]
            )
            .children
            .iter()
            .find(|c| c.key.as_str() == "sep-1")
            .and_then(|c| c.props.text.as_deref()),
            Some("/")
        );
    }

    /// The mark is furniture. The words are the link: same ink, same hover
    /// rule, same role as [`breadcrumb_item`]. Falsify by making the stack
    /// itself the button — the inner link's hover rule would never resolve.
    #[test]
    fn breadcrumb_item_icon_composes_a_toned_icon_and_a_link() {
        let node = breadcrumb_item_icon("home", IconMark::Calendar, "Home");
        assert_eq!(node.kind, NodeKind::Stack);
        assert_eq!(node.semantics.label.as_deref(), Some("Home"));
        assert!(
            !node.is_interactive(),
            "Hover has to land on the words, not the row"
        );
        assert_eq!(child_keys(&node), ["mark", "label"]);
        assert_eq!(
            named(&node, "mark").props.canvas,
            icon_toned("mark", IconMark::Calendar, IconTone::Primary)
                .props
                .canvas
        );
        let words = named(&node, "label");
        assert_eq!(words.kind, NodeKind::Text);
        assert_eq!(words.props.text.as_deref(), Some("Home"));
        assert_eq!(words.semantics.role, Some(Role::Button));
        assert_eq!(words.semantics.label.as_deref(), Some("Home"));
        assert!(words.interactions.contains(&Interaction::Focus));
        assert!(words.interactions.contains(&Interaction::Click));
        assert!(words.interactions.contains(&Interaction::Hover));
        assert_eq!(token(words, "foreground"), Some(LINK_PRIMARY));
        assert_ne!(token(words, "foreground"), Some(TEXT_PRIMARY));
        assert_eq!(token(words, "underline@hover"), Some(LINK_PRIMARY));
    }

    /// Collapse is a `max_visible` argument, not State. First and last
    /// stay; the middle that does not fit becomes a menu of labels.
    #[test]
    fn breadcrumb_overflow_collapses_the_middle_from_max_visible() {
        let crumbs = vec![
            breadcrumb_item("home", "Home"),
            breadcrumb_item("docs", "Docs"),
            breadcrumb_item("api", "API"),
            breadcrumb_item("ref", "Reference"),
            breadcrumb_item_current("here", "Rebuild"),
        ];
        let all = breadcrumb_overflow("trail", 5, crumbs.clone());
        assert_eq!(
            child_keys(&all),
            [
                "home", "sep-1", "docs", "sep-2", "api", "sep-3", "ref", "sep-4", "here"
            ]
        );
        assert!(!has_key(&all, "overflow"));

        let node = breadcrumb_overflow("trail", 3, crumbs);
        assert_eq!(
            child_keys(&node),
            ["home", "sep-1", "overflow", "sep-2", "ref", "sep-3", "here"]
        );
        assert!(!child_keys(&node).contains(&"docs"));
        assert!(!child_keys(&node).contains(&"api"));
        let overflow = named(&node, "overflow");
        let trigger = named(overflow, "trigger");
        assert_eq!(trigger.props.text.as_deref(), Some(ELLIPSIS));
        assert_eq!(trigger.semantics.role, Some(Role::Button));
        assert_eq!(trigger.semantics.label.as_deref(), Some(OVERFLOW_LABEL));
        assert!(trigger.interactions.contains(&Interaction::Focus));
        assert!(trigger.interactions.contains(&Interaction::Click));
        assert_eq!(
            named(overflow, "hidden").semantics.role,
            Some(Role::Overlay)
        );
        assert_eq!(
            named(overflow, "hidden").semantics.label.as_deref(),
            Some(OVERFLOW_LABEL)
        );
        assert_eq!(
            named(overflow, "docs").semantics.label.as_deref(),
            Some("Docs")
        );
        assert_eq!(
            named(overflow, "api").semantics.label.as_deref(),
            Some("API")
        );
        assert_eq!(named(overflow, "docs").semantics.role, Some(Role::Button));
        assert!(!has_key(overflow, "ref"));
        assert!(named(&node, "here").semantics.selected);
        assert_eq!(named(&node, "here").semantics.role, None);
        assert_eq!(named(&node, "home").semantics.role, Some(Role::Button));
        assert_eq!(
            token(named(&node, "home"), "foreground"),
            Some(LINK_PRIMARY)
        );
        assert_eq!(token(named(&node, "ref"), "foreground"), Some(LINK_PRIMARY));
        assert_eq!(
            token(named(&node, "here"), "foreground"),
            Some(TEXT_PRIMARY)
        );
    }

    #[test]
    fn overflow_menu_keeps_the_label_of_an_icon_item() {
        let node = breadcrumb_overflow(
            "trail",
            2,
            vec![
                breadcrumb_item("home", "Home"),
                breadcrumb_item_icon("docs", IconMark::Calendar, "Docs"),
                breadcrumb_item_current("here", "Here"),
            ],
        );
        assert_eq!(
            child_keys(&node),
            ["home", "sep-1", "overflow", "sep-2", "here"]
        );
        assert_eq!(
            named(&node, "docs").semantics.label.as_deref(),
            Some("Docs")
        );
        assert!(named(&node, "here").semantics.selected);
        assert_eq!(named(&node, "here").semantics.role, None);
    }

    #[test]
    fn breadcrumb_overflow_keeps_first_and_last_when_budget_is_two() {
        let node = breadcrumb_overflow(
            "trail",
            2,
            vec![
                breadcrumb_item("a", "Alpha"),
                breadcrumb_item("b", "Bravo"),
                breadcrumb_item("c", "Charlie"),
                breadcrumb_item_current("d", "Delta"),
            ],
        );
        assert_eq!(child_keys(&node), ["a", "sep-1", "overflow", "sep-2", "d"]);
        let overflow = named(&node, "overflow");
        assert!(has_key(overflow, "b"));
        assert!(has_key(overflow, "c"));
        assert!(named(&node, "d").semantics.selected);
    }

    #[test]
    fn max_visible_below_two_still_keeps_first_and_last() {
        let node = breadcrumb_overflow(
            "trail",
            1,
            vec![
                breadcrumb_item("a", "A"),
                breadcrumb_item("b", "B"),
                breadcrumb_item_current("c", "C"),
            ],
        );
        assert_eq!(child_keys(&node), ["a", "sep-1", "overflow", "sep-2", "c"]);
        assert!(has_key(named(&node, "overflow"), "b"));
        assert!(named(&node, "c").semantics.selected);
        assert_eq!(named(&node, "c").semantics.role, None);
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

    /// Check C/D: every crumb, its separators, and the trail itself place
    /// with a real rect, none of them outside their parent.
    #[test]
    fn frame_geometry_has_no_degenerate_or_overflowing_placements() {
        let node = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("home", "Home"),
                breadcrumb_item("docs", "Docs"),
                breadcrumb_item("here", "Here"),
            ],
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

    /// Check F: every crumb declares `Focus` and must be reachable; a
    /// breadcrumb has no disabled crumb, so there is no negative case.
    #[test]
    fn every_crumb_is_reachable_in_focus_order() {
        let node = breadcrumb(
            "trail",
            vec![
                breadcrumb_item("home", "Home"),
                breadcrumb_item("here", "Here"),
            ],
        );
        let frame = petrify_lone(node);
        let focus = crate::focus::FocusTree::from_placements(
            &frame.placements,
            &std::collections::BTreeMap::new(),
        );
        let crumb_ids: Vec<&str> = frame
            .placements
            .iter()
            .filter(|p| p.semantics.role == Some(Role::Button))
            .map(|p| p.id.as_str())
            .collect();
        assert_eq!(crumb_ids.len(), 2, "expected both crumbs placed");
        let order = focus.order();
        for id in crumb_ids {
            assert!(
                order.iter().any(|o| o == id),
                "{id} declares Focus but is not in focus order"
            );
        }
    }

    /// Check E: crumb labels and the `"/"` separator against the page
    /// ground they are read on (`surface.base`, matching how `text()`
    /// itself is styled to sit on the base layer), read through
    /// `Props.opacity`.
    #[test]
    fn crumb_and_separator_text_clears_aa_contrast_on_the_page_ground() {
        const MIN_TEXT_CONTRAST: f32 = 4.5;
        use crate::component::tokens::SURFACE_BASE;
        for theme in [crate::token::light(), crate::token::dark()] {
            let bg = color(&theme, SURFACE_BASE);
            let node = breadcrumb(
                "trail",
                vec![
                    breadcrumb_item("home", "Home"),
                    breadcrumb_item("here", "Here"),
                ],
            );
            fn walk_text(node: &ViewNode, bg: ColorValue, theme: &Theme, min: f32) {
                if node.props.text.is_some()
                    && let Some(fg_name) = node.props.tokens.get("foreground")
                {
                    let opacity = node.props.opacity.unwrap_or(1.0);
                    let fg = color(theme, fg_name.as_str()).faded(opacity).over(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= min,
                        "{:?} at {ratio:.2}:1 against {} fails AA {min}:1",
                        node.key,
                        fg_name.as_str()
                    );
                }
                for child in &node.children {
                    walk_text(child, bg, theme, min);
                }
            }
            walk_text(&node, bg, &theme, MIN_TEXT_CONTRAST);
        }
    }
}
