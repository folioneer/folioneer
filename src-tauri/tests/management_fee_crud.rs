//! Integration tests for the `record_management_fee` use-case (FEE spec).
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `HoldingTransactionUseCase` → `AccountService` / `AssetService` →
//! `AccountPerformanceUseCase` over real in-memory SQLite. No mocks — per
//! test-rules.md Tier 3 constraint. Mirrors `free_shares_crud.rs`, the
//! quantity-adding sibling of the quantity-reducing fee deduction.

use folioneer_lib::context::account::{
    Account, AccountService, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository, UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::account_performance::AccountPerformanceUseCase;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

async fn enable_management_fees(svc: &AccountService, account: &Account) -> Account {
    svc.update(
        account.id.clone(),
        account.name.clone(),
        String::new(),
        account.currency.clone(),
        account.update_frequency,
        true,
    )
    .await
    .unwrap()
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
    let bus = Arc::new(SideEffectEventBus::new());

    let account_service = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );

    let use_case = HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone());

    Ctx {
        use_case,
        account_service,
        asset_service,
    }
}

fn stocks_asset_dto(name: &str, reference: &str, currency: &str) -> CreateAssetDTO {
    CreateAssetDTO {
        kind: None,
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

// -------------------------------------------------------------------------
// FEE-024 — recording a deduction creates no AssetPrice row
// -------------------------------------------------------------------------

/// FEE-024 — recording a management-fee deduction must not create or modify any
/// AssetPrice record for the charged asset (the deduction is not a price
/// observation) — mirroring FSD-024.
#[tokio::test]
async fn record_management_fee_does_not_create_asset_price_row() {
    // FEE-024 — negative-space test: no AssetPrice write on a fee deduction
    let ctx = build_ctx().await;
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
    let account = enable_management_fees(&ctx.account_service, &account).await;

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

    // Record a 1% management fee (removes floor(10 × 1%) = 0.1 units).
    ctx.use_case
        .record_management_fee(
            &account.id,
            asset.id.clone(),
            "2024-06-15".to_string(),
            micro(1),
            None,
        )
        .await
        .unwrap();

    let latest_price = ctx.asset_service.get_latest_price(&asset.id).await.unwrap();
    assert!(
        latest_price.is_none(),
        "recording a management-fee deduction must not create any AssetPrice row (FEE-024)"
    );
}

// -------------------------------------------------------------------------
// FEE-071 — performance treatment: a fee is not a flow nor a dividend
// -------------------------------------------------------------------------

/// FEE-071 — a management-fee deduction is neither an external cash flow nor
/// dividend income: the performance bridge must record cash_flow from the
/// deposit only, asset_flow = 0 (it is not an in-kind contribution like free
/// shares), and dividends = 0. Its drag surfaces through the position's reduced
/// value, the inverse of FSD-070. Mirrors `record_free_shares_performance_neutrality`.
#[tokio::test]
async fn record_management_fee_performance_neutrality() {
    // FEE-071 — fee excluded from cash flows AND dividend totals
    let pool = make_pool().await;

    let account_service = Arc::new(AccountService::new(
        Box::new(SqliteAccountRepository::new(pool.clone())),
        Box::new(SqliteHoldingRepository::new(pool.clone())),
        Box::new(SqliteTransactionRepository::new(pool.clone())),
    ));
    let asset_service = Arc::new(AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone())),
        Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
        Box::new(SqliteAssetPriceRepository::new(pool.clone())),
    ));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    let perf_use_case = AccountPerformanceUseCase::new(
        account_service.clone(),
        asset_service.clone(),
        currency_service,
    );
    let uc = HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone());

    let asset = asset_service
        .create_asset(stocks_asset_dto("AAPL", "AAPL", "USD"))
        .await
        .unwrap();
    let account = account_service
        .create(
            "Portfolio".to_string(),
            String::new(),
            "USD".to_string(),
            UpdateFrequency::Automatic,
            false,
        )
        .await
        .unwrap();
    let account = enable_management_fees(&account_service, &account).await;

    uc.record_deposit(&account.id, "2024-01-01".to_string(), micro(1_000), None)
        .await
        .unwrap();
    uc.buy_holding(
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
    uc.record_management_fee(
        &account.id,
        asset.id.clone(),
        "2024-06-01".to_string(),
        micro(1), // 1% of the holding
        None,
    )
    .await
    .unwrap();

    let resp = perf_use_case
        .get_account_performance(&account.id, None)
        .await
        .unwrap();

    let year_2024 = resp
        .yearly
        .iter()
        .find(|p| p.year == 2024)
        .expect("a 2024 year row must exist in the performance response");

    // FEE-071 — the fee must NOT register as a cash flow: cash_flow reflects only
    // the deposit (deposits − withdrawals = 1000).
    assert_eq!(
        year_2024.cash_flow,
        micro(1_000),
        "management fee must not appear in cash_flow — only the deposit counts (FEE-071)"
    );
    // FEE-071 — the fee is not an in-kind contribution (unlike free shares).
    assert_eq!(
        year_2024.asset_flow, 0,
        "management fee must not appear in asset_flow (FEE-071)"
    );
    // FEE-071 — the fee is not dividend income.
    assert_eq!(
        year_2024.dividends, 0,
        "management fee must not appear in dividend totals (FEE-071)"
    );
}

// -------------------------------------------------------------------------
// FEE-028 / FEE-029 — one-off fee entered by the resulting quantity (TODO-018)
// -------------------------------------------------------------------------

/// An account with management fees enabled holding `quantity` micro-units of one asset,
/// bought on 2024-01-15.
async fn account_holding(ctx: &Ctx, quantity: i64) -> (Account, String) {
    let asset = ctx
        .asset_service
        .create_asset(stocks_asset_dto("WORLD", "WORLD", "USD"))
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
    let account = enable_management_fees(&ctx.account_service, &account).await;
    ctx.use_case
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(100_000), None)
        .await
        .unwrap();
    ctx.use_case
        .buy_holding(
            &account.id,
            asset.id.clone(),
            "2024-01-15".to_string(),
            quantity,
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
    (account, asset.id)
}

/// FEE-028 — the removal is the held quantity minus the resulting quantity, exactly: a
/// one-micro-unit removal no percentage could express survives untouched.
#[tokio::test]
async fn fee_028_resulting_quantity_records_the_exact_removal() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    let fee = ctx
        .use_case
        .record_management_fee_entry(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            None,
            Some(24_500_000),
            None,
        )
        .await
        .expect("FEE-028: a resulting quantity below the held one must be recorded");
    assert_eq!(
        fee.quantity, 500_000,
        "25.000000 → 24.500000 removes 0.500000"
    );

    let one_micro = ctx
        .use_case
        .record_management_fee_entry(
            &account.id,
            asset_id.clone(),
            "2024-06-16".to_string(),
            None,
            Some(24_499_999),
            None,
        )
        .await
        .expect("FEE-028: a one-micro-unit removal must be recorded");
    assert_eq!(
        one_micro.quantity, 1,
        "no rounding: 24.500000 → 24.499999 removes 0.000001"
    );

    let holding = ctx
        .account_service
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap()
        .expect("the holding stays open");
    assert_eq!(holding.quantity, 24_499_999);
}

/// FEE-028 — a resulting quantity of zero removes the whole holding as of that date.
#[tokio::test]
async fn fee_028_resulting_quantity_zero_removes_the_whole_holding() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    let fee = ctx
        .use_case
        .record_management_fee_entry(
            &account.id,
            asset_id,
            "2024-06-15".to_string(),
            None,
            Some(0),
            None,
        )
        .await
        .expect("FEE-028: zero is a valid resulting quantity");
    assert_eq!(fee.quantity, micro(25));
}

/// FEE-021/028 — a resulting quantity below zero, or not strictly below the quantity held
/// as of the date, is rejected, and the rejection carries the held quantity.
#[tokio::test]
async fn fee_028_resulting_quantity_out_of_bounds_is_rejected() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    for resulting in [micro(25), micro(26)] {
        let error = ctx
            .use_case
            .record_management_fee_entry(
                &account.id,
                asset_id.clone(),
                "2024-06-15".to_string(),
                None,
                Some(resulting),
                None,
            )
            .await
            .expect_err("FEE-028: a resulting quantity not below the held one is rejected");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({ "code": "ResultingQuantityNotBelowHeld", "held_quantity": micro(25) })
        );
    }

    let error = ctx
        .use_case
        .record_management_fee_entry(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            None,
            Some(-1),
            None,
        )
        .await
        .expect_err("FEE-021: a negative resulting quantity is rejected");
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({ "code": "ResultingQuantityNegative" })
    );
}

/// FEE-021 — exactly one of the percentage and the resulting quantity is provided.
#[tokio::test]
async fn fee_021_both_or_neither_amount_is_rejected() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    for (percent, resulting) in [(None, None), (Some(micro(1)), Some(micro(24)))] {
        let error = ctx
            .use_case
            .record_management_fee_entry(
                &account.id,
                asset_id.clone(),
                "2024-06-15".to_string(),
                percent,
                resulting,
                None,
            )
            .await
            .expect_err("FEE-021: both or neither amount is rejected");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({ "code": "ManagementFeeAmountInvalid" })
        );
    }
}

/// FEE-029 — the preview reports the quantity held as of the date, the removal and the
/// percentage it represents, from the same rules as the recording, and writes nothing.
#[tokio::test]
async fn fee_029_preview_reports_held_removed_and_percentage_without_writing() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    let preview = ctx
        .use_case
        .preview_management_fee(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            24_500_000,
        )
        .await
        .expect("FEE-029: a valid resulting quantity previews");
    assert_eq!(preview.held_quantity, micro(25));
    assert_eq!(preview.removed_quantity, 500_000);
    assert_eq!(preview.percent_micros, 2_000_000, "0.5 of 25 is 2 %");

    // FEE-021 — the date bounds are checked before the holding is replayed.
    for (date, code) in [
        ("2999-01-01", "DateInFuture"),
        ("not-a-date", "InvalidDate"),
    ] {
        let error = ctx
            .use_case
            .preview_management_fee(&account.id, asset_id.clone(), date.to_string(), 0)
            .await
            .expect_err("FEE-021: the preview rejects a date outside the bounds");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({ "code": code })
        );
    }

    // Before the purchase nothing is held, so nothing can result from a fee.
    let error = ctx
        .use_case
        .preview_management_fee(&account.id, asset_id.clone(), "2024-01-10".to_string(), 0)
        .await
        .expect_err("FEE-029: the preview rejects like the recording");
    assert_eq!(
        serde_json::to_value(&error).unwrap(),
        serde_json::json!({ "code": "ResultingQuantityNotBelowHeld", "held_quantity": 0 })
    );

    let holding = ctx
        .account_service
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap()
        .expect("the holding exists");
    assert_eq!(
        holding.quantity,
        micro(25),
        "FEE-029: a preview writes nothing"
    );
}

fn code<E: serde::Serialize>(error: &E) -> String {
    serde_json::to_value(error).unwrap()["code"]
        .as_str()
        .expect("a code")
        .to_string()
}

/// FEE-012 — a fee on an asset or an account that does not exist is refused with the
/// code that says which.
#[tokio::test]
async fn fee_012_an_unknown_asset_or_account_is_refused() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    let unknown_asset = ctx
        .use_case
        .record_management_fee(
            &account.id,
            "no-such-asset".to_string(),
            "2024-06-15".to_string(),
            micro(1),
            None,
        )
        .await
        .expect_err("an unknown asset");
    let unknown_account = ctx
        .use_case
        .record_management_fee(
            "no-such-account",
            asset_id,
            "2024-06-15".to_string(),
            micro(1),
            None,
        )
        .await
        .expect_err("an unknown account");

    assert_eq!(code(&unknown_asset), "AssetNotFound");
    assert_eq!(code(&unknown_account), "AccountNotFound");
}

/// FEE-021 — a date in the future, before the lower bound, or that is no date is refused
/// on recording, and the lower bound is refused by the preview too.
#[tokio::test]
async fn fee_021_a_date_outside_its_bounds_is_refused() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;

    for (date, expected) in [
        ("2999-01-01", "DateInFuture"),
        ("1899-12-31", "DateTooOld"),
        ("not-a-date", "InvalidDate"),
    ] {
        let error = ctx
            .use_case
            .record_management_fee(
                &account.id,
                asset_id.clone(),
                date.to_string(),
                micro(1),
                None,
            )
            .await
            .expect_err("a date outside the bounds");
        assert_eq!(code(&error), expected, "{date}");
    }
    let previewed = ctx
        .use_case
        .preview_management_fee(&account.id, asset_id, "1899-12-31".to_string(), 0)
        .await
        .expect_err("a date before the lower bound");
    assert_eq!(code(&previewed), "DateTooOld");
}

/// FEE-022 (a) — a percentage whose removal floors to nothing is refused, and removes
/// nothing.
#[tokio::test]
async fn fee_022_a_percentage_that_removes_nothing_is_refused() {
    let ctx = build_ctx().await;
    // 25 micro-units: 1 % of them floors to zero.
    let (account, asset_id) = account_holding(&ctx, 25).await;

    let error = ctx
        .use_case
        .record_management_fee(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            micro(1),
            None,
        )
        .await
        .expect_err("a removal of zero");

    assert_eq!(code(&error), "QuantityNotPositive");
    let holding = ctx
        .account_service
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap()
        .expect("the holding exists");
    assert_eq!(holding.quantity, 25);
}

/// FEE-029 — the preview refuses what the recording refuses: an unknown account or
/// asset, an asset that is not held, the cash line, and an account whose parameter is off.
#[tokio::test]
async fn fee_029_the_preview_refuses_what_the_recording_refuses() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;
    let not_held = ctx
        .asset_service
        .create_asset(stocks_asset_dto("OTHER", "OTHER", "USD"))
        .await
        .unwrap();
    let preview = |account_id: String, asset: String| {
        let use_case = &ctx.use_case;
        async move {
            let refused = use_case
                .preview_management_fee(&account_id, asset, "2024-06-15".to_string(), 0)
                .await
                .expect_err("refused");
            code(&refused)
        }
    };

    assert_eq!(
        preview("no-such-account".to_string(), asset_id.clone()).await,
        "AccountNotFound"
    );
    assert_eq!(
        preview(account.id.clone(), "no-such-asset".to_string()).await,
        "AssetNotFound"
    );
    assert_eq!(
        preview(account.id.clone(), not_held.id.clone()).await,
        "AssetNotHeld"
    );
    assert_eq!(
        preview(account.id.clone(), "system-cash-usd".to_string()).await,
        "ManagementFeeOnCashAsset"
    );

    ctx.account_service
        .update(
            account.id.clone(),
            account.name.clone(),
            String::new(),
            account.currency.clone(),
            account.update_frequency,
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        preview(account.id.clone(), asset_id).await,
        "ManagementFeesDisabled"
    );
}

/// FEE-063 — a recorded deduction is edited and deleted through the replay: its date and
/// note change and its quantity stays; an edit that would starve a later sale is refused;
/// deleting it gives the shares back.
#[tokio::test]
async fn fee_063_a_deduction_is_edited_and_deleted_through_the_replay() {
    let ctx = build_ctx().await;
    let (account, asset_id) = account_holding(&ctx, micro(25)).await;
    // 10 % of 25 removes 2.5; the sale then takes the 22.5 that are left.
    let fee = ctx
        .use_case
        .record_management_fee(
            &account.id,
            asset_id.clone(),
            "2024-06-15".to_string(),
            micro(10),
            None,
        )
        .await
        .unwrap();
    ctx.use_case
        .sell_holding(
            &account.id,
            asset_id.clone(),
            "2024-09-01".to_string(),
            22_500_000,
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let edited = ctx
        .account_service
        .correct_transaction(
            &account.id,
            &fee.id,
            "2024-07-01".to_string(),
            fee.quantity,
            0,
            micro(1),
            0,
            None,
            Some("TER".to_string()),
        )
        .await
        .expect("a date and a note are edited");
    assert_eq!(edited.date, "2024-07-01");
    assert_eq!(edited.note.as_deref(), Some("TER"));
    assert_eq!(edited.quantity, 2_500_000);
    assert_eq!(edited.asset_id, asset_id);

    let refused = ctx
        .account_service
        .correct_transaction(
            &account.id,
            &fee.id,
            "2024-07-01".to_string(),
            micro(3),
            0,
            micro(1),
            0,
            None,
            None,
        )
        .await
        .expect_err("removing more starves the sale");
    assert_eq!(code(&refused), "CascadingOversell");

    ctx.account_service
        .cancel_transaction(&account.id, &fee.id)
        .await
        .expect("a deletion is always replay-safe");
    let holding = ctx
        .account_service
        .get_holding_by_account_asset(&account.id, &asset_id)
        .await
        .unwrap()
        .expect("the holding exists");
    assert_eq!(holding.quantity, 2_500_000, "the removed shares are back");
}

/// FEE-075 — an account that existed before the parameter keeps its fees: what is checked
/// is the value the migration gives a row written without the column. A new account
/// starts with them off.
#[tokio::test]
async fn fee_075_an_account_from_before_the_parameter_keeps_its_fees() {
    let ctx = build_ctx().await;
    let pool = make_pool().await;
    sqlx::query(
        "INSERT INTO accounts (id, name, update_frequency, currency) VALUES ('old', 'Old', 'ManualMonth', 'EUR')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (enabled,): (bool,) =
        sqlx::query_as("SELECT management_fees_enabled FROM accounts WHERE id = 'old'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(enabled);

    let created = ctx
        .account_service
        .create(
            "New".to_string(),
            String::new(),
            "EUR".to_string(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    assert!(!created.management_fees_enabled);
}
