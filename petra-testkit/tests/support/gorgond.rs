//! Boot a real `gorgond` against `examples/demo/gorgon.yaml`, on a private
//! socket, the same way `gorgon/gorgond/tests/boot.rs`'s own `Bundle` does
//! for `the_shipped_demo_composition_boots`. Trimmed to the one shape
//! T044/T045 need — an external composition, no custom layers — plus a
//! [`Daemon::restart`] on the same socket path that T045's daemon-restart
//! honesty test drives.
//!
//! # Why this spawns the binary rather than calling `gorgond::boot`
//!
//! `gorgond/tests/boot.rs`'s own module doc gives the reason and it applies
//! unchanged here: exit codes, the flag table, and the readiness line are
//! all things `main` does, and an in-process harness would be exercising a
//! second boot path no operator ever runs.
//!
//! # Why this is not `env!("CARGO_BIN_EXE_gorgond")`
//!
//! That works inside `gorgond`'s own integration tests (`tests/boot.rs`)
//! because Cargo only populates `CARGO_BIN_EXE_<name>` for a binary target
//! in the *same* package as the test — never for one in a dependency,
//! `gorgond` here being one of `gorgon-petra-testkit`'s dev-dependencies.
//! [`ensure_gorgond_binary`] gets the same real, freshly-built binary the
//! honest way this crate already has a precedent for: `tests/exclusion.rs`
//! spawns `cargo build --message-format=json` and reads the artifact path
//! back out of the JSON stream rather than assuming a `target/debug/<name>`
//! layout. A `cargo build` here that finds the binary already fresh (the
//! common case once the workspace has been built once) is a freshness
//! check, not a rebuild — cheap, and safe to run from inside a running
//! test: Cargo's own target-directory lock serializes it against any other
//! concurrent `cargo` invocation rather than racing it.

// `mod support;` recompiles this file into every test binary that pulls it
// in (`support/mod.rs`'s own module doc), and each of T044/T045 exercises a
// different subset of `Daemon`'s API — `dead_code` would otherwise flag
// whichever half one binary does not call.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// How long to wait for `gorgond endpoint:` on stdout before this harness
/// fails naming the timeout (hazard: nothing may hang).
const BOOT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long to wait for `cargo build -p gorgond` before this harness fails
/// naming the timeout, rather than hanging the suite on a wedged build.
const BUILD_TIMEOUT: Duration = Duration::from_secs(300);

/// Repository root, from this crate's manifest directory — the same
/// `ancestors().nth(2)` formula `gorgond/tests/boot.rs::repo_root` uses.
/// `gorgon/petra-testkit` sits at the same depth under the repo root as
/// `gorgon/gorgond` does, so the formula carries over unchanged.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root above gorgon/petra-testkit")
        .to_path_buf()
}

/// Build (or confirm fresh) the `gorgond` binary and return its path, read
/// straight from `cargo build`'s own `--message-format=json` stream — never
/// assumed from a `target/<profile>/<name>` convention, which a
/// `--target-dir` override or a cross-compile would break silently.
fn ensure_gorgond_binary() -> PathBuf {
    let args = [
        "build".to_owned(),
        "-p".to_owned(),
        "gorgond".to_owned(),
        "--bin".to_owned(),
        "gorgond".to_owned(),
        "--message-format=json".to_owned(),
    ];
    let mut cmd = Command::new("cargo");
    cmd.args(&args)
        .current_dir(repo_root())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Process-group leader, so a timeout below can kill the whole subtree
    // this spawns (`cargo` -> `rustc`), not just `cargo` itself — the same
    // shape `gorgon/caps/src/subprocess.rs` uses under D-027. Without this,
    // SIGKILL to `cargo` alone leaves its descendants orphaned and running:
    // SIGKILL is not inherited and a dead `cargo` cannot relay it on the
    // way down.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd
        .spawn()
        .unwrap_or_else(|err| panic!("start `cargo {}`: {err}", args.join(" ")));
    let pid = child.id();

    // Drained on its own thread, the same shape `tests/exclusion.rs` uses:
    // `wait_with_output` cannot deadlock on a full pipe this way, and the
    // bound comes from `recv_timeout` rather than an unbounded wait
    // (hazard: nothing may hang).
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let output = match rx.recv_timeout(BUILD_TIMEOUT) {
        Ok(result) => result.unwrap_or_else(|err| {
            panic!(
                "collect `cargo {}` output (pid {pid}): {err}",
                args.join(" ")
            )
        }),
        Err(_) => {
            let kill_note = match kill_process_group(pid) {
                Ok(()) => "killed its process group".to_owned(),
                Err(err) => format!("failed to kill its process group: {err}"),
            };
            panic!(
                "`cargo {}` (pid {pid}) did not finish within {BUILD_TIMEOUT:?}; {kill_note}",
                args.join(" ")
            );
        }
    };
    assert!(
        output.status.success(),
        "`cargo {}` failed:\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message.get("reason").and_then(serde_json::Value::as_str) != Some("compiler-artifact") {
            continue;
        }
        let is_gorgond_bin = message
            .get("target")
            .and_then(|target| target.get("name"))
            .and_then(serde_json::Value::as_str)
            == Some("gorgond");
        if !is_gorgond_bin {
            continue;
        }
        if let Some(executable) = message
            .get("executable")
            .and_then(serde_json::Value::as_str)
        {
            return PathBuf::from(executable);
        }
    }
    panic!(
        "`cargo {}` produced no `gorgond` binary artifact; raw stdout:\n{stdout}",
        args.join(" ")
    );
}

/// Send SIGKILL to `pid`'s entire process group (`kill(-pid, SIGKILL)`),
/// never `pid` alone. `pid` here is always a `cargo` process-group leader
/// (see [`ensure_gorgond_binary`]'s `process_group(0)`): SIGKILL is not
/// inherited, so signalling only the leader leaves any real descendant it
/// spawned (`rustc`) running once the leader is gone. Same group-kill shape
/// `gorgon/caps/src/subprocess.rs::terminate_group` uses under D-027, minus
/// the SIGTERM grace period that function gives first — this only runs
/// after the caller has already waited [`BUILD_TIMEOUT`] (300s) past the
/// build's own bound, so there is nothing left worth shutting down
/// gracefully.
///
/// `nix` is an optional dependency of this crate, gated behind the
/// `testkit` feature (`Cargo.toml`'s `testkit = [..., "dep:nix", ...]`).
/// This file compiles only when `testkit` is on — every caller that pulls
/// in `mod support;` (`journey.rs`, `inspector_journey.rs`,
/// `inspector_reconnect.rs`, `semantic_audit.rs`) gates its whole crate
/// root on `#![cfg(feature = "testkit")]`, so `nix` is guaranteed present
/// in every build where this function's body actually exists. That is
/// unlike `gorgon-xtask` (no `nix` dependency at all) and
/// `tests/exclusion.rs` (deliberately builds with no features on), which
/// is why those two shell out to the `kill` binary instead.
///
/// # Errors
/// The signal failed to send (e.g. the group is already gone).
fn kill_process_group(pid: u32) -> Result<(), String> {
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(-(pid as i32)),
        nix::sys::signal::Signal::SIGKILL,
    )
    .map_err(|errno| format!("kill(-{pid}, SIGKILL): {errno}"))
}

/// A real `gorgond`, booted from the shipped demo composition, on a socket
/// this harness owns. Killed by [`Daemon::kill`], and by `Drop`, so a
/// panicking test never leaves one running.
pub struct Daemon {
    /// Holds the socket/state directory alive for as long as `Daemon` does;
    /// never read directly (`_`-prefixed so an unused-field lint does not
    /// flag the thing that is the whole point of holding it).
    _dir: tempfile::TempDir,
    binary: PathBuf,
    sock: PathBuf,
    out: PathBuf,
    err: PathBuf,
    state: PathBuf,
    instance: String,
    child: Option<Child>,
}

impl Daemon {
    /// Boot fresh against `examples/demo/gorgon.yaml`, waiting for the
    /// readiness line before returning.
    pub async fn boot_demo() -> Self {
        // Off the async runtime's own thread: `ensure_gorgond_binary` blocks
        // on a `std::sync::mpsc::Receiver`, which is fine on a dedicated
        // thread and would starve a single-threaded executor if run inline.
        let binary = tokio::task::spawn_blocking(ensure_gorgond_binary)
            .await
            .expect("the blocking `cargo build -p gorgond` task did not panic");

        let dir = tempfile::tempdir().expect("tempdir for gorgond's state/socket");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&state).expect("state dir");
        let mut daemon = Self {
            binary,
            sock: dir.path().join("ctl.sock"),
            out: dir.path().join("daemon.out"),
            err: dir.path().join("daemon.err"),
            state,
            instance: "inspector-journey".to_string(),
            child: None,
            _dir: dir,
        };
        daemon.spawn_and_wait().await;
        daemon
    }

    /// This daemon's ctl endpoint, in the `unix://` form both
    /// `gorgon_inspector::bridge::spawn` and `gorgond::endpoint::Endpoint`
    /// itself use — the exact text `gorgond` printed on `stdout`, reparsed
    /// from the same socket path rather than assumed, so a scheme drift in
    /// `Endpoint::url` would show up here as a connect failure, not silence.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("unix://{}", self.sock.display())
    }

    /// The socket path itself, for a caller that wants to assert on it
    /// directly (T045's disconnected-state labels name it).
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.sock
    }

    /// Whether the daemon this harness spawned is still tracked as running.
    /// `false` once [`Daemon::kill`] has run and before [`Daemon::restart`]
    /// replaces it.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Kill the daemon and reap it, leaving the socket path free for
    /// [`Daemon::restart`]. Idempotent: killing an already-killed daemon is
    /// a no-op, not a panic, so `Drop` can call it unconditionally.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Boot a fresh `gorgond` on the same socket path and state root — the
    /// restart half of US3 scenario 4 ("reconnection restores live data
    /// without restarting the inspector"). `gorgond::endpoint::bind`
    /// already knows how to rebind over the stale socket file the killed
    /// process left behind (`occupied_endpoint_is_refused_and_stale_one_is_rebound`),
    /// so this needs nothing special beyond spawning again.
    ///
    /// # Panics
    /// If the previous process is still tracked as running — call
    /// [`Daemon::kill`] first. A restart over a daemon this harness has not
    /// killed would prove nothing about recovery.
    pub async fn restart(&mut self) {
        assert!(
            self.child.is_none(),
            "Daemon::restart called without Daemon::kill first: the old gorgond is still \
             tracked as running"
        );
        self.spawn_and_wait().await;
    }

    async fn spawn_and_wait(&mut self) {
        // Truncated fresh on every spawn: a restart's readiness wait must
        // not read the *previous* boot's own "gorgond endpoint:" line off a
        // stale file and report ready before the new process has written
        // anything.
        let out_file = std::fs::File::create(&self.out).expect("daemon.out");
        let err_file = std::fs::File::create(&self.err).expect("daemon.err");

        let mut cmd = Command::new(&self.binary);
        cmd.args([
            "--composition",
            repo_root()
                .join("examples/demo/gorgon.yaml")
                .to_str()
                .expect("repo root is utf-8"),
            "--instance",
            &self.instance,
            "--no-home",
            "--endpoint",
            self.sock.to_str().expect("tempdir socket path is utf-8"),
            "--strict-boot",
        ])
        // The daemon derives its trace directory and its `workspace`
        // expression variable from the state root, so pinning it here is
        // what keeps this test out of the developer's own
        // `~/.local/state` — the same reason `gorgond/tests/boot.rs`'s
        // `Bundle::command` sets it.
        .env("XDG_STATE_HOME", &self.state)
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));

        let child = cmd
            .spawn()
            .unwrap_or_else(|err| panic!("spawn {}: {err}", self.binary.display()));
        self.child = Some(child);

        let deadline = tokio::time::Instant::now() + BOOT_TIMEOUT;
        loop {
            let out = std::fs::read_to_string(&self.out).unwrap_or_default();
            if out.contains("gorgond endpoint:") {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "gorgond never finished booting within {BOOT_TIMEOUT:?}.\nstdout:\n{out}\n\
                 stderr:\n{}",
                std::fs::read_to_string(&self.err).unwrap_or_default()
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::kill_process_group;
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::time::{Duration, Instant};

    /// The reproduction that motivated this fix, run for real: a
    /// process-group leader with two children of its own (`cargo` ->
    /// `rustc`, collapsed to one `sh` spawning two `sleep`s for a fast,
    /// dependency-free test). `kill_process_group` must take out every
    /// process in the group, not just the leader — which is exactly the
    /// bug this module used to have: killing only the leader's pid left
    /// its children (SIGKILL is not inherited, and a dead `cargo` cannot
    /// relay it) running until the operator's machine was cleaned up by
    /// hand.
    #[test]
    fn kill_process_group_kills_every_process_in_the_group_not_just_the_leader() {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg("sleep 30 & sleep 30 & wait");
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .expect("spawn a process-group leader with two children");
        let pid = child.id();
        // Give the shell a moment to fork both children before killing the
        // group out from under it.
        std::thread::sleep(Duration::from_millis(200));

        kill_process_group(pid).expect("kill the process group");
        let _ = child.wait();

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if !group_has_any_process(pid) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "expected no processes left in group {pid} after kill_process_group, \
                 but `kill -0 -{pid}` still reports at least one"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// `kill -0 -<pgid>` succeeds (sends no signal, only checks) iff at
    /// least one process in that group still exists. Stderr is swallowed —
    /// `kill` prints its own "no such process" line once the group is
    /// gone, which is the expected steady state this loop polls toward,
    /// not a failure worth surfacing on every poll.
    fn group_has_any_process(pgid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &format!("-{pgid}")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}
