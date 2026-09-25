//! Price freshness (MKT-200/201) on real SQLite: what the header reads once a fetch task
//! has completed.

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
    UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, PriceProvider, Quote, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::event_bus::Event;
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
use folioneer_lib::use_cases::asset_price_fetch::{AssetPriceFetchUseCase, FetchGuard};
use folioneer_lib::use_cases::price_freshness::{PriceFreshness, PriceFreshnessUseCase};
use folioneer_lib::use_cases::shared::price_fetch_log::{
    PriceFetchLogRepository, SqlitePriceFetchLogRepository,
};
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

struct DatedProvider;
#[async_trait::async_trait]
impl PriceProvider for DatedProvider {
    async fn fetch_price(&self, _symbol: &str) -> anyhow::Result<Option<Quote>> {
        Ok(Some(Quote {
            price: 100_000_000,
            date: Some("2026-09-15".to_string()),
        }))
    }
}

struct NoDataProvider;
#[async_trait::async_trait]
impl PriceProvider for NoDataProvider {
    async fn fetch_price(&self, _symbol: &str) -> anyhow::Result<Option<Quote>> {
        Ok(None)
    }
}

/// Seeds one account holding one asset, checks that nothing is reported before any fetch,
/// runs a fetch task on 2026-09-19 at 08:14 against `provider`, and returns what is read as
/// soon as the completion event arrives — which is when the header reads (MKT-203).
async fn freshness_read_on_completion(provider: Arc<dyn PriceProvider>) -> PriceFreshness {
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

    let fetch_log: Arc<dyn PriceFetchLogRepository> =
        Arc::new(SqlitePriceFetchLogRepository::new(pool.clone()));
    let freshness = PriceFreshnessUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        Arc::clone(&fetch_log),
    );
    assert_eq!(
        freshness.read().await.expect("read before any fetch"),
        PriceFreshness {
            newest_price_date: None,
            last_fetch_at: None,
        }
    );

    let dispatcher = Arc::new(
        Dispatcher::new(
            provider,
            Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
            Arc::clone(&bus),
            Arc::new(|| chrono::NaiveDate::from_ymd_opt(2026, 9, 19).expect("valid date")),
        )
        .with_fetch_log(
            fetch_log,
            Arc::new(|| {
                chrono::NaiveDate::from_ymd_opt(2026, 9, 19)
                    .and_then(|date| date.and_hms_opt(8, 14, 0))
                    .expect("valid moment")
            }),
        ),
    );
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
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed()
                .await
                .expect("bus closed before AssetPriceFetchCompleted arrived");
            if matches!(*rx.borrow(), Event::AssetPriceFetchCompleted { .. }) {
                return;
            }
        }
    })
    .await
    .expect("AssetPriceFetchCompleted within timeout");

    freshness.read().await.expect("read after the fetch")
}

/// MKT-200/201 — once the fetch task has announced its completion, the read gives the
/// fetched price's date and the task's moment: the moment is recorded before the event.
#[tokio::test]
async fn mkt_201_the_freshness_read_woken_by_the_completion_event_sees_the_new_fetch() {
    assert_eq!(
        freshness_read_on_completion(Arc::new(DatedProvider)).await,
        PriceFreshness {
            newest_price_date: Some("2026-09-15".to_string()),
            last_fetch_at: Some("2026-09-19T08:14:00".to_string()),
        }
    );
}

/// MKT-201 — a task that priced nothing is not a fetch: the device still never fetched.
#[tokio::test]
async fn mkt_201_a_fetch_task_that_priced_nothing_records_no_moment() {
    assert_eq!(
        freshness_read_on_completion(Arc::new(NoDataProvider)).await,
        PriceFreshness {
            newest_price_date: None,
            last_fetch_at: None,
        }
    );
}
