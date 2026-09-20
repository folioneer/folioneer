/// Integration tests for AccountService read/write operations.
///
/// Covers: delete, get_all, get_by_id, get_holdings_for_account,
/// get_holding_by_account_asset, get_transaction_by_id, get_transactions,
/// get_asset_ids_for_account, get_deletion_summary.
///
/// Uses real SQLite repos against an in-memory DB (B27).
mod common;

use common::micro;
use folioneer_lib::context::account::AccountService;
use folioneer_lib::context::account::{
    AccountError, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
    UpdateFrequency,
};
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

async fn make_service(pool: &sqlx::Pool<sqlx::Sqlite>) -> AccountService {
    AccountService::new(
        Box::new(SqliteAccountRepository::new(pool.clone())),
        Box::new(SqliteHoldingRepository::new(pool.clone())),
        Box::new(SqliteTransactionRepository::new(pool.clone())),
    )
}

async fn seed_asset(pool: &sqlx::Pool<sqlx::Sqlite>) -> String {
    let asset_id = "integ-asset-id".to_string();
    sqlx::query(
        "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&asset_id)
    .bind("IntegAsset")
    .bind("INTG")
    .bind("Stocks")
    .bind("default-uncategorized")
    .bind("EUR")
    .bind(2_i64)
    .execute(pool)
    .await
    .expect("seed test asset");
    asset_id
}

/// Seeds the system Cash Asset row for `currency` + a large Deposit so existing buy/sell
/// integration tests satisfy CSH-041 (purchase eligibility on cash).
async fn seed_cash_for_account(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    svc: &AccountService,
    account_id: &str,
    currency: &str,
) {
    let cash_asset_id = format!("system-cash-{}", currency.to_lowercase());
    sqlx::query(
        "INSERT OR IGNORE INTO categories (id, name, is_deleted) VALUES ('system-cash-category', 'cash', 0)",
    )
    .execute(pool)
    .await
    .expect("seed cash category");
    sqlx::query(
        "INSERT OR IGNORE INTO assets (id, name, reference, asset_class, category_id, currency, risk_level) \
         VALUES (?, ?, ?, 'Cash', 'system-cash-category', ?, 1)",
    )
    .bind(&cash_asset_id)
    .bind(format!("Cash {}", currency.to_uppercase()))
    .bind(currency.to_uppercase())
    .bind(currency)
    .execute(pool)
    .await
    .expect("seed cash asset");
    svc.record_deposit(
        account_id,
        "2020-01-01".to_string(),
        1_000_000_000_000,
        None,
    )
    .await
    .expect("seed cash deposit");
}

/// delete() removes the account so it no longer appears in get_all().
#[tokio::test]
async fn test_delete_account_removes_it_from_get_all() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;

    let account = svc
        .create(
            "ToDelete".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    svc.delete(&account.id).await.unwrap();

    let all = svc.get_all().await.unwrap();
    assert!(!all.iter().any(|a| a.id == account.id));
}

/// get_all() returns all created accounts.
#[tokio::test]
async fn test_get_all_returns_created_accounts() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;

    let a = svc
        .create(
            "Alpha".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    let b = svc
        .create(
            "Beta".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let all = svc.get_all().await.unwrap();
    let ids: Vec<_> = all.iter().map(|x| &x.id).collect();
    assert!(ids.contains(&&a.id));
    assert!(ids.contains(&&b.id));
}

/// get_by_id() returns Some for an existing account and None for an unknown id.
#[tokio::test]
async fn test_get_by_id_returns_some_or_none() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;

    let account = svc
        .create(
            "Existing".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let found = svc.get_by_id(&account.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.expect("account should exist").id, account.id);

    let missing = svc.get_by_id("nonexistent-id").await.unwrap();
    assert!(missing.is_none());
}

/// ACC-026 — bank_name round-trips through create, update, and get_by_id.
#[tokio::test]
async fn bank_name_round_trips_through_create_update_and_read() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;

    let account = svc
        .create(
            "Branded".to_string(),
            "Maple Bank".to_string(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    let loaded = svc.get_by_id(&account.id).await.unwrap().unwrap();
    assert_eq!(loaded.bank_name, "Maple Bank");

    svc.update(
        account.id.clone(),
        "Branded".to_string(),
        String::new(),
        "EUR".to_string(),
        UpdateFrequency::ManualMonth,
        false,
    )
    .await
    .unwrap();
    let cleared = svc.get_by_id(&account.id).await.unwrap().unwrap();
    assert_eq!(cleared.bank_name, "");
}

/// get_holdings_for_account() returns an empty vec when no transactions exist.
#[tokio::test]
async fn test_get_holdings_for_account_returns_empty_before_any_transaction() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;

    let account = svc
        .create(
            "NoHoldings".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    let holdings = svc.get_holdings_for_account(&account.id).await.unwrap();
    assert!(holdings.is_empty());
}

/// get_holding_by_account_asset() returns None before any buy and Some after a buy.
#[tokio::test]
async fn test_get_holding_by_account_asset_returns_none_then_some() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "HoldingTest".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    let before = svc
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap();
    assert!(before.is_none());

    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-01-01".to_string(),
        micro(1),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let after = svc
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap();
    assert!(after.is_some());
}

/// get_transactions() returns transactions in chronological order.
#[tokio::test]
async fn test_get_transactions_returns_chronological_order() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "TxOrder".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-03-01".to_string(),
        micro(1),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-01-01".to_string(),
        micro(1),
        micro(80),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-02-01".to_string(),
        micro(1),
        micro(90),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let txs = svc.get_transactions(&account.id, &asset_id).await.unwrap();
    assert_eq!(txs.len(), 3);
    assert!(txs[0].date <= txs[1].date);
    assert!(txs[1].date <= txs[2].date);
}

/// get_all_transactions_for_account() returns every asset's transactions for the
/// account, ordered chronologically by (date, created_at) — the account-wide journal.
#[tokio::test]
async fn test_get_all_transactions_for_account_spans_assets_chronologically() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_a = seed_asset(&pool).await;
    let asset_b = "integ-asset-b".to_string();
    sqlx::query(
        "INSERT INTO assets (id, name, reference, asset_class, category_id, currency, risk_level)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&asset_b)
    .bind("IntegAssetB")
    .bind("INTB")
    .bind("Stocks")
    .bind("default-uncategorized")
    .bind("EUR")
    .bind(2_i64)
    .execute(&pool)
    .await
    .expect("seed second asset");

    let account = svc
        .create(
            "AllTx".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    // Interleave two assets across non-monotonic dates to prove ordering + cross-asset span.
    svc.buy_holding(
        &account.id,
        asset_a.clone(),
        "2020-03-01".to_string(),
        micro(1),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_b.clone(),
        "2020-01-01".to_string(),
        micro(1),
        micro(50),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_a.clone(),
        "2020-02-01".to_string(),
        micro(1),
        micro(90),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let txs = svc
        .get_all_transactions_for_account(&account.id)
        .await
        .expect("get_all_transactions_for_account must succeed for a valid account");

    // The seeded cash Deposit (2020-01-01) plus the 3 buys = 4 transactions.
    assert_eq!(
        txs.len(),
        4,
        "all assets' transactions plus the cash deposit"
    );
    assert!(
        txs.windows(2).all(|w| w[0].date <= w[1].date),
        "transactions must be chronologically ordered across all assets"
    );
    assert!(
        txs.iter().any(|t| t.asset_id == asset_a) && txs.iter().any(|t| t.asset_id == asset_b),
        "result must span both assets"
    );
}

/// get_transaction_by_id() returns Some for an existing transaction and None for an unknown id.
#[tokio::test]
async fn test_get_transaction_by_id_returns_some_or_none() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "TxLookup".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-04-01".to_string(),
        micro(2),
        micro(150),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let txs = svc.get_transactions(&account.id, &asset_id).await.unwrap();
    let created_tx_id = &txs[0].id;

    let tx = svc
        .get_transaction_by_id(created_tx_id)
        .await
        .unwrap()
        .expect("existing transaction must be returned");
    assert_eq!(&tx.id, created_tx_id);
    assert_eq!(tx.account_id, account.id);
    assert_eq!(tx.asset_id, asset_id);

    let missing = svc
        .get_transaction_by_id("nonexistent-tx-id")
        .await
        .unwrap();
    assert!(missing.is_none(), "unknown id must return None");
}

/// get_asset_ids_for_account() returns distinct asset IDs that have transactions.
#[tokio::test]
async fn test_get_asset_ids_for_account_deduplicates() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "AssetIds".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    // Two buys on the same asset — must deduplicate to a single ID.
    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-01-01".to_string(),
        micro(1),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-02-01".to_string(),
        micro(1),
        micro(110),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let ids = svc.get_asset_ids_for_account(&account.id).await.unwrap();
    // Two distinct asset_ids: the test asset + system Cash Asset (CSH-090).
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&asset_id));
    assert!(ids.iter().any(|id| id.starts_with("system-cash-")));
}

/// get_deletion_summary() counts active holdings and total transactions correctly.
#[tokio::test]
async fn test_get_deletion_summary_counts_holdings_and_transactions() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "Summary".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-01-01".to_string(),
        micro(2),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2020-02-01".to_string(),
        micro(1),
        micro(110),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();

    let (holding_count, tx_count) = svc.get_deletion_summary(&account.id).await.unwrap();
    // 2 active holdings: the test asset + the Cash Holding (CSH-090).
    assert_eq!(holding_count, 2, "asset holding + cash holding");
    // 3 transactions: 2 purchases + the seeded Deposit.
    assert_eq!(tx_count, 3, "two purchases plus the seeded deposit");
}

// CSH-012 — seed_cash_holding rejects an unknown account (AccountNotFound branch).
#[tokio::test]
async fn seed_cash_holding_errors_when_account_missing() {
    let pool = make_pool().await;
    let service = make_service(&pool).await;

    let result = service.seed_cash_holding("does-not-exist").await;

    assert!(
        result.is_err(),
        "seed_cash_holding must reject an unknown account id"
    );
}

// SEL-024 / SEL-030 — full-stack guard: correcting a sell to a date before its buy is
// rejected with CascadingOversell. Transactions hydrate from SQLite ordered by date
// (ORDER BY date ASC, created_at ASC), the exact reload condition under which the
// chronological replay must run — independent of the in-session append order.
#[tokio::test]
async fn correct_transaction_rejects_moving_sell_before_buy_end_to_end() {
    let pool = make_pool().await;
    let svc = make_service(&pool).await;
    let asset_id = seed_asset(&pool).await;

    let account = svc
        .create(
            "ReorderGuard".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    seed_cash_for_account(&pool, &svc, &account.id, "EUR").await;

    svc.buy_holding(
        &account.id,
        asset_id.clone(),
        "2024-06-01".to_string(),
        micro(2),
        micro(100),
        micro(1),
        0,
        None,
        None,
    )
    .await
    .unwrap();
    let sell = svc
        .sell_holding(
            &account.id,
            asset_id.clone(),
            "2024-07-01".to_string(),
            micro(1),
            micro(150),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let err = svc
        .correct_transaction(
            &account.id,
            &sell.id,
            "2024-05-01".to_string(),
            micro(1),
            micro(150),
            micro(1),
            0,
            None, // total_amount (typed-total mode unused here)
            None,
        )
        .await
        .expect_err("moving a sell before its buy must be rejected");
    assert!(
        matches!(err, AccountError::CascadingOversell),
        "expected CascadingOversell, got: {err:?}"
    );

    // The rejected correction must not have mutated the persisted sell.
    let persisted = svc
        .get_transaction_by_id(&sell.id)
        .await
        .unwrap()
        .expect("sell transaction must still exist after a rejected correction");
    assert_eq!(
        persisted.date, "2024-07-01",
        "rejected correction must leave the sell's date unchanged"
    );
}
