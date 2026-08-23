//! AccessKit node emission from the semantic tree (T028, FR-027).
//!
//! Petra never constructs an egui `Ui` (`lib.rs`), so egui's own
//! `Ui`-driven AccessKit generation (`Response::widget_info` and friends)
//! never fires for Petra content: there is nothing here for it to walk.
//! [`emit`] is the manual node registration that R3 calls for instead — one
//! reverse-free pass over [`gorgon_petra::semantic::SemanticTree`], the same
//! tree that also answers the agent's UI-state query and the driver's
//! finders (FR-027, `contracts/semantic-tree.md`). No node here is read from
//! `PetrifiedFrame` placements directly; every field comes off the
//! [`SemanticNode`] the tree already projected.
//!
//! # Wiring this in: a `Plugin`, not a return value
//!
//! [`emit`] alone is not the hook. `egui::Context` runs its **own**
//! AccessKit generation every pass once anything has enabled it — and
//! something always does, eventually: `eframe`'s winit integration calls
//! `egui_ctx().enable_accesskit()` itself, from inside its handler for
//! `accesskit_winit::WindowEvent::InitialTreeRequested`
//! (`eframe-0.36.1/src/native/winit_integration.rs`, the
//! `on_accesskit_window_event` match arm) — the moment a real screen reader
//! attaches, with no call from this crate. From then on, every
//! `Context::begin_pass` seeds one root `Node` (`Role::Window`) into its own
//! `accesskit_state` (`egui-0.36.1/src/context.rs:517-529`), and every
//! `Context::end_pass` writes that state into
//! `FullOutput::platform_output.accesskit_update`
//! (`egui-0.36.1/src/context.rs:2690-2712`) — **unconditionally**, whether
//! or not Petra ever calls [`emit`]. A function that only *returns* a
//! `TreeUpdate` for someone else to assign after the pass is too late: by
//! the time `Host::pass` could see that return value, `end_pass` has
//! already run and already written egui's own near-empty tree over the
//! field. Assigning after that point from inside `Host::pass` doesn't work
//! either, because `Host::pass` itself never calls `end_pass` — the eframe
//! native/web backend does, outside any code this crate owns, so there is
//! no line in `host.rs` that runs *after* `end_pass` and *before* the
//! `FullOutput` leaves egui. Petra's own write would work fine with no
//! screen reader attached (egui leaves the field alone when
//! `enable_accesskit` was never called) and silently go missing the moment
//! one does — the exact worst failure mode: a window that looks wired and
//! is deaf.
//!
//! The seam that actually runs *after* egui's own write, from inside the
//! same `end_pass` call, is [`egui::Plugin::output_hook`]:
//! `Context::end_pass` computes its own output first, then runs
//! `plugins.on_output(self, &mut output)` on the result
//! (`egui-0.36.1/src/context.rs:2456-2462`), and `on_output` calls every
//! registered plugin's `output_hook(&mut self, ctx, &mut FullOutput)`
//! (`plugin.rs:44`, dispatched from `plugin.rs:172-178`). [`Publisher`] is
//! that plugin: [`publish`] moves this pass's [`TreeUpdate`] into it any
//! time during the pass, and its `output_hook` overwrites whatever egui
//! just wrote with Petra's own tree. `Plugin::on_end_pass` — and the
//! `Context::on_end_pass` convenience wrapper built on it — cannot do this
//! job; both only ever hand out `&mut Ui` (`plugin.rs:32`; `plugin.rs:224`'s
//! `ContextCallback = Arc<dyn Fn(&mut Ui) + Send + Sync>`), never
//! `FullOutput`, so there is no path from that seam to the output at all,
//! clobbering or otherwise. This was checked by construction, not assumed:
//! see the sabotage in `tests::publish_survives_the_real_end_of_pass_write`,
//! which reproduces the clobbering bug by neutering `output_hook` and shows
//! the test failing for exactly that reason.
//!
//! `Host::pass` needs exactly one call, anywhere after the frame's
//! [`SemanticTree`] is known and before the pass ends:
//!
//! ```ignore
//! if let Some(tree) = gorgon_petra::semantic::project(&frame) {
//!     crate::accesskit::publish(ctx, &tree);
//! }
//! ```
//!
//! No separate registration step: [`publish`] adds [`Publisher`] to the
//! `Context` itself, the first time it's called, via
//! [`egui::Context::plugin_or_default`] (idempotent per plugin type —
//! `plugin.rs`'s `Plugins::add` no-ops on a repeat of the same type), so
//! there is nothing else for `Host::new` to set up.
//!
//! An incoming `AccessKitActionRequest` is still not routed: this module
//! only emits nodes, it does not consume `ActionRequest`s. That remains
//! [`crate::input::EventTranslator::untranslated`]'s job until T030.
//!
//! # The R3 finding
//!
//! R3's stated rationale — "kittest finders read the AccessKit tree egui's
//! widgets emit — they structurally cannot see custom-painted content" — is
//! imprecise once you read `kittest`'s and `egui_kittest`'s source (checked
//! against `kittest-0.4.0` and `egui_kittest-0.36.1`, fetched into the local
//! registry cache for this task). Two different things were being
//! conflated:
//!
//! - `kittest`'s finder/query layer (`kittest::filter::By`,
//!   `kittest::node::NodeT`) is **not** structurally blind to anything. It
//!   queries an `accesskit_consumer::Tree`, built from whatever
//!   `accesskit::TreeUpdate` was handed to `kittest::State::new`/`update`.
//!   Nothing in `filter.rs`, `query.rs`, or `node.rs` reads egui internals or
//!   distinguishes a `Ui`-sourced node from a manually-registered one — a
//!   `TreeUpdate` assembled entirely by hand, as [`emit`] does, is exactly
//!   as queryable as one egui's widget stack produced.
//! - What actually cannot see custom-painted content is egui's own
//!   *automatic* generation: `Context::end_pass` only builds an
//!   `accesskit::TreeUpdate` from nodes registered via
//!   `accesskit_node_builder`, which `Ui`/`Response` calls do on a widget's
//!   behalf. Petra never makes those calls, so without [`emit`] the
//!   automatically-produced tree is near-empty (one `Role::Window` root and
//!   nothing else — see "Wiring this in" above) — not selectively blind,
//!   just empty of Petra content.
//! - A consequence worth naming for T029/T032: `egui_kittest::Harness`, as
//!   commonly built with `Harness::new_ui`, drives its *own* `egui::Context`
//!   through a bare `Ui` closure — it never runs Petra's host loop, so it
//!   would never see [`emit`]'s output either, regardless of the finder
//!   layer's generality. R3's actual decision — mirror kittest's snapshot
//!   machinery, build finders over Petra's own manually-registered tree —
//!   is still the right call; the tree just is not blind to hand-built
//!   nodes for the reason the research note gives, and the two facts happen
//!   to point at the same architecture.

use egui::accesskit::{
    Node as AkNode, NodeId as AkNodeId, Rect as AkRect, Role as AkRole, Tree as AkTree,
    TreeId as AkTreeId, TreeUpdate,
};
use egui::{Context, FullOutput, Plugin};

use gorgon_petra::semantic::{SemanticNode, SemanticTree};
use gorgon_petra::tree::Role;

/// A semantic node id's AccessKit identity.
///
/// Reuses `egui::Id`'s hash rather than inventing a second one: the
/// semantic id is already the stable key-path string
/// (`contracts/semantic-tree.md`'s stability rules), so hashing it the same
/// way egui hashes widget ids gives a `NodeId` that is stable for exactly as
/// long as the semantic id is, and never collides with an id egui's own
/// (unused, here) `Ui`-driven path would have produced.
fn node_id(id: &str) -> AkNodeId {
    egui::Id::new(id).accesskit_id()
}

/// Map one [`Role`] onto its AccessKit equivalent.
///
/// Closed match, no catch-all: `contracts/semantic-tree.md`'s role
/// vocabulary is closed and additive-only, and a wildcard arm here would be
/// exactly the silently-swallowed-role defect the wave contract warns
/// against — the compiler must refuse to build the day a new [`Role`]
/// variant lands without a line here to say what it becomes.
///
/// Returns the AccessKit role plus an optional exposed name for
/// `set_role_description`: `None` where AccessKit has a real 1:1 role,
/// `Some(name)` for `custom:<name>` (the contract's own words: "map to
/// AccessKit's generic container with the name exposed") and for the two
/// built-in roles — `overlay`, `separator` — that have no AccessKit role at
/// all and take the same generic-container fallback the contract prescribes
/// for `custom:`.
fn accesskit_role(role: &Role) -> (AkRole, Option<String>) {
    match role {
        Role::Pane => (AkRole::Pane, None),
        Role::List => (AkRole::List, None),
        Role::ListItem => (AkRole::ListItem, None),
        Role::Button => (AkRole::Button, None),
        Role::TextInput => (AkRole::TextInput, None),
        Role::Label => (AkRole::Label, None),
        Role::Status => (AkRole::Status, None),
        Role::Tab => (AkRole::Tab, None),
        Role::TabList => (AkRole::TabList, None),
        Role::Scroll => (AkRole::ScrollView, None),
        Role::Overlay => (AkRole::GenericContainer, Some("overlay".to_owned())),
        Role::Toast => (AkRole::Alert, None),
        Role::Dialog => (AkRole::Dialog, None),
        Role::Separator => (AkRole::GenericContainer, Some("separator".to_owned())),
        Role::Image => (AkRole::Image, None),
        Role::Progress => (AkRole::ProgressIndicator, None),
        Role::Tree => (AkRole::Tree, None),
        Role::TreeItem => (AkRole::TreeItem, None),
        Role::Table => (AkRole::Table, None),
        Role::Row => (AkRole::Row, None),
        Role::Cell => (AkRole::Cell, None),
        Role::Custom(name) => (AkRole::GenericContainer, Some(name.clone())),
    }
}

/// Build one AccessKit node from `node`, given its children's already-known
/// ids.
///
/// An absent [`SemanticNode::role`] (a spacer, or an undeclared custom node
/// — `contracts/semantic-tree.md`'s "Absent roles") becomes
/// `Role::GenericContainer` with no exposed name: AccessKit's own docs for
/// that role are "should be ignored by assistive technologies and filtered
/// out of platform accessibility trees", which is exactly "no semantic
/// role" — and, unlike the `Hidden` flag, `GenericContainer` does not
/// recurse and hide this node's children too, so a role-less wrapper around
/// real content never takes its descendants out of the tree with it.
fn build_node(node: &SemanticNode, children: Vec<AkNodeId>) -> (AkNodeId, AkNode) {
    let (role, exposed_name) = node
        .role
        .as_ref()
        .map_or((AkRole::GenericContainer, None), accesskit_role);

    let mut ak = AkNode::new(role);
    if let Some(name) = exposed_name {
        ak.set_role_description(name);
    }
    if !node.label.is_empty() {
        ak.set_label(node.label.clone());
    }
    if let Some(value) = &node.value {
        ak.set_value(value.clone());
    }
    if node.state.disabled {
        ak.set_disabled();
    }
    if node.state.selected {
        ak.set_selected(true);
    }
    if let Some(expanded) = node.state.expanded {
        ak.set_expanded(expanded);
    }
    if !children.is_empty() {
        ak.set_children(children);
    }
    let bounds = &node.bounds;
    ak.set_bounds(AkRect {
        x0: f64::from(bounds.x),
        y0: f64::from(bounds.y),
        x1: f64::from(bounds.x + bounds.w),
        y1: f64::from(bounds.y + bounds.h),
    });

    (node_id(&node.id), ak)
}

/// Project `tree` into the AccessKit tree update for this frame.
///
/// One pass over [`SemanticTree::iter`] (the tree's own pre-order walk, the
/// same one `contracts/semantic-tree.md` fixes as focus/reading order): for
/// every node, its already-computed children give the `Children` ids, and
/// [`build_node`] does the field mapping. `focus` follows whichever node (at
/// most one, per the contract) has `state.focused` set; with none, it
/// follows egui's own default and names the root, rather than an id nothing
/// points at.
///
/// Pure projection, no `egui::Context` — see [`publish`] for how the result
/// actually reaches the platform.
#[must_use]
pub fn emit(tree: &SemanticTree) -> TreeUpdate {
    let root_id = node_id(&tree.root().id);
    let mut nodes = Vec::with_capacity(tree.len());
    let mut focus = root_id;

    for node in tree.iter() {
        if node.state.focused {
            focus = node_id(&node.id);
        }
        let children = node.children.iter().map(|c| node_id(&c.id)).collect();
        nodes.push(build_node(node, children));
    }

    TreeUpdate {
        nodes,
        tree: Some(AkTree::new(root_id)),
        tree_id: AkTreeId::ROOT,
        focus,
    }
}

/// The plugin that carries Petra's tree past egui's own end-of-pass write.
///
/// See the module doc's "Wiring this in" section for why `output_hook` is
/// the only seam that can still reach `FullOutput` after `Context::end_pass`
/// has already written egui's own (near-empty) tree onto it. Not
/// constructed directly — [`publish`] adds and reaches it through
/// [`Context::plugin_or_default`].
#[derive(Default)]
struct Publisher {
    /// This pass's tree, moved in by [`publish`] and moved back out by
    /// `output_hook` (`Option::take`, never cloned).
    pending: Option<TreeUpdate>,
}

impl Plugin for Publisher {
    fn debug_name(&self) -> &'static str {
        "gorgon-petra-egui::accesskit::Publisher"
    }

    fn output_hook(&mut self, _ctx: &Context, output: &mut FullOutput) {
        if let Some(update) = self.pending.take() {
            // Overwrite, not merge: by the time this runs, egui has already
            // written its own tree for this pass into the same field (the
            // near-empty one described above) — the same "complete tree
            // every pass, overwrite rather than append" convention egui's
            // own `PlatformOutput::append` documents for the Ui-driven case
            // applies here too, just one step later in the pipeline.
            output.platform_output.accesskit_update = Some(update);
        }
    }
}

/// Publish `tree`'s AccessKit projection so it lands on this pass's
/// `FullOutput`, replacing whatever tree egui's own generation wrote for
/// the same pass.
///
/// Call once per pass, any time after the frame's [`SemanticTree`] is known
/// and before the pass ends — delivery happens later, inside
/// `Context::end_pass` itself (see the module doc), so it does not matter
/// how much of `Host::pass` runs after this call.
///
/// # Cost
///
/// One [`emit`] walk — unavoidable, it is the actual projection, and its
/// cost is the same order as the paint pass already walking this frame's
/// placements — plus two pointer-sized moves (into [`Publisher::pending`]
/// here, back out via `Option::take` in `output_hook`); nothing is cloned.
/// This runs every pass, including passes with no screen reader attached:
/// unlike egui's own generation, which `accesskit_node_builder` turns into
/// a cheap no-op per widget while `Context::is_accesskit_enabled` is false,
/// nothing here observes that flag — there is no public getter for it, only
/// `enable_accesskit`/`disable_accesskit`. The actual OS-level forwarding
/// stays free when nothing is attached regardless
/// (`accesskit_winit::Adapter::update_if_active` only forwards "if and only
/// if the tree has been initialized" — `accesskit_winit-0.32.2/src/lib.rs`),
/// so the cost that is *not* skipped is exactly the `emit` walk, not
/// anything past it. A cheap activity probe exists
/// (`Context::accesskit_node_builder` returns `None` while disabled, so a
/// throwaway call could detect the flag without a public getter) but was
/// left unbuilt: it leans on unexported implementation behavior rather than
/// a documented contract, for a cost in the same order as work this crate
/// already pays every pass regardless.
pub fn publish(ctx: &Context, tree: &SemanticTree) {
    let update = emit(tree);
    ctx.plugin_or_default::<Publisher>().lock().pending = Some(update);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gorgon_petra::semantic::{Bounds, NodeState};
    use gorgon_petra::tree::Interaction;

    fn leaf(id: &str, role: Option<Role>, label: &str) -> SemanticNode {
        SemanticNode {
            id: id.to_owned(),
            role,
            label: label.to_owned(),
            value: None,
            state: NodeState::default(),
            bounds: Bounds {
                x: 1,
                y: 2,
                w: 30,
                h: 40,
            },
            frame_seq: 7,
            actions: Vec::new(),
            total_count: None,
            children: Vec::new(),
        }
    }

    fn parent(
        id: &str,
        role: Option<Role>,
        label: &str,
        children: Vec<SemanticNode>,
    ) -> SemanticNode {
        SemanticNode {
            children,
            ..leaf(id, role, label)
        }
    }

    /// A button node carries the button role and its label.
    #[test]
    fn button_role_and_label() {
        let mut button = leaf("root/save", Some(Role::Button), "Save");
        button.actions = vec![Interaction::Click, Interaction::Focus];
        let tree = SemanticTree::new(button);

        let update = emit(&tree);
        assert_eq!(update.nodes.len(), 1);
        let (_, node) = &update.nodes[0];
        assert_eq!(node.role(), AkRole::Button);
        assert_eq!(node.label(), Some("Save"));
    }

    /// A role with no AccessKit equivalent lands on the generic container
    /// with its name exposed — both the built-in case (`separator`) and the
    /// host-declared case (`custom:gutter`).
    #[test]
    fn roles_without_an_accesskit_equivalent_expose_their_name() {
        let separator = leaf("root/rule", Some(Role::Separator), "");
        let (_, sep_node) = build_node(&separator, Vec::new());
        assert_eq!(sep_node.role(), AkRole::GenericContainer);
        assert_eq!(sep_node.role_description(), Some("separator"));

        let custom = leaf("root/gutter", Some(Role::Custom("gutter".to_owned())), "");
        let (_, custom_node) = build_node(&custom, Vec::new());
        assert_eq!(custom_node.role(), AkRole::GenericContainer);
        assert_eq!(custom_node.role_description(), Some("gutter"));
    }

    /// An undeclared role (a spacer) is a generic container with no exposed
    /// name — present in the tree, transparent to assistive tech, and not
    /// hiding its own children the way the `Hidden` flag would.
    #[test]
    fn absent_role_is_a_bare_generic_container() {
        let spacer = leaf("root/spacer", None, "");
        let (_, node) = build_node(&spacer, Vec::new());
        assert_eq!(node.role(), AkRole::GenericContainer);
        assert_eq!(node.role_description(), None);
    }

    /// The node tree's structure matches the semantic tree's child order.
    #[test]
    fn structure_matches_semantic_child_order() {
        let root = parent(
            "root",
            Some(Role::Pane),
            "",
            vec![
                leaf("root/a", Some(Role::Label), "A"),
                leaf("root/b", Some(Role::Label), "B"),
            ],
        );
        let tree = SemanticTree::new(root);
        let update = emit(&tree);

        assert_eq!(update.nodes.len(), 3);
        let root_id = node_id("root");
        let (_, root_ak) = update
            .nodes
            .iter()
            .find(|(id, _)| *id == root_id)
            .expect("root node present");
        let expected_children = vec![node_id("root/a"), node_id("root/b")];
        assert_eq!(root_ak.children(), expected_children.as_slice());
    }

    /// A node the semantic tree marks disabled or focused carries that
    /// through.
    #[test]
    fn disabled_and_focused_state_carries_through() {
        let mut disabled = leaf("root/off", Some(Role::Button), "Off");
        disabled.state.disabled = true;

        let mut focused = leaf("root/on", Some(Role::TextInput), "On");
        focused.state.focused = true;

        let root = parent("root", Some(Role::Pane), "", vec![disabled, focused]);
        let tree = SemanticTree::new(root);
        let update = emit(&tree);

        let (_, off_ak) = update
            .nodes
            .iter()
            .find(|(id, _)| *id == node_id("root/off"))
            .expect("disabled node present");
        assert!(off_ak.is_disabled());

        assert_eq!(update.focus, node_id("root/on"));
    }

    /// With no node focused, `focus` names the root — the same default
    /// egui's own emission uses — rather than an id nothing points at.
    #[test]
    fn focus_defaults_to_root_when_nothing_is_focused() {
        let root = leaf("root", Some(Role::Pane), "");
        let tree = SemanticTree::new(root);
        let update = emit(&tree);
        assert_eq!(update.focus, node_id("root"));
    }

    /// The defect this exercise exists to catch: publishing through the
    /// real `Context` pass cycle, with AccessKit enabled, must leave
    /// *Petra's* node on the `FullOutput` — not merely `Some(_)`, since
    /// `Context::begin_pass` unconditionally seeds one `Role::Window` root
    /// node into its own accesskit state whenever accesskit is enabled,
    /// `Ui` or no `Ui` (`egui-0.36.1/src/context.rs:517-529`). A test that
    /// only checked `is_some()` would pass even while silently reporting
    /// egui's placeholder tree instead of Petra's — which is exactly the
    /// clobbering bug a `Plugin::output_hook` (this test) catches and a
    /// bare return-value hook (rejected in the module doc) would not.
    #[test]
    fn publish_survives_the_real_end_of_pass_write() {
        let ctx = Context::default();
        ctx.enable_accesskit();

        let mut button = leaf("root/save", Some(Role::Button), "Save");
        button.state.focused = true;
        let tree = SemanticTree::new(button);

        ctx.begin_pass(egui::RawInput::default());
        publish(&ctx, &tree);
        let mut output = ctx.end_pass();
        // The real pass allocates a font-atlas texture delta; the test only
        // cares about `platform_output`, but epaint asserts on drop that a
        // `TexturesDelta` was handled, so it is cleared explicitly rather
        // than silently ignored.
        output.textures_delta.clear();

        let update = output
            .platform_output
            .accesskit_update
            .expect("accesskit is enabled, so end_pass always writes something");

        let root_id = node_id("root/save");
        let (_, node) = update.nodes.iter().find(|(id, _)| *id == root_id).expect(
            "Petra's node must be present on the real FullOutput, not clobbered by \
             egui's own end-of-pass write",
        );
        assert_eq!(node.role(), AkRole::Button);
        assert_eq!(node.label(), Some("Save"));
        assert_eq!(update.focus, root_id);
    }
}
