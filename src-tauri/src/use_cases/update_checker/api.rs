//! Tauri command handlers for application update management.
// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::AppHandle;

use super::error::UpdateError;
use super::service::{self, UpdateInfo, UpdateState};

/// Checks whether a new application version is available (R1, R25).
///
/// Returns `None` if the application is up to date or if the check fails due
/// to network or server errors (R21). Emits `"update:available"` on the app
/// handle if an update is found.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app_handle: AppHandle) -> Result<Option<UpdateInfo>, UpdateError> {
    Ok(service::check(&app_handle).await)
}

/// Starts downloading the available update in the background (R6).
///
/// Returns immediately — the download runs as a detached async task.
/// Progress is reported via `"update:progress"` events (R8).
/// On success, emits `"update:complete"` (R11).
/// On failure, emits `"update:error"` (R23).
/// Concurrent download requests are silently ignored (R10).
#[tauri::command]
#[specta::specta]
pub async fn download_update(
    app_handle: AppHandle,
    update_state: tauri::State<'_, Arc<UpdateState>>,
) -> Result<(), UpdateError> {
    let state = Arc::clone(&update_state);
    // service::download already logs and emits update:error on failure
    tauri::async_runtime::spawn(async move {
        let _ = service::download(app_handle, state).await;
    });
    Ok(())
}

/// Installs the downloaded update and restarts the application (R13).
///
/// Must be called after `download_update` has completed successfully.
/// Returns an error if no downloaded update is available.
#[tauri::command]
#[specta::specta]
pub async fn install_update(
    app_handle: AppHandle,
    update_state: tauri::State<'_, Arc<UpdateState>>,
) -> Result<(), UpdateError> {
    service::install(app_handle, Arc::clone(&update_state)).await
}
