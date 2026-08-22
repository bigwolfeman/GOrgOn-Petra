//! The frame digest covers what the frame draws.
//!
//! `contracts/frame-identity.md` promises that two frames with one digest are
//! two identical pictures. Under domain `gorgon-petra-frame-v1` that promise
//! was false for everything in `PaintContent`: a node's token bindings, its
//! typography token, its wrap policy, its line cap, its custom painter name,
//! and an input's placeholder all decided the picture and none of them reached
//! the hash. `paint.token_revision` is one global number — it says which theme
//! was in force, never which token this node asked for.
//!
//! Every test here is one of those blind spots, driven through the real
//! `petrify` path rather than through hand-built placements: the payload is
//! attached by the dispatcher and hashed by the sink, and a fix that only
//! worked on a hand-assembled `Placement` would prove nothing.
//!
//! Each case is a pair of trees that differ in exactly one field and lay out
//! to exactly the same rects. That is what makes the assertion sharp — the
//! digest cannot pass by noticing a geometry change instead.

use gorgon_petra::frame::{FrameDigest, PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::Size;
use gorgon_petra::testing::{Harness, validated_with};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{Interaction, NodeKind, Props, Registry, Role, TextWrap, ViewNode};

/// Every custom kind name a tree in this file declares. The digest tests
/// exercise custom-painter naming, not acceptance, so the registry has to
/// know both names or `frame`/`dig` would refuse trees this file's own tests
/// build on purpose.
fn registry() -> Registry {
    let mut registry = Registry::new();
    registry.register_custom_kind("sparkline");
    registry.register_custom_kind("gauge");
    registry
}

fn frame(tree: &ViewNode) -> PetrifiedFrame {
    let mut harness = Harness::new();
    petrify(
        1,
        validated_with(tree, &registry()),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

fn dig(tree: &ViewNode) -> FrameDigest {
    frame(tree).digest
}

/// Assert two trees draw differently, and that nothing but the field under
/// test moved — if the rects differ, the digest would separate them for the
/// wrong reason and the test would prove nothing about paint coverage.
fn differs_on_paint_alone(what: &str, a: &ViewNode, b: &ViewNode) {
    let (fa, fb) = (frame(a), frame(b));
    let rects_a: Vec<_> = fa
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.rect))
        .collect();
    let rects_b: Vec<_> = fb
        .placements
        .iter()
        .map(|p| (p.id.clone(), p.rect))
        .collect();
    assert_eq!(
        rects_a, rects_b,
        "{what}: the two trees lay out differently, so this case cannot tell \
         whether the digest sees the paint change or only the geometry one"
    );
    assert_ne!(
        fa.digest, fb.digest,
        "{what} decides the picture and the frame digest cannot see it"
    );
}

fn tokened(slot: &str, value: &str) -> ViewNode {
    let mut props = Props::default();
    props.tokens.insert(slot.into(), value.into());
    ViewNode::new(NodeKind::Stack, "panel").with_props(props)
}

/// The headline case: rebind one node's background and repaint the panel in a
/// different colour. Nothing moves, no theme revision changes.
#[test]
fn a_rebound_token_moves_the_digest() {
    differs_on_paint_alone(
        "background: surface.raised vs status.down",
        &tokened("background", "surface.raised"),
        &tokened("background", "status.down"),
    );
}

#[test]
fn declaring_a_token_at_all_moves_the_digest() {
    differs_on_paint_alone(
        "no background vs a background",
        &ViewNode::new(NodeKind::Stack, "panel"),
        &tokened("background", "surface.raised"),
    );
}

/// The same token in a different slot is a different picture: `background`
/// fills the rect, `foreground` colours the text.
#[test]
fn the_slot_a_token_is_bound_to_moves_the_digest() {
    differs_on_paint_alone(
        "surface.raised as background vs as foreground",
        &tokened("background", "surface.raised"),
        &tokened("foreground", "surface.raised"),
    );
}

fn text_node(props: Props) -> ViewNode {
    ViewNode::new(NodeKind::Text, "t").with_props(props)
}

fn labelled(extra: impl FnOnce(&mut Props)) -> ViewNode {
    let mut props = Props {
        text: Some("hi".into()),
        ..Props::default()
    };
    extra(&mut props);
    text_node(props)
}

/// Same words, two typography tokens. The engine's `content_hash` covers the
/// string and nothing else, and the fake measurer sizes every style alike, so
/// before v2 these were one frame.
#[test]
fn a_typography_token_moves_the_digest() {
    differs_on_paint_alone(
        "style: none vs heading",
        &labelled(|_| {}),
        &labelled(|p| p.style = Some("heading".into())),
    );
}

/// A wrap policy that changes nothing for a run this short still changes what
/// the shaper is told, and so what a longer run would draw.
#[test]
fn a_wrap_policy_moves_the_digest() {
    differs_on_paint_alone(
        "wrap vs clip",
        &labelled(|p| p.wrap = Some(TextWrap::Wrap)),
        &labelled(|p| p.wrap = Some(TextWrap::Clip)),
    );
}

#[test]
fn a_line_cap_moves_the_digest() {
    differs_on_paint_alone(
        "max_lines: none vs 3",
        &labelled(|_| {}),
        &labelled(|p| p.max_lines = Some(3)),
    );
}

/// The registered painter's name is the whole content of a custom node, and
/// `leaf::place` hashes nothing for a custom kind.
#[test]
fn a_custom_painter_name_moves_the_digest() {
    let named = |name: &str| {
        ViewNode::new(NodeKind::Custom, "c").with_props(Props {
            custom_kind: Some(name.into()),
            ..Props::default()
        })
    };
    differs_on_paint_alone(
        "custom: sparkline vs gauge",
        &named("sparkline"),
        &named("gauge"),
    );
}

/// An empty field draws its placeholder, and `content_hash` for an `input`
/// hashes `props.text` — which is empty in both of these.
#[test]
fn an_input_placeholder_moves_the_digest() {
    let field = |placeholder: &str| {
        ViewNode::new(NodeKind::Input, "f").with_props(Props {
            placeholder: Some(placeholder.into()),
            ..Props::default()
        })
    };
    differs_on_paint_alone(
        "placeholder: Filter fibers… vs Search",
        &field("Filter fibers…"),
        &field("Search"),
    );
}

/// Already covered before v2, through `leaf::place` hashing the source into
/// `content_hash`. Pinned here so the two routes cannot both be removed on the
/// theory that the other one has it.
#[test]
fn an_image_source_moves_the_digest() {
    let img = |src: &str| {
        ViewNode::new(NodeKind::Image, "i").with_props(Props {
            image: Some(src.into()),
            ..Props::default()
        })
    };
    differs_on_paint_alone("image: a.png vs b.png", &img("a.png"), &img("b.png"));
}

/// A frame's paint hashes describe the payload the frame actually carries.
///
/// Both arrays are public fields, so this can be made false by editing one of
/// them after petrify — which would leave the digest describing a picture the
/// renderer no longer draws. The check has to be able to say no, so the second
/// half proves it does.
#[test]
fn a_petrified_frames_paint_hashes_describe_its_payload() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .with_props({
            let mut p = Props::default();
            p.tokens.insert("background".into(), "surface.base".into());
            p
        })
        .child(labelled(|p| p.style = Some("heading".into())));
    let mut f = frame(&tree);
    assert!(f.paint_hashes_agree(), "{:?}", f.placements);

    f.content[0]
        .tokens
        .insert("background".into(), "status.down".into());
    assert!(
        !f.paint_hashes_agree(),
        "a payload edited after petrify must not keep reading as consistent"
    );

    let mut truncated = frame(&tree);
    truncated.content.pop();
    assert!(
        !truncated.paint_hashes_agree(),
        "a tail with no hash to compare against is a disagreement"
    );
}

/// SC-004's determinism claim, over a tree that exercises every payload field.
#[test]
fn a_payload_carrying_tree_digests_identically_a_hundred_times() {
    let mut panel = Props::default();
    panel
        .tokens
        .insert("background".into(), "surface.base".into());
    panel
        .tokens
        .insert("border".into(), "surface.raised".into());
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .with_props(panel)
        .child(labelled(|p| {
            p.style = Some("heading".into());
            p.wrap = Some(TextWrap::Ellipsis);
            p.max_lines = Some(1);
        }))
        .child(ViewNode::new(NodeKind::Image, "logo").with_props(Props {
            image: Some("logo.png".into()),
            ..Props::default()
        }))
        .child(ViewNode::new(NodeKind::Custom, "spark").with_props(Props {
            custom_kind: Some("sparkline".into()),
            ..Props::default()
        }));

    let first = dig(&tree);
    for _ in 0..100 {
        assert_eq!(dig(&tree), first);
    }
}

// --- Keyboard focus -------------------------------------------------------
//
// `gorgon-petra-egui` paints a focus ring, so which node holds keyboard focus
// decides the picture. It is the one member of `PlacementSemantics` the digest
// covers, and covering it is why the frame prefix is `v3`. Before that, a
// screenshot consumer verifying `(seq, digest)` would have accepted a capture
// of the wrong node ringed.

/// The same tree, placed from a state that focuses `id`.
fn frame_focused(tree: &ViewNode, id: Option<&str>) -> PetrifiedFrame {
    let mut harness = Harness::new();
    harness.state.focused = id.map(str::to_owned);
    petrify(
        1,
        validated_with(tree, &registry()),
        &mut harness.ctx(),
        Viewport::new(Size::new(400.0, 200.0), ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// Two focusable siblings, laid out identically whatever holds focus.
fn two_buttons() -> ViewNode {
    let button = |key: &str, label: &'static str| {
        ViewNode::new(NodeKind::Text, key)
            .with_props(Props {
                text: Some(label.into()),
                ..Props::default()
            })
            .interactive(
                Role::Button,
                label,
                &[Interaction::Click, Interaction::Focus],
            )
    };
    ViewNode::new(NodeKind::Stack, "root")
        .child(button("run", "Run"))
        .child(button("stop", "Stop"))
}

/// Assert two focus states are two frames, and that only focus moved — a
/// digest that separated them by noticing a moved rect would prove nothing.
fn differs_on_focus_alone(what: &str, a: Option<&str>, b: Option<&str>) {
    let tree = two_buttons();
    let (fa, fb) = (frame_focused(&tree, a), frame_focused(&tree, b));
    let rects = |f: &PetrifiedFrame| -> Vec<(String, gorgon_petra::geom::Rect)> {
        f.placements
            .iter()
            .map(|p| (p.id.clone(), p.rect))
            .collect()
    };
    assert_eq!(
        rects(&fa),
        rects(&fb),
        "{what}: the two frames lay out differently, so this case cannot tell \
         whether the digest sees focus or only geometry"
    );
    assert_ne!(
        fa.digest, fb.digest,
        "{what} decides the picture — a focus ring is painted from it — and \
         the frame digest cannot see it"
    );
}

#[test]
fn focusing_a_node_moves_the_digest() {
    differs_on_focus_alone("nothing focused vs /root/run", None, Some("/root/run"));
}

#[test]
fn moving_focus_between_two_nodes_moves_the_digest() {
    differs_on_focus_alone(
        "/root/run vs /root/stop",
        Some("/root/run"),
        Some("/root/stop"),
    );
}

/// The flag names one node. A focused frame that marked every placement would
/// pass both tests above and ring the whole window.
#[test]
fn exactly_the_focused_placement_carries_the_flag() {
    let frame = frame_focused(&two_buttons(), Some("/root/stop"));
    let flagged: Vec<&str> = frame
        .placements
        .iter()
        .filter(|p| p.semantics.focused)
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(flagged, ["/root/stop"]);
    assert_eq!(frame.placements.len(), 3, "root plus two buttons");

    let none = frame_focused(&two_buttons(), None);
    assert!(none.placements.iter().all(|p| !p.semantics.focused));
}

/// A focused id naming a node this frame does not contain leaves every
/// placement unflagged — and leaves the digest equal to the unfocused frame,
/// because the picture is the same one.
#[test]
fn a_focused_id_outside_the_frame_rings_nothing() {
    let tree = two_buttons();
    let stale = frame_focused(&tree, Some("/root/gone"));
    assert!(stale.placements.iter().all(|p| !p.semantics.focused));
    assert_eq!(stale.digest, frame_focused(&tree, None).digest);
}
