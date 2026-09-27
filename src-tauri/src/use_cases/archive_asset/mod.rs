/// Tauri command handler for asset archiving.
#[cfg(feature = "app")]
mod api;
/// Typed errors for the archive_asset use case (composite + use-case-owned application leaf).
mod error;
/// Cross-BC orchestrator: checks active holdings before delegating to AssetService.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::{ArchiveAssetError, ArchiveAssetTask};
pub use orchestrator::ArchiveAssetUseCase;
