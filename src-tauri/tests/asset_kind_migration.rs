//! Verifies the fill of migration 202610090001 (AST-034): every existing asset takes the
//! kind its class and ISIN make it, and nothing else of it changes. The fill is the
//! migration's own statement, run again here against rows as they stood before kinds.

use sqlx::sqlite::SqlitePoolOptions;

/// The migration's statements after the one that adds the column: the fill.
fn fill_kind() -> &'static str {
    let migration = include_str!("../migrations/202610090001_add_asset_kind.sql");
    let (_add_column, fill) = migration
        .split_once("DEFAULT 'Custom';")
        .expect("the migration adds the column, then fills it");
    fill
}

const EVERYTHING_BUT_THE_KIND: &str = "
    SELECT json_array(id, name, reference, isin, asset_class, currency, risk_level,
                      category_id, exchange_code, is_archived, is_deleted, interest_bearing,
                      price_refresh_blocked, sync_logical_timestamp)
    FROM assets ORDER BY id";

#[tokio::test]
async fn ast_034_every_existing_asset_takes_its_kind_and_nothing_else_changes() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO categories (id, name) VALUES ('system-cash-category','generic.cash')")
        .execute(&pool).await.unwrap();
    for (id, class, isin, exchange, archived) in [
        ("cash", "Cash", None, None, false),
        ("coin", "DigitalAsset", None, None, false),
        (
            "coin-with-isin",
            "DigitalAsset",
            Some("US0378331005"),
            None,
            false,
        ),
        ("share", "Stocks", Some("US0378331005"), Some("XNAS"), true),
        ("blank-isin", "ETF", Some("  "), None, false),
        ("flat", "RealEstate", None, None, false),
        ("keyword-found", "Stocks", None, Some("XPAR"), false),
    ] {
        sqlx::query(
            "INSERT INTO assets (id, name, reference, isin, asset_class, category_id, currency,
                                 risk_level, exchange_code, is_archived)
             VALUES (?, 'Name of ' || ?, upper(?), ?, ?, 'default-uncategorized', 'EUR', 3, ?, ?)",
        )
        .bind(id)
        .bind(id)
        .bind(id)
        .bind(isin)
        .bind(class)
        .bind(exchange)
        .bind(archived)
        .execute(&pool)
        .await
        .unwrap();
    }
    let column_default: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT kind FROM assets")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(
        column_default,
        vec![("Custom".to_string(),)],
        "a row written without a kind starts as the column's default"
    );
    let before: Vec<(String,)> = sqlx::query_as(EVERYTHING_BUT_THE_KIND)
        .fetch_all(&pool)
        .await
        .unwrap();

    sqlx::query(fill_kind()).execute(&pool).await.unwrap();

    let kinds: Vec<(String, String)> = sqlx::query_as("SELECT id, kind FROM assets ORDER BY id")
        .fetch_all(&pool)
        .await
        .unwrap();
    let kinds: Vec<(&str, &str)> = kinds
        .iter()
        .map(|(id, kind)| (id.as_str(), kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("blank-isin", "Custom"),
            ("cash", "Cash"),
            ("coin", "Crypto"),
            ("coin-with-isin", "Crypto"),
            ("flat", "Custom"),
            ("keyword-found", "Custom"),
            ("share", "Listed"),
        ]
    );
    let after: Vec<(String,)> = sqlx::query_as(EVERYTHING_BUT_THE_KIND)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(before, after, "the fill changes the kind and nothing else");
    let changes: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM changes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(changes.0, 0, "the fill records no change to publish");
}
