/// Tauri command handler for asset hard-deletion.
#[cfg(feature = "app")]
mod api;
/// Typed errors for the delete_asset use case (composite + use-case-owned application leaf).
mod error;
/// Cross-BC orchestrator: checks transaction history before delegating to AssetService.
mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::{DeleteAssetError, DeleteAssetTask};
pub use orchestrator::DeleteAssetUseCase;
