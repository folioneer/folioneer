//! Reads how fresh the holdings' prices are (MKT-200) and when this installation last
//! fetched prices (MKT-201).

use std::collections::HashSet;
use std::sync::Arc;

use serde::Serialize;
use specta::Type;

use super::error::PriceFreshnessError;
use crate::context::account::AccountServiceContract;
use crate::context::asset::AssetServiceContract;
use crate::core::logger::BACKEND;
use crate::use_cases::shared::price_fetch_log::PriceFetchLogRepository;

/// The two figures behind the header's price item (MKT-202).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct PriceFreshness {
    /// Date (`YYYY-MM-DD`) of the newest price recorded for a currently held asset,
    /// whichever device recorded it (MKT-200); `None` without a held, priced asset.
    pub newest_price_date: Option<String>,
    /// Local wall-clock moment (`YYYY-MM-DDTHH:MM:SS`) this installation last fetched
    /// prices, in the app or by a scheduled download (MKT-201); `None` if it never did.
    pub last_fetch_at: Option<String>,
}

/// Cross-context read over the account holdings, the asset prices and this
/// installation's fetch records.
pub struct PriceFreshnessUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    fetch_log: Arc<dyn PriceFetchLogRepository>,
}

impl PriceFreshnessUseCase {
    /// Creates the use case over its three read sources.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        fetch_log: Arc<dyn PriceFetchLogRepository>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            fetch_log,
        }
    }

    /// Reads both figures (MKT-200/201).
    pub async fn read(&self) -> Result<PriceFreshness, PriceFreshnessError> {
        Ok(PriceFreshness {
            newest_price_date: self.newest_price_date().await?,
            last_fetch_at: self.last_fetch_at().await?,
        })
    }

    /// MKT-200 — the newest price date among the assets held in any account.
    async fn newest_price_date(&self) -> Result<Option<String>, PriceFreshnessError> {
        let accounts = self.account_service.get_all().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "price_freshness: listing accounts failed");
            PriceFreshnessError::DatabaseError
        })?;
        let mut held_asset_ids: HashSet<String> = HashSet::new();
        for account in &accounts {
            let holdings = self
                .account_service
                .get_holdings_for_account(&account.id)
                .await
                .map_err(|e| {
                    tracing::error!(target: BACKEND, account_id = %account.id, err = ?e, "price_freshness: listing holdings failed");
                    PriceFreshnessError::DatabaseError
                })?;
            held_asset_ids.extend(
                holdings
                    .into_iter()
                    .filter(|holding| holding.quantity > 0)
                    .map(|holding| holding.asset_id),
            );
        }
        let mut newest: Option<String> = None;
        for asset_id in &held_asset_ids {
            let latest = self.asset_service.get_latest_price(asset_id).await.map_err(|e| {
                tracing::error!(target: BACKEND, asset_id = %asset_id, err = ?e, "price_freshness: reading the latest price failed");
                PriceFreshnessError::DatabaseError
            })?;
            if let Some(price) = latest {
                if newest
                    .as_deref()
                    .is_none_or(|date| price.date.as_str() > date)
                {
                    newest = Some(price.date);
                }
            }
        }
        Ok(newest)
    }

    /// MKT-201 — the moment this installation last completed a price fetch.
    async fn last_fetch_at(&self) -> Result<Option<String>, PriceFreshnessError> {
        self.fetch_log.last_completed_at().await.map_err(|e| {
            tracing::error!(target: BACKEND, err = ?e, "price_freshness: reading the fetch log failed");
            PriceFreshnessError::DatabaseError
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{Account, Holding, MockAccountServiceContract, UpdateFrequency};
    use crate::context::asset::{AssetPrice, AssetPriceSource, MockAssetServiceContract};
    use crate::use_cases::shared::price_fetch_log::MockPriceFetchLogRepository;

    /// One account holding `held` (asset id, quantity); `prices` gives each asset's latest
    /// price date.
    fn use_case(
        held: Vec<(&'static str, i64)>,
        prices: Vec<(&'static str, &'static str)>,
        last_fetch: Option<&'static str>,
    ) -> PriceFreshnessUseCase {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )])
        });
        account_service
            .expect_get_holdings_for_account()
            .returning(move |_| {
                Ok(held
                    .iter()
                    .map(|(asset_id, quantity)| {
                        Holding::restore(
                            format!("holding-{asset_id}"),
                            "acc-1".to_string(),
                            (*asset_id).to_string(),
                            *quantity,
                            0,
                            0,
                            None,
                        )
                    })
                    .collect())
            });
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_latest_price()
            .returning(move |asset_id| {
                Ok(prices
                    .iter()
                    .find(|(priced_asset, _)| *priced_asset == asset_id)
                    .map(|(priced_asset, date)| {
                        AssetPrice::restore(
                            (*priced_asset).to_string(),
                            (*date).to_string(),
                            100_000_000,
                            AssetPriceSource::Manual,
                        )
                    }))
            });
        let mut fetch_log = MockPriceFetchLogRepository::new();
        fetch_log
            .expect_last_completed_at()
            .returning(move || Ok(last_fetch.map(str::to_string)));
        PriceFreshnessUseCase::new(
            Arc::new(account_service),
            Arc::new(asset_service),
            Arc::new(fetch_log),
        )
    }

    // MKT-200 — the newest date among the held assets' latest prices.
    #[tokio::test]
    async fn mkt_200_reports_the_newest_price_date_among_held_assets() {
        let freshness = use_case(
            vec![("a", 1_000_000), ("b", 2_000_000)],
            vec![("a", "2026-08-28"), ("b", "2026-09-15")],
            None,
        )
        .read()
        .await
        .unwrap();
        assert_eq!(freshness.newest_price_date.as_deref(), Some("2026-09-15"));
    }

    // MKT-200 — a position that is no longer held does not count, however fresh its price.
    #[tokio::test]
    async fn mkt_200_ignores_the_prices_of_assets_no_longer_held() {
        let freshness = use_case(
            vec![("held", 1_000_000), ("closed", 0)],
            vec![("held", "2026-08-28"), ("closed", "2026-09-18")],
            None,
        )
        .read()
        .await
        .unwrap();
        assert_eq!(freshness.newest_price_date.as_deref(), Some("2026-08-28"));
    }

    // MKT-200 — nothing held, or nothing priced: no date.
    #[tokio::test]
    async fn mkt_200_reports_no_date_without_a_held_priced_asset() {
        let unpriced = use_case(vec![("a", 1_000_000)], vec![], None);
        assert_eq!(unpriced.read().await.unwrap().newest_price_date, None);
        let nothing_held = use_case(vec![], vec![("a", "2026-09-15")], None);
        assert_eq!(nothing_held.read().await.unwrap().newest_price_date, None);
    }

    // MKT-201 — the recorded moment is reported as is; a device that never fetched has none.
    #[tokio::test]
    async fn mkt_201_reports_the_last_fetch_on_this_device_or_none() {
        let fetched = use_case(vec![], vec![], Some("2026-09-19T08:14:00"));
        assert_eq!(
            fetched.read().await.unwrap().last_fetch_at.as_deref(),
            Some("2026-09-19T08:14:00")
        );
        let never = use_case(vec![], vec![], None);
        assert_eq!(never.read().await.unwrap().last_fetch_at, None);
    }

    fn one_account() -> MockAccountServiceContract {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| {
            Ok(vec![Account::restore(
                "acc-1".to_string(),
                "Alpha".to_string(),
                String::new(),
                "EUR".to_string(),
                UpdateFrequency::ManualMonth,
                false,
            )])
        });
        account_service
    }

    fn one_holding(mut account_service: MockAccountServiceContract) -> MockAccountServiceContract {
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| {
                Ok(vec![Holding::restore(
                    "holding-a".to_string(),
                    "acc-1".to_string(),
                    "a".to_string(),
                    1_000_000,
                    0,
                    0,
                    None,
                )])
            });
        account_service
    }

    fn quiet_fetch_log() -> MockPriceFetchLogRepository {
        let mut fetch_log = MockPriceFetchLogRepository::new();
        fetch_log.expect_last_completed_at().returning(|| Ok(None));
        fetch_log
    }

    // Every read failure reaches the frontend as the one code it can show.
    #[tokio::test]
    async fn a_failure_listing_accounts_is_a_database_error() {
        let mut account_service = MockAccountServiceContract::new();
        account_service
            .expect_get_all()
            .returning(|| Err(crate::context::account::AccountError::DatabaseError));
        let use_case = PriceFreshnessUseCase::new(
            Arc::new(account_service),
            Arc::new(MockAssetServiceContract::new()),
            Arc::new(quiet_fetch_log()),
        );
        assert_eq!(
            use_case.read().await,
            Err(PriceFreshnessError::DatabaseError)
        );
    }

    #[tokio::test]
    async fn a_failure_listing_holdings_is_a_database_error() {
        let mut account_service = one_account();
        account_service
            .expect_get_holdings_for_account()
            .returning(|_| Err(crate::context::account::AccountError::DatabaseError));
        let use_case = PriceFreshnessUseCase::new(
            Arc::new(account_service),
            Arc::new(MockAssetServiceContract::new()),
            Arc::new(quiet_fetch_log()),
        );
        assert_eq!(
            use_case.read().await,
            Err(PriceFreshnessError::DatabaseError)
        );
    }

    #[tokio::test]
    async fn a_failure_reading_a_latest_price_is_a_database_error() {
        let mut asset_service = MockAssetServiceContract::new();
        asset_service
            .expect_get_latest_price()
            .returning(|_| Err(anyhow::anyhow!("disk I/O error")));
        let use_case = PriceFreshnessUseCase::new(
            Arc::new(one_holding(one_account())),
            Arc::new(asset_service),
            Arc::new(quiet_fetch_log()),
        );
        assert_eq!(
            use_case.read().await,
            Err(PriceFreshnessError::DatabaseError)
        );
    }

    #[tokio::test]
    async fn a_failure_reading_the_fetch_log_is_a_database_error() {
        let mut account_service = MockAccountServiceContract::new();
        account_service.expect_get_all().returning(|| Ok(vec![]));
        let mut fetch_log = MockPriceFetchLogRepository::new();
        fetch_log
            .expect_last_completed_at()
            .returning(|| Err(anyhow::anyhow!("disk I/O error")));
        let use_case = PriceFreshnessUseCase::new(
            Arc::new(account_service),
            Arc::new(MockAssetServiceContract::new()),
            Arc::new(fetch_log),
        );
        assert_eq!(
            use_case.read().await,
            Err(PriceFreshnessError::DatabaseError)
        );
    }
}
