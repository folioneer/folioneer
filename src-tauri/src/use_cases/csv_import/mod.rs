//! CSV import use case: a broker's export read for an account through an import mapper,
//! each row matched to an asset and judged as manual entry would judge it (IMP).
//!
//! Reads the file and its rows in the account import context, matches the assets in the asset
//! context, and lets the account context judge the rows in order, nothing written.

/// Tauri command handlers for the import preview.
#[cfg(feature = "app")]
mod api;
/// The use case's failure surface.
mod error;
/// Cross-BC orchestrator.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::{CsvImportError, CsvImportTask};
pub use orchestrator::{
    CsvImportUseCase, ImportPreview, PreviewCounts, PreviewRow, RowReason, RowStatus,
};
