//! Folder value objects + the `FolderStore` port (D2, D8): why a folder cannot be used
//! (`FolderProblem`), the one readable file (`FolderHeader`), and the two published-but-sealed
//! files (`Manifest`, `Segment`). `FolderStore` deals in already-sealed bytes — sealing and
//! parsing the plaintext shapes is `infrastructure::codec`'s job (D2); this port only owns
//! whole-file visibility and per-device area layout (SYN-030/031/032/037).

use crate::context::sync::error::SyncError;
use crate::shared::domain::{Operation, Origin, RecordKind};
use serde::{Deserialize, Serialize};
use specta::Type;

/// Why a folder cannot be used (SYN-019/069). Structured so the frontend can translate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum FolderProblem {
    /// The folder does not exist.
    Missing,
    /// The path exists but is not a directory.
    NotADirectory,
    /// The process cannot read/write the folder.
    PermissionDenied,
    /// The folder's volume is not mounted.
    Unmounted,
    /// The volume has no space left.
    OutOfSpace,
    /// Catch-all: a mid-write failure, a failed whole-file rename (SYN-032), or an encryption
    /// failure during publish.
    IoFailure,
}

/// Pre-flight read of a candidate folder (SYN-011/014/019). Never rejects — every condition is
/// reported in the returned state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SyncFolderState {
    /// `None` when the folder exists and is writable; the problem otherwise.
    pub problem: Option<FolderProblem>,
    /// Whether a folder header is present.
    pub holds_portfolio: bool,
    /// The header's data format version, when `holds_portfolio` is true.
    pub data_format_version: Option<u32>,
    /// `false` → joining would raise `UpdateRequired` (SYN-035).
    pub format_readable: bool,
    /// `true` → joining would raise `InstallationHoldsUserData` (SYN-014).
    pub installation_holds_user_data: bool,
}

/// The public inputs every device combines with the passphrase to derive the same key
/// (SYN-051).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DerivationParameters {
    /// Random salt, unique per portfolio.
    pub salt: Vec<u8>,
    /// Argon2id memory cost, in KiB.
    pub memory_cost_kib: u32,
    /// Argon2id iteration count.
    pub iterations: u32,
    /// Argon2id parallelism (lanes).
    pub parallelism: u32,
}

/// The one readable file in the folder (SYN-050): what every device needs to derive the key
/// and check the passphrase before anything else (SYN-051/055). Contains no secret.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct FolderHeader {
    /// Public key-derivation inputs (SYN-051).
    pub derivation_parameters: DerivationParameters,
    /// Encrypted marker decrypting correctly only with the right passphrase (SYN-055).
    pub passphrase_check: Vec<u8>,
    /// The data format of the device that created the portfolio (SYN-035).
    pub data_format_version: u32,
    /// The creating device's logical timestamp at creation — decides between two headers
    /// written by two devices enabling into the same empty folder offline (SYN-081).
    pub created_at: String,
    /// The identity of the device that created the portfolio (SYN-084 follows this).
    pub created_by_device_id: String,
}

/// The version of the application this build belongs to, as published in its manifest (SYN-037).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A device's published identity card (SYN-037), the only published file rewritten in place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Manifest {
    /// The publishing device.
    pub device_id: String,
    /// Its current name.
    pub device_name: String,
    /// The data format of the application that last published from this device (SYN-035).
    pub data_format_version: u32,
    /// The version of the application that last published from this device — information
    /// only, it gates nothing (SYN-037). `None` for a manifest written before the field existed.
    #[serde(default)]
    pub app_version: Option<String>,
    /// The last change this device has published (SYN-037).
    pub latest_sequence: i64,
}

/// One change carried inside a `Segment` — the sealed payload's per-change shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SegmentChange {
    /// This device's own strictly increasing position (SYN-025).
    pub sequence: i64,
    /// The ordering value (CFR-010).
    pub logical_timestamp: String,
    /// The record state this change was made against; absent for a creation (CFR-011).
    pub based_on: Option<String>,
    /// What kind of record changed (SYN-021).
    pub record_kind: RecordKind,
    /// Which record changed (CFR-012).
    pub record_identity: String,
    /// Created, Updated, or Removed.
    pub operation: Operation,
    /// Who made the change (CFR-016).
    pub origin: Origin,
    /// The record's full state after the change, JSON-encoded; absent for a removal.
    pub content: Option<String>,
}

/// A published file carrying a consecutive batch of one device's changes (SYN-031).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct Segment {
    /// The publishing device.
    pub device_id: String,
    /// The first change in the batch.
    pub first_sequence: i64,
    /// The last change in the batch.
    pub last_sequence: i64,
    /// The data format the changes are expressed in (SYN-035).
    pub data_format_version: u32,
    /// The batch itself, in ascending sequence order.
    pub changes: Vec<SegmentChange>,
}

/// The file name of a segment carrying `first_sequence..=last_sequence` (SYN-031, D8):
/// `seg-<first20>-<last20>.bin`, zero-padded so filesystem order is sequence order.
pub fn segment_file_name(first_sequence: i64, last_sequence: i64) -> String {
    format!("seg-{first_sequence:020}-{last_sequence:020}.bin")
}

/// The sequence range a segment file name carries, or `None` for a name that is not a
/// segment's.
pub fn segment_sequence_range(name: &str) -> Option<(i64, i64)> {
    let range = name.strip_prefix("seg-")?.strip_suffix(".bin")?;
    let (first, last) = range.split_once('-')?;
    Some((first.parse().ok()?, last.parse().ok()?))
}

/// The result of a write-if-absent header publish (SYN-081).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteHeaderOutcome {
    /// No header existed; this device's header is now the folder's header.
    Written,
    /// A header already existed; nothing was written.
    AlreadyExists,
}

/// Whole-file folder access, area layout, and atomic (temp-then-rename) publishing
/// (SYN-030/031/032/037). Deals only in already-sealed bytes — sealing/parsing the plaintext
/// shapes is `infrastructure::codec`'s job.
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait FolderStore: Send + Sync {
    /// Points the store at `folder`. Every entry point names the folder it intends — the
    /// enrolled device's folder for a run, a candidate folder for a pre-flight read.
    fn retarget(&self, folder: &str);

    /// Whether the folder can be read and written right now (SYN-019/069).
    async fn check_available(&self) -> Result<(), FolderProblem>;

    /// Reads the header's raw bytes, or `None` when no header exists yet.
    async fn read_header_bytes(&self) -> Result<Option<Vec<u8>>, SyncError>;

    /// Writes the header only if none exists yet (SYN-081) — the last-moment re-check before
    /// a first device publishes.
    async fn write_header_if_absent(&self, bytes: Vec<u8>)
        -> Result<WriteHeaderOutcome, SyncError>;

    /// Publishes one segment into `device_id`'s area, named by its sequence range (SYN-031).
    /// Written whole-or-nothing via temp-then-rename (SYN-032); never rewritten afterward.
    async fn write_segment(
        &self,
        device_id: &str,
        first_sequence: i64,
        last_sequence: i64,
        bytes: Vec<u8>,
    ) -> Result<(), SyncError>;

    /// Rewrites `device_id`'s manifest in place (SYN-037), whole-or-nothing.
    async fn write_manifest(&self, device_id: &str, bytes: Vec<u8>) -> Result<(), SyncError>;

    /// Reads `device_id`'s manifest bytes, or `None` when it has no area yet.
    async fn read_manifest_bytes(&self, device_id: &str) -> Result<Option<Vec<u8>>, SyncError>;

    /// Lists `device_id`'s published segment file names, in filesystem order, ignoring any
    /// `*.tmp-*` file left by an interrupted write (SYN-032).
    async fn list_segment_names(&self, device_id: &str) -> Result<Vec<String>, SyncError>;

    /// Reads one of `device_id`'s published segments by the name `list_segment_names`
    /// returned, or `None` when it is gone.
    async fn read_segment_bytes(
        &self,
        device_id: &str,
        name: &str,
    ) -> Result<Option<Vec<u8>>, SyncError>;

    /// Lists every device area present in the folder (the roster's file-system half, SYN-037).
    async fn list_device_ids(&self) -> Result<Vec<String>, SyncError>;

    /// Removes `device_id`'s manifest — the device leaves the roster (SYN-082); its segments
    /// stay.
    async fn remove_manifest(&self, device_id: &str) -> Result<(), SyncError>;

    /// Removes `device_id`'s whole area — rolling back a failed first publish (SYN-013) or
    /// clearing the folder for a start-over (SYN-071).
    async fn remove_device_area(&self, device_id: &str) -> Result<(), SyncError>;

    /// Removes the folder header — rolling back a failed first publish (SYN-013) or clearing
    /// the folder for a start-over (SYN-071).
    async fn remove_header(&self) -> Result<(), SyncError>;
}
