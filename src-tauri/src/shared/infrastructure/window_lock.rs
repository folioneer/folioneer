//! The lock the application window holds on its data folder while it runs (CLI-030): a
//! command-line write finding it held refuses, so two programs never write the portfolio
//! at once. The operating system releases the lock when the holder exits, even abruptly.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

const LOCK_FILE: &str = "window.lock";

/// The lock of a running window; released when dropped or when the process ends.
#[derive(Debug)]
pub struct WindowLock {
    _file: File,
}

/// What a probe learns about the window when it tries the lock.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub enum WindowState {
    /// No window holds the lock.
    Closed,
    /// A window holds the lock.
    Open,
}

fn lock_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOCK_FILE)
}

/// Opens the lock file, refusing one that is a symbolic link: the lock is taken on the file
/// in the data folder, never on whatever a link points to.
fn open_lock_file(data_dir: &Path) -> io::Result<File> {
    let path = lock_path(data_dir);
    if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the window lock file is a symbolic link",
        ));
    }
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
}

impl WindowLock {
    /// Takes the lock, for a starting window or for a command while it writes (CLI-030).
    /// `Ok(None)` when another program already holds it.
    pub fn acquire(data_dir: &Path) -> io::Result<Option<Self>> {
        let file = open_lock_file(data_dir)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

/// CLI-030 — whether a window holds the lock. The probe takes the lock and releases it at
/// once; `Err` when the lock file cannot be opened or locked for another reason.
#[cfg(test)]
pub fn window_state(data_dir: &Path) -> io::Result<WindowState> {
    let file = open_lock_file(data_dir)?;
    match file.try_lock() {
        Ok(()) => Ok(WindowState::Closed),
        Err(TryLockError::WouldBlock) => Ok(WindowState::Open),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("folioneer-lock-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    // CLI-030 — a running window holds the lock; a command sees it open until it closes.
    #[test]
    fn cli_030_a_held_lock_reads_as_an_open_window_until_released() {
        let dir = fresh_dir("held");
        assert_eq!(window_state(&dir).expect("probe"), WindowState::Closed);

        let lock = WindowLock::acquire(&dir)
            .expect("acquire")
            .expect("free lock");
        assert_eq!(window_state(&dir).expect("probe"), WindowState::Open);
        assert!(WindowLock::acquire(&dir).expect("second acquire").is_none());

        drop(lock);
        assert_eq!(window_state(&dir).expect("probe"), WindowState::Closed);
    }

    // CLI-030 — a lock file replaced by a symbolic link is refused, not followed.
    #[cfg(unix)]
    #[test]
    fn cli_030_a_symbolic_link_is_refused() {
        let dir = fresh_dir("link");
        let elsewhere = dir.join("elsewhere");
        std::fs::write(&elsewhere, b"").expect("target");
        std::os::unix::fs::symlink(&elsewhere, dir.join(LOCK_FILE)).expect("link");
        assert!(WindowLock::acquire(&dir).is_err());
    }

    // CLI-030 — a folder the lock file cannot be created in is an error, not a guess.
    #[test]
    fn cli_030_an_unusable_folder_is_an_error() {
        let missing = std::env::temp_dir().join("folioneer-lock-no-such-folder/deeper");
        assert!(window_state(&missing).is_err());
        assert!(WindowLock::acquire(&missing).is_err());
    }
}
