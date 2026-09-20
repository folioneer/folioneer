//! `SyncService` — device lifecycle (pause/rename/leave), notice dismissal, and the sync-owned
//! half of `SyncStatus` assembly (D2). The cross-BC commands (`enable_sync`, `sync_now`, …)
//! live in `use_cases::portfolio_sync`; this service only ever touches sync-owned state.

use std::sync::{Arc, Mutex, PoisonError};

use crate::context::sync::application::run::SyncRun;
use crate::context::sync::domain::{
    FolderStore, RosterEntry, SyncDevice, SyncFailure, SyncReport, SyncStateRepository, SyncStatus,
};
use crate::context::sync::error::SyncError;
use crate::core::{Event, SideEffectEventBus, BACKEND};

/// What the last run of this process left behind for `SyncStatus` (SYN-063): when it
/// completed, what needs attention, and the roster it read.
#[derive(Clone)]
struct LastRun {
    completed_at: String,
    failures: Vec<SyncFailure>,
    roster: Vec<RosterEntry>,
    paused: bool,
}

/// Device lifecycle + status assembly for the sync bounded context.
pub struct SyncService {
    state_repo: Arc<dyn SyncStateRepository>,
    folder_store: Arc<dyn FolderStore>,
    sync_run: Option<Arc<SyncRun>>,
    event_bus: Option<Arc<SideEffectEventBus>>,
    last_run: Mutex<Option<LastRun>>,
}

impl SyncService {
    /// Creates the service bound to the given sync state and folder.
    pub fn new(
        state_repo: Arc<dyn SyncStateRepository>,
        folder_store: Arc<dyn FolderStore>,
    ) -> Self {
        Self {
            state_repo,
            folder_store,
            sync_run: None,
            event_bus: None,
            last_run: Mutex::new(None),
        }
    }

    /// Attaches the bus `SyncCompleted` is raised on (SYN-064).
    pub fn with_event_bus(mut self, event_bus: Arc<SideEffectEventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Attaches the publish run `leave_sync` flushes unpublished changes through (SYN-082).
    pub fn with_run(mut self, sync_run: Arc<SyncRun>) -> Self {
        self.sync_run = Some(sync_run);
        self
    }

    /// The attached run, or `DatabaseError` when none was wired (a wiring bug).
    fn sync_run(&self) -> Result<&SyncRun, SyncError> {
        self.sync_run.as_deref().ok_or_else(|| {
            tracing::error!(target: BACKEND, "sync_run not wired on SyncService");
            SyncError::DatabaseError
        })
    }

    /// Loads the device, rejecting `SyncDisabled` when sync has never been enabled.
    async fn require_device(&self) -> Result<SyncDevice, SyncError> {
        self.state_repo
            .get_device()
            .await?
            .ok_or(SyncError::SyncDisabled)
    }

    /// Keeps the run for `SyncStatus` (SYN-063/069) and raises `SyncCompleted` when the run applied at
    /// least one change or left different failures or paused state than the previous one
    /// (SYN-064) — so a reset, a format gate, or an unavailable folder reaches the shell
    /// without polling.
    pub fn remember_run(&self, report: &SyncReport) {
        let run = LastRun {
            completed_at: report.completed_at.clone(),
            failures: report.failures.clone(),
            roster: report.status.roster.clone(),
            paused: report.status.paused,
        };
        let previous = self
            .last_run
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(run);
        let changed = previous.as_ref().is_none_or(|previous| {
            previous.failures != report.failures || previous.paused != report.status.paused
        });
        if report.applied_changes > 0 || changed {
            if let Some(bus) = &self.event_bus {
                bus.publish(Event::SyncCompleted);
            }
        }
    }

    /// Raises `SyncCompleted` for a join (SYN-064): the rebuilt portfolio reaches every
    /// view as one event, like a run that applied changes.
    pub fn announce_join(&self) {
        if let Some(bus) = &self.event_bus {
            bus.publish(Event::SyncCompleted);
        }
    }

    fn last_run(&self) -> Option<LastRun> {
        self.last_run
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn forget_runs(&self) {
        *self.last_run.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    fn status_of(&self, device: &SyncDevice) -> SyncStatus {
        let last_run = self.last_run();
        let mut status = SyncStatus::for_device(
            device,
            last_run.as_ref().map(|run| run.completed_at.clone()),
            last_run
                .as_ref()
                .map(|run| run.failures.clone())
                .unwrap_or_default(),
        );
        status.roster = last_run.map(|run| run.roster).unwrap_or_default();
        status
    }

    /// Pauses sync on this device (SYN-070). `SyncDisabled` while never enabled;
    /// `AlreadyPaused` when already paused.
    pub async fn pause_sync(&self) -> Result<SyncStatus, SyncError> {
        let paused = self.require_device().await?.pause()?;
        self.state_repo.save_device(&paused).await?;
        Ok(self.status_of(&paused))
    }

    /// The precondition half of `resume_sync` (SYN-073): `SyncDisabled` while never enabled;
    /// `NotPaused` while not paused. Returns the loaded device so the use case can continue
    /// with publishing paused-era changes and running a sync.
    pub async fn resume_sync_precondition(&self) -> Result<SyncDevice, SyncError> {
        let device = self.require_device().await?;
        if !device.paused {
            return Err(SyncError::NotPaused);
        }
        Ok(device)
    }

    /// Renames this device (SYN-072). `DeviceNameBlank` on a blank name; the manifest is
    /// republished at the next sync, not here.
    pub async fn rename_sync_device(&self, device_name: String) -> Result<SyncStatus, SyncError> {
        let renamed = self.require_device().await?.rename(device_name)?;
        self.state_repo.save_device(&renamed).await?;
        Ok(self.status_of(&renamed))
    }

    /// Leaves sync on this device for good (SYN-082): publishes unpublished changes, removes
    /// its own manifest, drops device state (identity, cursors, held-back, notices, key), and
    /// leaves the local portfolio and its folder area untouched. Rejects `FolderUnavailable`
    /// so unpublished work is never abandoned. A device that has detected a reset (SYN-084)
    /// skips the folder entirely: nothing is ever written under the old key.
    pub async fn leave_sync(&self) -> Result<(), SyncError> {
        let device = self.require_device().await?;
        let reset_detected = self
            .last_run()
            .is_some_and(|run| run.failures.contains(&SyncFailure::PortfolioReset));
        if !reset_detected {
            self.folder_store.retarget(&device.folder);
            self.folder_store
                .check_available()
                .await
                .map_err(|problem| SyncError::FolderUnavailable { problem })?;
            let report = self.sync_run()?.publish(&device).await?;
            self.remember_run(&report);
            if let Some(problem) = report.failures.iter().find_map(|failure| match failure {
                SyncFailure::FolderUnavailable { problem } => Some(*problem),
                _ => None,
            }) {
                return Err(SyncError::PublishFailed { problem });
            }
            if !report.failures.contains(&SyncFailure::PortfolioReset) {
                self.folder_store.remove_manifest(&device.device_id).await?;
            }
        }
        self.state_repo.discard_device_state().await?;
        self.forget_runs();
        Ok(())
    }

    /// Dismisses a conflict notice (SYN-066). `NoticeNotFound` when it does not exist.
    pub async fn dismiss_conflict_notice(&self, notice_id: String) -> Result<(), SyncError> {
        let _device = self.require_device().await?;
        self.state_repo.dismiss_notice(&notice_id).await
    }

    /// Assembles the sync-owned half of `SyncStatus` (SYN-063): the device, the last run,
    /// the undismissed notices (SYN-066), and the held-back changes (SYN-041). While
    /// disabled, `enabled` is `false`, the `Option` fields are `None`, and every collection
    /// is empty.
    pub async fn status(&self) -> Result<SyncStatus, SyncError> {
        let Some(device) = self.state_repo.get_device().await? else {
            return Ok(SyncStatus::disabled());
        };
        let mut status = self.status_of(&device);
        status.notices = self.state_repo.list_undismissed_notices().await?;
        let held_back = self.state_repo.list_held_back().await?;
        status.held_back_count = held_back.len() as u32;
        status.oldest_held_back_since = held_back.first().map(|change| change.held_since.clone());
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::MockFolderStore;
    use crate::context::sync::domain::MockSyncStateRepository;

    fn paused_device() -> SyncDevice {
        SyncDevice::restore(crate::context::sync::StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: true,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    fn quiet_report(completed_at: &str) -> SyncReport {
        SyncReport {
            published_changes: 0,
            applied_changes: 0,
            held_back_changes: 0,
            dropped_changes: 0,
            notices_raised: 0,
            failures: vec![],
            completed_at: completed_at.into(),
            status: SyncStatus::for_device(&active_device(), None, vec![]),
        }
    }

    fn service_with_bus() -> (SyncService, Arc<SideEffectEventBus>) {
        let bus = Arc::new(SideEffectEventBus::new());
        let service = SyncService::new(
            Arc::new(MockSyncStateRepository::new()),
            Arc::new(MockFolderStore::new()),
        )
        .with_event_bus(Arc::clone(&bus));
        (service, bus)
    }

    #[test]
    fn first_remembered_run_raises_sync_completed() {
        let (service, bus) = service_with_bus();
        let mut rx = bus.subscribe();

        service.remember_run(&quiet_report("2026-08-22T10:00:00Z"));

        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), Event::SyncCompleted);
    }

    // SYN-064 — a join announces the rebuilt portfolio like a run that applied changes.
    #[test]
    fn announce_join_raises_sync_completed() {
        let (service, bus) = service_with_bus();
        let mut rx = bus.subscribe();

        service.announce_join();

        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), Event::SyncCompleted);
    }

    #[test]
    fn a_quiet_publish_after_an_identical_run_raises_nothing() {
        let (service, bus) = service_with_bus();
        service.remember_run(&quiet_report("2026-08-22T10:00:00Z"));
        let mut rx = bus.subscribe();
        rx.borrow_and_update();

        service.remember_run(&quiet_report("2026-08-22T10:00:05Z"));

        assert!(!rx.has_changed().unwrap());
    }

    #[test]
    fn applied_changes_or_new_failures_raise_sync_completed() {
        let (service, bus) = service_with_bus();
        service.remember_run(&quiet_report("2026-08-22T10:00:00Z"));
        let mut rx = bus.subscribe();
        rx.borrow_and_update();

        let mut applied = quiet_report("2026-08-22T10:00:05Z");
        applied.applied_changes = 1;
        service.remember_run(&applied);
        assert!(rx.has_changed().unwrap());
        rx.borrow_and_update();

        let mut failed = quiet_report("2026-08-22T10:00:10Z");
        failed.failures = vec![SyncFailure::PortfolioReset];
        service.remember_run(&failed);
        assert!(rx.has_changed().unwrap());
    }

    fn active_device() -> SyncDevice {
        SyncDevice::restore(crate::context::sync::StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    // SYN-010 — pause_sync rejects SyncDisabled while sync has never been enabled.
    #[tokio::test]
    async fn pause_sync_rejects_when_disabled() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.pause_sync().await;
        assert!(matches!(result, Err(SyncError::SyncDisabled)));
    }

    // SYN-070 — pause_sync rejects AlreadyPaused on an already-paused device.
    #[tokio::test]
    async fn pause_sync_rejects_when_already_paused() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(paused_device())));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.pause_sync().await;
        assert!(matches!(result, Err(SyncError::AlreadyPaused)));
    }

    // SYN-070 — pause_sync saves the paused device and reports it paused.
    #[tokio::test]
    async fn pause_sync_saves_and_reports_the_paused_device() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        state_repo
            .expect_save_device()
            .withf(|device| device.paused)
            .times(1)
            .returning(|_| Ok(()));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let status = service.pause_sync().await.unwrap();
        assert!(status.enabled);
        assert!(status.paused);
        assert_eq!(status.device_id.as_deref(), Some("desktop-device"));
    }

    // SYN-010 — resume_sync_precondition rejects SyncDisabled while never enabled.
    #[tokio::test]
    async fn resume_sync_precondition_rejects_when_disabled() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.resume_sync_precondition().await;
        assert!(matches!(result, Err(SyncError::SyncDisabled)));
    }

    // SYN-073 — resume_sync_precondition rejects NotPaused on an active device.
    #[tokio::test]
    async fn resume_sync_precondition_rejects_when_not_paused() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.resume_sync_precondition().await;
        assert!(matches!(result, Err(SyncError::NotPaused)));
    }

    // SYN-018/072 — rename_sync_device rejects a blank name.
    #[tokio::test]
    async fn rename_sync_device_rejects_blank_name() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.rename_sync_device("   ".into()).await;
        assert!(matches!(result, Err(SyncError::DeviceNameBlank)));
    }

    // SYN-010 — rename_sync_device rejects SyncDisabled while never enabled.
    #[tokio::test]
    async fn rename_sync_device_rejects_when_disabled() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.rename_sync_device("Laptop".into()).await;
        assert!(matches!(result, Err(SyncError::SyncDisabled)));
    }

    // SYN-072 — rename_sync_device saves the new name and reports it.
    #[tokio::test]
    async fn rename_sync_device_saves_and_reports_the_new_name() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        state_repo
            .expect_save_device()
            .withf(|device| device.device_name == "Laptop")
            .times(1)
            .returning(|_| Ok(()));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let status = service.rename_sync_device("Laptop".into()).await.unwrap();
        assert_eq!(status.device_name.as_deref(), Some("Laptop"));
    }

    // SYN-010 — leave_sync rejects SyncDisabled while never enabled.
    #[tokio::test]
    async fn leave_sync_rejects_when_disabled() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service.leave_sync().await;
        assert!(matches!(result, Err(SyncError::SyncDisabled)));
    }

    // SYN-082/069 — leave_sync rejects FolderUnavailable before touching anything, so
    // unpublished work is never abandoned.
    #[tokio::test]
    async fn leave_sync_rejects_folder_unavailable_and_keeps_the_device() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        state_repo.expect_discard_device_state().times(0);
        let mut folder_store = MockFolderStore::new();
        folder_store.expect_retarget().return_const(());
        folder_store
            .expect_check_available()
            .returning(|| Err(crate::context::sync::domain::FolderProblem::Unmounted));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(folder_store));
        let result = service.leave_sync().await;
        assert!(matches!(
            result,
            Err(SyncError::FolderUnavailable {
                problem: crate::context::sync::domain::FolderProblem::Unmounted
            })
        ));
    }

    // SYN-066 — dismiss_conflict_notice surfaces NoticeNotFound for an unknown id.
    #[tokio::test]
    async fn dismiss_conflict_notice_surfaces_notice_not_found() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        state_repo.expect_dismiss_notice().returning(|notice_id| {
            Err(SyncError::NoticeNotFound {
                notice_id: notice_id.to_string(),
            })
        });
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let result = service
            .dismiss_conflict_notice("does-not-exist".into())
            .await;
        assert!(matches!(
            result,
            Err(SyncError::NoticeNotFound { notice_id }) if notice_id == "does-not-exist"
        ));
    }

    // SYN-063 — status() returns the disabled shape: enabled=false, Option fields None,
    // collections empty.
    #[tokio::test]
    async fn status_returns_disabled_shape_when_never_enabled() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo.expect_get_device().returning(|| Ok(None));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let status = service
            .status()
            .await
            .expect("status must not error while disabled");
        assert!(!status.enabled);
        assert!(status.device_id.is_none());
        assert!(status.device_name.is_none());
        assert!(status.folder.is_none());
        assert!(status.roster.is_empty());
        assert!(status.notices.is_empty());
        assert!(status.failures.is_empty());
        assert_eq!(status.held_back_count, 0);
    }

    // SYN-063/069 — status() carries the last remembered run's completion time and failures.
    #[tokio::test]
    async fn status_reports_the_last_remembered_run() {
        let mut state_repo = MockSyncStateRepository::new();
        state_repo
            .expect_get_device()
            .returning(|| Ok(Some(active_device())));
        state_repo
            .expect_list_undismissed_notices()
            .returning(|| Ok(vec![]));
        state_repo.expect_list_held_back().returning(|| Ok(vec![]));
        let service = SyncService::new(Arc::new(state_repo), Arc::new(MockFolderStore::new()));
        let failures = vec![SyncFailure::FolderUnavailable {
            problem: crate::context::sync::domain::FolderProblem::Missing,
        }];
        let roster = vec![RosterEntry {
            device_id: "laptop-device".into(),
            device_name: "Laptop".into(),
            data_format_version: 1,
            app_version: Some("0.42.0".into()),
            last_applied_at: Some("2026-08-22T09:59:00Z".into()),
        }];
        let mut run_status = SyncStatus::for_device(&active_device(), None, vec![]);
        run_status.roster = roster.clone();
        service.remember_run(&SyncReport {
            published_changes: 0,
            applied_changes: 0,
            held_back_changes: 0,
            dropped_changes: 0,
            notices_raised: 0,
            failures: failures.clone(),
            completed_at: "2026-08-22T10:00:00Z".into(),
            status: run_status,
        });

        let status = service.status().await.unwrap();
        assert!(status.enabled);
        assert_eq!(
            status.last_sync_completed_at.as_deref(),
            Some("2026-08-22T10:00:00Z")
        );
        assert_eq!(status.failures, failures);
        assert_eq!(
            status.roster, roster,
            "SYN-063: the roster the last run read is reported"
        );
    }
}
