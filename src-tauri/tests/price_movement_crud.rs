//! Integration tests for the Price Movement report (PMV spec) — the wiring
//! between `fetch_all_asset_prices`'s `trigger` argument and the
//! `AssetPriceFetchCompleted.movement` payload (PMV-010/011/015).
//!
//! Exercises the full stack through the public API: orchestrator constructor →
//! AccountService / AssetService → real in-memory SQLite, mirroring
//! `asset_price_fetch_crud.rs`. No mocks — per test_convention.md Tier 3.

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
    UpdateFrequency,
};
use folioneer_lib::context::asset::AssetPriceRepository;
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::event_bus::Event;
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
use folioneer_lib::use_cases::asset_price_fetch::{
    AssetPriceFetchUseCase, FetchGuard, FetchTrigger,
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

/// PMV-026 — stands in for a writer that is not this refresh: a concurrent
/// Scheduled fetch (SPF-023), a manual entry, or another device's changes
/// arriving through sync. It writes a newer price straight to the database
/// during the fetch loop — i.e. squarely inside the capture -> report window —
/// and then reports no quote of its own, so nothing enters `fetched`.
struct ThirdPartyWritingProvider {
    pool: sqlx::SqlitePool,
    asset_id: String,
}

#[async_trait::async_trait]
impl folioneer_lib::context::asset::PriceProvider for ThirdPartyWritingProvider {
    async fn fetch_price(
        &self,
        _symbol: &str,
    ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
        let repo = SqliteAssetPriceRepository::new(self.pool.clone());
        repo.upsert(folioneer_lib::context::asset::AssetPrice::restore(
            self.asset_id.clone(),
            "2026-09-12".to_string(),
            999_000_000,
            folioneer_lib::context::asset::AssetPriceSource::Manual,
        ))
        .await
        .expect("third-party write");
        Ok(None)
    }
}

struct OkProvider;
#[async_trait::async_trait]
impl folioneer_lib::context::asset::PriceProvider for OkProvider {
    async fn fetch_price(
        &self,
        _symbol: &str,
    ) -> anyhow::Result<Option<folioneer_lib::context::asset::Quote>> {
        Ok(Some(folioneer_lib::context::asset::Quote {
            price: 120_000_000,
            date: Some("2026-09-11".to_string()),
        }))
    }
}

struct Ctx {
    use_case: AssetPriceFetchUseCase,
    bus: Arc<SideEffectEventBus>,
    account_id: String,
    asset_id: String,
}

async fn build_ctx() -> Ctx {
    build_ctx_with_provider(make_pool().await, |_| Arc::new(OkProvider)).await
}

/// Same fixture, with the price provider chosen by the caller. The factory
/// receives the seeded asset id so a provider can act on that asset.
async fn build_ctx_with_provider<F>(pool: sqlx::SqlitePool, provider: F) -> Ctx
where
    F: FnOnce(String) -> Arc<dyn folioneer_lib::context::asset::PriceProvider>,
{
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
            currency: "EUR".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("seed asset");
    // A baseline price recorded before the refresh, so the "before" reading is non-zero.
    asset_service
        .record_asset_price(&asset.id, "2026-09-01", 100.0)
        .await
        .expect("seed baseline price");

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
        provider(asset.id.clone()),
        Arc::new(SqliteAssetPriceRepository::new(pool.clone())),
        Arc::clone(&bus),
        Arc::new(|| chrono::Local::now().date_naive()),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        account_service,
        asset_service,
        Arc::new(FetchGuard::new()),
        dispatcher,
        currency_service,
    );

    Ctx {
        use_case,
        bus,
        account_id: account.id,
        asset_id: asset.id,
    }
}

async fn await_completion(bus: &SideEffectEventBus) -> Event {
    let mut rx = bus.subscribe();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed()
                .await
                .expect("bus closed before AssetPriceFetchCompleted arrived");
            if matches!(*rx.borrow(), Event::AssetPriceFetchCompleted { .. }) {
                return rx.borrow().clone();
            }
        }
    })
    .await
    .expect("AssetPriceFetchCompleted within timeout")
}

/// PMV-010/011 — a Global refresh (`trigger == Manual`) publishes
/// `AssetPriceFetchCompleted` carrying a present Price Movement report.
/// Exercises the full call chain: command surface → orchestrator → dispatcher
/// → event bus.
#[tokio::test]
async fn fetch_all_with_manual_trigger_publishes_a_price_movement_report() {
    let ctx = build_ctx().await;

    ctx.use_case
        .fetch_all(FetchTrigger::Manual)
        .await
        .expect("dispatch");

    let event = await_completion(&ctx.bus).await;
    let Event::AssetPriceFetchCompleted { movement, .. } = event else {
        panic!("expected AssetPriceFetchCompleted");
    };

    assert!(
        movement.is_some(),
        "a Manual-trigger Global refresh must carry a Price Movement report (PMV-010/011)"
    );
    let report = movement.unwrap();
    assert_eq!(
        report.rows.len(),
        1,
        "one row for the single seeded account"
    );

    // PMV-021/022/024 — the seed is fully determined: 1 unit held, a baseline
    // price of 100.00 recorded on 2026-09-01, and a provider quote of 120.00
    // dated 2026-09-11. Asserting only `is_some()` would let the whole capture
    // and overlay pipeline return anything at all.
    let row = &report.rows[0];
    assert_eq!(
        row.before, 100_000_000,
        "PMV-021 — the reading taken before the fetch"
    );
    assert_eq!(
        row.after, 120_000_000,
        "PMV-022 — the reading over this refresh's own write"
    );
    assert_eq!(
        row.movement_pct,
        Some(20_000_000),
        "PMV-024 — +20.00% in micro-percent"
    );
    assert!(
        !row.incomplete,
        "PMV-032 — the holding was priced, so nothing is incomplete"
    );
    assert_eq!(
        row.account_id, ctx.account_id,
        "the row describes the seeded account"
    );

    // PMV-050 — both observation dates come from the capture, not the panel.
    assert_eq!(
        report.observed_from.as_deref(),
        Some("2026-09-01"),
        "PMV-050 — the date the portfolio carried before this refresh"
    );
    assert_eq!(
        report.observed_to.as_deref(),
        Some("2026-09-11"),
        "PMV-050 — the date this refresh produced"
    );

    // PMV-040/041 — the total is converted and derived from the totals.
    assert_eq!(report.total_currency, "EUR");
    assert_eq!(report.total_before, 100_000_000);
    assert_eq!(report.total_after, 120_000_000);
    assert_eq!(report.total_movement_pct, Some(20_000_000));
    assert!(!report.incomplete, "PMV-043");
}

/// PMV-010/015 — the launch auto-fetch (`trigger == Launch`) never reports
/// movement, even though it runs the exact same scope (MKT-130).
#[tokio::test]
async fn fetch_all_with_launch_trigger_publishes_no_price_movement_report() {
    let ctx = build_ctx().await;

    ctx.use_case
        .fetch_all(FetchTrigger::Launch)
        .await
        .expect("dispatch");

    let event = await_completion(&ctx.bus).await;
    let Event::AssetPriceFetchCompleted { movement, .. } = event else {
        panic!("expected AssetPriceFetchCompleted");
    };

    assert!(
        movement.is_none(),
        "the launch auto-fetch must never carry a Price Movement report"
    );
}

/// PMV-010 — an Account refresh never reports movement, whatever its outcome.
/// It shares the dispatcher with the Global refresh, so this is the limb that
/// would silently start reporting if the trigger ever stopped gating it. Also
/// closes the contract's "absent on every account-scoped fetch" clause.
#[tokio::test]
async fn fetch_for_account_publishes_no_price_movement_report() {
    let ctx = build_ctx().await;

    ctx.use_case
        .fetch_for_account(&ctx.account_id)
        .await
        .expect("dispatch");

    let event = await_completion(&ctx.bus).await;
    let Event::AssetPriceFetchCompleted { movement, .. } = event else {
        panic!("expected AssetPriceFetchCompleted");
    };

    assert!(
        movement.is_none(),
        "an Account refresh must never carry a Price Movement report (PMV-010)"
    );
}

/// PMV-026 — a price written during the refresh by anything other than this
/// refresh is excluded from the later reading. The provider writes 999.00 dated
/// 2026-09-12 directly to the database inside the fetch loop and then returns no
/// quote, so `fetched` stays empty. A report built by re-reading the database
/// would value the holding at 999.00; one built from this refresh's own writes
/// keeps the 100.00 baseline. That difference is what this test discriminates —
/// a `build_report` unit test cannot, because the foreign write is not
/// expressible in its inputs.
#[tokio::test]
async fn a_price_written_by_another_writer_during_the_refresh_is_excluded() {
    let pool = make_pool().await;
    let ctx = build_ctx_with_provider(pool.clone(), |asset_id| {
        Arc::new(ThirdPartyWritingProvider {
            pool: pool.clone(),
            asset_id,
        })
    })
    .await;

    ctx.use_case
        .fetch_all(FetchTrigger::Manual)
        .await
        .expect("dispatch");

    let event = await_completion(&ctx.bus).await;
    let Event::AssetPriceFetchCompleted { movement, .. } = event else {
        panic!("expected AssetPriceFetchCompleted");
    };
    let report = movement.expect("a Manual refresh reports movement");
    let row = &report.rows[0];

    // Guard against a vacuous pass: if the foreign write never landed there
    // would be nothing for a re-reading implementation to pick up, and this
    // test would prove nothing at all.
    let latest = SqliteAssetPriceRepository::new(pool.clone())
        .get_latest(&ctx.asset_id)
        .await
        .expect("latest price")
        .expect("the third party wrote a price");
    assert_eq!(
        latest.price, 999_000_000,
        "the foreign write must actually be in the database for this test to mean anything"
    );

    assert_eq!(
        row.after, 100_000_000,
        "PMV-026 — the later reading must ignore a write this refresh did not make \
         (a re-reading implementation would report 999_000_000 here)"
    );
    assert_eq!(
        row.movement_pct, None,
        "the account is unmoved by this refresh"
    );
    assert_eq!(
        report.observed_to, None,
        "nor may a foreign write raise the observed-to date this refresh reports"
    );
}
