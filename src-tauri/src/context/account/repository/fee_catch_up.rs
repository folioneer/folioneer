use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::sync::Arc;

use crate::context::account::domain::{FeeCatchUpPosition, FeeCatchUpRepository};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};

#[derive(sqlx::FromRow)]
struct FeeCatchUpPositionRow {
    account_id: String,
    asset_id: String,
    last_applied_period: String,
}

impl From<FeeCatchUpPositionRow> for FeeCatchUpPosition {
    fn from(row: FeeCatchUpPositionRow) -> Self {
        FeeCatchUpPosition {
            account_id: row.account_id,
            asset_id: row.asset_id,
            last_applied_period: row.last_applied_period,
        }
    }
}

/// SQLite-backed implementation of `FeeCatchUpRepository` (D5, CFR-044). Owns the
/// `fee_catch_up_positions` table introduced by migration M3.
pub struct SqliteFeeCatchUpRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteFeeCatchUpRepository {
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

    async fn record(
        &self,
        conn: &mut SqliteConnection,
        position: &FeeCatchUpPosition,
        draft: ChangeDraft,
    ) -> Result<()> {
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                r#"UPDATE fee_catch_up_positions
                   SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                   WHERE account_id = ? AND asset_id = ?"#,
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                position.account_id,
                position.asset_id
            )
            .execute(conn)
            .await
            .context("stamp fee catch-up position rank")?;
        }
        Ok(())
    }
}

fn identity(account_id: &str, asset_id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::FeeCatchUpPosition, &[account_id, asset_id])
}

#[async_trait]
impl FeeCatchUpRepository for SqliteFeeCatchUpRepository {
    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<FeeCatchUpPosition>> {
        let row = sqlx::query_as!(
            FeeCatchUpPositionRow,
            r#"SELECT account_id, asset_id, last_applied_period
               FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?"#,
            account_id,
            asset_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("get_by_account_asset fee catch-up position")?;
        Ok(row.map(FeeCatchUpPosition::from))
    }

    async fn get_by_account(&self, account_id: &str) -> Result<Vec<FeeCatchUpPosition>> {
        let rows = sqlx::query_as!(
            FeeCatchUpPositionRow,
            r#"SELECT account_id, asset_id, last_applied_period
               FROM fee_catch_up_positions WHERE account_id = ?"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .context("get_by_account fee catch-up positions")?;
        Ok(rows.into_iter().map(FeeCatchUpPosition::from).collect())
    }

    async fn upsert(&self, incoming: FeeCatchUpPosition) -> Result<FeeCatchUpPosition> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin upsert fee catch-up position")?;
        let existing = sqlx::query!(
            r#"SELECT last_applied_period, sync_logical_timestamp
               FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?"#,
            incoming.account_id,
            incoming.asset_id
        )
        .fetch_optional(&mut *tx)
        .await
        .context("lookup fee catch-up position")?;
        // CFR-011 — the next change is based on the state this device holds.
        let based_on = existing
            .as_ref()
            .and_then(|row| row.sync_logical_timestamp.as_deref())
            .and_then(LogicalTimestamp::from_wire);
        let existing = existing.map(|row| row.last_applied_period);
        let stored = sqlx::query_as!(
            FeeCatchUpPositionRow,
            r#"INSERT INTO fee_catch_up_positions (account_id, asset_id, last_applied_period)
               VALUES (?, ?, ?)
               ON CONFLICT(account_id, asset_id) DO UPDATE SET
                   last_applied_period = MAX(last_applied_period, excluded.last_applied_period)
               RETURNING account_id, asset_id, last_applied_period"#,
            incoming.account_id,
            incoming.asset_id,
            incoming.last_applied_period
        )
        .fetch_one(&mut *tx)
        .await
        .context("upsert fee catch-up position")?;
        let stored = FeeCatchUpPosition::from(stored);
        let operation = match existing.as_deref() {
            None => Some(Operation::Created),
            Some(previous) if previous != stored.last_applied_period => Some(Operation::Updated),
            Some(_) => None,
        };
        if let Some(operation) = operation {
            // CFR-016/044 — the catch-up position is written only by the application.
            let draft = ChangeDraft::new(
                RecordKind::FeeCatchUpPosition,
                identity(&stored.account_id, &stored.asset_id),
                operation,
                Origin::Application,
                based_on,
                Some(serde_json::to_string(&stored)?),
            );
            self.record(&mut tx, &stored, draft).await?;
        }
        tx.commit()
            .await
            .context("commit upsert fee catch-up position")?;
        Ok(stored)
    }

    async fn delete_by_account_asset(&self, account_id: &str, asset_id: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin delete fee catch-up position")?;
        let based_on = sqlx::query_scalar!(
            r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String"
               FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?"#,
            account_id,
            asset_id
        )
        .fetch_optional(&mut *tx)
        .await
        .context("read fee catch-up position rank")?
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp));
        let deleted = sqlx::query!(
            "DELETE FROM fee_catch_up_positions WHERE account_id = ? AND asset_id = ?",
            account_id,
            asset_id
        )
        .execute(&mut *tx)
        .await
        .context("delete fee catch-up position")?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::FeeCatchUpPosition,
                identity(account_id, asset_id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit()
            .await
            .context("commit delete fee catch-up position")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> Pool<Sqlite> {
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

    /// Seeds an account and an asset so the FK on fee_catch_up_positions is satisfied.
    async fn seed_account_and_asset(pool: &Pool<Sqlite>, account_id: &str, asset_id: &str) {
        sqlx::query!(
            "INSERT INTO accounts (id, name, bank_name, currency, update_frequency, management_fees_enabled)
             VALUES (?, 'Test Account', '', 'EUR', 'ManualMonth', 1)",
            account_id,
        )
        .execute(pool)
        .await
        .expect("seed account");
        sqlx::query!(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES (?, 'Test Asset', 'REF', 'Stocks', 'EUR', 3, 'default-uncategorized', 0)",
            asset_id,
        )
        .execute(pool)
        .await
        .expect("seed asset");
    }

    fn position(account_id: &str, asset_id: &str, period: &str) -> FeeCatchUpPosition {
        FeeCatchUpPosition {
            account_id: account_id.to_string(),
            asset_id: asset_id.to_string(),
            last_applied_period: period.to_string(),
        }
    }

    // FEE-043 — upsert persists a first catch-up position, read back by (account, asset).
    #[tokio::test]
    async fn upsert_persists_first_catch_up_position() {
        let pool = setup_pool().await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let repo = SqliteFeeCatchUpRepository::new(pool);

        repo.upsert(position("acc-1", "asset-1", "2026-07-31"))
            .await
            .unwrap();

        let stored = repo
            .get_by_account_asset("acc-1", "asset-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.last_applied_period, "2026-07-31");
    }

    // CFR-044 — merging an OLDER incoming period never moves the stored cursor backwards.
    #[tokio::test]
    async fn upsert_merges_by_maximum_never_moves_backwards() {
        let pool = setup_pool().await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let repo = SqliteFeeCatchUpRepository::new(pool);

        repo.upsert(position("acc-1", "asset-1", "2026-08-31"))
            .await
            .unwrap();
        // A long-paused device publishes an older position — must not regress August.
        repo.upsert(position("acc-1", "asset-1", "2026-07-31"))
            .await
            .unwrap();

        let stored = repo
            .get_by_account_asset("acc-1", "asset-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.last_applied_period, "2026-08-31",
            "CFR-044: the maximum of stored and incoming prevails, regardless of arrival order"
        );
    }

    // CFR-044 — merging a NEWER incoming period advances the cursor (the symmetric case).
    #[tokio::test]
    async fn upsert_merges_by_maximum_advances_on_newer_incoming() {
        let pool = setup_pool().await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let repo = SqliteFeeCatchUpRepository::new(pool);

        repo.upsert(position("acc-1", "asset-1", "2026-07-31"))
            .await
            .unwrap();
        repo.upsert(position("acc-1", "asset-1", "2026-08-31"))
            .await
            .unwrap();

        let stored = repo
            .get_by_account_asset("acc-1", "asset-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.last_applied_period, "2026-08-31");
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

    // SYN-020/CFR-044 — a first position records Created, a later one Updated, and an older
    // position that leaves the stored cursor unchanged records nothing.
    #[tokio::test]
    async fn upsert_records_a_change_only_when_the_stored_position_moves() {
        let pool = setup_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteFeeCatchUpRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert(position("acc-1", "asset-1", "2026-07-31"))
            .await
            .unwrap();
        repo.upsert(position("acc-1", "asset-1", "2026-08-31"))
            .await
            .unwrap();
        repo.upsert(position("acc-1", "asset-1", "2026-06-30"))
            .await
            .unwrap();

        let operations: Vec<String> =
            sqlx::query_scalar!("SELECT operation FROM changes ORDER BY sequence ASC")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            operations,
            vec!["Created", "Updated"],
            "the no-op re-publish of an older position records nothing"
        );
    }

    // get_by_account_asset — None when generation has never run for this holding.
    #[tokio::test]
    async fn get_by_account_asset_none_when_absent() {
        let pool = setup_pool().await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let repo = SqliteFeeCatchUpRepository::new(pool);

        assert!(repo
            .get_by_account_asset("acc-1", "asset-1")
            .await
            .unwrap()
            .is_none());
    }

    // CFR-016/044 — the catch-up position is written only by the application (never the
    // user); its change must carry `Origin::Application`, not the recorder's current
    // hardcoded `Origin::User`.
    #[tokio::test]
    async fn upsert_records_the_change_with_application_origin() {
        let pool = setup_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool, "acc-1", "asset-1").await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteFeeCatchUpRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert(position("acc-1", "asset-1", "2026-07-31"))
            .await
            .unwrap();

        let origin: String = sqlx::query_scalar(
            "SELECT origin FROM changes WHERE record_kind = 'FeeCatchUpPosition'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            origin, "Application",
            "CFR-016/044: a catch-up position is an application record, never a user one"
        );
    }
}
