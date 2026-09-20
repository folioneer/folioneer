//! `Tombstone` (CFR-015): what a removal leaves behind — written by `SqliteChangeRecorder`
//! for a local removal and by the apply executor for an applied one, read by the executor as
//! the record's current state.

use crate::shared::domain::{LogicalTimestamp, Origin, Rank, RecordKind};

/// What a removal leaves behind (CFR-015): stands in for the removed record when a later or
/// earlier change to it arrives. Permanent — never pruned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tombstone {
    /// The kind of the removed record.
    pub record_kind: RecordKind,
    /// Its identity (CFR-012).
    pub record_identity: String,
    /// The removal's logical timestamp.
    pub logical_timestamp: LogicalTimestamp,
    /// Whether the user or the application removed it (CFR-016).
    pub origin: Origin,
    /// The device that removed it, named in conflict notices.
    pub removed_by: String,
}

impl Tombstone {
    /// The rank the removal carries (CFR-020).
    pub fn rank(&self) -> Rank {
        Rank {
            origin: self.origin,
            logical_timestamp: self.logical_timestamp.clone(),
            device_id: self.removed_by.clone(),
        }
    }
}
