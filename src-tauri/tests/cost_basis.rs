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

/// Records a transfer of `quantity` units of `asset` on `date`, at `rate` when given.
async fn transfer(
    ctx: &Ctx,
    from: &str,
    to: &str,
    asset: &str,
    date: &str,
    quantity: i64,
    rate: Option<i64>,
) {
    ctx.transfer
        .record_transfer(
            from,
            to,
            asset.into(),
            TransferFigures {
                date: date.into(),
                quantity_sent: units(quantity),
                quantity_received: units(quantity),
                exchange_rate: rate,
                note: None,
            },
        )
        .await
        .expect("transfer");
}

/// The lots of `account`'s holding of `asset`: the date, the units brought, their cost, the
/// units left and whether they came at no cost (CBM-034).
async fn lots(ctx: &Ctx, account: &str, asset: &str) -> Vec<(String, String, i64, String, bool)> {
    ctx.accounts
        .holding_lots(account, asset)
        .await
        .expect("lots")
        .lots
        .into_iter()
        .map(|l| (l.acquired_on, l.quantity, l.cost, l.remaining, l.free))
        .collect()
}

fn lot(
    date: &str,
    quantity: &str,
    cost: i64,
    left: &str,
    free: bool,
) -> (String, String, i64, String, bool) {
    (date.into(), quantity.into(), cost, left.into(), free)
}

// CBM-024 (a)(d) / TRF-020 — a transfer out of a FIFO account moves the cost of the oldest
// lots, and the transfer in brings those lots, free shares named, each at the date it was
// acquired on and placed among the receiving account's own: a later sale takes the oldest,
// a sale before the transfer is unchanged.
#[tokio::test]
async fn cbm_024_a_transfer_carries_its_lots() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let from = account(&ctx, "Depot", Some("DE"), None).await;
    let to = account(&ctx, "Depot 2", Some("DE"), None).await;
    two_lots(&ctx, &from, &acme).await;
    ctx.recording
        .record_free_shares(&from, acme.clone(), "2024-03-15".into(), units(2), None)
        .await
        .expect("free shares");
    buy(&ctx, &to, &acme, "2024-02-15", 5, 300).await;
    let before = sell(&ctx, &to, &acme, "2024-03-20", 2, 400).await;
    assert_eq!(before.realized_pnl, Some(micro(800 - 600)));
    transfer(&ctx, &from, &to, &acme, "2024-04-01", 22, None).await;
    let unchanged = transactions(&ctx, &to)
        .await
        .into_iter()
        .find(|t| t.id == before.id)
        .expect("sale");
    assert_eq!(unchanged.realized_pnl, Some(micro(800 - 600)));
    let transfer_in = transactions(&ctx, &to)
        .await
        .into_iter()
        .find(|t| t.transaction_type == TransactionType::TransferIn)
        .expect("transfer in");
    assert_eq!(transfer_in.total_amount, micro(3_000));
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [
            lot("2024-02-01", "10", micro(1_000), "10", false),
            lot("2024-02-15", "5", micro(1_500), "3", false),
            lot("2024-03-01", "10", micro(2_000), "10", false),
            lot("2024-03-15", "2", 0, "2", true),
        ]
    );
    // The 12 sold take the 10 carried from 2024-02-01, then 2 of the account's own.
    let sale = sell(&ctx, &to, &acme, "2024-05-01", 12, 200).await;
    assert_eq!(sale.realized_pnl, Some(micro(2_400 - 1_000 - 600)));
}

// CBM-024 (a) / TRF-021 — a fee in kind takes the oldest lots first, the transfer out the
// next: the transfer in carries what the transfer out took.
#[tokio::test]
async fn cbm_024_a_fee_in_kind_leaves_before_the_lots_carried() {
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
                quantity_sent: units(11),
                quantity_received: units(10),
                exchange_rate: None,
                note: None,
            },
        )
        .await
        .expect("transfer");
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [
            lot("2024-02-01", "9", micro(900), "9", false),
            lot("2024-03-01", "1", micro(200), "1", false),
        ]
    );
}

// CBM-024 (b)(c) — between currencies each lot's cost is converted at the transfer's rate;
// from an account under the weighted average the transfer in is one lot dated the transfer.
#[tokio::test]
async fn cbm_024_converted_lots_and_one_lot_from_the_weighted_average() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let fifo = account(&ctx, "Depot", Some("DE"), None).await;
    let average = account(&ctx, "Compte", Some("FR"), None).await;
    let dollars = ctx
        .accounts
        .create_classified(
            "Depot US".into(),
            String::new(),
            "USD".into(),
            false,
            Some("DE".into()),
            None,
        )
        .await
        .expect("account")
        .id;
    let to = account(&ctx, "Depot 2", Some("DE"), None).await;
    two_lots(&ctx, &fifo, &acme).await;
    two_lots(&ctx, &average, &acme).await;
    transfer(
        &ctx,
        &fifo,
        &dollars,
        &acme,
        "2024-04-01",
        15,
        Some(1_500_000),
    )
    .await;
    assert_eq!(
        lots(&ctx, &dollars, &acme).await,
        [
            lot("2024-02-01", "10", micro(1_500), "10", false),
            lot("2024-03-01", "5", micro(1_500), "5", false),
        ]
    );
    transfer(&ctx, &average, &to, &acme, "2024-04-01", 15, None).await;
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [lot("2024-04-01", "15", micro(2_250), "15", false)]
    );
}

// CBM-024 (e) / TRF-025 — a backdated purchase in the sending account changes the lots a
// transfer out takes: the transfer in's lots follow, though the cost moved stays the same.
#[tokio::test]
async fn cbm_024_the_carried_lots_follow_the_sending_account() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let from = account(&ctx, "Depot", Some("DE"), None).await;
    let to = account(&ctx, "Depot 2", Some("DE"), None).await;
    buy(&ctx, &from, &acme, "2024-02-01", 10, 100).await;
    transfer(&ctx, &from, &to, &acme, "2024-04-01", 5, None).await;
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [lot("2024-02-01", "5", micro(500), "5", false)]
    );
    buy(&ctx, &from, &acme, "2024-01-15", 10, 100).await;
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [lot("2024-01-15", "5", micro(500), "5", false)]
    );
}

// CBM-024 — a transfer recorded before its sides carried lots brings one lot dated the
// transfer, until the next write in its sending account re-costs it.
#[tokio::test]
async fn cbm_024_a_transfer_without_lots_carries_them_from_the_next_write() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let from = account(&ctx, "Depot", Some("DE"), None).await;
    let to = account(&ctx, "Depot 2", Some("DE"), None).await;
    two_lots(&ctx, &from, &acme).await;
    transfer(&ctx, &from, &to, &acme, "2024-04-01", 15, None).await;
    sqlx::query("UPDATE transactions SET carried_lots = NULL")
        .execute(&ctx.pool)
        .await
        .expect("lots removed");
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [lot("2024-04-01", "15", micro(2_000), "15", false)]
    );
    buy(&ctx, &from, &acme, "2024-05-01", 1, 100).await;
    assert_eq!(
        lots(&ctx, &to, &acme).await,
        [
            lot("2024-02-01", "10", micro(1_000), "10", false),
            lot("2024-03-01", "5", micro(1_000), "5", false),
        ]
    );
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
    // CBM-024 (e) — the transfer in now carries the lots the transfer out takes.
    let carried: Vec<(String, i64, i64)> = transactions(&ctx, &to)
        .await
        .into_iter()
        .find(|t| t.transaction_type == TransactionType::TransferIn)
        .expect("transfer in")
        .carried_lots
        .into_iter()
        .map(|l| (l.acquired_on, l.quantity, l.cost))
        .collect();
    assert_eq!(
        carried,
        [
            ("2024-02-01".to_string(), units(5), micro(500)),
            ("2024-03-01".to_string(), units(10), micro(2_000)),
        ]
    );

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

// CBM-014 — the method a country implies comes with the country it comes from: the
// wrapper's, else the account's, else the portfolio's.
#[test]
fn cbm_014_the_implied_method_names_its_country() {
    use folioneer_lib::context::account::implied_cost_basis;
    let read = |country, wrapper| {
        let implied = implied_cost_basis(country, wrapper);
        (implied.method, implied.country)
    };
    assert_eq!(
        read(Some("FR"), Some("de_depot")),
        (CostBasisMethod::Fifo, "DE".to_string())
    );
    assert_eq!(
        read(Some("ES"), Some("brokerage")),
        (CostBasisMethod::Fifo, "ES".to_string())
    );
    assert_eq!(
        read(None, None),
        (CostBasisMethod::WeightedAverage, "FR".to_string())
    );
    assert_eq!(
        read(Some(""), None),
        (CostBasisMethod::WeightedAverage, "FR".to_string())
    );
    assert_eq!(
        read(Some("US"), None),
        (CostBasisMethod::WeightedAverage, "US".to_string())
    );
}

// CBM-011A — a method chosen at creation is kept, even against the country's.
#[tokio::test]
async fn cbm_011a_a_chosen_method_is_kept() {
    use folioneer_lib::shared::infrastructure::uow::SqliteTransactionManager;
    use folioneer_lib::use_cases::account_creation::AccountCreationUseCase;
    let ctx = ctx().await;
    let creation = AccountCreationUseCase::new(
        ctx.accounts.clone(),
        ctx.assets.clone(),
        Arc::new(SqliteTransactionManager::new(ctx.pool.clone())),
    );
    let chosen = creation
        .create_with_method(
            "Depot".into(),
            String::new(),
            "EUR".into(),
            false,
            Some("DE".into()),
            None,
            Some(CostBasisMethod::WeightedAverage),
            None,
            None,
        )
        .await
        .expect("created");
    assert_eq!(
        method_of(&ctx, &chosen.id).await,
        CostBasisMethod::WeightedAverage
    );
    let implied = creation
        .create_with_method(
            "Depot 2".into(),
            String::new(),
            "EUR".into(),
            false,
            Some("DE".into()),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("created");
    assert_eq!(method_of(&ctx, &implied.id).await, CostBasisMethod::Fifo);
}

// CBM-034 — a holding's lots, oldest first: a lot taken whole shows what it brought and
// cost with nothing left, free shares are named; none under the weighted average.
#[tokio::test]
async fn cbm_034_a_holdings_lots_are_read() {
    let ctx = ctx().await;
    let acme = asset(&ctx).await;
    let fifo = account(&ctx, "Depot", Some("DE"), None).await;
    let average = account(&ctx, "Compte", Some("FR"), None).await;
    for account in [&fifo, &average] {
        two_lots(&ctx, account, &acme).await;
        ctx.recording
            .record_free_shares(account, acme.clone(), "2024-03-15".into(), units(2), None)
            .await
            .expect("free shares");
        sell(&ctx, account, &acme, "2024-04-01", 12, 200).await;
    }
    let lots = ctx.accounts.holding_lots(&fifo, &acme).await.expect("lots");
    assert_eq!(lots.method, CostBasisMethod::Fifo);
    let seen: Vec<(&str, &str, i64, &str, bool)> = lots
        .lots
        .iter()
        .map(|l| {
            (
                l.acquired_on.as_str(),
                l.quantity.as_str(),
                l.cost,
                l.remaining.as_str(),
                l.free,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("2024-02-01", "10", micro(1_000), "0", false),
            ("2024-03-01", "10", micro(2_000), "8", false),
            ("2024-03-15", "2", 0, "2", true),
        ]
    );
    // CBM-034A — the cash line and an asset never held read no lot.
    let cash = ctx
        .accounts
        .holding_lots(&fifo, "system-cash-eur")
        .await
        .expect("lots");
    assert!(cash.lots.is_empty());
    let never = ctx
        .accounts
        .holding_lots(&fifo, "never-held")
        .await
        .expect("lots");
    assert!(never.lots.is_empty());
    let none = ctx
        .accounts
        .holding_lots(&average, &acme)
        .await
        .expect("lots");
    assert_eq!(none.method, CostBasisMethod::WeightedAverage);
    assert!(none.lots.is_empty());
    assert!(matches!(
        ctx.accounts.holding_lots("nope", &acme).await,
        Err(AccountError::AccountNotFound { .. })
    ));
}
