//! Domain layer of the sync bounded context (B0/B38): the resolution engine
//! (`resolution.rs`), the device aggregate, the folder/manifest/segment value objects, the
//! ports the application layer writes through, and the wire shapes assembled into
//! `SyncStatus` (D2).

/// The `ChangeApplier` port — the owning contexts' verbatim reads and writes (CFR-017).
pub mod applier;
/// The `ChangeLogRepository` port — the change log and the enrolment-owned device state.
pub mod change_log;
/// Undismissed-notice persistence shape (SYN-066, CFR-060).
pub mod conflict_notice;
/// How far this device has taken in another device's changes (SYN-033).
pub mod cursor;
/// `SyncDevice` aggregate + the `SyncStateRepository` port (SYN-016/018/070/072/084).
pub mod device;
/// Folder/header/manifest/segment value objects + the `FolderStore` port (D2/D8).
pub mod folder;
/// A received change waiting for a record this device has not received yet (SYN-041).
pub mod held_back;
/// The `RankStamper` port — ranks the rows that existed before sync did (CFR-014, D6).
pub mod rank_stamper;
/// The shape a received change must have before the engine sees it (SYN-034, CFR-012).
pub mod received_change;
/// ⭐ The resolution engine — every CFR rule, nothing else (ADR-019, D4).
pub mod resolution;
/// The `PortfolioSnapshot` port — the whole current portfolio as `Created` changes (SYN-013).
pub mod snapshot;
/// `SyncStatus` / `SyncReport` wire shapes (SYN-063).
pub mod status;
/// What a removal leaves behind (CFR-015).
pub mod tombstone;

pub use applier::ChangeApplier;
pub use change_log::ChangeLogRepository;
pub use conflict_notice::{ConflictNotice, ConflictNoticeKind};
pub use cursor::SyncCursor;
pub use device::{ensure_device_name, StoredDevice, SyncDevice, SyncStateRepository};
pub use folder::{
    segment_file_name, segment_sequence_range, DerivationParameters, FolderHeader, FolderProblem,
    FolderStore, Manifest, Segment, SegmentChange, SyncFolderState, WriteHeaderOutcome,
    APP_VERSION,
};
pub use held_back::HeldBackChange;
pub use rank_stamper::RankStamper;
pub use received_change::{check_received_change, MalformedChange};
pub use resolution::{
    account_parent, cascade_child_tombstones, collision_notice, decide, display_name,
    duplicate_name_notice, local_write_allowed, notice_for, parent_references, reference_outcome,
    removed_child_notice, replay_order, resolve, resolve_observation, upgraded_content, Change,
    Concurrency, Decision, NoticeDraft, Outcome, RecordState, WaitingFor,
};
pub use snapshot::{PortfolioRecord, PortfolioSnapshot};
pub use status::{
    HoldingInconsistency, InconsistentHolding, RosterEntry, SyncFailure, SyncReport, SyncStatus,
};
pub use tombstone::Tombstone;

#[cfg(test)]
pub use applier::MockChangeApplier;
#[cfg(test)]
pub use device::MockSyncStateRepository;
#[cfg(test)]
pub use folder::MockFolderStore;
#[cfg(test)]
pub use rank_stamper::MockRankStamper;
#[cfg(test)]
pub use snapshot::MockPortfolioSnapshot;
