use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::core::{Event, SideEffectEventBus};
/// Integration tests for the price history CRUD feature (MKT-072, MKT-083, MKT-084, MKT-090).
///
/// These tests exercise the full stack: AssetService → SqliteAssetPriceRepository → SQLite.
/// They complement the unit tests in service.rs (which also use real SQLite) by living in
/// tests/ and being compiled as a separate crate — only the public API is accessible here.
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

async fn setup() -> (AssetService, Arc<SideEffectEventBus>) {
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
    let svc = AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool)),
    )
    .with_event_bus(Arc::clone(&bus));
    (svc, bus)
}

async fn create_asset(svc: &AssetService) -> String {
    svc.create_asset(CreateAssetDTO {
        name: "Apple".to_string(),
        reference: "AAPL".to_string(),
        isin: None,
        class: AssetClass::Stocks,
        currency: "USD".to_string(),
        risk_level: 3,
        category_id: SYSTEM_CATEGORY_ID.to_string(),
        exchange: None,
        interest_bearing: false,
    })
    .await
    .expect("create asset")
    .id
}

/// MKT-072 — get_asset_prices returns all rows sorted date descending and scoped to the asset.
#[tokio::test]
async fn get_asset_prices_returns_all_sorted_descending_and_scoped() {
    let (svc, _bus) = setup().await;
    let asset_a = create_asset(&svc).await;
    let asset_b = svc
        .create_asset(CreateAssetDTO {
            name: "Google".to_string(),
            reference: "GOOG".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("create asset b")
        .id;

    svc.record_asset_price(&asset_a, "2026-01-01", 100.0)
        .await
        .unwrap();
    svc.record_asset_price(&asset_a, "2026-01-03", 130.0)
        .await
        .unwrap();
    svc.record_asset_price(&asset_a, "2026-01-02", 120.0)
        .await
        .unwrap();
    svc.record_asset_price(&asset_b, "2026-01-01", 500.0)
        .await
        .unwrap();

    let prices = svc.get_asset_prices(&asset_a).await.unwrap();

    assert_eq!(prices.len(), 3, "only asset_a rows returned");
    assert!(
        prices.iter().all(|p| p.asset_id == asset_a),
        "all rows belong to asset_a"
    );
    assert_eq!(prices[0].date, "2026-01-03");
    assert_eq!(prices[1].date, "2026-01-02");
    assert_eq!(prices[2].date, "2026-01-01");
    assert_eq!(prices[0].price, 130_000_000);
}

/// MKT-084 — update_asset_price with a date change atomically removes the old record
/// and inserts the new one. Verifies the DB state after the operation.
#[tokio::test]
async fn update_asset_price_date_change_is_atomic_end_to_end() {
    let (svc, bus) = setup().await;
    let mut rx = bus.subscribe();
    let asset_id = create_asset(&svc).await;

    svc.record_asset_price(&asset_id, "2026-01-01", 100.0)
        .await
        .unwrap();
    // Drain the record_asset_price event
    rx.changed().await.unwrap();

    // Pre-existing record at the target date — must be silently overwritten (MKT-084)
    svc.record_asset_price(&asset_id, "2026-01-02", 105.0)
        .await
        .unwrap();
    rx.changed().await.unwrap();

    svc.update_asset_price(&asset_id, "2026-01-01", "2026-01-02", 200.0)
        .await
        .unwrap();

    // MKT-085 — event fired
    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);

    // DB state: only 2026-01-02 row remains with the updated price
    let prices = svc.get_asset_prices(&asset_id).await.unwrap();
    assert_eq!(prices.len(), 1, "old date must be removed");
    assert_eq!(prices[0].date, "2026-01-02");
    assert_eq!(prices[0].price, 200_000_000);
}

/// MKT-090/091 — delete_asset_price removes the targeted record, leaves others intact,
/// and publishes AssetPriceUpdated.
#[tokio::test]
async fn delete_asset_price_removes_record_leaves_others_and_publishes_event() {
    let (svc, bus) = setup().await;
    let mut rx = bus.subscribe();
    let asset_id = create_asset(&svc).await;

    svc.record_asset_price(&asset_id, "2026-01-01", 100.0)
        .await
        .unwrap();
    rx.changed().await.unwrap();
    svc.record_asset_price(&asset_id, "2026-01-02", 110.0)
        .await
        .unwrap();
    rx.changed().await.unwrap();

    svc.delete_asset_price(&asset_id, "2026-01-01")
        .await
        .unwrap();

    // MKT-091 — event fired
    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), Event::AssetPriceUpdated);

    // DB state: only the 2026-01-02 record remains
    let prices = svc.get_asset_prices(&asset_id).await.unwrap();
    assert_eq!(prices.len(), 1);
    assert_eq!(prices[0].date, "2026-01-02");
    assert_eq!(prices[0].price, 110_000_000);
}

/// MKT-072 — get_asset_prices propagates AssetNotFound through the full stack
/// when the asset_id does not exist in the database.
#[tokio::test]
async fn get_asset_prices_returns_asset_not_found_for_unknown_asset() {
    use folioneer_lib::context::asset::AssetError;

    let (svc, _bus) = setup().await;

    let err = svc.get_asset_prices("nonexistent-id").await.unwrap_err();

    assert!(
        matches!(
            &err,
            AssetError::AssetNotFound { id }
                if id == "nonexistent-id"
        ),
        "expected AssetError::AssetNotFound, got: {err:?}"
    );
}
