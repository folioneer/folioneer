/// Integration tests for the price history backfill (MKT-190–199).
///
/// Exercises the full stack through the public API: `PriceHistoryBackfillUseCase`
/// → AccountService / AssetService → real in-memory SQLite. The external provider
/// is a scripted adapter serving a fixed daily-close series, the network being the
/// one boundary a Tier 3 test cannot cross.
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;
use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
    UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetPriceSource, AssetService, CreateAssetDTO, DatedClose, PriceProvider, Quote,
    SqliteAssetCategoryRepository, SqliteAssetPriceRepository, SqliteAssetRepository,
    SYSTEM_CATEGORY_ID,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::price_history_backfill::{
    PriceHistoryBackfillError, PriceHistoryBackfillTask, PriceHistoryBackfillUseCase,
};

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

/// Serves a fixed daily-close series. Every request whose window starts on or after
/// `unreachable_from` fails the way a network error does.
struct ScriptedProvider {
    closes: Vec<DatedClose>,
    unreachable_from: Option<&'static str>,
    requested_windows: Mutex<Vec<(String, String)>>,
}

impl ScriptedProvider {
    fn serving(closes: Vec<DatedClose>) -> Self {
        Self {
            closes,
            unreachable_from: None,
            requested_windows: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl PriceProvider for ScriptedProvider {
    async fn fetch_price(&self, _symbol: &str) -> anyhow::Result<Option<Quote>> {
        Ok(None)
    }

    async fn fetch_daily_closes(
        &self,
        _symbol: &str,
        from: &str,
        to: &str,
    ) -> anyhow::Result<Option<Vec<DatedClose>>> {
        self.requested_windows
            .lock()
            .expect("windows lock")
            .push((from.to_string(), to.to_string()));
        if self.unreachable_from.is_some_and(|start| from >= start) {
            anyhow::bail!("provider unreachable");
        }
        Ok(Some(
            self.closes
                .iter()
                .filter(|close| close.date.as_str() >= from && close.date.as_str() <= to)
                .cloned()
                .collect(),
        ))
    }
}

fn close(date: &str, price: i64) -> DatedClose {
    DatedClose {
        date: date.to_string(),
        price,
    }
}

fn date(iso: &str) -> NaiveDate {
    NaiveDate::parse_from_str(iso, "%Y-%m-%d").expect("valid ISO date")
}

struct Ctx {
    use_case: PriceHistoryBackfillUseCase,
    asset_service: Arc<AssetService>,
    provider: Arc<ScriptedProvider>,
    account_id: String,
    asset_id: String,
}

/// One account holding one asset (reference "AI", no exchange) since `opened_on`,
/// with the use case's today fixed to `today`.
async fn build_ctx(opened_on: &str, today: &str, provider: ScriptedProvider) -> Ctx {
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

    let asset = asset_service
        .create_asset(CreateAssetDTO {
            name: "Air Liquide".to_string(),
            reference: "AI".to_string(),
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
    let account = account_service
        .create(
            "Backfill".to_string(),
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
            opened_on.to_string(),
            1_000_000,
            100_000_000,
        )
        .await
        .expect("seed holding");

    let provider = Arc::new(provider);
    let today = date(today);
    let use_case = PriceHistoryBackfillUseCase::new(
        account_service,
        asset_service.clone(),
        provider.clone(),
        Arc::new(move || today),
    );

    Ctx {
        use_case,
        asset_service,
        provider,
        account_id: account.id,
        asset_id: asset.id,
    }
}

/// MKT-191/192/194 — over a held period where one day was already priced by hand,
/// the provider's closes land on the missing days only; the manual price stays as
/// it was and is counted as already priced; the close the provider serves for
/// today is never recorded.
#[tokio::test]
async fn backfill_records_only_the_days_without_a_price() {
    let ctx = build_ctx(
        "2024-01-02",
        "2024-01-05",
        ScriptedProvider::serving(vec![
            close("2024-01-02", 10_000_000),
            close("2024-01-03", 11_000_000),
            close("2024-01-04", 12_000_000),
            close("2024-01-05", 13_000_000),
        ]),
    )
    .await;
    ctx.asset_service
        .record_asset_price(&ctx.asset_id, "2024-01-03", 50.0)
        .await
        .expect("seed a manual price");

    let outcome = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect("the backfill succeeds");

    assert_eq!(outcome.written, 2);
    assert_eq!(outcome.already_priced, 1);
    let prices = ctx
        .asset_service
        .get_asset_prices(&ctx.asset_id)
        .await
        .expect("prices");
    assert_eq!(
        prices.len(),
        3,
        "one price per trading day through yesterday"
    );
    assert!(
        prices.iter().all(|price| price.date != "2024-01-05"),
        "the provider serves a close for today (2024-01-05) and it must not be recorded"
    );
    let manual = prices
        .iter()
        .find(|price| price.date == "2024-01-03")
        .expect("the manual price is kept");
    assert_eq!(manual.price, 50_000_000);
    assert_eq!(manual.source, AssetPriceSource::Manual);
    let filled = prices
        .iter()
        .find(|price| price.date == "2024-01-04")
        .expect("the missing day is filled");
    assert_eq!(filled.price, 12_000_000);
    assert_eq!(filled.source, AssetPriceSource::YahooFinance);
}

/// MKT-192 — a second backfill over a history the first one completed writes
/// nothing and counts every served close as already priced.
#[tokio::test]
async fn backfill_over_a_complete_history_writes_nothing() {
    let ctx = build_ctx(
        "2024-01-02",
        "2024-01-05",
        ScriptedProvider::serving(vec![
            close("2024-01-02", 10_000_000),
            close("2024-01-03", 11_000_000),
        ]),
    )
    .await;
    let first = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect("the first backfill succeeds");
    assert_eq!(
        first.written, 2,
        "precondition: the first run fills the gaps"
    );

    let second = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect("the second backfill succeeds");

    assert_eq!(second.written, 0);
    assert_eq!(second.already_priced, 2);
}

/// MKT-196 — a held period covering only a weekend: the provider knows the
/// symbol but serves no close, so the backfill succeeds with nothing recorded
/// instead of reporting an unresolvable ticker.
#[tokio::test]
async fn backfill_over_a_weekend_only_period_succeeds_with_nothing_to_fill() {
    let ctx = build_ctx(
        "2024-06-29",
        "2024-07-01",
        ScriptedProvider::serving(vec![close("2024-06-28", 10_000_000)]),
    )
    .await;

    let outcome = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect("a period without a trading day is not a rejection");

    assert_eq!(outcome.written, 0);
    assert_eq!(outcome.already_priced, 0);
    assert_eq!(
        ctx.provider
            .requested_windows
            .lock()
            .expect("windows lock")
            .as_slice(),
        [("2024-06-29".to_string(), "2024-06-30".to_string())],
        "precondition: the weekend was requested"
    );
    let prices = ctx
        .asset_service
        .get_asset_prices(&ctx.asset_id)
        .await
        .expect("prices");
    assert!(prices.is_empty(), "nothing may be recorded, got {prices:?}");
}

/// MKT-191/193 — a held period longer than a year is requested in consecutive
/// windows of at most 365 days that start on the first held day and end yesterday.
#[tokio::test]
async fn backfill_requests_a_long_period_in_consecutive_windows() {
    let ctx = build_ctx(
        "2022-01-01",
        "2024-06-30",
        ScriptedProvider::serving(vec![
            close("2022-01-03", 10_000_000),
            close("2024-06-28", 20_000_000),
        ]),
    )
    .await;

    let outcome = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect("the backfill succeeds");

    assert_eq!(outcome.written, 2);
    let windows = ctx
        .provider
        .requested_windows
        .lock()
        .expect("windows lock")
        .clone();
    assert!(
        windows.len() >= 3,
        "two and a half years need at least three windows, got {windows:?}"
    );
    assert_eq!(windows.first().map(|w| w.0.as_str()), Some("2022-01-01"));
    assert_eq!(windows.last().map(|w| w.1.as_str()), Some("2024-06-29"));
    for (from, to) in &windows {
        assert!(
            (date(to) - date(from)).num_days() < 365,
            "window {from}..{to} is longer than 365 days"
        );
    }
    for pair in windows.windows(2) {
        assert_eq!(
            date(&pair[1].0),
            date(&pair[0].1).succ_opt().expect("next day"),
            "windows must be consecutive"
        );
    }
}

/// MKT-195 — when one window cannot be fetched the backfill is rejected and
/// records nothing, not even the closes the other windows returned.
#[tokio::test]
async fn backfill_writes_nothing_when_a_window_cannot_be_fetched() {
    let mut provider = ScriptedProvider::serving(vec![
        close("2022-01-03", 10_000_000),
        close("2024-06-28", 20_000_000),
    ]);
    provider.unreachable_from = Some("2023-06-01");
    let ctx = build_ctx("2022-01-01", "2024-06-30", provider).await;

    let error = ctx
        .use_case
        .backfill(&ctx.account_id, &ctx.asset_id)
        .await
        .expect_err("a failed window rejects the backfill");

    assert!(
        matches!(
            error,
            PriceHistoryBackfillError::Task(PriceHistoryBackfillTask::ProviderUnreachable)
        ),
        "got: {error:?}"
    );
    assert!(
        ctx.provider
            .requested_windows
            .lock()
            .expect("windows lock")
            .len()
            >= 2,
        "precondition: a window before the failing one was served"
    );
    let prices = ctx
        .asset_service
        .get_asset_prices(&ctx.asset_id)
        .await
        .expect("prices");
    assert!(prices.is_empty(), "nothing may be recorded, got {prices:?}");
}
