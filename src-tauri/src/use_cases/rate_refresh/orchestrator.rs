use std::sync::Arc;

use crate::context::account::AccountServiceContract;
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::scope::holding_fx_pairs;

use super::error::RateRefreshError;

/// Orchestrates the launch rate refresh (FXR-075).
pub struct RateRefreshUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
}

impl RateRefreshUseCase {
    /// Creates a new use case.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            currency_service,
        }
    }

    /// Ensures the pairs of active foreign holdings, then records the current rate of
    /// every persisted pair (FXR-071/075). A provider failure is silent (FXR-070/073):
    /// only a lookup or a write the database refuses is an error.
    pub async fn refresh(&self) -> Result<(), RateRefreshError> {
        let pairs = holding_fx_pairs(self.account_service.as_ref(), self.asset_service.as_ref())
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "rate refresh: holding lookup failed");
                RateRefreshError::DatabaseError
            })?;
        self.currency_service
            .refresh_all_rates(pairs)
            .await
            .map_err(|error| {
                tracing::error!(target: BACKEND, err = ?error, "rate refresh: rate write failed");
                RateRefreshError::DatabaseError
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        Account, AccountError, Holding, MockAccountServiceContract, UpdateFrequency,
    };
    use crate::context::asset::{
        Asset, AssetCategory, AssetClass, MockAssetServiceContract, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::domain::rate_provider::MockRateProvider;
    use crate::context::currency::{
        CurrencyRateSource, EurSnapshot, RateProvider, SqliteCurrencyPairRepository,
        SqliteCurrencyRateRepository,
    };
    use std::collections::HashMap;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
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

    /// One EUR account holding one USD asset.
    fn account_holding_a_usd_asset() -> MockAccountServiceContract {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![Account::restore(
                "acc-1".to_string(),
                "Portfolio".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::Automatic,
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(|account_id| {
                Ok(vec![Holding::restore(
                    "holding-1".to_string(),
                    account_id.to_string(),
                    "asset-usd".to_string(),
                    1_000_000,
                    100_000_000,
                    0,
                    None,
                )])
            });
        account_service
    }

    fn usd_asset_lookup() -> MockAssetServiceContract {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_asset_by_id()
            .returning(|asset_id| {
                Ok(Some(Asset::restore(
                    asset_id.to_string(),
                    "Apple".to_string(),
                    AssetClass::Stocks,
                    AssetCategory::from_storage(
                        SYSTEM_CATEGORY_ID.to_string(),
                        "generic.uncategorized".to_string(),
                    ),
                    "USD".to_string(),
                    4,
                    "AAPL".to_string(),
                    None,
                    false,
                    None,
                    false,
                    false,
                )))
            });
        asset_service
    }

    fn currency_service(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        provider: MockRateProvider,
    ) -> Arc<CurrencyService> {
        Arc::new(
            CurrencyService::new(
                Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
                Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
            )
            .with_rate_provider(Arc::new(provider) as Arc<dyn RateProvider>),
        )
    }

    // FXR-071/075 — the launch refresh follows the pair of a foreign holding and
    // records its current rate, with no price fetch involved.
    #[tokio::test]
    async fn the_launch_refresh_follows_holding_pairs_and_records_their_current_rate() {
        let pool = make_pool().await;
        let mut provider = MockRateProvider::new();
        provider.expect_fetch_eur_snapshot().times(1).returning(|| {
            Ok(EurSnapshot {
                date: "2026-09-25".to_string(),
                rates: HashMap::from([("USD".to_string(), 1_164_600i64)]),
                source: CurrencyRateSource::Frankfurter,
            })
        });
        let currency_service = currency_service(&pool, provider);
        let use_case = RateRefreshUseCase::new(
            Arc::new(account_holding_a_usd_asset()),
            Arc::new(usd_asset_lookup()),
            Arc::clone(&currency_service),
        );

        use_case.refresh().await.expect("refresh");

        let pairs = currency_service.list_currency_pairs().await.expect("pairs");
        assert_eq!(pairs.len(), 1, "the USD holding's pair must be followed");
        assert_eq!(
            (
                pairs[0].from_currency.as_str(),
                pairs[0].to_currency.as_str()
            ),
            ("USD", "EUR")
        );
        assert_eq!(pairs[0].latest_rate_date.as_deref(), Some("2026-09-25"));
    }

    // FXR-075 — an unreachable provider is silent: the refresh succeeds and the
    // stored rates stay as they were.
    #[tokio::test]
    async fn the_launch_refresh_is_silent_when_no_provider_answers() {
        let pool = make_pool().await;
        let mut provider = MockRateProvider::new();
        provider
            .expect_fetch_eur_snapshot()
            .times(1)
            .returning(|| Err(anyhow::anyhow!("unreachable")));
        let currency_service = currency_service(&pool, provider);
        let use_case = RateRefreshUseCase::new(
            Arc::new(account_holding_a_usd_asset()),
            Arc::new(usd_asset_lookup()),
            Arc::clone(&currency_service),
        );

        assert_eq!(use_case.refresh().await, Ok(()));
        let pairs = currency_service.list_currency_pairs().await.expect("pairs");
        assert_eq!(pairs.len(), 1, "the pair is still followed");
        assert_eq!(pairs[0].latest_rate, None, "no rate was written");
    }

    // A hard account-listing failure surfaces as DatabaseError, before any request.
    #[tokio::test]
    async fn the_launch_refresh_surfaces_an_account_listing_failure() {
        let pool = make_pool().await;
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Err(AccountError::DatabaseError));
        let mut provider = MockRateProvider::new();
        provider.expect_fetch_eur_snapshot().times(0);
        let use_case = RateRefreshUseCase::new(
            Arc::new(account_service),
            Arc::new(MockAssetServiceContract::new()),
            currency_service(&pool, provider),
        );

        assert_eq!(
            use_case.refresh().await,
            Err(RateRefreshError::DatabaseError)
        );
    }
}
