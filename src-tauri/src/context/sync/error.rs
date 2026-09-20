//! Single flat error enum for the `sync` bounded context (gold error model, PR-B scope).
//!
//! Carries only the wire codes PR-B raises (per the plan's Phase B test-stub checklist):
//! `AlreadyEnabled`, `SyncDisabled`, `SyncPaused`, `AlreadyPaused`, `NotPaused`,
//! `PassphraseTooShort`, `DeviceNameBlank`, `FolderUnavailable`, `UpdateRequired`,
//! `PublishFailed`, `PortfolioCreatedElsewhere`, `NoticeNotFound`, `FolderHoldsOtherPortfolio`,
//! `HeaderRejected`, `DatabaseError`. `PassphraseMismatch`, `InstallationHoldsUserData`, `HistoryIncomplete`, and
//! `RebuildInterrupted` are join/rebuild codes (PR-C) — `PassphraseMismatch` is declared here
//! because SYN-055's passphrase-check verification is a PR-B mechanism (`crypto.rs`), even
//! though it is only reachable from the join path in PR-C.

use crate::context::sync::domain::folder::FolderProblem;

/// Every failure the `sync` bounded context can raise. `#[serde(tag = "code")]` makes each
/// variant serialize as `{ "code": "VariantName", ...payload }` on the wire.
#[derive(Debug, thiserror::Error, serde::Serialize, specta::Type, Clone, PartialEq)]
#[serde(tag = "code")]
pub enum SyncError {
    /// Sync is already enabled on this device (SYN-010 precondition guard).
    #[error("Sync is already enabled on this device")]
    AlreadyEnabled,

    /// Sync has never been enabled on this device (SYN-010 precondition guard).
    #[error("Sync is not enabled on this device")]
    SyncDisabled,

    /// The device is paused; running a sync is rejected (SYN-070).
    #[error("Sync is paused on this device")]
    SyncPaused,

    /// `pause_sync` on an already-paused device (SYN-070 precondition guard).
    #[error("Sync is already paused on this device")]
    AlreadyPaused,

    /// `resume_sync` on a device that is not paused (SYN-073 precondition guard).
    #[error("Sync is not paused on this device")]
    NotPaused,

    /// The passphrase is shorter than the required minimum (SYN-012).
    #[error("Passphrase must be at least {minimum} characters")]
    PassphraseTooShort {
        /// The minimum required length.
        minimum: u32,
    },

    /// The device name is empty or whitespace-only (SYN-018).
    #[error("Device name cannot be blank")]
    DeviceNameBlank,

    /// The designated folder cannot be used (SYN-019/069).
    #[error("The synchronised folder is unavailable")]
    FolderUnavailable {
        /// Why the folder cannot be used.
        problem: FolderProblem,
    },

    /// The folder holds a portfolio published in a data format newer than this build reads
    /// (SYN-019/035).
    #[error("The portfolio was published in a newer data format")]
    UpdateRequired {
        /// The data format version found in the folder.
        data_format_version: u32,
    },

    /// The folder header cannot be used: its key-derivation parameters are outside the range
    /// this build runs with (SYN-051) — a corrupt or hostile header, never one this device
    /// can derive a key against.
    #[error("The folder header cannot be used")]
    HeaderRejected,

    /// The passphrase does not match the portfolio's passphrase check (SYN-015/055). Only
    /// reachable from the join path, shipped in PR-C.
    #[error("The passphrase does not match")]
    PassphraseMismatch,

    /// Publishing failed partway; everything written was rolled back (SYN-013).
    #[error("Publishing failed")]
    PublishFailed {
        /// Why publishing could not complete.
        problem: FolderProblem,
    },

    /// Another device published the folder header between this device's pre-check and its
    /// own write (SYN-081).
    #[error("Another device created the portfolio first")]
    PortfolioCreatedElsewhere,

    /// `dismiss_conflict_notice` for a notice that does not exist (SYN-066).
    #[error("Notice not found: {notice_id}")]
    NoticeNotFound {
        /// The unknown notice id.
        notice_id: String,
    },

    /// `change_sync_folder` to a non-empty folder whose passphrase check does not match the
    /// kept key (SYN-074).
    #[error("The folder holds a different portfolio")]
    FolderHoldsOtherPortfolio,

    /// An infrastructure / database failure occurred. The full diagnostic is preserved
    /// server-side via `tracing::error!`; the wire surface carries no hint.
    #[error("An unexpected database error occurred")]
    DatabaseError,
}

impl SyncError {
    /// Logs an infrastructure failure under `context` and returns `DatabaseError`.
    pub fn database(context: &'static str, error: impl std::fmt::Debug) -> Self {
        tracing::error!(target: crate::core::logger::BACKEND, err = ?error, "{context}");
        Self::DatabaseError
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    // error-model.md — every SyncError variant serializes to a flat `{ "code": ... }` object.
    #[test]
    fn each_variant_emits_a_code() {
        assert_eq!(
            to_value(SyncError::AlreadyEnabled).unwrap(),
            json!({ "code": "AlreadyEnabled" })
        );
        assert_eq!(
            to_value(SyncError::SyncDisabled).unwrap(),
            json!({ "code": "SyncDisabled" })
        );
        assert_eq!(
            to_value(SyncError::SyncPaused).unwrap(),
            json!({ "code": "SyncPaused" })
        );
        assert_eq!(
            to_value(SyncError::AlreadyPaused).unwrap(),
            json!({ "code": "AlreadyPaused" })
        );
        assert_eq!(
            to_value(SyncError::NotPaused).unwrap(),
            json!({ "code": "NotPaused" })
        );
        assert_eq!(
            to_value(SyncError::PassphraseTooShort { minimum: 12 }).unwrap(),
            json!({ "code": "PassphraseTooShort", "minimum": 12 })
        );
        assert_eq!(
            to_value(SyncError::DeviceNameBlank).unwrap(),
            json!({ "code": "DeviceNameBlank" })
        );
        assert_eq!(
            to_value(SyncError::FolderUnavailable {
                problem: FolderProblem::Missing
            })
            .unwrap(),
            json!({ "code": "FolderUnavailable", "problem": "Missing" })
        );
        assert_eq!(
            to_value(SyncError::UpdateRequired {
                data_format_version: 2
            })
            .unwrap(),
            json!({ "code": "UpdateRequired", "data_format_version": 2 })
        );
        assert_eq!(
            to_value(SyncError::HeaderRejected).unwrap(),
            json!({ "code": "HeaderRejected" })
        );
        assert_eq!(
            to_value(SyncError::PassphraseMismatch).unwrap(),
            json!({ "code": "PassphraseMismatch" })
        );
        assert_eq!(
            to_value(SyncError::PublishFailed {
                problem: FolderProblem::OutOfSpace
            })
            .unwrap(),
            json!({ "code": "PublishFailed", "problem": "OutOfSpace" })
        );
        assert_eq!(
            to_value(SyncError::PortfolioCreatedElsewhere).unwrap(),
            json!({ "code": "PortfolioCreatedElsewhere" })
        );
        assert_eq!(
            to_value(SyncError::NoticeNotFound {
                notice_id: "notice-1".into()
            })
            .unwrap(),
            json!({ "code": "NoticeNotFound", "notice_id": "notice-1" })
        );
        assert_eq!(
            to_value(SyncError::FolderHoldsOtherPortfolio).unwrap(),
            json!({ "code": "FolderHoldsOtherPortfolio" })
        );
        assert_eq!(
            to_value(SyncError::DatabaseError).unwrap(),
            json!({ "code": "DatabaseError" })
        );
    }
}
