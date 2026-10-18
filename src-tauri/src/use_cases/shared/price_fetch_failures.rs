//! Why this installation's last fetch could not price an asset (MKT-220): written by the
//! in-app fetch task and the scheduled download, read by `price_fetch_failures` — owned by
//! none of them. Device-local, never synced (SYN-023).

use anyhow::Context;
use async_trait::async_trait;
use chrono::NaiveDateTime;
use serde::Serialize;
use specta::Type;

/// MKT-220 — why a fetch left an asset's price unupdated.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Type,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum PriceFetchFailureReason {
    /// The provider answered with no price for the asset.
    NoData,
    /// The provider could not be reached, or its answer could not be read.
    ProviderUnreachable,
    /// The provider gave a price that could not be saved.
    NotSaved,
}

/// MKT-220 — the last failure recorded for one asset on this installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct PriceFetchFailure {
    /// The asset the fetch could not price.
    pub asset_id: String,
    /// Why it could not.
    pub reason: PriceFetchFailureReason,
    /// The local wall-clock moment of that fetch, `YYYY-MM-DDTHH:MM:SS`.
    pub failed_at: String,
}

/// Remembers, per asset, why the last fetch on this installation could not price it.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait PriceFetchFailureRepository: Send + Sync {
    /// Records why `asset_id` could not be priced at `failed_at`, replacing what it had.
    async fn record(
        &self,
        asset_id: &str,
        reason: PriceFetchFailureReason,
        failed_at: NaiveDateTime,
    ) -> anyhow::Result<()>;
    /// Forgets the failure of `asset_id`: a fetch priced it.
    async fn clear(&self, asset_id: &str) -> anyhow::Result<()>;
    /// Every failure recorded, by asset id.
    async fn all(&self) -> anyhow::Result<Vec<PriceFetchFailure>>;
}

/// SQLite implementation of [`PriceFetchFailureRepository`].
pub struct SqlitePriceFetchFailureRepository {
    pool: sqlx::Pool<sqlx::Sqlite>,
}

impl SqlitePriceFetchFailureRepository {
    /// Creates a new repository backed by the given connection pool.
    pub fn new(pool: sqlx::Pool<sqlx::Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PriceFetchFailureRepository for SqlitePriceFetchFailureRepository {
    async fn record(
        &self,
        asset_id: &str,
        reason: PriceFetchFailureReason,
        failed_at: NaiveDateTime,
    ) -> anyhow::Result<()> {
        let reason = reason.to_string();
        let failed_at = failed_at.format("%Y-%m-%dT%H:%M:%S").to_string();
        sqlx::query!(
            "INSERT INTO price_fetch_failures (asset_id, reason, failed_at) VALUES (?, ?, ?)
             ON CONFLICT (asset_id) DO UPDATE
             SET reason = excluded.reason, failed_at = excluded.failed_at",
            asset_id,
            reason,
            failed_at,
        )
        .execute(&self.pool)
        .await
        .context("recording a price fetch failure")?;
        Ok(())
    }

    async fn clear(&self, asset_id: &str) -> anyhow::Result<()> {
        sqlx::query!(
            "DELETE FROM price_fetch_failures WHERE asset_id = ?",
            asset_id
        )
        .execute(&self.pool)
        .await
        .context("clearing a price fetch failure")?;
        Ok(())
    }

    async fn all(&self) -> anyhow::Result<Vec<PriceFetchFailure>> {
        let rows = sqlx::query!(
            "SELECT asset_id, reason, failed_at FROM price_fetch_failures ORDER BY asset_id"
        )
        .fetch_all(&self.pool)
        .await
        .context("reading the price fetch failures")?;
        rows.into_iter()
            .map(|row| {
                let reason = row
                    .reason
                    .parse::<PriceFetchFailureReason>()
                    .with_context(|| {
                        format!("unknown price fetch failure reason {}", row.reason)
                    })?;
                Ok(PriceFetchFailure {
                    asset_id: row.asset_id,
                    reason,
                    failed_at: row.failed_at,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };

    fn moment(day: u32, hour: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 10, day)
            .and_then(|date| date.and_hms_opt(hour, 0, 0))
            .expect("valid moment")
    }

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign keys");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        pool
    }

    async fn an_asset(pool: &sqlx::Pool<sqlx::Sqlite>, reference: &str) -> String {
        let service = AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        );
        service
            .create_asset(CreateAssetDTO {
                kind: None,
                name: reference.to_string(),
                reference: reference.to_string(),
                isin: None,
                class: AssetClass::DigitalAsset,
                currency: "EUR".to_string(),
                risk_level: 5,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("asset")
            .id
    }

    // MKT-220 — the last failure of an asset replaces the one before; a price clears it.
    #[tokio::test]
    async fn mkt_220_keeps_the_last_failure_until_a_fetch_prices_the_asset() {
        let pool = make_pool().await;
        let repo = SqlitePriceFetchFailureRepository::new(pool.clone());
        let btc = an_asset(&pool, "BTC").await;
        let eth = an_asset(&pool, "ETH").await;
        assert!(repo.all().await.unwrap().is_empty());

        repo.record(
            &btc,
            PriceFetchFailureReason::ProviderUnreachable,
            moment(17, 8),
        )
        .await
        .unwrap();
        repo.record(&btc, PriceFetchFailureReason::NoData, moment(18, 8))
            .await
            .unwrap();
        repo.record(&eth, PriceFetchFailureReason::NotSaved, moment(18, 8))
            .await
            .unwrap();
        let mut failures = repo.all().await.unwrap();
        failures.sort_by_key(|failure| failure.reason.to_string());
        assert_eq!(
            failures,
            vec![
                PriceFetchFailure {
                    asset_id: btc.clone(),
                    reason: PriceFetchFailureReason::NoData,
                    failed_at: "2026-10-18T08:00:00".to_string(),
                },
                PriceFetchFailure {
                    asset_id: eth.clone(),
                    reason: PriceFetchFailureReason::NotSaved,
                    failed_at: "2026-10-18T08:00:00".to_string(),
                },
            ]
        );

        repo.clear(&btc).await.unwrap();
        let left = repo.all().await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].asset_id, eth);
    }
}
