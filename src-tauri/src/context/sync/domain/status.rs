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
    /// How many changes it has published: the last sequence its manifest states, which is
    /// a count because a device's sequences start at 1 and leave no gap (SYN-025, SYN-036);
    /// 0 when it has joined and published nothing yet.
    pub published_changes: i64,
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

/// SYN-063 — whether sync needs a look, in one word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncHealth {
    /// Sync is paused on this device.
    Paused,
    /// The status carries a failure, a held-back change, a notice or an inconsistent holding.
    NeedsAttention,
    /// Nothing waits for the user.
    UpToDate,
}

impl SyncStatus {
    /// SYN-063 — the health of sync, from the status alone: paused first, then anything
    /// that waits for the user.
    pub fn health(&self) -> SyncHealth {
        if self.paused {
            SyncHealth::Paused
        } else if self.failures.is_empty()
            && self.notices.is_empty()
            && self.inconsistent_holdings.is_empty()
            && self.held_back_count == 0
        {
            SyncHealth::UpToDate
        } else {
            SyncHealth::NeedsAttention
        }
    }
}

/// A sync status as the interface reads it (SYN-063): the status and the health it
/// states, computed when the status leaves the core so the two cannot disagree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SyncStatusView {
    /// The status.
    #[serde(flatten)]
    pub status: SyncStatus,
    /// The health that status states.
    pub health: SyncHealth,
}

impl std::ops::Deref for SyncStatusView {
    type Target = SyncStatus;

    /// The status the view carries: its fields read through the view.
    fn deref(&self) -> &SyncStatus {
        &self.status
    }
}

impl From<SyncStatus> for SyncStatusView {
    fn from(status: SyncStatus) -> Self {
        Self {
            health: status.health(),
            status,
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

/// A run's outcome as the interface reads it: the report, its status with its health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SyncReportView {
    /// Changes published this run.
    pub published_changes: u32,
    /// Changes applied this run.
    pub applied_changes: u32,
    /// Changes held back this run.
    pub held_back_changes: u32,
    /// Changes dropped this run (CFR-032).
    pub dropped_changes: u32,
    /// Conflict notices raised this run.
    pub notices_raised: u32,
    /// Empty when the run completed cleanly.
    pub failures: Vec<SyncFailure>,
    /// When the run finished.
    pub completed_at: String,
    /// The device state after the run, with its health.
    pub status: SyncStatusView,
}

impl From<SyncReport> for SyncReportView {
    fn from(report: SyncReport) -> Self {
        // Destructured in full: a field added to the report does not compile until it is
        // carried here.
        let SyncReport {
            published_changes,
            applied_changes,
            held_back_changes,
            dropped_changes,
            notices_raised,
            failures,
            completed_at,
            status,
        } = report;
        Self {
            published_changes,
            applied_changes,
            held_back_changes,
            dropped_changes,
            notices_raised,
            failures,
            completed_at,
            status: status.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::sync::domain::{ConflictNoticeKind, StoredDevice};
    use crate::shared::domain::RecordKind;

    // SYN-063 — the health of sync, from the status alone: paused before anything else;
    // then a failure, a held-back change, a notice or an inconsistent holding each call
    // for attention on its own; a status that carries none is up to date. The status
    // leaves for the interface with that health.
    #[test]
    fn syn_063_the_health_of_sync_follows_the_status() {
        let enrolled = || {
            let mut status = SyncStatus::disabled();
            status.enabled = true;
            status
        };
        assert_eq!(enrolled().health(), SyncHealth::UpToDate);

        let mut failed = enrolled();
        failed.failures = vec![SyncFailure::PortfolioReset];
        let mut held_back = enrolled();
        held_back.held_back_count = 1;
        let mut inconsistent = enrolled();
        inconsistent.inconsistent_holdings = vec![InconsistentHolding {
            account_id: "acc-1".into(),
            account_name: "Brokerage".into(),
            asset_id: "asset-1".into(),
            asset_name: "AAPL".into(),
            reason: HoldingInconsistency::Oversold {
                quantity: -5_000_000,
            },
        }];
        let mut noticed = enrolled();
        noticed.notices = vec![ConflictNotice {
            notice_id: "notice-1".into(),
            kind: ConflictNoticeKind::OverruledEdit,
            record_kind: RecordKind::Transaction,
            record_identity: "tx-1".into(),
            record_label: "Sell 10 AAPL".into(),
            other_device_id: "device-2".into(),
            other_device_name: "Laptop".into(),
            raised_at: "2026-08-20T10:00:00Z".into(),
        }];
        for needing in [failed.clone(), held_back, inconsistent, noticed] {
            assert_eq!(needing.health(), SyncHealth::NeedsAttention);
        }

        let mut only_paused = enrolled();
        only_paused.paused = true;
        assert_eq!(only_paused.health(), SyncHealth::Paused);
        let mut paused = failed;
        paused.paused = true;
        assert_eq!(paused.health(), SyncHealth::Paused);

        let view = SyncStatusView::from(paused.clone());
        assert_eq!((view.health, view.status), (SyncHealth::Paused, paused));
    }

    // SYN-063 — a run's report leaves for the interface with every figure it carried, and
    // its status with its health.
    #[test]
    fn syn_063_a_report_keeps_its_figures_and_states_the_health_of_its_status() {
        let mut status = SyncStatus::disabled();
        status.enabled = true;
        status.held_back_count = 2;
        let report = SyncReport {
            published_changes: 1,
            applied_changes: 2,
            held_back_changes: 3,
            dropped_changes: 4,
            notices_raised: 5,
            failures: vec![SyncFailure::PortfolioReset],
            completed_at: "2026-08-21T09:00:00Z".into(),
            status: status.clone(),
        };
        let view = SyncReportView::from(report);
        assert_eq!(
            (
                view.published_changes,
                view.applied_changes,
                view.held_back_changes,
                view.dropped_changes,
                view.notices_raised
            ),
            (1, 2, 3, 4, 5)
        );
        assert_eq!(view.failures, vec![SyncFailure::PortfolioReset]);
        assert_eq!(view.completed_at, "2026-08-21T09:00:00Z");
        assert_eq!(view.status.health, SyncHealth::NeedsAttention);
        assert_eq!(view.status.held_back_count, status.held_back_count);
    }

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
