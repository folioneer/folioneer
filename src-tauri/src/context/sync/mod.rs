//! Multi-device sync bounded context (SYN + CFR, ADR-019): the change log, the device
//! aggregate, folder/crypto/codec infrastructure, the first-device publish path, the
//! resolution engine, the full run (publish, read, resolve, apply), and the join rebuild (D2).

/// External API and Tauri command handlers (boundary, BC root per B39) — the four BC-local
/// commands (D3): `pause_sync`, `leave_sync`, `rename_sync_device`, `dismiss_conflict_notice`.
pub mod api;
/// Application layer (gold layout, B0/B38): device lifecycle, the sync run and its apply
/// executor, the join rebuild, the settling-interval batcher, and enrolling as the first
/// device.
pub mod application;
/// Domain layer (gold layout, B0/B38): the resolution engine, the device aggregate,
/// folder/manifest/segment value objects, the ports, and the wire shapes assembled into
/// `SyncStatus`.
pub mod domain;
/// Flat BC error enum (`SyncError`).
pub mod error;
/// Infrastructure layer — `SqliteChangeRecorder`, crypto, codec, folder store, and the
/// SQLite-backed change-log and sync state repositories.
pub mod infrastructure;

pub use api::*;
pub use application::{FirstPublish, JoinError, Publisher, SyncGate, SyncRun, SyncService};
pub use domain::{
    cascade_child_tombstones, collision_notice, duplicate_name_notice, ensure_device_name,
    local_write_allowed, notice_for, reference_outcome, resolve, resolve_observation,
    segment_file_name, segment_sequence_range, Change, ChangeApplier, ChangeLogRepository,
    Concurrency, ConflictNotice, ConflictNoticeKind, DerivationParameters, FolderHeader,
    FolderProblem, FolderStore, HeldBackChange, HoldingInconsistency, InconsistentHolding,
    Manifest, NoticeDraft, Outcome, PortfolioRecord, PortfolioSnapshot, RankStamper, RecordState,
    RosterEntry, Segment, SegmentChange, StoredDevice, SyncCursor, SyncDevice, SyncFailure,
    SyncFolderState, SyncReport, SyncStateRepository, SyncStatus, Tombstone, WaitingFor,
    WriteHeaderOutcome, APP_VERSION,
};
pub use error::SyncError;
pub use infrastructure::codec::{header_data_format_version, DATA_FORMAT_VERSION};
pub use infrastructure::crypto::ensure_passphrase_length;
pub use infrastructure::{
    FsFolderStore, RecordedChangeHook, SqliteChangeLogRepository, SqliteChangeRecorder,
    SqliteSyncStateRepository,
};

#[cfg(test)]
pub use domain::{
    MockChangeApplier, MockFolderStore, MockPortfolioSnapshot, MockRankStamper,
    MockSyncStateRepository,
};
