//! The retained-binding seam through the shipped `Host`
//! (design §5, §7, §8 of
//! `.agents/notes/proposed/architecture/2026-09-27-bound-slot-table-ui-model.md`,
//! task R-1).
//!
//! Everything here drives `Host::pass` and asserts about the frame it placed
//! and the `(node, property)` lists the host reports — the same public
//! surface spec 004's shell binary will hold. The evaluator and the reverse
//! index are proved at the unit level in `gorgon_petra::tree::binding_eval`;
//! what is proved here is the host half: snapshot-first delivery, the
//! reverse index across the mounted trees, `apply_slot_changes` landing
//! exactly the bound properties (before and after, on screen), the card that
//! gives way when values arrive, and the stale-version refusal.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::semantic::ContributionId;
use gorgon_petra::token::TokenName;
use gorgon_petra::tree::{
    DeriveExpr, NodeKind, PropKey, PropVal, Props, SlotChange, SlotKey, SlotValue, ViewNode,
};
use gorgon_petra_egui::host::{App, Contribution, Host, MountOutcome, default_presenter};

/// The window every test in this file runs in, logical points.
const WINDOW: [f32; 2] = [720.0, 360.0];

/// The mount slot the fixture shell declares.
const SLOT: &str = "shell.status-bar";

// ---------------------------------------------------------------- fixtures

/// A shell with one mount slot, and optionally one bound property of its
/// own, so the application's own tree can be driven through the same seam.
#[derive(Default)]
struct Shell {
    /// The header's `label` binds slot `title` instead of being literal.
    binds: bool,
}

impl RowSource for Shell {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Shell {
    fn view(&mut self) -> ViewNode {
        let mut header = text("shell.header", "GOrgOn shell");
        if self.binds {
            header
                .bound
                .set(PropKey::Label, PropVal::Bind(SlotKey::new("title")));
        }
        ViewNode::new(NodeKind::Stack, "root")
            .child(header)
            .child(ViewNode::new(NodeKind::Stack, SLOT))
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        // The honest answer for a shell whose own tree does not change, and
        // the only setting under which a slot change's own invalidation is
        // visible rather than masked by `All`.
        ChangeSet::None
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

fn token(name: &str) -> TokenName {
    TokenName::new(name).expect("a shipped token name")
}

fn str_value(text: &str) -> SlotValue {
    SlotValue::Str(text.to_owned())
}

/// A contribution rooted at one stack with `t1` (`label` ← `title`) and `t2`
/// (`label` ← `other`), each with its own literal `text`.
fn bound(id: u64) -> Contribution {
    let mut t1 = text("t1", "alpha");
    t1.bound
        .set(PropKey::Label, PropVal::Bind(SlotKey::new("title")));
    let mut t2 = text("t2", "beta");
    t2.bound
        .set(PropKey::Label, PropVal::Bind(SlotKey::new("other")));
    tree(
        id,
        ViewNode::new(NodeKind::Stack, "surface")
            .child(t1)
            .child(t2),
    )
}

/// A contribution whose `t1` reads three things of one slot: a `label`
/// binding, a `value` binding, and a `fmt` derivation — the reverse index's
/// "every binding" case.
fn thrice(id: u64) -> Contribution {
    let mut t1 = text("t1", "alpha");
    t1.bound
        .set(PropKey::Label, PropVal::Bind(SlotKey::new("k")));
    t1.bound
        .set(PropKey::Value, PropVal::Bind(SlotKey::new("k")));
    let mut args = BTreeMap::new();
    args.insert("x".into(), PropVal::Bind(SlotKey::new("k")));
    let mut t2 = text("t2", "beta");
    t2.bound.set(
        PropKey::Label,
        PropVal::Derive(DeriveExpr::Fmt {
            format: "<{x}>".to_owned(),
            args,
        }),
    );
    tree(
        id,
        ViewNode::new(NodeKind::Stack, "surface")
            .child(t1)
            .child(t2),
    )
}

/// A contribution whose `t1` binds only `title` — the smallest tree a card
/// can stand in for.
fn one(id: u64) -> Contribution {
    let mut t1 = text("t1", "alpha");
    t1.bound
        .set(PropKey::Label, PropVal::Bind(SlotKey::new("title")));
    tree(id, ViewNode::new(NodeKind::Stack, "surface").child(t1))
}

fn tree(id: u64, tree: ViewNode) -> Contribution {
    Contribution {
        id: ContributionId::new(id),
        revision: 1,
        slot: SLOT.to_owned(),
        tree,
    }
}

fn host(app: Shell) -> (Context, Host<Shell>) {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    let mut host = Host::new(&ctx, app, default_presenter());
    pass(&ctx, &mut host);
    (ctx, host)
}

fn pass(ctx: &Context, host: &mut Host<Shell>) {
    let owned = ctx.clone();
    owned
        .run_ui(sized(RawInput::default()), |_| host.pass(&owned))
        .drop_without_applying_deltas();
}

fn sized(mut input: RawInput) -> RawInput {
    input.screen_rect = Some(egui::Rect::from_min_size(
        Pos2::ZERO,
        egui::vec2(WINDOW[0], WINDOW[1]),
    ));
    input
}

/// The canonical id of a node inside the mounted contribution `ui:<id>`.
fn mounted(id: &str) -> String {
    format!("/root/{SLOT}/{id}")
}

/// The `label` of the placement with canonical id `id`, or `None`.
fn label_of(frame: &PetrifiedFrame, id: &str) -> Option<String> {
    frame
        .placements
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.semantics.label.clone())
}

/// Whether this frame is the one `refusal_view` paints.
fn is_blanked(frame: &PetrifiedFrame) -> bool {
    frame
        .placements
        .iter()
        .any(|p| p.id.starts_with("/petra-tree-refused"))
}

// -------------------------------------------------------------------- G3

/// A bind resolves to its slot's value, and a change moves exactly the
/// bound properties — asserted before and after, on screen.
#[test]
fn a_slot_change_moves_exactly_the_bound_properties() {
    let (ctx, mut host) = host(Shell::default());
    // Snapshot first (design §8): the values before the tree that reads them.
    host.apply_slot_changes(&[
        SlotChange::new("title", 1, str_value("before")),
        SlotChange::new("other", 1, str_value("untouched")),
    ])
    .expect("the first versions land");
    host.set_contributions(vec![bound(7)]);
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert_eq!(
        label_of(frame, &mounted("ui:7/t1")),
        Some("before".to_owned()),
        "a bind must show its slot's value"
    );
    assert_eq!(
        label_of(frame, &mounted("ui:7/t2")),
        Some("untouched".to_owned())
    );

    let touched = host
        .apply_slot_changes(&[SlotChange::new("title", 2, str_value("after"))])
        .expect("the newer version lands");
    assert_eq!(
        touched,
        [("/ui:7/t1".to_owned(), PropKey::Label)],
        "exactly the property bound to `title` is re-folded"
    );
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert_eq!(
        label_of(frame, &mounted("ui:7/t1")),
        Some("after".to_owned()),
        "the bound property must move"
    );
    assert_eq!(
        label_of(frame, &mounted("ui:7/t2")),
        Some("untouched".to_owned()),
        "a property bound to another slot must not move"
    );
}

/// The reverse index returns every binding for a changed slot, through both
/// `slot_bindings` (what a change would touch) and `apply_slot_changes`
/// (what it did).
#[test]
fn the_reverse_index_names_every_binding_for_a_changed_slot() {
    let (ctx, mut host) = host(Shell::default());
    host.apply_slot_changes(&[SlotChange::new("k", 1, str_value("v"))])
        .expect("the first version lands");
    host.set_contributions(vec![thrice(7)]);
    pass(&ctx, &mut host);

    let expected = [
        ("/ui:7/t1".to_owned(), PropKey::Label),
        ("/ui:7/t1".to_owned(), PropKey::Value),
        ("/ui:7/t2".to_owned(), PropKey::Label),
    ];
    assert_eq!(
        host.slot_bindings("k"),
        expected,
        "the index must name every site that reads the slot, across nodes \
         and across `Bind` and `Derive` sources"
    );
    assert!(
        host.slot_bindings("nothing-binds-this").is_empty(),
        "a slot nothing reads has no sites"
    );

    let touched = host
        .apply_slot_changes(&[SlotChange::new("k", 2, str_value("w"))])
        .expect("the newer version lands");
    assert_eq!(touched, expected, "the change touches exactly the index");
}

/// A tree whose values have not arrived is a card naming the missing slot,
/// and that card gives way the moment the values arrive (design §8).
#[test]
fn a_card_gives_way_the_moment_the_values_arrive() {
    let (ctx, mut host) = host(Shell::default());
    host.set_contributions(vec![one(7)]);
    pass(&ctx, &mut host);

    let outcome = host.mounts()[0].outcome.clone();
    assert!(
        matches!(&outcome, MountOutcome::Refused(why) if why.contains("title") && why.contains("has no value")),
        "the card must name the slot that never arrived, got {outcome:?}"
    );

    let touched = host
        .apply_slot_changes(&[SlotChange::new("title", 1, str_value("Fibers"))])
        .expect("the first version lands");
    assert_eq!(
        touched,
        [("/ui:7/t1".to_owned(), PropKey::Label)],
        "the card gives way to a tree that folds every site"
    );
    pass(&ctx, &mut host);

    assert_eq!(
        host.mounts()[0].outcome,
        MountOutcome::Mounted,
        "the tree must mount once its value arrived"
    );
    let frame = host.frame().expect("a frame");
    assert_eq!(
        label_of(frame, &mounted("ui:7/t1")),
        Some("Fibers".to_owned())
    );
}

/// A version that is not newer refuses the whole batch by name, and nothing
/// on screen moves.
#[test]
fn a_stale_version_is_refused_whole_and_names_the_slot() {
    let (ctx, mut host) = host(Shell::default());
    host.apply_slot_changes(&[SlotChange::new("title", 5, str_value("v5"))])
        .expect("the first version lands");
    host.set_contributions(vec![one(7)]);
    pass(&ctx, &mut host);

    let err = host
        .apply_slot_changes(&[SlotChange::new("title", 5, str_value("again"))])
        .expect_err("a replayed version is stale");
    let message = err.to_string();
    assert!(
        message.contains("title") && message.contains("5"),
        "the refusal must name the slot and both versions, got {message}"
    );
    pass(&ctx, &mut host);
    let frame = host.frame().expect("a frame");
    assert_eq!(
        label_of(frame, &mounted("ui:7/t1")),
        Some("v5".to_owned()),
        "the refused batch changed nothing"
    );
}

/// The application's own tree binds through the same seam: a missing value
/// blanks the shell with its cause, and the value arriving brings it back.
#[test]
fn the_application_tree_binds_blindly_and_comes_back_with_its_value() {
    let (ctx, mut host) = host(Shell { binds: true });
    assert!(is_blanked(host.frame().expect("a frame")), "no value yet");

    host.apply_slot_changes(&[SlotChange::new("title", 1, str_value("Fibers"))])
        .expect("the first version lands");
    pass(&ctx, &mut host);

    let frame = host.frame().expect("a frame");
    assert!(
        !is_blanked(frame),
        "the shell must return once the value arrived"
    );
    assert_eq!(
        label_of(frame, "/root/shell.header"),
        Some("Fibers".to_owned()),
        "the application tree's bound property must fold through the host"
    );
}
