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

/// The data format version from which a non-cash quantity is in hundred-millionths (SYN-038).
const HUNDRED_MILLIONTHS_SINCE: u32 = 14;

impl SegmentChange {
    /// SYN-038 — the change as the current data format writes it: a transaction written
    /// before version 14 has its non-cash quantity multiplied by 100. Its split's factor, its
    /// dividend's amount and what moves on the cash line stay in millionths. Content that is
    /// not a transaction's JSON is left for validation (SYN-034); a quantity that would not
    /// fit stays as it came and is refused by the same validation.
    pub fn in_current_format(mut self, written_in: u32) -> Self {
        if written_in >= HUNDRED_MILLIONTHS_SINCE || self.record_kind != RecordKind::Transaction {
            return self;
        }
        let Some(mut content) = self
            .content
            .as_deref()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok())
        else {
            return self;
        };
        let text = |field: &str| content.get(field).and_then(|value| value.as_str());
        let keeps_its_scale = matches!(text("transaction_type"), Some("Split" | "Dividend"))
            || text("asset_id").is_some_and(crate::core::cash::is_cash_asset);
        let scaled = content
            .get("quantity")
            .and_then(|quantity| quantity.as_i64())
            .map(|quantity| quantity.saturating_mul(100));
        if let (false, Some(scaled)) = (keeps_its_scale, scaled) {
            if let Some(quantity) = content.get_mut("quantity") {
                *quantity = scaled.into();
            }
            self.content = Some(content.to_string());
        }
        self
    }
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

    /// Reads the copies of the header a cloud client left beside it — a conflict copy,
    /// a numbered duplicate — in name order (SYN-084). The header itself is not among them.
    async fn read_header_copies(&self) -> Result<Vec<Vec<u8>>, SyncError>;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction(asset_id: &str, kind: &str, quantity: i64) -> SegmentChange {
        SegmentChange {
            sequence: 1,
            logical_timestamp: "00000000000000000001".to_string(),
            based_on: None,
            record_kind: RecordKind::Transaction,
            record_identity: "tx-1".to_string(),
            operation: Operation::Created,
            origin: Origin::User,
            content: Some(format!(
                r#"{{"id":"tx-1","asset_id":"{asset_id}","transaction_type":"{kind}","quantity":{quantity},"note":"kept"}}"#
            )),
        }
    }

    fn quantity(change: &SegmentChange) -> i64 {
        let content = change.content.as_deref().expect("content");
        let value: serde_json::Value = serde_json::from_str(content).expect("json");
        assert_eq!(value["note"], "kept");
        value["quantity"].as_i64().expect("quantity")
    }

    // SYN-038 — a transaction from an older format reads in hundred-millionths.
    #[test]
    fn syn_038_an_older_quantity_is_multiplied_on_read() {
        let read = transaction("asset-1", "Purchase", 1_234_567).in_current_format(13);
        assert_eq!(quantity(&read), 123_456_700);
    }

    // SYN-038 — version 14 and later already hold the new form.
    #[test]
    fn syn_038_a_current_quantity_is_left_as_written() {
        let read = transaction("asset-1", "Purchase", 7).in_current_format(14);
        assert_eq!(quantity(&read), 7);
    }

    // TRX-024 — what is not a non-cash quantity stays in its own scale.
    #[test]
    fn syn_038_cash_a_split_and_a_dividend_stay_as_written() {
        for (asset_id, kind) in [
            ("system-cash-eur", "Deposit"),
            ("asset-1", "Split"),
            ("asset-1", "Dividend"),
        ] {
            let read = transaction(asset_id, kind, 7).in_current_format(13);
            assert_eq!(quantity(&read), 7, "{kind}");
        }
    }

    // SYN-034 — a quantity past what fits saturates, past the bound validation refuses.
    #[test]
    fn syn_034_an_older_quantity_that_would_not_fit_is_refused_by_validation() {
        let read = transaction("asset-1", "Purchase", i64::MAX / 2).in_current_format(13);
        assert_eq!(quantity(&read), i64::MAX);
        assert!(matches!(
            crate::context::sync::domain::resolution::Change::from_segment_change("device", read),
            Err(crate::context::sync::domain::MalformedChange::AmountOutOfBounds { .. })
        ));
    }

    // SYN-034 — other kinds of record, and content that does not parse, are left for
    // validation.
    #[test]
    fn syn_038_what_is_not_a_transaction_is_left_as_written() {
        let mut asset = transaction("asset-1", "Purchase", 7);
        asset.record_kind = RecordKind::Asset;
        assert_eq!(quantity(&asset.in_current_format(13)), 7);

        let mut broken = transaction("asset-1", "Purchase", 7);
        broken.content = Some("not json".to_string());
        let read = broken.in_current_format(13);
        assert_eq!(read.content.as_deref(), Some("not json"));
    }
}
