//! `SO_PEERCRED` same-uid authentication for the driver socket.
//!
//! Same shape as `gorgond`'s ctl socket check
//! (`gorgon/gorgond/src/server.rs::check_peer`/`authorize_uid`), including
//! the same non-Linux fallback — replicated rather than shared, because
//! `gorgond` is a daemon binary crate and pulling a dependency on it into a
//! UI testkit for one function would be the wrong direction of coupling. The
//! house style this mirrors: on a non-Linux target, the check is a no-op
//! that accepts every peer. That is a real gap on those targets, already
//! accepted for the 001 ctl socket; it is not widened here, only matched.
//!
//! Split into two functions on purpose, the same split `gorgond` makes: a
//! syscall half ([`check_same_uid`]) that this crate's own tests can only
//! ever prove the *accept* branch of — a test process cannot manufacture a
//! peer with a different uid without a second real user — and a pure
//! comparison half ([`authorize_uid`]) a unit test drives with fabricated
//! numbers to prove the *reject* branch. Neither half alone would let "the
//! same-uid check exists and is exercised" be a true sentence.

use std::os::fd::AsFd;

use tokio::net::UnixStream;

use crate::wire::{ErrorKind, WireError};

/// Verify the connecting peer's uid matches this process's own.
///
/// # Errors
/// `driver-protocol.md`'s closed error-kind set has no
/// `unauthorized`/`forbidden` entry, so a rejected peer is reported as
/// [`ErrorKind::InvalidParams`] — the closest existing meaning ("this
/// connection cannot be served as presented") — rather than inventing a
/// sixth kind the contract does not declare. The message always names the
/// real reason.
pub(super) fn check_same_uid(stream: &UnixStream) -> Result<u32, WireError> {
    #[cfg(target_os = "linux")]
    {
        let creds = nix::sys::socket::getsockopt(
            &stream.as_fd(),
            nix::sys::socket::sockopt::PeerCredentials,
        )
        .map_err(|err| {
            WireError::new(
                ErrorKind::InvalidParams,
                format!("could not read peer credentials: {err}"),
            )
        })?;
        let peer_uid = creds.uid();
        let own_uid = nix::unistd::Uid::current().as_raw();
        authorize_uid(peer_uid, own_uid)?;
        Ok(peer_uid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = stream;
        Ok(0)
    }
}

/// The comparison [`check_same_uid`] makes, pulled out so it can be driven
/// with fabricated uids in a test — the reject branch a same-process test
/// socket can never exercise for real (`SO_PEERCRED` on a loopback pair
/// always reports this process's own uid on both ends).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn authorize_uid(peer: u32, own: u32) -> Result<(), WireError> {
    if peer == own {
        Ok(())
    } else {
        Err(WireError::new(
            ErrorKind::InvalidParams,
            "unauthorized: peer uid does not match this process's uid",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::authorize_uid;
    use crate::wire::ErrorKind;

    #[test]
    fn the_same_uid_is_authorized() {
        assert!(authorize_uid(1000, 1000).is_ok());
    }

    #[test]
    fn a_different_uid_is_refused() {
        let err = authorize_uid(1000, 0).unwrap_err();
        assert_eq!(err.kind, ErrorKind::InvalidParams);
        assert!(err.message.contains("unauthorized"), "{}", err.message);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod syscall_tests {
    use super::check_same_uid;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn a_socketpair_peer_is_this_same_process_so_it_passes() {
        let (a, _b) = UnixStream::pair().expect("socketpair");
        // `UnixStream::pair` connects two ends of the same process to each
        // other, so the peer uid `getsockopt` reads back is this process's
        // own uid by construction — this is the accept branch, and it is the
        // one branch a same-process test can exercise through the real
        // syscall. The reject branch is `peer::tests::a_different_uid_is_refused`
        // above, over `authorize_uid` directly.
        assert!(check_same_uid(&a).is_ok());
    }
}
