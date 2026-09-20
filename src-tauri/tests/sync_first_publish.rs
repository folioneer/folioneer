//! Integration tests for enabling as the first device (SYN-013/026) through the crate's public
//! API: seed a small portfolio via the real BC services, enable sync into a tempdir, and assert
//! the folder + database wiring is genuinely reachable end-to-end.
//!
//! Per `test_convention.md` Tier 3: only the public API is used, and `sync_now` is exercised
//! through the same public surface a second time after a new transaction.

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
    SqliteSyncStateRepository, SyncRun,
};
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

struct Ctx {
    orchestrator: PortfolioSyncOrchestrator,
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    pool: sqlx::Pool<sqlx::Sqlite>,
}

async fn build_ctx(folder: &std::path::Path) -> Ctx {
    let pool = make_pool().await;
    // Every synced repository records through the real change recorder, as the production
    // container wires it (SYN-020): `sync_now` publishes what it captured.
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
        )),
    );
    let asset_service = Arc::new(AssetService::new(
        Box::new(SqliteAssetRepository::new(pool.clone()).with_change_recorder(recorder.clone())),
        Box::new(
            SqliteAssetCategoryRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ),
        Box::new(
            SqliteAssetPriceRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ),
    ));
    let currency_service = Arc::new(CurrencyService::new(
        Box::new(
            SqliteCurrencyPairRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ),
        Box::new(
            SqliteCurrencyRateRepository::new(pool.clone()).with_change_recorder(recorder.clone()),
        ),
    ));
    let state_repo = Arc::new(SqliteSyncStateRepository::new(pool.clone()));
    let folder_store = Arc::new(FsFolderStore::new(folder));
    let change_log = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
    let sync_run = Arc::new(SyncRun::new(
        change_log.clone(),
        state_repo.clone(),
        folder_store.clone(),
        recorder,
    ));
    let sync_service = Arc::new(
        folioneer_lib::context::sync::SyncService::new(state_repo.clone(), folder_store.clone())
            .with_run(sync_run.clone()),
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
        currency_service,
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

// SYN-013 — enabling as the first device on an empty folder writes the header, one segment,
// and the manifest.
#[tokio::test]
async fn enable_as_first_device_writes_header_segment_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = build_ctx(dir.path()).await;
    seed_small_portfolio(&ctx).await;

    let status = ctx
        .orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            "correct horse battery staple".into(),
            "Desktop".into(),
        )
        .await
        .expect("enabling on an empty folder must succeed as the first device");
    assert!(status.enabled);

    let header_path = dir.path().join("vaultcompass-sync.json");
    assert!(header_path.exists(), "SYN-050: the header must be written");

    let devices_dir = dir.path().join("devices");
    let device_dirs: Vec<_> = std::fs::read_dir(&devices_dir)
        .expect("a devices/ directory must exist")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(
        device_dirs.len(),
        1,
        "exactly this device's area must exist"
    );

    let segments_dir = device_dirs[0].path().join("segments");
    let segment_files: Vec<_> = std::fs::read_dir(&segments_dir)
        .expect("a segments/ directory must exist")
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(segment_files.len(), 1, "SYN-013: exactly one first segment");

    let manifest_path = device_dirs[0].path().join("manifest.bin");
    assert!(
        manifest_path.exists(),
        "SYN-037: the manifest must be written"
    );
}

// SYN-061 — after enabling, sync_now following a new transaction publishes a second segment.
#[tokio::test]
async fn sync_now_after_a_new_transaction_publishes_a_second_segment() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = build_ctx(dir.path()).await;
    let (account_id, asset_id) = seed_small_portfolio(&ctx).await;

    ctx.orchestrator
        .enable_sync(
            dir.path().to_string_lossy().to_string(),
            "correct horse battery staple".into(),
            "Desktop".into(),
        )
        .await
        .expect("first publish must succeed");

    ctx.account_service
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

    let report = ctx
        .orchestrator
        .sync_now()
        .await
        .expect("sync_now after enabling must succeed");
    assert!(
        report.published_changes > 0,
        "the new transaction must publish"
    );

    let devices_dir = dir.path().join("devices");
    let device_dir = std::fs::read_dir(&devices_dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let segment_count = std::fs::read_dir(device_dir.join("segments"))
        .unwrap()
        .count();
    assert_eq!(
        segment_count, 2,
        "the new transaction must publish a second segment"
    );

    let unpublished: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM changes WHERE published = 0")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(
        unpublished, 0,
        "SYN-031/067: every captured change is marked published once its segment is written"
    );
}

const PASSPHRASE: &str = "correct horse battery staple";

async fn enable_first_device(ctx: &Ctx, folder: &std::path::Path) {
    ctx.orchestrator
        .enable_sync(
            folder.to_string_lossy().to_string(),
            PASSPHRASE.into(),
            "Desktop".into(),
        )
        .await
        .expect("enabling on an empty folder must succeed as the first device");
}

fn device_areas(folder: &std::path::Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(folder.join("devices"))
        .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default()
}

// SYN-074 — designating an empty folder republishes this device's portfolio there as a
// first device, under the kept key; the previous folder is left untouched.
#[tokio::test]
async fn change_sync_folder_to_an_empty_folder_republishes_there() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let ctx = build_ctx(first.path()).await;
    seed_small_portfolio(&ctx).await;
    enable_first_device(&ctx, first.path()).await;

    let status = ctx
        .orchestrator
        .change_sync_folder(second.path().to_string_lossy().to_string())
        .await
        .expect("an empty folder is adopted as a new origin");

    assert_eq!(
        status.folder.as_deref(),
        Some(second.path().to_string_lossy().as_ref())
    );
    assert!(second.path().join("vaultcompass-sync.json").exists());
    let areas = device_areas(second.path());
    assert_eq!(
        areas.len(),
        1,
        "this device's area is published in the new folder"
    );
    assert!(areas[0].join("manifest.bin").exists());
    assert!(
        first.path().join("vaultcompass-sync.json").exists(),
        "the previous folder keeps what was published there"
    );
}

// SYN-074 — a folder holding a portfolio under another passphrase is refused, and the
// device keeps its current folder.
#[tokio::test]
async fn change_sync_folder_to_a_folder_holding_another_portfolio_is_rejected() {
    let mine = tempfile::tempdir().unwrap();
    let theirs = tempfile::tempdir().unwrap();
    let ctx = build_ctx(mine.path()).await;
    seed_small_portfolio(&ctx).await;
    enable_first_device(&ctx, mine.path()).await;
    let other = build_ctx(theirs.path()).await;
    seed_small_portfolio(&other).await;
    other
        .orchestrator
        .enable_sync(
            theirs.path().to_string_lossy().to_string(),
            "another passphrase entirely".into(),
            "Office".into(),
        )
        .await
        .expect("the other portfolio enables in its own folder");

    let rejected = ctx
        .orchestrator
        .change_sync_folder(theirs.path().to_string_lossy().to_string())
        .await;

    assert!(
        matches!(
            rejected,
            Err(
                folioneer_lib::use_cases::portfolio_sync::PortfolioSyncError::Sync(
                    folioneer_lib::context::sync::SyncError::FolderHoldsOtherPortfolio
                )
            )
        ),
        "got {rejected:?}"
    );
    let status = ctx.orchestrator.get_sync_status().await.unwrap();
    assert_eq!(
        status.folder.as_deref(),
        Some(mine.path().to_string_lossy().as_ref()),
        "the device still follows its own folder"
    );
}

// SYN-071 — starting over clears the folder and publishes this device as a new origin under
// the new passphrase: one fresh area, a header the old passphrase no longer opens.
#[tokio::test]
async fn start_sync_over_clears_the_folder_and_publishes_as_a_new_origin() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = build_ctx(dir.path()).await;
    seed_small_portfolio(&ctx).await;
    enable_first_device(&ctx, dir.path()).await;
    let old_header = std::fs::read(dir.path().join("vaultcompass-sync.json")).unwrap();
    let old_area = device_areas(dir.path()).remove(0);

    let status = ctx
        .orchestrator
        .start_sync_over(
            dir.path().to_string_lossy().to_string(),
            "a brand new passphrase".into(),
            "Desktop".into(),
        )
        .await
        .expect("start over republishes as a new origin");

    assert!(status.enabled);
    let new_header = std::fs::read(dir.path().join("vaultcompass-sync.json")).unwrap();
    assert_ne!(
        old_header, new_header,
        "the header is rewritten for the new passphrase"
    );
    let areas = device_areas(dir.path());
    assert_eq!(
        areas.len(),
        1,
        "exactly one area: the previous history is gone"
    );
    assert_eq!(areas[0], old_area, "SYN-016: the device keeps its identity");
    assert!(areas[0].join("manifest.bin").exists());
    assert_eq!(
        std::fs::read_dir(areas[0].join("segments"))
            .unwrap()
            .count(),
        1,
        "the portfolio is published again as one first segment"
    );
}

// SYN-060/067 — the automatic run after a settled burst is a full sync: it completes and
// leaves a last-sync time behind for the status.
#[tokio::test]
async fn sync_after_changes_runs_a_full_sync_when_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = build_ctx(dir.path()).await;
    seed_small_portfolio(&ctx).await;
    enable_first_device(&ctx, dir.path()).await;

    ctx.orchestrator.sync_after_changes().await;

    let status = ctx.orchestrator.get_sync_status().await.unwrap();
    assert!(
        status.last_sync_completed_at.is_some(),
        "the automatic run completed a full sync"
    );
}
