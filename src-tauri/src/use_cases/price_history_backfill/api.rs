// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use crate::context::asset::PriceHistoryBackfillOutcome;

use super::error::PriceHistoryBackfillError;
use super::orchestrator::PriceHistoryBackfillUseCase;

/// Fills the price history of one holding over the held period, on the dates that
/// carry no price (MKT-190–199). Returns the closes written and the ones whose
/// date already had a price.
#[tauri::command]
#[specta::specta]
pub async fn backfill_holding_price_history(
    uc: State<'_, Arc<PriceHistoryBackfillUseCase>>,
    account_id: String,
    asset_id: String,
) -> Result<PriceHistoryBackfillOutcome, PriceHistoryBackfillError> {
    uc.backfill(&account_id, &asset_id).await
}
