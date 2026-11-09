use crate::context::account::{AccountError, AccountServiceContract};
use crate::context::asset::AssetServiceContract;
use crate::context::currency::CurrencyService;
use crate::use_cases::shared::global_performance::PerformanceAggregator;
use crate::use_cases::shared::performance::{
    account_performance_series, AccountPerformanceResponse,
};
use std::result::Result as StdResult;
use std::sync::Arc;

/// Orchestrates the portfolio-wide performance read (GPF spec): all accounts —
/// or one asset's positions across all accounts — aggregated in the reference
/// currency, with the single-account scopes served by the shared performance
/// series engine (B18, ADR-013).
pub struct GlobalPerformanceUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    currency_service: Arc<CurrencyService>,
    aggregator: PerformanceAggregator,
}

impl GlobalPerformanceUseCase {
    /// Creates a new use case instance. The currency service resolves both the
    /// per-holding valuation rates (FXR-042/035) and the account-currency →
    /// reference-currency conversion (GPF-020/030).
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            aggregator: PerformanceAggregator::new(
                account_service.clone(),
                asset_service.clone(),
                Arc::clone(&currency_service),
            ),
            account_service,
            asset_service,
            currency_service,
        }
    }

    /// Computes per-period performance for the requested scope (GPF-010): with
    /// an `account_id` the read is the single-account series of
    /// `get_account_performance` (PRF-010–084) for that account and optional
    /// asset; without one, every account — or every position of the scoped
    /// asset — is aggregated in the reference currency (GPF-011–041).
    pub async fn get_global_performance(
        &self,
        account_id: Option<&str>,
        asset_id: Option<&str>,
    ) -> StdResult<AccountPerformanceResponse, AccountError> {
        match account_id {
            Some(account_id) => {
                account_performance_series(
                    self.account_service.as_ref(),
                    self.asset_service.as_ref(),
                    &self.currency_service,
                    account_id,
                    asset_id,
                )
                .await
            }
            None => {
                let accounts = self.account_service.accounts_in_use().await?;
                self.aggregator
                    .aggregate_across_accounts(accounts, asset_id)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use crate::context::account::{Transaction, TransactionType};
    use crate::context::asset::{
        AssetService, CreateAssetDTO, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use crate::context::currency::domain::{
        CurrencyRate, CurrencyRateSource, MockCurrencyPairRepository, MockCurrencyRateRepository,
    };
    use crate::shared::domain::quantity::from_surface;
    use crate::use_cases::account_performance::AccountPerformanceUseCase;
    use crate::use_cases::shared::global_performance::reference_rate_dates;
    use crate::use_cases::shared::performance::PerformancePeriod;
    use crate::use_cases::shared::valuation::parse_date;
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

    fn make_currency_service_with_no_rate() -> Arc<CurrencyService> {
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0..)
            .returning(|_, _, _| Ok(None));
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ))
    }

    // USD→EUR observations: 0.80 before June 2024, 0.90 from June 2024 onward —
    // lets a test distinguish the transaction-date conversion of flows (GPF-030)
    // from the period-end conversion of end values (GPF-020).
    fn make_currency_service_with_usd_rate_by_date() -> Arc<CurrencyService> {
        let mut rate_repo = MockCurrencyRateRepository::new();
        rate_repo
            .expect_latest_rate_on_or_before()
            .times(0..)
            .returning(|from_currency, to_currency, as_of| {
                let rate_micros = if as_of < "2024-06-01" {
                    800_000
                } else {
                    900_000
                };
                Ok(Some(CurrencyRate::from_storage(
                    from_currency.to_string(),
                    to_currency.to_string(),
                    as_of.to_string(),
                    rate_micros,
                    CurrencyRateSource::Manual,
                )))
            });
        Arc::new(CurrencyService::new(
            Box::new(MockCurrencyPairRepository::new()),
            Box::new(rate_repo),
        ))
    }

    async fn create_account(
        account_svc: &AccountService,
        name: &str,
        currency: &str,
    ) -> crate::context::account::Account {
        account_svc
            .create(name.to_string(), String::new(), currency.to_string(), false)
            .await
            .expect("account created")
    }

    async fn create_stock(asset_svc: &AssetService, name: &str, reference: &str) -> String {
        asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: name.to_string(),
                reference: reference.to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "EUR".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("stock created")
            .id
    }

    fn year_row(response: &AccountPerformanceResponse, year: i32) -> &PerformancePeriod {
        response
            .yearly
            .iter()
            .find(|row| row.year == year)
            .expect("year row present")
    }

    // TRF-028, GPF-030 — a transfer between accounts of two currencies is no flow of the
    // portfolio: the since-inception gain is the end value less the deposit alone, though
    // the receiving side's date has no reference rate read for it.
    #[tokio::test]
    async fn trf_028_a_transfer_between_currencies_is_no_portfolio_flow() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let stock = create_stock(&asset_svc, "Stock", "STK").await;
        let sending = create_account(&account_svc, "Sending", "EUR").await;
        let receiving = create_account(&account_svc, "Receiving", "USD").await;
        account_svc
            .record_deposit(&sending.id, "2024-02-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &sending.id,
                stock.clone(),
                "2024-02-10".to_string(),
                from_surface(10_000_000),
                50_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        account_svc
            .record_transfer(
                &sending.id,
                &receiving.id,
                stock.clone(),
                "2024-07-01".to_string(),
                from_surface(4_000_000),
                from_surface(4_000_000),
                1_250_000,
                None,
            )
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_usd_rate_by_date(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(row.cash_flow, 1_000_000_000, "only the deposit");
        assert_eq!(row.asset_flow, 0, "GPF-040: the transfer is no asset flow");
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(
            since_inception.gain,
            row.end_value - 1_000_000_000,
            "net invested is the deposit alone"
        );
    }

    // GPF-020 — two reference-currency accounts sum exactly, with no conversion.
    #[tokio::test]
    async fn two_reference_currency_accounts_sum_into_one_series() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR").await;
        let second = create_account(&account_svc, "Second", "EUR").await;
        account_svc
            .record_deposit(&first.id, "2024-03-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&second.id, "2024-05-01".to_string(), 500_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert_eq!(response.account_name, "", "GPF-011: no backend label");
        assert_eq!(response.currency, "EUR", "GPF-011: reference currency");
        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 1_500_000_000,
            "end values of both accounts sum exactly"
        );
        assert_eq!(row.cash_flow, 1_500_000_000, "both deposits in the period");
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 0, "deposits only → no gain");
    }

    // GPF-020/030 — a foreign account's end value converts at the period-end
    // rate (0.90) while its deposit converts at the transaction-date rate
    // (0.80); the difference is currency movement and lands in the pnl residual.
    #[tokio::test]
    async fn foreign_account_converts_end_value_at_period_end_and_flows_at_transaction_date() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let account = create_account(&account_svc, "Foreign", "USD").await;
        account_svc
            .record_deposit(&account.id, "2024-03-15".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_usd_rate_by_date(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 900_000_000,
            "1000 USD cash × 0.90 (rate as of 2024-12-31)"
        );
        assert_eq!(
            row.cash_flow, 800_000_000,
            "1000 USD deposit × 0.80 (rate as of 2024-03-15)"
        );
        assert_eq!(row.asset_flow, 0);
        assert_eq!(row.dividends, 0);
        assert_eq!(
            row.pnl, 100_000_000,
            "currency movement is the bridge residual"
        );
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.dividends + row.pnl,
            "GPF-041 bridge identity balances"
        );
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(
            since_inception.gain, 100_000_000,
            "gain vs converted net invested"
        );
    }

    // GPF-020/030 — a foreign account with no usable rate contributes 0 to every
    // period and its flows are dropped (FXR-034 spirit).
    #[tokio::test]
    async fn foreign_account_without_usable_rate_contributes_zero() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let domestic = create_account(&account_svc, "Domestic", "EUR").await;
        let foreign = create_account(&account_svc, "Foreign", "USD").await;
        account_svc
            .record_deposit(&domestic.id, "2024-02-10".to_string(), 500_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&foreign.id, "2024-03-15".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 500_000_000,
            "only the EUR account contributes"
        );
        assert_eq!(
            row.cash_flow, 500_000_000,
            "the unconvertible USD deposit contributes 0"
        );
        assert_eq!(row.pnl, 0, "no residual from the excluded contribution");
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 0);
    }

    // GPF-014 — the monthly series exists for every aggregation (PRF-013).
    #[tokio::test]
    async fn the_monthly_series_spans_every_included_account() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR").await;
        let second = create_account(&account_svc, "Second", "EUR").await;
        account_svc
            .record_deposit(&first.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&second.id, "2024-04-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert!(!response.monthly.is_empty());

        // PRF-042 — the first month has no month before it to compare with; every
        // other month has, including the same month of a later year.
        for row in &response.monthly {
            let is_first = (row.year, row.month) == (2024, Some(3));
            assert_eq!(
                row.period_over_period.is_none(),
                is_first,
                "{}-{:?}",
                row.year,
                row.month
            );
        }
        assert!(response
            .monthly
            .iter()
            .any(|row| row.year == 2025 && row.month == Some(3)));
        assert!(response
            .monthly
            .iter()
            .any(|row| row.year == 2024 && row.month == Some(4)));
    }

    // GPF-012 — an account with no in-scope transactions is excluded from the
    // aggregation: the series start at the active account's first month.
    #[tokio::test]
    async fn accounts_without_in_scope_transactions_are_excluded() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let active = create_account(&account_svc, "Active", "EUR").await;
        create_account(&account_svc, "Dormant", "EUR").await;
        account_svc
            .record_deposit(&active.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert_eq!(
            response.monthly.last().map(|row| (row.year, row.month)),
            Some((2024, Some(3)))
        );
    }

    // GPF-010 — (Some, None) returns the exact single-account read.
    #[tokio::test]
    async fn account_scoped_read_matches_account_performance() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = create_account(&account_svc, "Delegated", "EUR").await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 12_500_000_000, None)
            .await
            .unwrap();
        let stock = create_stock(&asset_svc, "Delegated Stock", "DLG").await;
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-01".to_string(),
                from_surface(10_000_000),
                1_000_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 1350.0)
            .await
            .unwrap();
        account_svc
            .record_dividend(
                &account.id,
                stock.clone(),
                "2024-06-01".to_string(),
                200_000_000,
                1_000_000,
                None,
            )
            .await
            .unwrap();

        let account_uc = AccountPerformanceUseCase::new(
            account_svc.clone(),
            asset_svc.clone(),
            make_currency_service_with_no_rate(),
        );
        let global_uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let expected = account_uc
            .get_account_performance(&account.id, None)
            .await
            .unwrap();
        let actual = global_uc
            .get_global_performance(Some(&account.id), None)
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap(),
            "account-scoped global read must equal get_account_performance"
        );
    }

    // GPF-010 — (Some, Some) returns the exact single-account asset-scoped read.
    #[tokio::test]
    async fn account_and_asset_scoped_read_matches_account_performance() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = create_account(&account_svc, "Delegated", "EUR").await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 12_500_000_000, None)
            .await
            .unwrap();
        let stock = create_stock(&asset_svc, "Delegated Stock", "DLG").await;
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-01".to_string(),
                from_surface(10_000_000),
                1_000_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 1350.0)
            .await
            .unwrap();

        let account_uc = AccountPerformanceUseCase::new(
            account_svc.clone(),
            asset_svc.clone(),
            make_currency_service_with_no_rate(),
        );
        let global_uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let expected = account_uc
            .get_account_performance(&account.id, Some(&stock))
            .await
            .unwrap();
        let actual = global_uc
            .get_global_performance(Some(&account.id), Some(&stock))
            .await
            .unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&expected).unwrap(),
            "asset-scoped global read must equal get_account_performance"
        );
    }

    // #495, PRF-083 — across accounts, a window the position was held at the start of in
    // no included account has no figure: the stock is bought 2024-03-01 in one account
    // and 2025-02-03 in another, so its 2024 month rows carry no year-to-date figure,
    // while 2025's do (held on 2024-12-31 in the first account).
    #[tokio::test]
    async fn cross_account_asset_scope_has_no_figure_for_a_window_not_held_at_its_start() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let stock = create_stock(&asset_svc, "Shared Stock", "SHR").await;
        for (name, date) in [("First", "2024-03-01"), ("Second", "2025-02-03")] {
            let account = create_account(&account_svc, name, "EUR").await;
            account_svc
                .record_deposit(&account.id, date.to_string(), 2_000_000_000, None)
                .await
                .unwrap();
            account_svc
                .buy_holding(
                    &account.id,
                    stock.clone(),
                    date.to_string(),
                    from_surface(10_000_000),
                    100_000_000,
                    1_000_000,
                    0,
                    None,
                    None,
                )
                .await
                .unwrap();
        }
        asset_svc
            .record_asset_price(&stock, "2024-12-31", 110.0)
            .await
            .unwrap();
        let global_uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );

        let scoped = global_uc
            .get_global_performance(None, Some(&stock))
            .await
            .unwrap();
        for row in scoped.monthly.iter().filter(|row| row.year == 2024) {
            assert!(
                row.year_to_date.is_none(),
                "2024-{:?}: held nowhere on 2023-12-31, no year-to-date figure",
                row.month
            );
        }
        let march_2025 = scoped
            .monthly
            .iter()
            .find(|row| row.year == 2025 && row.month == Some(3))
            .expect("March 2025 row");
        assert!(
            march_2025.year_to_date.is_some(),
            "held on 2024-12-31 in the first account: the year-to-date figure stands"
        );
        let april_2024 = scoped
            .monthly
            .iter()
            .find(|row| row.year == 2024 && row.month == Some(4))
            .expect("April 2024 row");
        assert!(
            april_2024.since_inception.is_some(),
            "since-inception stays across accounts"
        );
    }

    // GPF-020/030 (asset scope) — the scoped read sums the asset's positions
    // across every account holding it.
    #[tokio::test]
    async fn asset_scope_sums_positions_across_accounts() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let first = create_account(&account_svc, "First", "EUR").await;
        let second = create_account(&account_svc, "Second", "EUR").await;
        let stock = create_stock(&asset_svc, "Shared Stock", "SHR").await;
        for account_id in [&first.id, &second.id] {
            account_svc
                .record_deposit(account_id, "2024-03-01".to_string(), 2_000_000_000, None)
                .await
                .unwrap();
            account_svc
                .buy_holding(
                    account_id,
                    stock.clone(),
                    "2024-03-01".to_string(),
                    from_surface(10_000_000),
                    100_000_000,
                    1_000_000,
                    0,
                    None,
                    None,
                )
                .await
                .unwrap();
        }
        asset_svc
            .record_asset_price(&stock, "2024-06-30", 120.0)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, Some(&stock)).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(
            row.end_value, 2_400_000_000,
            "2 positions × 10 units × 120 EUR"
        );
        assert_eq!(
            row.cash_flow, 2_000_000_000,
            "both purchases are position inflows (PRF-084)"
        );
        assert_eq!(row.pnl, 400_000_000, "price appreciation across positions");
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.pnl,
            "scoped bridge identity (dividends outside, PRF-084)"
        );
        let since_inception = row.since_inception.as_ref().expect("since_inception");
        assert_eq!(since_inception.gain, 400_000_000);
    }

    // GPF-015 — an empty portfolio produces the PRF-043-shaped empty response.
    #[tokio::test]
    async fn empty_portfolio_returns_empty_reference_currency_response() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        assert_eq!(response.account_name, "");
        assert_eq!(response.currency, "EUR");
        assert!(response.yearly.is_empty());
        assert!(response.monthly.is_empty());
    }

    // GPF-011 — the global performance is reported in the reference currency chosen: a
    // CHF account's deposit counts in full, with no rate, in a CHF series.
    #[tokio::test]
    async fn gpf_011_the_global_performance_is_in_the_reference_currency_chosen() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("CHF").await.unwrap();
        let account = create_account(&account_svc, "Swiss", "CHF").await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 100_000_000, None)
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

        let uc = GlobalPerformanceUseCase::new(account_svc, asset_svc, currency_svc);
        let response = uc.get_global_performance(None, None).await.unwrap();
        assert_eq!(response.currency, "CHF");
        let row = response
            .yearly
            .iter()
            .find(|row| row.year == 2024)
            .expect("a 2024 row");
        assert_eq!((row.cash_flow, row.end_value), (100_000_000, 100_000_000));
    }

    // GPF-012/015 — an asset scope no account holds excludes every account and
    // yields the empty response.
    #[tokio::test]
    async fn unknown_asset_scope_returns_empty_response() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let account = create_account(&account_svc, "Holder", "EUR").await;
        account_svc
            .record_deposit(&account.id, "2024-03-01".to_string(), 100_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc
            .get_global_performance(None, Some("missing-asset"))
            .await
            .unwrap();

        assert!(response.yearly.is_empty());
        assert!(response.monthly.is_empty());
        assert_eq!(response.currency, "EUR");
    }

    // GPF-013 — the aggregation spans from the earliest transaction across all
    // included accounts; an account contributes 0 to every period before its
    // own first transaction.
    #[tokio::test]
    async fn later_starting_account_contributes_zero_before_its_first_transaction() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("EUR").await.unwrap();
        let older = create_account(&account_svc, "Older", "EUR").await;
        let newer = create_account(&account_svc, "Newer", "EUR").await;
        account_svc
            .record_deposit(&older.id, "2023-03-01".to_string(), 500_000_000, None)
            .await
            .unwrap();
        account_svc
            .record_deposit(&newer.id, "2025-02-01".to_string(), 1_000_000_000, None)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_no_rate(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row_2023 = year_row(&response, 2023);
        assert_eq!(
            row_2023.end_value, 500_000_000,
            "before the newer account's start, the older account's value stands alone"
        );
        assert_eq!(row_2023.cash_flow, 500_000_000);
        let row_2024 = year_row(&response, 2024);
        assert_eq!(
            row_2024.end_value, 500_000_000,
            "the newer account still contributes 0 in 2024"
        );
        assert_eq!(row_2024.cash_flow, 0, "no flows dated 2024");
        let row_2025 = year_row(&response, 2025);
        assert_eq!(
            row_2025.end_value, 1_500_000_000,
            "both accounts contribute from the newer account's first transaction on"
        );
        assert_eq!(row_2025.cash_flow, 1_000_000_000);
    }

    // GPF-030/040 — the reference-rate date set only contains flow-relevant
    // dates per scope: Deposit/Withdrawal convert only unscoped, Purchase/Sell
    // only scoped, ManagementFee never.
    #[test]
    fn reference_rate_dates_narrow_to_flow_relevant_types_per_scope() {
        let tx = |transaction_type: TransactionType, date: &str| {
            Transaction::new(
                "acc".to_string(),
                "asset".to_string(),
                transaction_type,
                date.to_string(),
                from_surface(1_000_000),
                1_000_000,
                1_000_000,
                0,
                1_000_000,
                None,
                None,
            )
            .unwrap()
        };
        let transactions = vec![
            tx(TransactionType::Deposit, "2024-01-01"),
            tx(TransactionType::Purchase, "2024-02-01"),
            tx(TransactionType::Sell, "2024-03-01"),
            tx(TransactionType::Dividend, "2024-04-01"),
            tx(TransactionType::ManagementFee, "2024-05-01"),
            tx(TransactionType::OpeningBalance, "2024-06-01"),
            tx(TransactionType::FreeShares, "2024-07-01"),
            tx(TransactionType::Interest, "2024-08-01"),
        ];
        let date = |raw: &str| parse_date(raw).unwrap();

        let unscoped = reference_rate_dates(&transactions, None);
        assert!(unscoped.contains(&date("2024-01-01")), "deposit converts");
        assert!(
            !unscoped.contains(&date("2024-02-01")) && !unscoped.contains(&date("2024-03-01")),
            "unscoped trades convert nothing"
        );
        assert!(unscoped.contains(&date("2024-04-01")), "dividend converts");
        assert!(
            !unscoped.contains(&date("2024-05-01")),
            "a management fee is never a flow"
        );
        assert!(
            unscoped.contains(&date("2024-06-01")),
            "opening balance converts at its own date"
        );
        for scope in [None, Some("asset")] {
            let dates = reference_rate_dates(&transactions, scope);
            assert!(
                !dates.contains(&date("2024-07-01")) && !dates.contains(&date("2024-08-01")),
                "free shares and interest are earned, no flow to convert"
            );
        }

        let scoped = reference_rate_dates(&transactions, Some("asset"));
        assert!(
            !scoped.contains(&date("2024-01-01")),
            "deposits never touch a position"
        );
        assert!(
            scoped.contains(&date("2024-02-01")) && scoped.contains(&date("2024-03-01")),
            "scoped trade flows convert at their own date"
        );
    }

    // GPF-040/PRF-071 — a foreign account's free shares are no flow in the
    // reference currency either: their value, at the period-end rate (0.90) as
    // the end value has it, lands in pnl.
    #[tokio::test]
    async fn foreign_free_shares_are_a_gain_in_the_reference_currency() {
        let pool = make_pool().await;
        let (account_svc, asset_svc) = setup(&pool).await;
        asset_svc.seed_cash_asset("USD").await.unwrap();
        let account = create_account(&account_svc, "Foreign", "USD").await;
        // A USD-currency stock inside the USD account: no asset→account
        // conversion, so the only FX leg is USD→EUR (GPF-020/030/040).
        let stock = asset_svc
            .create_asset(CreateAssetDTO {
                kind: None,
                name: "US Stock".to_string(),
                reference: "UST".to_string(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".to_string(),
                risk_level: 1,
                category_id: SYSTEM_CATEGORY_ID.to_string(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("stock created")
            .id;
        account_svc
            .record_deposit(&account.id, "2024-03-15".to_string(), 2_000_000_000, None)
            .await
            .unwrap();
        account_svc
            .buy_holding(
                &account.id,
                stock.clone(),
                "2024-03-15".to_string(),
                from_surface(10_000_000),
                100_000_000,
                1_000_000,
                0,
                None,
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-20", 100.0)
            .await
            .unwrap();
        account_svc
            .record_free_shares(
                &account.id,
                stock.clone(),
                "2024-03-20".to_string(),
                from_surface(5_000_000),
                None,
            )
            .await
            .unwrap();
        asset_svc
            .record_asset_price(&stock, "2024-03-31", 100.0)
            .await
            .unwrap();

        let uc = GlobalPerformanceUseCase::new(
            account_svc,
            asset_svc,
            make_currency_service_with_usd_rate_by_date(),
        );
        let response = uc.get_global_performance(None, None).await.unwrap();

        let row = year_row(&response, 2024);
        assert_eq!(row.asset_flow, 0, "free shares are no in-kind contribution");
        // 2000 USD deposit × 0.80 (rate as of 2024-03-15).
        assert_eq!(row.cash_flow, 1_600_000_000);
        // Cash 1000 USD + 15 shares × $100 = 2500 USD × 0.90 (period-end rate).
        assert_eq!(row.end_value, 2_250_000_000);
        assert_eq!(row.dividends, 0);
        assert_eq!(row.pnl, 650_000_000, "the free shares' value is in the P&L");
        assert_eq!(
            row.end_value,
            row.previous_value + row.cash_flow + row.asset_flow + row.dividends + row.pnl,
            "GPF-041 bridge identity balances"
        );
    }
}
