//! Integration tests for the asset-price fetch orchestrator (MKT-122, MKT-132, MKT-111, MKT-113).
//!
//! These tests exercise the full stack through the public API: orchestrator constructor →
//! AccountService / AssetService → real in-memory SQLite. No mocks — per test_convention.md
//! Tier 3 constraint.

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
};
use folioneer_lib::context::asset::{
    AssetService, SqliteAssetCategoryRepository, SqliteAssetPriceRepository, SqliteAssetRepository,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::asset_price_fetch::{AssetPriceFetchUseCase, FetchGuard};
use std::sync::Arc;

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

struct Ctx {
    use_case: AssetPriceFetchUseCase,
    fetch_guard: Arc<FetchGuard>,
}

async fn build_ctx() -> Ctx {
    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());

    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let fetch_guard = Arc::new(FetchGuard::new());

    let use_case = {
        use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

        struct NoOpProvider;
        #[async_trait::async_trait]
        impl folioneer_lib::context::asset::PriceProvider for NoOpProvider {
            async fn fetch_price(
                &self,
                _symbol: &str,
            ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
                Ok(Some(folioneer_lib::context::asset::Quote {
                    price: 100_000_000,
                    date: None,
                }))
            }
        }

        let price_repo: Arc<dyn folioneer_lib::context::asset::AssetPriceRepository> =
            Arc::new(SqliteAssetPriceRepository::new(pool.clone()));

        let currency_service = Arc::new(CurrencyService::new(
            Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
            Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
        ));
        let dispatcher = Arc::new(Dispatcher::new(
            Arc::new(NoOpProvider),
            price_repo,
            Arc::clone(&bus),
            Arc::new(|| chrono::Local::now().date_naive()),
        ));

        AssetPriceFetchUseCase::new(
            account_service.clone(),
            asset_service.clone(),
            Arc::clone(&fetch_guard),
            dispatcher,
            currency_service,
        )
    };

    Ctx {
        use_case,
        fetch_guard,
    }
}

/// MKT-111 — fetch_all returns NoFetchableHoldings when no non-cash derivable holdings exist
/// (empty database). Exercises the full stack end-to-end.
#[tokio::test]
async fn fetch_all_returns_no_fetchable_holdings_on_empty_db() {
    use folioneer_lib::use_cases::asset_price_fetch::{
        FetchAllAssetPricesError, FetchPriceTask, FetchTrigger,
    };

    let ctx = build_ctx().await;
    let result = ctx.use_case.fetch_all(FetchTrigger::Launch).await;

    assert!(
        matches!(
            result,
            Err(FetchAllAssetPricesError::Failure(
                FetchPriceTask::NoFetchableHoldings
            ))
        ),
        "expected NoFetchableHoldings on empty DB, got: {result:?}"
    );
}

/// MKT-132 — fetch_for_account returns AccountNotFound for an unknown account_id.
/// Exercises the full existence-check stack.
#[tokio::test]
async fn fetch_for_account_returns_account_not_found_for_unknown_id() {
    use folioneer_lib::context::account::AccountError;
    use folioneer_lib::use_cases::asset_price_fetch::FetchAccountAssetPricesError;

    let ctx = build_ctx().await;
    let result = ctx.use_case.fetch_for_account("does-not-exist").await;

    assert!(
        matches!(
            result,
            Err(FetchAccountAssetPricesError::Account(
                AccountError::AccountNotFound { .. }
            ))
        ),
        "expected Account(AccountNotFound), got: {result:?}"
    );
}

/// MKT-113 — fetch_all returns FetchAlreadyRunning when the guard is held externally.
/// Verifies the in-flight guard propagates through the public use-case API.
#[tokio::test]
async fn fetch_all_returns_fetch_already_running_while_guard_held() {
    use folioneer_lib::use_cases::asset_price_fetch::{
        FetchAllAssetPricesError, FetchPriceTask, FetchTrigger,
    };

    let ctx = build_ctx().await;
    let _lease = ctx
        .fetch_guard
        .try_acquire()
        .expect("guard must be free at test start");

    let result = ctx.use_case.fetch_all(FetchTrigger::Launch).await;
    assert!(
        matches!(
            result,
            Err(FetchAllAssetPricesError::Failure(
                FetchPriceTask::FetchAlreadyRunning
            ))
        ),
        "expected FetchAlreadyRunning, got: {result:?}"
    );
}

/// MKT-110 — fetch_for_account uses derive_yahoo_symbol_with_exchange so an asset
/// carrying `exchange = Some(XPAR)` resolves to `<REF>.PA` (exchange-qualified)
/// rather than the bare-ticker legacy form. Guards the wiring of the picker-driven
/// exchange field into the actual Yahoo fetch symbol.
#[tokio::test]
async fn fetch_for_account_passes_exchange_qualified_symbol_to_provider() {
    use folioneer_lib::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository, UpdateFrequency,
    };
    use folioneer_lib::context::asset::{
        AssetPriceRepository, AssetService, CreateAssetDTO, PriceProvider,
        SqliteAssetCategoryRepository, SqliteAssetPriceRepository, SqliteAssetRepository,
        SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::SideEffectEventBus;
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
    use std::sync::Mutex;

    struct CapturingProvider {
        seen: Arc<Mutex<Vec<String>>>,
    }
    #[async_trait::async_trait]
    impl PriceProvider for CapturingProvider {
        async fn fetch_price(
            &self,
            symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            self.seen.lock().unwrap().push(symbol.to_string());
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 100_000_000,
                date: None,
            }))
        }
    }

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

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Air Liquide".to_string(),
            reference: "AI".to_string(),
            isin: None,
            class: folioneer_lib::context::asset::AssetClass::Stocks,
            currency: "EUR".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(folioneer_lib::context::asset::Exchange {
                code: "XPAR".to_string(),
                label: "Euronext Paris".to_string(),
            }),
            interest_bearing: false,
        })
        .await
        .expect("seed asset with XPAR exchange");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let provider = Arc::new(CapturingProvider {
        seen: Arc::clone(&seen),
    });
    let price_repo: Arc<dyn AssetPriceRepository> =
        Arc::new(SqliteAssetPriceRepository::new(pool.clone()));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        provider,
        price_repo,
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("fetch_for_account dispatch");

    // Dispatcher::spawn launches an async task — give it a moment to call the provider.
    for _ in 0..50 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let symbols = seen.lock().unwrap().clone();
    assert_eq!(
        symbols,
        vec!["AI.PA".to_string()],
        "MKT-110: orchestrator must derive `AI.PA` (XPAR → .PA suffix) for an asset carrying `exchange = Some(XPAR)`, not the bare `AI` legacy form"
    );
}

/// MKT-151 / ADR-014 — a locked asset (`price_refresh_blocked`) is excluded from
/// fetch scope. With the account's only holding locked, `build_scope` yields an
/// empty set and `fetch_for_account` is rejected with `NoFetchableHoldings`
/// (MKT-111), proving the asset was skipped before any provider call.
#[tokio::test]
async fn fetch_for_account_skips_locked_asset() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetPriceRepository, CreateAssetDTO, PriceProvider, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
    use folioneer_lib::use_cases::asset_price_fetch::{
        FetchAccountAssetPricesError, FetchPriceTask,
    };

    struct NoOpProvider;
    #[async_trait::async_trait]
    impl PriceProvider for NoOpProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 100_000_000,
                date: None,
            }))
        }
    }

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

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Apple".to_string(),
            reference: "AAPL".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");

    // MKT-156 — lock the asset before the fetch.
    asset_service
        .block_price_refresh(&asset.id)
        .await
        .expect("lock asset");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let price_repo: Arc<dyn AssetPriceRepository> =
        Arc::new(SqliteAssetPriceRepository::new(pool.clone()));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(NoOpProvider),
        price_repo,
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let result = use_case.fetch_for_account(&account.id).await;
    assert!(
        matches!(
            result,
            Err(FetchAccountAssetPricesError::Failure(
                FetchPriceTask::NoFetchableHoldings
            ))
        ),
        "MKT-151: a locked asset must be excluded from fetch scope → NoFetchableHoldings, got: {result:?}"
    );
}

/// MKT-156 — unblocking re-admits an asset to fetch scope. After locking then
/// unblocking the account's only holding, `fetch_for_account` dispatches
/// successfully (scope is non-empty again), exercising the unblock repo path.
#[tokio::test]
async fn fetch_for_account_includes_unblocked_asset() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetPriceRepository, CreateAssetDTO, PriceProvider, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    struct NoOpProvider;
    #[async_trait::async_trait]
    impl PriceProvider for NoOpProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 100_000_000,
                date: None,
            }))
        }
    }

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

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Apple".to_string(),
            reference: "AAPL".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");

    // Lock then unblock (MKT-156) — the asset must be fetchable again.
    asset_service
        .block_price_refresh(&asset.id)
        .await
        .expect("lock asset");
    asset_service
        .unblock_price_refresh(&asset.id)
        .await
        .expect("unlock asset");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let price_repo: Arc<dyn AssetPriceRepository> =
        Arc::new(SqliteAssetPriceRepository::new(pool.clone()));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(NoOpProvider),
        price_repo,
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    // Scope is non-empty (asset unblocked) → dispatch succeeds, not NoFetchableHoldings.
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("MKT-156: an unblocked asset must re-enter fetch scope and dispatch");
}

/// MKT-119 — a fetch task publishes `AssetPriceFetchCompleted` carrying outcome
/// counts. With one fetchable holding and a provider that returns a usable quote,
/// the terminal event reports `ok = 1, skipped = 0`.
#[tokio::test]
async fn fetch_publishes_completion_event_with_counts() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, CreateAssetDTO, PriceProvider, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::event_bus::Event;
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    struct OkProvider;
    #[async_trait::async_trait]
    impl PriceProvider for OkProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 100_000_000,
                date: Some("2026-01-02".to_string()),
            }))
        }
    }

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

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Apple".to_string(),
            reference: "AAPL".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");
    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(OkProvider),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let mut rx = bus.subscribe();
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("dispatch");

    // The fetch runs in a detached task; wait for the terminal completion event.
    let counts = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed()
                .await
                .expect("bus closed before AssetPriceFetchCompleted arrived");
            if let Event::AssetPriceFetchCompleted { ok, skipped, .. } = *rx.borrow() {
                return (ok, skipped);
            }
        }
    })
    .await
    .expect("AssetPriceFetchCompleted within timeout");

    assert_eq!(counts, (1, 0), "one holding fetched ok, none skipped");

    // MKT-102 — the row written by the fetch path is stamped source = YahooFinance.
    let latest = asset_service
        .get_latest_price(&asset.id)
        .await
        .expect("get_latest_price")
        .expect("a fetched price row must exist");
    assert_eq!(
        latest.source,
        folioneer_lib::context::asset::AssetPriceSource::YahooFinance,
        "fetch-path write must stamp source = YahooFinance (MKT-102)"
    );
}

// ── MKT-170 / MKT-171 — unpriced list on AssetPriceFetchCompleted ───────────

/// MKT-170/171 — when a provider returns no data for an asset (Ok(None)), the
/// completion event's `unpriced` list contains exactly that asset with its
/// identifying fields (name, reference, isin, currency) populated and
/// `last_price` / `last_price_date` set from its most recently recorded price.
///
/// Also verifies MKT-171: `unpriced.len() == skipped`.
#[tokio::test]
async fn fetch_completion_event_unpriced_list_contains_skipped_asset_with_last_price() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, PriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::event_bus::{Event, UnpricedAsset};
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    // Provider that returns no data — the canonical MKT-114 "no data" skip arm.
    struct NoDataProvider;
    #[async_trait::async_trait]
    impl PriceProvider for NoDataProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(None)
        }
    }

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(folioneer_lib::context::account::AccountService::new(
        Box::new(folioneer_lib::context::account::SqliteAccountRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteHoldingRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    // Seed an asset with an ISIN so we can assert it is forwarded.
    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Unpriced Corp".to_string(),
            reference: "UNPX".to_string(),
            isin: Some("US0231351067".to_string()),
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");

    // Seed a known prior price (50.0 USD = 50_000_000 micros) so last_price /
    // last_price_date are populated. `record_asset_price` stamps source = Manual
    // automatically (MKT-101).
    asset_service
        .record_asset_price(&asset.id, "2026-06-01", 50.0)
        .await
        .expect("seed prior price");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(folioneer_lib::context::currency::CurrencyService::new(
        Box::new(folioneer_lib::context::currency::SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::currency::SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(NoDataProvider),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = folioneer_lib::use_cases::asset_price_fetch::AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(folioneer_lib::use_cases::asset_price_fetch::FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let mut rx = bus.subscribe();
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("dispatch");

    // Wait for the terminal completion event.
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed()
                .await
                .expect("bus closed before AssetPriceFetchCompleted arrived");
            if let Event::AssetPriceFetchCompleted {
                ok,
                skipped,
                ref unpriced,
                ..
            } = *rx.borrow()
            {
                return (ok, skipped, unpriced.clone());
            }
        }
    })
    .await
    .expect("AssetPriceFetchCompleted within timeout");

    let (ok, skipped, unpriced) = event;

    // MKT-171: counts must agree.
    assert_eq!(ok, 0, "provider returned no data → ok must be 0");
    assert_eq!(skipped, 1, "one asset skipped");
    assert_eq!(
        unpriced.len(),
        skipped as usize,
        "MKT-171: unpriced.len() must equal skipped count"
    );

    // MKT-170: identifying fields and last-known price must be populated.
    let entry: &UnpricedAsset = &unpriced[0];
    assert_eq!(
        entry.asset_id, asset.id,
        "asset_id must match the skipped asset"
    );
    assert_eq!(entry.name, "Unpriced Corp", "name must match");
    assert_eq!(entry.reference, "UNPX", "reference (ticker) must match");
    assert_eq!(
        entry.isin,
        Some("US0231351067".to_string()),
        "isin must be forwarded when the asset has one"
    );
    assert_eq!(
        entry.currency, "USD",
        "currency must match asset native currency"
    );
    assert_eq!(
        entry.last_price,
        Some(50_000_000_i64),
        "MKT-170: last_price must be the most recently recorded price in i64 micros (ADR-001)"
    );
    assert_eq!(
        entry.last_price_date,
        Some("2026-06-01".to_string()),
        "MKT-170: last_price_date must be the ISO date of the most recently recorded price"
    );
}

/// MKT-170 — when an asset has never had a price recorded, `last_price` and
/// `last_price_date` in the `UnpricedAsset` entry are `None`.
#[tokio::test]
async fn fetch_completion_unpriced_entry_has_none_last_price_when_never_priced() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, PriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::event_bus::{Event, UnpricedAsset};
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    struct NoDataProvider;
    #[async_trait::async_trait]
    impl PriceProvider for NoDataProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(None)
        }
    }

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(folioneer_lib::context::account::AccountService::new(
        Box::new(folioneer_lib::context::account::SqliteAccountRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteHoldingRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    // Seed an asset with NO prior price recorded.
    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Never Priced".to_string(),
            reference: "NVPR".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "EUR".to_string(),
            risk_level: 2,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(folioneer_lib::context::currency::CurrencyService::new(
        Box::new(folioneer_lib::context::currency::SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::currency::SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(NoDataProvider),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = folioneer_lib::use_cases::asset_price_fetch::AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(folioneer_lib::use_cases::asset_price_fetch::FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let mut rx = bus.subscribe();
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("dispatch");

    let unpriced: Vec<UnpricedAsset> =
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                rx.changed()
                    .await
                    .expect("bus closed before AssetPriceFetchCompleted arrived");
                if let Event::AssetPriceFetchCompleted { ref unpriced, .. } = *rx.borrow() {
                    return unpriced.clone();
                }
            }
        })
        .await
        .expect("AssetPriceFetchCompleted within timeout");

    assert_eq!(unpriced.len(), 1, "one asset skipped, one entry expected");
    let entry = &unpriced[0];
    assert_eq!(
        entry.last_price, None,
        "MKT-170: last_price must be None when the asset has never been priced"
    );
    assert_eq!(
        entry.last_price_date, None,
        "MKT-170: last_price_date must be None when the asset has never been priced"
    );
}

/// MKT-171 — a successfully fetched asset must NOT appear in the `unpriced` list.
/// With one asset fetched ok (provider returns a quote), `unpriced` is empty.
#[tokio::test]
async fn fetch_completion_unpriced_list_excludes_successfully_fetched_asset() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, PriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::event_bus::{Event, UnpricedAsset};
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    struct OkProvider;
    #[async_trait::async_trait]
    impl PriceProvider for OkProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 150_000_000,
                date: Some("2026-06-01".to_string()),
            }))
        }
    }

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(folioneer_lib::context::account::AccountService::new(
        Box::new(folioneer_lib::context::account::SqliteAccountRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteHoldingRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Fetched Ok".to_string(),
            reference: "FOKO".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(folioneer_lib::context::currency::CurrencyService::new(
        Box::new(folioneer_lib::context::currency::SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::currency::SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(OkProvider),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = folioneer_lib::use_cases::asset_price_fetch::AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(folioneer_lib::use_cases::asset_price_fetch::FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let mut rx = bus.subscribe();
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("dispatch");

    let unpriced: Vec<UnpricedAsset> =
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                rx.changed()
                    .await
                    .expect("bus closed before AssetPriceFetchCompleted arrived");
                if let Event::AssetPriceFetchCompleted { ref unpriced, .. } = *rx.borrow() {
                    return unpriced.clone();
                }
            }
        })
        .await
        .expect("AssetPriceFetchCompleted within timeout");

    assert!(
        unpriced.is_empty(),
        "MKT-171: a successfully fetched asset must not appear in unpriced; got: {unpriced:?}"
    );
}

/// MKT-171 — `unpriced.len() == skipped` is an invariant that holds when multiple
/// assets are in scope and some succeed while others fail. Two assets: one fetched
/// ok (provider returns a quote), one skipped (provider returns an error). Only
/// the skipped one appears in `unpriced`, and `skipped == 1 == unpriced.len()`.
#[tokio::test]
async fn fetch_completion_unpriced_len_equals_skipped_count_in_mixed_outcome() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, PriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::core::event_bus::{Event, UnpricedAsset};
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;

    // Provider: succeeds for the first symbol queried, fails for the second.
    // Errors deterministically for one symbol so the ok/err split is independent of
    // fetch-scope iteration order; every other symbol fetches OK.
    struct ErrForSymbolProvider {
        err_symbol: &'static str,
    }
    #[async_trait::async_trait]
    impl PriceProvider for ErrForSymbolProvider {
        async fn fetch_price(
            &self,
            symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            if symbol == self.err_symbol {
                Err(anyhow::anyhow!("simulated network error"))
            } else {
                Ok(Some(folioneer_lib::context::asset::Quote {
                    price: 100_000_000,
                    date: None,
                }))
            }
        }
    }

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(folioneer_lib::context::account::AccountService::new(
        Box::new(folioneer_lib::context::account::SqliteAccountRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteHoldingRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    let asset_ok = asset_service
        .create_asset(CreateAssetDTO {
            name: "Asset Ok".to_string(),
            reference: "AOK".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset ok");
    let asset_err = asset_service
        .create_asset(CreateAssetDTO {
            name: "Asset Err".to_string(),
            reference: "AERR".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset err");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    // Open holdings for both assets so both enter fetch scope.
    for asset in [&asset_ok, &asset_err] {
        account_service
            .open_holding(
                &account.id,
                asset.id.clone(),
                "2024-01-01".to_string(),
                1_000_000,
                100_000_000,
            )
            .await
            .expect("seed holding");
    }

    let currency_service = Arc::new(folioneer_lib::context::currency::CurrencyService::new(
        Box::new(folioneer_lib::context::currency::SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::currency::SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(ErrForSymbolProvider { err_symbol: "AERR" }),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = folioneer_lib::use_cases::asset_price_fetch::AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(folioneer_lib::use_cases::asset_price_fetch::FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    let mut rx = bus.subscribe();
    use_case
        .fetch_for_account(&account.id)
        .await
        .expect("dispatch");

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed()
                .await
                .expect("bus closed before AssetPriceFetchCompleted arrived");
            if let Event::AssetPriceFetchCompleted {
                ok,
                skipped,
                ref unpriced,
                ..
            } = *rx.borrow()
            {
                return (ok, skipped, unpriced.clone());
            }
        }
    })
    .await
    .expect("AssetPriceFetchCompleted within timeout");

    let (ok, skipped, unpriced) = event;
    assert_eq!(ok, 1, "one asset fetched ok");
    assert_eq!(skipped, 1, "one asset errored → skipped");
    assert_eq!(
        unpriced.len(),
        skipped as usize,
        "MKT-171: unpriced.len() must equal skipped count; got ok={ok} skipped={skipped} unpriced={unpriced:?}"
    );
    // The entry must be for the errored asset, not the successful one.
    let entry: &UnpricedAsset = &unpriced[0];
    assert_eq!(
        entry.asset_id, asset_err.id,
        "the unpriced entry must identify the errored asset, not the successful one"
    );
}

/// MKT-171 — a cash asset (system cash, MKT-116) and a refresh-locked asset
/// (MKT-151) are excluded from fetch scope upstream, so they never enter the
/// `unpriced` list. The fetch scope built by the orchestrator already filters
/// them out; the dispatcher never sees them. With only a cash/locked asset in the
/// account, `fetch_for_account` returns `NoFetchableHoldings` — no completion
/// event is published and `unpriced` is therefore absent entirely. This test
/// confirms the path that would incorrectly include those assets in `unpriced`
/// is never reached.
#[tokio::test]
async fn fetch_completion_locked_asset_absent_from_unpriced_list() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, AssetService, CreateAssetDTO, PriceProvider, SqliteAssetCategoryRepository,
        SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::use_cases::asset_price_fetch::{
        FetchAccountAssetPricesError, FetchPriceTask,
    };

    // This provider would add to unpriced if the locked asset reached the dispatcher.
    struct NoDataProvider;
    #[async_trait::async_trait]
    impl PriceProvider for NoDataProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(None)
        }
    }

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(folioneer_lib::context::account::AccountService::new(
        Box::new(folioneer_lib::context::account::SqliteAccountRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteHoldingRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::account::SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Locked Asset".to_string(),
            reference: "LKDA".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed locked asset");

    // Lock the asset (MKT-151) before the fetch.
    asset_service
        .block_price_refresh(&asset.id)
        .await
        .expect("lock asset");

    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(folioneer_lib::context::currency::CurrencyService::new(
        Box::new(folioneer_lib::context::currency::SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(folioneer_lib::context::currency::SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(
        folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher::new(
            Arc::new(NoDataProvider),
            Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
            Arc::clone(&bus),
            Arc::new(|| chrono::Local::now().date_naive()),
        ),
    );
    let use_case = folioneer_lib::use_cases::asset_price_fetch::AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::new(folioneer_lib::use_cases::asset_price_fetch::FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    // The locked asset is excluded from scope upstream: the task is rejected with
    // NoFetchableHoldings, and no AssetPriceFetchCompleted is published.
    let result = use_case.fetch_for_account(&account.id).await;
    assert!(
        matches!(
            result,
            Err(FetchAccountAssetPricesError::Failure(
                FetchPriceTask::NoFetchableHoldings
            ))
        ),
        "MKT-151/171: locked asset excluded from scope → NoFetchableHoldings (no completion event, no unpriced entry); got: {result:?}"
    );
}

/// FXR-075 — a price fetch fetches no exchange rate: once the whole task has ended (its
/// in-flight lease released), a USD holding in a EUR account has no pair followed and
/// no rate recorded. Rates refresh through their own task.
#[tokio::test]
async fn a_price_fetch_leaves_exchange_rates_alone() {
    use folioneer_lib::context::account::UpdateFrequency;
    use folioneer_lib::context::asset::{
        AssetClass, CreateAssetDTO, PriceProvider, SYSTEM_CATEGORY_ID,
    };
    use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
    use folioneer_lib::use_cases::asset_price_fetch::FetchTrigger;

    struct PricedProvider;
    #[async_trait::async_trait]
    impl PriceProvider for PricedProvider {
        async fn fetch_price(
            &self,
            _symbol: &str,
        ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
            Ok(Some(folioneer_lib::context::asset::Quote {
                price: 100_000_000,
                date: None,
            }))
        }
    }

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
    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Apple".to_string(),
            reference: "AAPL".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");
    let account = account_service
        .create(
            "Test".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .expect("seed account");
    account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-01".to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(PricedProvider),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let fetch_guard = Arc::new(FetchGuard::new());
    let use_case = AssetPriceFetchUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::clone(&fetch_guard),
        dispatcher,
        Arc::clone(&currency_service),
    );

    use_case
        .fetch_all(FetchTrigger::Launch)
        .await
        .expect("dispatch");

    // The lease is dropped when the spawned task ends, rates step included.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while fetch_guard.try_acquire().is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the fetch task ends");

    let pairs = currency_service.list_currency_pairs().await.expect("pairs");
    assert!(
        pairs.is_empty(),
        "a price fetch must not follow or refresh any pair, got: {pairs:?}"
    );
}
