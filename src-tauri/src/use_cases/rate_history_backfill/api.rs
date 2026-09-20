// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::RateHistoryBackfillError;
use super::orchestrator::RateHistoryBackfillUseCase;

/// Backfills the historical exchange-rate series for every persisted pair,
/// from the earliest transaction date across all accounts through today
/// (FXR-110–114). Returns the number of rate rows written.
#[tauri::command]
#[specta::specta]
pub async fn backfill_currency_rate_history(
    uc: State<'_, Arc<RateHistoryBackfillUseCase>>,
) -> Result<u32, RateHistoryBackfillError> {
    uc.backfill().await
}
