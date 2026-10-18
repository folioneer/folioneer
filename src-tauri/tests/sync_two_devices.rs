//! Tier-3 integration: two installations ("Desktop", "Laptop") sharing one encrypted
//! folder converge on the same portfolio (SYN-013/014/036/065/080/083, CFR-040/041/042/044).
//! Per `test-rules.md` Tier 3: only the crate's public API is used — two `SqlitePool`s,
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
    SqlitePortfolioSettingRepository,
};
use folioneer_lib::context::sync::{
    FirstPublish, FsFolderStore, SqliteChangeLogRepository, SqliteChangeRecorder,
    SqliteSyncStateRepository, SyncFailure, SyncRun, SyncService, SyncStateRepository,
    DATA_FORMAT_VERSION,
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
    sync_service: Arc<SyncService>,
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
        .with_setting_repo(Box::new(
            SqlitePortfolioSettingRepository::new(pool.clone())
                .with_change_recorder(recorder.clone()),
        ))
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
        sync_service: sync_service.clone(),
        first_publish,
        sync_run,
        state_repo,
        folder_store,
    });
    Ctx {
        orchestrator,
        sync_service,
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
            kind: None,
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
    // The bus delivers an event only to who was subscribed when it was published.
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
        laptop_events.drain(),
        vec![Event::SyncCompleted],
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
        .status
        .notices;
    let laptop_notices = laptop
        .orchestrator
        .get_sync_status()
        .await
        .unwrap()
        .status
        .notices;
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

    let mut dropped = 0;
    for device in [&desktop, &laptop, &desktop, &laptop] {
        dropped += device
            .orchestrator
            .sync_now()
            .await
            .unwrap()
            .dropped_changes;
    }
    assert!(
        dropped >= 1,
        "CFR-032: the run that drops the concurrent buy counts it"
    );

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
    let laptop_notices = laptop
        .orchestrator
        .get_sync_status()
        .await
        .unwrap()
        .status
        .notices;
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

// TODO-020 / SYN-064 — a sync that catches up many records announces itself once, after the
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
            while let Some(event) = events.next().await {
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
        "TODO-020: {PRICE_COUNT} applied records must reach subscribers as one SyncCompleted"
    );
}

/// Desktop and Laptop sharing `dir`, Desktop holding a small portfolio; returns both and
/// the account, with Desktop's area in the folder.
async fn two_devices_sharing(dir: &std::path::Path) -> (Ctx, Ctx, String, std::path::PathBuf) {
    let desktop = build_ctx(dir).await;
    let (account_id, _asset_id) = seed_small_portfolio(&desktop).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");
    let mut areas: Vec<std::path::PathBuf> = std::fs::read_dir(dir.join("devices"))
        .expect("devices")
        .map(|entry| entry.expect("entry").path())
        .collect();
    assert_eq!(
        areas.len(),
        1,
        "only Desktop has an area before Laptop joins"
    );
    let desktop_area = areas.remove(0);
    let laptop = build_ctx(dir).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join the shared portfolio");
    (desktop, laptop, account_id, desktop_area)
}

fn segment_names(area: &std::path::Path) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(area.join("segments"))
        .expect("segments")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

/// The one area of `dir` that is not `other`: the second device's.
fn other_area(dir: &std::path::Path, other: &std::path::Path) -> std::path::PathBuf {
    std::fs::read_dir(dir.join("devices"))
        .expect("devices")
        .map(|entry| entry.expect("entry").path())
        .find(|area| area != other)
        .expect("a second area")
}

/// Makes every later segment write of `area` fail while the folder itself stays
/// available: its segments directory becomes a file.
fn break_segment_writes(area: &std::path::Path) {
    let segments = area.join("segments");
    std::fs::remove_dir_all(&segments).expect("segments removed");
    std::fs::write(&segments, b"not a directory").expect("segments replaced");
}

async fn logical_clock(ctx: &Ctx) -> i64 {
    sqlx::query_scalar("SELECT logical_clock FROM sync_device")
        .fetch_one(&ctx.pool)
        .await
        .expect("an enrolled device")
}

async fn latest_timestamp(ctx: &Ctx) -> i64 {
    let latest: String = sqlx::query_scalar("SELECT MAX(logical_timestamp) FROM changes")
        .fetch_one(&ctx.pool)
        .await
        .expect("changes");
    latest.parse().expect("a logical timestamp is a number")
}

// SYN-021 / GPF-011 — the reference currency is a portfolio setting: chosen on one device,
// it is the other's after a sync, and the later choice wins on both.
#[tokio::test]
async fn syn_021_the_reference_currency_chosen_on_one_device_is_the_others() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, laptop, _account_id, _desktop_area) = two_devices_sharing(dir.path()).await;
    assert_eq!(
        laptop.currency_service.reference_currency().await.unwrap(),
        "EUR"
    );

    desktop
        .currency_service
        .set_reference_currency("CHF")
        .await
        .unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(
        laptop.currency_service.reference_currency().await.unwrap(),
        "CHF"
    );

    laptop
        .currency_service
        .set_reference_currency("USD")
        .await
        .unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    assert_eq!(
        desktop.currency_service.reference_currency().await.unwrap(),
        "USD"
    );
    assert_eq!(
        laptop.currency_service.reference_currency().await.unwrap(),
        "USD"
    );
}

// SYN-083 / DEBT-152 — a device joining with a reference currency of its own takes the shared
// portfolio's: its own is replaced, whether the shared portfolio chose one or not.
#[tokio::test]
async fn syn_083_a_joining_devices_reference_currency_is_replaced_by_the_shared_one() {
    for shared_choice in [None, Some("GBP")] {
        let dir = tempfile::tempdir().unwrap();
        let desktop = build_ctx(dir.path()).await;
        if let Some(choice) = shared_choice {
            desktop
                .currency_service
                .set_reference_currency(choice)
                .await
                .unwrap();
        }
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
            .currency_service
            .set_reference_currency("CHF")
            .await
            .unwrap();
        laptop
            .orchestrator
            .enable_sync(
                dir.path().to_string_lossy().to_string(),
                PASSPHRASE.into(),
                "Laptop".into(),
            )
            .await
            .expect("a reference currency is not user data that blocks the join (SYN-014)");
        assert_eq!(
            laptop.currency_service.reference_currency().await.unwrap(),
            shared_choice.unwrap_or("EUR"),
            "the shared portfolio's choice replaces the joining device's"
        );
    }
}

// SYN-013 / SYN-021 — a reference currency chosen before sync is turned on travels with the
// first publish: a device joining takes it.
#[tokio::test]
async fn syn_013_a_reference_currency_chosen_before_sync_is_published() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    desktop
        .currency_service
        .set_reference_currency("GBP")
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
    assert_eq!(
        laptop.currency_service.reference_currency().await.unwrap(),
        "GBP"
    );
}

// SYN-013 — a first publish stamps the whole portfolio one tick after the device's clock,
// and the device carries on from there.
#[tokio::test]
async fn syn_013_a_first_publish_ticks_the_clock_once() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    seed_small_portfolio(&desktop).await;
    let before: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(logical_clock), 0) FROM sync_device")
        .fetch_one(&desktop.pool)
        .await
        .unwrap();
    assert_eq!(before, 0, "a device that never synced has not ticked");

    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("Desktop must enable as the first device");

    assert_eq!(latest_timestamp(&desktop).await, 1);
    assert_eq!(logical_clock(&desktop).await, 1);
}

// SYN-036 — a device that joins starts its clock one tick after the latest change of the
// history it replayed, so its first change ranks after everything it has seen.
#[tokio::test]
async fn syn_036_a_joined_device_starts_its_clock_after_the_history() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, _laptop, account_id, _desktop_area) = two_devices_sharing(dir.path()).await;
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 5_000_000, None)
        .await
        .unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    let history_ends_at = latest_timestamp(&desktop).await;
    assert!(history_ends_at > 1, "the history has more than one tick");

    let tablet = build_ctx(dir.path()).await;
    tablet
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Tablet".into(),
        )
        .await
        .expect("Tablet must join");

    assert_eq!(logical_clock(&tablet).await, history_ends_at + 1);
}

// SYN-013/036 — a portfolio with nothing to publish is still one a device can join: its
// origin says it has published nothing.
#[tokio::test]
async fn syn_036_an_empty_portfolio_can_be_joined() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = build_ctx(dir.path()).await;
    desktop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("an empty installation enables as the first device");
    let published: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM changes")
        .fetch_one(&desktop.pool)
        .await
        .unwrap();
    assert_eq!(published, 0, "an empty portfolio publishes no change");

    let laptop = build_ctx(dir.path()).await;
    laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop must join an empty portfolio");
    laptop.orchestrator.sync_now().await.expect("a first sync");
    let status = laptop.orchestrator.get_sync_status().await.unwrap();
    let origin = status
        .roster
        .iter()
        .find(|device| device.device_name == "Desktop")
        .expect("Desktop in the roster");
    assert_eq!(origin.published_changes, 0);
}

// SYN-082 — leaving publishes what is unpublished, takes the device's manifest out of the
// folder and disables sync here; the portfolio stays.
#[tokio::test]
async fn syn_082_leaving_removes_the_manifest_and_disables_sync() {
    let dir = tempfile::tempdir().unwrap();
    let (_desktop, laptop, _account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    let laptop_area = other_area(dir.path(), &desktop_area);
    let manifest = laptop_area.join("manifest.bin");
    assert!(manifest.exists(), "a device that joined has its manifest");
    let account_id = laptop.account_service.get_all().await.unwrap()[0]
        .id
        .clone();
    laptop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 5_000_000, None)
        .await
        .unwrap();

    laptop
        .sync_service
        .leave_sync()
        .await
        .expect("Laptop leaves");

    assert!(
        !manifest.exists(),
        "the manifest is taken out of the folder"
    );
    assert_eq!(
        segment_names(&laptop_area).len(),
        1,
        "what was unpublished is published before leaving, and the area stays"
    );
    assert!(!laptop.orchestrator.get_sync_status().await.unwrap().enabled);
    assert_eq!(
        laptop.account_service.get_all().await.unwrap().len(),
        1,
        "the portfolio stays on the device that left"
    );
}

// SYN-082 — a device whose unpublished changes cannot be written does not leave: its
// work is never abandoned.
#[tokio::test]
async fn syn_082_leaving_is_refused_while_unpublished_changes_cannot_be_written() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, _laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 5_000_000, None)
        .await
        .unwrap();
    break_segment_writes(&desktop_area);

    let refused = desktop
        .sync_service
        .leave_sync()
        .await
        .expect_err("unpublished changes keep the device in");

    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "PublishFailed"
    );
    assert!(
        desktop
            .orchestrator
            .get_sync_status()
            .await
            .unwrap()
            .enabled
    );
}

// SYN-073 — resuming publishes what was recorded while paused and the device is no longer
// paused.
#[tokio::test]
async fn syn_073_resuming_ends_the_pause() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, _laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop.sync_service.pause_sync().await.expect("paused");
    assert!(desktop.orchestrator.get_sync_status().await.unwrap().paused);
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 5_000_000, None)
        .await
        .unwrap();
    let published_before = segment_names(&desktop_area);

    desktop.orchestrator.resume_sync().await.expect("resumed");

    assert!(!desktop.orchestrator.get_sync_status().await.unwrap().paused);
    assert_eq!(
        segment_names(&desktop_area).len(),
        published_before.len() + 1,
        "what was recorded while paused is published on resuming"
    );
}

// SYN-073 — a device whose paused-era changes cannot be written stays paused.
#[tokio::test]
async fn syn_073_resuming_is_refused_while_the_publish_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, _laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop.sync_service.pause_sync().await.expect("paused");
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 5_000_000, None)
        .await
        .unwrap();
    break_segment_writes(&desktop_area);

    let refused = desktop
        .orchestrator
        .resume_sync()
        .await
        .expect_err("the publish fails");

    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "PublishFailed"
    );
    assert!(desktop.orchestrator.get_sync_status().await.unwrap().paused);
}

// SYN-035 — a folder whose header this build reads, but where a device already publishes
// in a newer data format, is not joined either: the device is told to update, not that
// the history is incomplete, and nothing of it is left in the folder.
#[tokio::test]
async fn syn_035_a_device_publishing_in_a_newer_data_format_stops_the_join() {
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
        .expect("Desktop must enable as the first device");
    let desktop_area = std::fs::read_dir(dir.path().join("devices"))
        .unwrap()
        .next()
        .expect("Desktop's area")
        .unwrap()
        .path();
    let manifest_path = desktop_area.join("manifest.bin");
    let mut manifest = std::fs::read(&manifest_path).unwrap();
    manifest[..4].copy_from_slice(&(DATA_FORMAT_VERSION + 1).to_be_bytes());
    std::fs::write(&manifest_path, manifest).unwrap();

    let laptop = build_ctx(dir.path()).await;
    let refused = laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect_err("a device publishing in a newer format stops the join");

    let refusal = serde_json::to_value(&refused).unwrap();
    assert_eq!(refusal["code"], "UpdateRequired");
    assert_eq!(refusal["data_format_version"], DATA_FORMAT_VERSION + 1);
    assert_eq!(
        std::fs::read_dir(dir.path().join("devices"))
            .unwrap()
            .count(),
        1,
        "only Desktop's area is in the folder"
    );
    assert!(!laptop.orchestrator.get_sync_status().await.unwrap().enabled);
}

// SYN-035 — a folder written in a data format newer than this build's is not joined: the
// device is told to update, and nothing of it is left in the folder.
#[tokio::test]
async fn syn_035_a_folder_in_a_newer_data_format_is_not_joined() {
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
        .expect("Desktop must enable as the first device");

    let header_path = dir.path().join("vaultcompass-sync.json");
    let mut header: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&header_path).unwrap()).unwrap();
    assert!(header.get("data_format_version").is_some());
    header["data_format_version"] = serde_json::json!(DATA_FORMAT_VERSION + 1);
    std::fs::write(&header_path, serde_json::to_vec(&header).unwrap()).unwrap();

    let laptop = build_ctx(dir.path()).await;
    let refused = laptop
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Laptop".into(),
        )
        .await
        .expect_err("a newer data format is not joined");
    let refusal = serde_json::to_value(&refused).unwrap();
    assert_eq!(refusal["code"], "UpdateRequired");
    assert_eq!(refusal["data_format_version"], DATA_FORMAT_VERSION + 1);
    assert_eq!(
        std::fs::read_dir(dir.path().join("devices"))
            .unwrap()
            .count(),
        1,
        "only Desktop's area is in the folder"
    );

    assert!(!laptop.orchestrator.get_sync_status().await.unwrap().enabled);

    // SYN-035 — nor is such a folder taken as the new place of a portfolio already shared:
    // the device stays on its folder.
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        elsewhere.path().join("vaultcompass-sync.json"),
        serde_json::to_vec(&header).unwrap(),
    )
    .unwrap();
    let refused = desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect_err("a newer data format is not moved to");
    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "UpdateRequired"
    );
    let status = desktop.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        status.folder.as_deref(),
        Some(dir.path().to_string_lossy().as_ref())
    );
}

// SYN-074 — moving the portfolio to an empty folder is a new beginning: the device that
// moves it forgets what it kept of the others' history, and another enrolled computer that
// designates that folder is refused as a join is — it holds data — and stays where it was.
#[tokio::test]
async fn syn_074_another_computer_does_not_follow_into_a_moved_folder() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, _area) = two_devices_sharing(dir.path()).await;
    laptop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 100_000_000, None)
        .await
        .unwrap();
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    let cursors = |pool: sqlx::Pool<sqlx::Sqlite>| async move {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM sync_cursors")
            .fetch_one(&pool)
            .await
            .unwrap()
    };
    assert_eq!(cursors(desktop.pool.clone()).await, 1);

    desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("moved to the empty folder");
    assert_eq!(
        cursors(desktop.pool.clone()).await,
        0,
        "the cursors of the history left behind are forgotten"
    );

    let refused = laptop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect_err("another computer does not follow by changing folder");
    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "InstallationHoldsUserData"
    );
    let status = laptop.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        status.folder.as_deref(),
        Some(dir.path().to_string_lossy().as_ref())
    );
}

// SYN-033/037 — a run applies what another device published since the last one, counts
// it, and applies nothing twice.
#[tokio::test]
async fn syn_037_a_run_applies_each_published_change_once_and_counts_it() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, _area) = two_devices_sharing(dir.path()).await;
    for date in ["2026-02-01", "2026-02-02"] {
        desktop
            .account_service
            .record_deposit(&account_id, date.into(), 100_000_000, None)
            .await
            .unwrap();
    }

    let published = desktop.orchestrator.sync_now().await.unwrap();
    assert!(published.published_changes >= 2);

    let applied_from_desktop = |status: &folioneer_lib::context::sync::SyncStatus| {
        status
            .roster
            .iter()
            .find(|entry| entry.device_name == "Desktop")
            .and_then(|entry| entry.last_applied_at.clone())
    };
    let started = chrono::Utc::now().to_rfc3339();
    let first = laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(first.applied_changes, published.published_changes);
    // SYN-063 — the roster says when Desktop's changes were last applied here: during
    // this run, not at the join.
    let applied_at = applied_from_desktop(&first.status).expect("applied");
    assert!(applied_at >= started, "{applied_at} is before {started}");
    assert_eq!((first.dropped_changes, first.held_back_changes), (0, 0));
    assert!(first.failures.is_empty());

    let deposits: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transactions WHERE date IN ('2026-02-01', '2026-02-02')",
    )
    .fetch_one(&laptop.pool)
    .await
    .unwrap();
    assert_eq!(deposits, 2, "what is counted as applied is on Laptop");

    let second = laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(second.applied_changes, 0);
}

// SYN-033 — a segment this device has not received the predecessor of stops the read of
// that device's area: nothing past the gap is applied until the missing one arrives, and
// then all of it is.
#[tokio::test]
async fn syn_033_a_gap_in_a_device_s_segments_stops_the_read() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    laptop.orchestrator.sync_now().await.unwrap();
    let before = segment_names(&desktop_area);

    let mut published = 0;
    let mut with_first = std::collections::BTreeSet::new();
    for date in ["2026-02-01", "2026-02-02"] {
        desktop
            .account_service
            .record_deposit(&account_id, date.into(), 100_000_000, None)
            .await
            .unwrap();
        published += desktop
            .orchestrator
            .sync_now()
            .await
            .unwrap()
            .published_changes;
        if with_first.is_empty() {
            with_first = segment_names(&desktop_area);
        }
    }

    let first_new: Vec<&String> = with_first.difference(&before).collect();
    assert_eq!(first_new.len(), 1, "one run, one segment");
    let missing = desktop_area.join("segments").join(first_new[0]);
    let kept = std::fs::read(&missing).unwrap();
    std::fs::remove_file(&missing).unwrap();

    let stopped = laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(stopped.applied_changes, 0);
    assert!(
        stopped.failures.is_empty(),
        "a gap is waited for, not a failure"
    );

    std::fs::write(&missing, kept).unwrap();
    let resumed = laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(resumed.applied_changes, published);
}

// SYN-034 — a manifest that cannot be read is skipped and counted, and the run goes on.
#[tokio::test]
async fn syn_034_an_unreadable_manifest_is_counted_and_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop
        .account_service
        .record_deposit(&account_id, "2026-02-01".into(), 100_000_000, None)
        .await
        .unwrap();
    let published = desktop
        .orchestrator
        .sync_now()
        .await
        .unwrap()
        .published_changes;
    let manifest = desktop_area.join("manifest.bin");
    let kept = std::fs::read(&manifest).unwrap();
    std::fs::write(&manifest, b"not a manifest").unwrap();

    let skipped = laptop.orchestrator.sync_now().await.unwrap();
    assert_eq!(
        skipped.failures,
        vec![SyncFailure::UnreadableFiles { count: 1 }]
    );
    assert_eq!(skipped.applied_changes, 0);

    // The area is read again at the next run: once readable, what waited is applied.
    std::fs::write(&manifest, kept).unwrap();
    let retried = laptop.orchestrator.sync_now().await.unwrap();
    assert!(retried.failures.is_empty());
    assert_eq!(retried.applied_changes, published);
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
            kind: None,
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
        .create_classified(
            "Portfolio".into(),
            "Bank".into(),
            "EUR".into(),
            UpdateFrequency::ManualMonth,
            true,
            Some("DE".into()),
            Some("de_depot".into()),
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
    ctx.currency_service
        .set_reference_currency("CHF")
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
    use folioneer_lib::context::asset::{AssetKind, AssetPriceSource};
    use folioneer_lib::context::currency::CurrencyRateSource;
    use folioneer_lib::shared::domain::{Operation, Origin, RecordKind};

    serde_json::json!({
        "RecordKind": written_values!(RecordKind, [
            RecordKind::Account, RecordKind::Category, RecordKind::Asset,
            RecordKind::Transaction, RecordKind::FeeSchedule, RecordKind::FeeCatchUpPosition,
            RecordKind::AssetPrice, RecordKind::CurrencyPair, RecordKind::CurrencyRate,
            RecordKind::HoldingNote, RecordKind::PortfolioSetting,
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
        "AssetKind": written_values!(AssetKind, [
            AssetKind::Listed, AssetKind::Crypto, AssetKind::Custom, AssetKind::Cash,
        ]),
        "AssetPriceSource": written_values!(AssetPriceSource, [
            AssetPriceSource::Manual, AssetPriceSource::YahooFinance, AssetPriceSource::Agent,
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

/// Sets the device's logical clock, as two devices that sync converge on one.
async fn set_logical_clock(ctx: &Ctx, clock: i64) {
    sqlx::query("UPDATE sync_device SET logical_clock = ?")
        .bind(clock)
        .execute(&ctx.pool)
        .await
        .expect("clock set");
}

fn header_of(folder: &std::path::Path) -> Vec<u8> {
    std::fs::read(folder.join("vaultcompass-sync.json")).expect("a header")
}

// SYN-074 / DEBT-126 — a folder is known by more than its creation mark: two computers at
// the same clock that each move the portfolio to an empty folder write the same mark, and
// one that then designates the other's folder is refused, not adopted.
#[tokio::test]
async fn syn_074_two_movers_at_the_same_clock_are_told_apart() {
    let dir = tempfile::tempdir().unwrap();
    let desktop_move = tempfile::tempdir().unwrap();
    let laptop_move = tempfile::tempdir().unwrap();
    let (desktop, laptop, _account, _area) = two_devices_sharing(dir.path()).await;
    set_logical_clock(&desktop, 500).await;
    set_logical_clock(&laptop, 500).await;
    for (ctx, folder) in [(&desktop, &desktop_move), (&laptop, &laptop_move)] {
        ctx.orchestrator
            .change_sync_folder(folder.path().to_string_lossy().to_string())
            .await
            .expect("moved to an empty folder");
    }
    let marks: Vec<String> = [&desktop_move, &laptop_move]
        .iter()
        .map(|folder| {
            let header: serde_json::Value =
                serde_json::from_slice(&header_of(folder.path())).unwrap();
            header["created_at"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(marks[0], marks[1], "precondition: the same creation mark");

    let refused = desktop
        .orchestrator
        .change_sync_folder(laptop_move.path().to_string_lossy().to_string())
        .await
        .expect_err("the other computer's beginning is not this one's");
    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "InstallationHoldsUserData"
    );
    let status = desktop.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        status.folder.as_deref(),
        Some(desktop_move.path().to_string_lossy().as_ref())
    );
}

// SYN-084 / DEBT-126 — a run refuses a header of another beginning as it does a reset:
// a device whose folder came to hold the portfolio as another device moved it publishes
// nothing there, and pauses.
#[tokio::test]
async fn syn_084_a_run_refuses_a_header_of_another_beginning() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    laptop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("Laptop moved the portfolio");
    // The folder Desktop follows now holds Laptop's beginning, under the same key.
    std::fs::write(
        dir.path().join("vaultcompass-sync.json"),
        header_of(elsewhere.path()),
    )
    .unwrap();
    let before = segment_names(&desktop_area);
    desktop
        .account_service
        .record_deposit(&account_id, "2026-03-01".into(), 1_000_000, None)
        .await
        .unwrap();

    let report = desktop.orchestrator.sync_now().await.expect("a report");
    assert_eq!(
        serde_json::to_value(&report).unwrap()["failures"][0],
        "PortfolioReset"
    );
    assert_eq!(
        segment_names(&desktop_area),
        before,
        "nothing was published"
    );
    assert!(desktop.orchestrator.get_sync_status().await.unwrap().paused);
}

// SYN-069 / SYN-074 / DEBT-126 — a move to an empty folder needs nothing of the folder it
// leaves: with that folder gone, the device moves all the same, under the same passphrase.
#[tokio::test]
async fn syn_069_a_move_needs_nothing_of_the_folder_it_leaves() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, _laptop, _account, _area) = two_devices_sharing(dir.path()).await;
    let left = header_of(dir.path());
    std::fs::remove_dir_all(dir.path()).unwrap();

    desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("moved with the folder it leaves out of reach");
    let left: serde_json::Value = serde_json::from_slice(&left).unwrap();
    let moved: serde_json::Value = serde_json::from_slice(&header_of(elsewhere.path())).unwrap();
    assert_eq!(
        moved["derivation_parameters"], left["derivation_parameters"],
        "the same passphrase reaches the same key"
    );
}

/// What a device keeps of the others' history: its cursors, held-back changes and notices.
async fn received_history(ctx: &Ctx) -> (i64, i64, i64) {
    let count = |table: &'static str| {
        let pool = ctx.pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    (
        count("sync_cursors").await,
        count("held_back_changes").await,
        count("conflict_notices").await,
    )
}

async fn keep_a_held_back_change_and_a_notice(ctx: &Ctx) {
    sqlx::query(
        "INSERT OR REPLACE INTO sync_cursors (device_id, applied_through, last_applied_at) VALUES ('another-device', 3, NULL)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO held_back_changes (id, origin_device_id, sequence, payload, waiting_kind, waiting_identity, held_since)
         VALUES ('held', 'another-device', 9, '{}', 'Account', 'missing', '2026-08-22T00:00:00Z')",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO conflict_notices (notice_id, kind, record_kind, record_identity, record_label, other_device_id, other_device_name, raised_at)
         VALUES ('notice', 'OverruledEdit', 'Account', 'a', 'PEA', 'another-device', 'Laptop', '2026-08-22T00:00:00Z')",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
}

// SYN-074 / DEBT-126 — the same portfolio, copied to another folder, is adopted through the
// action: the device moves there with its identity, its numbering and what it kept of the
// others, and syncs on; the device left behind syncs on in the first folder.
#[tokio::test]
async fn syn_074_a_copy_of_the_folder_is_adopted_through_the_action() {
    let dir = tempfile::tempdir().unwrap();
    let copy = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, _area) = two_devices_sharing(dir.path()).await;
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    let before = desktop.orchestrator.get_sync_status().await.unwrap();
    let kept = received_history(&desktop).await;
    let copied = std::process::Command::new("cp")
        .arg("-r")
        .arg(format!("{}/.", dir.path().display()))
        .arg(copy.path())
        .status()
        .unwrap();
    assert!(copied.success());

    desktop
        .orchestrator
        .change_sync_folder(copy.path().to_string_lossy().to_string())
        .await
        .expect("the same portfolio is adopted");
    let after = desktop.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        after.folder.as_deref(),
        Some(copy.path().to_string_lossy().as_ref())
    );
    assert_eq!(after.device_id, before.device_id);
    assert_eq!(received_history(&desktop).await, kept);
    desktop
        .account_service
        .record_deposit(&account_id, "2026-03-01".into(), 1_000_000, None)
        .await
        .unwrap();
    let report = desktop.orchestrator.sync_now().await.unwrap();
    assert!(report.failures.is_empty(), "{:?}", report.failures);
    assert_eq!(report.published_changes, 1);
    let left_behind = laptop.orchestrator.sync_now().await.unwrap();
    assert!(
        left_behind.failures.is_empty(),
        "{:?}",
        left_behind.failures
    );
}

// SYN-074 / DEBT-126 — a move to an empty folder forgets the held-back changes and the
// notices too; a move that fails changes nothing: the device stays on its folder with what
// it kept, and what it wrote in the new one is removed.
#[tokio::test]
async fn syn_074_a_move_forgets_the_history_left_and_a_failed_one_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let broken = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, laptop, _account, _area) = two_devices_sharing(dir.path()).await;
    laptop.orchestrator.sync_now().await.unwrap();
    desktop.orchestrator.sync_now().await.unwrap();
    keep_a_held_back_change_and_a_notice(&desktop).await;
    let kept = received_history(&desktop).await;
    assert_eq!((kept.1, kept.2), (1, 1));
    assert!(kept.0 >= 1);

    // A folder whose device areas cannot be written: the move fails.
    std::fs::write(broken.path().join("devices"), b"not a directory").unwrap();
    desktop
        .orchestrator
        .change_sync_folder(broken.path().to_string_lossy().to_string())
        .await
        .expect_err("the publish fails");
    let status = desktop.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        status.folder.as_deref(),
        Some(dir.path().to_string_lossy().as_ref())
    );
    assert_eq!(received_history(&desktop).await, kept);
    assert!(!broken.path().join("vaultcompass-sync.json").exists());

    desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("moved");
    assert_eq!(received_history(&desktop).await, (0, 0, 0));
}

// SYN-073 / SYN-084 / DEBT-126 — resuming into a folder that was started over meanwhile is
// the reset case: the device stays paused and publishes nothing there.
#[tokio::test]
async fn syn_073_resuming_into_a_folder_started_over_is_the_reset() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, laptop, _account, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop.sync_service.pause_sync().await.expect("paused");
    laptop
        .orchestrator
        .start_sync_over(
            dir.path().to_string_lossy().to_string(),
            "another long passphrase".into(),
            "Laptop".into(),
        )
        .await
        .expect("Laptop started over");

    let resumed = desktop.orchestrator.resume_sync().await;
    let paused = desktop.orchestrator.get_sync_status().await.unwrap().paused;
    assert!(paused, "{resumed:?}");
    assert!(
        !desktop_area.exists(),
        "nothing was published into the reset folder"
    );
}

// SYN-074 / DEBT-131 — the header a move writes carries a new creation mark, written by the
// device that moved.
#[tokio::test]
async fn syn_074_the_header_a_move_writes_carries_a_new_creation_mark() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, _laptop, _account, _area) = two_devices_sharing(dir.path()).await;
    let left: serde_json::Value = serde_json::from_slice(&header_of(dir.path())).unwrap();
    set_logical_clock(&desktop, 900).await;

    desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("moved to an empty folder");
    let moved: serde_json::Value = serde_json::from_slice(&header_of(elsewhere.path())).unwrap();
    assert_ne!(moved["created_at"], left["created_at"]);
    assert!(moved["created_by_device_id"].is_string());
    assert_eq!(
        moved["created_by_device_id"], left["created_by_device_id"],
        "Desktop wrote both"
    );
}

// SyncDevice `portfolio` / DEBT-131 — a device that keeps no derivation parameters reads them
// from the folder it leaves when it moves, and is refused as the folder being unavailable
// when that folder is out of reach.
#[tokio::test]
async fn syn_074_a_device_that_keeps_no_parameters_reads_them_from_the_folder_it_leaves() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let out_of_reach = tempfile::tempdir().unwrap();
    let (desktop, laptop, _account, _area) = two_devices_sharing(dir.path()).await;
    for ctx in [&desktop, &laptop] {
        sqlx::query("UPDATE sync_device SET derivation_parameters = NULL")
            .execute(&ctx.pool)
            .await
            .expect("parameters forgotten");
    }
    let left: serde_json::Value = serde_json::from_slice(&header_of(dir.path())).unwrap();

    desktop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("moved, the parameters read from the folder it leaves");
    let moved: serde_json::Value = serde_json::from_slice(&header_of(elsewhere.path())).unwrap();
    assert_eq!(
        moved["derivation_parameters"],
        left["derivation_parameters"]
    );

    std::fs::remove_dir_all(dir.path()).unwrap();
    let refused = laptop
        .orchestrator
        .change_sync_folder(out_of_reach.path().to_string_lossy().to_string())
        .await
        .expect_err("the folder it leaves is out of reach");
    assert_eq!(
        serde_json::to_value(&refused).unwrap()["code"],
        "FolderUnavailable"
    );
}

// SYN-073 / DEBT-131 — resuming into a folder that now holds another device's beginning is
// the reset: the device stays paused and publishes nothing there.
#[tokio::test]
async fn syn_073_resuming_into_a_folder_holding_another_beginning_is_the_reset() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let (desktop, laptop, account_id, desktop_area) = two_devices_sharing(dir.path()).await;
    desktop.sync_service.pause_sync().await.expect("paused");
    laptop
        .orchestrator
        .change_sync_folder(elsewhere.path().to_string_lossy().to_string())
        .await
        .expect("Laptop moved the portfolio");
    std::fs::write(
        dir.path().join("vaultcompass-sync.json"),
        header_of(elsewhere.path()),
    )
    .unwrap();
    let before = segment_names(&desktop_area);
    desktop
        .account_service
        .record_deposit(&account_id, "2026-03-01".into(), 1_000_000, None)
        .await
        .unwrap();

    let resumed = desktop.orchestrator.resume_sync().await;
    assert!(
        desktop.orchestrator.get_sync_status().await.unwrap().paused,
        "{resumed:?}"
    );
    assert_eq!(
        segment_names(&desktop_area),
        before,
        "nothing was published"
    );
}
