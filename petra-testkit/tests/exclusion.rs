//! T036: production-exclusion proof (FR-042) — checked against the built
//! **artifact**, never against the source. `Cargo.toml`'s `testkit` feature
//! is `default = []`, so the claim under test is "a build that did not ask
//! for `testkit` links no driver server code and opens no socket" — and the
//! only honest way to prove that is to build the thing and look at what
//! came out, with `nm`/`strings`, the way T036's own task text asks.
//!
//! Deliberately **no** `[[test]]` entry in `Cargo.toml`, the same treatment
//! `tests/driver.rs` already has: this file must build and run with no
//! feature on, by cargo's ordinary default-feature discovery, so a plain
//! `cargo test -p gorgon-petra-testkit` — no `--features` anywhere —
//! always exercises it. `tests/server.rs` and `tests/journey.rs` are the
//! opposite shape (`required-features = ["testkit"]`); this file is the
//! mirror the contract asks for.
//!
//! # Why the check has two tests, not one
//!
//! A check that only ever looked at the no-feature build could not tell
//! "these markers are genuinely absent" from "this check searches for the
//! wrong markers and would find nothing anywhere." T036's own text says so:
//! the exclusion proof "must discriminate" — the same check run against the
//! `testkit` build has to find what it is looking for, or it proves
//! nothing. [`no_testkit_feature_links_no_driver_server_or_socket`] is the
//! claim; [`the_check_discriminates_against_a_testkit_build`] is what makes
//! the first test's silence meaningful.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Symbols that exist only because the `testkit` feature pulls in the
/// driver server, the UI-thread bridge, and the GPU snapshotter
/// (`Cargo.toml`'s `testkit = [...]` list). Rust's symbol mangling embeds
/// the full type/module path as literal substrings (verified with `nm -C`
/// against a real build of this crate: `DriverHost<driven::driven::DrivenApp,
/// ...>` appears verbatim), so a substring match on any of these is a real
/// presence check on the linked code, not a guess about what the compiler
/// might have named something.
const SERVER_SYMBOL_MARKERS: &[&str] = &["DriverHost", "FrameHub", "Snapshotter"];

/// How long a nested `cargo build` may run before this test gives up and
/// names it (hazard: nothing may hang). Generous because the `testkit`
/// shape links `wgpu`/`egui-wgpu`, which is a real compile even from a warm
/// cache the first time this exact feature combination is asked for.
const BUILD_TIMEOUT: Duration = Duration::from_secs(300);

/// Build `examples/driven`, with `--features testkit` when `testkit` is
/// `true`, and return the exact executable path **cargo itself reports**.
///
/// Never the unhashed convenience symlink cargo also leaves at
/// `target/debug/examples/driven` — that name gets repointed at whichever
/// shape was compiled most recently, so reading it after building both
/// shapes in one test run would silently read the wrong binary depending on
/// build order. Parsing `--message-format=json` for this build's own
/// `compiler-artifact` message is what keeps the two builds honestly
/// separate.
///
/// # Panics
/// If cargo fails, if the deadline passes (the process is killed and the
/// timeout is named), or if cargo's own output never named an executable.
fn build_example(testkit: bool) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // A feature-specific `--target-dir`, not the workspace's shared one.
    //
    // Both tests in this file build the *same* example target (`driven`)
    // with *different* feature sets, and `cargo test`'s default parallelism
    // runs them concurrently. Cargo's own `compiler-artifact` message names
    // the conventional unhashed path (`target/debug/examples/driven`), and
    // that name gets repointed at whichever of the two concurrent builds
    // finishes last — a real TOCTOU race that a first version of this test
    // hit directly: `no_testkit_feature_links_no_driver_server_or_socket`
    // read a `.sock` string because `the_check_discriminates_against_a_testkit_build`'s
    // build had just overwritten the shared unhashed path out from under
    // it. A distinct `--target-dir` per feature combination is what closes
    // that race completely, rather than trying to serialize the two tests
    // (which would still leave every *other* concurrent `cargo` invocation
    // in this wave able to race the same shared path).
    let target_dir = PathBuf::from(manifest_dir)
        .join("../../target/petra-exclusion-proof")
        .join(if testkit { "testkit" } else { "no-feature" });
    let mut args = vec![
        "build".to_owned(),
        "--example".to_owned(),
        "driven".to_owned(),
        "--message-format=json".to_owned(),
        "--target-dir".to_owned(),
        target_dir.display().to_string(),
    ];
    if testkit {
        args.push("--features".to_owned());
        args.push("testkit".to_owned());
    }

    let child = Command::new("cargo")
        .args(&args)
        .current_dir(manifest_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|err| panic!("start `cargo {}`: {err}", args.join(" ")));
    let pid = child.id();

    // `Child::wait_with_output` drains stdout/stderr on its own threads as
    // it waits, so this cannot deadlock on a full pipe the way polling
    // `try_wait` without reading would. The bound comes from `recv_timeout`
    // on the channel the drainer thread reports through.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let output = match rx.recv_timeout(BUILD_TIMEOUT) {
        Ok(result) => result.unwrap_or_else(|err| {
            panic!(
                "collect output of `cargo {}` (pid {pid}): {err}",
                args.join(" ")
            )
        }),
        Err(_) => {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
            panic!(
                "`cargo {}` (pid {pid}) did not finish within {BUILD_TIMEOUT:?}; killed",
                args.join(" ")
            );
        }
    };
    assert!(
        output.status.success(),
        "`cargo {}` failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut executable = None;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let is_driven_example = message["reason"] == "compiler-artifact"
            && message["target"]["name"] == "driven"
            && message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|k| k == "example"));
        if is_driven_example && let Some(exe) = message["executable"].as_str() {
            executable = Some(PathBuf::from(exe));
        }
    }
    executable.unwrap_or_else(|| {
        panic!(
            "`cargo {}` reported no executable for the `driven` example; stdout was: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

/// Run a system tool and return its stdout as text, panicking (naming the
/// tool) if it is missing or fails — this check is meaningless without it,
/// so a silent skip would be exactly the theater FR-043's discipline rules
/// out for a missing prerequisite.
fn run_tool(tool: &str, args: &[&str], target: &std::path::Path) -> String {
    let output = Command::new(tool)
        .args(args)
        .arg(target)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                panic!(
                    "{tool} is not installed; this exclusion proof needs it to inspect the \
                     built artifact (install your distribution's binutils package)"
                );
            }
            panic!("failed to run {tool}: {err}");
        });
    assert!(
        output.status.success(),
        "{tool} {args:?} {} exited {}: {}",
        target.display(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Demangled symbol names present in `binary`, via `nm -C`.
fn demangled_symbols(binary: &std::path::Path) -> String {
    run_tool("nm", &["-C"], binary)
}

/// Every printable string embedded in `binary`, via `strings`.
fn embedded_strings(binary: &std::path::Path) -> String {
    run_tool("strings", &[], binary)
}

/// FR-042: a build with no `testkit` feature links none of the driver
/// server's symbols and carries no `.sock` string anywhere in its rodata —
/// checked against the artifact `nm`/`strings` actually see, not inferred
/// from `Cargo.toml`'s `optional = true` lines.
#[test]
fn no_testkit_feature_links_no_driver_server_or_socket() {
    let binary = build_example(false);

    let symbols = demangled_symbols(&binary);
    for marker in SERVER_SYMBOL_MARKERS {
        assert!(
            !symbols.contains(marker),
            "the no-feature build of examples/driven links `{marker}`, which only exists \
             behind the `testkit` feature: {binary:?}"
        );
    }

    let strings = embedded_strings(&binary);
    assert!(
        !strings.contains(".sock"),
        "the no-feature build of examples/driven carries a `.sock` string, which should only \
         appear from the driver socket's path formula: {binary:?}"
    );
}

/// The mirror proof: the same two checks, run against the `testkit` build,
/// DO find what they are looking for. Without this, a passing check above
/// could mean "the markers are genuinely absent" or "the check is broken
/// and would say that about any binary" — this test is what rules the
/// second reading out.
#[test]
fn the_check_discriminates_against_a_testkit_build() {
    let binary = build_example(true);

    let symbols = demangled_symbols(&binary);
    for marker in SERVER_SYMBOL_MARKERS {
        assert!(
            symbols.contains(marker),
            "the testkit build of examples/driven does NOT link `{marker}`; the exclusion \
             check above would pass on any binary and proves nothing: {binary:?}"
        );
    }

    let strings = embedded_strings(&binary);
    assert!(
        strings.contains(".sock"),
        "the testkit build of examples/driven carries no `.sock` string; the exclusion check \
         above would pass on any binary and proves nothing: {binary:?}"
    );
}
