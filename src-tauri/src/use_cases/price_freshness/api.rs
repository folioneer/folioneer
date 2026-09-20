// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::PriceFreshnessError;
use super::orchestrator::{PriceFreshness, PriceFreshnessUseCase};

/// Reads the newest price date among the holdings and when this installation last
/// fetched prices, for the header's price item (MKT-200–202).
#[tauri::command]
#[specta::specta]
pub async fn get_price_freshness(
    uc: State<'_, Arc<PriceFreshnessUseCase>>,
) -> Result<PriceFreshness, PriceFreshnessError> {
    uc.read().await
}
