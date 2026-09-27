// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use crate::context::asset::error::AssetError;
use crate::AppState;
use tauri::State;

use super::domain::{exchange, Asset, AssetCategory, AssetPrice, Exchange};
use super::dto::{CreateAssetDTO, UpdateAssetDTO};

// --- Assets ---

/// Fetches all active (non-archived) assets.
#[tauri::command]
#[specta::specta]
pub async fn get_assets(state: State<'_, AppState>) -> Result<Vec<Asset>, AssetError> {
    state.asset_service.get_all_assets().await
}

/// Fetches all assets including archived ones.
#[tauri::command]
#[specta::specta]
pub async fn get_assets_with_archived(
    state: State<'_, AppState>,
) -> Result<Vec<Asset>, AssetError> {
    state.asset_service.get_all_assets_with_archived().await
}

/// Adds a new asset.
#[tauri::command]
#[specta::specta]
pub async fn add_asset(
    state: State<'_, AppState>,
    dto: CreateAssetDTO,
) -> Result<Asset, AssetError> {
    state.asset_service.create_asset(dto).await
}

/// Updates an existing asset.
#[tauri::command]
#[specta::specta]
pub async fn update_asset(
    state: State<'_, AppState>,
    dto: UpdateAssetDTO,
) -> Result<Asset, AssetError> {
    state.asset_service.update_asset(dto).await
}

/// Unarchives an asset (R18).
#[tauri::command]
#[specta::specta]
pub async fn unarchive_asset(state: State<'_, AppState>, id: String) -> Result<(), AssetError> {
    state.asset_service.unarchive_asset(&id).await
}

/// Blocks automated price fetches for an asset (the lock — MKT-156, ADR-014).
#[tauri::command]
#[specta::specta]
pub async fn block_asset_price_refresh(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AssetError> {
    state.asset_service.block_price_refresh(&id).await
}

/// Re-allows automated price fetches for an asset (MKT-156).
#[tauri::command]
#[specta::specta]
pub async fn unblock_asset_price_refresh(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AssetError> {
    state.asset_service.unblock_price_refresh(&id).await
}

/// Returns the canonical curated set of supported trading venues (AST-021).
/// Infallible — backed by an in-binary constant.
#[tauri::command]
#[specta::specta]
pub fn get_supported_exchanges() -> Vec<Exchange> {
    exchange::all()
}

// --- Categories ---

/// Fetches all active categories.
///
/// Read-only — only infrastructure failures can fire here, so the surface is
/// the narrow `AssetError` (only `DatabaseError` is reachable).
#[tauri::command]
#[specta::specta]
pub async fn get_categories(
    state: State<'_, AppState>,
) -> Result<Vec<AssetCategory>, crate::context::asset::AssetError> {
    state.asset_service.get_all_categories().await
}

/// Creates a new category.
///
/// Returns the typed `AssetError` directly; each variant serializes as
/// `{ "code": "..." }` on the wire via `#[serde(tag = "code")]`.
#[tauri::command]
#[specta::specta]
pub async fn add_category(
    label: String,
    state: State<'_, AppState>,
) -> Result<AssetCategory, crate::context::asset::AssetError> {
    state.asset_service.create_category(&label).await
}

/// Updates an existing category.
#[tauri::command]
#[specta::specta]
pub async fn update_category(
    id: String,
    label: String,
    state: State<'_, AppState>,
) -> Result<AssetCategory, crate::context::asset::AssetError> {
    state.asset_service.update_category(&id, &label).await
}

/// Deletes a category.
#[tauri::command]
#[specta::specta]
pub async fn delete_category(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), crate::context::asset::AssetError> {
    state.asset_service.delete_category(&id).await
}

// --- AssetPrice ---

/// Records (or overwrites) a market price for an asset on a given date (MKT-024/025).
/// price is a human-readable decimal; the backend converts to i64 micros at this boundary (MKT-024).
#[tauri::command]
#[specta::specta]
pub async fn record_asset_price(
    state: State<'_, AppState>,
    asset_id: String,
    date: String,
    price: f64,
) -> Result<(), AssetError> {
    state
        .asset_service
        .record_asset_price(&asset_id, &date, price)
        .await
}

/// Returns all recorded prices for the given asset, sorted date descending (MKT-072).
#[tauri::command]
#[specta::specta]
pub async fn get_asset_prices(
    state: State<'_, AppState>,
    asset_id: String,
) -> Result<Vec<AssetPrice>, AssetError> {
    state.asset_service.get_asset_prices(&asset_id).await
}

/// Updates the date and/or price of an existing price record (MKT-083/084).
#[tauri::command]
#[specta::specta]
pub async fn update_asset_price(
    state: State<'_, AppState>,
    asset_id: String,
    original_date: String,
    new_date: String,
    new_price: f64,
) -> Result<(), AssetError> {
    state
        .asset_service
        .update_asset_price(&asset_id, &original_date, &new_date, new_price)
        .await
}

/// Deletes a specific price record by (asset_id, date) (MKT-090).
#[tauri::command]
#[specta::specta]
pub async fn delete_asset_price(
    state: State<'_, AppState>,
    asset_id: String,
    date: String,
) -> Result<(), AssetError> {
    state
        .asset_service
        .delete_asset_price(&asset_id, &date)
        .await
}
