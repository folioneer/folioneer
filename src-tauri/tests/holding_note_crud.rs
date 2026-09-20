//! Integration tests for the holding-note commands (HNO spec).
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `AccountService::{upsert_holding_note, delete_holding_note}` over the real
//! `SqliteHoldingNoteRepository` and in-memory SQLite. No mocks — per
//! test_convention.md Tier 3 constraint. Mirrors `free_shares_crud.rs` /
//! `management_fee_crud.rs` in structure.

use folioneer_lib::context::account::{
    AccountError, AccountService, SqliteAccountRepository, SqliteHoldingNoteRepository,
    SqliteHoldingRepository, SqliteTransactionRepository, ThresholdDirection, UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::core::cash::system_cash_asset_id;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
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

    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_holding_note_repo(Box::new(SqliteHoldingNoteRepository::new(pool.clone()))),
    );
    let asset_service = Arc::new(AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool.clone())),
    ));

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

/// Seeds an account holding 10 units of a Stocks asset (one deposit + one
/// purchase), the transaction history HNO-011 requires for the pair.
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
// HNO-010/020/031 — upsert creates the note
// -------------------------------------------------------------------------

/// HNO-020 — upsert_holding_note creates the note for the (account, asset)
/// pair, persisting text, threshold and direction through the real SQLite
/// repository (HNO-010 one note per pair, HNO-031 nominal threshold).
#[tokio::test]
async fn upsert_holding_note_creates_note_with_alarm() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    let note = ctx
        .account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "  Watch earnings  ".to_string(),
            Some(micro(150)),
            Some(ThresholdDirection::Below),
        )
        .await
        .unwrap();
    assert_eq!(
        note.text, "Watch earnings",
        "text must be stored trimmed (HNO-011)"
    );

    let notes = ctx
        .account_service
        .get_holding_notes(&account_id)
        .await
        .unwrap();
    assert_eq!(notes.len(), 1, "exactly one note per pair (HNO-010)");
    assert_eq!(notes[0].account_id, account_id);
    assert_eq!(notes[0].asset_id, asset_id);
    assert_eq!(notes[0].text, "Watch earnings");
    assert_eq!(notes[0].threshold_price, Some(micro(150)));
    assert_eq!(
        notes[0].threshold_direction,
        Some(ThresholdDirection::Below)
    );
}

// -------------------------------------------------------------------------
// HNO-020 — upsert replaces the existing note
// -------------------------------------------------------------------------

/// HNO-020 — a second upsert for the same pair fully replaces the stored note
/// (including dropping the alarm), never adds a second row.
#[tokio::test]
async fn upsert_holding_note_replaces_existing_note() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    ctx.account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "Watch earnings".to_string(),
            Some(micro(150)),
            Some(ThresholdDirection::Below),
        )
        .await
        .unwrap();
    ctx.account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "Position reviewed".to_string(),
            None,
            None,
        )
        .await
        .unwrap();

    let notes = ctx
        .account_service
        .get_holding_notes(&account_id)
        .await
        .unwrap();
    assert_eq!(
        notes.len(),
        1,
        "replace must not add a second row (HNO-020)"
    );
    assert_eq!(notes[0].text, "Position reviewed");
    assert_eq!(
        notes[0].threshold_price, None,
        "the replace must drop the alarm (HNO-020)"
    );
    assert_eq!(notes[0].threshold_direction, None);
}

// -------------------------------------------------------------------------
// HNO-021 — delete
// -------------------------------------------------------------------------

/// HNO-021 — delete_holding_note removes the pair's note.
#[tokio::test]
async fn delete_holding_note_removes_note() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    ctx.account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "Note".to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    ctx.account_service
        .delete_holding_note(&account_id, &asset_id)
        .await
        .unwrap();

    let notes = ctx
        .account_service
        .get_holding_notes(&account_id)
        .await
        .unwrap();
    assert!(notes.is_empty(), "the note must be gone (HNO-021)");
}

/// HNO-021 — deleting a non-existent note is a no-op success.
#[tokio::test]
async fn delete_holding_note_without_note_is_ok() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    ctx.account_service
        .delete_holding_note(&account_id, &asset_id)
        .await
        .expect("deleting a non-existent note must succeed (HNO-021)");
}

// -------------------------------------------------------------------------
// HNO-011 — validation rejections through the real stack
// -------------------------------------------------------------------------

/// HNO-011 — blank text (whitespace-only) and text above 500 characters are
/// rejected by the domain factory through the service.
#[tokio::test]
async fn upsert_holding_note_invalid_text_rejected() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    let err = ctx
        .account_service
        .upsert_holding_note(&account_id, asset_id.clone(), "   ".to_string(), None, None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::NoteTextEmpty),
        "expected NoteTextEmpty, got: {err:?}"
    );

    let err = ctx
        .account_service
        .upsert_holding_note(&account_id, asset_id.clone(), "x".repeat(501), None, None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::NoteTextTooLong),
        "expected NoteTextTooLong, got: {err:?}"
    );
}

/// HNO-011 — a threshold without a direction (and vice versa) is rejected.
#[tokio::test]
async fn upsert_holding_note_incomplete_alarm_rejected() {
    let ctx = build_ctx().await;
    let (account_id, asset_id) = seed_held_position(&ctx).await;

    let err = ctx
        .account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "Watch earnings".to_string(),
            Some(micro(150)),
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::ThresholdIncomplete),
        "expected ThresholdIncomplete, got: {err:?}"
    );

    let err = ctx
        .account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "Watch earnings".to_string(),
            None,
            Some(ThresholdDirection::Above),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::ThresholdIncomplete),
        "expected ThresholdIncomplete, got: {err:?}"
    );
}

/// HNO-011 — the cash line cannot carry a note.
#[tokio::test]
async fn upsert_holding_note_on_cash_line_rejected() {
    let ctx = build_ctx().await;
    let (account_id, _) = seed_held_position(&ctx).await;

    let err = ctx
        .account_service
        .upsert_holding_note(
            &account_id,
            system_cash_asset_id("USD"),
            "Cash note".to_string(),
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::NoteOnCashAsset),
        "expected NoteOnCashAsset, got: {err:?}"
    );
}

/// HNO-011 — a pair without any transaction history cannot carry a note.
#[tokio::test]
async fn upsert_holding_note_on_unheld_asset_rejected() {
    let ctx = build_ctx().await;
    let (account_id, _) = seed_held_position(&ctx).await;
    let unheld_asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("MSFT", "MSFT", "USD"))
        .await
        .unwrap();

    let err = ctx
        .account_service
        .upsert_holding_note(
            &account_id,
            unheld_asset.id.clone(),
            "Never bought".to_string(),
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(err, AccountError::NoteOnUnheldAsset),
        "expected NoteOnUnheldAsset, got: {err:?}"
    );
}
