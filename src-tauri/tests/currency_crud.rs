//! Integration tests for the currency rate CRUD feature (FXR-025, FXR-050–054).
//!
//! These tests exercise the full stack:
//! `CurrencyService → SqliteCurrencyPairRepository / SqliteCurrencyRateRepository → SQLite`.
//! Only the public API of `folioneer_lib` is used — no `crate::` imports,
//! no repository calls.

use folioneer_lib::context::currency::{
    CurrencyError, CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::{Event, SideEffectEventBus};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

async fn setup() -> (CurrencyService, Arc<SideEffectEventBus>) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("test pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    let bus = Arc::new(SideEffectEventBus::new());
    let pair_repo = Box::new(SqliteCurrencyPairRepository::new(pool.clone()));
    let rate_repo = Box::new(SqliteCurrencyRateRepository::new(pool));
    let svc = CurrencyService::new(pair_repo, rate_repo).with_event_bus(Arc::clone(&bus));
    (svc, bus)
}

/// FXR-054 — declare_currency_pair persists a new pair and returns it.
#[tokio::test]
async fn declare_currency_pair_end_to_end() {
    let (svc, _bus) = setup().await;

    let pair = svc
        .declare_currency_pair("USD".to_string(), "EUR".to_string())
        .await
        .expect("declare_currency_pair should succeed");

    assert_eq!(pair.from_currency, "USD");
    assert_eq!(pair.to_currency, "EUR");
}

/// FXR-054 — declare_currency_pair is idempotent: declaring an existing pair
/// returns the existing pair without creating a duplicate.
#[tokio::test]
async fn declare_currency_pair_idempotent_end_to_end() {
    let (svc, _bus) = setup().await;

    svc.declare_currency_pair("USD".to_string(), "EUR".to_string())
        .await
        .unwrap();
    let second = svc
        .declare_currency_pair("USD".to_string(), "EUR".to_string())
        .await
        .expect("second declare should succeed idempotently");

    assert_eq!(second.from_currency, "USD");

    let pairs = svc.list_currency_pairs().await.unwrap();
    assert_eq!(pairs.len(), 1, "no duplicate pair should be created");
}

/// FXR-025 / FXR-013 — record_currency_rate persists the rate and auto-creates
/// the pair as a side-effect when it did not previously exist.
#[tokio::test]
async fn record_currency_rate_end_to_end() {
    let (svc, _bus) = setup().await;

    let rate = svc
        .record_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            920_000,
        )
        .await
        .expect("record_currency_rate should succeed");

    assert_eq!(rate.from_currency, "USD");
    assert_eq!(rate.to_currency, "EUR");
    assert_eq!(rate.date, "2026-01-01");
    assert_eq!(rate.rate, 920_000);

    let pairs = svc.list_currency_pairs().await.unwrap();
    assert_eq!(pairs.len(), 1, "pair should have been auto-created");
}

/// FXR-026 — record_currency_rate publishes CurrencyRateUpdated through the full stack.
#[tokio::test]
async fn record_currency_rate_publishes_event_end_to_end() {
    let (svc, bus) = setup().await;
    let mut rx = bus.subscribe();

    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
        920_000,
    )
    .await
    .unwrap();

    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), Event::CurrencyRateUpdated);
}

/// FXR-053 — delete_currency_rate returns RateNotFound when the record does not exist.
#[tokio::test]
async fn delete_currency_rate_not_found_propagates_end_to_end() {
    let (svc, _bus) = setup().await;

    let err = svc
        .delete_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            &err,
            CurrencyError::RateNotFound { from_currency, to_currency, date }
                if from_currency == "USD" && to_currency == "EUR" && date == "2026-01-01"
        ),
        "expected RateNotFound, got: {err:?}"
    );
}

/// FXR-014 — delete_currency_rate never removes the pair.
#[tokio::test]
async fn delete_currency_rate_does_not_remove_pair_end_to_end() {
    let (svc, _bus) = setup().await;

    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
        920_000,
    )
    .await
    .unwrap();

    svc.delete_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
    )
    .await
    .unwrap();

    let pairs = svc.list_currency_pairs().await.unwrap();
    assert_eq!(pairs.len(), 1, "pair must survive after rate is deleted");

    let rates = svc
        .list_currency_rates("USD".to_string(), "EUR".to_string())
        .await
        .unwrap();
    assert!(rates.is_empty(), "rate must have been removed");
}

/// FXR-050 — list_currency_rates returns all rows for the pair ordered date descending.
#[tokio::test]
async fn list_currency_rates_ordered_descending_end_to_end() {
    let (svc, _bus) = setup().await;

    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
        910_000,
    )
    .await
    .unwrap();
    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-03".to_string(),
        930_000,
    )
    .await
    .unwrap();
    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-02".to_string(),
        920_000,
    )
    .await
    .unwrap();

    let rates = svc
        .list_currency_rates("USD".to_string(), "EUR".to_string())
        .await
        .unwrap();

    assert_eq!(rates.len(), 3);
    assert_eq!(rates[0].date, "2026-01-03");
    assert_eq!(rates[1].date, "2026-01-02");
    assert_eq!(rates[2].date, "2026-01-01");
}

/// FXR-052 — update_currency_rate with a date change deletes the original
/// and upserts at the new date.
#[tokio::test]
async fn update_currency_rate_date_change_end_to_end() {
    let (svc, _bus) = setup().await;

    svc.record_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
        920_000,
    )
    .await
    .unwrap();

    svc.update_currency_rate(
        "USD".to_string(),
        "EUR".to_string(),
        "2026-01-01".to_string(),
        "2026-01-02".to_string(),
        950_000,
    )
    .await
    .unwrap();

    let rates = svc
        .list_currency_rates("USD".to_string(), "EUR".to_string())
        .await
        .unwrap();

    assert_eq!(rates.len(), 1, "original date should be gone");
    assert_eq!(rates[0].date, "2026-01-02");
    assert_eq!(rates[0].rate, 950_000);
}

/// FXR-052 — update_currency_rate returns RateNotFound when the original does not exist.
#[tokio::test]
async fn update_currency_rate_not_found_propagates_end_to_end() {
    let (svc, _bus) = setup().await;

    let err = svc
        .update_currency_rate(
            "USD".to_string(),
            "EUR".to_string(),
            "2026-01-01".to_string(),
            "2026-01-02".to_string(),
            950_000,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            &err,
            CurrencyError::RateNotFound { from_currency, to_currency, date }
                if from_currency == "USD" && to_currency == "EUR" && date == "2026-01-01"
        ),
        "expected RateNotFound, got: {err:?}"
    );
}
