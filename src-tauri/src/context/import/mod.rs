/// External API and Tauri command handlers (boundary, BC root per B39).
#[cfg(feature = "app")]
pub mod api;
/// Application layer (the `ImportService` orchestrator).
pub mod application;
/// The column mapping aggregate and its repository port.
pub mod domain;
/// Flat BC error enum (`ImportError`).
pub mod error;
/// Data persistence implementations.
pub mod infrastructure;

// Glob re-export: `collect_commands!` in specta_builder.rs resolves each command via
// `import::<cmd>`, which needs the `#[specta::specta]`-generated companion items.
#[cfg(feature = "app")]
pub use api::*;
pub use application::ImportService;
pub use domain::{
    read_file, read_rows, AssetRead, DateReading, FileRead, OperationRead, ReadingChoices,
    RowFigures, RowFormat, RowRead, RowRejection,
};
pub use domain::{
    AssetMatch, ColumnMapping, ColumnMappingDraft, ColumnMappingRepository, DateFormat,
    DecimalMark, Encoding, MappedColumns, OperationLabel, OperationType, Separator,
};
pub use error::ImportError;
pub use infrastructure::SqliteColumnMappingRepository;
