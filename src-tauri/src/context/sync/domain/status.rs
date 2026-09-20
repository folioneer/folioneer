//! Wire shapes assembled for `SyncStatus` / `SyncReport` (SYN-063, sync-contract.md).
//! `InconsistentHolding` / `HoldingInconsistency` are declared here per the contract even
//! though nothing derives them until PR-C's `account_details` / `account_summary` orchestrators
//! exist (CFR-042) — in PR-B, `SyncStatus.inconsistent_holdings` is always empty.

use super::conflict_notice::ConflictNotice;
use super::device::SyncDevice;
use super::folder::{FolderProblem, APP_VERSION};
use serde::{Deserialize, Serialize};
use specta::Type;

/// One other device known from the roster (the manifest set, SYN-037).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RosterEntry {
    /// The other device's identity.
    pub device_id: String,
    /// Its current name.
    pub device_name: String,
    /// The data format of the application that last published from it (SYN-035).
    pub data_format_version: u32,
    /// The version of the application that last published from it; `None` when its manifest
    /// does not say (SYN-037).
    pub app_version: Option<String>,
    /// When its changes were last applied here; `None` if never.
    pub last_applied_at: Option<String>,
}

/// A holding whose merged ledger breaks an invariant (CFR-042). Derived on read, never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum HoldingInconsistency {
    /// The replayed quantity is negative.
    Oversold {
        /// The oversold quantity, in micros, negative by construction.
        quantity: i64,
    },
    /// The replayed cash balance is negative, in the account's currency.
    CashOverdrawn {
        /// The overdrawn amount, in micros, negative by construction.
        amount: i64,
    },
}

/// One inconsistent holding surfaced in sync status (SYN-040).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct InconsistentHolding {
    /// The affected account.
    pub account_id: String,
    /// Its display name.
    pub account_name: String,
    /// The affected asset.
    pub asset_id: String,
    /// Its display name.
    pub asset_name: String,
    /// Why the holding is inconsistent.
    pub reason: HoldingInconsistency,
}

/// Why the last run needs attention (SYN-063). Any number may apply at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum SyncFailure {
    /// A segment or manifest could not be decrypted or validated (SYN-034). PR-C reads.
    UnreadableFiles {
        /// How many files were skipped.
        count: u32,
    },
    /// A file in the folder is written in a data format newer than this build reads (SYN-035).
    UpdateRequired {
        /// The data format version found.
        data_format_version: u32,
    },
    /// The designated folder could not be read or written this run (SYN-069).
    FolderUnavailable {
        /// Why the folder could not be used.
        problem: FolderProblem,
    },
    /// The portfolio was started over elsewhere; this device has paused itself (SYN-084).
    PortfolioReset,
}

/// What the Settings section and the shell indicator read (SYN-063).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncStatus {
    /// Whether sync is enabled on this device.
    pub enabled: bool,
    /// Whether sync is paused on this device.
    pub paused: bool,
    /// `None` while disabled.
    pub device_id: Option<String>,
    /// `None` while disabled.
    pub device_name: Option<String>,
    /// `None` while disabled.
    pub folder: Option<String>,
    /// The version of the application running on this device (SYN-063).
    pub app_version: String,
    /// `None` when never synced.
    pub last_sync_completed_at: Option<String>,
    /// Every other device whose manifest the last run read (SYN-037/063).
    pub roster: Vec<RosterEntry>,
    /// Count of held-back changes (SYN-041). Always 0 in PR-B — nothing holds a change back
    /// until PR-C's apply path exists.
    pub held_back_count: u32,
    /// `None` when `held_back_count == 0`.
    pub oldest_held_back_since: Option<String>,
    /// Undismissed conflict notices (SYN-066). Always empty in PR-B.
    pub notices: Vec<ConflictNotice>,
    /// Derived on read from the account BC's replayed ledger (CFR-042/SYN-040). Always empty
    /// in PR-B.
    pub inconsistent_holdings: Vec<InconsistentHolding>,
    /// Empty when the last run was healthy; several may hold at once.
    pub failures: Vec<SyncFailure>,
}

impl SyncStatus {
    /// The shape reported while sync has never been enabled (SYN-063): `enabled` is
    /// `false`, every `Option` is `None`, every collection is empty.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            paused: false,
            device_id: None,
            device_name: None,
            folder: None,
            app_version: APP_VERSION.to_string(),
            last_sync_completed_at: None,
            roster: vec![],
            held_back_count: 0,
            oldest_held_back_since: None,
            notices: vec![],
            inconsistent_holdings: vec![],
            failures: vec![],
        }
    }

    /// The status of an enrolled device (SYN-063). The roster, held-back changes, notices,
    /// and inconsistent holdings are read-side products of applying other devices' changes;
    /// the caller fills them in.
    pub fn for_device(
        device: &SyncDevice,
        last_sync_completed_at: Option<String>,
        failures: Vec<SyncFailure>,
    ) -> Self {
        Self {
            enabled: true,
            paused: device.paused,
            device_id: Some(device.device_id.clone()),
            device_name: Some(device.device_name.clone()),
            folder: Some(device.folder.clone()),
            last_sync_completed_at,
            failures,
            ..Self::disabled()
        }
    }
}

/// Outcome of one run — the return value of `sync_now` / `resume_sync`; an automatic run's
/// outcome reaches the frontend through `get_sync_status` after `SyncCompleted`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncReport {
    /// Changes published this run.
    pub published_changes: u32,
    /// Changes applied this run. Always 0 in PR-B — applying starts in PR-C.
    pub applied_changes: u32,
    /// Changes held back this run. Always 0 in PR-B.
    pub held_back_changes: u32,
    /// Changes dropped this run (CFR-032). Always 0 in PR-B.
    pub dropped_changes: u32,
    /// Conflict notices raised this run. Always 0 in PR-B.
    pub notices_raised: u32,
    /// Empty when the run completed cleanly.
    pub failures: Vec<SyncFailure>,
    /// When the run finished.
    pub completed_at: String,
    /// The device state after the run (SYN-084 may have paused it).
    pub status: SyncStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::StoredDevice;

    // SYN-063 — the status states the version of the application it runs in, whether or
    // not sync is enabled.
    #[test]
    fn syn_063_the_status_states_this_build_s_app_version() {
        assert_eq!(SyncStatus::disabled().app_version, APP_VERSION);
        let device = SyncDevice::restore(StoredDevice {
            device_id: "desktop-device".into(),
            device_name: "Desktop".into(),
            folder: "/tmp/sync".into(),
            joined_at: "2026-08-22T00:00:00Z".into(),
            paused: false,
            portfolio_created_at: "2026-08-22T00:00:00Z".into(),
            data_format_version: 1,
        });
        let status = SyncStatus::for_device(&device, None, vec![]);
        assert_eq!(status.app_version, APP_VERSION);
        assert!(!APP_VERSION.is_empty());
    }
}
