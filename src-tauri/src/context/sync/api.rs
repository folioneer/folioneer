// The `#[tauri::command]` expansion generates an `unreachable!` arm that the crate-level
// `deny(clippy::unreachable)` would reject; allow it at this boundary module, consistent with
// the other BCs' `api.rs`.
#![allow(clippy::unreachable)]

//! BC-local Tauri commands (D3): the four sync commands that never touch another bounded
//! context. `resume_sync` and the other seven cross-BC commands live in
//! `use_cases::portfolio_sync` instead.

use std::result::Result as StdResult;
use std::sync::Arc;

use crate::context::sync::application::SyncService;
use crate::context::sync::domain::SyncStatus;
use crate::context::sync::error::SyncError;

/// Pauses sync on this device (SYN-070).
#[tauri::command]
#[specta::specta]
pub async fn pause_sync(
    svc: tauri::State<'_, Arc<SyncService>>,
) -> StdResult<SyncStatus, SyncError> {
    svc.pause_sync().await
}

/// Leaves sync on this device for good, keeping the local portfolio (SYN-082).
#[tauri::command]
#[specta::specta]
pub async fn leave_sync(svc: tauri::State<'_, Arc<SyncService>>) -> StdResult<(), SyncError> {
    svc.leave_sync().await
}

/// Renames this device (SYN-072).
#[tauri::command]
#[specta::specta]
pub async fn rename_sync_device(
    svc: tauri::State<'_, Arc<SyncService>>,
    device_name: String,
) -> StdResult<SyncStatus, SyncError> {
    svc.rename_sync_device(device_name).await
}

/// Dismisses a conflict notice (SYN-066).
#[tauri::command]
#[specta::specta]
pub async fn dismiss_conflict_notice(
    svc: tauri::State<'_, Arc<SyncService>>,
    notice_id: String,
) -> StdResult<(), SyncError> {
    svc.dismiss_conflict_notice(notice_id).await
}
