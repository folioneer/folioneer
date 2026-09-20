//! Integration tests for the `record_split` use-case (SPL spec).
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `HoldingTransactionUseCase` → `AccountService` / `AssetService` → real
//! in-memory SQLite. No mocks — per test_convention.md Tier 3 constraint.
//! Mirrors `free_shares_crud.rs`, the sibling zero-cash transaction suite.

use folioneer_lib::context::account::{
    AccountError, AccountService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository, TransactionType, UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::holding_transaction::{HoldingTransactionUseCase, SplitError};
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

    let use_case = HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone());

    Ctx {
        use_case,
        account_service,
        asset_service,
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

/// Seeds an account holding 10 units of a Stocks asset bought at 50 on
/// 2024-01-15 (cost basis 500), the shared starting position of every test.
async fn seed_held_position(ctx: &Ctx) -> (String, String) {
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

    (account.id, asset.id)
}

// -------------------------------------------------------------------------
// SPL-010/020 — happy-path end-to-end
// -------------------------------------------------------------------------

/// SPL-010/020 — record_split end-to-end: persists a Split transaction packing
/// the micro-scaled factor in `quantity` with the no-money convention
/// (unit_price=0, exchange_rate=1_000_000, fees=0, total_amount=0,
/// realized_pnl=None); the holding quantity rescales by the factor and the
/// cost basis (vwap numerator) is preserved.
#[tokio::test]
async fn record_split_rescales_quantity_and_preserves_cost_basis() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    // SPL-010 — record a 20-for-1 split
    let tx = ctx
        .use_case
        .record_split(
            &account_id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            micro(20),
            Some("20-for-1".to_string()),
        )
        .await
        .unwrap();

    // SPL-010 — contract packing convention
    assert_eq!(tx.transaction_type, TransactionType::Split);
    assert_eq!(tx.account_id, account_id);
    assert_eq!(tx.asset_id, asset_id);
    assert_eq!(
        tx.quantity,
        micro(20),
        "the micro-scaled factor must ride in quantity (SPL-010)"
    );
    assert_eq!(tx.unit_price, 0, "unit_price must be 0 (SPL-010)");
    assert_eq!(
        tx.exchange_rate, 1_000_000,
        "exchange_rate must be 1_000_000 (SPL-010)"
    );
    assert_eq!(tx.fees, 0, "fees must be 0 (SPL-010)");
    assert_eq!(
        tx.total_amount, 0,
        "total_amount must be 0 — a split moves no money (SPL-010)"
    );
    assert!(tx.realized_pnl.is_none(), "realized_pnl must be None");
    assert_eq!(tx.note.as_deref(), Some("20-for-1"));

    // SPL-020 — 10 units @ 50 → 200 units @ 2.50, cost basis 500 preserved
    let holding = ctx
        .account_service
        .get_holding_by_account_asset(&account_id, &asset_id)
        .await
        .unwrap()
        .expect("holding must survive the split");
    assert_eq!(
        holding.quantity,
        micro(200),
        "quantity must rescale by the factor (SPL-020)"
    );
    assert_eq!(
        holding.average_price, 2_500_000,
        "average price must rescale to 2.50 (SPL-020)"
    );
    assert_eq!(
        holding.quantity as i128 * holding.average_price as i128 / 1_000_000,
        micro(500) as i128,
        "cost basis must be preserved across the rescale (SPL-020)"
    );
}

// -------------------------------------------------------------------------
// SPL-011/012/021 — error propagation through the real stack
// -------------------------------------------------------------------------

/// SPL-011 — factor bounds are enforced through the full stack: zero (or
/// negative) factors and the ×1 identity factor are rejected.
#[tokio::test]
async fn record_split_invalid_factor_rejected() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    // SPL-011 — factor must be strictly positive
    let err = ctx
        .use_case
        .record_split(
            &account_id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            0,
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            SplitError::Account(AccountError::SplitFactorNotPositive)
        ),
        "expected Account(SplitFactorNotPositive), got: {err:?}"
    );

    // SPL-011 — a ×1 split is a no-op data-entry error
    let err = ctx
        .use_case
        .record_split(
            &account_id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            micro(1),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, SplitError::Account(AccountError::SplitFactorIsOne)),
        "expected Account(SplitFactorIsOne), got: {err:?}"
    );
}

/// SPL-012 — the cash line cannot be split.
#[tokio::test]
async fn record_split_on_cash_line_rejected() {
    let ctx = build_ctx().await;
    let cash_asset = ctx.asset_service.seed_cash_asset("USD").await.unwrap();
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
        .record_split(
            &account.id,
            cash_asset.id.clone(),
            "2024-06-15".to_string(),
            micro(2),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, SplitError::Account(AccountError::SplitOnCashAsset)),
        "expected Account(SplitOnCashAsset), got: {err:?}"
    );
}

/// SPL-012 — the chronological replay rejects a split dated where the
/// replayed position quantity is zero (before the first purchase).
#[tokio::test]
async fn record_split_dated_before_position_opens_rejected() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    // Position opens 2024-01-15; a split dated 2024-01-10 replays on quantity 0.
    let err = ctx
        .use_case
        .record_split(
            &account_id,
            asset_id.clone(),
            "2024-01-10".to_string(),
            micro(2),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, SplitError::Account(AccountError::ClosedPosition)),
        "expected Account(ClosedPosition), got: {err:?}"
    );
}
