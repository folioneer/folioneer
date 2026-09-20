use super::super::domain::{AssetPrice, AssetPriceRepository, AssetPriceSource};
use crate::core::logger::BACKEND;
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, SqliteConnection};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

#[derive(sqlx::FromRow)]
struct AssetPriceRow {
    asset_id: String,
    date: String,
    price: i64,
    source: String,
}

/// Restores a stored price source. A value this build does not know — a retired provider —
/// reads as `Manual` and is logged once per distinct value for the life of the process,
/// however many rows and reads carry it.
pub(super) fn source_from_storage(value: &str) -> AssetPriceSource {
    AssetPriceSource::from_str(value).unwrap_or_else(|_| {
        if first_sighting(value) {
            tracing::warn!(
                target: BACKEND,
                value = %value,
                "unknown asset_prices.source value, read as Manual"
            );
        }
        AssetPriceSource::Manual
    })
}

/// Whether `value` is met for the first time in this process.
fn first_sighting(value: &str) -> bool {
    static SIGHTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SIGHTED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(value.to_string())
}

impl From<AssetPriceRow> for AssetPrice {
    fn from(row: AssetPriceRow) -> Self {
        let source = source_from_storage(&row.source);
        AssetPrice::restore(row.asset_id, row.date, row.price, source)
    }
}

/// SQLite implementation of AssetPriceRepository.
pub struct SqliteAssetPriceRepository {
    pool: Pool<Sqlite>,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteAssetPriceRepository {
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

    /// Writes `price` on `conn` (insert or overwrite of its `(asset_id, date)` key) and
    /// records the matching Created / Updated change, rank-stamping the row.
    async fn write_price(&self, conn: &mut SqliteConnection, price: &AssetPrice) -> Result<()> {
        let existing = sqlx::query!(
            "SELECT sync_logical_timestamp FROM asset_prices WHERE asset_id = ? AND date = ?",
            price.asset_id,
            price.date
        )
        .fetch_optional(&mut *conn)
        .await
        .context("Failed to look up asset price")?;
        // CFR-011 — the next change is based on the state this device holds.
        let based_on = existing
            .as_ref()
            .and_then(|row| row.sync_logical_timestamp.as_deref())
            .and_then(LogicalTimestamp::from_wire);
        let source = price.source.to_string();
        sqlx::query!(
            "INSERT INTO asset_prices (asset_id, date, price, source) VALUES (?, ?, ?, ?)
             ON CONFLICT(asset_id, date) DO UPDATE SET price = excluded.price, source = excluded.source",
            price.asset_id,
            price.date,
            price.price,
            source,
        )
        .execute(&mut *conn)
        .await
        .context("Failed to upsert asset price")?;
        let operation = if existing.is_some() {
            Operation::Updated
        } else {
            Operation::Created
        };
        // CFR-016 — a price the application fetched on its own is an application change.
        let origin = if price.source == AssetPriceSource::Manual {
            Origin::User
        } else {
            Origin::Application
        };
        let draft = ChangeDraft::new(
            RecordKind::AssetPrice,
            identity(&price.asset_id, &price.date),
            operation,
            origin,
            based_on,
            Some(serde_json::to_string(price)?),
        );
        let rank = self.change_recorder.record(conn, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                "UPDATE asset_prices SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                 WHERE asset_id = ? AND date = ?",
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                price.asset_id,
                price.date
            )
            .execute(conn)
            .await
            .context("Failed to stamp rank on asset price")?;
        }
        Ok(())
    }

    /// Deletes the `(asset_id, date)` row on `conn` and records its removal; a missing
    /// row records nothing (SYN-020).
    async fn delete_price(
        &self,
        conn: &mut SqliteConnection,
        asset_id: &str,
        date: &str,
    ) -> Result<()> {
        let based_on = sqlx::query_scalar!(
            r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String"
               FROM asset_prices WHERE asset_id = ? AND date = ?"#,
            asset_id,
            date,
        )
        .fetch_optional(&mut *conn)
        .await
        .context("Failed to read asset price rank")?
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp));
        let deleted = sqlx::query!(
            "DELETE FROM asset_prices WHERE asset_id = ? AND date = ?",
            asset_id,
            date,
        )
        .execute(&mut *conn)
        .await
        .context("Failed to delete asset price")?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::AssetPrice,
                identity(asset_id, date),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(conn, draft).await?;
        }
        Ok(())
    }
}

fn identity(asset_id: &str, date: &str) -> RecordIdentity {
    RecordIdentity::canonical(RecordKind::AssetPrice, &[asset_id, date])
}

#[async_trait]
impl AssetPriceRepository for SqliteAssetPriceRepository {
    async fn upsert(&self, price: AssetPrice) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin asset price upsert")?;
        self.write_price(&mut tx, &price).await?;
        tx.commit()
            .await
            .context("Failed to commit asset price upsert")?;
        Ok(())
    }

    async fn get_latest(&self, asset_id: &str) -> Result<Option<AssetPrice>> {
        let row = sqlx::query_as!(
            AssetPriceRow,
            "SELECT asset_id, date, price, source FROM asset_prices WHERE asset_id = ? ORDER BY date DESC LIMIT 1",
            asset_id,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch latest asset price")?;

        Ok(row.map(AssetPrice::from))
    }

    async fn get_all_for_asset(&self, asset_id: &str) -> Result<Vec<AssetPrice>> {
        let rows = sqlx::query_as!(
            AssetPriceRow,
            "SELECT asset_id, date, price, source FROM asset_prices WHERE asset_id = ? ORDER BY date DESC",
            asset_id,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch asset prices")?;

        Ok(rows.into_iter().map(AssetPrice::from).collect())
    }

    async fn get_by_asset_and_date(
        &self,
        asset_id: &str,
        date: &str,
    ) -> Result<Option<AssetPrice>> {
        let row = sqlx::query_as!(
            AssetPriceRow,
            "SELECT asset_id, date, price, source FROM asset_prices WHERE asset_id = ? AND date = ?",
            asset_id,
            date,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch asset price by date")?;

        Ok(row.map(AssetPrice::from))
    }

    async fn delete(&self, asset_id: &str, date: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin asset price delete")?;
        self.delete_price(&mut tx, asset_id, date).await?;
        tx.commit()
            .await
            .context("Failed to commit asset price delete")?;
        Ok(())
    }

    async fn replace_atomic(
        &self,
        asset_id: &str,
        original_date: &str,
        new_price: AssetPrice,
    ) -> Result<()> {
        debug_assert_eq!(
            asset_id, new_price.asset_id,
            "replace_atomic: asset_id parameter must match new_price.asset_id"
        );
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin transaction")?;

        self.delete_price(&mut tx, asset_id, original_date).await?;
        self.write_price(&mut tx, &new_price).await?;

        tx.commit()
            .await
            .context("Failed to commit price replacement")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    // #020 — a retired provider's source reads as Manual, and only its first sighting in a
    // run is worth a log line.
    #[test]
    fn a_retired_provider_source_reads_as_manual_and_is_sighted_once() {
        assert!(first_sighting("RetiredProviderForThisTest"));
        assert!(!first_sighting("RetiredProviderForThisTest"));
        assert_eq!(
            source_from_storage("RetiredProviderForThisTest"),
            AssetPriceSource::Manual
        );
        assert_eq!(source_from_storage("Manual"), AssetPriceSource::Manual);
    }

    /// Inserts a throwaway asset row so FK constraints on asset_prices are satisfied.
    async fn seed_asset(pool: &Pool<Sqlite>, asset_id: &str) {
        sqlx::query!(
            "INSERT INTO assets (id, name, reference, asset_class, currency, risk_level, category_id, is_archived)
             VALUES (?, 'Test Asset', 'REF', 'cash', 'USD', 1, 'default-uncategorized', 0)",
            asset_id,
        )
        .execute(pool)
        .await
        .expect("seed asset");
    }

    // -------------------------------------------------------------------------
    // MKT-100 — source column round-trips through upsert → get_latest / get_all_for_asset
    // / get_by_asset_and_date. These fail until the migration adds the column and
    // the repository methods read/write it.
    // -------------------------------------------------------------------------

    // MKT-100 / MKT-102 — upsert with source=YahooFinance and read back via get_latest
    #[tokio::test]
    async fn upsert_and_get_latest_roundtrip_source_yahoo() {
        use crate::context::asset::AssetPriceSource;
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::YahooFinance,
        ))
        .await
        .unwrap();

        let price = repo.get_latest("asset-1").await.unwrap().unwrap();
        assert_eq!(price.source, AssetPriceSource::YahooFinance);
    }

    // MKT-100 / MKT-101 — upsert with source=Manual and read back via get_by_asset_and_date
    #[tokio::test]
    async fn upsert_and_get_by_date_roundtrip_source_manual() {
        use crate::context::asset::AssetPriceSource;
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            50_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        let price = repo
            .get_by_asset_and_date("asset-1", "2026-01-01")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(price.source, AssetPriceSource::Manual);
    }

    // MKT-100 — get_all_for_asset includes source on every returned row
    #[tokio::test]
    async fn get_all_for_asset_includes_source_field() {
        use crate::context::asset::AssetPriceSource;
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            110_000_000,
            AssetPriceSource::YahooFinance,
        ))
        .await
        .unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 2);
        // Sorted date desc: 2026-01-02 first
        assert_eq!(prices[0].source, AssetPriceSource::YahooFinance);
        assert_eq!(prices[1].source, AssetPriceSource::Manual);
    }

    // MKT-100 — replace_atomic preserves source on the new price row
    #[tokio::test]
    async fn replace_atomic_preserves_source_on_new_row() {
        use crate::context::asset::AssetPriceSource;
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::YahooFinance,
        ))
        .await
        .unwrap();

        let new_price = AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            110_000_000,
            AssetPriceSource::Manual,
        );
        repo.replace_atomic("asset-1", "2026-01-01", new_price)
            .await
            .unwrap();

        let price = repo.get_latest("asset-1").await.unwrap().unwrap();
        assert_eq!(price.source, AssetPriceSource::Manual);
        assert_eq!(price.date, "2026-01-02");
    }

    // -------------------------------------------------------------------------

    // get_all_for_asset — returns all rows for the given asset, sorted date descending (MKT-072)
    #[tokio::test]
    async fn get_all_for_asset_returns_rows_date_descending() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-03".into(),
            130_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            120_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 3);
        assert_eq!(prices[0].date, "2026-01-03");
        assert_eq!(prices[1].date, "2026-01-02");
        assert_eq!(prices[2].date, "2026-01-01");
    }

    // get_all_for_asset — returns empty list when no prices exist for the asset (MKT-072)
    #[tokio::test]
    async fn get_all_for_asset_returns_empty_list_when_none_recorded() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert!(prices.is_empty());
    }

    // get_all_for_asset — does not return rows belonging to a different asset
    #[tokio::test]
    async fn get_all_for_asset_scoped_to_requested_asset() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        seed_asset(&pool, "asset-2").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-2".into(),
            "2026-01-01".into(),
            200_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].asset_id, "asset-1");
    }

    // get_by_asset_and_date — returns the record when it exists (MKT-083)
    #[tokio::test]
    async fn get_by_asset_and_date_returns_record_when_present() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        let result = repo
            .get_by_asset_and_date("asset-1", "2026-01-01")
            .await
            .unwrap();
        assert!(result.is_some());
        let price = result.unwrap();
        assert_eq!(price.asset_id, "asset-1");
        assert_eq!(price.date, "2026-01-01");
        assert_eq!(price.price, 100_000_000);
    }

    // get_by_asset_and_date — returns None when no record for that (asset_id, date) exists
    #[tokio::test]
    async fn get_by_asset_and_date_returns_none_when_absent() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        let result = repo
            .get_by_asset_and_date("asset-1", "2026-01-01")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    // delete — removes the record when it exists (MKT-090)
    #[tokio::test]
    async fn delete_removes_the_record() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            110_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        repo.delete("asset-1", "2026-01-01").await.unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].date, "2026-01-02");
    }

    // delete — is a no-op (does not error) when the record does not exist
    // (presence check is the service's responsibility, not the repo's)
    #[tokio::test]
    async fn delete_is_noop_when_record_absent() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        // Should not error even though no record exists
        let result = repo.delete("asset-1", "2026-01-01").await;
        assert!(result.is_ok());
    }

    // replace_atomic — deletes original_date and upserts at new_date atomically (MKT-084)
    #[tokio::test]
    async fn replace_atomic_moves_price_to_new_date() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        let new_price = AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            110_000_000,
            AssetPriceSource::Manual,
        );
        repo.replace_atomic("asset-1", "2026-01-01", new_price)
            .await
            .unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 1, "old date must be gone");
        assert_eq!(prices[0].date, "2026-01-02");
        assert_eq!(prices[0].price, 110_000_000);
    }

    // replace_atomic — overwrites an existing record at new_date (MKT-084, silent overwrite)
    #[tokio::test]
    async fn replace_atomic_overwrites_existing_record_at_new_date() {
        let pool = setup_pool().await;
        seed_asset(&pool, "asset-1").await;
        let repo = SqliteAssetPriceRepository::new(pool);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-01".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();
        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            105_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        // Move 2026-01-01 to 2026-01-02 — must overwrite 105_000_000
        let new_price = AssetPrice::restore(
            "asset-1".into(),
            "2026-01-02".into(),
            200_000_000,
            AssetPriceSource::Manual,
        );
        repo.replace_atomic("asset-1", "2026-01-01", new_price)
            .await
            .unwrap();

        let prices = repo.get_all_for_asset("asset-1").await.unwrap();
        assert_eq!(prices.len(), 1);
        assert_eq!(prices[0].date, "2026-01-02");
        assert_eq!(prices[0].price, 200_000_000);
    }

    use crate::context::sync::SqliteChangeRecorder;
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

    async fn changes_with_operation(pool: &Pool<Sqlite>, operation: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM changes WHERE operation = ?",
            operation
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    // SYN-020/021 — upsert (creation) records exactly one Created change, rank-stamped.
    #[tokio::test]
    async fn upsert_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_asset(&pool, "asset-1").await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetPriceRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert(AssetPrice::restore(
            "asset-1".into(),
            "2026-08-20".into(),
            100_000_000,
            AssetPriceSource::Manual,
        ))
        .await
        .unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp FROM asset_prices WHERE asset_id = 'asset-1' AND date = '2026-08-20'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
    }

    // SYN-020/024 — delete records exactly one Removed change and a tombstone.
    #[tokio::test]
    async fn delete_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_asset(&pool, "asset-1").await;
        let setup_repo = SqliteAssetPriceRepository::new(pool.clone());
        setup_repo
            .upsert(AssetPrice::restore(
                "asset-1".into(),
                "2026-08-20".into(),
                100_000_000,
                AssetPriceSource::Manual,
            ))
            .await
            .unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetPriceRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete("asset-1", "2026-08-20").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'AssetPrice' AND record_identity = 'asset-1:2026-08-20'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }

    // SYN-021/D1 — replace_atomic (a date correction) emits two changes: a Removed at the
    // old (asset_id, date) identity and a Created at the new one.
    #[tokio::test]
    async fn replace_atomic_records_removed_old_identity_and_created_new_identity() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        seed_asset(&pool, "asset-1").await;
        let setup_repo = SqliteAssetPriceRepository::new(pool.clone());
        setup_repo
            .upsert(AssetPrice::restore(
                "asset-1".into(),
                "2026-08-20".into(),
                100_000_000,
                AssetPriceSource::Manual,
            ))
            .await
            .unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetPriceRepository::new(pool.clone()).with_change_recorder(recorder);
        let new_price = AssetPrice::restore(
            "asset-1".into(),
            "2026-08-21".into(),
            105_000_000,
            AssetPriceSource::Manual,
        );
        repo.replace_atomic("asset-1", "2026-08-20", new_price)
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'AssetPrice' AND record_identity = 'asset-1:2026-08-20'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some(), "the old date's identity is tombstoned");
    }

    // SYN-020 — a failed write records no change (rollback).
    #[tokio::test]
    async fn upsert_of_a_price_for_an_unknown_asset_records_no_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteAssetPriceRepository::new(pool.clone()).with_change_recorder(recorder);

        let result = repo
            .upsert(AssetPrice::restore(
                "unknown-asset".into(),
                "2026-08-20".into(),
                100_000_000,
                AssetPriceSource::Manual,
            ))
            .await;
        assert!(result.is_err(), "FK violation: no such asset");

        assert_eq!(
            changes_with_operation(&pool, "Created").await,
            0,
            "SYN-020: a failed write records no change"
        );
    }
}
