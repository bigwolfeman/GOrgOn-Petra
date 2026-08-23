//! The driver server: a Unix socket, NDJSON framing, verb dispatch.
//!
//! `testkit`-only (FR-042): this whole module tree does not exist in a build
//! that did not ask for it, because it is declared behind `#[cfg(feature =
//! "testkit")]` on the one `pub mod server;` line in `src/lib.rs`, and that
//! feature pulls in `gorgon-petra-egui` and `nix` only as `optional`
//! dependencies (see `Cargo.toml`). There is no runtime flag anywhere in this
//! module that turns the endpoint on or off.
//!
//! # Framing
//!
//! NDJSON: one JSON object per line, request `{id, verb, params}`, response
//! `{id, ok, result | error{kind, message}}` — see [`crate::wire`].
//!
//! # Concurrency (`driver-protocol.md`: "concurrent clients' actions execute
//! in arrival order; queries interleave freely and are consistent per
//! frame")
//!
//! Two decisions, both load-bearing:
//!
//! 1. **One connection, one request in flight at a time.** `handle_conn`
//!    reads a line, awaits its whole dispatch, writes the reply, then reads
//!    the next line — no per-line `tokio::spawn`, no pipelining. This is
//!    deliberately conservative: the contract requires no reordering *within*
//!    one client's own requests, and refusing to pipeline is the cheapest way
//!    to guarantee that (a spawned-and-raced design would have to reconstruct
//!    ordering with a sequence number instead of getting it for free from
//!    "don't start request N+1 until N's reply is written"). What it costs:
//!    a client cannot have two queries in flight on one connection at once.
//!    Nothing in the contract asks for that, and the future driver client
//!    (T033) is not designed to want it — a journey helper issues one call,
//!    waits for the answer, then issues the next.
//! 2. **Different connections already interleave for free.** Each accepted
//!    connection is its own `tokio::spawn`, so two clients' queries run
//!    concurrently as a normal consequence of being on separate tasks — no
//!    extra machinery earns "queries interleave freely" beyond not adding
//!    anything that would prevent it. Each query reads one
//!    [`hub::FrameHub::current`] snapshot and answers from it alone, so it is
//!    "consistent per frame" without needing to coordinate with any other
//!    in-flight query.
//!
//! `act` and `wait_settle` are different: two actions arriving on two
//! different connections must still execute in arrival order, and (1) above
//! only orders requests *within* a connection. [`Server::action_order`] is
//! the cross-connection seam: every `act`/`wait_settle` handler acquires it
//! before doing anything (`dispatch::action`), released only when that verb
//! is fully done. `tokio::sync::Mutex` hands its permit to waiters in the
//! order they queued to `lock().await`
//! (<https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html>, "fair" FIFO
//! semaphore), and a handler calls `lock().await` as the very first thing it
//! does — immediately after that connection's `read_frame` already
//! delivered the request — so the queueing order is arrival order to the
//! precision the OS scheduler gives two concurrent connections, which is the
//! best one process can promise without a wall-clock timestamp the protocol
//! does not carry. Both verbs answer `unsupported` today (T030, T031 have not
//! landed), but they already acquire and release the slot, so the ordering
//! guarantee is live the moment real logic replaces the stub body — nothing
//! about the concurrency shape changes when it does.
//!
//! The alternative considered and rejected: a single global request queue
//! (spawn nothing per-connection; one task drains one `mpsc` channel every
//! request feeds into). It would make ordering trivially exact, at the cost
//! of serializing every `health`/`tree`/`frame` query behind it too — the
//! exact thing "queries interleave freely" rules out. Splitting the query
//! path (uncoordinated) from the action path (one mutex) gets both halves of
//! the sentence right instead of picking one.

mod dispatch;
mod hub;
mod peer;

pub use hub::{FrameHub, Published};

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::wire::{self, ErrorKind, Request, WireError};

/// A driver server, bound to one [`FrameHub`].
pub struct Server {
    hub: FrameHub,
    app: String,
    /// Cross-connection action ordering. See the module docs' "Concurrency"
    /// section for why a mutex acquired at handler entry, rather than at the
    /// connection-loop level, is the seam T030/T031 build the real
    /// serialization behind.
    action_order: tokio::sync::Mutex<()>,
}

impl Server {
    /// A new server presenting as `app` in `health`, sharing a fresh
    /// [`FrameHub`] with the caller.
    ///
    /// Returns the server (wrapped for [`Server::serve`], which needs to
    /// clone it per connection) and the hub half the embedding application
    /// keeps to call [`FrameHub::publish`] on after every `Host::pass`.
    #[must_use]
    pub fn new(app: impl Into<String>) -> (Arc<Self>, FrameHub) {
        let hub = FrameHub::new();
        let server = Arc::new(Self {
            hub: hub.clone(),
            app: app.into(),
            action_order: tokio::sync::Mutex::new(()),
        });
        (server, hub)
    }

    /// The frame state this server answers `tree`/`frame` from.
    #[must_use]
    pub fn hub(&self) -> &FrameHub {
        &self.hub
    }

    /// The `app` name `health` reports.
    #[must_use]
    pub fn app_name(&self) -> &str {
        &self.app
    }

    /// The cross-connection action-ordering slot. `dispatch::action` is the
    /// one place that acquires it; exposed at `pub(crate)` visibility only so
    /// `dispatch` can, and so a unit test can drive it directly without
    /// standing up a socket.
    pub(crate) fn action_order(&self) -> &tokio::sync::Mutex<()> {
        &self.action_order
    }

    /// Where this process's socket lives when bound under `runtime_dir`:
    /// `<runtime_dir>/gorgon/ui-<pid>.sock`.
    #[must_use]
    pub fn socket_path_under(runtime_dir: &Path) -> PathBuf {
        runtime_dir
            .join("gorgon")
            .join(format!("ui-{}.sock", std::process::id()))
    }

    /// Bind the listener at `$XDG_RUNTIME_DIR/gorgon/ui-<pid>.sock`
    /// (`contracts/driver-protocol.md`).
    ///
    /// # Errors
    /// [`BindError::NoRuntimeDir`] when `$XDG_RUNTIME_DIR` is not set. This
    /// crate does not invent a fallback directory the way `gorgond`'s
    /// endpoint discovery does for the production ctl socket
    /// (`gorgon/gorgond/src/endpoint.rs`): the driver is a testkit-only
    /// debug surface, and a gate that cannot get it fails naming the missing
    /// prerequisite (FR-043) rather than silently binding somewhere a client
    /// was never told to look.
    pub async fn bind() -> Result<UnixListener, BindError> {
        let base = std::env::var_os("XDG_RUNTIME_DIR").ok_or(BindError::NoRuntimeDir)?;
        Self::bind_under(Path::new(&base)).await
    }

    /// [`Server::bind`], but under an explicit directory instead of
    /// `$XDG_RUNTIME_DIR`.
    ///
    /// This is what the tests in `tests/server.rs` call: `$XDG_RUNTIME_DIR`
    /// is process-wide state, and `cargo test`'s default parallelism runs
    /// many tests in one process, so reading it from the environment in a
    /// test would race every other test that also touches it. Taking the
    /// directory as a parameter sidesteps that instead of serializing the
    /// whole suite on an env-var mutex.
    pub async fn bind_under(runtime_dir: &Path) -> Result<UnixListener, BindError> {
        bind_at(&Self::socket_path_under(runtime_dir)).await
    }

    /// Serve forever: accept, spawn one task per connection, repeat.
    ///
    /// Returns when the listener itself errors (e.g. the socket file was
    /// removed out from under it). A per-connection error is logged to
    /// stderr and does not stop the loop — one misbehaving client must not
    /// take the endpoint down for every other one.
    pub async fn serve(self: Arc<Self>, listener: UnixListener) {
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        if let Err(err) = server.handle_conn(stream).await {
                            eprintln!("gorgon-petra-testkit: driver conn: {err}");
                        }
                    });
                }
                Err(err) => {
                    eprintln!("gorgon-petra-testkit: driver accept: {err}");
                    break;
                }
            }
        }
    }

    async fn handle_conn(&self, stream: UnixStream) -> std::io::Result<()> {
        if let Err(err) = peer::check_same_uid(&stream) {
            let mut stream = stream;
            let line = wire::err_response(&Value::Null, &err).to_string();
            let _ = stream.write_all(format!("{line}\n").as_bytes()).await;
            return Ok(());
        }
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let reply = self.dispatch_line(&line).await;
            writer.write_all(reply.as_bytes()).await?;
            writer.write_all(b"\n").await?;
        }
        Ok(())
    }

    async fn dispatch_line(&self, line: &str) -> String {
        let req: Request = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(err) => {
                return wire::err_response(
                    &Value::Null,
                    &WireError::new(
                        ErrorKind::InvalidParams,
                        format!("malformed request: {err}"),
                    ),
                )
                .to_string();
            }
        };
        let id = req.id.clone();
        match dispatch::dispatch(self, &req).await {
            Ok(result) => wire::ok_response(&id, result).to_string(),
            Err(err) => wire::err_response(&id, &err).to_string(),
        }
    }
}

/// Why [`Server::bind`]/[`Server::bind_under`] could not produce a listener.
#[derive(Debug)]
pub enum BindError {
    /// `$XDG_RUNTIME_DIR` is not set ([`Server::bind`] only).
    NoRuntimeDir,
    /// Bind or filesystem failure.
    Io(std::io::Error),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRuntimeDir => write!(
                f,
                "$XDG_RUNTIME_DIR is not set; the driver socket has nowhere \
                 defined to live"
            ),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for BindError {}

/// Bind the listener at `path`. Dir `0700`, socket `0600` — same discipline
/// as the 001 ctl socket (`gorgon/gorgond/src/server.rs::bind`), which
/// `contracts/driver-protocol.md` names directly ("same discipline as the 001
/// ctl socket").
async fn bind_at(path: &Path) -> Result<UnixListener, BindError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(BindError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(BindError::Io)?;
        }
    }
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).map_err(BindError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(BindError::Io)?;
    }
    Ok(listener)
}

#[cfg(test)]
mod tests {
    use super::{BindError, Server};

    #[test]
    fn socket_path_is_named_for_this_process() {
        let path = Server::socket_path_under(std::path::Path::new("/run/user/1000"));
        assert_eq!(
            path,
            std::path::PathBuf::from(format!(
                "/run/user/1000/gorgon/ui-{}.sock",
                std::process::id()
            ))
        );
    }

    #[tokio::test]
    async fn bind_without_xdg_runtime_dir_fails_naming_it() {
        // SAFETY-adjacent note, not unsafe: this mutates process environment,
        // which is why every other test in this file avoids `Server::bind`
        // and uses `bind_under` instead. This one test exists specifically to
        // prove the `NoRuntimeDir` path, so it owns the one read of the real
        // variable; `cargo test`'s default same-process parallelism is the
        // reason no other test in this crate reads `XDG_RUNTIME_DIR` this
        // way.
        let saved = std::env::var_os("XDG_RUNTIME_DIR");
        // SAFETY: no other test in this binary touches `XDG_RUNTIME_DIR`, and
        // the mutation is undone before this test returns.
        unsafe {
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
        let result = Server::bind().await;
        // SAFETY: restoring exactly what was read above.
        unsafe {
            match &saved {
                Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
                None => std::env::remove_var("XDG_RUNTIME_DIR"),
            }
        }
        match result {
            Err(BindError::NoRuntimeDir) => {}
            other => panic!("expected NoRuntimeDir, got {other:?}"),
        }
    }
}
