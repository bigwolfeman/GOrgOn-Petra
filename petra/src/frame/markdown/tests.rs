//! Unit tests for the line lead a copied selection writes.
//!
//! These place real component trees rather than hand-built placements. The
//! claim under test is that a **list** copies as Markdown, and a list is
//! `crate::component::list`'s arrangement of a canvas and a run inside a
//! `Role::ListItem`. A hand-built pair of placements would let this file
//! agree with itself about a shape the component does not build.

use crate::component::{list_item, list_item_with, ordered_list, stack, text, unordered_list};
use crate::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use crate::geom::{Axis, Size};
use crate::input::{TextPoint, TextSelection};
use crate::testing::{Harness, validated};
use crate::token::ThemeMode;
use crate::tree::{NodeKind, ViewNode};

fn viewport() -> Viewport {
    Viewport::new(Size::new(400.0, 400.0), ThemeMode::Dark)
}

fn place(tree: &ViewNode, selection: Option<TextSelection>) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state.text_selection = selection;
    petrify(
        1,
        validated(tree),
        &mut harness.ctx(),
        viewport(),
        TransitionActivity::default(),
    )
}

/// Every painted run in `tree`, in placement order, as `(id, byte length)`.
///
/// Empty runs are dropped because the resolver drops them too
/// (`layout::selection::clean`), so a selection anchored in one would cover
/// nothing and the test would assert against a shorter span than it named.
fn runs(tree: &ViewNode) -> Vec<(String, usize)> {
    place(tree, None)
        .drawn()
        .filter_map(|(placement, content)| {
            let painted = content.text.as_ref()?;
            (!painted.text.is_empty()).then(|| (placement.id.clone(), painted.text.len()))
        })
        .collect()
}

/// What the clipboard takes from a drag that starts `from` bytes into the
/// first painted run and ends at the last one.
fn copied_from(tree: &ViewNode, from: usize) -> String {
    let runs = runs(tree);
    let (head, _) = runs.first().expect("the tree paints at least one run");
    let (tail, len) = runs.last().expect("the tree paints at least one run");
    let selection = TextSelection {
        anchor: TextPoint::new(head.clone(), from),
        focus: TextPoint::new(tail.clone(), *len),
    };
    place(tree, Some(selection))
        .selected_text()
        .expect("a selection that covers every run copies something")
}

/// What the clipboard takes from a drag across the whole tree.
fn copied(tree: &ViewNode) -> String {
    copied_from(tree, 0)
}

/// Carbon draws a dash at level one and squares under it, and the operator's
/// picture on 2026-09-07 showed all four levels. Every one of them copies as
/// Markdown's single item mark: the depth is the indent's job, and `▪` is a
/// character no renderer reads as a list.
#[test]
fn a_copied_bullet_list_is_a_markdown_list() {
    let tree = unordered_list(
        "ul",
        vec![list_item_with(
            "one",
            "Inbox",
            Some(unordered_list(
                "l2",
                vec![list_item_with(
                    "two",
                    "Archive",
                    Some(unordered_list("l3", vec![list_item("three", "2026")])),
                )],
            )),
        )],
    );

    assert_eq!(copied(&tree), "- Inbox\n  - Archive\n    - 2026");
}

/// An ordered list's counters are painted text, so they copy themselves and
/// must not also collect a bullet: the marker run is the first thing on its
/// line and the label shares that line with it.
#[test]
fn a_copied_ordered_list_keeps_its_own_counters_and_gains_no_bullet() {
    let tree = ordered_list(
        "ol",
        vec![
            list_item("one", "Clone"),
            list_item_with(
                "two",
                "Build",
                Some(ordered_list("l2", vec![list_item("a", "Compile")])),
            ),
        ],
    );

    let copied = copied(&tree);
    assert_eq!(copied, "1. Clone\n2. Build\n  a. Compile");
    assert!(
        !copied.contains('-'),
        "a counter is already the item's mark, so a bullet beside it would \
         claim a second list: {copied:?}"
    );
}

/// A drag that starts halfway down a label copies half a word, and half a
/// word is not a list item. The indent stays, because a partial line is
/// still at its own depth.
#[test]
fn a_half_taken_line_gets_no_bullet() {
    let tree = unordered_list("ul", vec![list_item("one", "Archive")]);

    assert_eq!(copied_from(&tree, 4), "ive");
}

/// The mark leads the run it was drawn in front of, and no other. Two items
/// in one list each take their own bullet — a rule that reached past the
/// nearest sibling would put the first item's mark on the second item too,
/// or lose the second one's.
#[test]
fn every_item_takes_its_own_mark() {
    let tree = unordered_list(
        "ul",
        vec![list_item("one", "Disc"), list_item("two", "Ring")],
    );

    assert_eq!(copied(&tree), "- Disc\n- Ring");
}

/// `Props::markdown` on a node that paints its own string is inert. The run
/// already copies itself, so honouring the declaration as well would put the
/// same words on the clipboard twice.
#[test]
fn a_mark_declared_on_painted_text_is_ignored() {
    let mut spoken = text("spoken", "Alpha");
    spoken.props.markdown = Some("#".to_owned());
    let tree = stack(
        "row",
        Axis::Vertical,
        None,
        vec![spoken, text("after", "Beta")],
    );

    assert_eq!(copied(&tree), "Alpha\nBeta");
}

/// A drawn mark with nothing declared on it leads nothing. This is the state
/// every canvas in the library was in before 2026-09-07, and it is what a
/// component that draws a picture for its own sake still gets.
#[test]
fn a_silent_picture_leads_nothing() {
    let mut mark = ViewNode::new(NodeKind::Canvas, "mark");
    mark.props.canvas = Some(std::sync::Arc::new(
        crate::draw::DrawList::new(Vec::new()).expect("an empty picture is a legal one"),
    ));
    let tree = stack(
        "row",
        Axis::Vertical,
        None,
        vec![mark, text("after", "Beta")],
    );

    assert_eq!(copied(&tree), "Beta");
}
