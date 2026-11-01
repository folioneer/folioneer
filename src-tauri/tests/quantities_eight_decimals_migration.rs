//! Verifies migration 202611010002 (TRX-024, SYN-038): a non-cash quantity is multiplied by
//! 100 exactly, in the tables and in what this device logged or holds back; the Cash Holding,
//! a split's factor and a dividend's amount do not move; a quantity that would not fit leaves
//! everything as it was.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

const MIGRATION: &str = include_str!("../migrations/202611010002_quantities_eight_decimals.sql");

/// The tables as the migration finds them, with what it reads and writes.
async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(
        "CREATE TABLE holdings (id TEXT PRIMARY KEY, asset_id TEXT, quantity INTEGER);
         CREATE TABLE transactions (id TEXT PRIMARY KEY, asset_id TEXT, quantity INTEGER);
         CREATE TABLE changes (id TEXT PRIMARY KEY, record_kind TEXT, content TEXT);
         CREATE TABLE held_back_changes (id TEXT PRIMARY KEY, payload TEXT NOT NULL);",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

fn content(asset_id: &str, kind: &str, quantity: i64) -> String {
    format!(r#"{{"asset_id":"{asset_id}","transaction_type":"{kind}","quantity":{quantity}}}"#)
}

async fn quantity_in(pool: &SqlitePool, table: &str, id: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT quantity FROM {table} WHERE id = ?"))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn logged_quantity(pool: &SqlitePool, id: &str) -> i64 {
    sqlx::query_scalar("SELECT json_extract(content, '$.quantity') FROM changes WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

// TRX-024 — ×100 exactly on what is held and traded, nothing on the Cash Holding, a sub-unit
// figure keeping every digit.
#[tokio::test]
async fn trx_024_a_non_cash_quantity_moves_to_hundred_millionths_exactly() {
    let pool = pool().await;
    sqlx::raw_sql(
        "INSERT INTO holdings VALUES
             ('stock', 'asset-1', 1234567), ('cash', 'system-cash-eur', 500000000);
         INSERT INTO transactions VALUES
             ('purchase', 'asset-1', 1234567),
             ('deposit', 'system-cash-eur', 500000000),
             ('split', 'asset-1', 0),
             ('dividend', 'asset-1', 0);",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();

    assert_eq!(quantity_in(&pool, "holdings", "stock").await, 123_456_700);
    assert_eq!(quantity_in(&pool, "holdings", "cash").await, 500_000_000);
    assert_eq!(
        quantity_in(&pool, "transactions", "purchase").await,
        123_456_700
    );
    assert_eq!(
        quantity_in(&pool, "transactions", "deposit").await,
        500_000_000
    );
    assert_eq!(quantity_in(&pool, "transactions", "split").await, 0);
    assert_eq!(quantity_in(&pool, "transactions", "dividend").await, 0);
}

// SYN-038 — what is logged is published in the new form; only a transaction's non-cash
// quantity moves, the rest of its content and other kinds of record staying as they are.
#[tokio::test]
async fn syn_038_a_logged_change_moves_with_its_transaction() {
    let pool = pool().await;
    for (id, kind, body) in [
        ("purchase", "Transaction", content("asset-1", "Purchase", 7)),
        (
            "deposit",
            "Transaction",
            content("system-cash-eur", "Deposit", 7),
        ),
        ("split", "Transaction", content("asset-1", "Split", 7)),
        ("dividend", "Transaction", content("asset-1", "Dividend", 7)),
        ("other", "Asset", r#"{"quantity":7}"#.to_string()),
    ] {
        sqlx::query("INSERT INTO changes VALUES (?, ?, ?)")
            .bind(id)
            .bind(kind)
            .bind(body)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO changes VALUES ('removed', 'Transaction', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();

    assert_eq!(logged_quantity(&pool, "purchase").await, 700);
    for unmoved in ["deposit", "split", "dividend", "other"] {
        assert_eq!(logged_quantity(&pool, unmoved).await, 7, "{unmoved}");
    }
}

// SYN-038 — a change held back is applied later, in the new form; its content is a string
// inside the payload and stays one.
#[tokio::test]
async fn syn_038_a_change_held_back_moves_inside_its_payload() {
    let pool = pool().await;
    let payload = |kind: &str, body: &str| {
        serde_json::json!({ "record_kind": kind, "content": body }).to_string()
    };
    for (id, body) in [
        (
            "purchase",
            payload("Transaction", &content("asset-1", "Purchase", 7)),
        ),
        (
            "split",
            payload("Transaction", &content("asset-1", "Split", 7)),
        ),
        ("asset", payload("Asset", r#"{"quantity":7}"#)),
    ] {
        sqlx::query("INSERT INTO held_back_changes VALUES (?, ?)")
            .bind(id)
            .bind(body)
            .execute(&pool)
            .await
            .unwrap();
    }

    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();

    let held = |id: &'static str| {
        let pool = pool.clone();
        async move {
            let payload: String =
                sqlx::query_scalar("SELECT payload FROM held_back_changes WHERE id = ?")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
            let content: serde_json::Value =
                serde_json::from_str(payload["content"].as_str().expect("still a string")).unwrap();
            content["quantity"].as_i64().unwrap()
        }
    };
    assert_eq!(held("purchase").await, 700);
    assert_eq!(held("split").await, 7);
    assert_eq!(held("asset").await, 7);
}

// TRX-024 — SQLite would turn an overflowing product into a rounded real: the migration fails
// instead, and its transaction leaves the portfolio as it was.
#[tokio::test]
async fn trx_024_a_quantity_that_would_not_fit_refuses_the_whole_migration() {
    for table in ["holdings", "transactions"] {
        let pool = pool().await;
        sqlx::raw_sql(
            "INSERT INTO holdings VALUES ('kept', 'asset-1', 5);
             INSERT INTO transactions VALUES ('kept', 'asset-1', 5);",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(&format!(
            "INSERT INTO {table} VALUES ('huge', 'asset-1', 92233720368547759)"
        ))
        .execute(&pool)
        .await
        .unwrap();

        let mut transaction = pool.begin().await.unwrap();
        let refused = sqlx::raw_sql(MIGRATION).execute(&mut *transaction).await;
        let refusal = refused.expect_err(table).to_string();
        assert!(refusal.contains("CHECK constraint failed"), "{refusal}");
        transaction.rollback().await.unwrap();

        assert_eq!(quantity_in(&pool, "holdings", "kept").await, 5);
        assert_eq!(quantity_in(&pool, "transactions", "kept").await, 5);
    }
}

// TRX-024 — the largest quantity that fits still multiplies exactly.
#[tokio::test]
async fn trx_024_the_largest_quantity_that_fits_moves_exactly() {
    let pool = pool().await;
    sqlx::raw_sql("INSERT INTO holdings VALUES ('largest', 'asset-1', 92233720368547758);")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::raw_sql(MIGRATION).execute(&pool).await.unwrap();

    assert_eq!(
        quantity_in(&pool, "holdings", "largest").await,
        9_223_372_036_854_775_800
    );
}

// SYN-038 — a logged or held change too large to move refuses the migration too: left in
// millionths, it would be published in the new form and read 100 times too low.
#[tokio::test]
async fn syn_038_a_logged_change_that_would_not_fit_refuses_the_whole_migration() {
    let huge = content("asset-1", "Purchase", 92233720368547759);
    let held = serde_json::json!({ "record_kind": "Transaction", "content": huge }).to_string();
    for (insert, value) in [
        (
            "INSERT INTO changes VALUES ('huge', 'Transaction', ?)",
            huge,
        ),
        ("INSERT INTO held_back_changes VALUES ('huge', ?)", held),
    ] {
        let pool = pool().await;
        sqlx::query(insert)
            .bind(value)
            .execute(&pool)
            .await
            .unwrap();

        let mut transaction = pool.begin().await.unwrap();
        let refused = sqlx::raw_sql(MIGRATION).execute(&mut *transaction).await;
        let refusal = refused.expect_err(insert).to_string();
        assert!(refusal.contains("CHECK constraint failed"), "{refusal}");
        transaction.rollback().await.unwrap();
    }
}
