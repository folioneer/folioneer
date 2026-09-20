//! Cross-BC orchestration for the seven sync commands that read from or write into the
//! account/asset/currency bounded contexts (D3, ADR-003/ADR-004): a first publish reads the
//! whole portfolio, a join rebuilds it, a run applies other devices' changes through the
//! owning services (`ServiceChangeApplier`), and status enriches with inconsistent holdings.
//! Injects each BC's service — never a repository, never a sibling use case (B18).

use std::sync::Arc;

use crate::context::account::AccountService;
use crate::context::asset::{AssetService, SYSTEM_CATEGORY_ID};
use crate::context::currency::CurrencyService;
use crate::context::sync::{
    ensure_device_name, ensure_passphrase_length, header_data_format_version, FirstPublish,
    FolderStore, InconsistentHolding, JoinError, SyncError, SyncFailure, SyncFolderState,
    SyncReport, SyncRun, SyncService, SyncStateRepository, SyncStatus, DATA_FORMAT_VERSION,
};
use crate::core::cash::{is_cash_asset, SYSTEM_CASH_CATEGORY_ID};
use crate::core::BACKEND;
use crate::use_cases::shared::inconsistency::holding_inconsistency;

use super::applier::ServiceChangeApplier;
use super::error::{PortfolioSyncError, PortfolioSyncTask};

impl From<JoinError> for PortfolioSyncError {
    fn from(error: JoinError) -> Self {
        match error {
            JoinError::Sync(error) => error.into(),
            JoinError::HistoryIncomplete => PortfolioSyncTask::HistoryIncomplete.into(),
            JoinError::RebuildInterrupted => PortfolioSyncTask::RebuildInterrupted.into(),
        }
    }
}

/// Everything `PortfolioSyncOrchestrator` needs: the three services it reads from and
/// writes into (ADR-004), and the sync context's own device lifecycle, first publish, run,
/// state, and folder components.
pub struct PortfolioSyncDependencies {
    /// Owner of accounts, transactions, holding notes, fee schedules, and catch-up positions.
    pub account_service: Arc<AccountService>,
    /// Owner of assets, categories, and asset prices.
    pub asset_service: Arc<AssetService>,
    /// Owner of currency pairs and rates.
    pub currency_service: Arc<CurrencyService>,
    /// This device's sync lifecycle and status (pause, resume, rename, leave).
    pub sync_service: Arc<SyncService>,
    /// Enabling as the first device (SYN-013).
    pub first_publish: Arc<FirstPublish>,
    /// One sync run: publish, read, resolve, apply (SYN-060).
    pub sync_run: Arc<SyncRun>,
    /// This device's sync state: device row, cursors, held-back changes, notices.
    pub state_repo: Arc<dyn SyncStateRepository>,
    /// The shared folder's files.
    pub folder_store: Arc<dyn FolderStore>,
}

/// Orchestrates the cross-BC sync commands (D3).
pub struct PortfolioSyncOrchestrator {
    account_service: Arc<AccountService>,
    asset_service: Arc<AssetService>,
    currency_service: Arc<CurrencyService>,
    sync_service: Arc<SyncService>,
    first_publish: Arc<FirstPublish>,
    sync_run: Arc<SyncRun>,
    state_repo: Arc<dyn SyncStateRepository>,
    folder_store: Arc<dyn FolderStore>,
    applier: ServiceChangeApplier,
}

impl PortfolioSyncOrchestrator {
    /// Creates the orchestrator from every component it may read from or write into.
    pub fn new(dependencies: PortfolioSyncDependencies) -> Self {
        let PortfolioSyncDependencies {
            account_service,
            asset_service,
            currency_service,
            sync_service,
            first_publish,
            sync_run,
            state_repo,
            folder_store,
        } = dependencies;
        let applier = ServiceChangeApplier::new(
            Arc::clone(&account_service),
            Arc::clone(&asset_service),
            Arc::clone(&currency_service),
        );
        Self {
            account_service,
            asset_service,
            currency_service,
            sync_service,
            first_publish,
            sync_run,
            state_repo,
            folder_store,
            applier,
        }
    }

    /// Pre-flight read of a candidate folder (SYN-011/014/019). Never rejects.
    pub async fn inspect_sync_folder(
        &self,
        folder: String,
    ) -> Result<SyncFolderState, PortfolioSyncError> {
        self.folder_store.retarget(&folder);
        let mut problem = self.folder_store.check_available().await.err();
        let header = match problem {
            Some(_) => None,
            None => match self.folder_store.read_header_bytes().await {
                Ok(header) => header,
                Err(SyncError::FolderUnavailable { problem: found }) => {
                    problem = Some(found);
                    None
                }
                Err(error) => return Err(error.into()),
            },
        };
        let holds_portfolio = header.is_some();
        let data_format_version = header.as_deref().and_then(header_data_format_version);
        Ok(SyncFolderState {
            problem,
            holds_portfolio,
            data_format_version,
            format_readable: !holds_portfolio
                || data_format_version.is_some_and(|version| version <= DATA_FORMAT_VERSION),
            installation_holds_user_data: self.installation_holds_user_data().await?,
        })
    }

    /// Whether this installation holds user-entered records (SYN-014): any account, any
    /// user-created asset or category, any currency pair. System-seeded records (SYN-027)
    /// do not count.
    async fn installation_holds_user_data(&self) -> Result<bool, PortfolioSyncError> {
        if !self.account_service.get_all().await?.is_empty() {
            return Ok(true);
        }
        let assets = self.asset_service.get_all_assets_with_archived().await?;
        if assets.iter().any(|asset| !is_cash_asset(&asset.id)) {
            return Ok(true);
        }
        let categories = self.asset_service.get_all_categories().await?;
        if categories.iter().any(|category| {
            category.id != SYSTEM_CATEGORY_ID && category.id != SYSTEM_CASH_CATEGORY_ID
        }) {
            return Ok(true);
        }
        Ok(!self
            .currency_service
            .list_currency_pairs()
            .await?
            .is_empty())
    }

    /// Enables sync (SYN-011). The first-device branch (no header yet) delegates to
    /// `FirstPublish`; the join branch (a header already exists) rebuilds the portfolio
    /// from the folder's history (SYN-014/036/080) — only on an installation that holds no
    /// user data (`InstallationHoldsUserData`).
    pub async fn enable_sync(
        &self,
        folder: String,
        passphrase: String,
        device_name: String,
    ) -> Result<SyncStatus, PortfolioSyncError> {
        if self.state_repo.get_device().await?.is_some() {
            return Err(SyncError::AlreadyEnabled.into());
        }
        self.folder_store.retarget(&folder);
        if self.folder_store.read_header_bytes().await?.is_none() {
            return Ok(self
                .first_publish
                .enable_as_first_device(folder, passphrase, device_name)
                .await?);
        }
        if self.installation_holds_user_data().await? {
            return Err(PortfolioSyncTask::InstallationHoldsUserData.into());
        }
        let status = self
            .sync_run
            .join(&self.applier, folder, passphrase, device_name)
            .await?;
        self.sync_service.announce_join();
        Ok(status)
    }

    /// Re-enrolls this device as the new origin of the portfolio under a new passphrase,
    /// clearing the folder first (SYN-071).
    pub async fn start_sync_over(
        &self,
        folder: String,
        passphrase: String,
        device_name: String,
    ) -> Result<SyncStatus, PortfolioSyncError> {
        ensure_passphrase_length(&passphrase)?;
        ensure_device_name(&device_name)?;
        self.folder_store.retarget(&folder);
        self.folder_store
            .check_available()
            .await
            .map_err(|problem| SyncError::FolderUnavailable { problem })?;
        for device_id in self.folder_store.list_device_ids().await? {
            self.folder_store.remove_device_area(&device_id).await?;
        }
        self.folder_store.remove_header().await?;
        Ok(self
            .first_publish
            .enable_as_first_device(folder, passphrase, device_name)
            .await?)
    }

    /// Designates a different folder for an already-enrolled device (SYN-074): the same
    /// portfolio, or an empty folder (first-device path).
    pub async fn change_sync_folder(
        &self,
        folder: String,
    ) -> Result<SyncStatus, PortfolioSyncError> {
        let device = self
            .state_repo
            .get_device()
            .await?
            .ok_or(SyncError::SyncDisabled)?;
        Ok(self.first_publish.change_folder(device, folder).await?)
    }

    /// Runs a full sync immediately (SYN-061): publish, then apply the other devices'
    /// changes through the owning services.
    pub async fn sync_now(&self) -> Result<SyncReport, PortfolioSyncError> {
        let device = self
            .state_repo
            .get_device()
            .await?
            .ok_or(SyncError::SyncDisabled)?;
        device.ensure_not_paused()?;
        let report = self.sync_run.run(&device, &self.applier).await?;
        self.sync_service.remember_run(&report);
        Ok(report)
    }

    /// The automatic sync a settled burst of recorded changes fires (SYN-060/067): a full run
    /// on an enabled, non-paused device. Never fails outward — a failed run is logged, and
    /// the next recorded change retries.
    pub async fn sync_after_changes(&self) {
        match self.sync_now().await {
            Ok(_) => {}
            Err(PortfolioSyncError::Sync(SyncError::SyncDisabled | SyncError::SyncPaused)) => {}
            Err(error) => {
                tracing::warn!(target: BACKEND, err = %error, "automatic sync: run failed");
            }
        }
    }

    /// Resumes sync on a paused device: publishes paused-era changes, then runs as `sync_now`
    /// (SYN-073). The device stays paused while the folder is unavailable or the publish
    /// fails, and after a detected reset (SYN-084).
    pub async fn resume_sync(&self) -> Result<SyncReport, PortfolioSyncError> {
        let device = self.sync_service.resume_sync_precondition().await?;
        self.folder_store.retarget(&device.folder);
        self.folder_store
            .check_available()
            .await
            .map_err(|problem| SyncError::FolderUnavailable { problem })?;
        let resumed = device.resume()?;
        let report = self.sync_run.run(&resumed, &self.applier).await?;
        if let Some(problem) = report.failures.iter().find_map(|failure| match failure {
            SyncFailure::FolderUnavailable { problem } => Some(*problem),
            _ => None,
        }) {
            return Err(SyncError::PublishFailed { problem }.into());
        }
        if !report.failures.contains(&SyncFailure::PortfolioReset) {
            self.state_repo.save_device(&resumed).await?;
        }
        self.sync_service.remember_run(&report);
        Ok(report)
    }

    /// Reads the current sync status, enriched with the holdings whose replayed ledger
    /// breaks an invariant (CFR-042/SYN-040) — a cross-BC read of the account BC's holdings
    /// and the asset BC's names.
    pub async fn get_sync_status(&self) -> Result<SyncStatus, PortfolioSyncError> {
        let mut status = self.sync_service.status().await?;
        status.inconsistent_holdings = self.inconsistent_holdings().await?;
        Ok(status)
    }

    async fn inconsistent_holdings(&self) -> Result<Vec<InconsistentHolding>, PortfolioSyncError> {
        let mut inconsistent = Vec::new();
        for account in self.account_service.get_all().await? {
            for holding in self
                .account_service
                .get_holdings_for_account(&account.id)
                .await?
            {
                let Some(reason) = holding_inconsistency(&holding) else {
                    continue;
                };
                let asset_name = self
                    .asset_service
                    .get_asset_by_id(&holding.asset_id)
                    .await?
                    .map_or_else(|| holding.asset_id.clone(), |asset| asset.name);
                inconsistent.push(InconsistentHolding {
                    account_id: account.id.clone(),
                    account_name: account.name.clone(),
                    asset_id: holding.asset_id,
                    asset_name,
                    reason,
                });
            }
        }
        Ok(inconsistent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::account::{
        SqliteAccountRepository, SqliteHoldingRepository, SqliteTransactionRepository,
        UpdateFrequency,
    };
    use crate::context::asset::{
        SqliteAssetCategoryRepository, SqliteAssetPriceRepository, SqliteAssetRepository,
    };
    use crate::context::currency::{SqliteCurrencyPairRepository, SqliteCurrencyRateRepository};
    use crate::context::sync::infrastructure::codec::{
        encode_header, encode_manifest, encode_segment,
    };
    use crate::context::sync::infrastructure::crypto::{derive_key, make_check};
    use crate::context::sync::{
        segment_file_name, DerivationParameters, FolderHeader, FolderProblem, Manifest,
        MockFolderStore, MockSyncStateRepository, Segment, SegmentChange,
        SqliteChangeLogRepository, SqliteSyncStateRepository, StoredDevice, SyncDevice,
        APP_VERSION,
    };
    use crate::shared::domain::{Operation, Origin, RecordKind};
    use crate::use_cases::portfolio_sync::{ServicePortfolioSnapshot, ServiceRankStamper};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn make_pool() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = SqlitePoolOptions::new()
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
    }

    fn build_ctx_with_state_repo(
        pool: &sqlx::Pool<sqlx::Sqlite>,
        state_repo: Arc<dyn SyncStateRepository>,
        folder_store: Arc<dyn FolderStore>,
    ) -> Ctx {
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
        let change_log = Arc::new(SqliteChangeLogRepository::new(pool.clone()));
        let sync_run = Arc::new(SyncRun::new(
            change_log.clone(),
            Arc::clone(&state_repo),
            Arc::clone(&folder_store),
            Arc::new(crate::shared::infrastructure::change_recorder::NoopChangeRecorder),
        ));
        let sync_service = Arc::new(
            SyncService::new(Arc::clone(&state_repo), Arc::clone(&folder_store))
                .with_run(Arc::clone(&sync_run)),
        );
        let snapshot = Arc::new(ServicePortfolioSnapshot::new(
            Arc::clone(&account_service),
            Arc::clone(&asset_service),
            Arc::clone(&currency_service),
        ));
        let rank_stamper = Arc::new(ServiceRankStamper::new(
            Arc::clone(&account_service),
            Arc::clone(&asset_service),
            Arc::clone(&currency_service),
        ));
        let first_publish = Arc::new(FirstPublish::new(
            change_log,
            Arc::clone(&state_repo),
            Arc::clone(&folder_store),
            rank_stamper,
            snapshot,
        ));
        let orchestrator = PortfolioSyncOrchestrator::new(PortfolioSyncDependencies {
            account_service: Arc::clone(&account_service),
            asset_service: Arc::clone(&asset_service),
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
        }
    }

    fn device() -> SyncDevice {
        SyncDevice::restore(StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    // SYN-014 — a candidate folder already holding user data on THIS installation is flagged,
    // regardless of the folder's own content.
    #[tokio::test]
    async fn inspect_sync_folder_flags_installation_holds_user_data_when_local_accounts_exist() {
        let pool = make_pool().await;
        let account_service = AccountService::new(
            Box::new(SqliteAccountRepository::new(pool.clone())),
            Box::new(SqliteHoldingRepository::new(pool.clone())),
            Box::new(SqliteTransactionRepository::new(pool.clone())),
        );
        account_service
            .create(
                "Portfolio".into(),
                String::new(),
                "USD".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(SqliteSyncStateRepository::new(pool.clone())),
            Arc::new(crate::context::sync::FsFolderStore::new(dir.path())),
        );

        let state = ctx
            .orchestrator
            .inspect_sync_folder(dir.path().to_string_lossy().to_string())
            .await
            .expect("inspect_sync_folder never rejects (SYN-011/014/019)");
        assert!(
            state.installation_holds_user_data,
            "SYN-014: an existing local account must be flagged"
        );
        assert!(state.problem.is_none());
        assert!(!state.holds_portfolio);
        assert!(state.format_readable);
    }

    // SYN-019 — a fresh installation and a missing folder: the problem is reported, never
    // thrown.
    #[tokio::test]
    async fn inspect_sync_folder_reports_a_missing_folder_on_a_fresh_installation() {
        let pool = make_pool().await;
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(SqliteSyncStateRepository::new(pool.clone())),
            Arc::new(crate::context::sync::FsFolderStore::new(&missing)),
        );

        let state = ctx
            .orchestrator
            .inspect_sync_folder(missing.to_string_lossy().to_string())
            .await
            .unwrap();
        assert_eq!(state.problem, Some(FolderProblem::Missing));
        assert!(!state.installation_holds_user_data);
    }

    // SYN-019/035 — a folder holding a portfolio in a newer data format is reported as
    // unreadable for joining.
    #[tokio::test]
    async fn inspect_sync_folder_reports_a_too_new_portfolio_as_unreadable() {
        let pool = make_pool().await;
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(b"{\"data_format_version\":99}".to_vec())));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(SqliteSyncStateRepository::new(pool.clone())),
            Arc::new(folder_store),
        );

        let state = ctx
            .orchestrator
            .inspect_sync_folder("/tmp/sync".into())
            .await
            .unwrap();
        assert!(state.holds_portfolio);
        assert_eq!(state.data_format_version, Some(99));
        assert!(!state.format_readable);
    }

    // SYN-010 — enable_sync rejects AlreadyEnabled when a sync_device row already exists.
    #[tokio::test]
    async fn enable_sync_rejects_already_enabled() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(device())));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let result = ctx
            .orchestrator
            .enable_sync(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "Desktop".into(),
            )
            .await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::AlreadyEnabled))
        ));
    }

    // SYN-014 — the join branch (a header already exists) rejects an installation that
    // holds user data before reading anything else from the folder.
    #[tokio::test]
    async fn enable_sync_join_branch_rejects_an_installation_holding_user_data() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_read_header_bytes()
            .returning(|| Ok(Some(b"{\"data_format_version\":1}".to_vec())));
        let ctx = build_ctx_with_state_repo(&pool, Arc::new(state_repo), Arc::new(folder_store));
        ctx.account_service
            .create(
                "Mine".into(),
                String::new(),
                "EUR".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();

        let result = ctx
            .orchestrator
            .enable_sync(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                "Desktop".into(),
            )
            .await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Task(
                PortfolioSyncTask::InstallationHoldsUserData
            ))
        ));
    }

    // SYN-010 — sync_now rejects SyncDisabled while never enabled.
    #[tokio::test]
    async fn sync_now_rejects_when_disabled() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let result = ctx.orchestrator.sync_now().await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::SyncDisabled))
        ));
    }

    // SYN-070 — sync_now rejects SyncPaused on a paused device.
    #[tokio::test]
    async fn sync_now_rejects_when_paused() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        let mut paused = device();
        paused.paused = true;
        state_repo
            .expect_get_device()
            .returning(move || Ok(Some(paused.clone())));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let result = ctx.orchestrator.sync_now().await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::SyncPaused))
        ));
    }

    // SYN-073/069 — resume_sync rejects FolderUnavailable while the folder is unavailable, and
    // the device stays paused.
    #[tokio::test]
    async fn resume_sync_rejects_folder_unavailable_and_keeps_the_device_paused() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        let mut paused = device();
        paused.paused = true;
        state_repo
            .expect_get_device()
            .returning(move || Ok(Some(paused.clone())));
        state_repo.expect_save_device().times(0);
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_check_available()
            .returning(|| Err(FolderProblem::Unmounted));
        let ctx = build_ctx_with_state_repo(&pool, Arc::new(state_repo), Arc::new(folder_store));

        let result = ctx.orchestrator.resume_sync().await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::FolderUnavailable {
                problem: FolderProblem::Unmounted
            }))
        ));
    }

    // SYN-073 — resume_sync rejects NotPaused on an active device.
    #[tokio::test]
    async fn resume_sync_rejects_when_not_paused() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(device())));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let result = ctx.orchestrator.resume_sync().await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::NotPaused))
        ));
    }

    // SYN-012/018 — start_sync_over validates its inputs before clearing anything.
    #[tokio::test]
    async fn start_sync_over_validates_inputs_before_clearing_the_folder() {
        let pool = make_pool().await;
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(MockSyncStateRepository::new()),
            Arc::new(MockFolderStore::new()),
        );

        let short = ctx
            .orchestrator
            .start_sync_over("/tmp/sync".into(), "short".into(), "Desktop".into())
            .await;
        assert!(matches!(
            short,
            Err(PortfolioSyncError::Sync(SyncError::PassphraseTooShort {
                minimum: 12
            }))
        ));
        let blank = ctx
            .orchestrator
            .start_sync_over(
                "/tmp/sync".into(),
                "correct horse battery staple".into(),
                " ".into(),
            )
            .await;
        assert!(matches!(
            blank,
            Err(PortfolioSyncError::Sync(SyncError::DeviceNameBlank))
        ));
    }

    // SYN-010 — change_sync_folder rejects SyncDisabled while never enabled.
    #[tokio::test]
    async fn change_sync_folder_rejects_when_disabled() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let result = ctx
            .orchestrator
            .change_sync_folder("/tmp/elsewhere".into())
            .await;
        assert!(matches!(
            result,
            Err(PortfolioSyncError::Sync(SyncError::SyncDisabled))
        ));
    }

    // SYN-063 — get_sync_status delegates to SyncService and returns the disabled shape.
    #[tokio::test]
    async fn get_sync_status_returns_disabled_shape_when_never_enabled() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );

        let status = ctx
            .orchestrator
            .get_sync_status()
            .await
            .expect("must not error");
        assert!(!status.enabled);
    }

    // -------------------------------------------------------------------------
    // SYN-014/015/036/080 — the join / rebuild branch of enable_sync
    // -------------------------------------------------------------------------

    const PASSPHRASE: &str = "correct horse battery staple";

    fn empty_install_state_repo() -> MockSyncStateRepository {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        state_repo
    }

    /// The portfolio Desktop published: its header (sealed check under the key the
    /// passphrase derives), a manifest at sequence 1, and one segment — the bytes a joining
    /// device reads from the folder.
    struct PublishedPortfolio {
        header: Vec<u8>,
        manifest: Vec<u8>,
        segment: Vec<u8>,
        segment_name: String,
    }

    fn published_portfolio(
        changes: Vec<SegmentChange>,
        latest_sequence: i64,
    ) -> PublishedPortfolio {
        let derivation_parameters = DerivationParameters {
            salt: vec![7; 16],
            memory_cost_kib: 19_456,
            iterations: 2,
            parallelism: 1,
        };
        let key = derive_key(PASSPHRASE, &derivation_parameters).expect("key derives");
        let header = encode_header(&FolderHeader {
            derivation_parameters,
            passphrase_check: make_check(&key),
            data_format_version: DATA_FORMAT_VERSION,
            created_at: format!("{:020}", 1),
            created_by_device_id: "desktop-device".into(),
        })
        .expect("header encodes");
        let manifest = encode_manifest(
            &key,
            &Manifest {
                device_id: "desktop-device".into(),
                device_name: "Desktop".into(),
                data_format_version: DATA_FORMAT_VERSION,
                app_version: Some(APP_VERSION.to_string()),
                latest_sequence,
            },
        )
        .expect("manifest encodes");
        let changes_count = changes.len() as i64;
        let segment = encode_segment(
            &key,
            &Segment {
                device_id: "desktop-device".into(),
                first_sequence: 1,
                last_sequence: changes_count,
                data_format_version: DATA_FORMAT_VERSION,
                changes,
            },
        )
        .expect("segment encodes");
        PublishedPortfolio {
            header,
            manifest,
            segment,
            segment_name: segment_file_name(1, changes_count),
        }
    }

    fn desktop_change(
        sequence: i64,
        record_kind: RecordKind,
        identity: &str,
        content: &str,
    ) -> SegmentChange {
        SegmentChange {
            sequence,
            logical_timestamp: format!("{:020}", 1),
            based_on: None,
            record_kind,
            record_identity: identity.into(),
            operation: Operation::Created,
            origin: Origin::User,
            content: Some(content.into()),
        }
    }

    /// The folder as the joiner sees it: Desktop's area, and room for the joiner's manifest.
    fn folder_holding(published: PublishedPortfolio) -> MockFolderStore {
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store.expect_check_available().returning(|| Ok(()));
        let header = published.header;
        folder_store
            .expect_read_header_bytes()
            .returning(move || Ok(Some(header.clone())));
        folder_store
            .expect_list_device_ids()
            .returning(|| Ok(vec!["desktop-device".into()]));
        let manifest = published.manifest;
        folder_store
            .expect_read_manifest_bytes()
            .returning(move |_| Ok(Some(manifest.clone())));
        let segment_names = vec![published.segment_name];
        folder_store
            .expect_list_segment_names()
            .returning(move |_| Ok(segment_names.clone()));
        let segment = published.segment;
        folder_store
            .expect_read_segment_bytes()
            .returning(move |_, _| Ok(Some(segment.clone())));
        folder_store
            .expect_write_manifest()
            .returning(|_, _| Ok(()));
        folder_store
            .expect_remove_device_area()
            .returning(|_| Ok(()));
        folder_store
    }

    fn account_content(id: &str, name: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"{name}","bank_name":"","currency":"EUR","update_frequency":"ManualMonth","management_fees_enabled":false}}"#
        )
    }

    // SYN-014/036 — a fresh installation joining an existing portfolio rebuilds it by
    // replaying every published change: the account Desktop published exists locally, the
    // device row is written, and the cursor on Desktop stands at its latest sequence.
    #[tokio::test]
    async fn enable_sync_join_branch_rebuilds_when_installation_holds_no_user_data() {
        let pool = make_pool().await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let published = published_portfolio(
            vec![desktop_change(
                1,
                RecordKind::Account,
                "account-desktop",
                &account_content("account-desktop", "Livret A"),
            )],
            1,
        );
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::clone(&state_repo),
            Arc::new(folder_holding(published)),
        );

        let result = ctx
            .orchestrator
            .enable_sync("/tmp/sync".into(), PASSPHRASE.into(), "Laptop".into())
            .await;
        assert!(
            matches!(&result, Ok(status) if status.enabled),
            "SYN-014/036: a fresh installation must rebuild and succeed, got {result:?}"
        );
        let account = ctx
            .account_service
            .get_by_id("account-desktop")
            .await
            .unwrap()
            .expect("SYN-036: the replayed account must exist locally");
        assert_eq!(account.name, "Livret A");
        assert_eq!(
            state_repo
                .get_cursor("desktop-device")
                .await
                .unwrap()
                .map(|cursor| cursor.applied_through),
            Some(1),
            "SYN-033: the cursor on Desktop must stand at its latest sequence"
        );
    }

    // SYN-015 — a passphrase that does not match the portfolio's is rejected before
    // anything is read or rebuilt: the state repository is never written.
    #[tokio::test]
    async fn enable_sync_join_branch_rejects_passphrase_mismatch_before_rebuilding() {
        let pool = make_pool().await;
        let published = published_portfolio(vec![], 0);
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(empty_install_state_repo()),
            Arc::new(folder_holding(published)),
        );

        let result = ctx
            .orchestrator
            .enable_sync(
                "/tmp/sync".into(),
                "a wrong passphrase, still long enough".into(),
                "Laptop".into(),
            )
            .await;
        assert!(
            matches!(
                result,
                Err(PortfolioSyncError::Sync(SyncError::PassphraseMismatch))
            ),
            "SYN-015: a wrong passphrase must be rejected before any rebuild, got {result:?}"
        );
    }

    // SYN-036 — a manifest announcing more history than the segments carry rejects with
    // HistoryIncomplete: a join never skips a file the way a steady-state sync does.
    #[tokio::test]
    async fn enable_sync_join_branch_returns_history_incomplete_on_unreadable_segment() {
        let pool = make_pool().await;
        let published = published_portfolio(
            vec![desktop_change(
                1,
                RecordKind::Account,
                "account-desktop",
                &account_content("account-desktop", "Livret A"),
            )],
            2,
        );
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(empty_install_state_repo()),
            Arc::new(folder_holding(published)),
        );

        let result = ctx
            .orchestrator
            .enable_sync("/tmp/sync".into(), PASSPHRASE.into(), "Laptop".into())
            .await;
        assert!(
            matches!(
                result,
                Err(PortfolioSyncError::Task(
                    PortfolioSyncTask::HistoryIncomplete
                ))
            ),
            "SYN-036: an unreadable segment in the replay set must be HistoryIncomplete, \
             got {result:?}"
        );
    }

    // SYN-080 — a rebuild interrupted partway (a change whose content this build cannot
    // write) leaves the device exactly as before: no sync_device row, no account, retriable.
    #[tokio::test]
    async fn enable_sync_join_branch_rolls_back_after_a_rebuild_interruption() {
        let pool = make_pool().await;
        let state_repo: Arc<dyn SyncStateRepository> =
            Arc::new(SqliteSyncStateRepository::new(pool.clone()));
        let published = published_portfolio(
            vec![
                desktop_change(
                    1,
                    RecordKind::Account,
                    "account-desktop",
                    &account_content("account-desktop", "Livret A"),
                ),
                // Passes the SYN-034 shape check (its identity is its own `id`) but cannot be
                // written — the required fields are missing — so the rebuild is interrupted.
                desktop_change(
                    2,
                    RecordKind::Account,
                    "account-broken",
                    r#"{"id":"account-broken","name":"Broken"}"#,
                ),
            ],
            2,
        );
        let ctx = build_ctx_with_state_repo(&pool, state_repo, Arc::new(folder_holding(published)));

        let result = ctx
            .orchestrator
            .enable_sync("/tmp/sync".into(), PASSPHRASE.into(), "Laptop".into())
            .await;
        assert!(
            matches!(
                result,
                Err(PortfolioSyncError::Task(
                    PortfolioSyncTask::RebuildInterrupted
                ))
            ),
            "SYN-080: an interrupted rebuild must be reported as RebuildInterrupted, \
             got {result:?}"
        );
        let device_row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_device")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            device_row_count, 0,
            "SYN-080: an interrupted rebuild must leave no sync_device row"
        );
        assert!(
            ctx.account_service
                .get_by_id("account-desktop")
                .await
                .unwrap()
                .is_none(),
            "SYN-080: the change applied before the interruption must be rolled back"
        );
    }

    // -------------------------------------------------------------------------
    // get_sync_status — cross-BC enrichment with inconsistent holdings (CFR-042/SYN-040)
    // -------------------------------------------------------------------------

    // get_sync_status must enrich SyncStatus.inconsistent_holdings from the account BC's
    // replayed ledger — a cross-BC read, like account_details — not always report empty.
    #[tokio::test]
    async fn get_sync_status_enriches_inconsistent_holdings_from_the_account_bc() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(device())));
        state_repo
            .expect_list_undismissed_notices()
            .returning(|| Ok(vec![]));
        state_repo.expect_list_held_back().returning(|| Ok(vec![]));
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        );
        let account = ctx
            .account_service
            .create(
                "Inconsistent".into(),
                String::new(),
                "EUR".into(),
                UpdateFrequency::ManualMonth,
                false,
            )
            .await
            .unwrap();
        ctx.asset_service.seed_cash_asset("EUR").await.unwrap();
        ctx.account_service
            .seed_cash_holding(&account.id)
            .await
            .unwrap();
        let cash_asset_id = crate::core::cash::system_cash_asset_id("EUR");
        sqlx::query(
            "UPDATE holdings SET quantity = -5000000 WHERE account_id = ? AND asset_id = ?",
        )
        .bind(&account.id)
        .bind(&cash_asset_id)
        .execute(&pool)
        .await
        .unwrap();

        let status = ctx.orchestrator.get_sync_status().await.unwrap();
        assert!(
            !status.inconsistent_holdings.is_empty(),
            "CFR-042/SYN-040: an overdrawn cash holding must be enriched into \
             get_sync_status().inconsistent_holdings"
        );
    }

    // SYN-060/067 — the automatic run is a silent no-op while sync is disabled or paused
    // (the folder store has no expectations: any call would panic).
    #[tokio::test]
    async fn sync_after_changes_is_a_no_op_while_disabled_or_paused() {
        let pool = make_pool().await;
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let mut paused = MockSyncStateRepository::new();
        paused
            .expect_get_device()
            .returning(|| Ok(Some(device().pause().unwrap())));

        build_ctx_with_state_repo(
            &pool,
            Arc::new(state_repo),
            Arc::new(MockFolderStore::new()),
        )
        .orchestrator
        .sync_after_changes()
        .await;
        build_ctx_with_state_repo(&pool, Arc::new(paused), Arc::new(MockFolderStore::new()))
            .orchestrator
            .sync_after_changes()
            .await;
    }

    // SYN-083 — a joining installation's own observations (prices, currency pairs and
    // rates) are discarded before the shared history replaces them; its records stay.
    #[tokio::test]
    async fn discard_observations_drops_prices_pairs_and_rates_but_keeps_records() {
        let pool = make_pool().await;
        let ctx = build_ctx_with_state_repo(
            &pool,
            Arc::new(SqliteSyncStateRepository::new(pool.clone())),
            Arc::new(MockFolderStore::new()),
        );
        let asset = ctx
            .asset_service
            .create_asset(crate::context::asset::CreateAssetDTO {
                name: "AAPL".into(),
                reference: "AAPL".into(),
                isin: None,
                class: crate::context::asset::AssetClass::Stocks,
                currency: "USD".into(),
                risk_level: 2,
                category_id: SYSTEM_CATEGORY_ID.into(),
                exchange: None,
                interest_bearing: false,
            })
            .await
            .unwrap();
        ctx.asset_service
            .record_asset_price(&asset.id, "2026-08-20", 150.0)
            .await
            .unwrap();
        ctx.orchestrator
            .currency_service
            .declare_currency_pair("USD".into(), "EUR".into())
            .await
            .unwrap();
        ctx.orchestrator
            .currency_service
            .record_currency_rate("USD".into(), "EUR".into(), "2026-08-20".into(), 920_000)
            .await
            .unwrap();

        let mut conn = pool.acquire().await.unwrap();
        crate::context::sync::ChangeApplier::discard_observations(
            &ctx.orchestrator.applier,
            &mut conn,
        )
        .await
        .expect("discard succeeds");
        drop(conn);

        assert!(ctx
            .asset_service
            .get_latest_price(&asset.id)
            .await
            .unwrap()
            .is_none());
        assert!(ctx
            .orchestrator
            .currency_service
            .list_currency_pairs()
            .await
            .unwrap()
            .is_empty());
        assert!(
            ctx.asset_service
                .get_asset_by_id(&asset.id)
                .await
                .unwrap()
                .is_some(),
            "the asset record itself is not an observation"
        );
    }
}
