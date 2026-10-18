// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::PriceFetchFailuresError;
use super::orchestrator::PriceFetchFailuresUseCase;
use crate::use_cases::shared::price_fetch_failures::PriceFetchFailure;

/// Reads why this installation's last fetch could not price each asset (MKT-221).
#[tauri::command]
#[specta::specta]
pub async fn get_price_fetch_failures(
    uc: State<'_, Arc<PriceFetchFailuresUseCase>>,
) -> Result<Vec<PriceFetchFailure>, PriceFetchFailuresError> {
    uc.read().await
}
