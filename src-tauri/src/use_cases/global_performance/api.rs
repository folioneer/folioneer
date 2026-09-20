// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::orchestrator::GlobalPerformanceUseCase;
use crate::context::account::AccountError;
use crate::use_cases::shared::performance::AccountPerformanceResponse;
use tauri::State;

/// Returns per-period performance figures for the requested scope (GPF-010):
/// all accounts aggregated in the reference currency, one asset across all
/// accounts, or — with an `account_id` — the single-account read of
/// `get_account_performance`, optionally scoped to one asset (PRF-080).
#[tauri::command]
#[specta::specta]
pub async fn get_global_performance(
    account_id: Option<String>,
    asset_id: Option<String>,
    state: State<'_, GlobalPerformanceUseCase>,
) -> Result<AccountPerformanceResponse, AccountError> {
    state
        .get_global_performance(account_id.as_deref(), asset_id.as_deref())
        .await
}
