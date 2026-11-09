use crate::context::account::{AccountError, AccountServiceContract};
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::use_cases::shared::account_summaries::SummaryReader;
pub use crate::use_cases::shared::account_summaries::{
    AccountSummaries, AccountSummary, PortfolioTotal,
};
use std::result::Result as StdResult;
use std::sync::Arc;

/// Orchestrates a cross-context read of account + asset data to build the
/// Accounts-list view (ACC-021, B18).
pub struct AccountSummaryUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    reader: SummaryReader,
}

impl AccountSummaryUseCase {
    /// Creates a new use case instance. The currency service is the valuation
    /// read port for foreign-currency holdings (FXR-041/035).
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            reader: SummaryReader::new(account_service.clone(), asset_service, currency_service),
            account_service,
        }
    }

    /// ACC-051A — the summaries of the profile in use, only the accounts in `holder`'s name
    /// (case ignored) when one is given; the total and the holders stay every account's.
    pub async fn get_account_summaries_for_holder(
        &self,
        holder: Option<String>,
    ) -> StdResult<AccountSummaries, AccountError> {
        let mut read = self.get_account_summaries().await?;
        // ACC-051A — a blank holder reads every account.
        let wanted = holder
            .map(|holder| holder.trim().to_lowercase())
            .filter(|holder| !holder.is_empty());
        if let Some(wanted) = wanted {
            read.summaries.retain(|summary| {
                summary
                    .holder
                    .as_ref()
                    .is_some_and(|named| named.to_lowercase() == wanted)
            });
        }
        Ok(read)
    }

    /// Builds a summary row for every account of the profile in use (PFL-050).
    pub async fn get_account_summaries(&self) -> StdResult<AccountSummaries, AccountError> {
        let accounts = self.account_service.accounts_in_use().await?;
        self.reader.summaries_of(accounts).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, Holding, HoldingRepository, SqliteAccountRepository,
        SqliteHoldingRepository, SqliteTransactionRepository,
    };
    use crate::context::asset::{
        AssetService, CreateAssetDTO, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::use_cases::shared::account_summaries::portfolio_total;
    use chrono::Datelike;
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashMap;

    async fn setup(pool: &sqlx::Pool<sqlx::Sqlite>) -> (Arc<AccountService>, Arc<AssetService>) {
        let account_svc = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_svc = Arc::new(AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        (account_svc, asset_svc)
    }

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

    // GPF-011 — the portfolio total is reported in the reference currency chosen: a CHF
    // account counts in full, with no rate, in a CHF total.
    #[tokio::test]
    async fn gpf_011_the_portfolio_total_is_in_the_reference_currency_chosen() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create("Swiss".to_string(), String::new(), "CHF".to_string(), false)
            .await
            .unwrap();
        asset_svc.seed_cash_asset("CHF").await.unwrap();
        account_svc
            .record_deposit(&account.id, "2020-01-01".to_string(), 250_000_000, None)
            .await
            .unwrap();
        let currency_svc = Arc::new(
            CurrencyService::new(
                Box::new(crate::context::currency::SqliteCurrencyPairRepository::new(
                    pool.clone(),
                )),
                Box::new(crate::context::currency::SqliteCurrencyRateRepository::new(
                    pool.clone(),
                )),
            )
            .with_setting_repo(Box::new(
                crate::context::currency::SqlitePortfolioSettingRepository::new(pool.clone()),
            )),
        );
        currency_svc.set_reference_currency("CHF").await.unwrap();

        let uc = AccountSummaryUseCase::new(account_svc, asset_svc, currency_svc);
        let total = uc.get_account_summaries().await.unwrap().total;
        assert_eq!(total.currency, "CHF");
        assert_eq!(total.total_global_value, 250_000_000);
        assert!(!total.incomplete);
    }

    // ACC-021 — empty account returns total_global_value = 0
    #[tokio::test]
    async fn empty_account_has_zero_global_value() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create("Empty".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, account.id);
        assert_eq!(summaries[0].total_global_value, 0);
    }

    // ACC-021 / CSH-094 — cash + same-currency priced non-cash holding aggregate per account
    #[tokio::test]
    async fn aggregates_cash_and_priced_holdings_per_account() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        // Account A: 250 EUR cash + 2 units of a 110-EUR bond → 250 + 220 = 470 EUR
        let acc_a = account_svc
            .create("A".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        account_svc
            .record_deposit(&acc_a.id, "2020-01-01".to_string(), 250_000_000, None)
            .await
            .unwrap();
        let bond = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Bond".to_string(),
                reference: "BOND".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Bonds,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    acc_a.id.clone(),
                    bond.id.clone(),
                    crate::shared::domain::quantity::from_surface(2_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&bond.id, "2026-01-01", 110.0)
            .await
            .unwrap();

        // Account B: empty USD account
        let acc_b = account_svc
            .create("B".to_string(), String::new(), "USD".to_string(), false)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 2);

        let row_a = summaries.iter().find(|s| s.id == acc_a.id).unwrap();
        let row_b = summaries.iter().find(|s| s.id == acc_b.id).unwrap();
        assert_eq!(row_a.total_global_value, 470_000_000);
        assert_eq!(row_a.currency, "EUR");
        assert_eq!(row_b.total_global_value, 0);
        assert_eq!(row_b.currency, "USD");
    }

    // FXR-034 — foreign-currency non-cash holding with no usable rate contributes 0
    #[tokio::test]
    async fn foreign_currency_holding_contributes_zero() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "EUR Account".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        // USD-denominated asset held in EUR account; no rate recorded → contributes 0
        let asset = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "US Stock".to_string(),
                reference: "USX".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    asset.id.clone(),
                    crate::shared::domain::quantity::from_surface(1_000_000),
                    50_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&asset.id, "2026-01-01", 60.0)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries[0].total_global_value, 0);
    }

    // CSH-094 — unpriced non-cash holding contributes 0 (no fallback to average_price)
    #[tokio::test]
    async fn unpriced_holding_contributes_zero() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create("X".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let asset = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Unpriced".to_string(),
                reference: "UNP".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    asset.id.clone(),
                    crate::shared::domain::quantity::from_surface(2_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        // No record_asset_price → unpriced

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries[0].total_global_value, 0);
    }

    // ACC-045 — each row of the accounts list carries the account's wrapper, or none.
    #[tokio::test]
    async fn acc_045_each_summary_carries_the_account_wrapper() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        account_svc
            .create_classified(
                "PEA".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
                None,
                Some("fr_pea".to_string()),
            )
            .await
            .unwrap();
        account_svc
            .create("Plain".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        let wrapper = |name: &str| {
            summaries
                .iter()
                .find(|summary| summary.name == name)
                .unwrap()
                .wrapper
                .clone()
        };
        assert_eq!(wrapper("PEA").as_deref(), Some("fr_pea"));
        assert_eq!(wrapper("Plain"), None);
    }

    // MKT-206 — each account counts its holdings whose price is outdated (MKT-205), and the
    // portfolio total counts them over every account.
    #[tokio::test]
    async fn mkt_206_each_account_and_the_portfolio_total_count_outdated_prices() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create("X".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let today = chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        for (symbol, price_date) in [("OLD", "2026-01-02"), ("NEW", today.as_str())] {
            let asset = asset_svc
                .create_asset(CreateAssetDTO {
                    kind: None,
                    name: symbol.to_string(),
                    reference: symbol.to_string(),
                    isin: None,
                    class: crate::context::asset::AssetClass::DigitalAsset,
                    currency: "EUR".to_string(),
                    risk_level: 5,
                    category_id: SYSTEM_CATEGORY_ID.to_string(),
                    exchange: None,
                    interest_bearing: false,
                })
                .await
                .unwrap();
            SqliteHoldingRepository::new(pool.clone())
                .upsert(
                    Holding::new(
                        account.id.clone(),
                        asset.id.clone(),
                        crate::shared::domain::quantity::from_surface(1_000_000),
                        100_000_000,
                        0,
                        None,
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            asset_svc
                .record_asset_price(&asset.id, price_date, 100.0)
                .await
                .unwrap();
        }

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let read = uc.get_account_summaries().await.unwrap();
        assert_eq!(read.summaries[0].outdated_price_count, 1);
        assert_eq!(read.total.outdated_price_count, 1);
    }

    // -------------------------------------------------------------------------
    // FXR-041/ACC-021 — multi-currency global value lift
    // -------------------------------------------------------------------------
    //
    // Setup (mirrors account_details tests):
    //   account currency = EUR, asset currency = USD
    //   quantity = 1_000_000 (1.0 unit), current_price = 110_000_000 USD
    //   rate (USD→EUR) = 1_080_000
    //
    // converted market value = (1_000_000 × 118_800_000) / 1_000_000 = 118_800_000
    //   (where converted_price = (110_000_000 * 1_080_000) / 1_000_000 = 118_800_000)

    use crate::context::currency::{
        application::service::CurrencyService,
        domain::{MockCurrencyPairRepository, MockCurrencyRateRepository},
    };

    fn make_currency_service_with_fixed_rate(rate_micros: i64) -> Arc<CurrencyService> {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .returning(move |_, _, _| {
                Ok(Some(
                    crate::context::currency::domain::CurrencyRate::from_storage(
                        "USD".to_string(),
                        "EUR".to_string(),
                        "2026-01-01".to_string(),
                        rate_micros,
                        crate::context::currency::domain::CurrencyRateSource::Manual,
                    ),
                ))
            });
        Arc::new(CurrencyService::new(
            Box::new(pair_repo),
            Box::new(rate_repo),
        ))
    }

    fn make_currency_service_with_no_rate() -> Arc<CurrencyService> {
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0..)
            .returning(|_, _, _| Ok(None));
        Arc::new(CurrencyService::new(
            Box::new(pair_repo),
            Box::new(rate_repo),
        ))
    }

    // FXR-041/ACC-021 — a foreign holding's converted market value is included in
    // total_global_value when a rate is available.
    #[tokio::test]
    async fn foreign_holding_with_rate_included_in_total_global_value() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        let account = account_svc
            .create(
                "FX Summary".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let asset = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "US Stock".to_string(),
                reference: "USX".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 3,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();

        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    asset.id.clone(),
                    crate::shared::domain::quantity::from_surface(1_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        // current_price = 110.00 USD
        asset_svc
            .record_asset_price(&asset.id, "2026-01-01", 110.0)
            .await
            .unwrap();

        let currency_svc = make_currency_service_with_fixed_rate(1_080_000);
        let uc = AccountSummaryUseCase::new(account_svc, asset_svc, currency_svc);
        let summaries = uc.get_account_summaries().await.unwrap().summaries;

        // converted_price = (110_000_000 * 1_080_000) / 1_000_000 = 118_800_000
        // market_value = (1_000_000 * 118_800_000) / 1_000_000 = 118_800_000
        assert_eq!(
            summaries[0].total_global_value, 118_800_000,
            "got {}",
            summaries[0].total_global_value
        );
    }

    // Regression — same-currency holding contributes unchanged after FXR lift.
    #[tokio::test]
    async fn same_currency_holding_unchanged_in_summary() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        let account = account_svc
            .create(
                "Same CCY".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let bond = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Bond".to_string(),
                reference: "BOND".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Bonds,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();

        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    bond.id.clone(),
                    crate::shared::domain::quantity::from_surface(2_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        asset_svc
            .record_asset_price(&bond.id, "2026-01-01", 110.0)
            .await
            .unwrap();

        // currency service: no call expected for EUR→EUR
        let pair_repo = MockCurrencyPairRepository::new();
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0)
            .returning(|_, _, _| Ok(None));
        let currency_svc = Arc::new(CurrencyService::new(
            Box::new(pair_repo),
            Box::new(rate_repo),
        ));

        let uc = AccountSummaryUseCase::new(account_svc, asset_svc, currency_svc);
        let summaries = uc.get_account_summaries().await.unwrap().summaries;

        // 2 units × 110 EUR = 220 EUR = 220_000_000 micros (same as before FXR lift)
        assert_eq!(summaries[0].total_global_value, 220_000_000);
    }

    // FXR-034 — foreign holding with no rate contributes 0 to total_global_value.
    #[tokio::test]
    async fn foreign_holding_without_rate_contributes_zero_to_global_value() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        let account = account_svc
            .create(
                "No Rate".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let asset = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "US Stock".to_string(),
                reference: "USX".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 3,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();

        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    asset.id.clone(),
                    crate::shared::domain::quantity::from_surface(1_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();

        asset_svc
            .record_asset_price(&asset.id, "2026-01-01", 110.0)
            .await
            .unwrap();

        let currency_svc = make_currency_service_with_no_rate();
        let uc = AccountSummaryUseCase::new(account_svc, asset_svc, currency_svc);
        let summaries = uc.get_account_summaries().await.unwrap().summaries;

        assert_eq!(
            summaries[0].total_global_value, 0,
            "foreign holding with no rate must contribute 0; got {}",
            summaries[0].total_global_value
        );
    }

    // CSH-094 — closed holdings (quantity == 0) are excluded
    #[tokio::test]
    async fn closed_holdings_excluded() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create("X".to_string(), String::new(), "EUR".to_string(), false)
            .await
            .unwrap();
        let asset = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Closed".to_string(),
                reference: "CLS".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        // quantity = 0 → closed holding
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    asset.id.clone(),
                    0,
                    100_000_000,
                    0,
                    Some("2025-12-01".to_string()),
                )
                .unwrap(),
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&asset.id, "2026-01-01", 200.0)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(
            summaries[0].total_global_value, 0,
            "closed holdings (quantity = 0) must not contribute"
        );
    }

    // =========================================================================
    // ACC-023 — total_unrealized_pnl
    // =========================================================================

    // ACC-023 — a priced same-currency holding produces the correct
    // account-wide unrealized P&L (MKT-040 algorithm).
    //
    // Setup: 1 unit of a EUR stock bought at average_price = 100 EUR,
    // current_price = 120 EUR.
    // unrealized_pnl = (120 − 100) × 1 = 20 EUR = 20_000_000 micros.
    #[tokio::test]
    async fn priced_same_currency_holding_has_correct_total_unrealized_pnl() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "PnL Account".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let stock = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Priced Stock".to_string(),
                reference: "PRC".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        // 1 unit at average_price = 100 EUR → cost_basis = 100_000_000
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    stock.id.clone(),
                    crate::shared::domain::quantity::from_surface(1_000_000), // quantity: 1 unit
                    100_000_000, // average_price: 100 EUR
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        // current_price = 120 EUR
        asset_svc
            .record_asset_price(&stock.id, "2026-01-01", 120.0)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        // (120 − 100) × 1 unit = 20 EUR = 20_000_000 micros
        assert_eq!(
            summaries[0].total_unrealized_pnl,
            Some(20_000_000),
            "expected 20 EUR unrealized P&L; got {:?}",
            summaries[0].total_unrealized_pnl
        );
    }

    // ACC-023 — an empty account (no holdings) yields total_unrealized_pnl = None.
    #[tokio::test]
    async fn empty_account_has_none_total_unrealized_pnl() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        account_svc
            .create(
                "Empty PnL".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].total_unrealized_pnl.is_none(),
            "empty account must have total_unrealized_pnl = None; got {:?}",
            summaries[0].total_unrealized_pnl
        );
    }

    // ACC-023 — an account whose only holding is unpriced yields
    // total_unrealized_pnl = None (MKT-031: no price → no P&L).
    #[tokio::test]
    async fn unpriced_holding_account_has_none_total_unrealized_pnl() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "Unpriced PnL".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        let stock = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "Unpriced".to_string(),
                reference: "UNP2".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        SqliteHoldingRepository::new(pool.clone())
            .upsert(
                Holding::new(
                    account.id.clone(),
                    stock.id.clone(),
                    crate::shared::domain::quantity::from_surface(2_000_000),
                    100_000_000,
                    0,
                    None,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        // No price recorded → unpriced

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].total_unrealized_pnl.is_none(),
            "unpriced holding must produce total_unrealized_pnl = None; got {:?}",
            summaries[0].total_unrealized_pnl
        );
    }

    // =========================================================================
    // ACC-024 — ytd_performance_pct
    // =========================================================================

    // ACC-024 — a first-calendar-year account (account opens and deposits in
    // the current year, no prior-year baseline) must have a present
    // ytd_performance_pct (not None). PRF-034: inception-year accounts use
    // baseline 0 for the YTD computation, so the denominator is the weighted
    // deposit rather than 0.
    //
    // Setup: deposit 1000 EUR on Jan 1 of the current year.
    // YTD start baseline = 0 (prior Dec 31 of a year before any data = 0).
    // net_flow in the year = 1000 EUR; end_value = 1000 EUR (cash at face).
    // gain = 1000 − 0 − 1000 = 0. weighted flow = 1000 × full-year fraction > 0.
    // Dietz denominator > 0 → pct is Some(0).
    #[tokio::test]
    async fn first_calendar_year_account_has_present_ytd_performance_pct() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "YTD First Year".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        // Deposit on Jan 1 of the current year
        let current_year = chrono::Local::now().date_naive().year();
        let deposit_date = format!("{current_year}-01-01");
        account_svc
            .record_deposit(&account.id, deposit_date, 1_000_000_000, None)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].ytd_performance_pct.is_some(),
            "first-calendar-year account must have ytd_performance_pct present (not None); got None"
        );
    }

    // ACC-024 — an account with a prior-year baseline and a deposit this year has
    // ytd_performance_pct matching the PRF-034 Simple-Dietz computation.
    //
    // Setup (all in known past years so "today" doesn't affect the baseline):
    //   - Deposit 1000 EUR on 2024-12-31 (prior-year baseline = 1000).
    //   - Deposit 500 EUR on 2025-01-01 (flow in 2025).
    //   - No price changes → end_value for YTD = 1500 EUR.
    //
    // But because tests run on an unknown "today" we need the YTD span to be
    // a past year we fully control. We use 2024 as the prior-year baseline year
    // and seed a 2024-only deposit; then for 2025 we check the 2025 YTD via the
    // performance use-case route and compare.
    //
    // Simpler pinned scenario: use a year that is already complete (2023 → 2024).
    //   - 2023-12-31 baseline: deposit 1000 EUR
    //   - 2024-01-02 flow: deposit 500 EUR (Jan flow, not on Jan 1 so days_remaining > 0
    //     and the weighted denominator > 0).
    //   - No non-cash holdings, no price changes.
    //   - 2024 YTD at end of 2024: end_value = 1500, baseline = 1000,
    //     net_flow = 500, gain = 0, denom = 1000 + 500 × (364/365) ≈ 1498.6 EUR
    //     → pct ≈ 0 (integer truncation). The test only verifies pct is Some(0)
    //     to isolate the "denominator non-zero → present" invariant.
    //
    // NOTE: This test will only exercise the 2024 YTD for a current year > 2024
    // (i.e., the test runs in 2025+). In that case AccountSummary.ytd_performance_pct
    // is the CURRENT year's YTD (not 2024's). We assert it is Some(0) for the
    // current year (cash-only: no gain).
    #[tokio::test]
    async fn prior_year_baseline_account_has_present_ytd_performance_pct() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "YTD With Baseline".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        // Seed a deposit in a past year to establish a non-zero year-start baseline
        account_svc
            .record_deposit(&account.id, "2024-06-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();
        // Seed a deposit in the current year so the current-YTD denominator > 0
        let current_year = chrono::Local::now().date_naive().year();
        let this_year_flow_date = format!("{current_year}-01-02");
        account_svc
            .record_deposit(&account.id, this_year_flow_date, 500_000_000, None)
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].ytd_performance_pct.is_some(),
            "account with prior-year baseline must have ytd_performance_pct present; got None"
        );
    }

    // ACC-024 — an account with a prior-year baseline and a flow this year
    // has ytd_performance_pct that matches the equivalent get_account_performance
    // latest-period year_to_date.pct (PRF-034 reuse).
    //
    // This test verifies that the AccountSummary YTD value is consistent with
    // the AccountPerformance computation for the same account.
    //
    // Setup: deposit 1000 EUR on 2024-06-01 (prior year) and deposit 500 EUR
    // on Jan 2 of the current year. Cash-only, no price changes.
    // Both computations must agree on pct.
    #[tokio::test]
    async fn ytd_performance_pct_matches_account_performance_ytd() {
        use crate::context::currency::domain::{
            MockCurrencyPairRepository, MockCurrencyRateRepository,
        };
        use crate::use_cases::account_performance::AccountPerformanceUseCase;

        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "YTD Cross-check".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        // Establish a prior-year baseline
        account_svc
            .record_deposit(&account.id, "2024-06-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();
        // Flow in the current year (Jan 2 so days_remaining > 0 → denominator > 0)
        let current_year = chrono::Local::now().date_naive().year();
        let this_year_flow_date = format!("{current_year}-01-02");
        account_svc
            .record_deposit(&account.id, this_year_flow_date, 500_000_000, None)
            .await
            .unwrap();

        // Build a no-rate currency service usable by both use cases
        let make_no_rate_svc = || {
            let pair_repo = MockCurrencyPairRepository::new();
            let mut rate_repo = MockCurrencyRateRepository::new();
            rate_repo
                .expect_latest_rate_on_or_before()
                .times(0..)
                .returning(|_, _, _| Ok(None));
            Arc::new(crate::context::currency::CurrencyService::new(
                Box::new(pair_repo),
                Box::new(rate_repo),
            ))
        };

        // AccountSummary path
        let summary_uc =
            AccountSummaryUseCase::new(account_svc.clone(), asset_svc.clone(), make_no_rate_svc());
        let summaries = summary_uc.get_account_summaries().await.unwrap().summaries;
        let summary_ytd = summaries
            .iter()
            .find(|s| s.id == account.id)
            .unwrap()
            .ytd_performance_pct;

        // AccountPerformance path — find the current year's latest-month row ytd
        let perf_uc = AccountPerformanceUseCase::new(
            account_svc.clone(),
            asset_svc.clone(),
            make_no_rate_svc(),
        );
        let perf_resp = perf_uc
            .get_account_performance(&account.id, None)
            .await
            .unwrap();
        let current_month = chrono::Local::now().date_naive().month() as u8;
        let latest_month_row = perf_resp
            .monthly
            .iter()
            .find(|p| p.year == current_year && p.month == Some(current_month));
        let perf_ytd_pct = latest_month_row
            .and_then(|row| row.year_to_date.as_ref())
            .and_then(|m| m.pct);

        assert_eq!(
            summary_ytd, perf_ytd_pct,
            "AccountSummary.ytd_performance_pct ({summary_ytd:?}) must match \
             AccountPerformance latest-month year_to_date.pct ({perf_ytd_pct:?})"
        );
    }

    // ACC-024 / PRF-032 — an account with no transactions has no data span
    // and therefore no ytd period to compute → ytd_performance_pct = None.
    #[tokio::test]
    async fn no_transaction_account_has_none_ytd_performance_pct() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        account_svc
            .create(
                "YTD None".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0].ytd_performance_pct.is_none(),
            "account with no transactions must have ytd_performance_pct = None; got {:?}",
            summaries[0].ytd_performance_pct
        );
    }

    // CFR-042/SYN-040 — an account with an overdrawn cash holding (a merge-only state, no
    // single write's guard reaches it) must have `has_inconsistent_holding = true`, not a
    // hardcoded `false`.
    #[tokio::test]
    async fn account_with_overdrawn_cash_holding_has_inconsistent_holding_flag_set() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        let account = account_svc
            .create(
                "Inconsistent".to_string(),
                String::new(),
                "EUR".to_string(),
                false,
            )
            .await
            .unwrap();
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        account_svc
            .record_deposit(&account.id, "2026-01-01".to_string(), 1_000_000, None)
            .await
            .unwrap();
        let cash_asset_id = crate::core::cash::system_cash_asset_id("EUR");
        sqlx::query(
            "UPDATE holdings SET quantity = -5000000 WHERE account_id = ? AND asset_id = ?",
        )
        .bind(&account.id)
        .bind(&cash_asset_id)
        .execute(&pool)
        .await
        .unwrap();

        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let summaries = uc.get_account_summaries().await.unwrap().summaries;
        assert!(
            summaries[0].has_inconsistent_holding,
            "CFR-042/SYN-040: an overdrawn cash holding must set has_inconsistent_holding"
        );
    }

    // ── ACC-027 / ACC-028 — portfolio total ─────────────────────────────────

    fn summary(currency: &str, global_value: i64, unrealized_pnl: Option<i64>) -> AccountSummary {
        AccountSummary {
            id: format!("acc-{currency}-{global_value}"),
            name: "Account".to_string(),
            currency: currency.to_string(),
            wrapper: None,
            holder: None,
            total_global_value: global_value,
            total_unrealized_pnl: unrealized_pnl,
            ytd_performance_pct: None,
            has_inconsistent_holding: false,
            outdated_price_count: 0,
            created_by: None,
        }
    }

    // ACC-027 — every account's value and P&L converted to the reference currency.
    #[test]
    fn portfolio_total_converts_every_account_to_the_reference_currency() {
        let summaries = vec![
            summary("EUR", 100_000_000, Some(10_000_000)),
            summary("USD", 200_000_000, Some(-20_000_000)),
        ];
        let rates = HashMap::from([
            ("EUR".to_string(), Some(1_000_000)),
            ("USD".to_string(), Some(900_000)),
        ]);

        let total = portfolio_total(&summaries, &rates, "EUR");

        assert_eq!(total.total_global_value, 280_000_000);
        assert_eq!(total.total_unrealized_pnl, Some(-8_000_000));
        assert_eq!(total.currency, "EUR");
        assert!(!total.incomplete);
    }

    // ACC-028 — an account with no usable rate contributes zero and, holding a figure, marks the total incomplete.
    #[test]
    fn portfolio_total_leaves_out_an_unconvertible_account_and_is_incomplete() {
        let summaries = vec![
            summary("EUR", 100_000_000, Some(10_000_000)),
            summary("GBP", 500_000_000, Some(50_000_000)),
        ];
        let rates = HashMap::from([
            ("EUR".to_string(), Some(1_000_000)),
            ("GBP".to_string(), None),
        ]);

        let total = portfolio_total(&summaries, &rates, "EUR");

        assert_eq!(total.total_global_value, 100_000_000);
        assert_eq!(total.total_unrealized_pnl, Some(10_000_000));
        assert!(total.incomplete);
    }

    // ACC-029 — the total P&L is absent when no account carries one.
    #[test]
    fn portfolio_total_unrealized_pnl_is_absent_when_no_account_carries_one() {
        let summaries = vec![
            summary("EUR", 100_000_000, None),
            summary("USD", 50_000_000, None),
        ];
        let rates = HashMap::from([
            ("EUR".to_string(), Some(1_000_000)),
            ("USD".to_string(), Some(900_000)),
        ]);

        let total = portfolio_total(&summaries, &rates, "EUR");

        assert_eq!(total.total_global_value, 145_000_000);
        assert_eq!(total.total_unrealized_pnl, None);
        assert!(!total.incomplete);
    }

    // ACC-027 — no account: a zero, complete total with no P&L.
    #[test]
    fn portfolio_total_of_no_account_is_zero_and_complete() {
        let total = portfolio_total(&[], &HashMap::new(), "EUR");

        assert_eq!(total.total_global_value, 0);
        assert_eq!(total.total_unrealized_pnl, None);
        assert_eq!(total.currency, "EUR");
        assert!(!total.incomplete);
    }

    // ACC-027/028 — the response carries the total built from its rows: a EUR
    // account counts at identity, and an empty foreign account with no usable rate
    // hides no figure, so the total stays complete.
    #[tokio::test]
    async fn summaries_carry_a_portfolio_total_complete_despite_an_empty_unconvertible_account() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        for (name, currency) in [("Main", "EUR"), ("US", "USD")] {
            account_svc
                .create(name.to_string(), String::new(), currency.to_string(), false)
                .await
                .unwrap();
        }
        let uc = AccountSummaryUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let response = uc.get_account_summaries().await.unwrap();

        assert_eq!(response.summaries.len(), 2);
        assert_eq!(response.total.currency, "EUR");
        assert_eq!(response.total.total_global_value, 0);
        assert!(
            !response.total.incomplete,
            "an empty USD account with no rate to EUR hides no figure"
        );
    }

    // ACC-028 — an unconvertible account leaves the total incomplete only when its
    // zero hides a figure: an empty one does not; one carrying a P&L does.
    #[test]
    fn portfolio_total_is_incomplete_only_when_an_unconvertible_account_hides_a_figure() {
        let rates = HashMap::from([
            ("EUR".to_string(), Some(1_000_000)),
            ("USD".to_string(), None),
        ]);

        let with_empty = portfolio_total(
            &[summary("EUR", 100_000_000, None), summary("USD", 0, None)],
            &rates,
            "EUR",
        );
        assert!(!with_empty.incomplete);
        assert_eq!(with_empty.total_global_value, 100_000_000);

        // ACC-029 — the only P&L sits in the unconvertible account: absent and incomplete.
        let with_hidden_pnl = portfolio_total(
            &[
                summary("EUR", 100_000_000, None),
                summary("USD", 0, Some(-5_000_000)),
            ],
            &rates,
            "EUR",
        );
        assert!(with_hidden_pnl.incomplete);
        assert_eq!(with_hidden_pnl.total_unrealized_pnl, None);
    }
}
