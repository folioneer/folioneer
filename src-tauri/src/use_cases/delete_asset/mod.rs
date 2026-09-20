/// Tauri command handler for asset hard-deletion.
mod api;
/// Typed errors for the delete_asset use case (composite + use-case-owned application leaf).
mod error;
/// Cross-BC orchestrator: checks transaction history before delegating to AssetService.
mod orchestrator;

pub use api::*;
pub use error::{DeleteAssetError, DeleteAssetTask};
pub use orchestrator::DeleteAssetUseCase;
