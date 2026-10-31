// The `#[tauri::command]` expansion generates an `unreachable!` arm that the
// crate-level `deny(clippy::unreachable)` would reject; allow it at this boundary
// module, consistent with the other BCs' `api.rs`.
#![allow(clippy::unreachable)]

use crate::context::import::application::ImportService;
use crate::context::import::domain::{ImportMapper, ImportMapperDraft};
use crate::context::import::error::ImportError;
use crate::core::BACKEND;

use std::result::Result as StdResult;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// The mappers the folder holds, by provider (IMP-020A).
#[tauri::command]
#[specta::specta]
pub async fn list_import_mappers(
    svc: tauri::State<'_, Arc<ImportService>>,
) -> StdResult<Vec<ImportMapper>, ImportError> {
    svc.list_import_mappers().await
}

/// Writes a mapper's file: a new one without `mapper_id`, else the one it names (IMP-020,
/// IMP-024, IMP-025).
#[tauri::command]
#[specta::specta]
pub async fn save_import_mapper(
    svc: tauri::State<'_, Arc<ImportService>>,
    mapper_id: Option<String>,
    draft: ImportMapperDraft,
) -> StdResult<ImportMapper, ImportError> {
    svc.save_import_mapper(mapper_id, draft).await
}

/// IMP-020B — the system dialog opened for the user to pick a mapper's file, copied into the
/// mappers folder: the mapper it holds, or `None` when the user closed the dialog.
#[tauri::command]
#[specta::specta]
pub async fn add_import_mapper_from_file(
    app: AppHandle,
    svc: tauri::State<'_, Arc<ImportService>>,
) -> StdResult<Option<ImportMapper>, ImportError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("JSON", &["json"])
        .pick_file(move |file| {
            let _ = sender.send(file);
        });
    let Some(file) = receiver.await.ok().flatten() else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| {
        tracing::warn!(target: BACKEND, err = %e, "the picked mapper file has no path");
        ImportError::MapperFileUnreadable
    })?;
    svc.add_import_mapper_from(&path).await.map(Some)
}

/// Removes a mapper's file (IMP-040).
#[tauri::command]
#[specta::specta]
pub async fn delete_import_mapper(
    svc: tauri::State<'_, Arc<ImportService>>,
    mapper_id: String,
) -> StdResult<(), ImportError> {
    svc.delete_import_mapper(&mapper_id).await
}
