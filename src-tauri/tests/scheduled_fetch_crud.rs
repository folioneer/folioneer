/// Integration tests for the scheduled-fetch use case (SPF spec).
///
/// Exercises the full stack through the public `folioneer_lib` API:
/// `ScheduledFetchOrchestrator` → real in-memory SQLite (via
/// `SqliteScheduledFetchRepository`). The OS scheduler is the inert
/// `NoopScheduler` — the same adapter used by E2E runs — so these tests never
/// touch the host's real task scheduler. No mocks — per test_convention.md
/// Tier 3 constraint.
use chrono::NaiveDate;
use folioneer_lib::context::account::{
    AccountService, SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
};
use folioneer_lib::context::asset::{
    AssetService, ReqwestYahooClient, SqliteAssetCategoryRepository, SqliteAssetPriceRepository,
    SqliteAssetRepository,
};
use folioneer_lib::context::currency::{
    CurrencyService, SqliteCurrencyPairRepository, SqliteCurrencyRateRepository,
};
use folioneer_lib::shared::infrastructure::scheduler::NoopScheduler;
use folioneer_lib::use_cases::scheduled_fetch::{
    ScheduledFetchError, ScheduledFetchOrchestrator, SqliteScheduledFetchRepository,
};
use std::sync::Arc;

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

/// Builds an orchestrator wired to real in-memory SQLite and a `NoopScheduler`,
/// with "now" fixed to 2026-06-10T23:00:00 (after the default 22:15 trigger).
async fn build_orchestrator() -> ScheduledFetchOrchestrator {
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
    let price_provider = Arc::new(ReqwestYahooClient::new().expect("build yahoo client"));
    let repository = Arc::new(SqliteScheduledFetchRepository::new(pool.clone()));
    let scheduler = Arc::new(NoopScheduler);
    let now = NaiveDate::from_ymd_opt(2026, 6, 10)
        .expect("valid date")
        .and_hms_opt(23, 0, 0)
        .expect("valid time");

    ScheduledFetchOrchestrator::new(
        account_service,
        asset_service,
        price_provider,
        currency_service,
        repository,
        scheduler,
        Arc::new(move || now),
    )
}

// -------------------------------------------------------------------------
// configure_scheduled_fetch — happy path + error propagation
// -------------------------------------------------------------------------

/// SPF-011/012 — configuring the daily download end-to-end registers with the
/// (noop) OS scheduler, persists the configuration, and the persisted value
/// is readable back through `status()`.
#[tokio::test]
async fn configure_scheduled_fetch_end_to_end_persists_and_status_reflects_it() {
    let orchestrator = build_orchestrator().await;

    orchestrator
        .configure(true, "19:00".to_string())
        .await
        .expect("configure must succeed end-to-end");

    let status = orchestrator.status().await.expect("status must succeed");
    assert!(status.configuration.enabled);
    assert_eq!(status.configuration.trigger_time, "19:00");
}

/// SPF-019 — an invalid trigger time propagates as `InvalidTriggerTime`
/// through the full stack, and the stored configuration is left unchanged.
#[tokio::test]
async fn configure_scheduled_fetch_invalid_trigger_time_propagates() {
    let orchestrator = build_orchestrator().await;

    let err = orchestrator
        .configure(true, "24:00".to_string())
        .await
        .expect_err("a malformed trigger time must be rejected");
    assert!(
        matches!(err, ScheduledFetchError::InvalidTriggerTime),
        "expected InvalidTriggerTime, got: {err:?}"
    );

    let status = orchestrator.status().await.expect("status must succeed");
    assert!(
        !status.configuration.enabled,
        "the rejected configuration must not have been persisted (SPF-013)"
    );
}

// -------------------------------------------------------------------------
// get_scheduled_fetch_status — happy path
// -------------------------------------------------------------------------

/// SPF-052 — on a fresh install, status returns the migration-seeded default
/// configuration (disabled, 22:15) and `last_run = None`.
#[tokio::test]
async fn get_scheduled_fetch_status_returns_defaults_on_fresh_install() {
    let orchestrator = build_orchestrator().await;

    let status = orchestrator.status().await.expect("status must succeed");
    assert!(!status.configuration.enabled);
    assert_eq!(status.configuration.trigger_time, "22:15");
    assert!(status.last_run.is_none());
}

// -------------------------------------------------------------------------
// run_scheduled_fetch — the internal run pipeline (no Tauri command; SPF-021+)
// -------------------------------------------------------------------------

/// SPF-042/050 — with no accounts/holdings in the database, the scheduled run
/// records a quiet `Succeeded` run with zero updates and zero skips.
#[tokio::test]
async fn run_scheduled_fetch_records_a_run_for_empty_scope_end_to_end() {
    let orchestrator = build_orchestrator().await;

    let run = orchestrator
        .run_scheduled_fetch()
        .await
        .expect("a run must always be produced (SPF-050)");

    assert_eq!(run.updated_count, 0);
    assert_eq!(run.skipped_count, 0);

    let status = orchestrator.status().await.expect("status must succeed");
    assert!(
        status.last_run.is_some(),
        "the run must be persisted and visible via status (SPF-052)"
    );
}
