use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::str::FromStr;
use std::sync::Arc;

use crate::context::account::domain::{FeeFrequency, FeeSchedule, FeeScheduleRepository};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};

#[derive(sqlx::FromRow)]
struct FeeScheduleRow {
    id: String,
    account_id: String,
    asset_id: String,
    annual_rate_micros: i64,
    frequency: String,
    start_date: String,
    end_date: Option<String>,
    active: i64,
    last_applied_period: Option<String>,
}

impl TryFrom<FeeScheduleRow> for FeeSchedule {
    type Error = anyhow::Error;

    fn try_from(row: FeeScheduleRow) -> Result<Self> {
        let frequency = FeeFrequency::from_str(&row.frequency)
            .map_err(|_| anyhow::anyhow!("unknown fee frequency in DB: '{}'", row.frequency))?;
        Ok(FeeSchedule::restore(
            row.id,
            row.account_id,
            row.asset_id,
            row.annual_rate_micros,
            frequency,
            row.start_date,
            row.end_date,
            row.active != 0,
            row.last_applied_period,
        ))
    }
}

/// SQLite-backed implementation of `FeeScheduleRepository`. `last_applied_period` is read
/// from the schedule's `fee_catch_up_positions` row (CFR-044); the schedule's own columns
/// never carry it.
pub struct SqliteFeeScheduleRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteFeeScheduleRepository {
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
        schedule_id: &str,
        draft: ChangeDraft,
    ) -> Result<()> {
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                r#"UPDATE fee_schedules
                   SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                   WHERE id = ?"#,
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                schedule_id
            )
            .execute(conn)
            .await
            .context("stamp fee schedule rank")?;
        }
        Ok(())
    }
}

fn identity(account_id: &str, asset_id: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::FeeSchedule, &[account_id, asset_id])
}

/// CFR-011 — the logical timestamp of the schedule's current state, the `based_on` of the
/// next local change to it; `None` while absent or never ranked.
async fn current_timestamp(
    conn: &mut SqliteConnection,
    account_id: &str,
    asset_id: &str,
) -> Result<Option<LogicalTimestamp>> {
    let stored = sqlx::query_scalar!(
        r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String"
           FROM fee_schedules WHERE account_id = ? AND asset_id = ?"#,
        account_id,
        asset_id
    )
    .fetch_optional(conn)
    .await
    .context("read fee schedule rank")?;
    Ok(stored
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp)))
}

/// The schedule's own state as change content: `last_applied_period` is the derived read
/// of its `FeeCatchUpPosition` (CFR-044), never part of the schedule record.
pub(super) fn schedule_content(schedule: &FeeSchedule) -> Result<String> {
    let mut value = serde_json::to_value(schedule)?;
    if let Some(fields) = value.as_object_mut() {
        fields.remove("last_applied_period");
    }
    Ok(value.to_string())
}

#[async_trait]
impl FeeScheduleRepository for SqliteFeeScheduleRepository {
    async fn get_by_account_asset(
        &self,
        account_id: &str,
        asset_id: &str,
    ) -> Result<Option<FeeSchedule>> {
        let row = sqlx::query_as!(
            FeeScheduleRow,
            r#"SELECT s.id, s.account_id, s.asset_id, s.annual_rate_micros, s.frequency, s.start_date, s.end_date, s.active,
                      p.last_applied_period AS "last_applied_period?: String"
               FROM fee_schedules s
               LEFT JOIN fee_catch_up_positions p ON p.account_id = s.account_id AND p.asset_id = s.asset_id
               WHERE s.account_id = ? AND s.asset_id = ?"#,
            account_id,
            asset_id
        )
        .fetch_optional(&self.pool)
        .await
        .context("get_by_account_asset fee schedule")?;
        row.map(FeeSchedule::try_from).transpose()
    }

    async fn get_all_active(&self) -> Result<Vec<FeeSchedule>> {
        let rows = sqlx::query_as!(
            FeeScheduleRow,
            r#"SELECT s.id, s.account_id, s.asset_id, s.annual_rate_micros, s.frequency, s.start_date, s.end_date, s.active,
                      p.last_applied_period AS "last_applied_period?: String"
               FROM fee_schedules s
               LEFT JOIN fee_catch_up_positions p ON p.account_id = s.account_id AND p.asset_id = s.asset_id
               WHERE s.active = 1"#
        )
        .fetch_all(&self.pool)
        .await
        .context("get_all_active fee schedules")?;
        rows.into_iter().map(FeeSchedule::try_from).collect()
    }

    async fn get_active_by_account(&self, account_id: &str) -> Result<Vec<FeeSchedule>> {
        let rows = sqlx::query_as!(
            FeeScheduleRow,
            r#"SELECT s.id, s.account_id, s.asset_id, s.annual_rate_micros, s.frequency, s.start_date, s.end_date, s.active,
                      p.last_applied_period AS "last_applied_period?: String"
               FROM fee_schedules s
               LEFT JOIN fee_catch_up_positions p ON p.account_id = s.account_id AND p.asset_id = s.asset_id
               WHERE s.active = 1 AND s.account_id = ?"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .context("get_active_by_account fee schedules")?;
        rows.into_iter().map(FeeSchedule::try_from).collect()
    }

    async fn get_by_account(&self, account_id: &str) -> Result<Vec<FeeSchedule>> {
        let rows = sqlx::query_as!(
            FeeScheduleRow,
            r#"SELECT s.id, s.account_id, s.asset_id, s.annual_rate_micros, s.frequency, s.start_date, s.end_date, s.active,
                      p.last_applied_period AS "last_applied_period?: String"
               FROM fee_schedules s
               LEFT JOIN fee_catch_up_positions p ON p.account_id = s.account_id AND p.asset_id = s.asset_id
               WHERE s.account_id = ?"#,
            account_id
        )
        .fetch_all(&self.pool)
        .await
        .context("get_by_account fee schedules")?;
        rows.into_iter().map(FeeSchedule::try_from).collect()
    }

    async fn insert(&self, schedule: &FeeSchedule) -> Result<()> {
        let frequency = schedule.frequency.to_string();
        let active = schedule.active as i64;
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin insert fee schedule")?;
        sqlx::query!(
            r#"INSERT INTO fee_schedules (id, account_id, asset_id, annual_rate_micros, frequency, start_date, end_date, active)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            schedule.id,
            schedule.account_id,
            schedule.asset_id,
            schedule.annual_rate_percent_micros,
            frequency,
            schedule.start_date,
            schedule.end_date,
            active
        )
        .execute(&mut *tx)
        .await
        .context("insert fee schedule")?;
        let draft = ChangeDraft::new(
            RecordKind::FeeSchedule,
            identity(&schedule.account_id, &schedule.asset_id),
            Operation::Created,
            Origin::User,
            None,
            Some(schedule_content(schedule)?),
        );
        self.record(&mut tx, &schedule.id, draft).await?;
        tx.commit().await.context("commit insert fee schedule")?;
        Ok(())
    }

    async fn update(&self, schedule: &FeeSchedule) -> Result<()> {
        let frequency = schedule.frequency.to_string();
        let active = schedule.active as i64;
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin update fee schedule")?;
        let based_on = current_timestamp(&mut tx, &schedule.account_id, &schedule.asset_id).await?;
        let written = sqlx::query!(
            r#"UPDATE fee_schedules
               SET annual_rate_micros = ?, frequency = ?, start_date = ?, end_date = ?, active = ?
               WHERE id = ?"#,
            schedule.annual_rate_percent_micros,
            frequency,
            schedule.start_date,
            schedule.end_date,
            active,
            schedule.id
        )
        .execute(&mut *tx)
        .await
        .context("update fee schedule")?;
        if written.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::FeeSchedule,
                identity(&schedule.account_id, &schedule.asset_id),
                Operation::Updated,
                Origin::User,
                based_on,
                Some(schedule_content(schedule)?),
            );
            self.record(&mut tx, &schedule.id, draft).await?;
        }
        tx.commit().await.context("commit update fee schedule")?;
        Ok(())
    }

    async fn delete_by_account_asset(&self, account_id: &str, asset_id: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("begin delete fee schedule")?;
        let based_on = current_timestamp(&mut tx, account_id, asset_id).await?;
        let deleted = sqlx::query!(
            r#"DELETE FROM fee_schedules WHERE account_id = ? AND asset_id = ?"#,
            account_id,
            asset_id
        )
        .execute(&mut *tx)
        .await
        .context("delete fee schedule")?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::FeeSchedule,
                identity(account_id, asset_id),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit().await.context("commit delete fee schedule")?;
        Ok(())
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

    fn schedule() -> FeeSchedule {
        FeeSchedule::new(
            "acc-1".to_string(),
            "asset-1".to_string(),
            1_000_000,
            FeeFrequency::Monthly,
            "2026-01-01".to_string(),
            None,
        )
        .unwrap()
    }

    // SYN-020/021 — insert records exactly one Created change, rank-stamped, origin User.
    #[tokio::test]
    async fn insert_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteFeeScheduleRepository::new(pool.clone()).with_change_recorder(recorder);

        let schedule = schedule();
        repo.insert(&schedule).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp, sync_origin FROM fee_schedules WHERE id = ?",
            schedule.id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
        assert_eq!(row.sync_origin.as_deref(), Some("User"));
        let content: String = sqlx::query_scalar!(
            r#"SELECT content AS "content!: String" FROM changes WHERE operation = 'Created'"#
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            !content.contains("last_applied_period"),
            "CFR-044: the derived catch-up cursor is not part of the schedule's content"
        );
    }

    // SYN-020 — update records exactly one Updated change.
    #[tokio::test]
    async fn update_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let setup_repo = SqliteFeeScheduleRepository::new(pool.clone());
        let schedule = schedule();
        setup_repo.insert(&schedule).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteFeeScheduleRepository::new(pool.clone()).with_change_recorder(recorder);
        let updated = schedule.clone().update_from(2_000_000, None, true).unwrap();
        repo.update(&updated).await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020/024 — delete_by_account_asset records exactly one Removed change and a tombstone.
    #[tokio::test]
    async fn delete_by_account_asset_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_account_and_asset(&pool).await;
        let setup_repo = SqliteFeeScheduleRepository::new(pool.clone());
        setup_repo.insert(&schedule()).await.unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteFeeScheduleRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete_by_account_asset("acc-1", "asset-1")
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'FeeSchedule' AND record_identity = 'acc-1:asset-1'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }
}
