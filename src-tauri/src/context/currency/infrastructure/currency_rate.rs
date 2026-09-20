use crate::context::currency::domain::{CurrencyRate, CurrencyRateRepository, CurrencyRateSource};
use crate::core::logger::BACKEND;
use crate::shared::domain::{
    ChangeDraft, LogicalTimestamp, Operation, Origin, RecordIdentity, RecordKind,
};
use crate::shared::infrastructure::change_recorder::{
    ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::SqlitePool;
use std::str::FromStr;
use std::sync::Arc;

/// SQLite-backed implementation of `CurrencyRateRepository`.
pub struct SqliteCurrencyRateRepository {
    pool: SqlitePool,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteCurrencyRateRepository {
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

fn identity(from_currency: &str, to_currency: &str, date: &str) -> RecordIdentity {
    RecordIdentity::canonical(
        RecordKind::CurrencyRate,
        &[from_currency, to_currency, date],
    )
}

#[derive(sqlx::FromRow)]
struct RateRow {
    from_currency: String,
    to_currency: String,
    date: String,
    rate: i64,
    source: String,
}

impl From<RateRow> for CurrencyRate {
    fn from(row: RateRow) -> Self {
        let source = CurrencyRateSource::from_str(&row.source).unwrap_or_else(|_| {
            tracing::warn!(
                target: BACKEND,
                value = %row.source,
                "unknown currency_rates.source value, falling back to Manual"
            );
            CurrencyRateSource::Manual
        });
        CurrencyRate::from_storage(
            row.from_currency,
            row.to_currency,
            row.date,
            row.rate,
            source,
        )
    }
}

#[async_trait]
impl CurrencyRateRepository for SqliteCurrencyRateRepository {
    async fn upsert_rate(&self, rate: CurrencyRate) -> Result<CurrencyRate> {
        let source = rate.source.to_string();
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin currency rate upsert")?;
        let existing = sqlx::query!(
            "SELECT sync_logical_timestamp FROM currency_rates WHERE from_currency = ? AND to_currency = ? AND date = ?",
            rate.from_currency,
            rate.to_currency,
            rate.date
        )
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to look up currency rate")?;
        // CFR-011 — the next change is based on the state this device holds.
        let based_on = existing
            .as_ref()
            .and_then(|row| row.sync_logical_timestamp.as_deref())
            .and_then(LogicalTimestamp::from_wire);
        sqlx::query!(
            "INSERT INTO currency_rates (from_currency, to_currency, date, rate, source)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(from_currency, to_currency, date)
             DO UPDATE SET rate = excluded.rate, source = excluded.source",
            rate.from_currency,
            rate.to_currency,
            rate.date,
            rate.rate,
            source,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to upsert currency rate")?;
        let operation = if existing.is_some() {
            Operation::Updated
        } else {
            Operation::Created
        };
        // CFR-016 — a rate the application fetched on its own is an application change.
        let origin = if rate.source == CurrencyRateSource::Manual {
            Origin::User
        } else {
            Origin::Application
        };
        let draft = ChangeDraft::new(
            RecordKind::CurrencyRate,
            identity(&rate.from_currency, &rate.to_currency, &rate.date),
            operation,
            origin,
            based_on,
            Some(serde_json::to_string(&rate)?),
        );
        let rank = self.change_recorder.record(&mut tx, draft).await?;
        if let Some(rank) = rank {
            let columns = RankColumns::from(rank);
            sqlx::query!(
                "UPDATE currency_rates SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                 WHERE from_currency = ? AND to_currency = ? AND date = ?",
                columns.logical_timestamp,
                columns.origin,
                columns.device_id,
                rate.from_currency,
                rate.to_currency,
                rate.date
            )
            .execute(&mut *tx)
            .await
            .context("Failed to stamp rank on currency rate")?;
        }
        tx.commit()
            .await
            .context("Failed to commit currency rate upsert")?;
        Ok(rate)
    }

    async fn delete_rate(&self, from_currency: &str, to_currency: &str, date: &str) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin currency rate delete")?;
        let based_on = sqlx::query_scalar!(
            r#"SELECT sync_logical_timestamp AS "sync_logical_timestamp?: String"
               FROM currency_rates WHERE from_currency = ? AND to_currency = ? AND date = ?"#,
            from_currency,
            to_currency,
            date,
        )
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to read currency rate rank")?
        .flatten()
        .and_then(|timestamp| LogicalTimestamp::from_wire(&timestamp));
        let deleted = sqlx::query!(
            "DELETE FROM currency_rates
             WHERE from_currency = ? AND to_currency = ? AND date = ?",
            from_currency,
            to_currency,
            date,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to delete currency rate")?;
        if deleted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::CurrencyRate,
                identity(from_currency, to_currency, date),
                Operation::Removed,
                Origin::User,
                based_on,
                None,
            );
            self.change_recorder.record(&mut tx, draft).await?;
        }
        tx.commit()
            .await
            .context("Failed to commit currency rate delete")?;
        Ok(())
    }

    async fn list_rates_for_pair(
        &self,
        from_currency: &str,
        to_currency: &str,
    ) -> Result<Vec<CurrencyRate>> {
        let rows = sqlx::query_as!(
            RateRow,
            "SELECT from_currency, to_currency, date, rate, source FROM currency_rates
             WHERE from_currency = ? AND to_currency = ?
             ORDER BY date DESC",
            from_currency,
            to_currency,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list currency rates for pair")?;

        Ok(rows.into_iter().map(CurrencyRate::from).collect())
    }

    async fn get_by_key(
        &self,
        from_currency: &str,
        to_currency: &str,
        date: &str,
    ) -> Result<Option<CurrencyRate>> {
        let row = sqlx::query_as!(
            RateRow,
            "SELECT from_currency, to_currency, date, rate, source FROM currency_rates
             WHERE from_currency = ? AND to_currency = ? AND date = ?",
            from_currency,
            to_currency,
            date,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch currency rate by key")?;

        Ok(row.map(CurrencyRate::from))
    }

    async fn latest_rate_on_or_before(
        &self,
        from_currency: &str,
        to_currency: &str,
        as_of: &str,
    ) -> Result<Option<CurrencyRate>> {
        let row = sqlx::query_as!(
            RateRow,
            "SELECT from_currency, to_currency, date, rate, source FROM currency_rates
             WHERE from_currency = ? AND to_currency = ? AND date <= ?
             ORDER BY date DESC
             LIMIT 1",
            from_currency,
            to_currency,
            as_of,
        )
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch latest currency rate on or before date")?;

        Ok(row.map(CurrencyRate::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        // Seed the parent pair so the currency_rates FK is satisfied.
        sqlx::query!(
            "INSERT INTO currency_pairs (from_currency, to_currency) VALUES ('USD', 'EUR')"
        )
        .execute(&pool)
        .await
        .expect("seed pair");
        pool
    }

    fn rate(date: &str, micros: i64, source: CurrencyRateSource) -> CurrencyRate {
        CurrencyRate::from_storage("USD".into(), "EUR".into(), date.into(), micros, source)
    }

    // FXR-025 — upsert_rate inserts a new row, read back via get_by_key
    #[tokio::test]
    async fn upsert_rate_inserts_and_reads_back() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        repo.upsert_rate(rate("2026-01-01", 920_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        let got = repo
            .get_by_key("USD", "EUR", "2026-01-01")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.rate, 920_000);
        assert_eq!(got.source, CurrencyRateSource::Manual);
    }

    // FXR-025/ADR-012 — upsert_rate overwrites by (from, to, date) regardless of source
    #[tokio::test]
    async fn upsert_rate_overwrites_same_key() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        repo.upsert_rate(rate("2026-01-01", 920_000, CurrencyRateSource::Frankfurter))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-01-01", 950_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        let got = repo
            .get_by_key("USD", "EUR", "2026-01-01")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.rate, 950_000);
        assert_eq!(got.source, CurrencyRateSource::Manual);
        let all = repo.list_rates_for_pair("USD", "EUR").await.unwrap();
        assert_eq!(all.len(), 1, "overwrite, not append");
    }

    // FXR-053 — delete_rate removes the row
    #[tokio::test]
    async fn delete_rate_removes_row() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        repo.upsert_rate(rate("2026-01-01", 920_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.delete_rate("USD", "EUR", "2026-01-01").await.unwrap();

        assert!(repo
            .get_by_key("USD", "EUR", "2026-01-01")
            .await
            .unwrap()
            .is_none());
    }

    // delete_rate is a no-op (no error) when the row is absent
    #[tokio::test]
    async fn delete_rate_noop_when_absent() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        assert!(repo.delete_rate("USD", "EUR", "2026-01-01").await.is_ok());
    }

    // FXR-050 — list_rates_for_pair returns rows ordered by date descending
    #[tokio::test]
    async fn list_rates_for_pair_orders_date_desc() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        repo.upsert_rate(rate("2026-01-01", 910_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-01-03", 930_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-01-02", 920_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        let rows = repo.list_rates_for_pair("USD", "EUR").await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].date, "2026-01-03");
        assert_eq!(rows[1].date, "2026-01-02");
        assert_eq!(rows[2].date, "2026-01-01");
    }

    // FXR-050 — list_rates_for_pair returns an empty list for an unknown pair
    #[tokio::test]
    async fn list_rates_for_pair_empty_for_unknown() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        let rows = repo.list_rates_for_pair("JPY", "EUR").await.unwrap();
        assert!(rows.is_empty());
    }

    // get_by_key returns None when the row does not exist
    #[tokio::test]
    async fn get_by_key_none_when_absent() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        assert!(repo
            .get_by_key("USD", "EUR", "2026-01-01")
            .await
            .unwrap()
            .is_none());
    }

    // -------------------------------------------------------------------------
    // latest_rate_on_or_before — FXR-035
    // -------------------------------------------------------------------------

    // FXR-035 — returns the most-recent rate whose date is ≤ as_of when multiple
    // rates exist across different dates
    #[tokio::test]
    async fn latest_rate_on_or_before_returns_most_recent_le_as_of() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        // Seed three rates: 2026-01-01, 2026-01-10, 2026-01-20
        repo.upsert_rate(rate("2026-01-01", 900_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-01-10", 920_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-01-20", 950_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        // as_of = 2026-01-15: the 2026-01-10 row is the latest ≤ 2026-01-15
        let got = repo
            .latest_rate_on_or_before("USD", "EUR", "2026-01-15")
            .await
            .unwrap()
            .expect("should find a rate");
        assert_eq!(got.date, "2026-01-10");
        assert_eq!(got.rate, 920_000);
    }

    // FXR-035 — returns None when all seeded rates are strictly AFTER as_of
    #[tokio::test]
    async fn latest_rate_on_or_before_returns_none_when_all_rates_are_future() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        // All rates dated after the query date
        repo.upsert_rate(rate("2026-06-01", 920_000, CurrencyRateSource::Manual))
            .await
            .unwrap();
        repo.upsert_rate(rate("2026-07-01", 930_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        let got = repo
            .latest_rate_on_or_before("USD", "EUR", "2026-01-01")
            .await
            .unwrap();
        assert!(
            got.is_none(),
            "expected None when all rates are after as_of"
        );
    }

    // FXR-035 — returns None when the pair has no rates at all
    #[tokio::test]
    async fn latest_rate_on_or_before_returns_none_when_pair_has_no_rates() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        // No rates inserted at all

        let got = repo
            .latest_rate_on_or_before("USD", "EUR", "2026-06-01")
            .await
            .unwrap();
        assert!(
            got.is_none(),
            "expected None when the pair has no rates at all"
        );
    }

    // FXR-035 — exact-date match: a rate dated exactly as_of is returned
    #[tokio::test]
    async fn latest_rate_on_or_before_returns_exact_date_match() {
        let repo = SqliteCurrencyRateRepository::new(setup_pool().await);
        repo.upsert_rate(rate("2026-05-15", 940_000, CurrencyRateSource::Manual))
            .await
            .unwrap();

        let got = repo
            .latest_rate_on_or_before("USD", "EUR", "2026-05-15")
            .await
            .unwrap()
            .expect("exact-date match should be returned");
        assert_eq!(got.date, "2026-05-15");
        assert_eq!(got.rate, 940_000);
    }

    use crate::context::sync::SqliteChangeRecorder;
    use std::sync::Arc;

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
        sqlx::query!(
            "INSERT INTO currency_pairs (from_currency, to_currency) VALUES ('USD', 'EUR')"
        )
        .execute(&pool)
        .await
        .expect("seed pair");
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

    async fn changes_with_operation(pool: &SqlitePool, operation: &str) -> i64 {
        sqlx::query_scalar!(
            "SELECT COUNT(*) FROM changes WHERE operation = ?",
            operation
        )
        .fetch_one(pool)
        .await
        .unwrap()
    }

    fn manual_rate(date: &str, micros: i64) -> CurrencyRate {
        CurrencyRate::from_storage(
            "USD".into(),
            "EUR".into(),
            date.into(),
            micros,
            CurrencyRateSource::Manual,
        )
    }

    // SYN-020/021 — upsert_rate (creation) records exactly one Created change, rank-stamped
    // (origin User; CFR-050 resolves rate observations without regard to origin).
    #[tokio::test]
    async fn upsert_rate_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteCurrencyRateRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert_rate(manual_rate("2026-08-20", 920_000))
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp FROM currency_rates WHERE from_currency = 'USD' AND to_currency = 'EUR' AND date = '2026-08-20'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
    }

    // SYN-020 — overwriting the same (pair, date) key records exactly one Updated change.
    #[tokio::test]
    async fn upsert_rate_overwrite_records_one_updated_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteCurrencyRateRepository::new(pool.clone());
        setup_repo
            .upsert_rate(manual_rate("2026-08-20", 920_000))
            .await
            .unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteCurrencyRateRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.upsert_rate(manual_rate("2026-08-20", 930_000))
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Updated").await, 1);
    }

    // SYN-020/024 — delete_rate records exactly one Removed change and a tombstone.
    #[tokio::test]
    async fn delete_rate_records_one_removed_change_and_tombstone() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let setup_repo = SqliteCurrencyRateRepository::new(pool.clone());
        setup_repo
            .upsert_rate(manual_rate("2026-08-20", 920_000))
            .await
            .unwrap();

        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteCurrencyRateRepository::new(pool.clone()).with_change_recorder(recorder);
        repo.delete_rate("USD", "EUR", "2026-08-20").await.unwrap();

        assert_eq!(changes_with_operation(&pool, "Removed").await, 1);
        let tombstone = sqlx::query!(
            "SELECT record_identity FROM tombstones WHERE record_kind = 'CurrencyRate' AND record_identity = 'USD:EUR:2026-08-20'"
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert!(tombstone.is_some());
    }
}
