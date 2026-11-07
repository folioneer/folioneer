//! Integration tests for the cost basis method (CBM spec): the method an account starts
//! with, FIFO lots taken the oldest first, and a change of method recomputing the history.
//!
//! Exercises the full stack through the public `folioneer_lib` API — `AccountService`,
//! `HoldingTransactionUseCase`, `TransferUseCase` — over real in-memory SQLite. No mocks
//! (test-rules.md Tier 3). The portfolio is invented data.

use folioneer_lib::context::account::{
    AccountError, AccountService, CostBasisMethod, SqliteAccountRepository,
    SqliteHoldingRepository, SqliteTransactionRepository, Transaction, TransactionType,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::core::SideEffectEventBus;
use folioneer_lib::shared::domain::quantity::from_surface;
use folioneer_lib::use_cases::holding_transaction::HoldingTransactionUseCase;
use folioneer_lib::use_cases::transfer::{TransferFigures, TransferUseCase};
use std::sync::Arc;

fn micro(v: i64) -> i64 {
    v * 1_000_000
}

fn units(v: i64) -> i64 {
    from_surface(micro(v))
}

struct Ctx {
    pool: sqlx::Pool<sqlx::Sqlite>,
    accounts: Arc<AccountService>,
    assets: Arc<AssetService>,
    recording: HoldingTransactionUseCase,
    transfer: TransferUseCase,
}

async fn ctx() -> Ctx {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("test pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let bus = Arc::new(SideEffectEventBus::new());
    let accounts = Arc::new(
        AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let assets = Arc::new(
        AssetService::new(
            Box::new(SqliteAssetRepository::new(pool.clone())),
            Box::new(SqliteAssetCategoryRepository::new(pool.clone())),
            Box::new(SqliteAssetPriceRepository::new(pool.clone())),
        )
        .with_event_bus(Arc::clone(&bus)),
    );
    let currencies = Arc::new(CurrencyService::new(
        Box::new(SqliteCurrencyPairRepository::new(pool.clone())),
        Box::new(SqliteCurrencyRateRepository::new(pool.clone())),
    ));
    Ctx {
        recording: HoldingTransactionUseCase::new(accounts.clone(), assets.clone()),
        transfer: TransferUseCase::new(accounts.clone(), assets.clone(), currencies),
        pool,
        accounts,
        assets,
    }
}

async fn asset(ctx: &Ctx) -> String {
    ctx.assets
        .create_asset(CreateAssetDTO {
            kind: None,
            name: "Acme".into(),
            reference: "ACM".into(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "EUR".into(),
            risk_level: 3,
            category_id: SYSTEM_CATEGORY_ID.into(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .expect("asset")
        .id
}

/// An account of `country` and `wrapper`, with cash to buy with.
async fn account(ctx: &Ctx, name: &str, country: Option<&str>, wrapper: Option<&str>) -> String {
    let account = ctx
        .accounts
        .create_classified(
            name.into(),
            String::new(),
            "EUR".into(),
            false,
            country.map(str::to_string),
            wrapper.map(str::to_string),
        )
        .await
        .expect("account");
    ctx.recording
        .record_deposit(&account.id, "2024-01-01".into(), micro(100_000), None)
        .await
        .expect("deposit");
    account.id
}

async fn buy(ctx: &Ctx, account: &str, asset: &str, date: &str, quantity: i64, unit_price: i64) {
    ctx.recording
        .buy_holding(
            account,
            asset.into(),
            date.into(),
            units(quantity),
            micro(unit_price),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .expect("purchase");
}

async fn sell(
    ctx: &Ctx,
    account: &str,
    asset: &str,
    date: &str,
    quantity: i64,
    unit_price: i64,
) -> Transaction {
    ctx.recording
        .sell_holding(
            account,
            asset.into(),
            date.into(),
            units(quantity),
            micro(unit_price),
            micro(1),
            0,
            None,
            None,
        )
        .await
        .expect("sale")
}

async fn method_of(ctx: &Ctx, account: &str) -> CostBasisMethod {
    ctx.accounts
        .get_by_id(account)
        .await
        .expect("read")
        .expect("account")
        .cost_basis_method
}

async fn transactions(ctx: &Ctx, account: &str) -> Vec<Transaction> {
    ctx.accounts
        .get_all_transactions_for_account(account)
        .await
        .expect("transactions")
}

/// Two lots: 10 units at 100 on 2024-02-01, 10 at 200 on 2024-03-01.
async fn two_lots(ctx: &Ctx, account: &str, asset: &str) {
    buy(ctx, account, asset, "2024-02-01", 10, 100).await;
    buy(ctx, account, asset, "2024-03-01", 10, 200).await;
}

// CBM-011 — a new account takes the method its country implies: its wrapper's country
// first, then its own; the weighted average where the country implies none.
#[tokio::test]
async fn cbm_011_a_new_account_takes_its_countrys_method() {
    let ctx = ctx().await;
    let cases = [
        (Some("DE"), None, CostBasisMethod::Fifo),
        (Some("ES"), None, CostBasisMethod::Fifo),
        (Some("IT"), None, CostBasisMethod::WeightedAverage),
        (None, None, CostBasisMethod::WeightedAverage),
        (Some("US"), None, CostBasisMethod::WeightedAverage),
        (None, Some("de_depot"), CostBasisMethod::Fifo),
        (Some("DE"), Some("fr_pea"), CostBasisMethod::WeightedAverage),
        (Some("PL"), Some("brokerage"), CostBasisMethod::Fifo),
    ];
    for (n, (country, wrapper, expected)) in cases.into_iter().enumerate() {
        let id = account(&ctx, &format!("Account {n}"), country, wrapper).await;
        assert_eq!(
            method_of(&ctx, &id).await,
            expected,
            "{country:?} {wrapper:?}"
        );
    }
}

// CBM-022 — under FIFO a sale across two lots costs the oldest whole and the next in part;
// the weighted average costs every unit at the average.
#[tokio::test]
async fn cbm_022_a_sale_takes_the_oldest_lots_first() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let fifo = account(&ctx, "Depot", Some("DE"), None).await;
    let average = account(&ctx, "Compte", Some("FR"), None).await;
    for account in [&fifo, &average] {
        two_lots(&ctx, account, &acme).await;
    }
    let fifo_sale = sell(&ctx, &fifo, &acme, "2024-04-01", 15, 200).await;
    let average_sale = sell(&ctx, &average, &acme, "2024-04-01", 15, 200).await;
    // 3000 of proceeds: FIFO costs 10 × 100 + 5 × 200; the average 15 × 150.
    assert_eq!(fifo_sale.realized_pnl, Some(micro(1_000)));
    assert_eq!(average_sale.realized_pnl, Some(micro(750)));
    let held = ctx
        .accounts
        .holding_snapshot_as_of(&fifo, &acme, "2024-12-31")
        .await
        .expect("snapshot");
    assert_eq!((held.quantity, held.average_price), (units(5), micro(200)));
}

// CBM-020 / CBM-025 — free shares are a lot at no cost, and a split rescales every lot.
#[tokio::test]
async fn cbm_020_free_shares_and_a_split_keep_their_lots() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let fifo = account(&ctx, "Depot", Some("DE"), None).await;
    buy(&ctx, &fifo, &acme, "2024-02-01", 10, 100).await;
    ctx.recording
        .record_free_shares(&fifo, acme.clone(), "2024-03-01".into(), units(10), None)
        .await
        .expect("free shares");
    ctx.recording
        .record_split(&fifo, acme.clone(), "2024-04-01".into(), micro(2), None)
        .await
        .expect("split");
    // 40 units: the first 20 cost 1000, the 20 free ones nothing.
    let sale = sell(&ctx, &fifo, &acme, "2024-05-01", 30, 100).await;
    assert_eq!(sale.realized_pnl, Some(micro(2_000)));
}

// CBM-024 / CBM-022 — a transfer out of a FIFO account moves the cost of the oldest lots,
// and the transfer in brings it as one lot.
#[tokio::test]
async fn cbm_024_a_transfer_moves_the_cost_of_the_oldest_lots() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let from = account(&ctx, "Depot", Some("DE"), None).await;
    let to = account(&ctx, "Depot 2", Some("DE"), None).await;
    two_lots(&ctx, &from, &acme).await;
    ctx.transfer
        .record_transfer(
            &from,
            &to,
            acme.clone(),
            TransferFigures {
                date: "2024-04-01".into(),
                quantity_sent: units(15),
                quantity_received: units(15),
                exchange_rate: None,
                note: None,
            },
        )
        .await
        .expect("transfer");
    let transfer_in = transactions(&ctx, &to)
        .await
        .into_iter()
        .find(|t| t.transaction_type == TransactionType::TransferIn)
        .expect("transfer in");
    assert_eq!(transfer_in.total_amount, micro(2_000));
}

// CBM-030 — changing the method recomputes the account's sales and holdings, and re-costs
// the transfer in its transfer out feeds; CBM-031 — the same method changes nothing.
#[tokio::test]
async fn cbm_030_a_change_of_method_recomputes_the_history() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let from = account(&ctx, "Compte", Some("FR"), None).await;
    let to = account(&ctx, "Compte 2", Some("FR"), None).await;
    two_lots(&ctx, &from, &acme).await;
    let sale = sell(&ctx, &from, &acme, "2024-04-01", 5, 200).await;
    assert_eq!(sale.realized_pnl, Some(micro(250)));
    ctx.transfer
        .record_transfer(
            &from,
            &to,
            acme.clone(),
            TransferFigures {
                date: "2024-05-01".into(),
                quantity_sent: units(15),
                quantity_received: units(15),
                exchange_rate: None,
                note: None,
            },
        )
        .await
        .expect("transfer");
    let cost_in = |transactions: Vec<Transaction>| {
        transactions
            .into_iter()
            .find(|t| t.transaction_type == TransactionType::TransferIn)
            .map(|t| t.total_amount)
    };
    // The weighted average moves the 15 left at 150.
    assert_eq!(cost_in(transactions(&ctx, &to).await), Some(micro(2_250)));

    let changed = ctx
        .accounts
        .change_cost_basis_method(&from, CostBasisMethod::Fifo)
        .await
        .expect("changed");
    assert_eq!(changed.cost_basis_method, CostBasisMethod::Fifo);
    assert_eq!(method_of(&ctx, &from).await, CostBasisMethod::Fifo);
    let resold = transactions(&ctx, &from)
        .await
        .into_iter()
        .find(|t| t.id == sale.id)
        .expect("sale");
    // FIFO: the 5 sold cost 5 × 100, then the transfer moves 5 × 100 + 10 × 200.
    assert_eq!(resold.realized_pnl, Some(micro(500)));
    assert_eq!(cost_in(transactions(&ctx, &to).await), Some(micro(2_500)));

    let again = ctx
        .accounts
        .change_cost_basis_method(&from, CostBasisMethod::Fifo)
        .await
        .expect("unchanged");
    assert_eq!(again.cost_basis_method, CostBasisMethod::Fifo);
    let unknown = ctx
        .accounts
        .change_cost_basis_method("nope", CostBasisMethod::Fifo)
        .await;
    assert!(matches!(unknown, Err(AccountError::AccountNotFound { .. })));
}

// CBM-030 — an account update through its form keeps its method.
#[tokio::test]
async fn cbm_030_an_account_update_keeps_its_method() {
    let ctx = ctx().await;
    let id = account(&ctx, "Depot", Some("DE"), None).await;
    ctx.accounts
        .update_classified(
            id.clone(),
            "Depot renamed".into(),
            String::new(),
            "EUR".into(),
            false,
            Some("DE".into()),
            None,
        )
        .await
        .expect("updated");
    ctx.accounts
        .update(
            id.clone(),
            "Depot again".into(),
            String::new(),
            "EUR".into(),
            false,
        )
        .await
        .expect("updated");
    assert_eq!(method_of(&ctx, &id).await, CostBasisMethod::Fifo);
}

// CBM-033 — an account change of method applied from another device replays the account,
// whatever order its transactions arrived in.
#[tokio::test]
async fn cbm_033_a_synced_change_of_method_replays_the_account() {
    use folioneer_lib::shared::domain::{LogicalTimestamp, Origin, Rank};
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let id = account(&ctx, "Compte", Some("FR"), None).await;
    two_lots(&ctx, &id, &acme).await;
    let sale = sell(&ctx, &id, &acme, "2024-04-01", 5, 200).await;
    assert_eq!(sale.realized_pnl, Some(micro(250)));

    let mut changed = ctx.accounts.get_by_id(&id).await.unwrap().unwrap();
    changed.cost_basis_method = CostBasisMethod::Fifo;
    let content = serde_json::to_string(&changed).unwrap();
    let mut conn = ctx.pool.acquire().await.unwrap();
    ctx.accounts
        .apply_account(
            &mut conn,
            &content,
            Rank {
                origin: Origin::User,
                logical_timestamp: LogicalTimestamp::new(1_000),
                device_id: "laptop".into(),
            },
        )
        .await
        .expect("applied");
    drop(conn);

    let resold = transactions(&ctx, &id)
        .await
        .into_iter()
        .find(|t| t.id == sale.id)
        .expect("sale");
    assert_eq!(resold.realized_pnl, Some(micro(500)));
    let holding = ctx
        .accounts
        .get_holding_by_account_asset(&id, &acme)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(holding.average_price, micro(2_500) / 15);
}
