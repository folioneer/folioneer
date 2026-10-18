use crate::context::currency::domain::{PortfolioSetting, PortfolioSettingRepository};
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, Rank, RecordIdentity, RecordKind,
    SyncedRecord,
};
use crate::shared::infrastructure::change_recorder::{
    rank_from_columns, ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{SqliteConnection, SqlitePool};
use std::sync::Arc;

/// SQLite-backed implementation of `PortfolioSettingRepository`.
pub struct SqlitePortfolioSettingRepository {
    pool: SqlitePool,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqlitePortfolioSettingRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
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

#[async_trait]
impl PortfolioSettingRepository for SqlitePortfolioSettingRepository {
    async fn get(&self, name: &str) -> Result<Option<String>> {
        sqlx::query_scalar!("SELECT value FROM portfolio_settings WHERE name = ?", name)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("Failed to read portfolio setting {name}"))
    }

    async fn set(&self, setting: &PortfolioSetting) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin portfolio setting write")?;
        let stored = sqlx::query!(
            "SELECT value, sync_logical_timestamp FROM portfolio_settings WHERE name = ?",
            setting.name
        )
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to read portfolio setting before writing it")?;
        if stored
            .as_ref()
            .is_some_and(|row| row.value == setting.value)
        {
            return Ok(());
        }
        sqlx::query!(
            r#"INSERT INTO portfolio_settings (name, value) VALUES (?, ?)
               ON CONFLICT(name) DO UPDATE SET value = excluded.value"#,
            setting.name,
            setting.value
        )
        .execute(&mut *tx)
        .await
        .context("Failed to write portfolio setting")?;
        let (operation, based_on) = match &stored {
            Some(row) => (
                Operation::Updated,
                row.sync_logical_timestamp
                    .as_deref()
                    .and_then(LogicalTimestamp::from_wire),
            ),
            None => (Operation::Created, None),
        };
        let draft = ChangeDraft::new(
            RecordKind::PortfolioSetting,
            RecordIdentity::canonical(RecordKind::PortfolioSetting, &[&setting.name]),
            operation,
            Origin::User,
            based_on,
            Some(serde_json::to_string(setting)?),
        );
        if let Some(rank) = self.change_recorder.record(&mut tx, draft).await? {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                "UPDATE portfolio_settings SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                 WHERE name = ?",
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                setting.name
            )
            .execute(&mut *tx)
            .await
            .context("Failed to stamp rank on portfolio setting")?;
        }
        tx.commit()
            .await
            .context("Failed to commit portfolio setting write")
    }

    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64> {
        let columns = RankColumns::from(rank.clone());
        let stamped = sqlx::query!(
            "UPDATE portfolio_settings SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to stamp unranked portfolio settings")?;
        Ok(stamped.rows_affected())
    }

    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>> {
        if kind != RecordKind::PortfolioSetting {
            return Ok(None);
        }
        let row = sqlx::query!(
            r#"SELECT name, value, sync_logical_timestamp, sync_origin, sync_device_id
               FROM portfolio_settings WHERE name = ?"#,
            identity
        )
        .fetch_optional(conn)
        .await
        .with_context(|| format!("Failed to read synced portfolio setting {identity}"))?;
        row.map(|row| {
            let setting = PortfolioSetting {
                name: row.name,
                value: row.value,
            };
            Ok(SyncedRecord {
                rank: rank_from_columns(
                    row.sync_logical_timestamp,
                    row.sync_origin,
                    row.sync_device_id,
                ),
                content: serde_json::to_string(&setting)?,
            })
        })
        .transpose()
    }

    async fn apply(
        &self,
        conn: &mut SqliteConnection,
        setting: &PortfolioSetting,
        rank: &Rank,
    ) -> Result<()> {
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO portfolio_settings (name, value, sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(name) DO UPDATE SET
                   value = excluded.value,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            setting.name,
            setting.value,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply portfolio setting")?;
        Ok(())
    }

    async fn remove_synced(&self, conn: &mut SqliteConnection, identity: &str) -> Result<()> {
        sqlx::query!("DELETE FROM portfolio_settings WHERE name = ?", identity)
            .execute(conn)
            .await
            .with_context(|| format!("Failed to remove portfolio setting {identity}"))?;
        Ok(())
    }

    async fn discard(&self, conn: &mut SqliteConnection) -> Result<()> {
        sqlx::query!("DELETE FROM portfolio_settings")
            .execute(conn)
            .await
            .context("Failed to discard portfolio settings")?;
        Ok(())
    }

    async fn list(&self) -> Result<Vec<PortfolioSetting>> {
        let rows = sqlx::query!("SELECT name, value FROM portfolio_settings ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .context("Failed to list portfolio settings")?;
        Ok(rows
            .into_iter()
            .map(|row| PortfolioSetting {
                name: row.name,
                value: row.value,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::SqliteChangeRecorder;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> SqlitePool {
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

    async fn seed_sync_device(pool: &SqlitePool) {
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

    async fn operations(pool: &SqlitePool) -> Vec<String> {
        sqlx::query_scalar!(
            "SELECT operation FROM changes WHERE record_kind = 'PortfolioSetting' ORDER BY rowid"
        )
        .fetch_all(pool)
        .await
        .unwrap()
    }

    fn reference(value: &str) -> PortfolioSetting {
        PortfolioSetting {
            name: "reference_currency".to_string(),
            value: value.to_string(),
        }
    }

    // SYN-020/021 — a setting written records a Created change, a new value an Updated one,
    // the same value nothing; the value is read back.
    #[tokio::test]
    async fn syn_021_a_setting_records_its_changes_and_reads_back() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo =
            SqlitePortfolioSettingRepository::new(pool.clone()).with_change_recorder(recorder);

        assert_eq!(repo.get("reference_currency").await.unwrap(), None);
        repo.set(&reference("CHF")).await.unwrap();
        repo.set(&reference("CHF")).await.unwrap();
        repo.set(&reference("USD")).await.unwrap();

        assert_eq!(
            repo.get("reference_currency").await.unwrap().as_deref(),
            Some("USD")
        );
        assert_eq!(operations(&pool).await, ["Created", "Updated"]);
        assert_eq!(repo.list().await.unwrap(), vec![reference("USD")]);
    }

    // CFR-014/017 — an applied setting is held with its rank and read back as synced; a
    // removal drops it.
    #[tokio::test]
    async fn cfr_017_an_applied_setting_is_read_back_then_removed() {
        let pool = make_pool().await;
        let repo = SqlitePortfolioSettingRepository::new(pool.clone());
        let mut conn = pool.acquire().await.unwrap();
        let rank = Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(7),
            device_id: "laptop".to_string(),
        };

        repo.apply(&mut conn, &reference("GBP"), &rank)
            .await
            .unwrap();
        let held = repo
            .synced_record(
                &mut conn,
                RecordKind::PortfolioSetting,
                "reference_currency",
            )
            .await
            .unwrap()
            .expect("held");
        assert_eq!(held.rank, Some(rank));
        let content: PortfolioSetting = serde_json::from_str(&held.content).unwrap();
        assert_eq!(content, reference("GBP"));

        repo.remove_synced(&mut conn, "reference_currency")
            .await
            .unwrap();
        assert!(repo
            .synced_record(
                &mut conn,
                RecordKind::PortfolioSetting,
                "reference_currency"
            )
            .await
            .unwrap()
            .is_none());
    }
}
