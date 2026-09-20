/// Tauri command handler for asset archiving.
mod api;
/// Typed errors for the archive_asset use case (composite + use-case-owned application leaf).
mod error;
/// Cross-BC orchestrator: checks active holdings before delegating to AssetService.
mod orchestrator;

pub use api::*;
pub use error::{ArchiveAssetError, ArchiveAssetTask};
pub use orchestrator::ArchiveAssetUseCase;
