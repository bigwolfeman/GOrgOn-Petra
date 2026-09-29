//! Bound component parameters through the shipped `Host`
//! (`.agents/notes/implemented/architecture/2026-09-28-bound-component-parameters.md`).
//!
//! A contribution names a registry component and binds one of its openable
//! parameters to a slot. What is proved here is the host half: the value
//! arrives before the tree mounts or the card says which slot it waits on;
//! a commit re-expands exactly the component that reads it and leaves every
//! sibling's placements and hashes alone; a source on a parameter the
//! component does not open is a card naming it; a regrowth that refuses
//! refuses the batch whole.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use egui::{Context, Pos2, RawInput};
use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::input::{InputEvent, Route};
use gorgon_petra::layout::{ChangeSet, RowSource};
use gorgon_petra::semantic::ContributionId;
use gorgon_petra::tree::{
    ApplyError, ComponentRef, NodeKind, PropVal, SlotChange, SlotKey, SlotValue, ViewNode,
};
use gorgon_petra_egui::host::{App, Contribution, Host, MountOutcome, default_presenter};
use serde_json::json;

const WINDOW: [f32; 2] = [720.0, 360.0];
const SLOT: &str = "shell.status-bar";

struct Shell;

impl RowSource for Shell {
    fn rows(&mut self, _source: &str, _range: Range<usize>) -> Vec<Arc<ViewNode>> {
        Vec::new()
    }
}

impl App for Shell {
    fn view(&mut self) -> ViewNode {
        ViewNode::new(NodeKind::Stack, "root").child(ViewNode::new(NodeKind::Stack, SLOT))
    }

    fn handle(&mut self, _event: &InputEvent, _route: &Route, _frame: Option<&PetrifiedFrame>) {}

    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::None
    }
}

fn host() -> (Context, Host<Shell>) {
    let ctx = Context::default();
    ctx.run_ui(sized(RawInput::default()), |_| {})
        .drop_without_applying_deltas();
    let mut host = Host::new(&ctx, Shell, default_presenter());
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

fn toggle(key: &str, bound: &[(&str, &str)], literal: serde_json::Value) -> ViewNode {
    let mut params = json!({ "key": key, "label": format!("Toggle {key}") });
    if let (Some(into), Some(extra)) = (params.as_object_mut(), literal.as_object()) {
        into.extend(extra.clone());
    }
    let mut component = ComponentRef::literal("toggle", params);
    for (param, slot) in bound {
        component
            .bound
            .insert((*param).to_owned(), PropVal::Bind(SlotKey::new(*slot)));
    }
    let mut node = ViewNode::new(NodeKind::Component, key);
    node.component = Some(component);
    node
}

fn contribution(id: u64, tree: ViewNode) -> Contribution {
    Contribution {
        id: ContributionId::new(id),
        revision: 1,
        slot: SLOT.to_owned(),
        tree,
    }
}

fn two_toggles(id: u64) -> Contribution {
    contribution(
        id,
        ViewNode::new(NodeKind::Stack, "surface")
            .child(toggle("a", &[("selected", "on-a")], json!({})))
            .child(toggle("b", &[("selected", "on-b")], json!({}))),
    )
}

fn mounted(tail: &str) -> String {
    format!("/root/{SLOT}/{tail}")
}

fn selected(frame: &PetrifiedFrame, id: &str) -> bool {
    frame
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("{id} is placed"))
        .semantics
        .selected
}

fn hashes_under(frame: &PetrifiedFrame, root: &str) -> BTreeMap<String, [u8; 32]> {
    let under = format!("{root}/");
    frame
        .placements
        .iter()
        .zip(&frame.subtree_hashes)
        .filter(|(p, _)| p.id == root || p.id.starts_with(&under))
        .map(|(p, h)| (p.id.clone(), *h))
        .collect()
}

#[test]
fn a_commit_re_expands_only_the_component_that_reads_it() {
    let (ctx, mut host) = host();
    host.apply_slot_changes(&[
        SlotChange::new("on-a", 1, SlotValue::Bool(false)),
        SlotChange::new("on-b", 1, SlotValue::Bool(false)),
    ])
    .expect("the first versions land");
    host.set_contributions(vec![two_toggles(7)]);
    pass(&ctx, &mut host);
    assert_eq!(host.mounts()[0].outcome, MountOutcome::Mounted);
    assert_eq!(host.slot_components("on-a"), ["/ui:7/a"]);

    let before = host.frame().expect("a frame").clone();
    let (a, b) = (mounted("ui:7/a"), mounted("ui:7/b"));
    assert!(!selected(&before, &a) && !selected(&before, &b));

    let touched = host
        .apply_slot_changes(&[SlotChange::new("on-a", 2, SlotValue::Bool(true))])
        .expect("the newer version lands");
    assert!(touched.is_empty(), "no node-level property binds `on-a`");
    assert_eq!(
        host.regrown_components(),
        ["/ui:7/a"],
        "exactly the toggle that reads `on-a` re-expands"
    );
    pass(&ctx, &mut host);

    let after = host.frame().expect("a frame");
    assert!(selected(after, &a), "the regrown toggle is on");
    assert!(!selected(after, &b), "its sibling is not");
    assert_ne!(hashes_under(&before, &a), hashes_under(after, &a));
    let sibling = hashes_under(&before, &b);
    assert!(!sibling.is_empty());
    assert_eq!(
        sibling,
        hashes_under(after, &b),
        "every placement of the sibling toggle keeps its hash"
    );
}

#[test]
fn a_card_waiting_on_a_bound_parameter_gives_way_when_its_value_arrives() {
    let (ctx, mut host) = host();
    host.set_contributions(vec![contribution(
        7,
        toggle("a", &[("selected", "on-a")], json!({})),
    )]);
    pass(&ctx, &mut host);
    let outcome = host.mounts()[0].outcome.clone();
    assert!(
        matches!(&outcome, MountOutcome::Refused(why)
            if why.contains("on-a") && why.contains("params.selected")),
        "the card must name the slot and the parameter, got {outcome:?}"
    );

    host.apply_slot_changes(&[SlotChange::new("on-a", 1, SlotValue::Bool(true))])
        .expect("the value lands");
    pass(&ctx, &mut host);
    assert_eq!(host.mounts()[0].outcome, MountOutcome::Mounted);
    assert!(selected(host.frame().expect("a frame"), &mounted("ui:7")));
}

#[test]
fn a_source_on_a_parameter_the_component_does_not_open_is_a_card_naming_it() {
    let (ctx, mut host) = host();
    host.apply_slot_changes(&[SlotChange::new("title", 1, SlotValue::Str("x".into()))])
        .expect("the value lands");
    let mut component = ComponentRef::literal("toggle", json!({ "key": "a", "selected": false }));
    component
        .bound
        .insert("label".into(), PropVal::Bind(SlotKey::new("title")));
    let mut node = ViewNode::new(NodeKind::Component, "a");
    node.component = Some(component);
    host.set_contributions(vec![contribution(7, node)]);
    pass(&ctx, &mut host);
    let outcome = host.mounts()[0].outcome.clone();
    assert!(
        matches!(&outcome, MountOutcome::Refused(why)
            if why.contains("parameter `label` is not openable")
                && why.contains("a component expands from literal parameters")),
        "got {outcome:?}"
    );
}

#[test]
fn a_regrowth_that_refuses_refuses_the_batch_whole() {
    let (ctx, mut host) = host();
    host.apply_slot_changes(&[
        SlotChange::new("on-a", 1, SlotValue::Bool(false)),
        SlotChange::new("on-b", 1, SlotValue::Bool(false)),
    ])
    .expect("the first versions land");
    host.set_contributions(vec![two_toggles(7)]);
    pass(&ctx, &mut host);

    let err = host
        .apply_slot_changes(&[
            SlotChange::new("on-b", 2, SlotValue::Bool(true)),
            SlotChange::new("on-a", 2, SlotValue::Str("yes".into())),
        ])
        .expect_err("a string cannot fill a boolean parameter");
    assert!(
        matches!(&err, ApplyError::Regrow { component, reason }
            if component == "/ui:7/a" && reason.contains("params.selected")),
        "the refusal names the component and the parameter, got {err}"
    );
    pass(&ctx, &mut host);
    let frame = host.frame().expect("a frame");
    assert!(
        !selected(frame, &mounted("ui:7/b")),
        "the refused batch moved nothing, not even the half that would have landed"
    );
    host.apply_slot_changes(&[SlotChange::new("on-b", 2, SlotValue::Bool(true))])
        .expect("the refused versions were never stored");
}
