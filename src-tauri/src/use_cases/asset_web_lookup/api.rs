//! Tauri command handler for asset web lookup (WEB-020).
// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::WebLookupError;
use super::orchestrator::{AssetWebLookupUseCase, LookupMode};
use super::primary_listing_processor::AssetLookupResult;

/// Searches OpenFIGI for instruments matching `query` along the explicit `mode`
/// path (WEB-014, WEB-020). Returns up to 30 results (WEB-022).
///
/// The frontend chooses the path: `Isin` validates the query against ISO 6166
/// (WEB-016) before any HTTP call and routes to `/v3/mapping`; `Keyword`
/// normalizes diacritics (WEB-015) and routes to `/v3/search`. HTTP 429
/// surfaces as `RateLimited`; every other reachability failure surfaces as
/// `NetworkError`; ISIN format failures surface as `InvalidIsinFormat` (WEB-025).
#[tauri::command]
#[specta::specta]
pub async fn lookup_asset(
    uc: tauri::State<'_, AssetWebLookupUseCase>,
    query: String,
    mode: LookupMode,
) -> Result<Vec<AssetLookupResult>, WebLookupError> {
    uc.search(query, mode).await
}
