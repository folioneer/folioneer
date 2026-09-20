//! `SyncDevice` aggregate (SYN-016/018/070/072/084) + the `SyncStateRepository` port it is
//! persisted through (device singleton, cursors, held-back changes, conflict notices).
//!
//! Factory / mutating-method shape per CLAUDE.md § Domain Entities: `new()` validates and
//! generates an id, `restore()` reconstructs without validation, `pause`/`resume`/`rename`
//! apply a state-dependent change and return the updated aggregate, `ensure_not_paused` is a
//! fail-fast guard.

use sqlx::SqliteConnection;

use crate::context::sync::domain::conflict_notice::ConflictNotice;
use crate::context::sync::domain::cursor::SyncCursor;
use crate::context::sync::domain::held_back::HeldBackChange;
use crate::context::sync::error::SyncError;

/// The persisted shape of this device's membership, as the repository holds it — what
/// `SyncDevice::restore` reconstructs from without validation.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredDevice {
    /// The stable identity of this installation (SYN-016).
    pub device_id: String,
    /// Its user-given name (SYN-018).
    pub device_name: String,
    /// The synchronised folder on this device.
    pub folder: String,
    /// When this device joined the shared portfolio.
    pub joined_at: String,
    /// Whether sync is paused on this device (SYN-070).
    pub paused: bool,
    /// The folder header's creation mark this device follows (SYN-084).
    pub portfolio_created_at: String,
    /// The data format of the application that last published from this device (SYN-035).
    pub data_format_version: u32,
}

/// One installation participating in sync (SYN Entity Definition — SyncDevice).
#[derive(Debug, Clone, PartialEq)]
pub struct SyncDevice {
    /// The stable identity of this installation (SYN-016).
    pub device_id: String,
    /// A user-friendly label shown in sync status (SYN-018).
    pub device_name: String,
    /// The synchronised folder's location on this device.
    pub folder: String,
    /// When this device joined the shared portfolio.
    pub joined_at: String,
    /// Whether sync is currently paused on this device (SYN-070).
    pub paused: bool,
    /// Which folder header this device follows — its creation mark — so a reset is
    /// recognised (SYN-084).
    pub portfolio_created_at: String,
    /// The data format of the application that last published from this device (SYN-035).
    pub data_format_version: u32,
}

/// Rejects an empty or whitespace-only device name (SYN-018); returns the trimmed name.
pub fn ensure_device_name(device_name: &str) -> Result<String, SyncError> {
    let trimmed = device_name.trim();
    if trimmed.is_empty() {
        return Err(SyncError::DeviceNameBlank);
    }
    Ok(trimmed.to_string())
}

impl SyncDevice {
    /// Enrolls a new device (SYN-013/SYN-071's first-device branch): validates the name
    /// (SYN-018) and generates a stable identity (SYN-016).
    pub fn new(
        device_name: String,
        folder: String,
        portfolio_created_at: String,
        data_format_version: u32,
    ) -> Result<Self, SyncError> {
        let device_name = ensure_device_name(&device_name)?;
        Ok(Self {
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name,
            folder,
            joined_at: chrono::Utc::now().to_rfc3339(),
            paused: false,
            portfolio_created_at,
            data_format_version,
        })
    }

    /// Reconstructs a `SyncDevice` from storage, unvalidated (already validated at write time).
    pub fn restore(stored: StoredDevice) -> Self {
        let StoredDevice {
            device_id,
            device_name,
            folder,
            joined_at,
            paused,
            portfolio_created_at,
            data_format_version,
        } = stored;
        Self {
            device_id,
            device_name,
            folder,
            joined_at,
            paused,
            portfolio_created_at,
            data_format_version,
        }
    }

    /// Pauses sync on this device (SYN-070). Rejects `AlreadyPaused` when already paused.
    pub fn pause(self) -> Result<Self, SyncError> {
        if self.paused {
            return Err(SyncError::AlreadyPaused);
        }
        Ok(Self {
            paused: true,
            ..self
        })
    }

    /// Resumes sync on this device (SYN-073). Rejects `NotPaused` when not paused.
    pub fn resume(self) -> Result<Self, SyncError> {
        if !self.paused {
            return Err(SyncError::NotPaused);
        }
        Ok(Self {
            paused: false,
            ..self
        })
    }

    /// Renames this device (SYN-072). Rejects blank names (SYN-018).
    pub fn rename(self, device_name: String) -> Result<Self, SyncError> {
        let device_name = ensure_device_name(&device_name)?;
        Ok(Self {
            device_name,
            ..self
        })
    }

    /// Fail-fast guard used before an action that requires sync to be actively running
    /// (publishing, applying). Returns `SyncError::SyncPaused` when paused.
    pub fn ensure_not_paused(&self) -> Result<(), SyncError> {
        if self.paused {
            return Err(SyncError::SyncPaused);
        }
        Ok(())
    }

    /// Adopts a (possibly new) portfolio creation mark — used on `start_sync_over` /
    /// `change_sync_folder`'s empty-folder path, where this device becomes the new origin
    /// (SYN-071/074).
    pub fn follow_portfolio(self, portfolio_created_at: String) -> Self {
        Self {
            portfolio_created_at,
            ..self
        }
    }

    /// Re-enrolls this device as the origin of a portfolio it publishes from `folder`
    /// (SYN-071/074): it keeps its identity (SYN-016), takes `device_name`, joins now, and
    /// is no longer paused.
    pub fn re_enroll(
        self,
        device_name: String,
        folder: String,
        portfolio_created_at: String,
        data_format_version: u32,
    ) -> Result<Self, SyncError> {
        let renamed = self.rename(device_name)?;
        Ok(Self {
            folder,
            joined_at: chrono::Utc::now().to_rfc3339(),
            paused: false,
            data_format_version,
            ..renamed.follow_portfolio(portfolio_created_at)
        })
    }

    /// Designates a different folder holding the same portfolio (SYN-074).
    pub fn designate_folder(self, folder: String) -> Self {
        Self { folder, ..self }
    }
}

/// Persistence for the `sync_device` singleton, cursors, held-back changes, and conflict
/// notices (D2).
#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait SyncStateRepository: Send + Sync {
    /// Reads the singleton device row, or `None` while sync has never been enabled.
    async fn get_device(&self) -> Result<Option<SyncDevice>, SyncError>;
    /// Writes the singleton device row (insert or full overwrite). The derived key and the
    /// logical clock are owned by the enrolment transaction (`FirstPublish`) and the change
    /// recorder; this write never touches them.
    async fn save_device(&self, device: &SyncDevice) -> Result<(), SyncError>;
    /// Drops everything that makes this installation a member (SYN-082): the device row,
    /// every cursor, every held-back change, and every notice. The local portfolio is
    /// untouched.
    async fn discard_device_state(&self) -> Result<(), SyncError>;

    /// Reads the cursor for `device_id`, or `None` if never advanced.
    async fn get_cursor(&self, device_id: &str) -> Result<Option<SyncCursor>, SyncError>;
    /// Inserts or updates the cursor for its device.
    async fn upsert_cursor(&self, cursor: &SyncCursor) -> Result<(), SyncError>;
    /// `upsert_cursor` on the apply transaction's connection (SYN-065).
    async fn upsert_cursor_on(
        &self,
        conn: &mut SqliteConnection,
        cursor: &SyncCursor,
    ) -> Result<(), SyncError>;

    /// Persists a held-back change (SYN-041).
    async fn insert_held_back(&self, change: &HeldBackChange) -> Result<(), SyncError>;
    /// `insert_held_back` on the apply transaction's connection (SYN-065).
    async fn insert_held_back_on(
        &self,
        conn: &mut SqliteConnection,
        change: &HeldBackChange,
    ) -> Result<(), SyncError>;
    /// Lists every held-back change, oldest first.
    async fn list_held_back(&self) -> Result<Vec<HeldBackChange>, SyncError>;
    /// Removes a held-back change once it has been applied or dropped.
    async fn remove_held_back(&self, id: &str) -> Result<(), SyncError>;
    /// `remove_held_back` on the apply transaction's connection (SYN-065).
    async fn remove_held_back_on(
        &self,
        conn: &mut SqliteConnection,
        id: &str,
    ) -> Result<(), SyncError>;

    /// Persists a conflict notice (SYN-066).
    async fn insert_notice(&self, notice: &ConflictNotice) -> Result<(), SyncError>;
    /// `insert_notice` on the apply transaction's connection (SYN-065).
    async fn insert_notice_on(
        &self,
        conn: &mut SqliteConnection,
        notice: &ConflictNotice,
    ) -> Result<(), SyncError>;
    /// Lists undismissed notices only.
    async fn list_undismissed_notices(&self) -> Result<Vec<ConflictNotice>, SyncError>;
    /// Marks a notice dismissed. `SyncError::NoticeNotFound` when it does not exist.
    async fn dismiss_notice(&self, notice_id: &str) -> Result<(), SyncError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> SyncDevice {
        SyncDevice::restore(StoredDevice {
            device_id: "device-1".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        })
    }

    // SYN-018 — new() rejects a blank device name.
    #[test]
    fn new_rejects_blank_device_name() {
        let result = SyncDevice::new(
            "   ".into(),
            "/tmp/sync".into(),
            "2026-08-22T00:00:00Z".into(),
            1,
        );
        assert!(matches!(result, Err(SyncError::DeviceNameBlank)));
    }

    // SYN-016 — new() generates a non-empty, stable device_id.
    #[test]
    fn new_generates_a_non_empty_device_id() {
        let device = SyncDevice::new(
            "Desktop".into(),
            "/tmp/sync".into(),
            "2026-08-22T00:00:00Z".into(),
            1,
        )
        .expect("valid name must succeed");
        assert!(!device.device_id.trim().is_empty());
    }

    // SYN-016 — each enrolment draws a fresh random identity; two draws differ.
    #[test]
    fn new_generates_different_ids_across_calls() {
        let first = SyncDevice::new(
            "Desktop".into(),
            "/tmp/sync".into(),
            "2026-08-22T00:00:00Z".into(),
            1,
        )
        .unwrap();
        let second = SyncDevice::new(
            "Laptop".into(),
            "/tmp/sync".into(),
            "2026-08-22T00:00:00Z".into(),
            1,
        )
        .unwrap();
        assert_ne!(first.device_id, second.device_id);
    }

    // SYN-070 — pause() succeeds from the not-paused state.
    #[test]
    fn pause_flips_paused_flag_true() {
        let device = sample();
        assert!(!device.paused);
        let paused = device.pause().expect("not yet paused: must succeed");
        assert!(paused.paused);
    }

    // SYN-070 — pause() rejects an already-paused device.
    #[test]
    fn pause_rejects_when_already_paused() {
        let mut device = sample();
        device.paused = true;
        let result = device.pause();
        assert!(matches!(result, Err(SyncError::AlreadyPaused)));
    }

    // SYN-073 — resume() succeeds from the paused state.
    #[test]
    fn resume_flips_paused_flag_false() {
        let mut device = sample();
        device.paused = true;
        let resumed = device.resume().expect("paused: must succeed");
        assert!(!resumed.paused);
    }

    // SYN-073 — resume() rejects a not-paused device.
    #[test]
    fn resume_rejects_when_not_paused() {
        let device = sample();
        let result = device.resume();
        assert!(matches!(result, Err(SyncError::NotPaused)));
    }

    // SYN-072 — rename() rejects a blank name.
    #[test]
    fn rename_rejects_blank_name() {
        let device = sample();
        let result = device.rename("   ".into());
        assert!(matches!(result, Err(SyncError::DeviceNameBlank)));
    }

    // SYN-072 — rename() replaces the name on a valid input.
    #[test]
    fn rename_replaces_device_name() {
        let device = sample();
        let renamed = device.rename("Office Desktop".into()).unwrap();
        assert_eq!(renamed.device_name, "Office Desktop");
    }

    // SYN-070 — ensure_not_paused() passes when not paused.
    #[test]
    fn ensure_not_paused_ok_when_not_paused() {
        let device = sample();
        assert!(device.ensure_not_paused().is_ok());
    }

    // SYN-070 — ensure_not_paused() rejects when paused.
    #[test]
    fn ensure_not_paused_rejects_when_paused() {
        let mut device = sample();
        device.paused = true;
        let result = device.ensure_not_paused();
        assert!(matches!(result, Err(SyncError::SyncPaused)));
    }

    // SYN-071/074 — follow_portfolio() replaces the creation mark this device follows.
    #[test]
    fn follow_portfolio_replaces_portfolio_created_at() {
        let device = sample();
        let updated = device.follow_portfolio("2026-09-01T00:00:00Z".into());
        assert_eq!(updated.portfolio_created_at, "2026-09-01T00:00:00Z");
    }

    // SYN-071/016 — re-enrolling as a new origin keeps the identity, resumes, and adopts the
    // new name, folder, and creation mark.
    #[test]
    fn re_enroll_keeps_identity_and_adopts_the_new_portfolio() {
        let mut device = sample();
        device.paused = true;
        let re_enrolled = device
            .clone()
            .re_enroll(
                "Laptop".into(),
                "/mnt/new".into(),
                "00000000000000000009".into(),
                2,
            )
            .unwrap();
        assert_eq!(re_enrolled.device_id, device.device_id);
        assert_eq!(re_enrolled.device_name, "Laptop");
        assert_eq!(re_enrolled.folder, "/mnt/new");
        assert_eq!(re_enrolled.portfolio_created_at, "00000000000000000009");
        assert_eq!(re_enrolled.data_format_version, 2);
        assert!(!re_enrolled.paused);
    }

    // SYN-018 — re-enrolling with a blank name is rejected.
    #[test]
    fn re_enroll_rejects_blank_name() {
        let result = sample().re_enroll("  ".into(), "/mnt/new".into(), "1".into(), 1);
        assert!(matches!(result, Err(SyncError::DeviceNameBlank)));
    }

    // SYN-074 — designating a folder changes only the folder.
    #[test]
    fn designate_folder_replaces_only_the_folder() {
        let device = sample();
        let moved = device.clone().designate_folder("/mnt/moved".into());
        assert_eq!(moved.folder, "/mnt/moved");
        assert_eq!(moved.device_id, device.device_id);
        assert_eq!(moved.portfolio_created_at, device.portfolio_created_at);
    }

    // The mock repository compiles and reports expectations — a capture-site test can express
    // "save_device was called once with this device" as a hard, checkable expectation.
    #[tokio::test]
    async fn mock_sync_state_repository_compiles_and_reports_expectations() {
        let mut mock = MockSyncStateRepository::new();
        mock.expect_get_device().returning(|| Ok(None));
        assert!(mock.get_device().await.unwrap().is_none());
    }
}
