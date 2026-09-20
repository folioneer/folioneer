//! `ChangeRecorder` port (D1, ADR-019): every synced repository write appends a change
//! through this port on the same database connection/transaction as the write, so the
//! record and its change exist together or not at all (SYN-020). The port lives in
//! `infrastructure/` rather than `domain/` because it takes the live connection handle;
//! the sync bounded context implements it (`context::sync::infrastructure::change_log`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::shared::domain::{ChangeDraft, LogicalTimestamp, Rank};

/// Holds the apply gate (SYN-020) until dropped: while it lives, the recorder it came from
/// records nothing and reports not-recording.
pub struct SuspendGuard {
    suspended: Option<Arc<AtomicBool>>,
}

impl SuspendGuard {
    /// The guard of a recorder that is never recording anyway — holds nothing.
    pub fn inert() -> Self {
        Self { suspended: None }
    }

    /// Raises `suspended` until the guard is dropped.
    pub fn holding(suspended: Arc<AtomicBool>) -> Self {
        suspended.store(true, Ordering::SeqCst);
        Self {
            suspended: Some(suspended),
        }
    }
}

impl Drop for SuspendGuard {
    fn drop(&mut self) {
        if let Some(suspended) = &self.suspended {
            suspended.store(false, Ordering::SeqCst);
        }
    }
}

/// The application's own local write was refused before it was recorded (CFR-016): the
/// record's current state — a removal the user made — outranks it. The repository's
/// transaction rolls back with it; the write is not made and produces no change.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("local write of {record_kind} {record_identity} is outranked by the user's state")]
pub struct LocalWriteOutranked {
    /// The kind of the refused record.
    pub record_kind: crate::shared::domain::RecordKind,
    /// Its canonical identity (CFR-012).
    pub record_identity: String,
}

/// Appends one change on the same connection/transaction as the write it describes
/// (SYN-020), allocating the device's next sequence number (SYN-025) and advancing the
/// Lamport logical clock (CFR-010). Returns the `Rank` the record's row must be stamped
/// with (CFR-014), or `None` — the D6 NULL sentinel, `Rank::NEVER` — when nothing was
/// recorded (no `sync_device` row exists yet, or the device is paused, SYN-010/070).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait ChangeRecorder: Send + Sync {
    /// Records `draft`, or reports dormancy via `Ok(None)`. `Err` carrying
    /// `LocalWriteOutranked` when the draft is the application's own write over a state
    /// the user made (CFR-016) — the caller's transaction must roll back.
    async fn record(
        &self,
        conn: &mut sqlx::SqliteConnection,
        draft: ChangeDraft,
    ) -> anyhow::Result<Option<Rank>>;

    /// `false` for the duration of an apply — the gate that satisfies SYN-020's "applying
    /// another device's change never records a change" (`suspend`) — or while sync has
    /// never been enabled or is paused; `true` otherwise.
    async fn is_recording(&self) -> bool;

    /// Holds the apply gate (SYN-020) until the returned guard is dropped: `record` writes
    /// nothing and `is_recording` reports `false` for its lifetime. The `SyncGate` mutex
    /// that keeps a local write and an in-progress apply from interleaving (SYN-064) is
    /// this invariant's other half — the sync run holds both.
    fn suspend(&self) -> SuspendGuard;
}

/// The CFR-014 rank in the TEXT form the three `sync_*` columns store: a repository stamps
/// a row with these right after `ChangeRecorder::record` returned a rank.
pub struct RankColumns {
    /// `sync_logical_timestamp` — the zero-padded 20-character wire form.
    pub logical_timestamp: String,
    /// `sync_origin` — `User` or `Application`.
    pub origin: String,
    /// `sync_device_id` — the device that made the change.
    pub device_id: String,
}

impl From<Rank> for RankColumns {
    fn from(rank: Rank) -> Self {
        Self {
            logical_timestamp: rank.logical_timestamp.as_str().to_string(),
            origin: rank.origin.to_string(),
            device_id: rank.device_id,
        }
    }
}

/// Reads the CFR-014 rank back from the three nullable `sync_*` columns of a row: `None` —
/// the D6 NULL sentinel — when the row has never been ranked or a column is unreadable.
pub fn rank_from_columns(
    logical_timestamp: Option<String>,
    origin: Option<String>,
    device_id: Option<String>,
) -> Option<Rank> {
    Some(Rank {
        logical_timestamp: LogicalTimestamp::from_wire(&logical_timestamp?)?,
        origin: origin?.parse().ok()?,
        device_id: device_id?,
    })
}

/// The recorder every repository is wired with while sync has never been enabled
/// (SYN-010): records nothing, and is never "recording".
pub struct NoopChangeRecorder;

#[async_trait::async_trait]
impl ChangeRecorder for NoopChangeRecorder {
    async fn record(
        &self,
        _conn: &mut sqlx::SqliteConnection,
        _draft: ChangeDraft,
    ) -> anyhow::Result<Option<Rank>> {
        Ok(Rank::NEVER)
    }

    async fn is_recording(&self) -> bool {
        false
    }

    fn suspend(&self) -> SuspendGuard {
        SuspendGuard::inert()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::domain::{Operation, Origin, RecordIdentity, RecordKind};
    use sqlx::sqlite::SqlitePoolOptions;

    // The trait compiles with mockall's automock — a capture-site test expresses "record
    // was called once with this draft" as a hard, checkable expectation.
    #[tokio::test]
    async fn mock_change_recorder_compiles_and_reports_expectations() {
        let mut mock = MockChangeRecorder::new();
        mock.expect_is_recording().returning(|| true);
        assert!(mock.is_recording().await);
    }

    // SYN-010/D6 — a NoopChangeRecorder records nothing and reports the NULL sentinel.
    #[tokio::test]
    async fn noop_change_recorder_records_nothing_and_reports_never_ranked() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        let mut conn = pool.acquire().await.expect("conn");

        let recorder = NoopChangeRecorder;
        assert!(!recorder.is_recording().await);

        let draft = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Created,
            Origin::User,
            None,
            None,
        );
        let rank = recorder
            .record(&mut conn, draft)
            .await
            .expect("noop record never fails");
        assert_eq!(
            rank,
            Rank::NEVER,
            "D6: the noop recorder reports the NULL sentinel"
        );
    }
}
