//! Data export (EXP): every account, asset, transaction and price written as CSV or JSON in
//! a documented format the account import reads back.

/// Tauri command handler (`export_portfolio`).
#[cfg(feature = "app")]
pub mod api;
/// Flat wire-facing error enum (`DataExportError`).
pub mod error;
/// The format: cells, CSV, JSON, file names and the import mapper (pure).
pub mod format;
/// Orchestrator reading the portfolio and writing the export, complete or not at all.
pub mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::DataExportError;
pub use orchestrator::{DataExportUseCase, ExportFormat, ExportReport};
