//! Headless entry point for the OS-triggered scheduled run (SPF-016, SPF-020).
//! Invoked by `main.rs` when the process is launched with `--scheduled-fetch`
//! (no Tauri window is ever created for this path).

use std::path::PathBuf;
use std::sync::Arc;

use crate::context::asset::PriceProvider;
use crate::context::currency::RateHistoryProvider;
use crate::context::sync::{
    FsFolderStore, SqliteChangeLogRepository, SqliteChangeRecorder, SqliteSyncStateRepository,
    SyncDevice, SyncRun, SyncStateRepository,
};
use crate::core::{Database, BACKEND};
use crate::shared::infrastructure::app_directories;
use crate::shared::infrastructure::change_recorder::ChangeRecorder;
use crate::shared::infrastructure::container::AppContainer;
use crate::shared::infrastructure::scheduler::platform_scheduler;

use super::orchestrator::ScheduledFetchOrchestrator;
use super::repository::SqliteScheduledFetchRepository;
use crate::use_cases::shared::price_fetch_log::SqlitePriceFetchLogRepository;

/// SYN-068 — after the scheduled fetch, verifies the passphrase check (SYN-055) and
/// publishes the recorded price/rate changes as one segment; never applies — merging,
/// resolving, and recomputing happen only at the next application launch (SYN-060). On a
/// passphrase-check mismatch, publishes nothing and leaves the reset for the next launch
/// (SYN-084). A no-op when sync has never been enabled or is paused on this device.
pub async fn publish_after_scheduled_fetch(device: Option<SyncDevice>, sync_run: Arc<SyncRun>) {
    let Some(device) = device.filter(|device| !device.paused) else {
        return;
    };
    match sync_run.publish(&device).await {
        Ok(report) => tracing::info!(
            target: BACKEND,
            published = report.published_changes,
            failures = ?report.failures,
            "scheduled fetch: recorded changes published"
        ),
        Err(error) => {
            tracing::warn!(target: BACKEND, err = %error, "scheduled fetch: publish failed");
        }
    }
}

/// The external data sources the headless run fetches from, built by the composition
/// root once logging is up so a client that cannot start is logged like any failure.
pub struct HeadlessProviders {
    /// Daily closes of the assets in the fetch scope, from the External provider.
    /// `None` in a build composed without one (MKT-210), where this run has nothing
    /// to do (SPF-072).
    pub price: Option<Arc<dyn PriceProvider>>,
    /// Exchange rates of the pairs in the fetch scope.
    pub rate_history: Arc<dyn RateHistoryProvider>,
}

/// Resolves the app's data directory, opens the database, wires the minimal
/// service graph, runs [`ScheduledFetchOrchestrator::run_scheduled_fetch`],
/// and returns a process exit code — `0` unless the run record itself could
/// not be written.
pub async fn run(build_providers: impl FnOnce() -> anyhow::Result<HeadlessProviders>) -> i32 {
    // SPF-072 — no External provider, nothing to fetch. Such a build removes its
    // schedule at start-up (SPF-070), so reaching here means one outlived its build and
    // fired before the next start. It is decided first: no log file, no data folder,
    // no database, no recorded run — the run answers success having done nothing.
    let fetchable = match build_providers() {
        Ok(HeadlessProviders {
            price: Some(price_provider),
            rate_history,
        }) => Ok((price_provider, rate_history)),
        Ok(HeadlessProviders { price: None, .. }) => return 0,
        Err(error) => Err(error),
    };

    // A logging failure must not abandon the fetch — the subscriber is
    // best-effort; its absence falls back to the eprintln below only.
    match app_directories::resolve_log_dir() {
        Some(log_dir) => {
            if let Err(error) = std::fs::create_dir_all(&log_dir)
                .map_err(anyhow::Error::from)
                .and_then(|()| crate::initialize_tracing(&log_dir))
            {
                eprintln!("scheduled fetch: tracing initialization failed: {error:#}");
            }
        }
        None => eprintln!("scheduled fetch: no platform log directory available"),
    }

    // A client that cannot start is reported once logging is up.
    let (price_provider, rate_history) = match fetchable {
        Ok(providers) => providers,
        Err(error) => {
            tracing::error!(target: BACKEND, err = %format!("{error:#}"), "scheduled fetch: HTTP client initialization failed");
            return 1;
        }
    };

    let Some(data_dir) = app_directories::resolve_local_data_dir() else {
        tracing::error!(target: BACKEND, "scheduled fetch: no platform data directory available");
        return 1;
    };
    let database = match Database::new(data_dir).await {
        Ok(database) => database,
        Err(error) => {
            tracing::error!(target: BACKEND, err = %format!("{error:#}"), "scheduled fetch: database initialization failed");
            return 1;
        }
    };
    let pool = database.pool;

    // No event bus: the headless run must never publish side-effect events
    // (SPF-024 — there is no window to forward them to).
    let change_recorder: Arc<dyn ChangeRecorder> =
        Arc::new(SqliteChangeRecorder::new(pool.clone()));
    let container = AppContainer::build(
        pool.clone(),
        None,
        Some(rate_history),
        None,
        Arc::clone(&change_recorder),
    );
    let sync_pool = pool.clone();
    let fetch_log = Arc::new(SqlitePriceFetchLogRepository::new(pool.clone()));
    let repository = Arc::new(SqliteScheduledFetchRepository::new(pool));

    let orchestrator = ScheduledFetchOrchestrator::new(
        container.account_service,
        container.asset_service,
        price_provider,
        container.currency_service,
        repository,
        platform_scheduler(),
        Arc::new(|| chrono::Local::now().naive_local()),
    )
    .with_fetch_log(fetch_log);

    let exit_code = match orchestrator.run_scheduled_fetch().await {
        Ok(run) => {
            tracing::info!(
                target: BACKEND,
                outcome = %run.outcome,
                updated = run.updated_count,
                skipped = run.skipped_count,
                trigger_date = %run.trigger_date,
                "scheduled fetch run completed"
            );
            0
        }
        Err(error) => {
            tracing::error!(target: BACKEND, err = %error, "scheduled fetch: run failed");
            1
        }
    };

    // SYN-068 — publish what the fetch recorded before the process exits; never apply.
    let state_repo = SqliteSyncStateRepository::new(sync_pool.clone());
    let device = match state_repo.get_device().await {
        Ok(device) => device,
        Err(error) => {
            tracing::warn!(target: BACKEND, err = %error, "scheduled fetch: sync device not loaded");
            None
        }
    };
    let sync_run = Arc::new(SyncRun::new(
        Arc::new(SqliteChangeLogRepository::new(sync_pool)),
        Arc::new(state_repo),
        Arc::new(FsFolderStore::new(PathBuf::new())),
        change_recorder,
    ));
    publish_after_scheduled_fetch(device, sync_run).await;
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    // SPF-072 — a scheduled fetch that fires in a build without an External provider
    // does nothing and answers success. The rate-history mock has no expectation, so
    // any attempt to fetch rates on the way would fail the test.
    #[tokio::test]
    async fn a_leftover_in_a_build_without_an_external_provider_does_nothing() {
        use crate::context::currency::MockRateHistoryProvider;

        let exit_code = run(|| {
            Ok(HeadlessProviders {
                price: None,
                rate_history: Arc::new(MockRateHistoryProvider::new()),
            })
        })
        .await;

        assert_eq!(exit_code, 0);
    }

    // SYN-068 — a device with sync disabled (no SyncDevice) is a no-op: nothing is
    // published, and the call never panics.
    #[tokio::test]
    async fn publish_after_scheduled_fetch_is_a_noop_when_sync_is_disabled() {
        use crate::context::sync::MockFolderStore;
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("test pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations");
        let change_log = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
        let state_repo = Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let sync_run = Arc::new(SyncRun::new(
            change_log,
            state_repo,
            Arc::new(MockFolderStore::new()),
            Arc::new(crate::shared::infrastructure::change_recorder::NoopChangeRecorder),
        ));

        // SYN-068: never applies — there is no apply path reachable from this function at
        // all, only a publish. Documented by the absence of an applier parameter above.
        publish_after_scheduled_fetch(None, sync_run).await;
    }
}
