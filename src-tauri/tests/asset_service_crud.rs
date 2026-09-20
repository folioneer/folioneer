//! Integration tests for AssetService category CRUD, update_asset error paths,
//! and event bus emission on write operations.
//!
//! Uses real SQLite repos against an in-memory DB (B27).

use folioneer_lib::context::asset::exchange::Exchange;
use folioneer_lib::context::asset::{
    AssetClass, AssetError, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, UpdateAssetDTO, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::core::{Event, SideEffectEventBus};
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use std::time::Duration;

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

async fn setup() -> (AssetService, Arc<SideEffectEventBus>) {
    let pool = make_pool().await;
    let bus = Arc::new(SideEffectEventBus::new());
    let svc = AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool)),
    )
    .with_event_bus(Arc::clone(&bus));
    (svc, bus)
}

fn base_create_dto(name: &str) -> CreateAssetDTO {
    CreateAssetDTO {
        name: name.to_string(),
        reference: "REF".to_string(),
        isin: None,
        class: AssetClass::Stocks,
        currency: "USD".to_string(),
        risk_level: 1,
        category_id: SYSTEM_CATEGORY_ID.to_string(),
        exchange: None,
        interest_bearing: false,
    }
}

fn xpar() -> Exchange {
    folioneer_lib::context::asset::exchange::lookup("XPAR")
        .expect("XPAR must be in the curated set")
}

// ── Category CRUD ─────────────────────────────────────────────────────────────

/// create_category() persists and get_category_by_id() retrieves it.
#[tokio::test]
async fn test_create_category_and_retrieve_by_id() {
    let (svc, _bus) = setup().await;

    let cat = svc.create_category("Bonds").await.expect("seed category");
    assert_eq!(cat.name, "Bonds");

    let found = svc.get_category_by_id(&cat.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(
        found.expect("category should exist after create").name,
        "Bonds"
    );
}

/// update_category() persists the new label.
#[tokio::test]
async fn test_update_category_changes_label() {
    let (svc, _bus) = setup().await;

    let cat = svc.create_category("OldLabel").await.unwrap();
    let updated = svc.update_category(&cat.id, "NewLabel").await.unwrap();
    assert_eq!(updated.name, "NewLabel");

    let found = svc
        .get_category_by_id(&cat.id)
        .await
        .expect("DB read should succeed")
        .expect("category should exist after update");
    assert_eq!(found.name, "NewLabel");
}

/// delete_category() removes it from get_all_categories().
#[tokio::test]
async fn test_delete_category_removes_it() {
    let (svc, _bus) = setup().await;

    let cat = svc
        .create_category("ToRemove")
        .await
        .expect("seed category");
    svc.delete_category(&cat.id).await.unwrap();

    let all = svc.get_all_categories().await.unwrap();
    assert!(!all.iter().any(|c| c.id == cat.id));
}

// ── update_asset error paths ──────────────────────────────────────────────────

/// update_asset() returns Archived error when the asset is archived.
#[tokio::test]
async fn test_update_asset_rejected_when_archived() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(base_create_dto("ArchiveMe"))
        .await
        .expect("seed asset");
    svc.archive_asset(&asset.id).await.unwrap();

    let err = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id,
            name: "NewName".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .unwrap_err();

    use folioneer_lib::context::asset::AssetError;
    assert!(
        matches!(&err, AssetError::Archived),
        "expected Archived, got: {err:?}"
    );
}

/// update_asset() returns CategoryNotFound when the given category_id does not exist.
#[tokio::test]
async fn test_update_asset_rejected_when_category_not_found() {
    let (svc, _bus) = setup().await;

    let asset = svc.create_asset(base_create_dto("CatCheck")).await.unwrap();

    let err = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id,
            name: "CatCheck".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: "nonexistent-category-id".to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .unwrap_err();

    use folioneer_lib::context::asset::AssetError;
    assert!(
        matches!(&err, AssetError::CategoryNotFound { .. }),
        "expected AssetError::CategoryNotFound, got: {err:?}"
    );
}

// ── Event bus emission ────────────────────────────────────────────────────────

/// create_asset() fires AssetUpdated on the event bus.
#[tokio::test]
async fn test_create_asset_publishes_asset_updated_event() {
    let (svc, bus) = setup().await;
    let mut rx = bus.subscribe();

    svc.create_asset(base_create_dto("EventAsset"))
        .await
        .unwrap();

    tokio::time::timeout(Duration::from_millis(200), rx.changed())
        .await
        .expect("event not received within 200ms")
        .expect("watch sender dropped before event fired");
    assert_eq!(*rx.borrow(), Event::AssetUpdated);
}

/// archive_asset() fires AssetUpdated on the event bus.
#[tokio::test]
async fn test_archive_asset_publishes_asset_updated_event() {
    let (svc, bus) = setup().await;

    let asset = svc
        .create_asset(base_create_dto("ToArchive"))
        .await
        .unwrap();
    let mut rx = bus.subscribe();

    svc.archive_asset(&asset.id).await.unwrap();

    tokio::time::timeout(Duration::from_millis(200), rx.changed())
        .await
        .expect("event not received within 200ms")
        .expect("watch sender dropped before event fired");
    assert_eq!(*rx.borrow(), Event::AssetUpdated);
}

/// unarchive_asset() fires AssetUpdated on the event bus.
#[tokio::test]
async fn test_unarchive_asset_publishes_asset_updated_event() {
    let (svc, bus) = setup().await;

    let asset = svc
        .create_asset(base_create_dto("ToUnarchive"))
        .await
        .unwrap();
    svc.archive_asset(&asset.id).await.unwrap();
    let mut rx = bus.subscribe();

    svc.unarchive_asset(&asset.id).await.unwrap();

    tokio::time::timeout(Duration::from_millis(200), rx.changed())
        .await
        .expect("event not received within 200ms")
        .expect("watch sender dropped before event fired");
    assert_eq!(*rx.borrow(), Event::AssetUpdated);
}

/// delete_asset() fires AssetUpdated on the event bus.
#[tokio::test]
async fn test_delete_asset_publishes_asset_updated_event() {
    let (svc, bus) = setup().await;

    let asset = svc.create_asset(base_create_dto("ToDelete")).await.unwrap();
    let mut rx = bus.subscribe();

    svc.delete_asset(&asset.id).await.unwrap();

    tokio::time::timeout(Duration::from_millis(200), rx.changed())
        .await
        .expect("event not received within 200ms")
        .expect("watch sender dropped before event fired");
    assert_eq!(*rx.borrow(), Event::AssetUpdated);
}

/// create_category() fires CategoryUpdated on the event bus.
#[tokio::test]
async fn test_create_category_publishes_category_updated_event() {
    let (svc, bus) = setup().await;
    let mut rx = bus.subscribe();

    svc.create_category("EventCat").await.unwrap();

    tokio::time::timeout(Duration::from_millis(200), rx.changed())
        .await
        .expect("event not received within 200ms")
        .expect("watch sender dropped before event fired");
    assert_eq!(*rx.borrow(), Event::CategoryUpdated);
}

// ── Exchange field (AST-001, AST-022) ────────────────────────────────────────

/// add_asset with exchange = Some(canonical) persists and round-trips (AST-022 set).
#[tokio::test]
async fn test_create_asset_with_canonical_exchange_persists_and_round_trips() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(CreateAssetDTO {
            name: "Air Liquide".to_string(),
            reference: "AI".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "EUR".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(xpar()),
            interest_bearing: false,
        })
        .await
        .expect("create_asset with canonical exchange should succeed");

    let exchange = asset
        .exchange
        .expect("exchange should be Some after create");
    assert_eq!(exchange.code, "XPAR");

    let all = svc
        .get_all_assets()
        .await
        .expect("get_all_assets should succeed");
    let found = all
        .iter()
        .find(|a| a.id == asset.id)
        .expect("asset should appear in get_all_assets");
    let found_exchange = found
        .exchange
        .as_ref()
        .expect("exchange should survive round-trip via get_all_assets");
    assert_eq!(found_exchange.code, "XPAR");
}

/// add_asset with exchange = Some(non-curated) returns InvalidExchange (AST-001).
#[tokio::test]
async fn test_create_asset_with_non_curated_exchange_returns_invalid_exchange() {
    let (svc, _bus) = setup().await;

    let err = svc
        .create_asset(CreateAssetDTO {
            name: "Some Asset".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(Exchange {
                code: "BOGUS".to_string(),
                label: "Bogus Exchange".to_string(),
            }),
            interest_bearing: false,
        })
        .await
        .unwrap_err();

    assert!(
        matches!(&err, AssetError::InvalidExchange { exchange_code } if exchange_code == "BOGUS"),
        "expected InvalidExchange {{ exchange_code: \"BOGUS\" }}, got: {err:?}"
    );
}

/// add_asset with exchange = None persists and round-trips with None (AST-022 absent).
#[tokio::test]
async fn test_create_asset_with_no_exchange_persists_and_round_trips() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(CreateAssetDTO {
            name: "AAPL".to_string(),
            reference: "AAPL".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 4,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("create_asset with no exchange should succeed");

    assert!(
        asset.exchange.is_none(),
        "exchange should be None when not provided"
    );

    let all = svc
        .get_all_assets()
        .await
        .expect("get_all_assets should succeed");
    let found = all
        .iter()
        .find(|a| a.id == asset.id)
        .expect("asset should appear in get_all_assets");
    assert!(
        found.exchange.is_none(),
        "exchange should remain None after round-trip"
    );
}

/// update_asset can change exchange from None → Some(canonical) (AST-022 set via update).
#[tokio::test]
async fn test_update_asset_sets_exchange_from_none() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(base_create_dto("SetExchange"))
        .await
        .expect("seed asset");
    assert!(asset.exchange.is_none());

    let updated = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id.clone(),
            name: "SetExchange".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(xpar()),
            interest_bearing: false,
        })
        .await
        .expect("update_asset to set exchange should succeed");

    let exchange = updated
        .exchange
        .expect("exchange should be Some after update");
    assert_eq!(exchange.code, "XPAR");
}

/// update_asset can change exchange from Some(X) → Some(Y) (AST-022 change).
#[tokio::test]
async fn test_update_asset_changes_exchange_to_different_canonical_value() {
    let (svc, _bus) = setup().await;

    let xnas = folioneer_lib::context::asset::exchange::lookup("XNAS")
        .expect("XNAS must be in curated set");

    let asset = svc
        .create_asset(CreateAssetDTO {
            name: "ChangeExchange".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(xnas),
            interest_bearing: false,
        })
        .await
        .expect("seed asset with XNAS");

    let updated = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id.clone(),
            name: "ChangeExchange".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(xpar()),
            interest_bearing: false,
        })
        .await
        .expect("update_asset to change exchange should succeed");

    let exchange = updated
        .exchange
        .expect("exchange should be Some after change");
    assert_eq!(exchange.code, "XPAR");
}

/// update_asset can clear exchange from Some → None (AST-022 clear).
#[tokio::test]
async fn test_update_asset_clears_exchange() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(CreateAssetDTO {
            name: "ClearExchange".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(xpar()),
            interest_bearing: false,
        })
        .await
        .expect("seed asset with XPAR");

    let updated = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id.clone(),
            name: "ClearExchange".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("update_asset to clear exchange should succeed");

    assert!(
        updated.exchange.is_none(),
        "exchange should be None after clearing"
    );
}

/// update_asset rejects non-curated exchange with InvalidExchange (AST-001).
#[tokio::test]
async fn test_update_asset_with_non_curated_exchange_returns_invalid_exchange() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(base_create_dto("InvalidExchangeUpdate"))
        .await
        .expect("seed asset");

    let err = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id,
            name: "InvalidExchangeUpdate".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: Some(Exchange {
                code: "BOGUS".to_string(),
                label: "Bogus Exchange".to_string(),
            }),
            interest_bearing: false,
        })
        .await
        .unwrap_err();

    assert!(
        matches!(&err, AssetError::InvalidExchange { exchange_code } if exchange_code == "BOGUS"),
        "expected InvalidExchange {{ exchange_code: \"BOGUS\" }}, got: {err:?}"
    );
}

/// interest_bearing round-trips through create → read → update → read (AST-024).
#[tokio::test]
async fn test_interest_bearing_round_trips_through_create_and_update() {
    let (svc, _bus) = setup().await;

    let asset = svc
        .create_asset(CreateAssetDTO {
            interest_bearing: true,
            ..base_create_dto("EuroFund")
        })
        .await
        .expect("create interest-bearing asset");
    assert!(asset.interest_bearing);

    let loaded = svc
        .get_asset_by_id(&asset.id)
        .await
        .expect("read asset")
        .expect("asset must exist");
    assert!(loaded.interest_bearing);

    let updated = svc
        .update_asset(UpdateAssetDTO {
            asset_id: asset.id.clone(),
            name: "EuroFund".to_string(),
            reference: "REF".to_string(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".to_string(),
            risk_level: 1,
            category_id: SYSTEM_CATEGORY_ID.to_string(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("update_asset to clear interest_bearing should succeed");
    assert!(!updated.interest_bearing);

    let reloaded = svc
        .get_asset_by_id(&asset.id)
        .await
        .expect("re-read asset")
        .expect("asset must exist");
    assert!(!reloaded.interest_bearing);
}
