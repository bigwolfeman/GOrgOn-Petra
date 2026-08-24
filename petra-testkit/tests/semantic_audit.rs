//! T058: the SC-010/SC-012 semantic audit gate, run over every inspector
//! screen — the seven panels `gorgon_inspector::app::Shell::mounted` names,
//! including the three that only exist behind a tab.
//!
//! [`gorgon_petra::semantic::audit`] is the rules engine
//! (`contracts/semantic-tree.md`'s four "machine-checkable" obligations
//! plus four projection-integrity checks). This file does not reimplement
//! any of it: every screen this test visits is fed through the same
//! [`audit`] function `contracts/semantic-tree.md` and spec 004's gates
//! call, never a hand-rolled copy of its rules.
//!
//! # Reaching every screen "via the driver"
//!
//! Navigation is exactly `tests/inspector_journey.rs`'s shape: a real
//! `gorgond` (`support::gorgond::Daemon::boot_demo`), a real inspector
//! `Shell::mounted` composition hosted under a real `DriverHost`, driven
//! keyboard-only through the real, importable
//! [`gorgon_petra_testkit::driver::Client`] — Tab to a tab's control, Enter
//! to select it, the same `support::journey::tab_until`/`press_enter`
//! primitives that file uses. Three panels (identity, the fiber list, the
//! fiber detail) are always on screen; the other four
//! (Approvals/Leaks/Unload preview/Trace) are `Slot::Tab` mounts and are
//! reached only by actually switching to them — `gorgon_inspector::app`'s
//! own `a_tab_is_selected_by_keyboard_alone` test is what proves that
//! keyboard-only switch is possible at all, and this file does it against
//! a live daemon instead of a stub.
//!
//! # Why the audit input is *not* built from `tree`/`frame` wire replies
//!
//! [`gorgon_petra::semantic::audit`] takes `&SemanticTree` and
//! `&PetrifiedFrame` — the real engine types. The driver protocol's `tree`
//! and `frame` verbs deliberately answer with narrower wire mirrors
//! (`gorgon_petra_testkit::driver::DriverNode`,
//! `gorgon_petra_testkit::wire::FrameResult`/`WirePlacement`): reconstructing
//! a real `PetrifiedFrame` from `FrameResult` is not possible from the wire
//! alone, because `WirePlacement` carries none of
//! `gorgon_petra::frame::PlacementSemantics`'s `actions`/`disabled` fields —
//! and `audit`'s `FocusOrderIsChildOrder` rule needs exactly those to build
//! a real `gorgon_petra::focus::FocusTree` (`crate::focus::FocusTree::from_placements`
//! reads `PlacementSemantics::actions`/`.disabled` directly). Reconstructing
//! a `PetrifiedFrame` with invented values for the fields the wire does not
//! carry would not be auditing this frame; it would be auditing a frame this
//! test made up.
//!
//! The real `(SemanticTree, PetrifiedFrame)` pair for the frame the driver
//! itself is answering from is available with no invention at all: it is
//! exactly what `gorgon_petra_testkit::server::hub::FrameHub::current`
//! publishes and what `server::dispatch::tree`/`frame` themselves read
//! (`dispatch.rs`: `"tree"`/`"frame"` "answer from what the application
//! last published — `FrameHub`'s snapshot"). So navigation goes through the
//! driver client exactly as `inspector_journey.rs` does, and each screen's
//! audit input is read from the same `FrameHub` the driver's own verbs
//! read from — the two are provably the same frame, not two different
//! views of it (the `wire_frame.seq`/`last.frame.seq` assertion in the test
//! below checks this directly).
//!
//! That hub arrives through
//! [`support::inspector::DrivenInspector::hub`]. This file used to stand up
//! its own copy of that harness (`mount_audited_inspector`) purely because
//! the accessor did not exist and the leaf that wrote this file could not
//! add it — file ownership was frozen during a subagent fan-out. The
//! 2026-08-24 shakedown named the copy (F16) and it is gone: the audit now
//! mounts the window through the same `support::inspector` every other
//! inspector test uses, so the gate and the journey cannot drift into
//! auditing two different compositions.
//!
//! # Two things this file was told to expect, and confirms rather than
//! assumes
//!
//! 1. `semantic::project` only carries a `label` for interactive nodes and
//!    `component::status` readouts — a plain `text()`/`heading()` node's
//!    prose never reaches the tree. Confirmed below by
//!    `plain_text_nodes_carry_no_label_but_status_and_actionable_nodes_do`.
//! 2. There is a known, open, `#[ignore]`d defect
//!    (`gorgon/inspector/tests/layout_overlap.rs::report_text_runs_that_do_not_fit_the_box_they_were_given`,
//!    written up in
//!    `.agents/notes/proposed/bug-fix/2026-08-24-inspector-text-paints-outside-its-rect.md`)
//!    where `Placement::clip` is inherited rather than derived, which
//!    `PaintState::truncated` cannot distinguish from a genuine vertical
//!    overflow. If the live run below surfaces a real `TruncationIsReal`
//!    violation, it is printed and named, never suppressed or allowlisted
//!    — see `REAL VIOLATIONS` in the test output.
//!
//! # Sabotage
//!
//! This leaf owns no panel or component source file, so per the task's own
//! fallback ("If the sabotage target is a file you do not own ... use an
//! in-test fixture") the sabotage below mutates an **in-memory clone** of a
//! real captured `(SemanticTree, PetrifiedFrame)` pair — never a file on
//! disk — one obligation at a time, and asserts `audit` goes red naming the
//! mutated node while the untouched original stays clean. Nothing on disk
//! is written, so there is nothing to restore or `diff`.

#![cfg(feature = "testkit")]
// `mod support;` pulls in the whole shared harness (`support::mod`'s own
// module doc); this file uses `gorgond`, `inspector` and `journey` but not
// `measure` (T046/T064's timing twins, irrelevant to an audit gate).
#![allow(dead_code)]

mod support;

use std::time::Duration;

use gorgon_petra::frame::PetrifiedFrame;
use gorgon_petra::semantic::{AuditRule, AuditViolation, SemanticNode, SemanticTree, audit};
use gorgon_petra::tree::Role;
use gorgon_petra_testkit::server::FrameHub;

use support::gorgond::Daemon;
use support::inspector::inspector_with_client;
use support::journey::{press_enter, tab_until, wait_tree};

/// Generous headroom over the focusable nodes this window ever mounts
/// against the shipped demo — `inspector_journey.rs`'s own constant,
/// carried over unchanged (same window, same demo composition).
const MAX_TAB_PRESSES: usize = 80;
/// Budget for a wait on a real round trip to the daemon (hazard: nothing
/// may hang).
const DATA_WAIT: Duration = Duration::from_secs(15);
/// The four tabbed screens, in the order `Shell::mounted` composes them.
const TABS: [&str; 4] = ["Approvals", "Leaks", "Unload preview", "Trace"];

/// The real, currently-published `(SemanticTree, PetrifiedFrame)` pair —
/// the same objects `server::dispatch::tree`/`frame` answer the driver's
/// own verbs from, read via `FrameHub::current` rather than reassembled
/// from a wire reply (module doc's "Why the audit input is not built from
/// `tree`/`frame` wire replies").
fn snapshot(hub: &FrameHub) -> (SemanticTree, PetrifiedFrame) {
    let published = hub
        .current()
        .expect("a frame has been published by the time a screen is captured");
    let tree = published
        .tree
        .clone()
        .expect("a frame with placements always projects a tree (project() returns None only for zero placements, which petrify never produces)");
    (tree, published.frame.clone())
}

/// Recursively blank one node's label — an in-memory sabotage of a cloned
/// tree, never a file on disk (module doc's "Sabotage" section).
fn blank_label(node: &mut SemanticNode, id: &str) -> bool {
    if node.id == id {
        node.label.clear();
        return true;
    }
    node.children.iter_mut().any(|child| blank_label(child, id))
}

/// Recursively flip one node's `state.truncated` flag to `true`.
fn set_truncated(node: &mut SemanticNode, id: &str) -> bool {
    if node.id == id {
        node.state.truncated = true;
        return true;
    }
    node.children
        .iter_mut()
        .any(|child| set_truncated(child, id))
}

/// The first node, in pre-order, with at least two directly-focusable
/// children — the target `swap_two_focusable_children` needs.
fn find_parent_with_two_focusable_children(node: &SemanticNode) -> Option<&SemanticNode> {
    let focusable_children = node.children.iter().filter(|c| c.is_focusable()).count();
    if focusable_children >= 2 {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(find_parent_with_two_focusable_children)
}

/// Swap the first two focusable children of the node named `parent_id`,
/// perturbing reading order without touching the frame's placements at
/// all — so `audit`'s `FocusOrderIsChildOrder` rule (which compares tree
/// pre-order against `FocusTree::from_placements(&frame.placements, ..)`,
/// untouched) sees a real divergence.
fn swap_two_focusable_children(node: &mut SemanticNode, parent_id: &str) -> bool {
    if node.id == parent_id {
        let indices: Vec<usize> = node
            .children
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_focusable())
            .map(|(i, _)| i)
            .collect();
        assert!(
            indices.len() >= 2,
            "find_parent_with_two_focusable_children lied about {parent_id}"
        );
        node.children.swap(indices[0], indices[1]);
        return true;
    }
    node.children
        .iter_mut()
        .any(|child| swap_two_focusable_children(child, parent_id))
}

/// One captured screen: a human-readable name, and the real
/// `(SemanticTree, PetrifiedFrame)` pair it projected.
struct Screen {
    name: &'static str,
    tree: SemanticTree,
    frame: PetrifiedFrame,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_inspector_screen_passes_the_semantic_audit() {
    let daemon = Daemon::boot_demo().await;
    let ui_dir = tempfile::tempdir().expect("tempdir for the driver socket");
    let (driven, driver) = inspector_with_client(
        ui_dir.path(),
        "gorgon-inspector-semantic-audit",
        daemon.endpoint(),
    )
    .await;
    // The hub the driver's own `tree`/`frame` verbs answer from (module doc).
    let hub = driven.hub();

    // Wait for a live connection the same way `inspector_journey.rs` does,
    // so the default screen this test captures next is not an empty
    // "connecting…" placeholder.
    let endpoint = daemon.endpoint();
    wait_tree(&driver, "connected to the daemon", DATA_WAIT, |root| {
        root.iter()
            .any(|n| n.label.starts_with("connected to ") && n.label.contains(&endpoint))
            .then_some(())
    })
    .await;
    wait_tree(
        &driver,
        "showed at least one live fiber row",
        DATA_WAIT,
        |root| {
            root.iter()
                .any(|n| n.role == Some(Role::ListItem))
                .then_some(())
        },
    )
    .await;

    let mut screens: Vec<Screen> = Vec::new();

    // ---- Screen 1: the three always-on panels (identity, fiber list, ----
    // ---- fiber detail) -- `Slot::Leading`/`Slot::Trailing`, on screen ----
    // ---- with no navigation at all. ----
    let (tree, frame) = snapshot(hub);
    println!("SCREEN: default (identity + fiber list + fiber detail)");
    screens.push(Screen {
        name: "default",
        tree,
        frame,
    });

    // ---- Screens 2-5: the four `Slot::Tab` panels, reached by real ----
    // ---- Tab presses and a real Enter -- exactly how an operator ----
    // ---- without a mouse reaches them (module doc). ----
    for tab_label in TABS {
        tab_until(
            &driver,
            &format!("reached the {tab_label} tab control"),
            MAX_TAB_PRESSES,
            |node| node.role == Some(Role::Tab) && node.label.contains(tab_label),
        )
        .await;
        press_enter(&driver).await;
        wait_tree(
            &driver,
            &format!("selected the {tab_label} tab"),
            DATA_WAIT,
            |root| {
                root.iter()
                    .any(|n| {
                        n.role == Some(Role::Tab) && n.label.contains(tab_label) && n.state.selected
                    })
                    .then_some(())
            },
        )
        .await;
        let (tree, frame) = snapshot(hub);
        println!("SCREEN: {tab_label}");
        screens.push(Screen {
            name: tab_label,
            tree,
            frame,
        });
    }

    // The property the message under the old `screens.len() == 5` assertion
    // named and that assertion never checked: the length was set four lines
    // above by this same loop and could not fail. Found by the 2026-08-24
    // shakedown. What matters is that every panel `Shell::mounted` composes
    // actually appeared on some captured screen — an audit that ran over a
    // composition missing a panel would report zero violations for it.
    const PANELS: [&str; 7] = [
        "identity",
        "fibers",
        "fiber-detail",
        "approvals",
        "leaks",
        "unload",
        "trace",
    ];
    for panel in PANELS {
        let seen = screens.iter().any(|screen| {
            screen
                .tree
                .iter()
                .any(|node| node.id.split('/').any(|segment| segment == panel))
        });
        assert!(
            seen,
            "no captured screen carries a node under `{panel}`; this audit ran over a \
             composition that is missing that panel, and reported zero violations for it"
        );
    }

    // ---- Consistency: the hub and the wire agree on the current frame. ----
    // Proves `FrameHub::current` (what this test audits) and the driver's
    // own `frame` verb (what a real driver caller would read) are the same
    // frame, not two different views of it.
    let wire_frame = driver.frame().await.expect("`frame` verb decodes");
    let last = screens.last().expect("five screens were pushed above");
    assert_eq!(
        wire_frame.seq, last.frame.seq,
        "the hub and the wire disagree about the current frame; the audit input would not be \
         what the driver itself is answering from"
    );

    // ---- Confirm the driver-protocol limitation this file was told to ----
    // ---- expect, rather than assuming it. ----
    plain_text_nodes_carry_no_label_but_status_and_actionable_nodes_do(&screens);

    // ---- Run the real rule engine over every screen, tallying how many ----
    // ---- real candidate nodes each of the four named obligations saw. ----
    let mut actionable_candidates = 0usize;
    let mut status_candidates = 0usize;
    let mut focusable_candidates = 0usize;
    let mut truncated_candidates = 0usize;
    let mut all_violations: Vec<(&'static str, AuditViolation)> = Vec::new();

    for screen in &screens {
        for node in screen.tree.iter() {
            if !node.actions.is_empty() {
                actionable_candidates += 1;
            }
            if node.role == Some(Role::Status) {
                status_candidates += 1;
            }
            if node.is_focusable() {
                focusable_candidates += 1;
            }
            if node.state.truncated {
                truncated_candidates += 1;
            }
        }
        for violation in audit(&screen.tree, &screen.frame) {
            all_violations.push((screen.name, violation));
        }
    }

    println!(
        "OBLIGATION-COUNT: actionable-needs-role-and-label candidates={actionable_candidates}"
    );
    println!("OBLIGATION-COUNT: status-needs-label candidates={status_candidates}");
    println!("OBLIGATION-COUNT: focus-order-is-child-order candidates={focusable_candidates}");
    println!("OBLIGATION-COUNT: truncation-is-real candidates={truncated_candidates}");
    println!("status nodes checked: {status_candidates}");

    assert!(
        actionable_candidates > 0,
        "no node accepting an action was captured on any screen; \
         ActionableNeedsRoleAndLabel was never exercised against real data"
    );
    assert!(
        status_candidates > 0,
        "no Role::Status node was captured on any screen; StatusNeedsLabel was never exercised \
         against real data"
    );
    assert!(
        focusable_candidates > 0,
        "no focusable node was captured on any screen; FocusOrderIsChildOrder was never \
         exercised against real data"
    );
    // `truncated_candidates` is not asserted `> 0`: a clean run legitimately
    // has zero truncated nodes, and TruncationIsReal's obligation is still
    // exercised (it runs against every placement, truncated or not) even
    // when nothing is currently hiding content.

    // ---- Report every real violation plainly. Never suppressed, never ----
    // ---- allowlisted -- see the module doc's "Two things" section. ----
    println!(
        "REAL VIOLATIONS: {} across {} screens",
        all_violations.len(),
        screens.len()
    );
    for (screen, violation) in &all_violations {
        println!("  [{screen}] {violation}");
    }
    let truncation_violations: Vec<_> = all_violations
        .iter()
        .filter(|(_, v)| v.rule == AuditRule::TruncationIsReal)
        .collect();
    if !truncation_violations.is_empty() {
        println!(
            "NOTE: {} TruncationIsReal violation(s) found. These may trace to the known open \
             defect where `Placement::clip` is inherited from the nearest clipping ancestor \
             rather than derived from the node's own rect \
             (.agents/notes/proposed/bug-fix/2026-08-24-inspector-text-paints-outside-its-rect.md) \
             — reported honestly here, not suppressed and not allowlisted.",
            truncation_violations.len()
        );
    }

    // The gate: zero violations for the three obligations this task can
    // assert unconditionally. `TruncationIsReal` is reported above but does
    // not fail this assertion on its own — the task brief is explicit that
    // a real violation tracing to the known open defect is a legitimate,
    // honestly-reported outcome, and weakening or gating around the rule to
    // force a green run here would be exactly the theater this wave exists
    // to prevent.
    // `TruncationIsReal` is not asserted with the other three, but it is not
    // unbounded either. Pinning the count is what keeps the rule
    // load-bearing: excluded-and-unpinned, it could go from zero to any
    // number and this gate would stay green, which the 2026-08-24 shakedown
    // named. Raising this number is a deliberate act that has to name the
    // defect it is admitting.
    const KNOWN_TRUNCATION_VIOLATIONS: usize = 0;
    assert_eq!(
        truncation_violations.len(),
        KNOWN_TRUNCATION_VIOLATIONS,
        "the number of TruncationIsReal violations moved. This rule is reported rather than \
         asserted alongside the other three because the open `Placement::clip` defect \
         (.agents/notes/proposed/bug-fix/2026-08-24-inspector-text-paints-outside-its-rect.md) \
         can legitimately produce one — but the count is pinned, so a change has to be read \
         and accounted for rather than absorbed silently:\n{}",
        truncation_violations
            .iter()
            .map(|(s, v)| format!("[{s}] {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    let hard_violations: Vec<_> = all_violations
        .iter()
        .filter(|(_, v)| v.rule != AuditRule::TruncationIsReal)
        .collect();
    assert!(
        hard_violations.is_empty(),
        "semantic audit found violations outside the known truncation defect:\n{}",
        hard_violations
            .iter()
            .map(|(s, v)| format!("[{s}] {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // ---- Sabotage: prove `audit` is load-bearing for each of the four ----
    // ---- named obligations (module doc's "Sabotage" section). ----
    sabotage_actionable_needs_role_and_label(&screens);
    sabotage_status_needs_label(&screens);
    sabotage_focus_order_is_child_order(&screens);
    sabotage_truncation_is_real(&screens);
}

/// Confirms, against the real live run, the driver-protocol limitation this
/// task was told to expect: `semantic::project` carries a `label` only on
/// interactive nodes and `status()` readouts, never on plain
/// `text()`/`heading()` prose.
fn plain_text_nodes_carry_no_label_but_status_and_actionable_nodes_do(screens: &[Screen]) {
    let mut saw_unlabeled_non_interactive = false;
    let mut saw_labeled_actionable = false;
    let mut saw_labeled_status = false;
    for screen in screens {
        for node in screen.tree.iter() {
            let interactive = !node.actions.is_empty();
            let status = node.role == Some(Role::Status);
            if !interactive && !status && node.label.is_empty() {
                saw_unlabeled_non_interactive = true;
            }
            if interactive && !node.label.trim().is_empty() {
                saw_labeled_actionable = true;
            }
            if status && !node.label.trim().is_empty() {
                saw_labeled_status = true;
            }
        }
    }
    assert!(
        saw_unlabeled_non_interactive,
        "expected at least one plain, non-interactive, non-status node with no label anywhere \
         in the captured screens (the claimed driver-protocol limitation); found none, so the \
         claim this file's module doc repeats does not hold against this build and should be \
         corrected rather than assumed"
    );
    assert!(
        saw_labeled_actionable,
        "expected at least one actionable node with a real label; found none"
    );
    assert!(
        saw_labeled_status,
        "expected at least one Role::Status node with a real label; found none"
    );
    println!(
        "CONFIRMED: plain non-interactive nodes carry no label; actionable and status nodes do"
    );
}

/// Sabotage 1/4: blank the label on a real actionable node's in-memory
/// clone; `ActionableNeedsRoleAndLabel` must go red naming it.
fn sabotage_actionable_needs_role_and_label(screens: &[Screen]) {
    let screen = screens
        .iter()
        .find(|s| {
            s.tree
                .iter()
                .any(|n| !n.actions.is_empty() && !n.label.trim().is_empty())
        })
        .expect("no screen carried an actionable, labeled node to sabotage");
    let baseline = audit(&screen.tree, &screen.frame);
    assert!(
        !baseline
            .iter()
            .any(|v| v.rule == AuditRule::ActionableNeedsRoleAndLabel),
        "baseline already violates ActionableNeedsRoleAndLabel on {}; sabotage cannot prove \
         anything against a screen that is not clean to start with",
        screen.name
    );
    let target = screen
        .tree
        .iter()
        .find(|n| !n.actions.is_empty() && !n.label.trim().is_empty())
        .expect("found above")
        .id
        .clone();

    let mut sabotaged_root = screen.tree.root().clone();
    assert!(
        blank_label(&mut sabotaged_root, &target),
        "failed to reach {target} while blanking its label"
    );
    let sabotaged_tree = SemanticTree::new(sabotaged_root);
    let after = audit(&sabotaged_tree, &screen.frame);
    let caught = after
        .iter()
        .any(|v| v.rule == AuditRule::ActionableNeedsRoleAndLabel && v.node_id == target);
    if caught {
        println!(
            "SABOTAGE actionable-needs-role-and-label caught: blanked label on {target} \
             ({}) -> audit flagged it",
            screen.name
        );
    } else {
        println!(
            "SABOTAGE actionable-needs-role-and-label MISSED: blanked label on {target} \
             ({}) but audit stayed clean",
            screen.name
        );
    }
    assert!(
        caught,
        "ActionableNeedsRoleAndLabel is not load-bearing: blanking the label on actionable node \
         {target} did not trip the rule"
    );
}

/// Sabotage 2/4: blank the label on a real `Role::Status` node's in-memory
/// clone; `StatusNeedsLabel` must go red naming it.
fn sabotage_status_needs_label(screens: &[Screen]) {
    let screen = screens
        .iter()
        .find(|s| {
            s.tree
                .iter()
                .any(|n| n.role == Some(Role::Status) && !n.label.trim().is_empty())
        })
        .expect("no screen carried a labeled Role::Status node to sabotage");
    let baseline = audit(&screen.tree, &screen.frame);
    assert!(
        !baseline
            .iter()
            .any(|v| v.rule == AuditRule::StatusNeedsLabel),
        "baseline already violates StatusNeedsLabel on {}",
        screen.name
    );
    let target = screen
        .tree
        .iter()
        .find(|n| n.role == Some(Role::Status) && !n.label.trim().is_empty())
        .expect("found above")
        .id
        .clone();

    let mut sabotaged_root = screen.tree.root().clone();
    assert!(
        blank_label(&mut sabotaged_root, &target),
        "failed to reach {target} while blanking its label"
    );
    let sabotaged_tree = SemanticTree::new(sabotaged_root);
    let after = audit(&sabotaged_tree, &screen.frame);
    let caught = after
        .iter()
        .any(|v| v.rule == AuditRule::StatusNeedsLabel && v.node_id == target);
    if caught {
        println!(
            "SABOTAGE status-needs-label caught: blanked label on {target} ({}) -> audit \
             flagged it",
            screen.name
        );
    } else {
        println!(
            "SABOTAGE status-needs-label MISSED: blanked label on {target} ({}) but audit \
             stayed clean",
            screen.name
        );
    }
    assert!(
        caught,
        "StatusNeedsLabel is not load-bearing: blanking the label on status node {target} did \
         not trip the rule -- this is exactly the colour-only-status failure mode FR-015 exists \
         to catch"
    );
}

/// Sabotage 3/4: swap two focusable siblings' positions on an in-memory
/// clone of the tree, leaving the frame's placements untouched;
/// `FocusOrderIsChildOrder` must go red.
fn sabotage_focus_order_is_child_order(screens: &[Screen]) {
    let screen = screens
        .iter()
        .find(|s| find_parent_with_two_focusable_children(s.tree.root()).is_some())
        .expect("no screen had a parent with two focusable children to sabotage");
    let baseline = audit(&screen.tree, &screen.frame);
    assert!(
        !baseline
            .iter()
            .any(|v| v.rule == AuditRule::FocusOrderIsChildOrder),
        "baseline already violates FocusOrderIsChildOrder on {}",
        screen.name
    );
    let parent_id = find_parent_with_two_focusable_children(screen.tree.root())
        .expect("found above")
        .id
        .clone();

    let mut sabotaged_root = screen.tree.root().clone();
    assert!(
        swap_two_focusable_children(&mut sabotaged_root, &parent_id),
        "failed to reach {parent_id} while swapping its focusable children"
    );
    let sabotaged_tree = SemanticTree::new(sabotaged_root);
    let after = audit(&sabotaged_tree, &screen.frame);
    let caught = after
        .iter()
        .any(|v| v.rule == AuditRule::FocusOrderIsChildOrder);
    if caught {
        println!(
            "SABOTAGE focus-order-is-child-order caught: swapped two focusable children under \
             {parent_id} ({}) -> audit flagged it",
            screen.name
        );
    } else {
        println!(
            "SABOTAGE focus-order-is-child-order MISSED: swapped two focusable children under \
             {parent_id} ({}) but audit stayed clean",
            screen.name
        );
    }
    assert!(
        caught,
        "FocusOrderIsChildOrder is not load-bearing: reordering two focusable children under \
         {parent_id} did not trip the rule"
    );
}

/// Sabotage 4/4: flip `state.truncated` to `true` on a real node's
/// in-memory clone whose placement still says `false`; `TruncationIsReal`
/// must go red naming the mismatch.
fn sabotage_truncation_is_real(screens: &[Screen]) {
    // A target this rule was not already flagging in the baseline, so the
    // new violation after sabotage is provably caused by the sabotage and
    // not a pre-existing one this test happened to also match.
    let screen = screens
        .iter()
        .find(|s| {
            let baseline_flagged: std::collections::HashSet<String> = audit(&s.tree, &s.frame)
                .into_iter()
                .filter(|v| v.rule == AuditRule::TruncationIsReal)
                .map(|v| v.node_id)
                .collect();
            s.tree
                .iter()
                .any(|n| !n.state.truncated && !baseline_flagged.contains(&n.id))
        })
        .expect("no screen had an untruncated node clear of the baseline to sabotage");
    let target = screen
        .tree
        .iter()
        .find(|n| !n.state.truncated)
        .expect("found above")
        .id
        .clone();

    let mut sabotaged_root = screen.tree.root().clone();
    assert!(
        set_truncated(&mut sabotaged_root, &target),
        "failed to reach {target} while flipping its truncated flag"
    );
    let sabotaged_tree = SemanticTree::new(sabotaged_root);
    let after = audit(&sabotaged_tree, &screen.frame);
    let caught = after
        .iter()
        .any(|v| v.rule == AuditRule::TruncationIsReal && v.node_id == target);
    if caught {
        println!(
            "SABOTAGE truncation-is-real caught: flipped state.truncated on {target} ({}) -> \
             audit flagged the tree/placement mismatch",
            screen.name
        );
    } else {
        println!(
            "SABOTAGE truncation-is-real MISSED: flipped state.truncated on {target} ({}) but \
             audit stayed clean",
            screen.name
        );
    }
    assert!(
        caught,
        "TruncationIsReal is not load-bearing: a tree/placement truncated mismatch on {target} \
         did not trip the rule"
    );
}
