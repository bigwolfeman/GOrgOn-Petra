//! Finding a driver socket.
//!
//! `contracts/driver-protocol.md`: the endpoint lives at
//! `$XDG_RUNTIME_DIR/gorgon/ui-<pid>.sock`. The server's own half of this is
//! [`crate::server::Server::socket_path_under`], which is `testkit`-gated
//! and therefore not importable from here (FR-041: this client must build
//! with no feature on). [`socket_path`] recomputes the identical formula
//! rather than import it — [`crate::server::tests::socket_path_is_named_for_this_process`]
//! and [`agrees_with_the_servers_own_socket_path_formula`] below are what
//! keep the two from drifting apart.

use std::path::{Path, PathBuf};

use super::error::DriverError;

/// Where a process's driver socket lives under `runtime_dir`, given its pid.
/// Must agree exactly with `Server::socket_path_under`
/// (`petra-testkit/src/server/mod.rs`) — the two are never allowed to
/// drift, since a client using the wrong formula simply never connects.
#[must_use]
pub fn socket_path(runtime_dir: &Path, pid: u32) -> PathBuf {
    runtime_dir.join("gorgon").join(format!("ui-{pid}.sock"))
}

/// This process's own socket path under `runtime_dir` — the common case for
/// an application driving its own UI in-process (a gate embedded in the same
/// binary it is testing).
#[must_use]
pub fn this_process_socket_path(runtime_dir: &Path) -> PathBuf {
    socket_path(runtime_dir, std::process::id())
}

/// `$XDG_RUNTIME_DIR`, or the honest error naming what is missing
/// (FR-043: a gate that cannot get its prerequisite fails naming it).
pub fn xdg_runtime_dir() -> Result<PathBuf, DriverError> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .ok_or(DriverError::NoRuntimeDir)
}

/// Find "another process's" socket: scan `<runtime_dir>/gorgon/` for
/// `ui-*.sock` entries. Exactly one candidate resolves unambiguously; zero or
/// more than one is reported as its own error rather than guessed at with
/// `.first()` — the same "no silent narrowing" discipline the finders in
/// [`super::client`] apply to `tree` matches.
pub fn discover_socket(runtime_dir: &Path) -> Result<PathBuf, DriverError> {
    let dir = runtime_dir.join("gorgon");
    let entries = std::fs::read_dir(&dir).map_err(DriverError::Io)?;
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(DriverError::Io)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(stripped) = name.strip_prefix("ui-")
            && stripped.ends_with(".sock")
        {
            found.push(entry.path());
        }
    }
    match found.len() {
        1 => Ok(found.into_iter().next().expect("len checked above")),
        0 => Err(DriverError::SocketNotFound { dir }),
        n => Err(DriverError::SocketDiscoveryAmbiguous { dir, found: n }),
    }
}

#[cfg(test)]
mod tests {
    use super::{discover_socket, socket_path, this_process_socket_path};

    /// The formula this module computes must match
    /// `Server::socket_path_under`'s own test
    /// (`socket_path_is_named_for_this_process` in `src/server/mod.rs`)
    /// byte-for-byte, since that function is not importable from here. This
    /// is the guard against the two drifting apart silently.
    #[test]
    fn agrees_with_the_servers_own_socket_path_formula() {
        let path = socket_path(std::path::Path::new("/run/user/1000"), 4242);
        assert_eq!(
            path,
            std::path::PathBuf::from("/run/user/1000/gorgon/ui-4242.sock")
        );
    }

    #[test]
    fn this_process_socket_path_uses_the_real_pid() {
        let path = this_process_socket_path(std::path::Path::new("/run/user/1000"));
        assert_eq!(
            path,
            std::path::PathBuf::from(format!(
                "/run/user/1000/gorgon/ui-{}.sock",
                std::process::id()
            ))
        );
    }

    #[test]
    fn discover_finds_the_one_socket_present() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gorgon_dir = dir.path().join("gorgon");
        std::fs::create_dir_all(&gorgon_dir).expect("mkdir");
        std::fs::write(gorgon_dir.join("ui-777.sock"), b"").expect("touch");
        let found = discover_socket(dir.path()).expect("exactly one candidate");
        assert_eq!(found, gorgon_dir.join("ui-777.sock"));
    }

    #[test]
    fn discover_refuses_to_guess_among_several() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gorgon_dir = dir.path().join("gorgon");
        std::fs::create_dir_all(&gorgon_dir).expect("mkdir");
        std::fs::write(gorgon_dir.join("ui-1.sock"), b"").expect("touch");
        std::fs::write(gorgon_dir.join("ui-2.sock"), b"").expect("touch");
        let err = discover_socket(dir.path()).unwrap_err();
        assert!(matches!(
            err,
            super::DriverError::SocketDiscoveryAmbiguous { found: 2, .. }
        ));
    }

    #[test]
    fn discover_reports_none_found_rather_than_hanging() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("gorgon")).expect("mkdir");
        let err = discover_socket(dir.path()).unwrap_err();
        assert!(matches!(err, super::DriverError::SocketNotFound { .. }));
    }
}
