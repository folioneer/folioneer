//! Verifies the eager-cash backfill (migration 202606200001) is insert-if-absent
//! and preserves existing cash balances (CSH-012). The backfill INSERT mirrors the
//! migration's step (3); re-running it against populated data must be a no-op for
//! accounts that already hold cash and seed a 0-balance row for those that don't.

use sqlx::sqlite::SqlitePoolOptions;

/// Mirror of migration 202606200001 step (3) — re-run here to exercise the
/// insert-if-absent path against pre-existing data.
const BACKFILL_HOLDINGS: &str = "
    INSERT INTO holdings (id, account_id, asset_id, quantity, average_price, total_realized_pnl, last_sold_date)
    SELECT 'cash-' || a.id, a.id, 'system-cash-' || lower(a.currency), 0, 1000000, 0, NULL
    FROM accounts a
    WHERE NOT EXISTS (
        SELECT 1 FROM holdings h
        WHERE h.account_id = a.id AND h.asset_id = 'system-cash-' || lower(a.currency)
    )";

#[tokio::test]
async fn backfill_preserves_existing_cash_and_seeds_missing() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Cash category + asset (seeded by the migration on an empty accounts table is a
    // no-op, so insert them explicitly to simulate a prior runtime seed).
    sqlx::query("INSERT OR IGNORE INTO categories (id, name) VALUES ('system-cash-category','generic.cash')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO assets (id,name,reference,asset_class,category_id,currency,risk_level) VALUES ('system-cash-eur','Cash EUR','EUR','Cash','system-cash-category','EUR',1)")
        .execute(&pool).await.unwrap();

    // Account A — legacy account that already has a cash holding with a non-zero balance.
    sqlx::query("INSERT INTO accounts (id,name,update_frequency,currency) VALUES ('acct-a','A','ManualMonth','EUR')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO holdings (id,account_id,asset_id,quantity,average_price) VALUES ('legacy-cash','acct-a','system-cash-eur',777,1000000)")
        .execute(&pool).await.unwrap();

    // Account B — no cash holding yet.
    sqlx::query("INSERT INTO accounts (id,name,update_frequency,currency) VALUES ('acct-b','B','ManualMonth','EUR')")
        .execute(&pool).await.unwrap();

    sqlx::query(BACKFILL_HOLDINGS).execute(&pool).await.unwrap();

    // A's existing holding is untouched — id and balance preserved, no duplicate.
    let (id_a, qty_a): (String, i64) = sqlx::query_as(
        "SELECT id, quantity FROM holdings WHERE account_id='acct-a' AND asset_id='system-cash-eur'",
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(id_a, "legacy-cash");
    assert_eq!(qty_a, 777, "existing cash balance must be preserved");

    // B receives a fresh 0-balance holding.
    let (id_b, qty_b): (String, i64) = sqlx::query_as(
        "SELECT id, quantity FROM holdings WHERE account_id='acct-b' AND asset_id='system-cash-eur'",
    )
    .fetch_one(&pool).await.unwrap();
    assert_eq!(id_b, "cash-acct-b");
    assert_eq!(qty_b, 0);
}
