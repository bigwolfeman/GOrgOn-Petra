//! The driver client's own error vocabulary.
//!
//! [`crate::wire::ErrorKind`] is the server's closed vocabulary and is
//! preserved verbatim inside [`DriverError::Server`] — a client that
//! flattened `stale-node` and `timeout` into one string would have thrown
//! away the thing the contract made closed on purpose
//! (`contracts/driver-protocol.md`). The variants below are the client's
//! *own* failure modes: everything that can go wrong before, or instead of,
//! getting a well-formed server answer at all.

use std::fmt;
use std::path::PathBuf;

use crate::wire::WireError;

/// Everything that can go wrong using the driver client.
#[derive(Debug)]
pub enum DriverError {
    /// Connecting to the socket, or writing/reading on it, failed at the OS
    /// level.
    Io(std::io::Error),
    /// The connection closed (EOF, or a read/write error on the background
    /// reader task) while a request was still awaiting its reply. The
    /// request's outcome is unknown — never mistaken for a timeout, which
    /// carries a positive answer from the server saying so.
    ConnectionClosed,
    /// A line off the wire did not parse as `{id, ok, result | error}`, or a
    /// `result` did not parse into the type the caller asked for. Carries
    /// the parse failure's own message.
    Decode(String),
    /// The server answered with a wire error, kind preserved exactly as the
    /// closed vocabulary defines it — never flattened to a string.
    Server(WireError),
    /// A `tree` response's shape (single node vs. array of matches) was not
    /// the one this call expected, given whether the query was filtered
    /// (`contracts/driver-protocol.md`'s asymmetry).
    UnexpectedShape(String),
    /// A finder expected exactly one match and found a different count.
    /// Never silently narrowed with `.first()`.
    NotExactlyOneMatch {
        /// What was searched for, in words.
        query: String,
        /// How many nodes actually matched.
        found: usize,
    },
    /// `$XDG_RUNTIME_DIR` is not set; there is nowhere defined to look for a
    /// socket. Mirrors [`crate::server::BindError::NoRuntimeDir`] on the
    /// client side (that type is `testkit`-only, so it cannot be reused
    /// directly here).
    NoRuntimeDir,
    /// No socket exists under `<runtime_dir>/gorgon/`.
    SocketNotFound {
        /// The directory searched.
        dir: PathBuf,
    },
    /// More than one socket exists under `<runtime_dir>/gorgon/`, so "the
    /// other process's socket" is ambiguous without a pid.
    SocketDiscoveryAmbiguous {
        /// The directory searched.
        dir: PathBuf,
        /// How many candidate sockets were found.
        found: usize,
    },
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "driver io error: {err}"),
            Self::ConnectionClosed => {
                write!(f, "driver connection closed before a reply arrived")
            }
            Self::Decode(msg) => write!(f, "driver could not decode a wire message: {msg}"),
            Self::Server(err) => {
                write!(
                    f,
                    "driver server error [{}]: {}",
                    err.kind.as_wire(),
                    err.message
                )
            }
            Self::UnexpectedShape(msg) => write!(f, "unexpected tree response shape: {msg}"),
            Self::NotExactlyOneMatch { query, found } => {
                write!(f, "expected exactly one match for {query}, found {found}")
            }
            Self::NoRuntimeDir => write!(
                f,
                "$XDG_RUNTIME_DIR is not set; the driver socket has nowhere defined to live"
            ),
            Self::SocketNotFound { dir } => {
                write!(f, "no driver socket found under {}", dir.display())
            }
            Self::SocketDiscoveryAmbiguous { dir, found } => write!(
                f,
                "{found} driver sockets found under {}; name a pid to disambiguate",
                dir.display()
            ),
        }
    }
}

impl std::error::Error for DriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}
