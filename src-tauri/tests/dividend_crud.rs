//! Integration tests for the `record_dividend` use-case and the DIV read-model fields
//! (DIV-023, DIV-024, DIV-026, DIV-027, DIV-070, DIV-071, DIV-073).
//!
//! All tests exercise the full stack through the public `folioneer_lib` API:
//! `HoldingTransactionUseCase` → `AccountService` / `AssetService` → real in-memory
//! SQLite. No mocks — per test_convention.md Tier 3 constraint.

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteFeeScheduleRepository,
    SqliteHoldingNoteRepository, SqliteHoldingRepository, SqliteTransactionRepository,
    TransactionType, UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::account_details::AccountDetailsUseCase;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

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
    use_case: HoldingTransactionUseCase,
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    details_use_case: AccountDetailsUseCase,
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
        .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
        .with_holding_note_repo(Box::new(SqliteHoldingNoteRepository::new(pool.clone())))
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

    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let use_case = HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone());
    let details_use_case = AccountDetailsUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        currency_service,
    );

    Ctx {
        use_case,
        account_service,
        asset_service,
        details_use_case,
    }
}

fn stocks_asset_dto(name: &str, reference: &str, currency: &str) -> CreateAssetDTO {
    CreateAssetDTO {
        name: name.to_string(),
        reference: reference.to_string(),
        isin: None,
        class: AssetClass::Stocks,
        currency: currency.to_string(),
        risk_level: 2,
        category_id: SYSTEM_CATEGORY_ID.to_string(),
        exchange: None,
        interest_bearing: false,
    }
}

// -------------------------------------------------------------------------
// DIV-023 — happy-path end-to-end
// -------------------------------------------------------------------------

/// DIV-023 — record_dividend end-to-end: persists a Dividend transaction with
/// asset_id = paying asset, type = Dividend, fees = 0, realized_pnl = None,
/// total_amount in account currency.
#[tokio::test]
async fn record_dividend_end_to_end_persists_correct_fields() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "My Account".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    // Seed cash and buy to establish the holding.
    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let tx = ctx
        .use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200), // 200 USD dividend
            micro(1),   // exchange_rate = 1 (same currency)
            Some("Q2".to_string()),
        )
        .await
        .unwrap();

    assert_eq!(tx.transaction_type, TransactionType::Dividend);
    assert_eq!(tx.asset_id, asset.id, "asset_id must be the paying asset");
    assert_eq!(tx.account_id, account.id);
    assert_eq!(tx.total_amount, micro(200));
    assert_eq!(tx.fees, 0);
    assert!(
        tx.realized_pnl.is_none(),
        "dividend realized_pnl must be None"
    );
    assert_eq!(tx.note.as_deref(), Some("Q2"));
}

/// DIV-024 — after a dividend, the paying asset's holding quantity, average_price,
/// and total_realized_pnl are unchanged.
#[tokio::test]
async fn record_dividend_leaves_paying_asset_holding_unchanged() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let holdings_before = ctx
        .account_service
        .get_holdings_for_account(&account.id)
        .await
        .unwrap();
    let paying_before = holdings_before
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("paying asset holding must exist before dividend");
    let qty_before = paying_before.quantity;
    let vwap_before = paying_before.average_price;
    let pnl_before = paying_before.total_realized_pnl;

    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let holdings_after = ctx
        .account_service
        .get_holdings_for_account(&account.id)
        .await
        .unwrap();
    let paying_after = holdings_after
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("paying asset holding must still exist after dividend");

    assert_eq!(
        paying_after.quantity, qty_before,
        "quantity must be unchanged"
    );
    assert_eq!(
        paying_after.average_price, vwap_before,
        "average_price must be unchanged"
    );
    assert_eq!(
        paying_after.total_realized_pnl, pnl_before,
        "realized_pnl must be unchanged"
    );
}

/// DIV-027 — recording a dividend must not create or modify any AssetPrice row.
#[tokio::test]
async fn record_dividend_does_not_create_asset_price_row() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let latest_price = ctx.asset_service.get_latest_price(&asset.id).await.unwrap();
    assert!(
        latest_price.is_none(),
        "recording a dividend must not create any AssetPrice row (DIV-027)"
    );
}

// -------------------------------------------------------------------------
// DIV-011 — error propagation (representative variant: AssetNotFound)
// -------------------------------------------------------------------------

/// DIV-011 — AccountNotFound surfaces through the full stack.
#[tokio::test]
async fn record_dividend_account_not_found_propagates() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();

    let err = ctx
        .use_case
        .record_dividend(
            "nonexistent-account",
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(100),
            micro(1),
            None,
        )
        .await
        .unwrap_err();

    use folioneer_lib::context::account::AccountError;
    use folioneer_lib::use_cases::holding_transaction::DividendError;
    assert!(
        matches!(
            err,
            DividendError::Account(AccountError::AccountNotFound { .. })
        ),
        "expected Application(AccountNotFound), got: {err:?}"
    );
}

/// DIV-011 — AssetNotHeld: asset exists but is not held in this account.
#[tokio::test]
async fn record_dividend_asset_not_held_propagates() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let err = ctx
        .use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(100),
            micro(1),
            None,
        )
        .await
        .unwrap_err();

    use folioneer_lib::use_cases::holding_transaction::{DividendError, DividendTask};
    assert!(
        matches!(err, DividendError::UseCase(DividendTask::AssetNotHeld)),
        "expected UseCase(AssetNotHeld), got: {err:?}"
    );
}

// -------------------------------------------------------------------------
// DIV-026 — TransactionUpdated event is published on success
// -------------------------------------------------------------------------

/// DIV-026 — recording a dividend publishes the TransactionUpdated event.
#[tokio::test]
async fn record_dividend_publishes_transaction_updated_event() {
    use folioneer_lib::context::account::{
        AccountService, SqliteAccountRepository, SqliteHoldingRepository,
        SqliteTransactionRepository,
    };
    use folioneer_lib::context::asset::{
        AssetService, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
        SqliteAssetRepository,
    };
    use folioneer_lib::core::SideEffectEventBus;

    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let mut rx = bus.subscribe();

    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_fee_schedule_repo(Box::new(SqliteFeeScheduleRepository::new(pool.clone())))
        .with_holding_note_repo(Box::new(SqliteHoldingNoteRepository::new(pool.clone())))
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
    let uc = HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone());

    let asset = asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    // Drain the AssetUpdated event from create_asset.
    let _ = rx.changed().await;

    let account = account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    // Drain AccountUpdated.
    let _ = rx.changed().await;

    uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    // Drain TransactionUpdated from deposit.
    let _ = rx.changed().await;

    uc.buy_holding(
        &account.id,
        asset.id.clone(),
        "2024-01-15".to_string(),
        micro(10),
        micro(50),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    // Drain TransactionUpdated from buy.
    let _ = rx.changed().await;

    uc.record_dividend(
        &account.id,
        asset.id.clone(),
        "2024-06-15".to_string(),
        micro(200),
        micro(1),
        None,
    )
    .await
    .unwrap();

    // The next event must be TransactionUpdated (DIV-026).
    let changed = rx.changed().await;
    assert!(changed.is_ok(), "expected an event after record_dividend");
    let event = rx.borrow().clone();
    use folioneer_lib::core::event_bus::Event;
    assert_eq!(
        event,
        Event::TransactionUpdated,
        "record_dividend must publish TransactionUpdated (DIV-026)"
    );
}

// -------------------------------------------------------------------------
// DIV-070 — HoldingDetail.dividends_received
// -------------------------------------------------------------------------

/// DIV-070 — dividends_received = 0 when no dividend recorded for a holding.
#[tokio::test]
async fn dividends_received_is_zero_when_no_dividend() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    let holding = resp
        .holdings
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("AAPL holding must be in response");
    assert_eq!(
        holding.dividends_received, 0,
        "dividends_received must be 0 when no dividend recorded (DIV-070)"
    );
}

/// DIV-070 — dividends_received = sum of dividend total_amounts for the (account, asset) pair.
#[tokio::test]
async fn dividends_received_sums_all_dividends_for_the_holding() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(2_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    // Record two dividends: 200 + 150 = 350 total.
    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-03-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();
    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(150),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    let holding = resp
        .holdings
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("AAPL holding must be in response");
    assert_eq!(
        holding.dividends_received,
        micro(350),
        "dividends_received must sum all dividend total_amounts (DIV-070)"
    );
}

/// DIV-070 — dividends_received for a different (account, asset) pair is 0 (isolation).
#[tokio::test]
async fn dividends_received_scoped_to_account_asset_pair() {
    let ctx = build_ctx().await;
    let asset_a = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let asset_b = ctx
        .asset_service
        .create_asset(stocks_asset_dto("MSFT", "MSFT", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(5_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset_a.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset_b.id.clone(),
            "2024-01-16".to_string(),
            micro(5),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    // Only record a dividend for asset_a.
    ctx.use_case
        .record_dividend(
            &account.id,
            asset_a.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    let holding_b = resp
        .holdings
        .iter()
        .find(|h| h.asset_id == asset_b.id)
        .expect("MSFT holding must be in response");
    assert_eq!(
        holding_b.dividends_received, 0,
        "dividends_received for MSFT must be 0 — dividend was only for AAPL (DIV-070)"
    );
}

// -------------------------------------------------------------------------
// DIV-071 — HoldingDetail.total_return_pct
// -------------------------------------------------------------------------

/// DIV-071 — total_return_pct is None when performance_pct is None (no price recorded).
#[tokio::test]
async fn total_return_pct_is_none_when_no_price_recorded() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    let holding = resp
        .holdings
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("AAPL holding must be in response");
    // No price recorded → performance_pct is None → total_return_pct must also be None (DIV-071).
    assert!(
        holding.performance_pct.is_none(),
        "performance_pct must be None when no price recorded"
    );
    assert!(
        holding.total_return_pct.is_none(),
        "total_return_pct must be None when performance_pct is None (DIV-071)"
    );
}

/// DIV-071 — total_return_pct = (unrealized_pnl + dividends_received) × 100 / cost_basis.
/// Setup: 10 units bought at 50 USD (cost_basis = 500), current price = 60 USD,
/// unrealized_pnl = (60 - 50) × 10 = 100, dividends = 200,
/// total_return_pct = (100 + 200) × 100 / 500 = 60_000_000 micro-percent.
#[tokio::test]
async fn total_return_pct_combines_unrealized_pnl_and_dividends() {
    let ctx = build_ctx().await;
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    // Record a price of 60 USD per share.
    ctx.asset_service
        .record_asset_price(&asset.id, "2024-06-01", 60.0)
        .await
        .unwrap();

    // Record a dividend of 200 USD.
    ctx.use_case
        .record_dividend(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    let holding = resp
        .holdings
        .iter()
        .find(|h| h.asset_id == asset.id)
        .expect("AAPL holding must be in response");

    // unrealized_pnl = (60 - 50) × 10 = 100 USD = 100_000_000 micros
    // cost_basis = 50 × 10 = 500 USD = 500_000_000 micros
    // dividends_received = 200 USD = 200_000_000 micros
    // total_return_pct = (100_000_000 + 200_000_000) × 100 / 500_000_000 = 60_000_000 micro-%
    assert_eq!(
        holding.dividends_received,
        micro(200),
        "dividends_received must be 200 USD"
    );
    assert_eq!(
        holding.total_return_pct,
        Some(60_000_000),
        "total_return_pct must be 60% (DIV-071)"
    );
}

// -------------------------------------------------------------------------
// DIV-073 — AccountDetailsResponse.total_dividends_received
// -------------------------------------------------------------------------

/// DIV-073 — total_dividends_received = 0 when no dividends recorded.
#[tokio::test]
async fn total_dividends_received_is_zero_when_none() {
    let ctx = build_ctx().await;
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    assert_eq!(
        resp.total_dividends_received, 0,
        "total_dividends_received must be 0 when no dividends recorded (DIV-073)"
    );
}

/// DIV-073 — total_dividends_received = sum across ALL the account's dividend transactions.
#[tokio::test]
async fn total_dividends_received_sums_all_account_dividends() {
    let ctx = build_ctx().await;
    let asset_a = ctx
        .asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let asset_b = ctx
        .asset_service
        .create_asset(stocks_asset_dto("MSFT", "MSFT", "USD"))
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(10_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset_a.id.clone(),
            "2024-01-15".to_string(),
            micro(10),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset_b.id.clone(),
            "2024-01-16".to_string(),
            micro(5),
            micro(100),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    // Dividend from AAPL: 200
    ctx.use_case
        .record_dividend(
            &account.id,
            asset_a.id.clone(),
            "2024-06-15".to_string(),
            micro(200),
            micro(1),
            None,
        )
        .await
        .unwrap();
    // Dividend from MSFT: 75
    ctx.use_case
        .record_dividend(
            &account.id,
            asset_b.id.clone(),
            "2024-06-20".to_string(),
            micro(75),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let resp = ctx
        .details_use_case
        .get_account_details(&account.id, None)
        .await
        .unwrap();

    assert_eq!(
        resp.total_dividends_received,
        micro(275),
        "total_dividends_received must sum all dividend transactions (DIV-073)"
    );
}
