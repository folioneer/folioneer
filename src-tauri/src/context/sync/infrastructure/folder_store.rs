//! `FsFolderStore` — the filesystem-backed `FolderStore` (D8): area paths, whole-file
//! temp-then-rename publishing (SYN-032), append-only segment naming (SYN-031), manifest
//! rewrite-in-place (SYN-037), and `FolderProblem` detection (SYN-019/069).
//!
//! Layout:
//! ```text
//! <folder>/
//! ├── vaultcompass-sync.json
//! └── devices/<device_id>/
//!     ├── manifest.bin
//!     └── segments/seg-<first20>-<last20>.bin
//! ```
//!
//! Every write lands in `<name>.tmp-<uuid>` next to its destination, is fsynced, then renamed
//! into place — except the header, created in place with the OS's create-new open so an
//! existing header is never replaced (SYN-081); readers ignore `*.tmp-*`. Reads that fail
//! surface as `FolderUnavailable { problem }`, writes that fail as `PublishFailed { problem }`.
//!
//! The folder is shared with other parties: a file larger than its cap is never read, and a
//! symlink under `devices/` is never followed into, written through, or removed through.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{PoisonError, RwLock};

use crate::context::sync::domain::{
    segment_file_name, FolderProblem, FolderStore, WriteHeaderOutcome,
};
use crate::context::sync::error::SyncError;
use crate::core::logger::BACKEND;

/// Part of the folder format, not of the application's name: every device sharing a
/// portfolio looks for this file, whatever release it runs.
const HEADER_FILE_NAME: &str = "vaultcompass-sync.json";
const DEVICES_DIR: &str = "devices";
const MANIFEST_FILE_NAME: &str = "manifest.bin";
const SEGMENTS_DIR: &str = "segments";
const SEGMENT_EXTENSION: &str = ".bin";
const TEMP_MARKER: &str = ".tmp-";
/// The largest header this build reads, in bytes (1 MiB).
const MAX_HEADER_BYTES: u64 = 1024 * 1024;
/// The largest manifest this build reads, in bytes (1 MiB).
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
/// The largest segment this build reads, in bytes (64 MiB).
const MAX_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;

/// Filesystem-backed `FolderStore`.
pub struct FsFolderStore {
    root: RwLock<PathBuf>,
}

impl FsFolderStore {
    /// Creates a store rooted at `root` — the user-designated synchronised folder.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: RwLock::new(root.into()),
        }
    }

    fn root(&self) -> PathBuf {
        self.root
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn header_path(&self) -> PathBuf {
        self.root().join(HEADER_FILE_NAME)
    }
}

/// Refuses a path that is a symlink — an entry under `devices/` planted by another party is
/// never followed into, written through, or removed through. A missing path passes.
fn ensure_not_symlink(path: &Path) -> std::io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => {
            tracing::warn!(target: BACKEND, path = %path.display(), "folder store: symlink refused");
            Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                "symlink refused under devices/",
            ))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

/// `<root>/devices/<device_id>`, refusing a symlinked `devices/` or device directory.
fn checked_device_dir(root: &Path, device_id: &str) -> std::io::Result<PathBuf> {
    let devices_dir = root.join(DEVICES_DIR);
    ensure_not_symlink(&devices_dir)?;
    let device_dir = devices_dir.join(device_id);
    ensure_not_symlink(&device_dir)?;
    Ok(device_dir)
}

/// Maps an I/O failure to the folder condition it reveals (SYN-019/069).
fn classify(error: &std::io::Error) -> FolderProblem {
    match error.kind() {
        ErrorKind::NotFound => FolderProblem::Missing,
        ErrorKind::PermissionDenied => FolderProblem::PermissionDenied,
        ErrorKind::NotADirectory => FolderProblem::NotADirectory,
        ErrorKind::StorageFull => FolderProblem::OutOfSpace,
        _ => FolderProblem::IoFailure,
    }
}

fn unavailable(error: &std::io::Error) -> SyncError {
    SyncError::FolderUnavailable {
        problem: classify(error),
    }
}

fn publish_failed(error: &std::io::Error) -> SyncError {
    SyncError::PublishFailed {
        problem: classify(error),
    }
}

/// Runs blocking filesystem work off the async runtime.
async fn blocking<T, F>(work: F) -> Result<T, SyncError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, SyncError> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(target: BACKEND, err = %error, "folder store: blocking task failed");
            Err(SyncError::FolderUnavailable {
                problem: FolderProblem::IoFailure,
            })
        })
}

/// Writes `bytes` whole-or-nothing (SYN-032): temp file next to the destination, fsync,
/// rename into place.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let temp_path = parent.join(format!("{file_name}{TEMP_MARKER}{}", uuid::Uuid::new_v4()));
    let written = (|| {
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    written
}

/// Creates `path` with `bytes` only if nothing exists there yet (SYN-081): the OS's create-new
/// open is the atomic check, so an existing file is never replaced. A write that fails after
/// the create removes what it created.
fn write_create_only(path: &Path, bytes: &[u8]) -> std::io::Result<WriteHeaderOutcome> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::new(ErrorKind::InvalidInput, "path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Ok(WriteHeaderOutcome::AlreadyExists)
        }
        Err(error) => return Err(error),
    };
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(error) = written {
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(WriteHeaderOutcome::Written)
}

/// Reads a whole file, or `None` when it does not exist. A file larger than `max_bytes` is
/// never read into memory: it is unreadable (`IoFailure`).
fn read_optional(path: &Path, max_bytes: u64) -> Result<Option<Vec<u8>>, SyncError> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unavailable(&error)),
    };
    let oversized = || {
        tracing::warn!(target: BACKEND, path = %path.display(), max_bytes, "folder store: file exceeds its size cap");
        SyncError::FolderUnavailable {
            problem: FolderProblem::IoFailure,
        }
    };
    let length = file.metadata().map_err(|error| unavailable(&error))?.len();
    if length > max_bytes {
        return Err(oversized());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unavailable(&error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(oversized());
    }
    Ok(Some(bytes))
}

/// Lists the entry names of a directory, or nothing when it does not exist.
fn list_names(dir: &Path, keep: impl Fn(&fs::DirEntry) -> bool) -> Result<Vec<String>, SyncError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(vec![]),
        Err(error) => return Err(unavailable(&error)),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| unavailable(&error))?;
        if keep(&entry) {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

fn remove_if_present(
    path: &Path,
    remove: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<(), SyncError> {
    match remove(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(publish_failed(&error)),
    }
}

#[async_trait::async_trait]
impl FolderStore for FsFolderStore {
    fn retarget(&self, folder: &str) {
        *self.root.write().unwrap_or_else(PoisonError::into_inner) = PathBuf::from(folder);
    }

    async fn check_available(&self) -> Result<(), FolderProblem> {
        let root = self.root();
        let checked = blocking(move || {
            let metadata = fs::metadata(&root).map_err(|error| unavailable(&error))?;
            if !metadata.is_dir() {
                return Err(SyncError::FolderUnavailable {
                    problem: FolderProblem::NotADirectory,
                });
            }
            fs::read_dir(&root).map_err(|error| unavailable(&error))?;
            Ok(())
        })
        .await;
        match checked {
            Ok(()) => Ok(()),
            Err(SyncError::FolderUnavailable { problem }) => Err(problem),
            Err(_) => Err(FolderProblem::IoFailure),
        }
    }

    async fn read_header_bytes(&self) -> Result<Option<Vec<u8>>, SyncError> {
        let path = self.header_path();
        blocking(move || read_optional(&path, MAX_HEADER_BYTES)).await
    }

    async fn write_header_if_absent(
        &self,
        bytes: Vec<u8>,
    ) -> Result<WriteHeaderOutcome, SyncError> {
        let path = self.header_path();
        blocking(move || write_create_only(&path, &bytes).map_err(|error| publish_failed(&error)))
            .await
    }

    async fn write_segment(
        &self,
        device_id: &str,
        first_sequence: i64,
        last_sequence: i64,
        bytes: Vec<u8>,
    ) -> Result<(), SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let path = checked_device_dir(&root, &device_id)
                .map_err(|error| publish_failed(&error))?
                .join(SEGMENTS_DIR)
                .join(segment_file_name(first_sequence, last_sequence));
            write_whole(&path, &bytes).map_err(|error| publish_failed(&error))
        })
        .await
    }

    async fn write_manifest(&self, device_id: &str, bytes: Vec<u8>) -> Result<(), SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let path = checked_device_dir(&root, &device_id)
                .map_err(|error| publish_failed(&error))?
                .join(MANIFEST_FILE_NAME);
            write_whole(&path, &bytes).map_err(|error| publish_failed(&error))
        })
        .await
    }

    async fn read_manifest_bytes(&self, device_id: &str) -> Result<Option<Vec<u8>>, SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let path = checked_device_dir(&root, &device_id)
                .map_err(|error| unavailable(&error))?
                .join(MANIFEST_FILE_NAME);
            read_optional(&path, MAX_MANIFEST_BYTES)
        })
        .await
    }

    async fn list_segment_names(&self, device_id: &str) -> Result<Vec<String>, SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let dir = checked_device_dir(&root, &device_id)
                .map_err(|error| unavailable(&error))?
                .join(SEGMENTS_DIR);
            list_names(&dir, |entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.ends_with(SEGMENT_EXTENSION) && !name.contains(TEMP_MARKER)
            })
        })
        .await
    }

    async fn read_segment_bytes(
        &self,
        device_id: &str,
        name: &str,
    ) -> Result<Option<Vec<u8>>, SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        let name = name.to_string();
        blocking(move || {
            let dir = checked_device_dir(&root, &device_id)
                .map_err(|error| unavailable(&error))?
                .join(SEGMENTS_DIR);
            // A segment name is a bare file name from `list_segment_names`; one carrying a
            // path separator never leaves the segments directory.
            if name.contains(['/', '\\']) || !name.ends_with(SEGMENT_EXTENSION) {
                tracing::warn!(target: BACKEND, name = %name, "folder store: segment name refused");
                return Err(SyncError::FolderUnavailable {
                    problem: FolderProblem::IoFailure,
                });
            }
            read_optional(&dir.join(&name), MAX_SEGMENT_BYTES)
        })
        .await
    }

    async fn list_device_ids(&self) -> Result<Vec<String>, SyncError> {
        let dir = self.root().join(DEVICES_DIR);
        blocking(move || {
            ensure_not_symlink(&dir).map_err(|error| unavailable(&error))?;
            list_names(&dir, |entry| {
                entry
                    .file_type()
                    .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink())
            })
        })
        .await
    }

    async fn remove_manifest(&self, device_id: &str) -> Result<(), SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let path = checked_device_dir(&root, &device_id)
                .map_err(|error| publish_failed(&error))?
                .join(MANIFEST_FILE_NAME);
            remove_if_present(&path, |path| fs::remove_file(path))
        })
        .await
    }

    async fn remove_device_area(&self, device_id: &str) -> Result<(), SyncError> {
        let root = self.root();
        let device_id = device_id.to_string();
        blocking(move || {
            let dir =
                checked_device_dir(&root, &device_id).map_err(|error| publish_failed(&error))?;
            remove_if_present(&dir, |dir| fs::remove_dir_all(dir))
        })
        .await
    }

    async fn remove_header(&self) -> Result<(), SyncError> {
        let path = self.header_path();
        blocking(move || remove_if_present(&path, |path| fs::remove_file(path))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn header_bytes() -> Vec<u8> {
        b"{\"data_format_version\":1}".to_vec()
    }

    // SYN-019/069 — a missing folder is reported as FolderProblem::Missing.
    #[tokio::test]
    async fn check_available_reports_missing_when_folder_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let store = FsFolderStore::new(missing);
        let result = store.check_available().await;
        assert!(matches!(result, Err(FolderProblem::Missing)));
    }

    // SYN-019 — a path that exists but is a file, not a directory.
    #[tokio::test]
    async fn check_available_reports_not_a_directory_when_path_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a-file");
        std::fs::write(&file_path, b"not a directory").unwrap();
        let store = FsFolderStore::new(file_path);
        let result = store.check_available().await;
        assert!(matches!(result, Err(FolderProblem::NotADirectory)));
    }

    // SYN-019 — a writable existing directory is available.
    #[tokio::test]
    async fn check_available_ok_for_a_writable_existing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        assert!(store.check_available().await.is_ok());
    }

    // SYN-019/069 — a directory without write permission (skipped on Windows: no POSIX modes).
    #[cfg(not(windows))]
    #[tokio::test]
    async fn check_available_reports_permission_denied_for_an_unwritable_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let locked = dir.path().join("locked");
        std::fs::create_dir(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let store = FsFolderStore::new(&locked);
        let result = store.check_available().await;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(result, Err(FolderProblem::PermissionDenied)));
    }

    // Every entry point names its folder: retarget moves the store to the new root.
    #[tokio::test]
    async fn retarget_points_every_later_read_at_the_new_folder() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(first.path());
        store.write_header_if_absent(header_bytes()).await.unwrap();
        store.retarget(&second.path().to_string_lossy());
        assert!(store.read_header_bytes().await.unwrap().is_none());
    }

    // SYN-081 — the first write of the header succeeds and reports Written.
    #[tokio::test]
    async fn write_header_if_absent_reports_written_when_no_header_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        let outcome = store
            .write_header_if_absent(header_bytes())
            .await
            .expect("first write must succeed");
        assert!(matches!(outcome, WriteHeaderOutcome::Written));
    }

    // SYN-081 — a second write-if-absent reports AlreadyExists and leaves the first header
    // untouched (the last-moment re-check before a first device publishes).
    #[tokio::test]
    async fn write_header_if_absent_reports_already_exists_on_a_second_call() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store.write_header_if_absent(header_bytes()).await.unwrap();
        let outcome = store
            .write_header_if_absent(b"{\"data_format_version\":2}".to_vec())
            .await
            .expect("second call must not error");
        assert!(matches!(outcome, WriteHeaderOutcome::AlreadyExists));
        let read_back = store.read_header_bytes().await.unwrap().unwrap();
        assert_eq!(
            read_back,
            header_bytes(),
            "the original header must be left untouched"
        );
    }

    // SYN-081 — several devices racing to create the header: exactly one wins, every other
    // sees AlreadyExists, and the folder keeps the winner's header untouched.
    #[tokio::test]
    async fn write_header_if_absent_keeps_the_first_header_under_a_concurrent_race() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(FsFolderStore::new(dir.path()));
        let mut attempts = tokio::task::JoinSet::new();
        for index in 0..8u8 {
            let store = Arc::clone(&store);
            attempts.spawn(async move {
                let payload =
                    format!("{{\"data_format_version\":1,\"device\":{index}}}").into_bytes();
                let outcome = store
                    .write_header_if_absent(payload.clone())
                    .await
                    .expect("a lost race is an outcome, not an error");
                (outcome, payload)
            });
        }
        let mut written = Vec::new();
        let mut already_exists = 0;
        while let Some(result) = attempts.join_next().await {
            match result.expect("task must not panic") {
                (WriteHeaderOutcome::Written, payload) => written.push(payload),
                (WriteHeaderOutcome::AlreadyExists, _) => already_exists += 1,
            }
        }
        assert_eq!(written.len(), 1, "exactly one device creates the header");
        assert_eq!(already_exists, 7);
        let read_back = store.read_header_bytes().await.unwrap().unwrap();
        assert_eq!(
            read_back, written[0],
            "the winner's header must never be replaced"
        );
    }

    // SYN-081 — a header planted directly in the folder is never replaced.
    #[tokio::test]
    async fn write_header_if_absent_never_replaces_a_header_written_by_another_party() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(HEADER_FILE_NAME), b"theirs").unwrap();
        let store = FsFolderStore::new(dir.path());
        let outcome = store.write_header_if_absent(header_bytes()).await.unwrap();
        assert!(matches!(outcome, WriteHeaderOutcome::AlreadyExists));
        assert_eq!(
            std::fs::read(dir.path().join(HEADER_FILE_NAME)).unwrap(),
            b"theirs"
        );
    }

    // A header above its 1 MiB cap is never read: unreadable, reported as IoFailure.
    #[tokio::test]
    async fn read_header_bytes_refuses_an_oversized_header() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(HEADER_FILE_NAME),
            vec![b'{'; (MAX_HEADER_BYTES + 1) as usize],
        )
        .unwrap();
        let store = FsFolderStore::new(dir.path());
        let result = store.read_header_bytes().await;
        assert!(matches!(
            result,
            Err(SyncError::FolderUnavailable {
                problem: FolderProblem::IoFailure
            })
        ));
    }

    // A header exactly at its cap is still read.
    #[tokio::test]
    async fn read_header_bytes_reads_a_header_at_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let at_cap = vec![b'{'; MAX_HEADER_BYTES as usize];
        std::fs::write(dir.path().join(HEADER_FILE_NAME), &at_cap).unwrap();
        let store = FsFolderStore::new(dir.path());
        let read_back = store.read_header_bytes().await.unwrap().unwrap();
        assert_eq!(read_back.len(), at_cap.len());
    }

    // A manifest above its 1 MiB cap is never read: unreadable, reported as IoFailure.
    #[tokio::test]
    async fn read_manifest_bytes_refuses_an_oversized_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let device_dir = dir.path().join(DEVICES_DIR).join("desktop-device");
        std::fs::create_dir_all(&device_dir).unwrap();
        std::fs::write(
            device_dir.join(MANIFEST_FILE_NAME),
            vec![0u8; (MAX_MANIFEST_BYTES + 1) as usize],
        )
        .unwrap();
        let store = FsFolderStore::new(dir.path());
        let result = store.read_manifest_bytes("desktop-device").await;
        assert!(matches!(
            result,
            Err(SyncError::FolderUnavailable {
                problem: FolderProblem::IoFailure
            })
        ));
    }

    // A symlinked device directory is never listed, written through, read through, or
    // removed through — the real area behind it stays intact.
    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_device_dir_is_skipped_and_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_manifest("real-device", b"manifest".to_vec())
            .await
            .unwrap();
        let devices_dir = dir.path().join(DEVICES_DIR);
        std::os::unix::fs::symlink(
            devices_dir.join("real-device"),
            devices_dir.join("linked-device"),
        )
        .unwrap();

        assert_eq!(
            store.list_device_ids().await.unwrap(),
            vec!["real-device".to_string()],
            "a symlink under devices/ is not a device area"
        );
        assert!(matches!(
            store.write_manifest("linked-device", b"x".to_vec()).await,
            Err(SyncError::PublishFailed {
                problem: FolderProblem::IoFailure
            })
        ));
        assert!(matches!(
            store.read_manifest_bytes("linked-device").await,
            Err(SyncError::FolderUnavailable {
                problem: FolderProblem::IoFailure
            })
        ));
        assert!(matches!(
            store.remove_device_area("linked-device").await,
            Err(SyncError::PublishFailed {
                problem: FolderProblem::IoFailure
            })
        ));
        assert!(
            devices_dir
                .join("real-device")
                .join(MANIFEST_FILE_NAME)
                .exists(),
            "the area behind the symlink must never be removed"
        );
    }

    // SYN-032 — after a segment write, no `*.tmp-*` file remains in the segments directory.
    #[tokio::test]
    async fn write_segment_leaves_no_tmp_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_segment("desktop-device", 1, 3, b"sealed-segment".to_vec())
            .await
            .expect("write_segment must succeed");

        let segments_dir = dir
            .path()
            .join(DEVICES_DIR)
            .join("desktop-device")
            .join(SEGMENTS_DIR);
        let entries: Vec<_> = std::fs::read_dir(&segments_dir)
            .expect("segments dir must exist")
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries.iter().all(|name| !name.contains(".tmp-")),
            "no temp file must remain after a completed write: {entries:?}"
        );
    }

    // SYN-031 — the segment is named by its first/last sequence range.
    #[tokio::test]
    async fn write_segment_names_the_file_by_sequence_range() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_segment("desktop-device", 1, 3, b"sealed-segment".to_vec())
            .await
            .unwrap();
        let names = store.list_segment_names("desktop-device").await.unwrap();
        assert_eq!(names, vec![segment_file_name(1, 3)]);
    }

    // SYN-032 — a reader listing segments ignores a leftover `*.tmp-*` file (an interrupted
    // write from a previous run).
    #[tokio::test]
    async fn list_segment_names_ignores_leftover_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let segments_dir = dir
            .path()
            .join(DEVICES_DIR)
            .join("desktop-device")
            .join(SEGMENTS_DIR);
        std::fs::create_dir_all(&segments_dir).unwrap();
        std::fs::write(
            segments_dir.join(format!("{}.tmp-abc123", segment_file_name(1, 3))),
            b"partial",
        )
        .unwrap();
        let store = FsFolderStore::new(dir.path());
        let names = store.list_segment_names("desktop-device").await.unwrap();
        assert!(
            names.is_empty(),
            "a *.tmp-* file must never be listed as a finished segment"
        );
    }

    // SYN-031 — a written segment reads back by the name `list_segment_names` returns; a
    // name that would leave the segments directory is refused.
    #[tokio::test]
    async fn read_segment_bytes_reads_back_a_listed_segment_and_refuses_a_path() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_segment("desktop-device", 1, 3, b"sealed".to_vec())
            .await
            .unwrap();
        let names = store.list_segment_names("desktop-device").await.unwrap();
        let bytes = store
            .read_segment_bytes("desktop-device", &names[0])
            .await
            .unwrap();
        assert_eq!(bytes, Some(b"sealed".to_vec()));
        assert_eq!(
            store
                .read_segment_bytes("desktop-device", "missing.bin")
                .await
                .unwrap(),
            None
        );
        assert!(store
            .read_segment_bytes("desktop-device", "../manifest.bin")
            .await
            .is_err());
    }

    // SYN-037 — the manifest is rewritten in place: a second write with a new payload replaces
    // the first, and reading back returns only the latest content.
    #[tokio::test]
    async fn write_manifest_rewrites_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_manifest("desktop-device", b"manifest-v1".to_vec())
            .await
            .unwrap();
        store
            .write_manifest("desktop-device", b"manifest-v2".to_vec())
            .await
            .unwrap();
        let read_back = store
            .read_manifest_bytes("desktop-device")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read_back, b"manifest-v2".to_vec());
    }

    // read_manifest_bytes returns None for a device with no area yet.
    #[tokio::test]
    async fn read_manifest_bytes_returns_none_for_an_unknown_device() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        let result = store.read_manifest_bytes("unknown-device").await.unwrap();
        assert!(result.is_none());
    }

    // SYN-037 — list_device_ids returns every device area present in the folder.
    #[tokio::test]
    async fn list_device_ids_returns_every_published_area() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_manifest("desktop-device", b"manifest".to_vec())
            .await
            .unwrap();
        store
            .write_manifest("laptop-device", b"manifest".to_vec())
            .await
            .unwrap();
        let mut ids = store.list_device_ids().await.unwrap();
        ids.sort();
        assert_eq!(
            ids,
            vec!["desktop-device".to_string(), "laptop-device".to_string()]
        );
    }

    // SYN-082 — removing the manifest takes the device out of the roster but keeps its
    // segments (SYN-036).
    #[tokio::test]
    async fn remove_manifest_keeps_the_segments() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store
            .write_segment("desktop-device", 1, 1, b"sealed".to_vec())
            .await
            .unwrap();
        store
            .write_manifest("desktop-device", b"manifest".to_vec())
            .await
            .unwrap();
        store.remove_manifest("desktop-device").await.unwrap();
        assert!(store
            .read_manifest_bytes("desktop-device")
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            store
                .list_segment_names("desktop-device")
                .await
                .unwrap()
                .len(),
            1
        );
    }

    // SYN-013 rollback — removing the area and the header leaves the folder as it was;
    // removing what is already absent is not an error.
    #[tokio::test]
    async fn remove_device_area_and_header_leave_the_folder_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = FsFolderStore::new(dir.path());
        store.write_header_if_absent(header_bytes()).await.unwrap();
        store
            .write_segment("desktop-device", 1, 1, b"sealed".to_vec())
            .await
            .unwrap();
        store.remove_device_area("desktop-device").await.unwrap();
        store.remove_header().await.unwrap();
        store.remove_header().await.unwrap();
        assert!(store.read_header_bytes().await.unwrap().is_none());
        assert!(store.list_device_ids().await.unwrap().is_empty());
    }
}
