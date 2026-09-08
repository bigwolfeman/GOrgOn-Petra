//! The shell end of the mount path (spec 005 T152,
//! `contracts/surface-contribution.md` §5, §6, §8, §9).
//!
//! Everything here drives the shipped `Host::pass` and asserts about the
//! frame it placed. Nothing reaches into the splice directly.
//!
//! # Why an integration test and not a unit test
//!
//! Two reasons, and the second is the one that matters. The first is that
//! `gorgon-petra-testkit` is a dev-dependency of this crate, so the headless
//! GPU snapshotter is reachable from here. The second is that this file can
//! only touch `gorgon_petra_egui`'s public surface, which is the same surface
//! spec 004's shell binary will hold: if a contribution cannot be mounted
//! from here, it cannot be mounted from there either.

use std::ops::Range;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput};
use gorgon_petra::frame::{PetrifiedFrame, Placement};
use gorgon_petra::geom::Axis;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::semantic::ContributionId;
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    AxisConstraint, ComponentRef, Constraints, InsetRefs, Interaction, NodeKind, Props, Role,
    ViewNode,
};
use gorgon_petra_egui::host::{App, Contribution, Host, MountOutcome, default_presenter};

/// The window every test in this file runs in, in logical points.
const WINDOW: [f32; 2] = [720.0, 360.0];

/// The key of the mount slot the fixture shell declares.
const SLOT: &str = "shell.status-bar";

/// A custom kind the fixture host registers, so a contribution naming it is
/// acceptable **only** against the live registry.
const HOSTED_KIND: &str = "shell.spark";

// ---------------------------------------------------------------- fixtures

/// A shell with a header, a status bar to mount into, and a body.
///
/// Deliberately three named nodes rather than one: a splice that ignored the
/// slot and appended to the root would still produce a frame, and only a tree
/// with somewhere else to go can tell the two apart.
#[derive(Default)]
struct Shell {
    /// Declare a second node keyed [`SLOT`], to make the slot ambiguous.
    twin: bool,
    /// Declare a node keyed `ui:1` in the slot, squatting on the mount path's
    /// own namespace.
    squatter: bool,
    /// Declare two focusable buttons, so Tab can start a focus-caret hop.
    focusable: bool,
    /// Report `ChangeSet::None` rather than `ChangeSet::All`.
    ///
    /// The honest answer for a shell whose own tree does not change, and the
    /// only setting under which the measure cache is a real cache. `All`
    /// clears it every frame, which masks every question about who
    /// invalidates what.
    quiet: bool,
}

impl RowSource for Shell {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Shell {
    fn view(&mut self) -> ViewNode {
        let mut bar = ViewNode::new(NodeKind::Stack, SLOT).with_props(Props {
            axis: Some(Axis::Horizontal),
            spacing: Some(token("spacing-05")),
            ..Props::default()
        });
        if self.squatter {
            bar = bar.child(text("ui:1", "squatter"));
        }
        let mut root = ViewNode::new(NodeKind::Stack, "root")
            .with_props(Props {
                axis: Some(Axis::Vertical),
                spacing: Some(token("spacing-05")),
                padding: Some(InsetRefs::all(token("spacing-05"))),
                // A ground, so the shell is a shell and not a white void the
                // theme's own ink is invisible against.
                tokens: [("background".to_owned(), token("surface.base"))]
                    .into_iter()
                    .collect(),
                ..Props::default()
            })
            .child(text("shell.header", "GOrgOn shell"))
            .child(bar)
            .child(text("shell.body", "application content"));
        if self.focusable {
            root = root.child(
                ViewNode::new(NodeKind::Stack, "shell.actions")
                    .with_props(Props {
                        axis: Some(Axis::Horizontal),
                        spacing: Some(token("spacing-05")),
                        ..Props::default()
                    })
                    .child(button("one"))
                    .child(button("two")),
            );
        }
        if self.twin {
            // Under a *different* parent. Two siblings under one key is
            // `Violation::DuplicateSiblingKey`, which refuses the tree
            // outright — a tree that never reaches the splice cannot show
            // what the splice does with an ambiguous slot.
            root = root.child(
                ViewNode::new(NodeKind::Stack, "shell.footer")
                    .child(ViewNode::new(NodeKind::Stack, SLOT)),
            );
        }
        root
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        if self.quiet {
            ChangeSet::None
        } else {
            ChangeSet::All
        }
    }
}

fn text(key: &str, body: &str) -> ViewNode {
    ViewNode::new(NodeKind::Text, key).with_props(Props {
        text: Some(body.to_owned()),
        tokens: [("foreground".to_owned(), token("text.primary"))]
            .into_iter()
            .collect(),
        ..Props::default()
    })
}

/// A focusable, labelled run — the least a node needs to enter focus order.
fn button(key: &str) -> ViewNode {
    let mut node = text(key, key);
    node.interactions = vec![Interaction::Focus];
    node.semantics.role = Some(Role::Button);
    node.semantics.label = Some(key.to_owned());
    node
}

fn token(name: &str) -> TokenName {
    TokenName::new(name).expect("a shipped token name")
}

/// A contribution whose whole tree is one named registry constructor.
fn names(id: u64, component: &str, params: &str) -> Contribution {
    at(id, 1, component, params)
}

/// [`names`], at a chosen publisher revision.
fn at(id: u64, revision: u64, component: &str, params: &str) -> Contribution {
    let mut node = ViewNode::new(NodeKind::Component, "surface");
    node.component = Some(ComponentRef {
        name: component.to_owned(),
        params: params
            .parse()
            .expect("the fixture's parameter table is JSON"),
    });
    Contribution {
        id: ContributionId::new(id),
        revision,
        slot: SLOT.to_owned(),
        tree: node,
    }
}

/// A `custom` node naming `kind`, sized so it needs no measurer.
///
/// The constraints are not decoration: `PaintReport::is_complete` counts a
/// placement with no area as silent, and `Host::pass` debug-asserts on a
/// silent placement.
fn hosted_kind_node(kind: &str) -> ViewNode {
    ViewNode::new(NodeKind::Custom, "spark")
        .with_props(Props {
            custom_kind: Some(kind.to_owned()),
            ..Props::default()
        })
        .with_constraints(Constraints {
            horizontal: AxisConstraint {
                min: Some(80.0),
                max: Some(80.0),
                priority: 10,
            },
            vertical: AxisConstraint {
                min: Some(24.0),
                max: Some(24.0),
                priority: 10,
            },
        })
}

/// A context that has been through one pass, so `pixels_per_point` and the
/// screen rect are real rather than defaulted.
fn headless() -> Context {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    ctx
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

/// A host over `app` that has drawn one frame.
fn host(app: Shell) -> (Context, Host<Shell>) {
    let ctx = headless();
    let mut host = Host::new(&ctx, app, default_presenter());
    host.registry_mut().register_custom_kind(HOSTED_KIND);
    // Registering the kind makes a tree acceptable; registering the painter
    // makes it drawn. Both, because `Host::pass` debug-asserts that no
    // placement went silent.
    host.painters_mut()
        .register(HOSTED_KIND, |_painter, _ctx| true);
    pass(&ctx, &mut host);
    (ctx, host)
}

fn pass(ctx: &Context, host: &mut Host<Shell>) {
    let owned = ctx.clone();
    owned
        .run_ui(sized(RawInput::default()), |_| host.pass(&owned))
        .drop_without_applying_deltas();
}

/// Every placement whose canonical id sits at or beneath `ui:<id>`.
fn beneath(frame: &PetrifiedFrame, id: u64) -> Vec<&Placement> {
    let prefix = format!("/root/{SLOT}/ui:{id}");
    frame
        .placements
        .iter()
        .filter(|p| p.id == prefix || p.id.starts_with(&format!("{prefix}/")))
        .collect()
}

/// Whether this frame is the one `refusal_view` paints, meaning the whole
/// tree was refused and the shell is blank apart from the violations.
fn is_blanked(frame: &PetrifiedFrame) -> bool {
    frame
        .placements
        .iter()
        .any(|p| p.id.starts_with("/petra-tree-refused"))
}

/// Every text run in the frame, joined, so an assertion can ask what the
/// picture says without knowing which node says it.
fn spoken(frame: &PetrifiedFrame) -> String {
    frame
        .placements
        .iter()
        .filter_map(|p| {
            p.semantics
                .value
                .clone()
                .or_else(|| p.semantics.label.clone())
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

// -------------------------------------------------------------------- G2

/// A contribution that names a component reaches layout as a real subtree.
///
/// This is the whole of "expand, then validate". `validate` refuses
/// `NodeKind::Component` by name and layout `unreachable!`s on it, so a
/// splice that ran the two in the other order would produce the refusal view
/// or a panic — never a placed tag. The assertion that the tag's own rect has
/// area is what separates "expanded" from "accepted and then dropped".
#[test]
fn a_named_component_is_expanded_before_it_is_validated() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "the shell was blanked: {}",
        spoken(frame)
    );
    let placed = beneath(frame, 7);
    assert!(
        !placed.is_empty(),
        "nothing was placed under /root/{SLOT}/ui:7; ids were {:?}",
        frame.placements.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    assert!(
        placed.iter().all(|p| p.kind != NodeKind::Component),
        "a component reference survived expansion into the frame"
    );
    let root = placed
        .iter()
        .find(|p| p.id.ends_with("ui:7"))
        .expect("the contribution's own root");
    assert!(
        root.rect.size().w > 0.0 && root.rect.size().h > 0.0,
        "the expanded tag has no area: {:?}",
        root.rect
    );
    assert_eq!(host.mounts()[0].outcome, MountOutcome::Mounted);
}

/// A component reference nested inside another component's parameters is
/// expanded too, because a plugin composes by nesting and the bottom-up walk
/// is the only thing that hands a constructor real nodes.
#[test]
fn a_component_nested_in_another_components_parameters_is_expanded() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![names(
        9,
        "unordered_list",
        r#"{
            "key": "items",
            "children": [
                { "kind": "component", "key": "a",
                  "component": { "name": "list_item",
                                 "params": { "key": "a", "label": "first" } } },
                { "kind": "component", "key": "b",
                  "component": { "name": "list_item",
                                 "params": { "key": "b", "label": "second" } } }
            ]
        }"#,
    )]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "the shell was blanked: {}",
        spoken(frame)
    );
    assert_eq!(host.mounts()[0].outcome, MountOutcome::Mounted);
    let placed = beneath(frame, 9);
    assert!(
        placed.len() > 2,
        "a two-item list expanded to {} placement(s)",
        placed.len()
    );
    assert!(
        placed.iter().all(|p| p.kind != NodeKind::Component),
        "a nested component reference survived expansion"
    );
}

// ------------------------------------------- keying, staleness, ordering

/// The splice key survives expansion, because keying happens after it.
///
/// `registry::expand` on a component-rooted tree returns what the
/// **constructor** built, under the constructor's own key. A shell that keyed
/// the tree first would hand that key straight back to the constructor's, and
/// two plugins in one list slot would then collide as
/// `Violation::DuplicateSiblingKey` — which refuses the whole tree, so one
/// plugin could refuse another's surface.
#[test]
fn the_splice_key_survives_expansion() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        frame
            .placements
            .iter()
            .any(|p| p.id == format!("/root/{SLOT}/ui:7")),
        "the contribution is not keyed by its id: {:?}",
        frame.placements.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
    assert!(
        !frame
            .placements
            .iter()
            .any(|p| p.id == format!("/root/{SLOT}/beta")),
        "the constructor's own key reached the slot, so keying ran before \
         expansion"
    );
}

/// Two plugins in one list slot coexist, because their keys are their ids.
#[test]
fn two_contributions_in_one_slot_do_not_refuse_each_other() {
    let (ctx, mut host) = host(Shell::default());
    // The same constructor with the same inner key, which is exactly the
    // case that collides if the shell does not re-key.
    host.set_contributions(vec![
        names(41, "tag", r#"{ "key": "same", "label": "one" }"#),
        names(42, "tag", r#"{ "key": "same", "label": "two" }"#),
    ]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "two contributions refused each other: {}",
        spoken(frame)
    );
    assert!(!beneath(frame, 41).is_empty());
    assert!(!beneath(frame, 42).is_empty());
}

/// A contribution whose fiber owes an answer is drawn stale, and its age is
/// published beside it.
///
/// Staleness is *behind*, not *old*: nothing here reads a clock, and a
/// surface nobody asked anything of is never stale however long it sits.
#[test]
fn a_contribution_that_owes_an_answer_is_drawn_stale() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    pass(&ctx, &mut host);
    let root = |host: &Host<Shell>| {
        host.frame()
            .expect("a frame")
            .placements
            .iter()
            .find(|p| p.id == format!("/root/{SLOT}/ui:7"))
            .expect("the contribution root")
            .semantics
            .stale
    };
    assert!(!root(&host), "a fresh contribution is drawn stale");

    assert!(host.contribution_behind(ContributionId::new(7)));
    pass(&ctx, &mut host);
    assert!(
        root(&host),
        "the surface is still drawn as current after its fiber fell behind"
    );
    let status = host
        .contribution_statuses()
        .into_iter()
        .find(|s| s.id == ContributionId::new(7))
        .expect("a published status");
    assert!(status.stale());
    assert!(
        status.age_frames > 0,
        "no age was published beside the surface"
    );

    // The answering push clears it, through the ordinary set path.
    host.set_contributions(vec![at(
        7,
        2,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    pass(&ctx, &mut host);
    assert!(
        !root(&host),
        "an answered contribution is still drawn stale"
    );
}

/// A push that is not newer than the picture on screen is dropped, not
/// promoted.
#[test]
fn an_out_of_order_push_does_not_replace_a_newer_picture() {
    let (ctx, mut host) = host(Shell {
        quiet: true,
        ..Shell::default()
    });
    host.set_contributions(vec![at(
        8,
        5,
        "tag",
        r#"{ "key": "t", "label": "a considerably longer label" }"#,
    )]);
    pass(&ctx, &mut host);
    let wide = beneath(host.frame().expect("a frame"), 8)[0].rect.size().w;

    // Revision 4 arrives after revision 5. Taking it would put an older
    // picture on screen.
    host.set_contributions(vec![at(8, 4, "tag", r#"{ "key": "t", "label": "i" }"#)]);
    pass(&ctx, &mut host);
    let after = beneath(host.frame().expect("a frame"), 8)[0].rect.size().w;
    assert_eq!(
        after, wide,
        "an out-of-order push replaced the newer picture"
    );
}

// -------------------------------------------------------------------- G3

/// Stage 2 runs against the host's **live** registry, not a fresh one.
///
/// `shell.spark` is registered on this host and on nothing else, so a
/// `custom` node naming it is acceptable only to a check that consulted that
/// registry. A splice that validated against
/// `Registry::with_vocabulary(standard_vocabulary())` — the weaker,
/// plugin-specific path §5 forbids — would refuse this and mount a card.
#[test]
fn stage_two_accepts_against_the_live_registry() {
    let (ctx, mut host) = host(Shell::default());
    let mut node = hosted_kind_node(HOSTED_KIND);
    node.semantics.label = Some("a host-registered kind".to_owned());
    host.set_contributions(vec![Contribution {
        id: ContributionId::new(11),
        revision: 1,
        slot: SLOT.to_owned(),
        tree: node,
    }]);
    pass(&ctx, &mut host);

    assert_eq!(
        host.mounts()[0].outcome,
        MountOutcome::Mounted,
        "a kind this host registered was refused, so stage 2 consulted some \
         other registry"
    );
    assert!(!beneath(host.frame().expect("a frame"), 11).is_empty());
}

/// The other half of the same claim: an *unregistered* custom kind is
/// refused. Without this, "accepted" above could mean "accepted everything".
#[test]
fn stage_two_refuses_a_kind_no_registry_knows() {
    let (ctx, mut host) = host(Shell::default());
    let node = hosted_kind_node("nobody.registered.this");
    host.set_contributions(vec![Contribution {
        id: ContributionId::new(12),
        revision: 1,
        slot: SLOT.to_owned(),
        tree: node,
    }]);
    pass(&ctx, &mut host);

    let MountOutcome::Refused(reason) = &host.mounts()[0].outcome else {
        panic!("expected a refusal, got {:?}", host.mounts()[0].outcome);
    };
    assert!(
        reason.contains("nobody.registered.this"),
        "the refusal does not name the kind: {reason}"
    );
}

// -------------------------------------------------------------------- G4

/// A refused contribution becomes an attributed card, and the shell keeps
/// drawing everything else.
///
/// Both halves matter. One bad plugin blanking the window is the failure this
/// exists to prevent, and a card with nothing on it that names the
/// contribution is a failure of a different kind: an operator who cannot tell
/// which plugin broke has to guess.
#[test]
fn a_refused_contribution_becomes_a_card_and_the_shell_keeps_drawing() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![
        names(3, "no_such_component_exists", r#"{ "key": "x" }"#),
        names(4, "tag", r#"{ "key": "ok", "label": "ok" }"#),
    ]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "one refused contribution blanked the whole shell"
    );
    // The application's own nodes are all still on screen.
    for id in ["/root/shell.header", "/root/shell.body"] {
        assert!(
            frame.placements.iter().any(|p| p.id == id),
            "{id} is gone from a frame that carried a refused contribution"
        );
    }
    // The good contribution beside the bad one still mounted.
    assert_eq!(host.mounts()[1].outcome, MountOutcome::Mounted);
    assert!(!beneath(frame, 4).is_empty());

    // The card is where the subtree would have been, keyed by the same id,
    // so `who-owns ui:3` still answers for it (§9).
    let card = beneath(frame, 3);
    assert!(!card.is_empty(), "no card was mounted for the refused ui:3");
    let said = spoken(frame);
    assert!(
        said.contains("ui:3"),
        "the card does not name the contribution: {said}"
    );
    // The card's own padding resolved, so its text is inside it rather than
    // flush against its edge. A token the live theme does not define is
    // dropped silently by construction; this is what makes that visible.
    let ground = card
        .iter()
        .find(|p| p.id.ends_with("ui:3"))
        .expect("the card's own rect");
    let headline = card
        .iter()
        .find(|p| p.id.ends_with("/headline"))
        .expect("the card's headline");
    assert!(
        headline.rect.origin().x > ground.rect.origin().x,
        "the card's padding did not resolve: headline at {} on a card at {}",
        headline.rect.origin().x,
        ground.rect.origin().x
    );
    let MountOutcome::Refused(reason) = &host.mounts()[0].outcome else {
        panic!("expected a refusal, got {:?}", host.mounts()[0].outcome);
    };
    assert!(
        reason.contains("no_such_component_exists"),
        "the refusal does not name the component: {reason}"
    );
}

/// A tree that nests past `registry::MAX_DEPTH` is refused by expansion
/// rather than recursing until the stack gives out.
#[test]
fn a_tree_that_nests_too_deep_is_refused_not_expanded() {
    let (ctx, mut host) = host(Shell::default());
    let mut node = ViewNode::new(NodeKind::Stack, "leaf");
    for level in 0..200 {
        node = ViewNode::new(NodeKind::Stack, format!("level-{level}")).child(node);
    }
    host.set_contributions(vec![Contribution {
        id: ContributionId::new(5),
        revision: 1,
        slot: SLOT.to_owned(),
        tree: node,
    }]);
    pass(&ctx, &mut host);

    let MountOutcome::Refused(reason) = &host.mounts()[0].outcome else {
        panic!("expected a refusal, got {:?}", host.mounts()[0].outcome);
    };
    assert!(
        reason.contains("nests deeper"),
        "the refusal is not the depth cap: {reason}"
    );
    assert!(!is_blanked(host.frame().expect("a frame")));
}

// -------------------------------------------------------------------- G5

/// The frame path pulls a retained snapshot and asks nobody anything.
///
/// `Contribution` holds an owned `ViewNode`: there is no channel, no
/// callback and no handle in it, so a frame has nothing to call into and no
/// timeout to tune (§8). What this test can show is the observable half —
/// twelve frames after a single `set_contributions`, with nothing touching
/// the host in between, still carry the surface, byte-identical.
#[test]
fn the_frame_path_keeps_the_last_snapshot_with_no_further_calls() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);

    let mut digests = Vec::new();
    for _ in 0..12 {
        pass(&ctx, &mut host);
        let frame = host.frame().expect("a frame");
        assert!(
            !beneath(frame, 7).is_empty(),
            "the retained snapshot stopped being drawn"
        );
        digests.push(frame.digest.hex());
    }
    assert!(
        digests.windows(2).all(|w| w[0] == w[1]),
        "an untouched contribution set produced {} different pictures",
        digests
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
}

// -------------------------------------------------- slots and collisions

/// A slot no node carries is reported, and its card goes to the root rather
/// than nowhere.
#[test]
fn a_slot_that_no_node_carries_is_reported_and_still_shown() {
    let (ctx, mut host) = host(Shell::default());
    let mut contribution = names(21, "tag", r#"{ "key": "t", "label": "t" }"#);
    contribution.slot = "shell.nowhere".to_owned();
    host.set_contributions(vec![contribution]);
    pass(&ctx, &mut host);

    assert_eq!(host.mounts()[0].outcome, MountOutcome::NoSuchSlot);
    let frame = host.frame().expect("a frame");
    assert!(!is_blanked(frame));
    assert!(
        frame.placements.iter().any(|p| p.id == "/root/ui:21"),
        "the orphan card was not mounted at the root: {:?}",
        frame.placements.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
}

/// Two nodes carrying the slot key is an ambiguity, not a race to be first.
///
/// Mounting under whichever the walk met first would put a plugin's surface
/// somewhere neither the plugin nor the shell author chose, and it would move
/// the day the shell reordered its own tree.
#[test]
fn an_ambiguous_slot_is_refused_rather_than_guessed() {
    let (ctx, mut host) = host(Shell {
        twin: true,
        ..Shell::default()
    });
    host.set_contributions(vec![names(22, "tag", r#"{ "key": "t", "label": "t" }"#)]);
    pass(&ctx, &mut host);

    assert_eq!(host.mounts()[0].outcome, MountOutcome::AmbiguousSlot(2));
    let frame = host.frame().expect("a frame");
    assert!(!is_blanked(frame));
    assert!(
        beneath(frame, 22).is_empty(),
        "an ambiguous slot was mounted into anyway"
    );
    assert!(
        frame.placements.iter().any(|p| p.id == "/root/ui:22"),
        "the ambiguity was not reported on screen"
    );
}

/// A shell node squatting on `ui:1` inside a mount slot is replaced, not
/// duplicated.
///
/// Two siblings under one key is `Violation::DuplicateSiblingKey`, which
/// refuses the tree it is in. Appending would therefore let one contribution
/// blank the whole window, which is the exact failure §5's error card exists
/// to prevent.
#[test]
fn a_contribution_replaces_a_squatter_rather_than_colliding_with_it() {
    let (ctx, mut host) = host(Shell {
        squatter: true,
        ..Shell::default()
    });
    host.set_contributions(vec![names(
        1,
        "tag",
        r#"{ "key": "t", "label": "mounted" }"#,
    )]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "a squatting key blanked the shell: {}",
        spoken(frame)
    );
    assert_eq!(host.mounts()[0].outcome, MountOutcome::Mounted);
    let roots = frame
        .placements
        .iter()
        .filter(|p| p.id == format!("/root/{SLOT}/ui:1"))
        .count();
    assert_eq!(roots, 1, "the mount point holds {roots} nodes keyed ui:1");
}

/// Contributions keep the order they were published in, never id order.
///
/// §6 makes that order `TraceSeq` of the `contribute` call, which the shell
/// cannot recompute. Sorting by id here would invent a second, disagreeing
/// order out of a number that only looks monotonic.
#[test]
fn contributions_keep_publication_order_not_id_order() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![
        names(70, "tag", r#"{ "key": "first", "label": "first" }"#),
        names(30, "tag", r#"{ "key": "second", "label": "second" }"#),
    ]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    let x = |id: u64| {
        frame
            .placements
            .iter()
            .find(|p| p.id == format!("/root/{SLOT}/ui:{id}"))
            .unwrap_or_else(|| panic!("ui:{id} was not placed"))
            .rect
            .origin()
            .x
    };
    assert!(
        x(70) < x(30),
        "the horizontal slot laid ui:30 out before ui:70, so the shell re-sorted"
    );
}

/// Republishing under the same id replaces the surface rather than stacking a
/// second one beside it, and the new content is measured as its own.
#[test]
fn republishing_one_id_replaces_the_surface_and_remeasures_it() {
    // `quiet`, so the application never clears the measure cache. The only
    // thing that can invalidate the republished surface's measurements is
    // `set_contributions` itself, which is the claim under test.
    let (ctx, mut host) = host(Shell {
        quiet: true,
        ..Shell::default()
    });
    host.set_contributions(vec![names(8, "tag", r#"{ "key": "t", "label": "i" }"#)]);
    pass(&ctx, &mut host);
    let narrow = beneath(host.frame().expect("a frame"), 8)[0].rect.size().w;

    host.set_contributions(vec![at(
        8,
        2,
        "tag",
        r#"{ "key": "t", "label": "a considerably longer label" }"#,
    )]);
    pass(&ctx, &mut host);
    let frame = host.frame().expect("a frame");
    let wide = beneath(frame, 8)[0].rect.size().w;

    assert_eq!(
        frame
            .placements
            .iter()
            .filter(|p| p.id == format!("/root/{SLOT}/ui:8"))
            .count(),
        1,
        "republishing left two surfaces under one id"
    );
    assert!(
        wide > narrow,
        "the replacement was laid out to the old content's width \
         ({wide} vs {narrow}), so the measure cache was not invalidated"
    );
}

/// Clearing the set takes the surface off the screen and leaves the
/// application's own tree exactly as it was before anything mounted.
#[test]
fn clearing_the_set_restores_the_untouched_tree() {
    let (ctx, mut host) = host(Shell::default());
    pass(&ctx, &mut host);
    let bare = host.frame().expect("a frame").digest.hex();

    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    pass(&ctx, &mut host);
    assert_ne!(host.frame().expect("a frame").digest.hex(), bare);

    host.set_contributions(Vec::new());
    pass(&ctx, &mut host);
    assert_eq!(
        host.frame().expect("a frame").digest.hex(),
        bare,
        "retracting every contribution did not restore the shell's own picture"
    );
    assert!(host.mounts().is_empty());
}

/// A surface mounted while the focus caret is mid-hop reaches the screen on
/// the very next frame.
///
/// `Host::can_reuse_frame` paints the last picture again while the caret
/// flies, without asking the application for a tree. A splice that only ran
/// on the rebuild path would make a contribution wait out the hop — up to
/// nine frames of a plugin's surface simply not being there, with nothing in
/// any frame record to say so.
#[test]
fn a_surface_mounted_mid_hop_is_on_the_very_next_frame() {
    let (ctx, mut host) = host(Shell {
        focusable: true,
        ..Shell::default()
    });
    let mut clock = 1.0;
    let tab = |host: &mut Host<Shell>, clock: &mut f64| {
        *clock += 1.0 / 60.0;
        let owned = ctx.clone();
        let mut raw = sized(RawInput::default());
        raw.time = Some(*clock);
        raw.events.push(egui::Event::Key {
            key: egui::Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        owned
            .run_ui(raw, |_| host.pass(&owned))
            .drop_without_applying_deltas();
    };
    tab(&mut host, &mut clock);
    tab(&mut host, &mut clock);
    assert!(
        host.caret().is_moving(),
        "the fixture never started a caret hop, so this test proves nothing"
    );

    host.set_contributions(vec![names(
        7,
        "tag",
        r#"{ "key": "beta", "label": "beta" }"#,
    )]);
    clock += 1.0 / 60.0;
    let owned = ctx.clone();
    let mut raw = sized(RawInput::default());
    raw.time = Some(clock);
    owned
        .run_ui(raw, |_| host.pass(&owned))
        .drop_without_applying_deltas();

    assert!(
        !beneath(host.frame().expect("a frame"), 7).is_empty(),
        "the surface waited out the caret hop instead of being drawn"
    );
}

// -------------------------------------------------------------------- G6

/// Rasterize a frame carrying one mounted surface and one refused one, on a
/// real GPU, and prove the picture is not blank.
///
/// The assertions here are deliberately coarse — a frame record cannot show
/// a surface drawn on top of itself or a card with no ink — and the PNG this
/// writes to `PETRA_SHOT_DIR` is what a human looks at. Skipped, loudly, when
/// no adapter answers; a machine with no GPU must still run the rest of this
/// file.
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn a_contributed_surface_rasterizes_to_more_than_one_colour() {
    use gorgon_petra_testkit::snapshot::Snapshotter;

    let ctx = headless();
    ctx.set_pixels_per_point(2.0);
    let mut host = Host::new(&ctx, Shell::default(), default_presenter());
    host.registry_mut().register_custom_kind(HOSTED_KIND);
    pass(&ctx, &mut host);
    host.set_contributions(vec![
        names(7, "tag", r#"{ "key": "beta", "label": "contributed tag" }"#),
        names(3, "no_such_component_exists", r#"{ "key": "x" }"#),
    ]);
    pass(&ctx, &mut host);

    let owned = ctx.clone();
    let output = owned.run_ui(sized(RawInput::default()), |_| host.pass(&owned));
    let frame = host.frame().expect("a frame");
    assert!(!is_blanked(frame));
    assert!(!beneath(frame, 7).is_empty(), "nothing to photograph");

    let mut shooter = Snapshotter::new();
    let shot = match shooter.capture(&ctx, &output, frame, None) {
        Ok(shot) => shot,
        Err(err) => {
            eprintln!("SKIPPED: no GPU adapter answered ({err:?})");
            output.drop_without_applying_deltas();
            return;
        }
    };
    output.drop_without_applying_deltas();

    if let Some(dir) = std::env::var_os("PETRA_SHOT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).expect("shot dir");
        std::fs::write(dir.join("ui-contribution.png"), &shot.png).expect("write shot");
    }

    let image = image::load_from_memory(&shot.png)
        .expect("the shot is a PNG")
        .to_rgba8();
    let (w, h) = image.dimensions();
    assert_eq!(
        (w, h),
        ((WINDOW[0] * 2.0) as u32, (WINDOW[1] * 2.0) as u32),
        "the shot is not the window"
    );
    let distinct: std::collections::BTreeSet<[u8; 4]> = image.pixels().map(|p| p.0).collect();
    assert!(
        distinct.len() > 1,
        "the page rasterized to a single colour, so nothing was drawn"
    );
}
