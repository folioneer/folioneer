//! Update checker service — detects, downloads, and installs application updates.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use super::error::UpdateError;
use crate::core::BACKEND;

/// Information about an available application update.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct UpdateInfo {
    /// Semantic version string of the available update (e.g. "1.2.3").
    pub version: String,
}

/// Shared state for the update lifecycle, managed across Tauri commands.
///
/// Tracks whether a download is in progress (R10) and stores downloaded bytes
/// between the download command and the install command.
#[derive(Debug, Default)]
pub struct UpdateState {
    /// True while a download is in progress — prevents concurrent downloads (R10).
    pub is_downloading: AtomicBool,
    /// Downloaded installer bytes stored after a successful download (R9).
    downloaded_bytes: Mutex<Option<Vec<u8>>>,
}

impl UpdateState {
    /// Creates a new, empty update state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores downloaded bytes for later installation.
    pub fn set_bytes(&self, bytes: Vec<u8>) {
        if let Ok(mut guard) = self.downloaded_bytes.lock() {
            *guard = Some(bytes);
        }
    }

    /// Takes the downloaded bytes, clearing the stored value.
    pub fn take_bytes(&self) -> Option<Vec<u8>> {
        self.downloaded_bytes.lock().ok()?.take()
    }
}

/// Checks whether a new application version is available.
///
/// Returns `None` silently on network or server errors (R21), logging them for
/// diagnostics (R22). Emits `"update:available"` on the app handle if an update
/// is found, so that all listeners (banner, manual check) react consistently.
pub async fn check(app_handle: &AppHandle) -> Option<UpdateInfo> {
    let updater = match app_handle.updater() {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(target: BACKEND, error = %e, "Failed to initialize updater (R22)");
            return None;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            let version = update.version.clone();
            tracing::info!(target: BACKEND, version = %version, "Update available");
            let info = UpdateInfo { version };
            let _ = app_handle.emit("update:available", &info);
            Some(info)
        }
        Ok(None) => {
            tracing::info!(target: BACKEND, "Application is up to date");
            None
        }
        Err(e) => {
            tracing::warn!(target: BACKEND, error = %e, "Update check failed — silent (R21, R22)");
            None
        }
    }
}

/// Downloads the available update in the background, emitting progress events (R8).
///
/// Does nothing if a download is already in progress (R10).
/// Emits `"update:progress"` (percent 0–100) during download,
/// `"update:complete"` on success, or `"update:error"` on failure (R23).
/// Checksum verification is performed by the Tauri updater plugin (R9).
pub async fn download(app_handle: AppHandle, state: Arc<UpdateState>) -> Result<(), UpdateError> {
    // R10 — prevent concurrent downloads
    if state
        .is_downloading
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::warn!(target: BACKEND, "Download already in progress — ignoring (R10)");
        return Ok(());
    }

    let result = do_download(&app_handle, &state).await;
    state.is_downloading.store(false, Ordering::SeqCst);

    if let Err(ref error) = result {
        // The underlying cause is logged inside do_download; emit the typed error
        // (never the raw cause string) so the renderer can localise it (R23).
        let _ = app_handle.emit("update:error", error);
    }

    result
}

async fn do_download(app_handle: &AppHandle, state: &UpdateState) -> Result<(), UpdateError> {
    let updater = app_handle.updater().map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Failed to initialize updater (R23)");
        UpdateError::OperationFailed
    })?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            tracing::error!(target: BACKEND, error = %e, "Failed to check for update during download (R23)");
            UpdateError::OperationFailed
        })?
        .ok_or_else(|| {
            tracing::error!(target: BACKEND, "No update available to download (R23)");
            UpdateError::OperationFailed
        })?;

    let downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let ah = app_handle.clone();

    // R8 — emit progress events; checksum is verified by the plugin (R9)
    let bytes = update
        .download(
            move |chunk, total| {
                let current = downloaded.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let percent = total
                    .and_then(|t| (current * 100).checked_div(t))
                    .map(|p| p.min(100))
                    .unwrap_or(0);
                let _ = ah.emit("update:progress", percent);
            },
            || {},
        )
        .await
        .map_err(|e| {
            tracing::error!(target: BACKEND, error = %e, "Download or checksum verification failed (R9, R23)");
            UpdateError::OperationFailed
        })?;

    // Store bytes BEFORE emitting complete — prevents install racing (R11)
    state.set_bytes(bytes);
    let _ = app_handle.emit("update:complete", ());
    tracing::info!(target: BACKEND, "Update downloaded and checksum verified (R9)");
    Ok(())
}

/// Installs the previously downloaded update and restarts the application (R13).
///
/// Re-checks for the update to obtain a fresh handle for the install call.
/// Requires that `download` has been called successfully beforehand.
pub async fn install(app_handle: AppHandle, state: Arc<UpdateState>) -> Result<(), UpdateError> {
    let bytes = state.take_bytes().ok_or(UpdateError::NoDownloadedUpdate)?;

    let updater = app_handle.updater().map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Failed to initialize updater for install");
        UpdateError::OperationFailed
    })?;

    let update = updater
        .check()
        .await
        .map_err(|e| {
            tracing::error!(target: BACKEND, error = %e, "Failed to get update for installation");
            UpdateError::OperationFailed
        })?
        .ok_or_else(|| {
            tracing::error!(target: BACKEND, "No update found for installation");
            UpdateError::OperationFailed
        })?;

    update.install(bytes).map_err(|e| {
        tracing::error!(target: BACKEND, error = %e, "Installation failed");
        UpdateError::OperationFailed
    })?;
    tracing::info!(target: BACKEND, "Update installed — restarting application (R13)");
    app_handle.restart();
}
