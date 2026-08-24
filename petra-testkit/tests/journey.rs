//! T035: the US2 acceptance journey — health -> tree -> find -> act ->
//! settle -> screenshot-verify against a sample app, zero manual steps.
//!
//! Every step here goes through [`gorgon_petra_testkit::driver::Client`],
//! never a hand-rolled socket call — `driver-protocol.md`'s own words:
//! "Hand-rolled per-test HTTP/socket code is a review-rejectable defect."
//! The sample app is `support::CountingApp` (the same one
//! `tests/server.rs`'s driven tests already prove against a real socket),
//! run for real on its own thread by `support::driven_with_client`. Nothing
//! in this file is a manual step: connecting, finding the button, clicking
//! it, waiting for settle and capturing a verified screenshot all happen
//! inside one scripted run.
//!
//! # T035's sabotage, and why it is not a test in this file
//!
//! The wave contract asks for a one-time proof that this journey is
//! load-bearing: bypass `support::CountingApp::handle`'s routing condition
//! (the real-input path for the `click` action kind), run this file, and
//! confirm the failure lands on the routing assertion below — not on some
//! unrelated digest mismatch — then restore the file with `cp` and `diff`
//! it byte-identical. That is a repo-hygiene exercise done once against a
//! working tree, not a standing test: a test that permanently sabotaged the
//! harness would just be a second, worse copy of the bug it is supposed to
//! catch. The result of that exercise is reported at the end of the wave,
//! not committed here.

#![cfg(feature = "testkit")]

mod support;

use gorgon_petra::tree::Interaction;
use gorgon_petra_testkit::driver::{ActTarget, JourneyStep, TreeAnswer, TreeQuery};

use support::driven_with_client;

/// health -> tree -> find -> act -> settle -> screenshot-verify, in order,
/// against a real driven application.
#[tokio::test]
async fn the_acceptance_journey_runs_end_to_end() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (_app, driver) = driven_with_client(dir.path()).await;

    // health: the connection is real and answers for this process.
    let health = driver.health().await.expect("health decodes");
    assert_eq!(health.app, "driven-test-app");
    assert!(health.frame_seq > 0, "no frame published yet: {health:?}");

    // tree: an unfiltered query decodes as the root node.
    match driver
        .tree(&TreeQuery::default())
        .await
        .expect("an unfiltered tree decodes")
    {
        TreeAnswer::Node(root) => assert_eq!(root.id, "/root"),
        TreeAnswer::Matches(matches) => {
            panic!("an unfiltered query answered an array of {}", matches.len())
        }
    }

    // find: the button is findable by role, starting at zero clicks.
    let button = driver
        .find_by_role("button")
        .await
        .expect("exactly one button");
    assert_eq!(button.label, "Go 0", "{button:?}");
    assert_eq!(button.id, "/root/go");

    // act -> settle -> screenshot-verify, scripted as one journey. The click
    // step asks for a screenshot too, so its evidence carries an
    // identity-verified capture on top of the frame seq/digest every step
    // already records.
    let report = driver
        .run_journey(vec![
            JourneyStep::wait_settle("settle before starting", 5_000),
            JourneyStep::act(
                "click the button",
                Interaction::Click,
                ActTarget::NodeId(button.id.clone()),
                None,
            )
            .with_screenshot(),
            JourneyStep::wait_settle("settle after the click", 5_000),
            JourneyStep::query(
                "read the label back",
                TreeQuery::default().with_role("button"),
            ),
        ])
        .await;

    assert!(
        report.all_ok(),
        "the journey did not complete: {}",
        report
            .failure_report()
            .unwrap_or_else(|| "no failure report".to_owned())
    );
    assert_eq!(report.steps.len(), 4, "{report:#?}");

    // The load-bearing assertion (T035's routing proof): the click step's
    // own evidence shows the frame actually moved, and the query step right
    // after it shows the application's real state — through the real
    // router, not merely that `act` answered. This is the assertion the
    // sabotage exercise above targets.
    let click = &report.steps[1];
    assert!(
        click.after.frame_seq > click.before.frame_seq,
        "the click step's after-evidence is not later than its before-evidence: {click:#?}"
    );
    let screenshot = click
        .after
        .screenshot
        .as_ref()
        .unwrap_or_else(|| panic!("the click step asked for a screenshot: {click:#?}"));
    assert_eq!(
        screenshot.digest.len(),
        64,
        "a digest is 32 hex bytes: {screenshot:?}"
    );
    let png = screenshot
        .png_bytes()
        .expect("the screenshot's base64 decodes");
    assert_eq!(
        &png[..8],
        &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        "the screenshot payload is not a PNG"
    );

    let last = report.steps[3]
        .outcome
        .as_ref()
        .expect("the final query step succeeded");
    match last {
        gorgon_petra_testkit::driver::StepOutcome::Queried(TreeAnswer::Matches(nodes)) => {
            assert_eq!(nodes.len(), 1, "{nodes:#?}");
            assert_eq!(
                nodes[0].label, "Go 1",
                "the click never reached the application through the real router: \
                 the button still reads {:?} (started at {:?})",
                nodes[0].label, button.label
            );
        }
        other => panic!("expected the filtered query's matches, got {other:?}"),
    }
}
