// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::orchestrator::AccountPerformanceUseCase;
use crate::context::account::AccountError;
use crate::use_cases::shared::performance::AccountPerformanceResponse;
use tauri::State;

/// Returns per-period performance figures for a single account (PRF spec),
/// optionally scoped to one asset's position (PRF-080).
#[tauri::command]
#[specta::specta]
pub async fn get_account_performance(
    account_id: String,
    asset_id: Option<String>,
    state: State<'_, AccountPerformanceUseCase>,
) -> Result<AccountPerformanceResponse, AccountError> {
    state
        .get_account_performance(&account_id, asset_id.as_deref())
        .await
}
