// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::RateRefreshError;
use super::orchestrator::RateRefreshUseCase;

/// Records the current rate of every persisted pair, after ensuring the pairs of
/// active foreign holdings (FXR-075). Called once at launch; a provider failure is
/// silent.
#[tauri::command]
#[specta::specta]
pub async fn refresh_currency_rates(
    uc: State<'_, Arc<RateRefreshUseCase>>,
) -> Result<(), RateRefreshError> {
    uc.refresh().await
}
