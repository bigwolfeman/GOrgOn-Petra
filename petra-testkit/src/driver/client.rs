//! The importable driver client (T033, FR-041).
//!
//! One [`Client`] owns one connection. Writing is a plain mutex-guarded
//! half; reading runs on a background task that owns the other half for the
//! connection's whole lifetime and routes each reply to the caller that sent
//! the matching request id — **by id, not by arrival order**
//! (`contracts/driver-protocol.md`'s own client-library obligation, and this
//! module's sabotage-tested property; see `tests/driver.rs`). That
//! separation is what makes concurrent callers on one connection safe even
//! though the server today never pipelines within a connection
//! (`server/mod.rs`'s "one request in flight at a time" — see there for
//! why): a client that assumed replies always arrive in send order would be
//! correct today by accident and wrong the day that changes.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{Mutex as AsyncMutex, oneshot};
use tokio::task::JoinHandle;

use gorgon_petra::tree::Interaction;

use crate::wire::{ErrorKind, FrameResult, HealthResult, WireError};

use super::error::DriverError;
use super::node::{TreeAnswer, TreeQuery};
use super::verbs::{ActResult, ActTarget, ScreenshotRegion, ScreenshotResult, WaitSettleResult};

/// One reply line, parsed just enough to route it and hand the rest to the
/// caller: `{id, ok, result | error}`.
#[derive(Debug, Deserialize)]
struct RawEnvelope {
    id: Value,
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<RawWireError>,
}

#[derive(Debug, Deserialize)]
struct RawWireError {
    kind: String,
    message: String,
}

type Pending = Arc<StdMutex<HashMap<u64, oneshot::Sender<RawEnvelope>>>>;

/// A connected driver client.
pub struct Client {
    writer: AsyncMutex<OwnedWriteHalf>,
    pending: Pending,
    next_id: AtomicU64,
    reader_task: JoinHandle<()>,
}

impl Client {
    /// Connect to the driver socket at `path`.
    ///
    /// # Errors
    /// [`DriverError::Io`] if the connection cannot be made.
    pub async fn connect(path: &Path) -> Result<Self, DriverError> {
        let stream = UnixStream::connect(path).await.map_err(DriverError::Io)?;
        Ok(Self::from_stream(stream))
    }

    /// Wrap an already-connected stream — the seam
    /// [`Client::connect`] and any test harness that hands the client a
    /// stream it built itself both go through.
    fn from_stream(stream: UnixStream) -> Self {
        let (read_half, write_half) = stream.into_split();
        let pending: Pending = Arc::new(StdMutex::new(HashMap::new()));
        let reader_task =
            tokio::spawn(reader_loop(BufReader::new(read_half), Arc::clone(&pending)));
        Self {
            writer: AsyncMutex::new(write_half),
            pending,
            next_id: AtomicU64::new(1),
            reader_task,
        }
    }

    /// Issue one request and await its matched reply. `id`s are monotonic
    /// starting at 1 (never reused within a `Client`'s lifetime) — the
    /// server echoes exactly what it was sent
    /// ([`wire::Request`]'s own doc comment), so a monotonic, never-repeated
    /// id is what makes "match by id" a real guarantee rather than a
    /// coincidence of a small counter wrapping into a collision.
    async fn request(&self, verb: &str, params: Value) -> Result<Value, DriverError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("pending map mutex poisoned")
            .insert(id, tx);

        let line = serde_json::json!({"id": id, "verb": verb, "params": params}).to_string();
        {
            let mut writer = self.writer.lock().await;
            if let Err(err) = writer.write_all(line.as_bytes()).await {
                self.pending
                    .lock()
                    .expect("pending map mutex poisoned")
                    .remove(&id);
                return Err(DriverError::Io(err));
            }
            if let Err(err) = writer.write_all(b"\n").await {
                self.pending
                    .lock()
                    .expect("pending map mutex poisoned")
                    .remove(&id);
                return Err(DriverError::Io(err));
            }
        }

        let envelope = rx.await.map_err(|_| DriverError::ConnectionClosed)?;
        if envelope.ok {
            Ok(envelope.result)
        } else {
            let raw = envelope.error.ok_or_else(|| {
                DriverError::Decode("ok:false carried no error object".to_owned())
            })?;
            let kind = ErrorKind::parse(&raw.kind).ok_or_else(|| {
                DriverError::Decode(format!("unknown error kind on the wire: {:?}", raw.kind))
            })?;
            Err(DriverError::Server(WireError::new(kind, raw.message)))
        }
    }

    fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, DriverError> {
        serde_json::from_value(value).map_err(|err| DriverError::Decode(err.to_string()))
    }

    /// `health`: `{app, pid, testkit_version, frame_seq}`.
    pub async fn health(&self) -> Result<HealthResult, DriverError> {
        let value = self.request("health", serde_json::json!({})).await?;
        Self::decode(value)
    }

    /// `frame`: the full petrified-frame snapshot.
    pub async fn frame(&self) -> Result<FrameResult, DriverError> {
        let value = self.request("frame", serde_json::json!({})).await?;
        Self::decode(value)
    }

    /// `tree`: a query against the semantic tree. Returns
    /// [`TreeAnswer::Node`] for an unfiltered query, [`TreeAnswer::Matches`]
    /// for a filtered one — determined from the actual response shape (a
    /// JSON array vs. object), not re-derived from `query`, so this stays
    /// correct even if a future server judgment call about what counts as
    /// "filtered" ever shifts.
    pub async fn tree(&self, query: &TreeQuery) -> Result<TreeAnswer, DriverError> {
        let value = self.request("tree", query.to_params()).await?;
        if value.is_array() {
            Ok(TreeAnswer::Matches(Self::decode(value)?))
        } else {
            Ok(TreeAnswer::Node(Self::decode(value)?))
        }
    }

    /// `act`: synthesize `kind` at `target`, block until applied and
    /// settled.
    pub async fn act(
        &self,
        kind: Interaction,
        target: ActTarget,
        payload: Option<Value>,
    ) -> Result<ActResult, DriverError> {
        let params = super::verbs::act_params(kind, &target, payload);
        let value = self.request("act", params).await?;
        Self::decode(value)
    }

    /// `wait_settle`: block until the UI settles or `timeout_ms` elapses.
    pub async fn wait_settle(&self, timeout_ms: u64) -> Result<WaitSettleResult, DriverError> {
        let value = self
            .request("wait_settle", super::verbs::wait_settle_params(timeout_ms))
            .await?;
        Self::decode(value)
    }

    /// `screenshot`: identity-verified capture, optionally restricted to
    /// `region`.
    pub async fn screenshot(
        &self,
        region: ScreenshotRegion,
    ) -> Result<ScreenshotResult, DriverError> {
        let value = self.request("screenshot", region.to_params()).await?;
        Self::decode(value)
    }

    // -- Finders, built on `tree` (contract: "Finders by role/label/id"). --

    /// The node with `id` — an unfiltered `tree` query, so a match answers
    /// a single node. A `TreeAnswer::Matches` here would mean the server's
    /// asymmetry rule changed under this client; reported as
    /// [`DriverError::UnexpectedShape`] rather than silently unwrapped.
    pub async fn find_by_id(&self, id: &str) -> Result<super::node::DriverNode, DriverError> {
        let query = TreeQuery::new().with_id(id);
        self.tree(&query).await?.into_node()
    }

    /// Every node whose role matches `role`'s wire spelling exactly.
    pub async fn find_all_by_role(
        &self,
        role: &str,
    ) -> Result<Vec<super::node::DriverNode>, DriverError> {
        let query = TreeQuery::new().with_role(role);
        self.tree(&query).await?.into_matches()
    }

    /// The single node whose role matches `role`. [`DriverError::NotExactlyOneMatch`]
    /// if zero or more than one node matched — never a silent `.first()`.
    pub async fn find_by_role(&self, role: &str) -> Result<super::node::DriverNode, DriverError> {
        let query = TreeQuery::new().with_role(role);
        one(self.find_all_by_role(role).await?, query.describe())
    }

    /// Every node whose label contains `needle`.
    pub async fn find_all_by_label(
        &self,
        needle: &str,
    ) -> Result<Vec<super::node::DriverNode>, DriverError> {
        let query = TreeQuery::new().with_label_contains(needle);
        self.tree(&query).await?.into_matches()
    }

    /// The single node whose label contains `needle`.
    /// [`DriverError::NotExactlyOneMatch`] if zero or more than one matched.
    pub async fn find_by_label(
        &self,
        needle: &str,
    ) -> Result<super::node::DriverNode, DriverError> {
        let query = TreeQuery::new().with_label_contains(needle);
        one(self.find_all_by_label(needle).await?, query.describe())
    }
}

fn one<T>(mut matches: Vec<T>, query_desc: String) -> Result<T, DriverError> {
    if matches.len() == 1 {
        Ok(matches.pop().expect("len checked above"))
    } else {
        Err(DriverError::NotExactlyOneMatch {
            query: query_desc,
            found: matches.len(),
        })
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

/// Read NDJSON reply lines forever, routing each to the pending sender its
/// id names. Runs for the connection's whole lifetime; when the connection
/// closes (EOF or a read error), every request still waiting is told so via
/// [`DriverError::ConnectionClosed`] (dropping its sender — the receiving
/// `await` in [`Client::request`] turns that into the error itself, since
/// nothing else could produce a `RecvError` on a channel this module never
/// sends a manual close signal on).
async fn reader_loop(mut reader: BufReader<OwnedReadHalf>, pending: Pending) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF: the server hung up.
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(envelope) = serde_json::from_str::<RawEnvelope>(trimmed) else {
                    // A line this client cannot parse as a reply envelope at
                    // all cannot be routed to anyone; there is no request id
                    // to blame it on. Dropped rather than panicking the
                    // reader task, which would strand every other pending
                    // request too.
                    continue;
                };
                let Some(id) = envelope.id.as_u64() else {
                    // This client only ever sends `u64` ids
                    // (`Client::request`), so a reply whose id is not one is
                    // not a reply to anything this client asked for.
                    continue;
                };
                if let Some(sender) = pending
                    .lock()
                    .expect("pending map mutex poisoned")
                    .remove(&id)
                {
                    let _ = sender.send(envelope);
                }
            }
            Err(_) => break,
        }
    }
    // Connection is gone: nobody still waiting will ever get a reply. Drain
    // and drop every sender so each waiter's `rx.await` resolves to
    // `ConnectionClosed` instead of hanging forever.
    let waiters: Vec<_> = pending
        .lock()
        .expect("pending map mutex poisoned")
        .drain()
        .collect();
    drop(waiters);
}
