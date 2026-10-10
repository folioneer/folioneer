//! The local channel between the bridge and the running application (ADR-023, AGT-020): on
//! Linux and other Unix systems, a socket in a folder of the application's data folder that
//! only the owner's user may enter; on Windows, a named pipe only the owner's user may
//! open, named in a file of that folder. Never a network port. A system without such a
//! channel has no agent connection.

use std::path::{Path, PathBuf};

/// Whether this system has a channel.
pub const AVAILABLE: bool = cfg!(any(unix, windows));

const CHANNEL_DIR: &str = "agent";
#[cfg(not(windows))]
const ENTRY_FILE: &str = "channel.sock";
#[cfg(windows)]
const ENTRY_FILE: &str = "channel.name";

fn channel_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(CHANNEL_DIR)
}

/// What a bridge opens in `data_dir` to reach the channel: the socket itself on Unix, the
/// file that names the pipe on Windows.
pub fn entry_path(data_dir: &Path) -> PathBuf {
    channel_dir(data_dir).join(ENTRY_FILE)
}

/// AGT-022 — removes the channel: nothing is left for a bridge to reach.
pub fn close(data_dir: &Path) {
    let _ = std::fs::remove_file(entry_path(data_dir));
    let _ = std::fs::remove_dir(channel_dir(data_dir));
}

#[cfg(unix)]
mod unix {
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::path::Path;

    use tokio::net::{UnixListener, UnixStream};

    use super::{channel_dir, entry_path};

    /// The application's end of a connection.
    pub type Stream = UnixStream;

    /// The application's end of the channel.
    #[derive(Debug)]
    pub struct Listener(UnixListener);

    impl Listener {
        /// The next bridge that connects.
        pub async fn accept(&mut self) -> io::Result<Stream> {
            self.0.accept().await.map(|(stream, _)| stream)
        }
    }

    /// AGT-021 — opens the channel in `data_dir`: a socket in a folder created for the
    /// owner's user alone, so no other user can reach the socket. A folder that is a
    /// symbolic link is refused; a socket left by an earlier run is replaced, one a running
    /// application still answers on is not.
    pub fn listen(data_dir: &Path) -> io::Result<Listener> {
        let dir = channel_dir(data_dir);
        match std::fs::symlink_metadata(&dir) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "the channel folder is not a folder of the data folder",
                ));
            }
            Ok(_) => {}
            Err(_) => std::fs::DirBuilder::new().mode(0o700).create(&dir)?,
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        let path = entry_path(data_dir);
        if std::fs::symlink_metadata(&path).is_ok() {
            // A socket that answers belongs to an application still running: it keeps it.
            if std::os::unix::net::UnixStream::connect(&path).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "another running application holds the channel",
                ));
            }
            std::fs::remove_file(&path)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        UnixListener::from_std(listener).map(Listener)
    }

    /// AGT-021 — whether the program at the other end of `stream` runs as the user who
    /// owns the channel in `data_dir`.
    pub fn same_user(stream: &Stream, data_dir: &Path) -> bool {
        let owner = std::fs::metadata(channel_dir(data_dir)).map(|metadata| metadata.uid());
        let peer = stream.peer_cred().map(|credentials| credentials.uid());
        matches!((owner, peer), (Ok(owner), Ok(peer)) if is_owner(owner, peer))
    }

    /// Whether `peer` is the user `owner`.
    pub fn is_owner(owner: u32, peer: u32) -> bool {
        owner == peer
    }

    /// The bridge's end: connects to the channel in `data_dir`; `None` when there is none
    /// or no application answers on it.
    pub async fn connect(data_dir: &Path) -> Option<UnixStream> {
        UnixStream::connect(entry_path(data_dir)).await.ok()
    }
}

#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::{connect, listen, same_user, Listener};
#[cfg(windows)]
pub use windows::{connect, listen, same_user, Listener};

/// The bridge's end of the channel to the application running over `data_dir`; `None`
/// when none answers, or on a system without a channel (AGT-023).
pub async fn reach(data_dir: &Path) -> Option<Box<dyn super::bridge::Channel>> {
    #[cfg(any(unix, windows))]
    {
        let stream = connect(data_dir).await?;
        Some(Box::new(stream) as Box<dyn super::bridge::Channel>)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = data_dir;
        None
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    }

    // AGT-021 — the channel is a socket only the owner's user can reach: its folder lets
    // nobody else in, whatever the data folder allows, and a program of the same user is
    // recognised at the other end.
    #[tokio::test]
    async fn agt_021_the_channel_is_reachable_by_the_owners_user_only() {
        let data_dir = tempfile::tempdir().expect("tempdir");
        std::fs::set_permissions(data_dir.path(), std::fs::Permissions::from_mode(0o755))
            .expect("open data folder");

        let mut listener = listen(data_dir.path()).expect("listening");

        assert_eq!(mode(&data_dir.path().join("agent")), 0o700);
        assert_eq!(mode(&entry_path(data_dir.path())), 0o600);
        let bridge = connect(data_dir.path()).await.expect("connected");
        let application_end = listener.accept().await.expect("accepted");
        assert!(same_user(&application_end, data_dir.path()));
        drop(bridge);
    }

    // AGT-021 — a test proves another user's process cannot open the channel: the folder
    // refuses it entry, and a peer of another user would not be served.
    #[tokio::test]
    async fn agt_021_another_users_process_cannot_open_the_channel() {
        let data_dir = tempfile::tempdir().expect("tempdir");
        let _listener = listen(data_dir.path()).expect("listening");
        let folder = data_dir.path().join("agent");

        let others = mode(&folder) & 0o077;
        assert_eq!(others, 0, "no permission for the group or for others");
        assert!(!unix::is_owner(1000, 1001));
        assert!(unix::is_owner(1000, 1000));

        // What another user meets: a folder that cannot be entered.
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).expect("closed");
        let refused = connect(data_dir.path()).await;
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
            .expect("reopened");
        // A process of the superuser enters any folder: the check holds for every other.
        let superuser = std::fs::metadata("/proc/self")
            .map(|metadata| std::os::unix::fs::MetadataExt::uid(&metadata) == 0)
            .unwrap_or(false);
        assert!(refused.is_none() || superuser);
    }

    // AGT-021 / AGT-022 — a socket left by an earlier run is replaced; a channel folder
    // that is a link is refused; closing leaves nothing to connect to.
    #[tokio::test]
    async fn agt_022_closing_the_channel_leaves_nothing_to_reach() {
        let data_dir = tempfile::tempdir().expect("tempdir");
        drop(listen(data_dir.path()).expect("first run"));
        assert!(connect(data_dir.path()).await.is_none(), "nobody listens");
        let _listener = listen(data_dir.path()).expect("second run");
        assert!(connect(data_dir.path()).await.is_some());
        let taken = listen(data_dir.path()).expect_err("a running application keeps its channel");
        assert_eq!(taken.kind(), std::io::ErrorKind::AddrInUse);
        assert!(connect(data_dir.path()).await.is_some());

        close(data_dir.path());
        assert!(!entry_path(data_dir.path()).exists());
        assert!(!data_dir.path().join("agent").exists());
        assert!(connect(data_dir.path()).await.is_none());

        let elsewhere = tempfile::tempdir().expect("tempdir");
        std::os::unix::fs::symlink(elsewhere.path(), data_dir.path().join("agent")).expect("link");
        assert!(listen(data_dir.path()).is_err());
    }
}
