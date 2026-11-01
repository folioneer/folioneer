//! Verifies the fill of migration 202611010001 (SPL-010, DIV-023): a split's factor and a
//! dividend's amount move unchanged into their fields, `quantity` becoming 0, and nothing
//! else moves. The fill is the migration's own statements, run again here against rows as
//! they stood before the fields.

use sqlx::sqlite::SqlitePoolOptions;

/// The migration's statements after the two that add the columns: the fill.
fn fill() -> &'static str {
    let migration = include_str!("../migrations/202611010001_split_factor_dividend_amount.sql");
    let (_add_columns, fill) = migration
        .split_once("ADD COLUMN dividend_amount INTEGER;")
        .expect("the migration adds the columns, then fills them");
    fill
}

/// id, quantity, split factor, dividend amount, unit price, total.
type Row = (String, i64, Option<i64>, Option<i64>, i64, i64);

#[tokio::test]
async fn spl_010_div_023_the_figures_move_to_their_fields_unchanged() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&pool)
        .await
        .unwrap();
    for (id, kind, quantity) in [
        ("split", "Split", 20_000_000_i64),
        ("dividend", "Dividend", 12_345_678),
        ("purchase", "Purchase", 3_000_000),
        ("deposit", "Deposit", 500_000_000),
    ] {
        sqlx::query(
            "INSERT INTO transactions (id, account_id, asset_id, transaction_type, date, quantity,
                 unit_price, exchange_rate, fees, total_amount, created_at)
             VALUES (?, 'a', 'b', ?, '2024-01-01', ?, 7, 1000000, 0, 9, '2024-01-01T00:00:00Z')",
        )
        .bind(id)
        .bind(kind)
        .bind(quantity)
        .execute(&pool)
        .await
        .unwrap();
    }

    sqlx::raw_sql(fill()).execute(&pool).await.unwrap();

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, quantity, split_factor, dividend_amount, unit_price, total_amount
         FROM transactions ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            ("deposit".to_string(), 500_000_000, None, None, 7, 9),
            ("dividend".to_string(), 0, None, Some(12_345_678), 7, 9),
            ("purchase".to_string(), 3_000_000, None, None, 7, 9),
            ("split".to_string(), 0, Some(20_000_000), None, 7, 9),
        ]
    );
}
