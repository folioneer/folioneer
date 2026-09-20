//! `SqliteChangeRecorder` — the SQLite-backed `ChangeRecorder` (D1, PR-A). Dormant when no
//! `sync_device` row exists (SYN-010) or while an apply holds its gate (SYN-020) — a paused
//! device keeps recording (SYN-070), its changes publish on resume (SYN-026/073); otherwise
//! asks the resolution engine whether the local
//! write outranks the record's tombstone (CFR-016), appends to `changes` / `tombstones`,
//! and advances `sync_device.logical_clock` (SYN-025, CFR-010) on the same connection as
//! the write it describes (SYN-020), then tells the recorded-change hook so the
//! settling-interval batcher restarts its window (SYN-067).
//!
//! `SqliteChangeLogRepository` — the SQLite-backed `ChangeLogRepository`: the publish run's
//! reads of the same `changes` table and the enrolment transaction's writes (SYN-013).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use anyhow::Context;
use sqlx::{Pool, Sqlite, SqliteConnection, Transaction};

use crate::context::sync::domain::{
    local_write_allowed, ChangeLogRepository, RecordState, SegmentChange, SyncDevice, Tombstone,
};
use crate::context::sync::error::SyncError;
use crate::core::logger::BACKEND;
use crate::shared::domain::{ChangeDraft, LogicalTimestamp, Operation, Rank, RecordKind};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, LocalWriteOutranked, SuspendGuard,
};

/// The future a recorded-change hook returns.
pub type HookFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Called after every change recorded on an enabled device (SYN-067) — the
/// settling-interval batcher restarts its window from it; a paused device's batcher
/// publishes nothing until resume (SYN-070).
pub type RecordedChangeHook = Arc<dyn Fn() -> HookFuture + Send + Sync>;

/// SQLite-backed `ChangeRecorder`. Reads/writes the `sync_device` singleton (id = 1),
/// `changes`, and `tombstones` tables through the connection handed to `record()` so the
/// change is written in the same transaction as the record it describes (SYN-020).
pub struct SqliteChangeRecorder {
    pool: Pool<Sqlite>,
    suspended: Arc<AtomicBool>,
    on_recorded: OnceLock<RecordedChangeHook>,
}

impl SqliteChangeRecorder {
    /// Creates a recorder backed by the given connection pool.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            suspended: Arc::new(AtomicBool::new(false)),
            on_recorded: OnceLock::new(),
        }
    }

    /// Attaches the hook told after every recorded change (SYN-067) — once; the recorder
    /// is shared before the publishing side that the hook reaches exists.
    pub fn attach_recorded_change_hook(&self, hook: RecordedChangeHook) {
        if self.on_recorded.set(hook).is_err() {
            tracing::warn!(target: BACKEND, "attach_recorded_change_hook: a hook is already attached");
        }
    }

    /// CFR-016 — the record's current state as this recorder knows it: the tombstone a
    /// removal left. A live row is the owning context's — a creation over it fails on its
    /// own key before the recorder is reached, and no application-origin update of a
    /// rank-resolved kind exists.
    async fn current_state(
        conn: &mut SqliteConnection,
        record_kind: &str,
        record_identity: &str,
    ) -> anyhow::Result<Option<RecordState>> {
        let row = sqlx::query_as!(
            TombstoneRow,
            r#"SELECT record_kind, record_identity, logical_timestamp, origin, removed_by
               FROM tombstones WHERE record_kind = ? AND record_identity = ?"#,
            record_kind,
            record_identity
        )
        .fetch_optional(conn)
        .await
        .context("Failed to read the record's tombstone")?;
        Ok(row
            .map(Tombstone::try_from)
            .transpose()
            .context("Failed to read the record's tombstone")?
            .map(|tombstone| RecordState::Tombstone(tombstone.rank())))
    }
}

#[async_trait::async_trait]
impl ChangeRecorder for SqliteChangeRecorder {
    async fn record(
        &self,
        conn: &mut SqliteConnection,
        draft: ChangeDraft,
    ) -> anyhow::Result<Option<Rank>> {
        if self.suspended.load(Ordering::SeqCst) {
            return Ok(Rank::NEVER);
        }
        let device = sqlx::query!("SELECT device_id, logical_clock FROM sync_device WHERE id = 1")
            .fetch_optional(&mut *conn)
            .await
            .context("Failed to read sync_device")?;
        let Some(device) = device else {
            return Ok(Rank::NEVER);
        };

        let sequence = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence), 0) + 1 AS "sequence!: i64" FROM changes WHERE device_id = ?"#,
            device.device_id
        )
        .fetch_one(&mut *conn)
        .await
        .context("Failed to allocate change sequence")?;

        let logical_clock = device.logical_clock + 1;
        let logical_timestamp = LogicalTimestamp::new(logical_clock as u64);
        let record_kind = draft.record_kind.to_string();
        let record_identity = draft.record_identity.as_str().to_string();
        if draft.operation != Operation::Removed {
            let draft_rank = Rank {
                origin: draft.origin,
                logical_timestamp: logical_timestamp.clone(),
                device_id: device.device_id.clone(),
            };
            let current = Self::current_state(conn, &record_kind, &record_identity).await?;
            if !local_write_allowed(draft.record_kind, &draft_rank, current.as_ref()) {
                return Err(LocalWriteOutranked {
                    record_kind: draft.record_kind,
                    record_identity,
                }
                .into());
            }
        }

        sqlx::query!(
            "UPDATE sync_device SET logical_clock = ? WHERE id = 1",
            logical_clock
        )
        .execute(&mut *conn)
        .await
        .context("Failed to advance logical clock")?;

        let based_on = draft
            .based_on
            .as_ref()
            .map(|timestamp| timestamp.as_str().to_string());
        let operation = draft.operation.to_string();
        let origin = draft.origin.to_string();
        let logical_timestamp_text = logical_timestamp.as_str().to_string();
        sqlx::query!(
            r#"INSERT INTO changes
               (device_id, sequence, logical_timestamp, based_on, record_kind, record_identity,
                operation, origin, content, published)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)"#,
            device.device_id,
            sequence,
            logical_timestamp_text,
            based_on,
            record_kind,
            record_identity,
            operation,
            origin,
            draft.content
        )
        .execute(&mut *conn)
        .await
        .context("Failed to insert change")?;

        match draft.operation {
            Operation::Removed => {
                sqlx::query!(
                    r#"INSERT INTO tombstones (record_kind, record_identity, logical_timestamp, origin, removed_by)
                       VALUES (?, ?, ?, ?, ?)
                       ON CONFLICT(record_kind, record_identity) DO UPDATE SET
                           logical_timestamp = excluded.logical_timestamp,
                           origin = excluded.origin,
                           removed_by = excluded.removed_by"#,
                    record_kind,
                    record_identity,
                    logical_timestamp_text,
                    origin,
                    device.device_id
                )
                .execute(&mut *conn)
                .await
                .context("Failed to upsert tombstone")?;
            }
            Operation::Created => {
                sqlx::query!(
                    "DELETE FROM tombstones WHERE record_kind = ? AND record_identity = ?",
                    record_kind,
                    record_identity
                )
                .execute(&mut *conn)
                .await
                .context("Failed to clear tombstone")?;
            }
            Operation::Updated => {}
        }

        if let Some(hook) = self.on_recorded.get() {
            hook().await;
        }

        Ok(Some(Rank {
            origin: draft.origin,
            logical_timestamp,
            device_id: device.device_id,
        }))
    }

    async fn is_recording(&self) -> bool {
        if self.suspended.load(Ordering::SeqCst) {
            return false;
        }
        sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!: i64" FROM sync_device WHERE id = 1"#
        )
        .fetch_one(&self.pool)
        .await
        .map(|count| count > 0)
        .unwrap_or_else(|error| {
            tracing::error!(target: BACKEND, err = ?error, "is_recording: sync_device lookup failed");
            false
        })
    }

    fn suspend(&self) -> SuspendGuard {
        SuspendGuard::holding(Arc::clone(&self.suspended))
    }
}

/// SQLite-backed `ChangeLogRepository`.
pub struct SqliteChangeLogRepository {
    pool: Pool<Sqlite>,
}

impl SqliteChangeLogRepository {
    /// Creates a repository backed by the given connection pool.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ChangeRow {
    sequence: i64,
    logical_timestamp: String,
    based_on: Option<String>,
    record_kind: String,
    record_identity: String,
    operation: String,
    origin: String,
    content: Option<String>,
}

impl TryFrom<ChangeRow> for SegmentChange {
    type Error = SyncError;

    fn try_from(row: ChangeRow) -> Result<Self, SyncError> {
        Ok(SegmentChange {
            sequence: row.sequence,
            logical_timestamp: row.logical_timestamp,
            based_on: row.based_on,
            record_kind: super::parse_stored("changes", "record_kind", &row.record_kind)?,
            record_identity: row.record_identity,
            operation: super::parse_stored("changes", "operation", &row.operation)?,
            origin: super::parse_stored("changes", "origin", &row.origin)?,
            content: row.content,
        })
    }
}

#[derive(sqlx::FromRow)]
struct TombstoneRow {
    record_kind: String,
    record_identity: String,
    logical_timestamp: String,
    origin: String,
    removed_by: String,
}

impl TryFrom<TombstoneRow> for Tombstone {
    type Error = SyncError;

    fn try_from(row: TombstoneRow) -> Result<Self, SyncError> {
        let logical_timestamp =
            LogicalTimestamp::from_wire(&row.logical_timestamp).ok_or_else(|| {
                tracing::error!(target: BACKEND, value = %row.logical_timestamp, "tombstones: malformed logical timestamp");
                SyncError::DatabaseError
            })?;
        Ok(Tombstone {
            record_kind: super::parse_stored("changes", "record_kind", &row.record_kind)?,
            record_identity: row.record_identity,
            logical_timestamp,
            origin: super::parse_stored("changes", "origin", &row.origin)?,
            removed_by: row.removed_by,
        })
    }
}

#[async_trait::async_trait]
impl ChangeLogRepository for SqliteChangeLogRepository {
    async fn tombstone(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<Tombstone>, SyncError> {
        let record_kind = kind.to_string();
        let row = sqlx::query_as!(
            TombstoneRow,
            r#"SELECT record_kind, record_identity, logical_timestamp, origin, removed_by
               FROM tombstones WHERE record_kind = ? AND record_identity = ?"#,
            record_kind,
            identity
        )
        .fetch_optional(conn)
        .await
        .map_err(|error| SyncError::database("tombstone: query failed", error))?;
        row.map(Tombstone::try_from).transpose()
    }

    async fn upsert_tombstone(
        &self,
        conn: &mut SqliteConnection,
        tombstone: &Tombstone,
    ) -> Result<(), SyncError> {
        let record_kind = tombstone.record_kind.to_string();
        let logical_timestamp = tombstone.logical_timestamp.as_str().to_string();
        let origin = tombstone.origin.to_string();
        sqlx::query!(
            r#"INSERT INTO tombstones (record_kind, record_identity, logical_timestamp, origin, removed_by)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(record_kind, record_identity) DO UPDATE SET
                   logical_timestamp = excluded.logical_timestamp,
                   origin = excluded.origin,
                   removed_by = excluded.removed_by"#,
            record_kind,
            tombstone.record_identity,
            logical_timestamp,
            origin,
            tombstone.removed_by
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("upsert_tombstone: write failed", error))?;
        Ok(())
    }

    async fn clear_tombstone(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<(), SyncError> {
        let record_kind = kind.to_string();
        sqlx::query!(
            "DELETE FROM tombstones WHERE record_kind = ? AND record_identity = ?",
            record_kind,
            identity
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("clear_tombstone: delete failed", error))?;
        Ok(())
    }

    async fn advance_logical_clock(
        &self,
        conn: &mut SqliteConnection,
        at_least: i64,
    ) -> Result<(), SyncError> {
        sqlx::query!(
            "UPDATE sync_device SET logical_clock = MAX(logical_clock, ?) WHERE id = 1",
            at_least
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("advance_logical_clock: update failed", error))?;
        Ok(())
    }

    async fn kept_key_bytes(&self) -> Result<Option<Vec<u8>>, SyncError> {
        sqlx::query_scalar!(
            r#"SELECT derived_key AS "derived_key!: Vec<u8>" FROM sync_device WHERE id = 1"#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| SyncError::database("kept_key_bytes: query failed", error))
    }

    async fn logical_clock(&self) -> Result<i64, SyncError> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(logical_clock), 0) AS "logical_clock!: i64" FROM sync_device WHERE id = 1"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| SyncError::database("logical_clock: query failed", error))
    }

    async fn list_unpublished(&self, device_id: &str) -> Result<Vec<SegmentChange>, SyncError> {
        let rows = sqlx::query_as!(
            ChangeRow,
            r#"SELECT sequence, logical_timestamp, based_on, record_kind, record_identity,
                      operation, origin, content
               FROM changes WHERE device_id = ? AND published = 0
               ORDER BY sequence ASC"#,
            device_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| SyncError::database("list_unpublished: query failed", error))?;
        rows.into_iter().map(SegmentChange::try_from).collect()
    }

    async fn mark_published(
        &self,
        device_id: &str,
        first_sequence: i64,
        last_sequence: i64,
    ) -> Result<(), SyncError> {
        sqlx::query!(
            "UPDATE changes SET published = 1 WHERE device_id = ? AND sequence BETWEEN ? AND ?",
            device_id,
            first_sequence,
            last_sequence
        )
        .execute(&self.pool)
        .await
        .map_err(|error| SyncError::database("mark_published: update failed", error))?;
        Ok(())
    }

    async fn latest_published_sequence(&self, device_id: &str) -> Result<i64, SyncError> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence), 0) AS "sequence!: i64"
               FROM changes WHERE device_id = ? AND published = 1"#,
            device_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|error| SyncError::database("latest_published_sequence: query failed", error))
    }

    async fn begin(&self) -> Result<Transaction<'static, Sqlite>, SyncError> {
        self.pool
            .begin()
            .await
            .map_err(|error| SyncError::database("begin: transaction not opened", error))
    }

    async fn save_enrolment(
        &self,
        conn: &mut SqliteConnection,
        device: &SyncDevice,
        derived_key: &[u8],
        logical_clock: i64,
    ) -> Result<(), SyncError> {
        let data_format_version = i64::from(device.data_format_version);
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, ?, ?, ?, ?, 0, ?, ?, ?, ?)
               ON CONFLICT(id) DO UPDATE SET
                   device_id = excluded.device_id,
                   device_name = excluded.device_name,
                   folder = excluded.folder,
                   joined_at = excluded.joined_at,
                   paused = excluded.paused,
                   portfolio_created_at = excluded.portfolio_created_at,
                   logical_clock = excluded.logical_clock,
                   derived_key = excluded.derived_key,
                   data_format_version = excluded.data_format_version"#,
            device.device_id,
            device.device_name,
            device.folder,
            device.joined_at,
            device.portfolio_created_at,
            logical_clock,
            derived_key,
            data_format_version
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("save_enrolment: write failed", error))?;
        Ok(())
    }

    async fn retire_earlier_changes(
        &self,
        conn: &mut SqliteConnection,
        device_id: &str,
    ) -> Result<(), SyncError> {
        sqlx::query!(
            "UPDATE changes SET published = 1 WHERE device_id = ?",
            device_id
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("retire_earlier_changes: update failed", error))?;
        Ok(())
    }

    async fn next_sequence(
        &self,
        conn: &mut SqliteConnection,
        device_id: &str,
    ) -> Result<i64, SyncError> {
        sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(sequence), 0) + 1 AS "sequence!: i64" FROM changes WHERE device_id = ?"#,
            device_id
        )
        .fetch_one(conn)
        .await
        .map_err(|error| SyncError::database("next_sequence: query failed", error))
    }

    async fn append_published_change(
        &self,
        conn: &mut SqliteConnection,
        device_id: &str,
        change: &SegmentChange,
    ) -> Result<(), SyncError> {
        let record_kind = change.record_kind.to_string();
        let operation = change.operation.to_string();
        let origin = change.origin.to_string();
        sqlx::query!(
            r#"INSERT INTO changes
               (device_id, sequence, logical_timestamp, based_on, record_kind, record_identity,
                operation, origin, content, published)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)"#,
            device_id,
            change.sequence,
            change.logical_timestamp,
            change.based_on,
            record_kind,
            change.record_identity,
            operation,
            origin,
            change.content
        )
        .execute(conn)
        .await
        .map_err(|error| SyncError::database("append_published_change: insert failed", error))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::application::publisher::{Publisher, SETTLING_INTERVAL};
    use crate::shared::domain::{Operation, Origin, RecordIdentity, RecordKind};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    async fn make_pool() -> Pool<Sqlite> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    async fn seed_sync_device(pool: &Pool<Sqlite>, device_id: &str) {
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, ?, 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#,
            device_id,
        )
        .execute(pool)
        .await
        .expect("seed sync_device");
    }

    fn account_created_draft(account_id: &str) -> ChangeDraft {
        ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &[account_id]),
            Operation::Created,
            Origin::User,
            None,
            Some(format!("{{\"id\":\"{account_id}\"}}")),
        )
    }

    // SYN-010 — dormant when no sync_device row exists: records nothing, returns the NEVER
    // sentinel.
    #[tokio::test]
    async fn record_is_dormant_and_returns_never_when_no_sync_device_row_exists() {
        let pool = make_pool().await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let rank = recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("dormant record must not error");
        assert_eq!(rank, Rank::NEVER);
        drop(conn);

        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM changes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "SYN-010: nothing is recorded while dormant");
    }

    // SYN-025/CFR-010 — with a sync_device row present, three successive records for this
    // device allocate sequence 1, 2, 3 and a strictly increasing logical timestamp each time.
    #[tokio::test]
    async fn record_allocates_sequence_and_advances_logical_timestamp_per_device() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let rank_1 = recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("record 1")
            .expect("sync_device row exists: must be ranked, not NEVER");
        let rank_2 = recorder
            .record(&mut conn, account_created_draft("account-2"))
            .await
            .expect("record 2")
            .expect("sync_device row exists: must be ranked, not NEVER");
        let rank_3 = recorder
            .record(&mut conn, account_created_draft("account-3"))
            .await
            .expect("record 3")
            .expect("sync_device row exists: must be ranked, not NEVER");

        assert!(rank_2.logical_timestamp > rank_1.logical_timestamp);
        assert!(rank_3.logical_timestamp > rank_2.logical_timestamp);
        drop(conn);

        let sequences: Vec<i64> = sqlx::query_scalar!(
            "SELECT sequence FROM changes WHERE device_id = ? ORDER BY sequence ASC",
            "desktop-device"
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            sequences,
            vec![1, 2, 3],
            "SYN-025: strictly increasing, never reused"
        );
    }

    // CFR-016 — the recorder stores whatever origin the draft carries: an Application-origin
    // draft (a generated fee deduction, an auto-fetched price) is stamped `origin = Application`,
    // not silently promoted to User.
    #[tokio::test]
    async fn record_stores_application_origin_verbatim() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let draft = ChangeDraft::new(
            RecordKind::AssetPrice,
            RecordIdentity::canonical(RecordKind::AssetPrice, &["asset-1", "2026-08-20"]),
            Operation::Created,
            Origin::Application,
            None,
            Some("{\"price\":100000000}".to_string()),
        );
        let rank = recorder
            .record(&mut conn, draft)
            .await
            .unwrap()
            .expect("sync_device row exists: must be ranked");
        assert_eq!(rank.origin, Origin::Application);
        drop(conn);

        let row = sqlx::query!("SELECT origin FROM changes WHERE device_id = 'desktop-device'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(row.origin, "Application");
    }

    // CFR-014 — a recorded change writes exactly one `changes` row carrying the draft's
    // operation, origin, record_kind, record_identity, and content.
    #[tokio::test]
    async fn record_writes_one_changes_row_with_the_drafts_fields() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .unwrap();
        drop(conn);

        let row = sqlx::query!(
            "SELECT operation, origin, record_kind, record_identity, content FROM changes WHERE device_id = ?",
            "desktop-device"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.operation, "Created");
        assert_eq!(row.origin, "User");
        assert_eq!(row.record_kind, "Account");
        assert_eq!(row.record_identity, "account-1");
        assert_eq!(row.content.as_deref(), Some("{\"id\":\"account-1\"}"));
    }

    // CFR-015/SYN-024 — a Removed change leaves a permanent tombstone.
    #[tokio::test]
    async fn record_writes_a_tombstone_on_removed_operation() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let draft = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Removed,
            Origin::User,
            None,
            None,
        );
        recorder.record(&mut conn, draft).await.unwrap();
        drop(conn);

        let tombstone = sqlx::query!(
            "SELECT record_kind, record_identity, removed_by FROM tombstones WHERE record_kind = 'Account' AND record_identity = 'account-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some(), "CFR-015: a removal leaves a tombstone");
        assert_eq!(tombstone.unwrap().removed_by, "desktop-device");
    }

    // CFR-015/CFR-022 — a later Created for the same identity removes the prior tombstone
    // (a re-creation supersedes the removal it followed).
    #[tokio::test]
    async fn record_removes_prior_tombstone_on_later_created() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let removed = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Removed,
            Origin::User,
            None,
            None,
        );
        recorder.record(&mut conn, removed).await.unwrap();
        recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .unwrap();
        drop(conn);

        let tombstone = sqlx::query!(
            "SELECT record_kind FROM tombstones WHERE record_kind = 'Account' AND record_identity = 'account-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(
            tombstone.is_none(),
            "a later Created removes the prior tombstone for the same identity"
        );
    }

    // CFR-016 — the application's own creation over a tombstone the user left is refused
    // with a typed error the caller's transaction rolls back on: no changes row, no clock
    // advance, and the user's tombstone stands.
    #[tokio::test]
    async fn application_write_over_a_user_tombstone_is_refused_and_rolls_back() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");
        let user_removal = ChangeDraft::new(
            RecordKind::Transaction,
            RecordIdentity::canonical(RecordKind::Transaction, &["fee-deduction-1"]),
            Operation::Removed,
            Origin::User,
            None,
            None,
        );
        recorder.record(&mut conn, user_removal).await.unwrap();
        drop(conn);

        let mut transaction = pool.begin().await.expect("transaction");
        let regeneration = ChangeDraft::new(
            RecordKind::Transaction,
            RecordIdentity::canonical(RecordKind::Transaction, &["fee-deduction-1"]),
            Operation::Created,
            Origin::Application,
            None,
            Some("{\"id\":\"fee-deduction-1\"}".to_string()),
        );
        let refused = recorder
            .record(&mut transaction, regeneration)
            .await
            .expect_err("CFR-016: the regeneration must be refused");
        assert_eq!(
            refused.downcast_ref::<LocalWriteOutranked>(),
            Some(&LocalWriteOutranked {
                record_kind: RecordKind::Transaction,
                record_identity: "fee-deduction-1".into(),
            }),
            "the refusal must be the typed error the repository rolls back on: {refused:?}"
        );
        drop(transaction);

        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM changes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "only the user's removal is recorded");
        let clock: i64 = sqlx::query_scalar("SELECT logical_clock FROM sync_device WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(clock, 1, "a refused write advances nothing");
        let tombstone_origin: Option<String> = sqlx::query_scalar(
            "SELECT origin FROM tombstones WHERE record_kind = 'Transaction' AND record_identity = 'fee-deduction-1'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(
            tombstone_origin.as_deref(),
            Some("User"),
            "the user's tombstone stands"
        );
    }

    // CFR-016/CFR-020 — a user write is never refused: the user's own re-creation over the
    // user's tombstone records and clears the tombstone.
    #[tokio::test]
    async fn user_write_over_a_tombstone_is_never_refused() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");
        let removal = ChangeDraft::new(
            RecordKind::Account,
            RecordIdentity::canonical(RecordKind::Account, &["account-1"]),
            Operation::Removed,
            Origin::User,
            None,
            None,
        );
        recorder.record(&mut conn, removal).await.unwrap();

        let rank = recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("a user write is never refused")
            .expect("sync_device row exists: must be ranked");
        assert_eq!(rank.origin, Origin::User);
    }

    // CFR-050 — an observation never consults origin: the application's auto-fetched price
    // records over a price the user removed.
    #[tokio::test]
    async fn observation_write_over_a_user_tombstone_is_never_refused() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");
        let identity =
            RecordIdentity::canonical(RecordKind::AssetPrice, &["asset-1", "2026-08-20"]);
        let user_removal = ChangeDraft::new(
            RecordKind::AssetPrice,
            identity.clone(),
            Operation::Removed,
            Origin::User,
            None,
            None,
        );
        recorder.record(&mut conn, user_removal).await.unwrap();

        let fetched = ChangeDraft::new(
            RecordKind::AssetPrice,
            identity,
            Operation::Created,
            Origin::Application,
            None,
            Some("{\"price\":100000000}".to_string()),
        );
        let rank = recorder
            .record(&mut conn, fetched)
            .await
            .expect("CFR-050: an observation is never refused on origin")
            .expect("sync_device row exists: must be ranked");
        assert_eq!(rank.origin, Origin::Application);
    }

    // SYN-020 — the apply gate: is_recording() is false while an apply is in progress
    // (`suspend()`'s guard), and true again once the guard is dropped.
    #[tokio::test]
    async fn is_recording_false_while_apply_gate_held() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());

        assert!(
            recorder.is_recording().await,
            "recording before any gate is held"
        );
        let guard = recorder.suspend();
        assert!(
            !recorder.is_recording().await,
            "SYN-020: gate held, must not record"
        );
        drop(guard);
        assert!(
            recorder.is_recording().await,
            "gate released, recording resumes"
        );
    }

    // SYN-020 — while the apply gate is held, record() writes nothing and reports the NEVER
    // sentinel; once the guard drops, the same recorder records again.
    #[tokio::test]
    async fn record_writes_nothing_while_suspended_and_resumes_after_guard_drops() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");

        let guard = recorder.suspend();
        let rank = recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("suspended record must not error");
        assert_eq!(
            rank,
            Rank::NEVER,
            "SYN-020: nothing is recorded during an apply"
        );
        drop(guard);

        let rank = recorder
            .record(&mut conn, account_created_draft("account-2"))
            .await
            .expect("record after resume");
        assert!(
            rank.is_some(),
            "recording resumes once the gate is released"
        );
        drop(conn);

        let count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM changes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "only the post-resume record left a changes row");
    }

    // SYN-067 — a recorded change restarts the settling window through the hook: exactly one
    // publish fires once the window elapses, and none before. The clock is paused only once
    // the database work is done — the pool's own timeouts must not be auto-advanced.
    #[tokio::test]
    async fn recorded_change_publishes_once_after_the_settling_window() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let published = Arc::new(AtomicUsize::new(0));
        let published_for_hook = Arc::clone(&published);
        let hook = Arc::new(Publisher::new()).recorded_change_hook(move || {
            published_for_hook.fetch_add(1, Ordering::SeqCst);
            async {}
        });
        let recorder = SqliteChangeRecorder::new(pool.clone());
        recorder.attach_recorded_change_hook(hook);
        let mut conn = pool.acquire().await.expect("conn");

        recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("record")
            .expect("sync_device row exists: must be ranked");
        drop(conn);
        tokio::time::pause();
        assert_eq!(
            published.load(Ordering::SeqCst),
            0,
            "nothing publishes before the settling window elapses"
        );

        tokio::time::advance(SETTLING_INTERVAL + Duration::from_millis(1)).await;
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            published.load(Ordering::SeqCst),
            1,
            "SYN-067: the recorded change publishes once the settling window elapses"
        );
        tokio::time::advance(SETTLING_INTERVAL).await;
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            published.load(Ordering::SeqCst),
            1,
            "a burst publishes exactly once"
        );
    }

    // SYN-010 — a dormant recorder (no sync_device row) never tells the hook.
    #[tokio::test]
    async fn dormant_record_does_not_call_the_hook() {
        let pool = make_pool().await;
        let called = Arc::new(AtomicUsize::new(0));
        let called_for_hook = Arc::clone(&called);
        let hook: RecordedChangeHook = Arc::new(move || {
            called_for_hook.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {})
        });
        let recorder = SqliteChangeRecorder::new(pool.clone());
        recorder.attach_recorded_change_hook(hook);
        let mut conn = pool.acquire().await.expect("conn");

        recorder
            .record(&mut conn, account_created_draft("account-1"))
            .await
            .expect("dormant record must not error");
        assert_eq!(called.load(Ordering::SeqCst), 0);
    }

    // SYN-060 — the repository lists only unpublished changes, in sequence order, and marks
    // a range published.
    #[tokio::test]
    async fn change_log_repository_lists_unpublished_and_marks_published() {
        let pool = make_pool().await;
        seed_sync_device(&pool, "desktop-device").await;
        let recorder = SqliteChangeRecorder::new(pool.clone());
        let mut conn = pool.acquire().await.expect("conn");
        for account_id in ["account-1", "account-2", "account-3"] {
            recorder
                .record(&mut conn, account_created_draft(account_id))
                .await
                .expect("record");
        }
        drop(conn);
        let repository = SqliteChangeLogRepository::new(pool.clone());

        repository
            .mark_published("desktop-device", 1, 1)
            .await
            .expect("mark published");
        let unpublished = repository
            .list_unpublished("desktop-device")
            .await
            .expect("list");
        let sequences: Vec<i64> = unpublished.iter().map(|change| change.sequence).collect();
        assert_eq!(sequences, vec![2, 3]);
        assert_eq!(
            repository
                .latest_published_sequence("desktop-device")
                .await
                .expect("latest"),
            1
        );
        assert_eq!(repository.logical_clock().await.expect("clock"), 3);
    }

    // SYN-070/026 — a paused device keeps recording: its changes publish on resume.
    #[tokio::test]
    async fn is_recording_true_when_device_is_paused() {
        let pool = make_pool().await;
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, 'desktop-device', 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 1,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#,
        )
        .execute(&pool)
        .await
        .expect("seed paused sync_device");
        let recorder = SqliteChangeRecorder::new(pool.clone());

        assert!(recorder.is_recording().await);
        let mut conn = pool.acquire().await.expect("conn");
        let rank = recorder
            .record(&mut conn, account_created_draft("acc-paused"))
            .await
            .expect("record")
            .expect("a paused device still records");
        assert_eq!(rank.logical_timestamp, LogicalTimestamp::new(1));
    }
}
