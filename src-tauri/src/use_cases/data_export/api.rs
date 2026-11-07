// Allow unreachable lint as tauri::command and specta::specta macros generate false positives
#![allow(clippy::unreachable)]

use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

use super::error::DataExportError;
use super::orchestrator::{DataExportUseCase, ExportFormat, ExportReport};

/// EXP-010 to EXP-016 — writes every account, asset, transaction and price into `folder`,
/// as CSV or JSON, and says where and how much.
#[tauri::command]
#[specta::specta]
pub async fn export_portfolio(
    uc: State<'_, Arc<DataExportUseCase>>,
    format: ExportFormat,
    folder: String,
) -> Result<ExportReport, DataExportError> {
    uc.export(format, &PathBuf::from(folder)).await
}
