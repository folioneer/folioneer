// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use super::error::CsvImportError;
use super::orchestrator::{CsvImportUseCase, ImportPreview};
use crate::context::import::{ColumnMappingDraft, FileRead, ReadingChoices};
use tauri::State;

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
