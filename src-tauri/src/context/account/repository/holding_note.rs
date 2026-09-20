use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::str::FromStr;
use std::sync::Arc;

use crate::context::account::domain::{HoldingNote, HoldingNoteRepository, ThresholdDirection};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};

#[derive(sqlx::FromRow)]
struct HoldingNoteRow {
    account_id: String,
    asset_id: String,
    text: String,
    threshold_price: Option<i64>,
    threshold_direction: Option<String>,
}

impl TryFrom<HoldingNoteRow> for HoldingNote {
    type Error = anyhow::Error;

    fn try_from(row: HoldingNoteRow) -> Result<Self> {
        let threshold_direction = row
            .threshold_direction
            .map(|direction| {
                ThresholdDirection::from_str(&direction).map_err(|_| {
                    anyhow::anyhow!("unknown threshold direction in DB: '{direction}'")
                })
            })
            .transpose()?;
        Ok(HoldingNote::from_storage(
            row.account_id,
            row.asset_id,
            row.text,
            row.threshold_price,
            threshold_direction,
        ))
    }
}

/// SQLite-backed implementation of `HoldingNoteRepository`.
pub struct SqliteHoldingNoteRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteHoldingNoteRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            change_recorder: Arc::new(NoopChangeRecorder),
        }
    }

    /// Attaches the change recorder every write appends through (SYN-020).
    pub fn with_change_recorder(mut self, change_recorder: Arc<dyn ChangeRecorder>) -> Self {
        self.change_recorder = change_recorder;
        self
    }
}

fn identity(account_id: &str, asset_id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::HoldingNote, &[account_id, asset_id])
}

/// CFR-011 — the logical timestamp of the note's current state, the `based_on` of the next
/// local change to it; `None` while absent or never ranked.
async fn current_timestamp(
    conn: &mut SqliteConnection,
    account_id: &str,
    asset_id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String"
           FROM holding_notes WHERE account_id = ? AND asset_id = ?"#,
        account_id,
        asset_id
    )
    .fetch_optional(conn)
    .await
    .context("read holding note rank")?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

#[async_trait]
impl HoldingNoteRepository for SqliteHoldingNoteRepository {
    async fn upsert(&self, note: &HoldingNote) -> Result<()> {
        let threshold_direction = note.threshold_direction.map(|d| d.to_string());
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin upsert holding note")?;
        let existing = sqlx::query_scalar!(
            "SELECT account_id FROM holding_notes WHERE account_id = ? AND asset_id = ?",
            note.account_id,
            note.asset_id
        )
        .fetch_optional(&mut *tx)
        .await
        .context("lookup holding note")?;
        let based_on = current_timestamp(&mut tx, &note.account_id, &note.asset_id).await?;
        sqlx::query!(
            r#"INSERT INTO holding_notes (account_id, asset_id, text, threshold_price, threshold_direction)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(account_id, asset_id) DO UPDATE SET
                   text = excluded.text,
                   threshold_price = excluded.threshold_price,
                   threshold_direction = excluded.threshold_direction"#,
            note.account_id,
            note.asset_id,
            note.text,
            note.threshold_price,
            threshold_direction
        )
        .execute(&mut *tx)
        .await
        .context("upsert holding note")?;
        let operation = if existing.is_some() {
            Operation::Updated
        } else {
            Operation::Created
        };
        let draft = ChangeDraft::new(
            RecordKind::HoldingNote,
            identity(&note.account_id, &note.asset_id),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(note)?),
        );
        let rank = self.change_recorder.record(&mut tx, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                r#"UPDATE holding_notes
                   SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                   WHERE account_id = ? AND asset_id = ?"#,
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                note.account_id,
                note.asset_id
            )
            .execute(&mut *tx)
            .await
            .context("stamp holding note rank")?;
        }
        tx.commit().await.context("commit upsert holding note")?;
        Ok(())
    }

    async fn delete(&self, account_id: &str, asset_id: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin delete holding note")?;
        let based_on = current_timestamp(&mut tx, account_id, asset_id).await?;
        let deleted = sqlx::query!(
            r#"DELETE FROM holding_notes WHERE account_id = ? AND asset_id = ?"#,
            account_id,
            asset_id
        )
        .execute(&mut *tx)
        .await
        .context("delete holding note")?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::HoldingNote,
                identity(account_id, asset_id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit().await.context("commit delete holding note")?;
        Ok(())
    }

    async fn get_for_account(&self, account_id: &str) -> Result<Vec<HoldingNote>> {
        let rows = sqlx::query_as!(
            HoldingNoteRow,
            r#"SELECT account_id, asset_id, text, threshold_price, threshold_direction
               FROM holding_notes WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .context("get_for_account holding notes")?;
        rows.into_iter().map(HoldingNote::try_from).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;

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

    async fn seed_sync_device(pool: &Pool<Sqlite>) {
        sqlx::query!(
            r#"INSERT INTO sync_device
               (id, device_id, device_name, folder, joined_at, paused, portfolio_created_at,
                logical_clock, derived_key, data_format_version)
               VALUES (1, 'desktop-device', 'Desktop', '/tmp/sync', '2026-08-22T00:00:00Z', 0,
                       '2026-08-22T00:00:00Z', 0, X'00', 1)"#
        )
        .execute(pool)
        .await
        .expect("seed sync_device");
    }

    async fn seed_account_and_asset(pool: &Pool<Sqlite>) {
        sqlx::query!(
            "INSERT INTO accounts (id, name, bank_name, currency, update_frequency, management_fees_enabled)
             VALUES ('acc-1', 'Test Account', '', 'EUR', 'ManualMonth', 1)"
        )
        .execute(pool)
        .await
        .expect("seed account");
        sqlx::query!(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES ('asset-1', 'Test Asset', 'REF', 'Stocks', 'EUR', 3, 'default-uncategorized', 0)"
        )
        .execute(pool)
        .await
        .expect("seed asset");
    }

    async fn changes_with_operation(pool: &Pool<Sqlite>, operation: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM changes WHERE operation = ?",
            operation
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn note() -> HoldingNote {
        HoldingNote::from_storage(
            "acc-1".to_string(),
            "asset-1".to_string(),
            "watch".to_string(),
            None,
            None,
        )
    }

    // SYN-020/021 — upsert records exactly one Created change, rank-stamped.
    #[tokio::test]
    async fn upsert_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteHoldingNoteRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert(&note()).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp FROM holding_notes WHERE account_id = 'acc-1' AND asset_id = 'asset-1'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            row.sync_logical_timestamp.is_some(),
            "CFR-014: rank columns stamped"
        );
    }

    // SYN-020/024 — delete records exactly one Removed change and leaves a tombstone.
    #[tokio::test]
    async fn delete_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let setup_repo = SqliteHoldingNoteRepository::new(pool.clone());
        setup_repo.upsert(&note()).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteHoldingNoteRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete("acc-1", "asset-1").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'HoldingNote' AND record_identity = 'acc-1:asset-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }

    // SYN-020 — a failed write records no change (rollback).
    #[tokio::test]
    async fn delete_of_a_holding_note_that_was_never_upserted_records_no_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteHoldingNoteRepository::new(pool.clone()).with_change_recorder(recorder);

        // No row exists — nothing to remove, so nothing must be recorded either.
        repo.delete("acc-1", "asset-1").await.unwrap();

        assert_eq!(
            changes_with_operation(&pool, "Removed").await,
            0,
            "SYN-020: a no-op delete records no change"
        );
    }
}
