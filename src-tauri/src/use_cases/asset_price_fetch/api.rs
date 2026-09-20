#![allow(clippy::unreachable)]

use std::sync::Arc;
use tauri::State;

use super::error::{FetchAccountAssetPricesError, FetchAllAssetPricesError};
use super::orchestrator::AssetPriceFetchUseCase;
use super::trigger::FetchTrigger;

/// Dispatches an all-accounts auto-fetch task (MKT-122, MKT-130). Keyless (ADR-017).
/// Returns `Ok(())` immediately after successful dispatch; per-asset results
/// arrive asynchronously via `AssetPriceUpdated` events (MKT-112). `trigger`
/// states which action started the fetch (PMV-010/015) — only `Manual`
/// produces a Price Movement report.
#[tauri::command]
#[specta::specta]
pub async fn fetch_all_asset_prices(
    uc: State<'_, Arc<AssetPriceFetchUseCase>>,
    trigger: FetchTrigger,
) -> Result<(), FetchAllAssetPricesError> {
    uc.fetch_all(trigger).await
}

/// Dispatches a per-account price-fetch task (MKT-132, MKT-131). Keyless (ADR-017).
/// Returns `Ok(())` immediately after successful dispatch.
#[tauri::command]
#[specta::specta]
pub async fn fetch_account_asset_prices(
    uc: State<'_, Arc<AssetPriceFetchUseCase>>,
    account_id: String,
) -> Result<(), FetchAccountAssetPricesError> {
    uc.fetch_for_account(&account_id).await
}
