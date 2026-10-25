// The `#[tauri::command]` expansion generates an `unreachable!` arm that the
// crate-level `deny(clippy::unreachable)` would reject; allow it at this boundary
// module, consistent with the other BCs' `api.rs`.
#![allow(clippy::unreachable)]

use crate::context::import::application::ImportService;
use crate::context::import::domain::{ColumnMapping, ColumnMappingDraft};
use crate::context::import::error::ImportError;

use std::result::Result as StdResult;
use std::sync::Arc;

/// Every saved column mapping, by name (IMP-020).
#[tauri::command]
#[specta::specta]
pub async fn list_column_mappings(
    svc: tauri::State<'_, Arc<ImportService>>,
) -> StdResult<Vec<ColumnMapping>, ImportError> {
    svc.list_column_mappings().await
}

/// Saves a column mapping: a new one without `mapping_id`, else the one it names, under
/// its id (IMP-020, IMP-024, IMP-025).
#[tauri::command]
#[specta::specta]
pub async fn save_column_mapping(
    svc: tauri::State<'_, Arc<ImportService>>,
    mapping_id: Option<String>,
    draft: ColumnMappingDraft,
) -> StdResult<ColumnMapping, ImportError> {
    svc.save_column_mapping(mapping_id, draft).await
}

/// Deletes a saved column mapping (IMP-040).
#[tauri::command]
#[specta::specta]
pub async fn delete_column_mapping(
    svc: tauri::State<'_, Arc<ImportService>>,
    mapping_id: String,
) -> StdResult<(), ImportError> {
    svc.delete_column_mapping(&mapping_id).await
}
