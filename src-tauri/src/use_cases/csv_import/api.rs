// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::{CsvImportError, CsvImportTask};
use super::orchestrator::{CsvImportUseCase, ImportPreview};
use crate::context::import::{ColumnMappingDraft, FileRead, ReadingChoices};
use crate::core::logger::BACKEND;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

/// IMP-010 — the system dialog opened for the user to pick a file: the path the other two
/// commands take, or `None` when the user closed the dialog. Only a file picked here is read.
#[tauri::command]
#[specta::specta]
pub async fn pick_import_file(
    app: AppHandle,
    use_case: State<'_, CsvImportUseCase>,
) -> Result<Option<String>, CsvImportError> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_file(move |file| {
        let _ = sender.send(file);
    });
    let Some(file) = receiver.await.ok().flatten() else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| {
        tracing::warn!(target: BACKEND, err = %e, "import: the picked file has no path");
        CsvImportTask::FileUnreadable
    })?;
    use_case.allow_file(&path).await.map(Some)
}

/// How the file at `path` reads for `account_id`: its format, detected or chosen, and its
/// columns (IMP-010 to IMP-016, IMP-018).
#[tauri::command]
#[specta::specta]
pub async fn read_import_file(
    use_case: State<'_, CsvImportUseCase>,
    account_id: String,
    path: String,
    choices: ReadingChoices,
) -> Result<FileRead, CsvImportError> {
    use_case
        .read_import_file(&account_id, &path, &choices)
        .await
}

/// The file at `path` read through `mapping` for `account_id`, every row judged, nothing
/// written (IMP-017, IMP-020, IMP-030). `mapping_id` names the saved mapping it was picked
/// from, when it was.
#[tauri::command]
#[specta::specta]
pub async fn preview_import(
    use_case: State<'_, CsvImportUseCase>,
    account_id: String,
    path: String,
    mapping_id: Option<String>,
    mapping: ColumnMappingDraft,
) -> Result<ImportPreview, CsvImportError> {
    use_case
        .preview_import(&account_id, &path, mapping_id.as_deref(), &mapping)
        .await
}

/// The file at `path` read and judged again for `account_id`, then every ready row recorded
/// at once (IMP-050): the rows as judged then. `add_deposits`: the deposits the purchases
/// need are added, marked as added (IMP-051).
#[tauri::command]
#[specta::specta]
pub async fn confirm_import(
    use_case: State<'_, CsvImportUseCase>,
    account_id: String,
    path: String,
    mapping_id: Option<String>,
    mapping: ColumnMappingDraft,
    add_deposits: bool,
) -> Result<ImportPreview, CsvImportError> {
    use_case
        .confirm_import(
            &account_id,
            &path,
            mapping_id.as_deref(),
            &mapping,
            add_deposits,
        )
        .await
}
