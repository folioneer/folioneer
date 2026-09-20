use crate::context::currency::domain::{
    CurrencyPair, CurrencyPairRepository, CurrencyPairSummary, CurrencyRate, CurrencyRateSource,
};
use crate::core::logger::BACKEND;
use crate::shared::domain::{
    ChangeDraft, Operation, Origin, Rank, RecordIdentity, RecordKind, SyncedRecord,
};
use crate::shared::infrastructure::change_recorder::{
    rank_from_columns, ChangeRecorder, NoopChangeRecorder, RankColumns,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use sqlx::{SqliteConnection, SqlitePool};
use std::str::FromStr;
use std::sync::Arc;

/// SQLite-backed implementation of `CurrencyPairRepository`.
pub struct SqliteCurrencyPairRepository {
    pool: SqlitePool,
    change_recorder: Arc<dyn ChangeRecorder>,
}

impl SqliteCurrencyPairRepository {
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

/// Splits a `from:to` pair identity or a `from:to:date` rate identity (CFR-012).
/// The two currencies of a `from:to` pair identity and, for a `from:to:date` rate identity,
/// its date (CFR-012).
fn split_identity(identity: &str) -> Result<(&str, &str, Option<&str>)> {
    let mut keys = identity.split(':');
    match (keys.next(), keys.next(), keys.next()) {
        (Some(from_currency), Some(to_currency), date) => Ok((from_currency, to_currency, date)),
        _ => anyhow::bail!("malformed currency identity: '{identity}'"),
    }
}

#[derive(sqlx::FromRow)]
struct PairSummaryRow {
    from_currency: String,
    to_currency: String,
    latest_rate: Option<i64>,
    latest_rate_date: Option<String>,
    latest_rate_source: Option<String>,
}

#[async_trait]
impl CurrencyPairRepository for SqliteCurrencyPairRepository {
    async fn stamp_sync_rank(&self, conn: &mut SqliteConnection, rank: &Rank) -> Result<u64> {
        let columns = RankColumns::from(rank.clone());
        let (timestamp, origin, device_id) = (
            &columns.logical_timestamp,
            &columns.origin,
            &columns.device_id,
        );
        let mut stamped = 0;
        stamped += sqlx::query!(
            "UPDATE currency_pairs SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked currency pairs")?
        .rows_affected();
        stamped += sqlx::query!(
            "UPDATE currency_rates SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
             WHERE sync_logical_timestamp IS NULL",
            timestamp,
            origin,
            device_id
        )
        .execute(&mut *conn)
        .await
        .context("Failed to stamp unranked currency rates")?
        .rows_affected();
        Ok(stamped)
    }

    async fn upsert_pair(&self, pair: CurrencyPair) -> Result<CurrencyPair> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("Failed to begin currency pair upsert")?;
        let inserted = sqlx::query!(
            "INSERT INTO currency_pairs (from_currency, to_currency) VALUES (?, ?)
             ON CONFLICT(from_currency, to_currency) DO NOTHING",
            pair.from_currency,
            pair.to_currency,
        )
        .execute(&mut *tx)
        .await
        .context("Failed to upsert currency pair")?;
        if inserted.rows_affected() > 0 {
            let draft = ChangeDraft::new(
                RecordKind::CurrencyPair,
                RecordIdentity::canonical(
                    RecordKind::CurrencyPair,
                    &[&pair.from_currency, &pair.to_currency],
                ),
                Operation::Created,
                Origin::User,
                None,
                Some(serde_json::to_string(&pair)?),
            );
            let rank = self.change_recorder.record(&mut tx, draft).await?;
            if let Some(rank) = rank {
                let columns = RankColumns::from(rank);
                sqlx::query!(
                    "UPDATE currency_pairs SET sync_logical_timestamp = ?, sync_origin = ?, sync_device_id = ?
                     WHERE from_currency = ? AND to_currency = ?",
                    columns.logical_timestamp,
                    columns.origin,
                    columns.device_id,
                    pair.from_currency,
                    pair.to_currency
                )
                .execute(&mut *tx)
                .await
                .context("Failed to stamp rank on currency pair")?;
            }
        }
        tx.commit()
            .await
            .context("Failed to commit currency pair upsert")?;
        Ok(pair)
    }

    async fn synced_record(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<Option<SyncedRecord>> {
        let (from_currency, to_currency, date) = split_identity(identity)?;
        match kind {
            RecordKind::CurrencyPair => {
                let row = sqlx::query!(
                    r#"SELECT from_currency, to_currency, sync_logical_timestamp, sync_origin, sync_device_id
                       FROM currency_pairs WHERE from_currency = ? AND to_currency = ?"#,
                    from_currency,
                    to_currency
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced currency pair {}", identity))?;
                row.map(|row| {
                    let pair = CurrencyPair::from_storage(row.from_currency, row.to_currency);
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&pair)?,
                    })
                })
                .transpose()
            }
            RecordKind::CurrencyRate => {
                let Some(date) = date else {
                    anyhow::bail!("malformed currency rate identity: '{identity}'");
                };
                let row = sqlx::query!(
                    r#"SELECT from_currency, to_currency, date, rate, source,
                              sync_logical_timestamp, sync_origin, sync_device_id
                       FROM currency_rates WHERE from_currency = ? AND to_currency = ? AND date = ?"#,
                    from_currency,
                    to_currency,
                    date
                )
                .fetch_optional(conn)
                .await
                .with_context(|| format!("Failed to read synced currency rate {}", identity))?;
                row.map(|row| {
                    let source = CurrencyRateSource::from_str(&row.source).unwrap_or_else(|_| {
                        tracing::warn!(target: BACKEND, value = %row.source, "unknown currency_rates.source value, falling back to Manual");
                        CurrencyRateSource::Manual
                    });
                    let rate = CurrencyRate::from_storage(
                        row.from_currency,
                        row.to_currency,
                        row.date,
                        row.rate,
                        source,
                    );
                    Ok(SyncedRecord {
                        rank: rank_from_columns(
                            row.sync_logical_timestamp,
                            row.sync_origin,
                            row.sync_device_id,
                        ),
                        content: serde_json::to_string(&rate)?,
                    })
                })
                .transpose()
            }
            RecordKind::Account
            | RecordKind::Category
            | RecordKind::Asset
            | RecordKind::Transaction
            | RecordKind::FeeSchedule
            | RecordKind::FeeCatchUpPosition
            | RecordKind::AssetPrice
            | RecordKind::HoldingNote => Ok(None),
        }
    }

    async fn apply_pair(
        &self,
        conn: &mut SqliteConnection,
        pair: &CurrencyPair,
        rank: &Rank,
    ) -> Result<()> {
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO currency_pairs (from_currency, to_currency, sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(from_currency, to_currency) DO UPDATE SET
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            pair.from_currency,
            pair.to_currency,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply currency pair")?;
        Ok(())
    }

    async fn apply_rate(
        &self,
        conn: &mut SqliteConnection,
        rate: &CurrencyRate,
        rank: &Rank,
    ) -> Result<()> {
        let source = rate.source.to_string();
        let columns = RankColumns::from(rank.clone());
        sqlx::query!(
            r#"INSERT INTO currency_rates (from_currency, to_currency, date, rate, source,
                                           sync_logical_timestamp, sync_origin, sync_device_id)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(from_currency, to_currency, date) DO UPDATE SET
                   rate = excluded.rate,
                   source = excluded.source,
                   sync_logical_timestamp = excluded.sync_logical_timestamp,
                   sync_origin = excluded.sync_origin,
                   sync_device_id = excluded.sync_device_id"#,
            rate.from_currency,
            rate.to_currency,
            rate.date,
            rate.rate,
            source,
            columns.logical_timestamp,
            columns.origin,
            columns.device_id
        )
        .execute(conn)
        .await
        .context("Failed to apply currency rate")?;
        Ok(())
    }

    async fn remove_synced(
        &self,
        conn: &mut SqliteConnection,
        kind: RecordKind,
        identity: &str,
    ) -> Result<()> {
        let (from_currency, to_currency, date) = split_identity(identity)?;
        match kind {
            RecordKind::CurrencyPair => {
                sqlx::query!(
                    "DELETE FROM currency_pairs WHERE from_currency = ? AND to_currency = ?",
                    from_currency,
                    to_currency
                )
                .execute(conn)
                .await
                .with_context(|| format!("Failed to remove currency pair {}", identity))?;
            }
            RecordKind::CurrencyRate => {
                let Some(date) = date else {
                    anyhow::bail!("malformed currency rate identity: '{identity}'");
                };
                sqlx::query!(
                    "DELETE FROM currency_rates WHERE from_currency = ? AND to_currency = ? AND date = ?",
                    from_currency,
                    to_currency,
                    date
                )
                .execute(conn)
                .await
                .with_context(|| format!("Failed to remove currency rate {}", identity))?;
            }
            RecordKind::Account
            | RecordKind::Category
            | RecordKind::Asset
            | RecordKind::Transaction
            | RecordKind::FeeSchedule
            | RecordKind::FeeCatchUpPosition
            | RecordKind::AssetPrice
            | RecordKind::HoldingNote => {}
        }
        Ok(())
    }

    async fn discard_pairs_and_rates(&self, conn: &mut SqliteConnection) -> Result<()> {
        sqlx::query!("DELETE FROM currency_rates")
            .execute(&mut *conn)
            .await
            .context("Failed to discard currency rates")?;
        sqlx::query!("DELETE FROM currency_pairs")
            .execute(conn)
            .await
            .context("Failed to discard currency pairs")?;
        Ok(())
    }

    async fn list_pairs_with_latest_rate(&self) -> Result<Vec<CurrencyPairSummary>> {
        let rows = sqlx::query_as!(
            PairSummaryRow,
            r#"SELECT
                 p.from_currency AS "from_currency!: String",
                 p.to_currency   AS "to_currency!: String",
                 r.rate          AS "latest_rate?: i64",
                 r.date          AS "latest_rate_date?: String",
                 r.source        AS "latest_rate_source?: String"
               FROM currency_pairs p
               LEFT JOIN currency_rates r
                 ON r.from_currency = p.from_currency
                AND r.to_currency = p.to_currency
                AND r.date = (
                    SELECT MAX(r2.date) FROM currency_rates r2
                    WHERE r2.from_currency = p.from_currency
                      AND r2.to_currency = p.to_currency
                )
               ORDER BY p.from_currency, p.to_currency"#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list currency pairs with latest rate")?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let latest_rate_source = row.latest_rate_source.map(|s| {
                    CurrencyRateSource::from_str(&s).unwrap_or_else(|_| {
                        tracing::warn!(
                            target: BACKEND,
                            value = %s,
                            "unknown currency_rates.source value, falling back to Manual"
                        );
                        CurrencyRateSource::Manual
                    })
                });
                CurrencyPairSummary {
                    from_currency: row.from_currency,
                    to_currency: row.to_currency,
                    latest_rate: row.latest_rate,
                    latest_rate_date: row.latest_rate_date,
                    latest_rate_source,
                }
            })
            .collect())
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
        pool
    }

    /// Inserts a rate row directly so latest-rate resolution can be exercised
    /// without depending on the rate repository.
    async fn insert_rate(pool: &SqlitePool, from: &str, to: &str, date: &str, rate: i64) {
        sqlx::query!(
            "INSERT INTO currency_rates (from_currency, to_currency, date, rate, source)
             VALUES (?, ?, ?, ?, 'Manual')",
            from,
            to,
            date,
            rate,
        )
        .execute(pool)
        .await
        .expect("seed rate");
    }

    // CFR-014/D6 — stamp_sync_rank ranks every unranked pair and rate once; a second call
    // finds nothing left to stamp.
    #[tokio::test]
    async fn stamp_sync_rank_stamps_unranked_pairs_and_rates_once() {
        let pool = setup_pool().await;
        let repo = SqliteCurrencyPairRepository::new(pool.clone());
        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();
        insert_rate(&pool, "USD", "EUR", "2026-08-01", 920_000).await;

        let rank = Rank {
            origin: Origin::User,
            logical_timestamp: crate::shared::domain::LogicalTimestamp::new(99),
            device_id: "desktop-device".to_string(),
        };
        let mut conn = pool.acquire().await.unwrap();
        let stamped = repo.stamp_sync_rank(&mut conn, &rank).await.unwrap();
        assert_eq!(stamped, 2, "one pair and one rate are stamped");
        let stamped_again = repo.stamp_sync_rank(&mut conn, &rank).await.unwrap();
        assert_eq!(stamped_again, 0, "already-ranked rows are never restamped");
        drop(conn);

        let pair_stamp: Option<String> = sqlx::query_scalar(
            "SELECT sync_logical_timestamp FROM currency_pairs WHERE from_currency = 'USD'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pair_stamp.as_deref(), Some("00000000000000000099"));
    }

    // FXR-054 — upsert_pair persists a new pair and returns it
    #[tokio::test]
    async fn upsert_pair_inserts_and_returns() {
        let repo = SqliteCurrencyPairRepository::new(setup_pool().await);
        let pair = repo
            .upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();
        assert_eq!(pair.from_currency, "USD");
        assert_eq!(pair.to_currency, "EUR");

        let pairs = repo.list_pairs_with_latest_rate().await.unwrap();
        assert_eq!(pairs.len(), 1);
    }

    // FXR-054 — upsert_pair is idempotent: a second insert of the same key is a no-op
    #[tokio::test]
    async fn upsert_pair_is_idempotent() {
        let repo = SqliteCurrencyPairRepository::new(setup_pool().await);
        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();
        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();

        let pairs = repo.list_pairs_with_latest_rate().await.unwrap();
        assert_eq!(pairs.len(), 1, "no duplicate pair");
    }

    // FXR-051 — list_pairs_with_latest_rate returns an empty list when no pair exists
    #[tokio::test]
    async fn list_pairs_with_latest_rate_empty() {
        let repo = SqliteCurrencyPairRepository::new(setup_pool().await);
        let pairs = repo.list_pairs_with_latest_rate().await.unwrap();
        assert!(pairs.is_empty());
    }

    // FXR-051 — a pair with no recorded rate has None latest_* fields
    #[tokio::test]
    async fn list_pairs_with_latest_rate_pair_without_rate() {
        let repo = SqliteCurrencyPairRepository::new(setup_pool().await);
        repo.upsert_pair(CurrencyPair::from_storage("GBP".into(), "EUR".into()))
            .await
            .unwrap();

        let pairs = repo.list_pairs_with_latest_rate().await.unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].from_currency, "GBP");
        assert!(pairs[0].latest_rate.is_none());
        assert!(pairs[0].latest_rate_date.is_none());
        assert!(pairs[0].latest_rate_source.is_none());
    }

    // FXR-035/051 — latest_* reflect the most recently dated rate for the pair
    #[tokio::test]
    async fn list_pairs_with_latest_rate_resolves_most_recent() {
        let pool = setup_pool().await;
        let repo = SqliteCurrencyPairRepository::new(pool.clone());
        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();
        insert_rate(&pool, "USD", "EUR", "2026-01-01", 910_000).await;
        insert_rate(&pool, "USD", "EUR", "2026-01-03", 930_000).await;
        insert_rate(&pool, "USD", "EUR", "2026-01-02", 920_000).await;

        let pairs = repo.list_pairs_with_latest_rate().await.unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].latest_rate, Some(930_000));
        assert_eq!(pairs[0].latest_rate_date.as_deref(), Some("2026-01-03"));
        assert_eq!(
            pairs[0].latest_rate_source,
            Some(CurrencyRateSource::Manual)
        );
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

    // SYN-020/021 — upsert_pair records exactly one Created change, rank-stamped.
    #[tokio::test]
    async fn upsert_pair_records_one_created_change_with_rank_stamped() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteCurrencyPairRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();

        assert_eq!(changes_with_operation(&pool, "Created").await, 1);
        let row = sqlx::query!(
            "SELECT sync_logical_timestamp FROM currency_pairs WHERE from_currency = 'USD' AND to_currency = 'EUR'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(row.sync_logical_timestamp.is_some());
    }

    // SYN-020 — a repeated upsert_pair for the same key is a no-op (ON CONFLICT DO NOTHING)
    // and must record no second change.
    #[tokio::test]
    async fn upsert_pair_idempotent_second_call_records_no_change() {
        let pool = make_pool().await;
        seed_sync_device(&pool).await;
        let recorder = Arc::new(SqliteChangeRecorder::new(pool.clone()));
        let repo = SqliteCurrencyPairRepository::new(pool.clone()).with_change_recorder(recorder);

        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();
        repo.upsert_pair(CurrencyPair::from_storage("USD".into(), "EUR".into()))
            .await
            .unwrap();

        assert_eq!(
            changes_with_operation(&pool, "Created").await,
            1,
            "the second, no-op upsert must not record a second change"
        );
    }
}
