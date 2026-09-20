//! Integration tests for AccountService::open_holding (TRX-042, TRX-047, TRX-056).
//!
//! Asset existence / archived-status checks are tested in the OpenHoldingUseCase orchestrator.
//! These tests exercise AccountService directly: persist correct fields, account-not-found.

use folioneer_lib::context::account::{
    AccountError, AccountService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository, TransactionType, UpdateFrequency,
};
use sqlx::sqlite::SqlitePoolOptions;

fn micro(v: i64) -> i64 {
    v * 1_000_000
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

async fn setup_with_active_asset() -> (AccountService, sqlx::Pool<sqlx::Sqlite>, String) {
    let pool = make_pool().await;
    let svc = AccountService::new(
        Box::new(SqliteAccountRepository::new(pool.clone())),
        Box::new(SqliteHoldingRepository::new(pool.clone())),
        Box::new(SqliteTransactionRepository::new(pool.clone())),
    );
    let asset_id = "test-asset-id".to_string();
    sqlx::query(
        "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&asset_id)
    .bind("TestAsset")
    .bind("TST")
    .bind("Stocks")
    .bind("default-uncategorized")
    .bind("USD")
    .bind(1_i64)
    .execute(&pool)
    .await
    .unwrap();
    (svc, pool, asset_id)
}

/// TRX-047 — happy path: open_holding persists transaction and holding with correct fields.
/// Verifies transaction_type = OpeningBalance, total_amount = total_cost, fees = 0,
/// exchange_rate = 1_000_000, and unit_price = floor(total_cost * MICRO / quantity).
#[tokio::test]
async fn open_holding_end_to_end_persists_correct_fields() {
    let (svc, _pool, asset_id) = setup_with_active_asset().await;
    let account = svc
        .create(
            "Acc".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let tx = svc
        .open_holding(
            &account.id,
            asset_id.clone(),
            "2024-01-01".to_string(),
            micro(2),
            micro(200),
        )
        .await
        .unwrap();

    assert_eq!(tx.transaction_type, TransactionType::OpeningBalance);
    assert_eq!(
        tx.total_amount,
        micro(200),
        "total_amount must equal total_cost"
    );
    assert_eq!(tx.fees, 0, "fees must be 0");
    assert_eq!(tx.exchange_rate, 1_000_000, "exchange_rate must be 1.0");
    // unit_price = floor(200_000_000 * 1_000_000 / 2_000_000) = 100_000_000
    assert_eq!(
        tx.unit_price,
        micro(100),
        "unit_price = total_cost / quantity"
    );
    assert_eq!(tx.account_id, account.id);
    assert_eq!(tx.asset_id, asset_id);

    let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
    assert_eq!(holdings.len(), 1);
    assert_eq!(holdings[0].quantity, micro(2));
    assert_eq!(holdings[0].average_price, micro(100));
}

/// TRX-056 — AccountNotFound error propagates end-to-end when account does not exist.
#[tokio::test]
async fn open_holding_account_not_found_propagates() {
    let (svc, _pool, asset_id) = setup_with_active_asset().await;

    let err = svc
        .open_holding(
            "nonexistent-account-id",
            asset_id,
            "2024-01-01".to_string(),
            micro(1),
            micro(100),
        )
        .await
        .unwrap_err();

    use folioneer_lib::use_cases::holding_transaction::OpenHoldingError;
    assert!(
        matches!(
            err,
            OpenHoldingError::Account(AccountError::AccountNotFound { .. })
        ),
        "expected Application(AccountNotFound), got: {err:?}"
    );
}
