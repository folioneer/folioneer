//! `ServiceRankStamper` — the `RankStamper` port over the account, asset, and currency
//! services (ADR-004): each bounded context stamps its own tables with the first segment's
//! rank (CFR-014, D6) on the enrolment transaction's connection (SYN-013), the way
//! `ServicePortfolioSnapshot` reads them.

use std::sync::Arc;

use sqlx::SqliteConnection;

use crate::context::account::AccountService;
use crate::context::asset::AssetService;
use crate::context::currency::CurrencyService;
use crate::context::sync::{RankStamper, SyncError};
use crate::shared::domain::Rank;

/// Ranks the rows that existed before sync did, through the owning bounded contexts' services.
pub struct ServiceRankStamper {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
}

impl ServiceRankStamper {
    /// Creates the stamper over the three services that own synced records.
    pub fn new(
        account_service: Arc<AccountService>,
        asset_service: Arc<AssetService>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }
}

#[async_trait::async_trait]
impl RankStamper for ServiceRankStamper {
    async fn stamp_unranked_rows(
        &self,
        conn: &mut SqliteConnection,
        rank: &Rank,
    ) -> Result<u64, SyncError> {
        let account_rows = self
            .account_service
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|error| SyncError::database("rank stamper: account rows", error))?;
        let asset_rows = self
            .asset_service
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|error| SyncError::database("rank stamper: asset rows", error))?;
        let currency_rows = self
            .currency_service
            .stamp_sync_rank(conn, rank)
            .await
            .map_err(|error| SyncError::database("rank stamper: currency rows", error))?;
        Ok(account_rows + asset_rows + currency_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
        UpdateFrequency,
    };
    use crate::context::asset::{
        AssetClass, CreateAssetDTO, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::{SqliteCurrencyPairRepository, SqliteCurrencyRateRepository};
    use crate::shared::domain::{LogicalTimestamp, Origin};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
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

    fn build(pool: &sqlx::Pool<sqlx::Sqlite>) -> ServiceRankStamper {
        let account_service = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_service = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        let currency_service = Arc::new(CurrencyService::new(
            Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
            Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
        ));
        ServiceRankStamper::new(account_service, asset_service, currency_service)
    }

    fn rank() -> Rank {
        Rank {
            origin: Origin::User,
            logical_timestamp: LogicalTimestamp::new(7),
            device_id: "desktop-device".into(),
        }
    }

    async fn unranked_rows(pool: &sqlx::Pool<sqlx::Sqlite>, table: &str) -> i64 {
        sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM {table} WHERE sync_logical_timestamp IS NULL"
        ))
        .fetch_one(pool)
        .await
        .unwrap()
    }

    // CFR-014/D6 — every unranked row of the three contexts is stamped through the enrolment
    // connection, and the stamps are only visible once that transaction commits (SYN-013).
    #[tokio::test]
    async fn stamps_every_unranked_row_of_the_three_contexts_on_the_given_connection() {
        let pool = make_pool().await;
        let stamper = build(&pool);
        stamper
            .asset_service
            .create_asset(CreateAssetDTO {
                name: "AAPL".into(),
                reference: "AAPL".into(),
                isin: None,
                class: AssetClass::Stocks,
                currency: "USD".into(),
                risk_level: 2,
                category_id: SYSTEM_CATEGORY_ID.into(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        let account = stamper
            .account_service
            .create(
                "Portfolio".into(),
                String::new(),
                "USD".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        stamper
            .currency_service
            .declare_currency_pair("USD".into(), "EUR".into())
            .await
            .unwrap();
        assert_eq!(unranked_rows(&pool, "accounts").await, 1);
        assert_eq!(unranked_rows(&pool, "assets").await, 1);
        assert_eq!(unranked_rows(&pool, "currency_pairs").await, 1);

        let mut transaction = pool.begin().await.unwrap();
        let stamped = stamper
            .stamp_unranked_rows(&mut transaction, &rank())
            .await
            .expect("stamping must succeed");
        assert!(
            stamped >= 3,
            "the account, the asset, and the pair must all be stamped: {stamped}"
        );
        transaction.commit().await.unwrap();

        assert_eq!(unranked_rows(&pool, "accounts").await, 0);
        assert_eq!(unranked_rows(&pool, "assets").await, 0);
        assert_eq!(unranked_rows(&pool, "categories").await, 0);
        assert_eq!(unranked_rows(&pool, "currency_pairs").await, 0);
        let stamped_account: (String, String, String) = sqlx::query_as(
            "SELECT sync_logical_timestamp, sync_origin, sync_device_id FROM accounts WHERE id = ?",
        )
        .bind(&account.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            stamped_account,
            (
                "00000000000000000007".to_string(),
                "User".to_string(),
                "desktop-device".to_string()
            )
        );

        let stamped_again = stamper
            .stamp_unranked_rows(&mut pool.acquire().await.unwrap(), &rank())
            .await
            .unwrap();
        assert_eq!(stamped_again, 0, "already-ranked rows are never restamped");
    }
}
