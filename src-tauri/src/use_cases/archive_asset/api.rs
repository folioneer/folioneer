// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::{ArchiveAssetError, ArchiveAssetUseCase};
use tauri::State;

/// Archives an asset, guarded against active holdings (OQ-6).
#[tauri::command]
#[specta::specta]
pub async fn archive_asset(
    uc: State<'_, ArchiveAssetUseCase>,
    id: String,
) -> Result<(), ArchiveAssetError> {
    uc.archive_asset(&id).await
}
