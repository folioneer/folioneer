use crate::context::account::{AccountError, AccountServiceContract};
use crate::context::asset::{Asset, AssetError, AssetServiceContract};
use crate::context::currency::CurrencyService;
use crate::use_cases::shared::scope::build_fx_pairs;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::dispatcher::Dispatcher;
use super::error::{FetchAccountAssetPricesError, FetchAllAssetPricesError, FetchPriceTask};
use super::guard::FetchGuard;
use super::movement_capture::PriceMovementCapture;
use super::trigger::FetchTrigger;

/// Orchestrates the asset-price fetch tasks — `fetch_all` (MKT-122 / MKT-130) and
/// `fetch_for_account` (MKT-132 / MKT-131). Both methods share the same in-flight
/// guard, dispatcher, and scope-building logic.
pub struct AssetPriceFetchUseCase {
    account_service: Arc<dyn AccountServiceContract>,
    asset_service: Arc<dyn AssetServiceContract>,
    fetch_guard: Arc<FetchGuard>,
    dispatcher: Arc<Dispatcher>,
    /// The frozen-rate source the Price Movement baseline resolves against
    /// (PMV-020) — the same service the task's FX refresh (FXR-075) later
    /// writes through, which is why the baseline captures before the loop.
    currency_service: Arc<CurrencyService>,
}

impl AssetPriceFetchUseCase {
    /// Creates a new use case instance.
    pub fn new(
        account_service: Arc<dyn AccountServiceContract>,
        asset_service: Arc<dyn AssetServiceContract>,
        fetch_guard: Arc<FetchGuard>,
        dispatcher: Arc<Dispatcher>,
        currency_service: Arc<CurrencyService>,
    ) -> Self {
        Self {
            account_service,
            asset_service,
            fetch_guard,
            dispatcher,
            currency_service,
        }
    }

    /// Runs the all-accounts fetch task:
    /// (a) acquire guard or return `FetchAlreadyRunning`;
    /// (b) load all active holdings across all accounts;
    /// (c) filter system cash assets (MKT-116);
    /// (d) derive Yahoo symbols, discard non-derivable entries;
    /// (e) if empty scope → `NoFetchableHoldings` (MKT-111);
    /// (f) dispatch background task and return `Ok(())`.
    ///
    /// `trigger` states which action started this fetch (PMV-010/015): only a
    /// `Manual` trigger produces a Price Movement report. A rejection below
    /// (`FetchAlreadyRunning`, `NoFetchableHoldings`) returns BEFORE `spawn` is
    /// ever reached, so no `AssetPriceFetchCompleted` is published and no
    /// report exists either way (PMV-012).
    pub async fn fetch_all(&self, trigger: FetchTrigger) -> Result<(), FetchAllAssetPricesError> {
        let lease = self
            .fetch_guard
            .try_acquire()
            .ok_or(FetchPriceTask::FetchAlreadyRunning)?;

        let accounts = self.account_service.get_all().await?;

        let mut asset_ids: HashSet<String> = HashSet::new();
        // FXR-071 — collect (account_currency, asset_id) for every active holding so
        // foreign pairs can be derived, including assets with no fetchable price.
        let mut fx_inputs: Vec<(String, String)> = Vec::new();
        for account in &accounts {
            let holdings = self
                .account_service
                .get_holdings_for_account(&account.id)
                .await?;
            for holding in holdings {
                if holding.quantity > 0 {
                    fx_inputs.push((account.currency.clone(), holding.asset_id.clone()));
                    asset_ids.insert(holding.asset_id);
                }
            }
        }

        let (scope, currency_by_asset) = self.build_scope(asset_ids).await?;
        if scope.is_empty() {
            return Err(FetchPriceTask::NoFetchableHoldings.into());
        }

        let fx_pairs = build_fx_pairs(fx_inputs, &currency_by_asset);
        // PMV-010/015 — only a user-started Global refresh reports movement. The
        // capture is handed over unawaited: it runs inside the spawned task, before
        // any price is written (still refresh start, PMV-020) and before the FX
        // refresh that piggybacks on it (FXR-075), so this command keeps
        // acknowledging immediately (MKT-130).
        let movement_capture = match trigger {
            FetchTrigger::Manual => Some(Arc::new(PriceMovementCapture::new(
                Arc::clone(&self.account_service),
                Arc::clone(&self.asset_service),
                Arc::clone(&self.currency_service),
            ))),
            FetchTrigger::Launch => None,
        };
        Arc::clone(&self.dispatcher).spawn(scope, fx_pairs, lease, movement_capture);
        Ok(())
    }

    /// Runs the per-account fetch task:
    /// (a) check account exists via `account_service.get_by_id`, else `AccountNotFound` (MKT-132);
    /// (b) acquire guard or return `FetchAlreadyRunning` (MKT-113);
    /// (c) load holdings for the account;
    /// (d) filter system cash assets (MKT-116);
    /// (e) derive Yahoo symbols, discard non-derivable entries;
    /// (f) if empty scope → `NoFetchableHoldings` (MKT-111);
    /// (g) dispatch background task and return `Ok(())`.
    pub async fn fetch_for_account(
        &self,
        account_id: &str,
    ) -> Result<(), FetchAccountAssetPricesError> {
        let account = self
            .account_service
            .get_by_id(account_id)
            .await?
            .ok_or_else(|| {
                FetchAccountAssetPricesError::Account(AccountError::AccountNotFound {
                    account_id: account_id.to_string(),
                })
            })?;

        let lease = self
            .fetch_guard
            .try_acquire()
            .ok_or(FetchPriceTask::FetchAlreadyRunning)?;

        let holdings = self
            .account_service
            .get_holdings_for_account(&account.id)
            .await?;
        let mut asset_ids: HashSet<String> = HashSet::new();
        // FXR-071 — pairs for this account's active foreign holdings.
        let mut fx_inputs: Vec<(String, String)> = Vec::new();
        for holding in holdings.into_iter().filter(|holding| holding.quantity > 0) {
            fx_inputs.push((account.currency.clone(), holding.asset_id.clone()));
            asset_ids.insert(holding.asset_id);
        }

        let (scope, currency_by_asset) = self.build_scope(asset_ids).await?;
        if scope.is_empty() {
            return Err(FetchPriceTask::NoFetchableHoldings.into());
        }

        let fx_pairs = build_fx_pairs(fx_inputs, &currency_by_asset);
        // PMV-010 — an account-scoped fetch never reports movement; `Launch` is
        // the trigger value that never produces a report, same as the launch
        // auto-fetch. `fetch_for_account` carries no trigger of its own on the
        // wire (unchanged contract) — this is purely an internal `spawn` value.
        Arc::clone(&self.dispatcher).spawn(scope, fx_pairs, lease, None);
        Ok(())
    }

    /// Builds the auto-fetch scope via the shared builder (MKT-116/151
    /// exclusions live in `use_cases::shared::scope`).
    async fn build_scope(
        &self,
        asset_ids: HashSet<String>,
    ) -> Result<(Vec<(Asset, String)>, HashMap<String, String>), AssetError> {
        crate::use_cases::shared::scope::build_scope(self.asset_service.as_ref(), asset_ids).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use crate::context::asset::{
        AssetService, MockAssetRepository, MockPriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository,
    };
    use crate::context::currency::{
        CurrencyPair, CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
    };
    use crate::core::cash::system_cash_asset_id;
    use crate::core::SideEffectEventBus;
    use chrono::NaiveDate;
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

    /// Builds a use case whose asset lookups are driven by `asset_repo`. The account
    /// service and dispatcher are real but inert — `build_scope` only touches the
    /// asset service, so a mocked asset repository fully controls every branch.
    fn build_use_case(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        asset_repo: MockAssetRepository,
    ) -> AssetPriceFetchUseCase {
        let bus = Arc::new(SideEffectEventBus::new());
        let account_service = Arc::new(AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        ));
        let asset_service = Arc::new(AssetService::new(
            Box::new(asset_repo),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        ));
        let currency_service = Arc::new(CurrencyService::new(
            Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
            Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
        ));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(MockPriceProvider::new()),
            Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
            Arc::clone(&bus),
            Arc::clone(&currency_service),
            Arc::new(|| NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date")),
        ));
        AssetPriceFetchUseCase::new(
            account_service,
            asset_service,
            Arc::new(FetchGuard::new()),
            dispatcher,
            currency_service,
        )
    }

    fn pairs_as_tuples(pairs: &[CurrencyPair]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|pair| (pair.from_currency.clone(), pair.to_currency.clone()))
            .collect()
    }

    // FXR-071 — a foreign holding yields one (asset_currency → account_currency) pair.
    #[test]
    fn build_fx_pairs_creates_pair_for_foreign_holding() {
        let currency_by_asset = HashMap::from([("asset-usd".to_string(), "USD".to_string())]);
        let pairs = build_fx_pairs(
            vec![("EUR".to_string(), "asset-usd".to_string())],
            &currency_by_asset,
        );
        assert_eq!(
            pairs_as_tuples(&pairs),
            vec![("USD".to_string(), "EUR".to_string())]
        );
    }

    // FXR-013 — a holding whose currency equals the account currency yields no pair.
    #[test]
    fn build_fx_pairs_skips_same_currency_holding() {
        let currency_by_asset = HashMap::from([("asset-eur".to_string(), "EUR".to_string())]);
        let pairs = build_fx_pairs(
            vec![("EUR".to_string(), "asset-eur".to_string())],
            &currency_by_asset,
        );
        assert!(
            pairs.is_empty(),
            "same-currency holding must not yield a pair"
        );
    }

    // FXR-071 — a cash holding is filtered before the currency map is consulted.
    #[test]
    fn build_fx_pairs_skips_cash_holding() {
        let cash_id = system_cash_asset_id("USD");
        let pairs = build_fx_pairs(vec![("EUR".to_string(), cash_id)], &HashMap::new());
        assert!(pairs.is_empty(), "cash holding must not yield a pair");
    }

    // FXR-071 — a holding whose asset is absent from the map is skipped without error.
    #[test]
    fn build_fx_pairs_skips_missing_asset() {
        let pairs = build_fx_pairs(
            vec![("EUR".to_string(), "ghost".to_string())],
            &HashMap::new(),
        );
        assert!(pairs.is_empty(), "missing asset must not yield a pair");
    }

    // FXR-071 — two foreign holdings resolving to the same pair are de-duplicated.
    #[test]
    fn build_fx_pairs_dedups_repeated_pair() {
        let currency_by_asset = HashMap::from([
            ("asset-a".to_string(), "USD".to_string()),
            ("asset-b".to_string(), "USD".to_string()),
        ]);
        let pairs = build_fx_pairs(
            vec![
                ("EUR".to_string(), "asset-a".to_string()),
                ("EUR".to_string(), "asset-b".to_string()),
            ],
            &currency_by_asset,
        );
        assert_eq!(
            pairs_as_tuples(&pairs),
            vec![("USD".to_string(), "EUR".to_string())],
            "the same pair from two assets must appear once"
        );
    }

    // MKT-116 — build_scope surfaces a repository failure as a typed DatabaseError
    // (the asset-load error arm shared with the FX-pair path).
    #[tokio::test]
    async fn build_scope_returns_database_error_when_load_fails() {
        let pool = make_pool().await;
        let mut asset_repo = MockAssetRepository::new();
        asset_repo
            .expect_get_by_id()
            .returning(|_| Err(anyhow::anyhow!("simulated repository failure")));
        let use_case = build_use_case(&pool, asset_repo);

        let mut asset_ids = HashSet::new();
        asset_ids.insert("asset-x".to_string());
        let error = use_case
            .build_scope(asset_ids)
            .await
            .expect_err("must surface a typed error");

        assert!(
            matches!(error, AssetError::DatabaseError),
            "repository failure must map to DatabaseError, got: {error:?}"
        );
    }

    // PMV-012 — a refresh rejected before any asset is attempted (here,
    // NoFetchableHoldings on an empty DB) never reaches `spawn`, so no
    // AssetPriceFetchCompleted — or any other event — is published, and no
    // report exists either way.
    #[tokio::test]
    async fn fetch_all_publishes_no_event_when_rejected_before_dispatch() {
        let pool = make_pool().await;
        let bus = Arc::new(SideEffectEventBus::new());
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
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(MockPriceProvider::new()),
            Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
            Arc::clone(&bus),
            Arc::clone(&currency_service),
            Arc::new(|| NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date")),
        ));
        let use_case = AssetPriceFetchUseCase::new(
            account_service,
            asset_service,
            Arc::new(FetchGuard::new()),
            dispatcher,
            currency_service,
        );

        let mut rx = bus.subscribe();
        let result = use_case.fetch_all(FetchTrigger::Manual).await;

        assert!(
            matches!(
                result,
                Err(FetchAllAssetPricesError::Failure(
                    FetchPriceTask::NoFetchableHoldings
                ))
            ),
            "an empty DB must reject before dispatch, got: {result:?}"
        );
        // No event was published: `changed()` must not resolve within a short window.
        let outcome =
            tokio::time::timeout(std::time::Duration::from_millis(200), rx.changed()).await;
        assert!(
            outcome.is_err(),
            "no AssetPriceFetchCompleted (or any other event) may be published on a pre-dispatch rejection"
        );
    }
}
