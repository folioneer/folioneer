/// External API and Tauri commands.
#[cfg(feature = "app")]
mod api;
/// Inputs the asset service accepts from any adapter.
mod dto;
pub use dto::{CreateAssetDTO, UpdateAssetDTO};
/// Core business entities and repository traits.
mod domain;
/// Flat BC error enum (error-model.md).
pub mod error;
/// Data persistence implementations.
mod repository;
/// Coordination layer for business operations.
mod service;

#[cfg(feature = "app")]
pub use api::*;
pub use domain::exchange;
pub use domain::isin::validate_isin;
pub use domain::*;
pub use error::AssetError;
pub use repository::*;
pub use service::*;

#[cfg(test)]
pub use domain::{
    MockAssetCategoryRepository, MockAssetPriceRepository, MockAssetRepository, MockPriceProvider,
};
