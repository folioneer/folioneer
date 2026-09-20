// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::{DeleteAssetError, DeleteAssetUseCase};
use tauri::State;

/// Deletes an asset, guarded against existing transactions.
#[tauri::command]
#[specta::specta]
pub async fn delete_asset(
    uc: State<'_, DeleteAssetUseCase>,
    id: String,
) -> Result<(), DeleteAssetError> {
    uc.delete_asset(&id).await
}
