/// External API and Tauri command handlers (boundary, BC root per B39).
#[cfg(feature = "app")]
pub mod api;
/// Application layer (the `ImportService` orchestrator).
pub mod application;
/// The import mapper aggregate and its ports.
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
    folded, read_file, read_rows, AssetRead, DateReading, FileRead, OperationRead, ReadingChoices,
    RowFigures, RowFormat, RowRead, RowRejection, LARGEST_FILE,
};
pub use domain::{
    AssetMatch, DateFormat, DecimalMark, Encoding, ImportMapper, ImportMapperDraft,
    ImportMapperStore, MappedColumns, MapperChoiceRepository, MapperInfo, OperationLabel,
    OperationType, Separator,
};
pub use error::ImportError;
pub use infrastructure::{FsImportMapperStore, SqliteMapperChoiceRepository};
