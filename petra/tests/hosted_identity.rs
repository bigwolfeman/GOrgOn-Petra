//! The one thing a frame digest cannot see, pinned.
//!
//! `contracts/frame-identity.md`, "Not covered", says it plainly: for a
//! `custom` node the paint payload stream hashes the **painter's name**, and
//! for an `image` node it hashes the **source string**. Neither hashes what
//! appeared. So two frames in which one registered painter drew two completely
//! different pictures into the same rect carry the **same digest**.
//!
//! That exclusion is right and cannot be closed by hashing harder. Petra does
//! not produce those pixels; a digest that read them back would depend on a GPU
//! readback and on a driver version — the exact nondeterminism the first
//! exclusion in that list bars, and the thing SC-004's cross-target digest
//! equality would lose first.
//!
//! What the exclusion costs is one live claim, and only one:
//!
//! - **SC-004** — 100 identical petrifications produce one digest — is
//!   untouched. Petra's contribution to a hosted node (rect, clip, opacity,
//!   painter name) is as deterministic as everything else.
//! - **FR-040** — screenshot verification — is *weaker over hosted content*.
//!   "This screenshot is of the frame with this digest" still holds. "Two
//!   screenshots with one digest are the same picture" does not.
//!
//! So the frame says which claim a consumer is holding: `PetrifiedFrame::hosted`
//! (FR-060). These tests are the pin on both halves. They fail if a future
//! change quietly **closes** the hole (the two digests would stop matching) and
//! they fail if one quietly **widens** it (the controls below show the digest
//! still sees text, tokens, and geometry). Either way someone has to come back
//! here and to the contract together.
//!
//! The painters here are the test's own, in the same spirit as
//! `testing::MonoContent` standing in for a real shaper: `gorgon-petra` has no
//! renderer to ask, and the claim under test is precisely about output Petra
//! never sees. Each painter produces a byte picture, so "drew a different
//! picture" is something this file can assert rather than assume.

use std::collections::BTreeMap;

use gorgon_petra::frame::{PetrifiedFrame, TransitionActivity, Viewport, petrify};
use gorgon_petra::geom::{Rect, Size};
use gorgon_petra::testing::{Harness, MonoContent, NoRows, validated_with};
use gorgon_petra::token::ThemeMode;
use gorgon_petra::tree::{AxisConstraint, Constraints, NodeKind, Props, Registry, ViewNode};

/// The painter name both frames declare. One name, two pictures — that is the
/// whole point.
const PAINTER: &str = "gauge";
/// A second registered painter, used only to show that the *name* is hashed
/// even though what it draws is not.
const OTHER_PAINTER: &str = "dial";
/// The image source both frames declare. One string, two decodings.
const SOURCE: &str = "camera://0";
const GAUGE_ID: &str = "/root/gauge";
const PHOTO_ID: &str = "/root/photo";
const VIEWPORT: Size = Size { w: 200.0, h: 100.0 };

/// What a painter put on screen: one byte per cell of a coarse raster across
/// the rect it was handed.
///
/// Coarse on purpose. A real painter's output depends on its rect and on
/// whatever host state it reads, and those two are all this file needs to tell
/// two pictures apart.
type Picture = Vec<u8>;

/// A horizontal bar filling `fill` of `rect`, rastered one cell per logical
/// unit of width.
///
/// The rect is an input, so a painter handed a different box draws a different
/// picture — which is what makes the geometry control below mean something.
fn bar(rect: Rect, fill: f32) -> Picture {
    let cells = rect.w.max(0.0) as usize;
    let filled = (fill * cells as f32).round() as usize;
    (0..cells)
        .map(|i| if i < filled { b'#' } else { b'.' })
        .collect()
}

/// The host side of the two hosted kinds: painters registered by name, images
/// resolved by source string.
///
/// This is the boundary the digest stops at. Petra hands over a name and a
/// rect; everything that decides the pixels lives in here, where no hash can
/// reach it.
struct Host {
    painters: BTreeMap<&'static str, Box<dyn Fn(Rect) -> Picture>>,
    images: BTreeMap<&'static str, Picture>,
}

impl Host {
    /// A host whose `gauge` painter reads `fill` and whose `camera://0` decodes
    /// to a picture of `brightness`.
    ///
    /// Both are ordinary host state — a sensor reading, a camera frame. Neither
    /// appears anywhere in the view tree, which is exactly why the digest
    /// cannot see either one move.
    fn new(fill: f32, brightness: u8) -> Self {
        let mut painters: BTreeMap<&'static str, Box<dyn Fn(Rect) -> Picture>> = BTreeMap::new();
        painters.insert(PAINTER, Box::new(move |rect| bar(rect, fill)));
        let mut images = BTreeMap::new();
        images.insert(SOURCE, vec![brightness; 4]);
        Self { painters, images }
    }

    /// Everything in `frame` this host draws, keyed by placement id, in tree
    /// pre-order.
    ///
    /// Deliberately walks every placement and decides from the payload itself,
    /// **not** from [`PetrifiedFrame::hosted_placements`]. A renderer does the
    /// same — it dispatches on what the payload declares — but the reason to
    /// insist on it here is that `hosted` and `hosted_placements` are under
    /// test. A fixture that located its own hosted regions through the flag
    /// would go blind the moment the flag broke, and every assertion below it,
    /// including the digest one, would stop being reached. That failure mode is
    /// not hypothetical: this file had it, and the sabotage run that proves the
    /// flag assertion has teeth is what found it.
    fn render(&self, frame: &PetrifiedFrame) -> Vec<(String, Picture)> {
        frame
            .drawn()
            .filter_map(|(placement, content)| {
                let picture = match (content.custom.as_deref(), content.image.as_deref()) {
                    (Some(name), _) => {
                        let painter = self
                            .painters
                            .get(name)
                            .unwrap_or_else(|| panic!("no painter registered as {name:?}"));
                        painter(placement.rect)
                    }
                    (None, Some(source)) => self
                        .images
                        .get(source)
                        .unwrap_or_else(|| panic!("no image for {source:?}"))
                        .clone(),
                    // Text and containers: Petra's own renderer draws these,
                    // and the digest covers them.
                    (None, None) => return None,
                };
                Some((placement.id.clone(), picture))
            })
            .collect()
    }
}

/// A tree with one custom node of the given key beside a caption.
///
/// The caption is here so the frame is not *only* hosted content: the flag has
/// to survive a mixed frame, and the text is what the controls below move to
/// show the digest is still awake.
fn gauge_tree(caption: &str) -> ViewNode {
    gauge_tree_with(caption, PAINTER, None)
}

/// [`gauge_tree`] with a chosen painter name and a chosen minimum height for
/// the gauge.
///
/// The two controls below each move exactly one of those. `painter` changes a
/// hashed payload string with the geometry fixed; `min_h` changes the rect with
/// every hashed string fixed — a node's constraints are not themselves a digest
/// input, only the rect they produce is.
fn gauge_tree_with(caption: &str, painter: &str, min_h: Option<f32>) -> ViewNode {
    let mut gauge = ViewNode::new(NodeKind::Custom, "gauge").with_props(Props {
        custom_kind: Some(painter.to_string()),
        ..Props::default()
    });
    if let Some(min) = min_h {
        gauge = gauge.with_constraints(Constraints {
            vertical: AxisConstraint {
                min: Some(min),
                ..AxisConstraint::default()
            },
            ..Constraints::default()
        });
    }
    ViewNode::new(NodeKind::Stack, "root")
        .child(text_node("caption", caption))
        .child(gauge)
}

fn photo_tree() -> ViewNode {
    ViewNode::new(NodeKind::Stack, "root")
        .child(text_node("caption", "live"))
        .child(ViewNode::new(NodeKind::Image, "photo").with_props(Props {
            image: Some(SOURCE.to_string()),
            ..Props::default()
        }))
}

fn text_node(key: &str, body: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(body.to_string()),
        ..Props::default()
    })
}

/// Petrify `tree` at `seq` against [`VIEWPORT`].
///
/// Every call builds a fresh harness, so two frames share no measurement cache
/// and no layout state: whatever they end up agreeing on, they agreed on
/// independently rather than by reading one cached answer twice.
fn frame(seq: u64, tree: &ViewNode) -> PetrifiedFrame {
    // Both painter names are registered, so a tree that renames its painter
    // still passes acceptance: the rename must reach the digest as a payload
    // change, not as a validation failure.
    let mut registry = Registry::new();
    registry.register_custom_kind(PAINTER);
    registry.register_custom_kind(OTHER_PAINTER);
    let mut harness = Harness::with(MonoContent::default(), NoRows);
    petrify(
        seq,
        validated_with(tree, &registry),
        &mut harness.ctx(),
        Viewport::new(VIEWPORT, ThemeMode::Dark),
        TransitionActivity::default(),
    )
}

/// The documented limitation itself: one painter, one rect, two pictures, one
/// digest — and both frames say they are hosted.
///
/// The assertions are ordered so a failure names which half broke. The picture
/// inequality comes first, because if the two pictures were equal the digest
/// equality below would prove nothing at all. Then the digest equality, which
/// is the limitation. Then the flag, which is the mitigation FR-060 requires:
/// break the flag and this test still reaches the digest assertion and passes
/// it, and fails here instead.
#[test]
fn one_painter_two_pictures_one_digest() {
    let tree = gauge_tree("load");
    let first = frame(1, &tree);
    let second = frame(2, &tree);

    let quiet = Host::new(0.10, 0).render(&first);
    let loud = Host::new(0.90, 0).render(&second);
    assert_eq!(
        quiet.len(),
        1,
        "the fixture must paint exactly one hosted region: {quiet:?}"
    );
    assert_ne!(
        quiet, loud,
        "the two hosts must really draw different pictures, or this test \
         proves nothing about the digest"
    );

    let (gauge_a, gauge_b) = (
        first.placement(GAUGE_ID).expect("frame 1 places the gauge"),
        second
            .placement(GAUGE_ID)
            .expect("frame 2 places the gauge"),
    );
    assert_eq!(
        gauge_a.rect, gauge_b.rect,
        "the two pictures must land in the same rect, or the digests would \
         have geometry to differ on"
    );

    assert_eq!(
        first.digest, second.digest,
        "contracts/frame-identity.md, \"Not covered\": the payload stream \
         hashes the painter's NAME, not what it drew. If this now fails, the \
         digest gained sight of hosted pixels — update the contract, the \
         stream prefixes, and this test together, and say what it costs \
         SC-004's cross-target equality."
    );

    assert!(
        first.hosted() && second.hosted(),
        "a frame the digest cannot fully see must say so (FR-060): a consumer \
         reading the flag as false would take these two digests as proof of \
         one picture"
    );
    for (label, frame) in [("first", &first), ("second", &second)] {
        let hosted: Vec<&str> = frame
            .hosted_placements()
            .map(|(p, _)| p.id.as_str())
            .collect();
        assert_eq!(
            hosted,
            [GAUGE_ID],
            "{label}: the gauge is the region a pixel comparison must cover, \
             and the caption and the stack are not"
        );
    }
}

/// The same hole on the image side: one source string, two decodings.
///
/// A file that changed on disk, a `camera://` source, a URL behind a CDN — the
/// source string is stable and the pixels are not.
#[test]
fn one_image_source_two_decodings_one_digest() {
    let tree = photo_tree();
    let first = frame(1, &tree);
    let second = frame(2, &tree);

    let dark = Host::new(0.5, 0).render(&first);
    let bright = Host::new(0.5, 255).render(&second);
    assert_ne!(dark, bright, "the two decodings must really differ");

    assert_eq!(
        first.digest, second.digest,
        "the payload stream hashes the image SOURCE, not its pixels \
         (contracts/frame-identity.md, \"Not covered\")"
    );
    let hosted: Vec<&str> = first
        .hosted_placements()
        .map(|(p, _)| p.id.as_str())
        .collect();
    assert_eq!(
        hosted,
        [PHOTO_ID],
        "the image is the hosted placement; the stack and the caption are not"
    );
    assert!(first.hosted() && second.hosted());
}

/// Control: the hole is exactly the pixels, and nothing next to them.
///
/// Everything Petra *does* decide about a hosted node still enters the digest.
/// Move the gauge's box and the digest moves with it, under the same painter
/// name. Without this, `one_painter_two_pictures_one_digest` would also pass
/// against a digest function that had stopped hashing anything.
#[test]
fn the_digest_still_sees_everything_petra_decides() {
    let tree = gauge_tree("load");
    let base = frame(1, &tree);

    let taller = frame(2, &gauge_tree_with("load", PAINTER, Some(64.0)));
    assert_eq!(
        base.placement("/root/caption").unwrap().rect,
        taller.placement("/root/caption").unwrap().rect,
        "only the gauge may move, or this control would pass on the caption"
    );
    assert_ne!(
        base.placement(GAUGE_ID).unwrap().rect,
        taller.placement(GAUGE_ID).unwrap().rect,
        "the fixture must actually resize the gauge"
    );
    assert_ne!(
        base.digest, taller.digest,
        "the rect a hosted node was given is Petra's own output and is hashed, \
         under an unchanged painter name"
    );

    let relabelled = frame(3, &gauge_tree("idle"));
    assert_eq!(
        base.placement(GAUGE_ID).unwrap().rect,
        relabelled.placement(GAUGE_ID).unwrap().rect,
        "the caption change must not move the gauge, or this control would \
         pass on geometry instead of on text"
    );
    assert_ne!(
        base.digest, relabelled.digest,
        "text is hashed by content, not by name: a changed caption is a \
         changed frame"
    );

    let renamed = frame(4, &gauge_tree_with("load", OTHER_PAINTER, None));
    assert_ne!(
        base.digest, renamed.digest,
        "the painter's NAME is hashed even though its output is not"
    );
}

/// Control: the flag is not stuck on.
///
/// A frame of text and containers is fully covered by its digest, and must say
/// so — otherwise every consumer of every ordinary frame would be told to fall
/// back to pixel comparison and FR-040 would be worth nothing.
#[test]
fn a_frame_without_hosted_content_says_so() {
    let tree = ViewNode::new(NodeKind::Stack, "root")
        .child(text_node("caption", "load"))
        .child(ViewNode::new(NodeKind::Spacer, "gap"));
    let plain = frame(1, &tree);

    assert!(plain.placements.len() >= 3, "{:?}", plain.placements);
    assert!(
        !plain.hosted(),
        "nothing here is drawn by a host: {:?}",
        plain
            .hosted_placements()
            .map(|(p, _)| p.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(plain.hosted_placements().count(), 0);
    assert!(
        Host::new(0.1, 0).render(&plain).is_empty(),
        "there is nothing for a host to paint in this frame"
    );
    assert!(
        plain.paint_hashes_agree(),
        "the payloads and their hashes must still agree, or `hosted` would be \
         reading an array the digest no longer describes"
    );
}

/// A registered painter is deterministic *for Petra*: SC-004 is untouched.
///
/// The limitation above is about the host's output, not about Petra's. Repeat
/// the petrification of a hosted tree and the digest must be identical every
/// time, or the exclusion would be covering for real nondeterminism inside this
/// crate rather than for a boundary outside it.
#[test]
fn petrifying_a_hosted_tree_is_still_deterministic() {
    let tree = gauge_tree("load");
    let want = frame(1, &tree).digest;
    for seq in 2..=100 {
        assert_eq!(
            frame(seq, &tree).digest,
            want,
            "petrify {seq} of a hosted tree disagreed with petrify 1 (SC-004)"
        );
    }
}
