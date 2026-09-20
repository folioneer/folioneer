// The `#[tauri::command]` expansion generates an `unreachable!` arm that the crate-level
// `deny(clippy::unreachable)` would reject; allow it at this boundary module, consistent with
// the other use cases' `api.rs`.
#![allow(clippy::unreachable)]

//! The seven cross-BC sync commands (D3): everything that reads from or writes into the
//! account/asset/currency bounded contexts through `PortfolioSyncOrchestrator`.

use std::result::Result as StdResult;
use std::sync::Arc;

use tauri::State;

use crate::context::sync::{SyncFolderState, SyncReport, SyncStatus};

use super::error::PortfolioSyncError;
use super::orchestrator::PortfolioSyncOrchestrator;

/// Pre-flight read of a candidate folder (SYN-011/014/019). Never rejects.
#[tauri::command]
#[specta::specta]
pub async fn inspect_sync_folder(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
    folder: String,
) -> StdResult<SyncFolderState, PortfolioSyncError> {
    uc.inspect_sync_folder(folder).await
}

/// Enables sync on this device (SYN-011).
#[tauri::command]
#[specta::specta]
pub async fn enable_sync(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
    folder: String,
    passphrase: String,
    device_name: String,
) -> StdResult<SyncStatus, PortfolioSyncError> {
    uc.enable_sync(folder, passphrase, device_name).await
}

/// Starts the portfolio over under a new passphrase (SYN-071).
#[tauri::command]
#[specta::specta]
pub async fn start_sync_over(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
    folder: String,
    passphrase: String,
    device_name: String,
) -> StdResult<SyncStatus, PortfolioSyncError> {
    uc.start_sync_over(folder, passphrase, device_name).await
}

/// Designates a different folder for an already-enrolled device (SYN-074).
#[tauri::command]
#[specta::specta]
pub async fn change_sync_folder(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
    folder: String,
) -> StdResult<SyncStatus, PortfolioSyncError> {
    uc.change_sync_folder(folder).await
}

/// Runs a publish-only sync immediately (SYN-061).
#[tauri::command]
#[specta::specta]
pub async fn sync_now(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
) -> StdResult<SyncReport, PortfolioSyncError> {
    uc.sync_now().await
}

/// Resumes sync on a paused device (SYN-073).
#[tauri::command]
#[specta::specta]
pub async fn resume_sync(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
) -> StdResult<SyncReport, PortfolioSyncError> {
    uc.resume_sync().await
}

/// Reads the current sync status (SYN-063).
#[tauri::command]
#[specta::specta]
pub async fn get_sync_status(
    uc: State<'_, Arc<PortfolioSyncOrchestrator>>,
) -> StdResult<SyncStatus, PortfolioSyncError> {
    uc.get_sync_status().await
}
