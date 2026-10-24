//! Integration tests for the transfer (TRF spec): a holding moved from one account to
//! another at its cost, its fee in kind, its rate, and its correction and removal as one.
//!
//! Exercises the full stack through the public `folioneer_lib` API:
//! `TransferUseCase` → `AccountService` / `AssetService` / `CurrencyService` → real
//! in-memory SQLite. No mocks — per test-rules.md Tier 3 constraint.

use folioneer_lib::context::account::{
    AccountError, AccountService, Holding, SqliteAccountRepository, SqliteHoldingRepository,
    SqliteTransactionRepository, Transaction, TransactionType, WriteOffReason,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use folioneer_lib::use_cases::transfer::{
    TransferError, TransferFigures, TransferTask, TransferUseCase,
};
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
    transfer: TransferUseCase,
    recording: HoldingTransactionUseCase,
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
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
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    Ctx {
        transfer: TransferUseCase::new(
            account_service.clone(),
            asset_service.clone(),
            currency_service.clone(),
        ),
        recording: HoldingTransactionUseCase::new(account_service.clone(), asset_service.clone()),
        account_service,
        asset_service,
        currency_service,
    }
}

fn asset_dto(class: AssetClass) -> CreateAssetDTO {
    CreateAssetDTO {
        kind: None,
        name: "Bitcoin".to_string(),
        reference: "BTC".to_string(),
        isin: None,
        class,
        currency: "EUR".to_string(),
        risk_level: 5,
        category_id: SYSTEM_CATEGORY_ID.to_string(),
        exchange: None,
        interest_bearing: false,
    }
}

async fn account(ctx: &Ctx, name: &str, currency: &str) -> String {
    let account = ctx
        .account_service
        .create(name.to_string(), String::new(), currency.to_string(), false)
        .await
        .unwrap();
    ctx.recording
        .record_deposit(&account.id, "2024-01-01".to_string(), micro(10_000), None)
        .await
        .unwrap();
    account.id
}

/// Two accounts of `to_currency` and EUR; the EUR one holds 10 units bought at 50 on
/// 2024-01-15 (cost 500). Returns (sending, receiving, asset).
async fn seed(ctx: &Ctx, to_currency: &str) -> (String, String, String) {
    let asset = ctx
        .asset_service
        .create_asset(asset_dto(AssetClass::DigitalAsset))
        .await
        .unwrap();
    let from = account(ctx, "Exchange", "EUR").await;
    let to = account(ctx, "Wallet", to_currency).await;
    ctx.recording
        .buy_holding(
            &from,
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
    (from, to, asset.id)
}

fn figures(sent: i64, received: i64, rate: Option<i64>) -> TransferFigures {
    TransferFigures {
        date: "2024-03-01".to_string(),
        quantity_sent: sent,
        quantity_received: received,
        exchange_rate: rate,
        note: Some("to cold storage".to_string()),
    }
}

async fn holding(ctx: &Ctx, account_id: &str, asset_id: &str) -> Option<Holding> {
    ctx.account_service
        .get_holdings_for_account(account_id)
        .await
        .unwrap()
        .into_iter()
        .find(|h| h.asset_id == asset_id)
}

/// The account's cash: the quantity of its Cash Asset line.
async fn cash(ctx: &Ctx, account_id: &str) -> i64 {
    let mut total = 0;
    for h in ctx
        .account_service
        .get_holdings_for_account(account_id)
        .await
        .unwrap()
    {
        let asset = ctx
            .asset_service
            .get_asset_by_id(&h.asset_id)
            .await
            .unwrap();
        if asset.is_some_and(|a| a.class == AssetClass::Cash) {
            total += h.quantity;
        }
    }
    total
}

fn part(parts: &[Transaction], kind: TransactionType) -> &Transaction {
    parts
        .iter()
        .find(|t| t.transaction_type == kind)
        .unwrap_or_else(|| panic!("a {kind:?} part"))
}

// TRF-020, TRF-023 — the transfer moves the quantity and its cost, no cash and no realized
// result, on both sides.
#[tokio::test]
async fn trf_020_a_transfer_moves_quantity_and_cost_without_cash() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let cash_from = cash(&ctx, &from).await;
    let cash_to = cash(&ctx, &to).await;

    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();

    assert_eq!(parts.len(), 2, "no fee in kind when all is received");
    let out = part(&parts, TransactionType::TransferOut);
    let inn = part(&parts, TransactionType::TransferIn);
    assert_eq!((out.quantity, out.total_amount), (micro(4), micro(200)));
    assert_eq!((inn.quantity, inn.total_amount), (micro(4), micro(200)));
    assert_eq!(out.transfer_id, inn.transfer_id);
    assert!(out.transfer_id.is_some());
    assert_eq!(out.realized_pnl, None);

    let sending = holding(&ctx, &from, &asset).await.unwrap();
    assert_eq!(sending.quantity, micro(6));
    assert_eq!(
        sending.average_price,
        micro(50),
        "the sending average stays"
    );
    assert_eq!(sending.total_realized_pnl, 0, "TRF-023");
    let receiving = holding(&ctx, &to, &asset).await.unwrap();
    assert_eq!(receiving.quantity, micro(4));
    assert_eq!(receiving.average_price, micro(50));
    assert_eq!(receiving.total_realized_pnl, 0);
    assert_eq!(cash(&ctx, &from).await, cash_from, "no cash leaves");
    assert_eq!(cash(&ctx, &to).await, cash_to, "no cash arrives");
}

// TRF-024 — a transfer of everything held closes the sending position.
#[tokio::test]
async fn trf_024_a_transfer_of_everything_closes_the_sending_position() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    ctx.transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(10), micro(10), None),
        )
        .await
        .unwrap();
    let sending = holding(&ctx, &from, &asset).await.unwrap();
    assert_eq!(sending.quantity, 0);
    assert_eq!(sending.last_sold_date.as_deref(), Some("2024-03-01"));
    assert_eq!(
        holding(&ctx, &to, &asset).await.unwrap().quantity,
        micro(10)
    );
}

// TRF-021 — less received than sent: the difference leaves as a NetworkFee write-off of
// the transfer, at its cost, counted as a write-off.
#[tokio::test]
async fn trf_021_a_quantity_received_below_the_quantity_sent_is_a_fee_in_kind() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(4), 3_900_000, None),
        )
        .await
        .unwrap();

    let fee = part(&parts, TransactionType::WriteOff);
    let out = part(&parts, TransactionType::TransferOut);
    assert_eq!(fee.quantity, 100_000);
    assert_eq!(fee.write_off_reason, Some(WriteOffReason::NetworkFee));
    assert_eq!(fee.transfer_id, out.transfer_id);
    assert_eq!(
        fee.realized_pnl,
        Some(-micro(5)),
        "the fee leaves at its cost"
    );
    assert_eq!(out.quantity, 3_900_000);
    assert_eq!(out.total_amount, micro(195));

    let sending = holding(&ctx, &from, &asset).await.unwrap();
    assert_eq!(sending.quantity, micro(6));
    assert_eq!(
        sending.total_realized_pnl,
        -micro(5),
        "only the fee realizes"
    );
    let receiving = holding(&ctx, &to, &asset).await.unwrap();
    assert_eq!(receiving.quantity, 3_900_000);
    assert_eq!(receiving.average_price, micro(50));
}

// TRF-022 — between currencies the cost converts at the stored rate of the date, recorded
// on the transfer in; a rate given wins; no rate stored is refused.
#[tokio::test]
async fn trf_022_between_currencies_the_cost_converts_at_the_dates_rate() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "USD").await;

    let refused = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        TransferError::Account(AccountError::TransferRateMissing { ref from_currency, ref to_currency })
            if from_currency == "EUR" && to_currency == "USD"
    ));
    assert!(
        holding(&ctx, &to, &asset).await.is_none(),
        "nothing persists"
    );

    ctx.currency_service
        .record_currency_rate(
            "EUR".to_string(),
            "USD".to_string(),
            "2024-02-20".to_string(),
            1_100_000,
        )
        .await
        .unwrap();
    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    let inn = part(&parts, TransactionType::TransferIn);
    assert_eq!(
        inn.exchange_rate, 1_100_000,
        "the rate on or before the date"
    );
    assert_eq!(inn.total_amount, micro(220));
    assert_eq!(inn.unit_price, micro(55));
    assert_eq!(
        part(&parts, TransactionType::TransferOut).total_amount,
        micro(200)
    );
    assert_eq!(
        holding(&ctx, &to, &asset).await.unwrap().average_price,
        micro(55)
    );

    let given = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(1), micro(1), Some(1_200_000)),
        )
        .await
        .unwrap();
    assert_eq!(
        part(&given, TransactionType::TransferIn).total_amount,
        micro(60)
    );
}

// TRF-022 — between accounts of one currency a rate other than one is refused.
#[tokio::test]
async fn trf_022_one_currency_takes_no_rate() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let refused = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset,
            figures(micro(1), micro(1), Some(1_100_000)),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        TransferError::Account(AccountError::TransferRateNotOne)
    ));
}

// TRF-010, TRF-011 — what cannot be transferred is refused, each with its error.
#[tokio::test]
async fn trf_010_what_cannot_be_transferred_is_refused() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;

    let same = ctx
        .transfer
        .record_transfer(
            &from,
            &from,
            asset.clone(),
            figures(micro(1), micro(1), None),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        same,
        TransferError::Account(AccountError::TransferToSameAccount)
    ));
    let above = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(1), micro(2), None))
        .await
        .unwrap_err();
    assert!(matches!(
        above,
        TransferError::Account(AccountError::TransferReceivedAboveSent)
    ));
    let oversold = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(11), micro(11), None),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        oversold,
        TransferError::Account(AccountError::Oversell { .. })
    ));
    let unknown = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            "nope".to_string(),
            figures(micro(1), micro(1), None),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        unknown,
        TransferError::UseCase(TransferTask::AssetNotFound)
    ));

    let cash_asset = ctx
        .account_service
        .get_holdings_for_account(&from)
        .await
        .unwrap()
        .into_iter()
        .find(|h| h.asset_id != asset)
        .unwrap()
        .asset_id;
    let cash = ctx
        .transfer
        .record_transfer(&from, &to, cash_asset, figures(micro(1), micro(1), None))
        .await
        .unwrap_err();
    assert!(matches!(
        cash,
        TransferError::UseCase(TransferTask::TransferOfCash)
    ));
    assert!(
        holding(&ctx, &to, &asset).await.is_none(),
        "nothing persists"
    );
}

// TRF-030 — a correction rewrites every part at once, keeping their ids; the fee in kind
// comes and goes with the quantities.
#[tokio::test]
async fn trf_030_a_transfer_is_corrected_together() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    let transfer_id = parts[0].transfer_id.clone().unwrap();
    let out_id = part(&parts, TransactionType::TransferOut).id.clone();

    let corrected = ctx
        .transfer
        .correct_transfer(&transfer_id, figures(micro(6), 5_500_000, None))
        .await
        .unwrap();
    assert_eq!(corrected.len(), 3, "the fee in kind is added");
    assert_eq!(part(&corrected, TransactionType::TransferOut).id, out_id);
    assert_eq!(
        holding(&ctx, &from, &asset).await.unwrap().quantity,
        micro(4)
    );
    assert_eq!(
        holding(&ctx, &to, &asset).await.unwrap().quantity,
        5_500_000
    );

    let back = ctx
        .transfer
        .correct_transfer(&transfer_id, figures(micro(2), micro(2), None))
        .await
        .unwrap();
    assert_eq!(back.len(), 2, "the fee in kind is removed");
    let fees = ctx
        .account_service
        .get_transactions(&from, &asset)
        .await
        .unwrap();
    assert!(fees
        .iter()
        .all(|t| t.transaction_type != TransactionType::WriteOff));
    assert_eq!(
        holding(&ctx, &from, &asset).await.unwrap().quantity,
        micro(8)
    );
    assert_eq!(holding(&ctx, &to, &asset).await.unwrap().quantity, micro(2));

    let unknown = ctx
        .transfer
        .correct_transfer("nope", figures(micro(1), micro(1), None))
        .await
        .unwrap_err();
    assert!(matches!(
        unknown,
        TransferError::Account(AccountError::TransferNotFound)
    ));
}

// TRF-030, TRF-040 — the receiving account's later sale must stay covered: a correction or
// a removal that would leave it short is refused, the transfer left as it was.
#[tokio::test]
async fn trf_040_a_removal_leaving_the_receiving_account_short_is_refused() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    let transfer_id = parts[0].transfer_id.clone().unwrap();
    ctx.recording
        .sell_holding(
            &to,
            asset.clone(),
            "2024-04-01".to_string(),
            micro(3),
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let shrink = ctx
        .transfer
        .correct_transfer(&transfer_id, figures(micro(2), micro(2), None))
        .await
        .unwrap_err();
    assert!(matches!(
        shrink,
        TransferError::Account(AccountError::CascadingOversell)
    ));
    let removal = ctx
        .transfer
        .cancel_transfer(&transfer_id)
        .await
        .unwrap_err();
    assert!(matches!(
        removal,
        TransferError::Account(AccountError::CascadingOversell)
    ));
    assert_eq!(
        holding(&ctx, &from, &asset).await.unwrap().quantity,
        micro(6)
    );
    assert_eq!(holding(&ctx, &to, &asset).await.unwrap().quantity, micro(1));
}

// TRF-040, TRX-034 — removing any part removes the whole transfer and restores both
// accounts exactly.
#[tokio::test]
async fn trf_040_a_transfer_is_cancelled_together() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(4), 3_900_000, None),
        )
        .await
        .unwrap();
    let inn = part(&parts, TransactionType::TransferIn).id.clone();

    ctx.account_service
        .cancel_transaction(&to, &inn)
        .await
        .unwrap();

    let sending = holding(&ctx, &from, &asset).await.unwrap();
    assert_eq!(sending.quantity, micro(10));
    assert_eq!(
        sending.total_realized_pnl, 0,
        "the fee in kind went with it"
    );
    assert_eq!(
        ctx.account_service
            .get_transactions(&from, &asset)
            .await
            .unwrap()
            .len(),
        1,
        "only the purchase is left"
    );
    assert!(ctx
        .account_service
        .get_transactions(&to, &asset)
        .await
        .unwrap()
        .is_empty());

    let gone = ctx
        .transfer
        .cancel_transfer(parts[0].transfer_id.as_deref().unwrap())
        .await
        .unwrap_err();
    assert!(matches!(
        gone,
        TransferError::Account(AccountError::TransferNotFound)
    ));
}

// TRF-031 — a part is not corrected alone, a fee in kind neither.
#[tokio::test]
async fn trf_031_a_part_is_not_corrected_alone() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(4), 3_900_000, None),
        )
        .await
        .unwrap();
    let out = part(&parts, TransactionType::TransferOut);
    let refused = ctx
        .recording
        .correct_transaction(
            &from,
            &out.id,
            out.date.clone(),
            micro(1),
            micro(50),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(refused, AccountError::PartOfTransfer));
    let fee = part(&parts, TransactionType::WriteOff);
    let refused = ctx
        .account_service
        .correct_write_off(
            &from,
            &fee.id,
            fee.date.clone(),
            micro(1),
            WriteOffReason::Dust,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(refused, AccountError::PartOfTransfer));
}

// TRF-025 — a correction in the sending account that moves its average re-costs both
// sides, and a transfer onward follows in turn.
#[tokio::test]
async fn trf_025_the_receiving_cost_follows_the_sending_cost() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let third = account(&ctx, "Hardware", "EUR").await;
    ctx.transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    ctx.transfer
        .record_transfer(
            &to,
            &third,
            asset.clone(),
            TransferFigures {
                date: "2024-03-10".to_string(),
                ..figures(micro(2), micro(2), None)
            },
        )
        .await
        .unwrap();
    let purchase = ctx
        .account_service
        .get_transactions(&from, &asset)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.transaction_type == TransactionType::Purchase)
        .unwrap();

    ctx.recording
        .correct_transaction(
            &from,
            &purchase.id,
            purchase.date.clone(),
            micro(10),
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();

    let inn = ctx
        .account_service
        .get_transactions(&to, &asset)
        .await
        .unwrap()
        .into_iter()
        .find(|t| t.transaction_type == TransactionType::TransferIn)
        .unwrap();
    assert_eq!(inn.total_amount, micro(240), "4 units at the corrected 60");
    assert_eq!(
        holding(&ctx, &to, &asset).await.unwrap().average_price,
        micro(60)
    );
    assert_eq!(
        holding(&ctx, &third, &asset).await.unwrap().average_price,
        micro(60),
        "the transfer onward follows"
    );
}

// TRF-011 — between currencies with no rate given, a malformed date is named as such
// before any rate is looked up.
#[tokio::test]
async fn trf_011_a_malformed_date_is_named_before_the_rate() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "USD").await;
    let refused = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset,
            TransferFigures {
                date: "not-a-date".to_string(),
                ..figures(micro(1), micro(1), None)
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        TransferError::Account(AccountError::InvalidDate)
    ));
}

// TRF-012 — a backdated transfer that would leave a later sale above what is held is
// refused, and a sale backdated before a transfer out is refused likewise.
#[tokio::test]
async fn trf_012_a_backdated_transfer_keeps_later_sales_covered() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    ctx.recording
        .sell_holding(
            &from,
            asset.clone(),
            "2024-04-01".to_string(),
            micro(8),
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap();
    let refused = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        TransferError::Account(AccountError::CascadingOversell)
    ));
    assert!(
        holding(&ctx, &to, &asset).await.is_none(),
        "nothing persists"
    );

    ctx.transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(2), micro(2), None))
        .await
        .unwrap();
    let backdated = ctx
        .recording
        .sell_holding(
            &from,
            asset.clone(),
            "2024-02-01".to_string(),
            micro(1),
            micro(60),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .unwrap_err();
    assert!(matches!(backdated, AccountError::CascadingOversell));
}

// TRF-025 — a backdated opening balance in the sending account re-costs the transfer.
#[tokio::test]
async fn trf_025_a_backdated_opening_balance_recosts_the_transfer() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    ctx.transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    ctx.recording
        .open_holding(
            &from,
            asset.clone(),
            "2024-01-05".to_string(),
            micro(10),
            micro(1_500),
        )
        .await
        .unwrap();
    let receiving = holding(&ctx, &to, &asset).await.unwrap();
    assert_eq!(
        receiving.average_price,
        micro(100),
        "(500 + 1500) / 20 units"
    );
}

// TRF-030 — a correction naming no rate takes the stored rate of its new date.
#[tokio::test]
async fn trf_030_a_correction_without_a_rate_reads_the_rate_of_its_date() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "USD").await;
    for (date, rate) in [("2024-02-20", 1_100_000), ("2024-03-05", 1_200_000)] {
        ctx.currency_service
            .record_currency_rate("EUR".to_string(), "USD".to_string(), date.to_string(), rate)
            .await
            .unwrap();
    }
    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    let transfer_id = parts[0].transfer_id.clone().unwrap();
    let corrected = ctx
        .transfer
        .correct_transfer(
            &transfer_id,
            TransferFigures {
                date: "2024-03-10".to_string(),
                ..figures(micro(4), micro(4), None)
            },
        )
        .await
        .unwrap();
    let inn = part(&corrected, TransactionType::TransferIn);
    assert_eq!(inn.exchange_rate, 1_200_000);
    assert_eq!(inn.total_amount, micro(240));
}

// TRF-041, ACC-006 — deleting the sending account leaves the transfer in at its cost; it is
// no longer corrected, and removing it removes what is left of the transfer.
#[tokio::test]
async fn trf_041_a_removed_account_leaves_the_other_side() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "EUR").await;
    let parts = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(micro(4), micro(4), None))
        .await
        .unwrap();
    let transfer_id = parts[0].transfer_id.clone().unwrap();
    ctx.account_service.delete(&from).await.unwrap();

    let receiving = holding(&ctx, &to, &asset).await.unwrap();
    assert_eq!(
        (receiving.quantity, receiving.average_price),
        (micro(4), micro(50))
    );
    let refused = ctx
        .transfer
        .correct_transfer(&transfer_id, figures(micro(1), micro(1), None))
        .await
        .unwrap_err();
    assert!(matches!(
        refused,
        TransferError::Account(AccountError::TransferNotFound)
    ));
    let inn = part(&parts, TransactionType::TransferIn).id.clone();
    ctx.account_service
        .cancel_transaction(&to, &inn)
        .await
        .unwrap();
    assert!(ctx
        .account_service
        .get_transactions(&to, &asset)
        .await
        .unwrap()
        .is_empty());
}

// TRF-010, TRF-011 — the other refusals: an unknown account, an archived asset, nothing held
// on the date, a quantity not positive, a rate not positive.
#[tokio::test]
async fn trf_010_the_other_refusals() {
    let ctx = build_ctx().await;
    let (from, to, asset) = seed(&ctx, "USD").await;

    let unknown = ctx
        .transfer
        .record_transfer(
            &from,
            "nope",
            asset.clone(),
            figures(micro(1), micro(1), Some(micro(1))),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        unknown,
        TransferError::Account(AccountError::AccountNotFound { .. })
    ));
    let before = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            TransferFigures {
                date: "2024-01-10".to_string(),
                ..figures(micro(1), micro(1), Some(micro(1)))
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        before,
        TransferError::Account(AccountError::ClosedPosition)
    ));
    let zero = ctx
        .transfer
        .record_transfer(&from, &to, asset.clone(), figures(0, 0, Some(micro(1))))
        .await
        .unwrap_err();
    assert!(matches!(
        zero,
        TransferError::Account(AccountError::QuantityNotPositive)
    ));
    let no_rate = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            asset.clone(),
            figures(micro(1), micro(1), Some(0)),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        no_rate,
        TransferError::Account(AccountError::ExchangeRateNotPositive)
    ));

    let other = ctx
        .asset_service
        .create_asset(CreateAssetDTO {
            reference: "ETH".to_string(),
            name: "Ether".to_string(),
            ..asset_dto(AssetClass::DigitalAsset)
        })
        .await
        .unwrap();
    ctx.asset_service.archive_asset(&other.id).await.unwrap();
    let archived = ctx
        .transfer
        .record_transfer(
            &from,
            &to,
            other.id,
            figures(micro(1), micro(1), Some(micro(1))),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        archived,
        TransferError::UseCase(TransferTask::ArchivedAsset)
    ));
}
