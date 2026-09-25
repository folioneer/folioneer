//! Golden portfolio — the figures a fixed synthetic portfolio must produce.
//!
//! A two-account, two-currency portfolio is built through the real services
//! against a fresh in-memory database, then every figure the user reads is
//! computed and compared to `tests/golden/expected.json`: the as-of holdings
//! view of each account, the yearly and monthly performance series, the global
//! performance series in the reference currency, the accounts list, and the
//! price-movement report of a refresh. Any drift fails.
//!
//! The snapshot is pinned to dates in the past and to a fixed "today" for the
//! refresh, so it does not move with the calendar. Generated identifiers and
//! the one calendar-relative figure (year-to-date on the accounts list) are
//! stripped before comparison. The accounts list converts foreign holdings at
//! the rate in force on the real today; the fixture's newest rate is dated
//! 2025-07-01, so every later day resolves the same rate. Array order is
//! pinned: it is what the user sees.
//!
//! A moved number is a failing build until the change that moves it names the
//! figure in its entry's Done when and regenerates the file:
//!
//!     GOLDEN_UPDATE=1 cargo test --test golden_portfolio
use std::sync::Arc;

use folioneer_lib::context::account::{
    AccountService, FeeFrequency, SqliteAccountRepository, SqliteFeeCatchUpRepository,
    SqliteFeeScheduleRepository, SqliteHoldingRepository, SqliteTransactionRepository,
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
use folioneer_lib::use_cases::account_details::AccountDetailsUseCase;
use folioneer_lib::use_cases::account_performance::AccountPerformanceUseCase;
use folioneer_lib::use_cases::account_summary::AccountSummaryUseCase;
use folioneer_lib::use_cases::asset_price_fetch::dispatcher::Dispatcher;
use folioneer_lib::use_cases::asset_price_fetch::{
    AssetPriceFetchUseCase, FetchGuard, FetchTrigger,
};
use folioneer_lib::use_cases::fee_generation::FeeGenerationOrchestrator;
use folioneer_lib::use_cases::global_performance::GlobalPerformanceUseCase;
use serde_json::{Map, Value};

const EXPECTED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/expected.json");
const AS_OF: &str = "2025-12-31";
const REFRESH_DAY: &str = "2026-01-05";
const LAST_PINNED_YEAR: i64 = 2025;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

fn price(v: f64) -> i64 {
    (v * 1_000_000.0).round() as i64
}

/// Quotes for the refresh, keyed by asset reference, all dated `REFRESH_DAY`.
struct FixedQuotes;

#[async_trait::async_trait]
impl PriceProvider for FixedQuotes {
    async fn fetch_price(&self, symbol: &str) -> anyhow::Result<Option<Quote>> {
        let quoted = match symbol {
            "ALPHA" => 140.0,
            "BETA" => 128.0,
            "GAMMA" => 215.0,
            _ => return Ok(None),
        };
        Ok(Some(Quote {
            price: price(quoted),
            date: Some(REFRESH_DAY.to_string()),
        }))
    }
}

struct Portfolio {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
    bus: Arc<SideEffectEventBus>,
    pool: sqlx::SqlitePool,
    main_eur: String,
    growth_usd: String,
}

async fn make_pool() -> sqlx::SqlitePool {
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

fn month_end(month_index: u32) -> String {
    // month_index 1..=24 walks 2024-01 .. 2025-12; every month ends on its last day.
    let year = 2024 + (month_index - 1) / 12;
    let month = (month_index - 1) % 12 + 1;
    let last_day = match month {
        2 if year.is_multiple_of(4) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    format!("{year}-{month:02}-{last_day:02}")
}

/// Builds the portfolio. Every figure downstream derives from what happens here;
/// change it only together with `expected.json` and the entry that asks for it.
async fn build_portfolio() -> Portfolio {
    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus))
        .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
        .with_fee_catch_up_repo(Box::new(SqliteFeeCatchUpRepository::new(pool.clone()))),
    );
    let asset_service = Arc::new(AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool.clone())),
    ));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));

    asset_service
        .seed_cash_asset("EUR")
        .await
        .expect("EUR cash");
    asset_service
        .seed_cash_asset("USD")
        .await
        .expect("USD cash");

    // Exchange rates: four observations, so conversions pick different rates over time.
    currency_service
        .declare_currency_pair("USD".into(), "EUR".into())
        .await
        .expect("pair");
    for (date, rate) in [
        ("2024-01-01", 0.90),
        ("2024-07-01", 0.92),
        ("2025-01-01", 0.95),
        ("2025-07-01", 0.93),
    ] {
        currency_service
            .record_currency_rate("USD".into(), "EUR".into(), date.into(), price(rate))
            .await
            .expect("rate");
    }

    // Three assets: two in EUR, one in USD; Gamma's price series has gaps.
    let mut ids = Vec::with_capacity(3);
    for (name, reference, currency, class) in [
        ("Alpha Industries", "ALPHA", "EUR", AssetClass::Stocks),
        ("Beta Corp", "BETA", "USD", AssetClass::Stocks),
        ("Gamma Fund", "GAMMA", "EUR", AssetClass::Stocks),
    ] {
        let asset = asset_service
            .create_asset(CreateAssetDTO {
                name: name.into(),
                reference: reference.into(),
                isin: None,
                class,
                currency: currency.into(),
                risk_level: 3,
                category_id: SYSTEM_CATEGORY_ID.into(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .expect("asset");
        ids.push(asset.id);
    }
    let (alpha, beta, gamma) = (ids[0].clone(), ids[1].clone(), ids[2].clone());

    for month in 1..=24u32 {
        let date = month_end(month);
        let m = f64::from(month);
        asset_service
            .record_asset_price(&alpha, &date, 100.0 + m * 1.5)
            .await
            .expect("alpha price");
        asset_service
            .record_asset_price(&beta, &date, 150.0 - m * 0.5)
            .await
            .expect("beta price");
        if !month.is_multiple_of(7) {
            asset_service
                .record_asset_price(&gamma, &date, 200.0 + f64::from(month % 5) * 3.0)
                .await
                .expect("gamma price");
        }
    }

    // Main EUR: buys (one in USD), a dividend, free shares, a split, a partial sale, a one-off fee.
    let main_eur = account_service
        .create(
            "Main EUR".into(),
            "Bank One".into(),
            "EUR".into(),
            UpdateFrequency::Automatic,
            true,
        )
        .await
        .expect("main account")
        .id;
    account_service
        .record_deposit(&main_eur, "2024-01-02".into(), micro(20_000), None)
        .await
        .expect("deposit");
    account_service
        .buy_holding(
            &main_eur,
            alpha.clone(),
            "2024-01-15".into(),
            micro(50),
            micro(100),
            micro(1),
            micro(5),
            None,
            None,
        )
        .await
        .expect("buy alpha");
    account_service
        .buy_holding(
            &main_eur,
            gamma.clone(),
            "2024-02-10".into(),
            micro(8),
            micro(200),
            micro(1),
            micro(4),
            None,
            None,
        )
        .await
        .expect("buy gamma");
    // A USD holding in the EUR account: the per-holding conversion path.
    account_service
        .buy_holding(
            &main_eur,
            beta.clone(),
            "2024-04-08".into(),
            micro(10),
            micro(140),
            price(0.90),
            micro(1),
            None,
            None,
        )
        .await
        .expect("buy beta in eur account");
    account_service
        .record_dividend(
            &main_eur,
            alpha.clone(),
            "2024-06-14".into(),
            price(75.5),
            micro(1),
            None,
        )
        .await
        .expect("dividend");
    account_service
        .record_free_shares(
            &main_eur,
            alpha.clone(),
            "2024-09-02".into(),
            micro(5),
            None,
        )
        .await
        .expect("free shares");
    account_service
        .record_split(
            &main_eur,
            gamma.clone(),
            "2025-02-03".into(),
            micro(2),
            None,
        )
        .await
        .expect("split");
    account_service
        .sell_holding(
            &main_eur,
            alpha.clone(),
            "2025-03-11".into(),
            micro(10),
            micro(120),
            micro(1),
            micro(3),
            None,
            None,
        )
        .await
        .expect("sell alpha");
    account_service
        .record_management_fee(
            &main_eur,
            alpha.clone(),
            "2025-06-30".into(),
            micro(1),
            None,
        )
        .await
        .expect("one-off fee");

    // Growth USD: one holding, a recurring fee schedule that ends in the past, a sale.
    let growth_usd = account_service
        .create(
            "Growth USD".into(),
            "Bank Two".into(),
            "USD".into(),
            UpdateFrequency::ManualMonth,
            true,
        )
        .await
        .expect("growth account")
        .id;
    account_service
        .record_deposit(&growth_usd, "2024-03-01".into(), micro(8_000), None)
        .await
        .expect("deposit usd");
    account_service
        .buy_holding(
            &growth_usd,
            beta.clone(),
            "2024-03-05".into(),
            micro(20),
            micro(140),
            micro(1),
            micro(2),
            None,
            None,
        )
        .await
        .expect("buy beta");
    account_service
        .create_fee_schedule(
            &growth_usd,
            beta.clone(),
            price(1.2),
            FeeFrequency::Monthly,
            "2024-06-01".into(),
            Some("2024-12-31".into()),
        )
        .await
        .expect("fee schedule");
    FeeGenerationOrchestrator::new(account_service.clone())
        .apply_due_fee_deductions()
        .await
        .expect("fee catch-up");
    account_service
        .sell_holding(
            &growth_usd,
            beta.clone(),
            "2025-08-20".into(),
            micro(5),
            micro(130),
            micro(1),
            micro(0),
            None,
            None,
        )
        .await
        .expect("sell beta");

    Portfolio {
        account_service,
        asset_service,
        currency_service,
        bus,
        pool,
        main_eur,
        growth_usd,
    }
}

async fn refresh_report(p: &Portfolio) -> Value {
    let dispatcher = Arc::new(Dispatcher::new(
        Arc::new(FixedQuotes),
        Arc::new(SqliteAssetPriceRepository::new(p.pool.clone())),
        Arc::clone(&p.bus),
        Arc::new(|| {
            chrono::NaiveDate::parse_from_str(REFRESH_DAY, "%Y-%m-%d").expect("refresh day")
        }),
    ));
    let use_case = AssetPriceFetchUseCase::new(
        p.account_service.clone(),
        p.asset_service.clone(),
        Arc::new(FetchGuard::new()),
        dispatcher,
        p.currency_service.clone(),
    );
    let mut rx = p.bus.subscribe();
    use_case
        .fetch_all(FetchTrigger::Manual)
        .await
        .expect("refresh");
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            rx.changed().await.expect("bus closed");
            if matches!(*rx.borrow(), Event::AssetPriceFetchCompleted { .. }) {
                return rx.borrow().clone();
            }
        }
    })
    .await
    .expect("refresh completed");
    let Event::AssetPriceFetchCompleted { movement, .. } = event else {
        panic!("expected AssetPriceFetchCompleted");
    };
    serde_json::to_value(movement.expect("a manual refresh carries a report")).expect("json")
}

/// Removes generated ids and calendar-relative fields and drops periods after
/// the last pinned year. Array order is kept: the order the services return is
/// part of what the user sees, so it is pinned like any other figure.
fn canonical(value: Value) -> Value {
    const DROPPED_KEYS: &[&str] = &["id", "asset_id", "account_id", "ytd_performance_pct"];
    match value {
        Value::Object(map) => {
            let kept: Map<String, Value> = map
                .into_iter()
                .filter(|(k, _)| !DROPPED_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k, canonical(v)))
                .collect();
            Value::Object(kept)
        }
        Value::Array(items) => {
            let kept: Vec<Value> = items
                .into_iter()
                .filter(|item| {
                    item.get("year")
                        .and_then(Value::as_i64)
                        .is_none_or(|year| year <= LAST_PINNED_YEAR)
                })
                .map(canonical)
                .collect();
            Value::Array(kept)
        }
        other => other,
    }
}

fn differences(path: &str, expected: &Value, actual: &Value, out: &mut Vec<String>) {
    match (expected, actual) {
        (Value::Object(e), Value::Object(a)) => {
            for key in e.keys().chain(a.keys().filter(|k| !e.contains_key(*k))) {
                let child = format!("{path}.{key}");
                match (e.get(key), a.get(key)) {
                    (Some(ev), Some(av)) => differences(&child, ev, av, out),
                    (Some(_), None) => out.push(format!("{child}: missing in actual")),
                    (None, Some(_)) => out.push(format!("{child}: new in actual")),
                    (None, None) => {}
                }
            }
        }
        (Value::Array(e), Value::Array(a)) => {
            for (index, (ev, av)) in e.iter().zip(a).enumerate() {
                differences(&format!("{path}[{index}]"), ev, av, out);
            }
            for (index, item) in e.iter().enumerate().skip(a.len()) {
                out.push(format!("{path}[{index}]: missing in actual: {item}"));
            }
            for (index, item) in a.iter().enumerate().skip(e.len()) {
                out.push(format!("{path}[{index}]: new in actual: {item}"));
            }
        }
        _ if expected != actual => {
            out.push(format!("{path}: expected {expected}, actual {actual}"))
        }
        _ => {}
    }
}

#[tokio::test]
async fn golden_portfolio_figures_are_unchanged() {
    let p = build_portfolio().await;
    let details = AccountDetailsUseCase::new(
        p.account_service.clone(),
        p.asset_service.clone(),
        p.currency_service.clone(),
    );
    let performance = AccountPerformanceUseCase::new(
        p.account_service.clone(),
        p.asset_service.clone(),
        p.currency_service.clone(),
    );
    let global = GlobalPerformanceUseCase::new(
        p.account_service.clone(),
        p.asset_service.clone(),
        p.currency_service.clone(),
    );
    let summaries = AccountSummaryUseCase::new(
        p.account_service.clone(),
        p.asset_service.clone(),
        p.currency_service.clone(),
    );

    let mut actual = Map::new();
    for (label, id) in [("main_eur", &p.main_eur), ("growth_usd", &p.growth_usd)] {
        let view = details
            .get_account_details(id, Some(AS_OF))
            .await
            .expect("details");
        actual.insert(
            format!("details_{label}_as_of_{AS_OF}"),
            serde_json::to_value(view).expect("json"),
        );
        let series = performance
            .get_account_performance(id, None)
            .await
            .expect("performance");
        actual.insert(
            format!("performance_{label}"),
            serde_json::to_value(series).expect("json"),
        );
    }
    let global_series = global
        .get_global_performance(None, None)
        .await
        .expect("global");
    actual.insert(
        "global_performance".into(),
        serde_json::to_value(global_series).expect("json"),
    );
    let list = summaries.get_account_summaries().await.expect("summaries");
    actual.insert(
        "account_summaries".into(),
        serde_json::to_value(list).expect("json"),
    );
    actual.insert(
        format!("price_movement_{REFRESH_DAY}"),
        refresh_report(&p).await,
    );

    let actual = canonical(Value::Object(actual));
    let rendered = serde_json::to_string_pretty(&actual).expect("render") + "\n";

    if std::env::var_os("GOLDEN_UPDATE").is_some() {
        std::fs::write(EXPECTED, &rendered).expect("write expected.json");
        return;
    }
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(EXPECTED)
            .expect("tests/golden/expected.json is missing — run with GOLDEN_UPDATE=1 once"),
    )
    .expect("expected.json parses");
    let mut diffs = Vec::new();
    differences("$", &expected, &actual, &mut diffs);
    assert!(
        diffs.is_empty(),
        "golden portfolio drifted in {} place(s):\n  {}\n\
         If the entry's Done when names this figure, regenerate with GOLDEN_UPDATE=1.",
        diffs.len(),
        diffs
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
