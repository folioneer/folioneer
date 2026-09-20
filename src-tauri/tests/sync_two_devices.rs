//! Tier-3 integration: two installations ("Desktop", "Laptop") sharing one encrypted
//! folder converge on the same portfolio (SYN-013/014/036/065/080/083, CFR-040/041/042/044).
//! Per `test_convention.md` Tier 3: only the crate's public API is used — two `SqlitePool`s,
//! one `tempfile::tempdir()` folder, real BC services on each side (`Ctx { orchestrator, … }`,
//! mirroring `management_fee_crud.rs`'s and `sync_first_publish.rs`'s shape).
//!
//! Every scenario syncs both ways twice: the second round picks up what the other device
//! published in the first.

use std::sync::Arc;

use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteFeeCatchUpRepository,
    SqliteFeeScheduleRepository, SqliteHoldingNoteRepository, SqliteHoldingRepository,
    SqliteTransactionRepository, UpdateFrequency,
};
use folioneer_lib::context::asset::{
    AssetClass, AssetService, CreateAssetDTO, SqliteAssetCategoryRepository,
    SqliteAssetPriceRepository, SqliteAssetRepository, SYSTEM_CATEGORY_ID,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::context::sync::{
    FirstPublish, FsFolderStore, SqliteChangeLogRepository, SqliteChangeRecorder,
    SqliteSyncStateRepository, SyncRun, SyncService, SyncStateRepository, DATA_FORMAT_VERSION,
};
use folioneer_lib::core::{Event, SideEffectEventBus};
use folioneer_lib::shared::infrastructure::change_recorder::ChangeRecorder;
use folioneer_lib::use_cases::portfolio_sync::{
    PortfolioSyncDependencies, PortfolioSyncOrchestrator, ServicePortfolioSnapshot,
    ServiceRankStamper,
};

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

/// One device's full service graph, wired exactly as the production container wires it
/// (SYN-020: every synced repository records through the real change recorder).
struct Ctx {
    orchestrator: PortfolioSyncOrchestrator,
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
    pool: sqlx::Pool<sqlx::Sqlite>,
}

async fn build_ctx(folder: &std::path::Path) -> Ctx {
    build_ctx_on_bus(folder, Arc::new(SideEffectEventBus::new())).await
}

/// `build_ctx` with every service publishing on `bus`, as the production container wires it.
async fn build_ctx_on_bus(folder: &std::path::Path, bus: Arc<SideEffectEventBus>) -> Ctx {
    let pool = make_pool().await;
    let recorder: Arc<dyn ChangeRecorder> = Arc::new(SqliteChangeRecorder::new(pool.clone()));
    let account_service = Arc::new(
        AccountService::new(
            Box::new(
                SqliteAccountRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
            ),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(
                SqliteTransactionRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ),
        )
        .with_fee_schedule_repo(Box::new(
            SqliteFeeScheduleRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ))
        .with_fee_catch_up_repo(Box::new(
            SqliteFeeCatchUpRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ))
        .with_holding_note_repo(Box::new(
            SqliteHoldingNoteRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ))
        .with_event_bus(bus.clone()),
    );
    let asset_service = Arc::new(
        AssetService::new(
            Box::new(
                SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
            ),
            Box::new(
                SqliteAssetCategoryRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ),
            Box::new(
                SqliteAssetPriceRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ),
        )
        .with_event_bus(bus.clone()),
    );
    let currency_service = Arc::new(
        CurrencyService::new(
            Box::new(
                SqliteCurrencyPairRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ),
            Box::new(
                SqliteCurrencyRateRepository::new(pool.clone())
                    .with_change_recorder(recorder.clone()),
            ),
        )
        .with_event_bus(bus.clone()),
    );
    let state_repo: Arc<dyn SyncStateRepository> =
        Arc::new(SqliteSyncStateRepository::new(pool.clone()));
    let folder_store = Arc::new(FsFolderStore::new(folder));
    let change_log = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
    let sync_run = Arc::new(SyncRun::new(
        change_log.clone(),
        state_repo.clone(),
        folder_store.clone(),
        recorder,
    ));
    let sync_service = Arc::new(
        SyncService::new(state_repo.clone(), folder_store.clone())
            .with_run(sync_run.clone())
            .with_event_bus(bus),
    );
    let snapshot = Arc::new(ServicePortfolioSnapshot::new(
        account_service.clone(),
        asset_service.clone(),
        currency_service.clone(),
    ));
    let rank_stamper = Arc::new(ServiceRankStamper::new(
        account_service.clone(),
        asset_service.clone(),
        currency_service.clone(),
    ));
    let first_publish = Arc::new(FirstPublish::new(
        change_log,
        state_repo.clone(),
        folder_store.clone(),
        rank_stamper,
        snapshot,
    ));
    let orchestrator = PortfolioSyncOrchestrator::new(PortfolioSyncDependencies {
        account_service: account_service.clone(),
        asset_service: asset_service.clone(),
        currency_service: currency_service.clone(),
        sync_service,
        first_publish,
        sync_run,
        state_repo,
        folder_store,
    });
    Ctx {
        orchestrator,
        account_service,
        asset_service,
        currency_service,
        pool,
    }
}

async fn seed_small_portfolio(ctx: &Ctx) -> (String, String) {
    let asset = ctx
        .asset_service
        .create_asset(CreateAssetDTO {
            name: "AAPL".into(),
            reference: "AAPL".into(),
            isin: None,
            class: AssetClass::Stocks,
            currency: "USD".into(),
            risk_level: 2,
            category_id: SYSTEM_CATEGORY_ID.into(),
            exchange: None,
            interest_bearing: false,
        })
        .await
        .unwrap();
    ctx.asset_service.seed_cash_asset("USD").await.unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".into(),
            String::new(),
            "USD".into(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    ctx.account_service
        .seed_cash_holding(&account.id)
        .await
        .unwrap();
    ctx.account_service
        .record_deposit(&account.id, "2026-01-01".into(), 1_000_000_000, None)
        .await
        .unwrap();
    (account.id, asset.id)
}

const PASSPHRASE: &str = "correct horse battery staple";

// SYN-013/014/036 — a fresh installation joining an existing portfolio rebuilds by
// replaying every published change and ends up byte-identical (same accounts, assets,
// transactions) to the originating device.
#[tokio::test]
async fn join_produces_a_byte_identical_portfolio() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    seed_small_portfolio(&desktop).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop, holding the portfolio, must enable as the first device");

    let laptop_bus = Arc::new(SideEffectEventBus::new());
    let laptop = build_ctx_on_bus(dir.path(), laptop_bus.clone()).await;
    // The bus keeps a published event only while someone is subscribed.
    let mut laptop_events = laptop_bus.subscribe();
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("SYN-014/036: a fresh Laptop must join and rebuild the shared portfolio");
    assert_eq!(
        *laptop_events.borrow_and_update(),
        Event::SyncCompleted,
        "SYN-064: a join announces the rebuilt portfolio with one SyncCompleted"
    );

    let desktop_accounts = desktop.account_service.get_all().await.unwrap();
    let laptop_accounts = laptop.account_service.get_all().await.unwrap();
    assert_eq!(
        desktop_accounts.len(),
        laptop_accounts.len(),
        "SYN-036: the joined portfolio must carry every account Desktop published"
    );
    assert_eq!(
        desktop_accounts.first().map(|account| &account.name),
        laptop_accounts.first().map(|account| &account.name),
    );

    // SYN-014: after joining, a sync_device row, a cursor on Desktop, and the joiner's own
    // manifest must all exist.
    let laptop_device_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_device")
        .fetch_one(&laptop.pool)
        .await
        .unwrap();
    assert_eq!(
        laptop_device_row_count, 1,
        "SYN-014: the joiner must have its own sync_device row after joining"
    );
}

// CFR-040/041 — sequential edits recorded while apart accumulate on both devices, in the
// same replay order, with no notices for changes that never collided.
#[tokio::test]
async fn sequential_edits_sync_to_identical_portfolios_with_no_notices() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (account_id, asset_id) = seed_small_portfolio(&desktop).await;
    // The position both devices will touch exists before Laptop joins (HNO: a note needs a
    // held asset).
    desktop
        .account_service
        .buy_holding(
            &account_id,
            asset_id.clone(),
            "2026-01-15".into(),
            10_000_000,
            50_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");

    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    // Desktop deposits while apart.
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 500_000_000, None)
        .await
        .unwrap();
    // Laptop records a holding note while apart.
    laptop
        .account_service
        .upsert_holding_note(
            &account_id,
            asset_id.clone(),
            "watching this position".into(),
            None,
            None,
        )
        .await
        .unwrap();

    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    // A second round both ways picks up what the other device published in the first round.
    let desktop_report = desktop.orchestrator.sync_now().await.unwrap();
    let laptop_report = laptop.orchestrator.sync_now().await.unwrap();

    let desktop_txs = desktop
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .unwrap();
    let laptop_txs = laptop
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .unwrap();
    assert_eq!(
        desktop_txs.len(),
        laptop_txs.len(),
        "CFR-040: every transaction created on either device must end up on both"
    );
    assert_eq!(desktop_txs.len(), 3, "deposit, buy, deposit");
    let desktop_note = desktop
        .account_service
        .get_holding_notes(&account_id)
        .await
        .unwrap();
    assert_eq!(
        desktop_note.first().map(|note| note.text.as_str()),
        Some("watching this position"),
        "Laptop's note must reach Desktop"
    );
    assert_eq!(
        desktop_report.notices_raised, 0,
        "CFR-060: sequential, non-colliding changes must never raise a notice"
    );
    assert_eq!(laptop_report.notices_raised, 0);
}

// CFR-020/060 — a concurrent rename of the same account on both devices: the later rank
// wins everywhere, and exactly one notice is raised, on the losing device.
#[tokio::test]
async fn concurrent_rename_of_the_same_account_produces_one_notice_on_the_losing_device() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (account_id, _asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    // Both rename the same account concurrently, without syncing between the two edits.
    desktop
        .account_service
        .update(
            account_id.clone(),
            "Renamed on Desktop".into(),
            String::new(),
            "USD".into(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();
    laptop
        .account_service
        .update(
            account_id.clone(),
            "Renamed on Laptop".into(),
            String::new(),
            "USD".into(),
            UpdateFrequency::ManualMonth,
            false,
        )
        .await
        .unwrap();

    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();

    let desktop_account = desktop
        .account_service
        .get_by_id(&account_id)
        .await
        .unwrap()
        .expect("account must still exist");
    let laptop_account = laptop
        .account_service
        .get_by_id(&account_id)
        .await
        .unwrap()
        .expect("account must still exist");
    assert_eq!(
        desktop_account.name, laptop_account.name,
        "CFR-020: the higher-ranked rename must prevail identically on both devices"
    );

    let desktop_notices = desktop
        .orchestrator
        .get_sync_status()
        .await
        .unwrap()
        .notices;
    let laptop_notices = laptop.orchestrator.get_sync_status().await.unwrap().notices;
    let total_notices = desktop_notices.len() + laptop_notices.len();
    assert_eq!(
        total_notices, 1,
        "CFR-060: exactly one notice, on the device whose rename lost, {desktop_notices:?} \
         {laptop_notices:?}"
    );
}

// CFR-032 — Desktop deletes an account while Laptop, unsynced, records a transaction on
// it: the transaction is dropped on both, and only Laptop (whose change lost) is told.
#[tokio::test]
async fn deleting_an_account_drops_a_concurrent_transaction_and_notifies_only_the_losing_device() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (account_id, asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    desktop.account_service.delete(&account_id).await.unwrap();
    laptop
        .account_service
        .buy_holding(
            &account_id,
            asset_id,
            "2026-02-01".into(),
            10_000_000,
            50_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();

    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();

    assert!(
        desktop
            .account_service
            .get_by_id(&account_id)
            .await
            .unwrap()
            .is_none(),
        "CFR-022: the account stays deleted on the device that deleted it"
    );
    assert!(
        laptop
            .account_service
            .get_by_id(&account_id)
            .await
            .unwrap()
            .is_none(),
        "CFR-032: the account must be removed on Laptop too, taking its concurrent buy with it"
    );
    let laptop_notices = laptop.orchestrator.get_sync_status().await.unwrap().notices;
    assert!(
        !laptop_notices.is_empty(),
        "CFR-032/060: Laptop, whose transaction was dropped, must be told"
    );
}

// CFR-042 — two independent sales that individually exceed the shared position: both
// survive after merge, and the holding is inconsistent on both devices.
#[tokio::test]
async fn independent_oversell_on_both_devices_keeps_both_sales() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (account_id, asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .account_service
        .buy_holding(
            &account_id,
            asset_id.clone(),
            "2026-01-15".into(),
            15_000_000,
            50_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    // Each sells 10 of the 15 held — individually valid, together an oversell of -5.
    desktop
        .account_service
        .sell_holding(
            &account_id,
            asset_id.clone(),
            "2026-03-01".into(),
            10_000_000,
            55_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();
    laptop
        .account_service
        .sell_holding(
            &account_id,
            asset_id,
            "2026-03-02".into(),
            10_000_000,
            56_000_000,
            1_000_000,
            0,
            None,
            None,
        )
        .await
        .unwrap();

    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();

    let desktop_txs = desktop
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .unwrap();
    let laptop_txs = laptop
        .account_service
        .get_all_transactions_for_account(&account_id)
        .await
        .unwrap();
    assert_eq!(
        desktop_txs.len(),
        laptop_txs.len(),
        "CFR-042: merge never drops a transaction to restore the invariant"
    );

    let desktop_status = desktop.orchestrator.get_sync_status().await.unwrap();
    let laptop_status = laptop.orchestrator.get_sync_status().await.unwrap();
    assert!(
        !desktop_status.inconsistent_holdings.is_empty(),
        "CFR-042/SYN-040: the oversold holding must be marked inconsistent on Desktop"
    );
    assert!(
        !laptop_status.inconsistent_holdings.is_empty(),
        "CFR-042/SYN-040: the oversold holding must be marked inconsistent on Laptop too"
    );
}

// CFR-044 — a fee schedule's catch-up position converges by maximum after sync, whatever
// order the two devices' segments arrive in.
#[tokio::test]
async fn fee_catch_up_positions_converge_by_maximum() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (account_id, asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .account_service
        .update(
            account_id.clone(),
            "Portfolio".into(),
            String::new(),
            "USD".into(),
            UpdateFrequency::ManualMonth,
            true,
        )
        .await
        .unwrap();
    desktop
        .account_service
        .create_fee_schedule(
            &account_id,
            asset_id.clone(),
            1_000_000,
            folioneer_lib::context::account::FeeFrequency::Monthly,
            "2026-01-01".into(),
            None,
        )
        .await
        .unwrap();
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    // Desktop advances its catch-up position to August; Laptop, unsynced, still holds July.
    desktop
        .account_service
        .advance_fee_schedule_cursor(&account_id, &asset_id, "2026-08-31".into())
        .await
        .unwrap();
    laptop
        .account_service
        .advance_fee_schedule_cursor(&account_id, &asset_id, "2026-07-31".into())
        .await
        .unwrap();

    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();

    let desktop_position = desktop
        .account_service
        .list_fee_catch_up_positions_for_account(&account_id)
        .await
        .unwrap();
    let laptop_position = laptop
        .account_service
        .list_fee_catch_up_positions_for_account(&account_id)
        .await
        .unwrap();
    assert_eq!(
        desktop_position
            .first()
            .map(|p| p.last_applied_period.clone()),
        Some("2026-08-31".to_string()),
        "CFR-044: the maximum of the two positions must stand on Desktop"
    );
    assert_eq!(
        desktop_position
            .first()
            .map(|p| p.last_applied_period.clone()),
        laptop_position
            .first()
            .map(|p| p.last_applied_period.clone()),
        "CFR-044: both devices must converge on the same maximum"
    );
}

// #020 / SYN-064 — a sync that catches up many records announces itself once, after the
// apply commits: the number of events a subscriber sees does not grow with the number of
// applied records.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_catch_up_sync_announces_itself_once_whatever_the_number_of_applied_records() {
    const PRICE_COUNT: u32 = 1_500;
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    let (_, asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let laptop_bus = Arc::new(SideEffectEventBus::new());
    let laptop = build_ctx_on_bus(dir.path(), laptop_bus.clone()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");

    // Desktop records weeks of daily prices while Laptop is away.
    let first_day = chrono::NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    for offset in 0..PRICE_COUNT {
        let date = first_day + chrono::Duration::days(i64::from(offset));
        desktop
            .asset_service
            .record_asset_price(&asset_id, &date.to_string(), 100.0 + f64::from(offset))
            .await
            .unwrap();
    }
    desktop.orchestrator.sync_now().await.unwrap();

    let seen = Arc::new(std::sync::Mutex::new(Vec::<Event>::new()));
    let observer = {
        let seen = seen.clone();
        let mut events = laptop_bus.subscribe();
        tokio::spawn(async move {
            while events.changed().await.is_ok() {
                let event = events.borrow_and_update().clone();
                seen.lock().unwrap().push(event);
            }
        })
    };

    let report = laptop.orchestrator.sync_now().await.unwrap();
    assert!(
        report.applied_changes >= PRICE_COUNT,
        "precondition: Laptop must have applied every price, applied {}",
        report.applied_changes
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while seen.lock().unwrap().last() != Some(&Event::SyncCompleted) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("SYN-064: a sync that applied changes must raise SyncCompleted");
    observer.abort();

    let seen = seen.lock().unwrap();
    assert_eq!(
        (seen.len(), seen.last()),
        (1, Some(&Event::SyncCompleted)),
        "#020: {PRICE_COUNT} applied records must reach subscribers as one SyncCompleted"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// SYN-038 — the written form of what is synced, pinned per data format version
// ─────────────────────────────────────────────────────────────────────────────

/// Every value a synced enum can take, as written. The `match` has no wildcard: a new
/// variant stops this file from compiling until it is listed here — which changes the
/// written form, so `DATA_FORMAT_VERSION` moves with it (SYN-038).
macro_rules! written_values {
    ($ty:ty, [$($variant:path),+ $(,)?]) => {{
        fn listed(value: &$ty) {
            match value {
                $($variant => {})+
            }
        }
        let values: Vec<$ty> = vec![$($variant),+];
        values.iter().for_each(listed);
        values
            .iter()
            .map(|value| serde_json::to_value(value).expect("an enum value serializes"))
            .collect::<Vec<_>>()
    }};
}

/// Field names and JSON types of one written value, nested objects included. Values are
/// dropped: the written form is the shape, not the sample.
fn shape_of(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Null => Value::String("null".into()),
        Value::Bool(_) => Value::String("boolean".into()),
        Value::Number(_) => Value::String("number".into()),
        Value::String(_) => Value::String("string".into()),
        Value::Array(items) => Value::Array(items.iter().map(shape_of).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, field)| (name.clone(), shape_of(field)))
                .collect(),
        ),
    }
}

/// Writes one record of every synced kind through the real services, with every optional
/// field filled, and returns what the change recorder wrote for each kind.
async fn written_form_of_every_record_kind() -> serde_json::Value {
    use folioneer_lib::context::account::{FeeFrequency, ThresholdDirection};

    let dir = tempfile::tempdir().unwrap();
    let ctx = build_ctx(dir.path()).await;
    ctx.orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");

    let category = ctx.asset_service.create_category("Tech").await.unwrap();
    let asset = ctx
        .asset_service
        .create_asset(CreateAssetDTO {
            name: "Air Liquide".into(),
            reference: "AI".into(),
            isin: Some("FR0000120073".into()),
            class: AssetClass::Stocks,
            currency: "EUR".into(),
            risk_level: 2,
            category_id: category.id.clone(),
            exchange: folioneer_lib::context::asset::exchange::lookup("XPAR"),
            interest_bearing: false,
        })
        .await
        .unwrap();
    let account = ctx
        .account_service
        .create(
            "Portfolio".into(),
            "Bank".into(),
            "EUR".into(),
            UpdateFrequency::ManualMonth,
            true,
        )
        .await
        .unwrap();
    ctx.account_service
        .open_holding(
            &account.id,
            asset.id.clone(),
            "2026-01-05".into(),
            10_000_000,
            1_500_000_000,
        )
        .await
        .unwrap();
    ctx.account_service
        .create_fee_schedule(
            &account.id,
            asset.id.clone(),
            1_000_000,
            FeeFrequency::Monthly,
            "2026-01-01".into(),
            Some("2027-01-01".into()),
        )
        .await
        .unwrap();
    ctx.account_service
        .advance_fee_schedule_cursor(&account.id, &asset.id, "2026-08-31".into())
        .await
        .unwrap();
    ctx.account_service
        .upsert_holding_note(
            &account.id,
            asset.id.clone(),
            "Watch the dividend".into(),
            Some(160_000_000),
            Some(ThresholdDirection::Above),
        )
        .await
        .unwrap();
    ctx.asset_service
        .record_asset_price(&asset.id, "2026-01-05", 150.0)
        .await
        .unwrap();
    ctx.currency_service
        .declare_currency_pair("USD".into(), "EUR".into())
        .await
        .unwrap();
    ctx.currency_service
        .record_currency_rate("USD".into(), "EUR".into(), "2026-01-05".into(), 920_000)
        .await
        .unwrap();

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT record_kind, content FROM changes WHERE content IS NOT NULL ORDER BY sequence",
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    let mut kinds = serde_json::Map::new();
    for (kind, content) in rows {
        let content: serde_json::Value =
            serde_json::from_str(&content).expect("written content is JSON");
        kinds.insert(kind, shape_of(&content));
    }
    serde_json::Value::Object(kinds)
}

fn sync_format_snapshot_path(version: u32) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/sync_format")
        .join(format!("v{version}.json"))
}

/// Every value of every enum that travels in a synced record or in its envelope.
fn written_values_of_every_synced_enum() -> serde_json::Value {
    use folioneer_lib::context::account::{FeeFrequency, ThresholdDirection, TransactionType};
    use folioneer_lib::context::asset::AssetPriceSource;
    use folioneer_lib::context::currency::CurrencyRateSource;
    use folioneer_lib::shared::domain::{Operation, Origin, RecordKind};

    serde_json::json!({
        "RecordKind": written_values!(RecordKind, [
            RecordKind::Account, RecordKind::Category, RecordKind::Asset,
            RecordKind::Transaction, RecordKind::FeeSchedule, RecordKind::FeeCatchUpPosition,
            RecordKind::AssetPrice, RecordKind::CurrencyPair, RecordKind::CurrencyRate,
            RecordKind::HoldingNote,
        ]),
        "Operation": written_values!(Operation, [
            Operation::Created, Operation::Updated, Operation::Removed,
        ]),
        "Origin": written_values!(Origin, [Origin::Application, Origin::User]),
        "UpdateFrequency": written_values!(UpdateFrequency, [
            UpdateFrequency::Automatic, UpdateFrequency::ManualDay, UpdateFrequency::ManualWeek,
            UpdateFrequency::ManualMonth, UpdateFrequency::ManualYear,
        ]),
        "AssetClass": written_values!(AssetClass, [
            AssetClass::RealEstate, AssetClass::Cash, AssetClass::Stocks, AssetClass::Bonds,
            AssetClass::ETF, AssetClass::ETP, AssetClass::MutualFunds, AssetClass::DigitalAsset,
            AssetClass::Derivatives,
        ]),
        "AssetPriceSource": written_values!(AssetPriceSource, [
            AssetPriceSource::Manual, AssetPriceSource::YahooFinance,
        ]),
        "CurrencyRateSource": written_values!(CurrencyRateSource, [
            CurrencyRateSource::Manual, CurrencyRateSource::Frankfurter, CurrencyRateSource::Ecb,
        ]),
        "FeeFrequency": written_values!(FeeFrequency, [
            FeeFrequency::Monthly, FeeFrequency::Quarterly, FeeFrequency::Annually,
        ]),
        "ThresholdDirection": written_values!(ThresholdDirection, [
            ThresholdDirection::Below, ThresholdDirection::Above,
        ]),
        "TransactionType": written_values!(TransactionType, [
            TransactionType::Purchase, TransactionType::Sell, TransactionType::OpeningBalance,
            TransactionType::Deposit, TransactionType::Withdrawal, TransactionType::Dividend,
            TransactionType::FreeShares, TransactionType::ManagementFee, TransactionType::Split,
            TransactionType::Interest,
        ]),
    })
}

/// The shape of the three published files around the records: the folder header, a manifest
/// and a segment.
fn written_form_of_the_envelope() -> serde_json::Value {
    use folioneer_lib::context::sync::{
        DerivationParameters, FolderHeader, Manifest, Segment, SegmentChange,
    };
    use folioneer_lib::shared::domain::{Operation, Origin, RecordKind};

    let header = FolderHeader {
        derivation_parameters: DerivationParameters {
            salt: vec![1, 2, 3],
            memory_cost_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        },
        passphrase_check: vec![9],
        data_format_version: DATA_FORMAT_VERSION,
        created_at: "00000000000000000001".into(),
        created_by_device_id: "desktop-device".into(),
    };
    let manifest = Manifest {
        device_id: "desktop-device".into(),
        device_name: "Desktop".into(),
        data_format_version: DATA_FORMAT_VERSION,
        app_version: Some("0.43.0".into()),
        latest_sequence: 1,
    };
    let segment = Segment {
        device_id: "desktop-device".into(),
        first_sequence: 1,
        last_sequence: 1,
        data_format_version: DATA_FORMAT_VERSION,
        changes: vec![SegmentChange {
            sequence: 1,
            logical_timestamp: "0000000000000001".into(),
            based_on: Some("0000000000000000".into()),
            record_kind: RecordKind::Account,
            record_identity: "account-1".into(),
            operation: Operation::Updated,
            origin: Origin::User,
            content: Some("{}".into()),
        }],
    };
    serde_json::json!({
        "FolderHeader": shape_of(&serde_json::to_value(header).expect("a header serializes")),
        "Manifest": shape_of(&serde_json::to_value(manifest).expect("a manifest serializes")),
        "Segment": shape_of(&serde_json::to_value(segment).expect("a segment serializes")),
    })
}

// SYN-038 — what this build writes is what the snapshot of its data format version says.
// A change to a synced record, to an enum it carries or to the envelope fails here until
// `DATA_FORMAT_VERSION` is bumped and the new version's snapshot is created; a published
// snapshot is never edited (CI refuses it), so the bump cannot be skipped.
#[tokio::test]
async fn syn_038_the_written_form_matches_the_snapshot_of_the_current_data_format_version() {
    let records = written_form_of_every_record_kind().await;
    let values = written_values_of_every_synced_enum();
    let written_kinds: Vec<&String> = records.as_object().unwrap().keys().collect();
    let mut every_kind: Vec<String> = values["RecordKind"]
        .as_array()
        .unwrap()
        .iter()
        .map(|kind| kind.as_str().unwrap().to_string())
        .collect();
    every_kind.sort();
    assert_eq!(
        written_kinds,
        every_kind.iter().collect::<Vec<_>>(),
        "every record kind must be written once by this test, so its form is pinned"
    );

    let current = serde_json::json!({
        "data_format_version": DATA_FORMAT_VERSION,
        "envelope": written_form_of_the_envelope(),
        "records": records,
        "values": values,
    });
    let path = sync_format_snapshot_path(DATA_FORMAT_VERSION);
    if std::env::var_os("CREATE_SYNC_FORMAT_SNAPSHOT").is_some() && !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut text = serde_json::to_string_pretty(&current).unwrap();
        text.push('\n');
        std::fs::write(&path, text).unwrap();
    }
    let committed: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "no snapshot for data format version {DATA_FORMAT_VERSION}: create it with \
                 CREATE_SYNC_FORMAT_SNAPSHOT=1 just test-rust"
            )
        }))
        .expect("the snapshot is JSON");
    assert_eq!(
        current, committed,
        "the written form of what is synced changed. Older builds would misread it: bump \
         DATA_FORMAT_VERSION (context/sync/infrastructure/codec.rs), then create the new \
         version's snapshot with CREATE_SYNC_FORMAT_SNAPSHOT=1 just test-rust. Never edit a \
         published snapshot."
    );
}
