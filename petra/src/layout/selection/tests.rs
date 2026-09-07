//! Unit tests for the span resolver.
//!
//! These build placements by hand rather than by placing a tree. The thing
//! under test is what "between two points" means over a list of placements,
//! and a hand-built list is the only way to write a case a component happens
//! not to produce yet — a span whose ends arrive in the wrong order, a run
//! claimed by a control in the middle of one, a string that shrank under a
//! live gesture.

use super::resolve;
use crate::frame::placement::{PaintContent, PaintState, Placement, PlacementSemantics, TextPaint};
use crate::geom::Rect;
use crate::input::{TextPoint, TextSelection};
use crate::tree::{NodeKind, TextWrap};

/// One text placement carrying `text`, optionally inside `parent`.
fn run(id: &str, text: &str, parent: Option<usize>) -> (Placement, PaintContent) {
    node(id, text, parent, NodeKind::Text, false)
}

/// A placement that declares the opt-out, so everything under it is a
/// control's.
fn control(id: &str, text: &str, parent: Option<usize>) -> (Placement, PaintContent) {
    node(id, text, parent, NodeKind::Text, true)
}

/// A container: no text of its own, so no run.
fn box_of(id: &str, parent: Option<usize>) -> (Placement, PaintContent) {
    node(id, "", parent, NodeKind::Stack, false)
}

fn node(
    id: &str,
    text: &str,
    parent: Option<usize>,
    kind: NodeKind,
    owns: bool,
) -> (Placement, PaintContent) {
    let placement = Placement {
        id: id.into(),
        kind,
        rect: Rect::new(0.0, 0.0, 100.0, 20.0),
        z: 0,
        clip: Rect::new(0.0, 0.0, 1000.0, 1000.0),
        opacity: 1.0,
        paint: PaintState::default(),
        semantics: PlacementSemantics {
            owns_its_text: owns,
            ..PlacementSemantics::default()
        },
        parent,
    };
    let content = if text.is_empty() {
        PaintContent::default()
    } else {
        PaintContent {
            text: Some(TextPaint {
                text: text.to_owned(),
                style: None,
                wrap: TextWrap::Clip,
                max_lines: None,
                runs: Vec::new(),
            }),
            ..PaintContent::default()
        }
    };
    (placement, content)
}

fn split(parts: Vec<(Placement, PaintContent)>) -> (Vec<Placement>, Vec<PaintContent>) {
    parts.into_iter().unzip()
}

/// The ranges the resolver wrote, as `(id, selected substring)`.
fn selected(placements: &[Placement], content: &[PaintContent]) -> Vec<(String, String)> {
    content
        .iter()
        .enumerate()
        .filter_map(|(index, content)| {
            let range = content.selection.clone()?;
            let text = content.text.as_ref()?;
            Some((
                placements[index].id.clone(),
                text.text.get(range)?.to_owned(),
            ))
        })
        .collect()
}

fn span(anchor: (&str, usize), focus: (&str, usize)) -> TextSelection {
    TextSelection {
        anchor: TextPoint::new(anchor.0, anchor.1),
        focus: TextPoint::new(focus.0, focus.1),
    }
}

/// The whole point: a span that starts in one run and ends in another takes
/// the tail of the first, all of every run between, and the head of the last.
#[test]
fn a_span_takes_the_tail_the_middle_and_the_head() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alpha", Some(0)),
        run("/root/b", "bravo", Some(0)),
        run("/root/c", "charlie", Some(0)),
        run("/root/d", "delta", Some(0)),
    ]);
    let touched = resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 2), ("/root/c", 4)),
    );
    assert_eq!(touched, vec![1, 2, 3]);
    assert_eq!(
        selected(&placements, &content),
        vec![
            ("/root/a".to_owned(), "pha".to_owned()),
            ("/root/b".to_owned(), "bravo".to_owned()),
            ("/root/c".to_owned(), "char".to_owned()),
        ],
        "the run after the focus is untouched"
    );
}

/// A drag that went up the page names its focus first. The span is the same
/// stretch of the document either way.
#[test]
fn a_span_dragged_backwards_covers_the_same_words() {
    let build = || {
        split(vec![
            box_of("/root", None),
            run("/root/a", "alpha", Some(0)),
            run("/root/b", "bravo", Some(0)),
        ])
    };
    let (mut down, mut down_text) = build();
    resolve(
        &mut down,
        &mut down_text,
        &span(("/root/a", 2), ("/root/b", 3)),
    );
    let (mut up, mut up_text) = build();
    resolve(&mut up, &mut up_text, &span(("/root/b", 3), ("/root/a", 2)));
    assert_eq!(selected(&down, &down_text), selected(&up, &up_text));
    assert_eq!(
        selected(&down, &down_text),
        vec![
            ("/root/a".to_owned(), "pha".to_owned()),
            ("/root/b".to_owned(), "bra".to_owned()),
        ]
    );
}

/// Both ends in one run is the old single-node case, and it still works.
#[test]
fn both_ends_in_one_run_is_a_plain_range() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alphabet", Some(0)),
        run("/root/b", "bravo", Some(0)),
    ]);
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 5), ("/root/a", 1)),
    );
    assert_eq!(
        selected(&placements, &content),
        vec![("/root/a".to_owned(), "lpha".to_owned())],
        "ordered low-offset-first whichever way the drag went"
    );
}

/// A press that never dragged selects nothing, and paints nothing.
#[test]
fn a_collapsed_span_paints_nothing() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alpha", Some(0)),
    ]);
    let touched = resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 3), ("/root/a", 3)),
    );
    assert!(touched.is_empty());
    assert!(selected(&placements, &content).is_empty());
}

/// A control in the middle of a span keeps its own text, and the span closes
/// over it. This is a browser's `user-select: none`, and it is what stops a
/// toolbar's button labels landing on the clipboard between two paragraphs.
#[test]
fn a_control_in_the_middle_of_a_span_keeps_its_text() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "before", Some(0)),
        control("/root/btn", "Cancel", Some(0)),
        run("/root/b", "after", Some(0)),
    ]);
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 0), ("/root/b", 5)),
    );
    assert_eq!(
        selected(&placements, &content),
        vec![
            ("/root/a".to_owned(), "before".to_owned()),
            ("/root/b".to_owned(), "after".to_owned()),
        ]
    );
}

/// A control's *descendant* is the control's too, which is the case that
/// actually occurs: a button declares the opt-out and its label is a child.
#[test]
fn a_controls_child_run_is_the_controls() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "before", Some(0)),
        node("/root/btn", "", Some(0), NodeKind::Stack, true),
        run("/root/btn/label", "Cancel", Some(2)),
        run("/root/b", "after", Some(0)),
    ]);
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 0), ("/root/b", 5)),
    );
    assert_eq!(
        selected(&placements, &content),
        vec![
            ("/root/a".to_owned(), "before".to_owned()),
            ("/root/b".to_owned(), "after".to_owned()),
        ]
    );
}

/// An end this frame did not place covers nothing at all. The alternative —
/// running the span to the bottom of the page — would light up a screenful
/// of text because an application replaced one label under a live drag.
#[test]
fn a_span_with_a_missing_end_covers_nothing() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alpha", Some(0)),
        run("/root/b", "bravo", Some(0)),
    ]);
    let touched = resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 0), ("/root/gone", 2)),
    );
    assert!(touched.is_empty());
    assert!(selected(&placements, &content).is_empty());
}

/// The string got shorter between the frame the offsets were measured
/// against and this one. Clamping is what keeps that from being a panicking
/// slice inside the layout pass, on a code path the operator reaches by
/// dragging.
#[test]
fn offsets_are_clamped_to_the_string_this_frame_paints() {
    let (mut placements, mut content) =
        split(vec![box_of("/root", None), run("/root/a", "abc", Some(0))]);
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 1), ("/root/a", 99)),
    );
    assert_eq!(
        selected(&placements, &content),
        vec![("/root/a".to_owned(), "bc".to_owned())]
    );

    let (mut placements, mut content) =
        split(vec![box_of("/root", None), run("/root/a", "abc", Some(0))]);
    let touched = resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 40), ("/root/a", 99)),
    );
    assert!(
        touched.is_empty(),
        "a range entirely past the end is no highlight, not an empty one"
    );
}

/// An offset that lands inside a multi-byte character snaps outward rather
/// than panicking. Reachable the same way the clamp is: the offsets were
/// measured against a string this frame no longer paints.
#[test]
fn an_offset_inside_a_character_snaps_outward() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        // Four bytes each.
        run("/root/a", "日本語", Some(0)),
    ]);
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 1), ("/root/a", 5)),
    );
    assert_eq!(
        selected(&placements, &content),
        vec![("/root/a".to_owned(), "日本".to_owned())],
        "the whole of both characters the offsets landed inside"
    );
}

/// A blank run inside a span is skipped rather than carrying an empty range.
/// An empty highlight is not a picture, so it is not a payload.
#[test]
fn a_blank_run_inside_a_span_carries_nothing() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alpha", Some(0)),
        box_of("/root/gap", Some(0)),
        run("/root/b", "bravo", Some(0)),
    ]);
    let touched = resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 0), ("/root/b", 5)),
    );
    assert_eq!(touched, vec![1, 3]);
}

/// The digest sees the highlight. A frame with a selection painted into it is
/// a different picture from the same frame without one, so the placement's
/// paint hash has to move — the field's own doc calls a sink that leaves it
/// alone "the exact defect this field exists to close".
#[test]
fn a_selected_run_rehashes_its_payload() {
    let (mut placements, mut content) = split(vec![
        box_of("/root", None),
        run("/root/a", "alpha", Some(0)),
    ]);
    let before = placements[1].paint.paint_hash;
    resolve(
        &mut placements,
        &mut content,
        &span(("/root/a", 1), ("/root/a", 3)),
    );
    let after = placements[1].paint.paint_hash;
    assert_ne!(before, after);
    assert_eq!(
        after,
        crate::frame::digest::hash_paint_content(&content[1]),
        "the hash must be of the payload as it now stands"
    );
}
