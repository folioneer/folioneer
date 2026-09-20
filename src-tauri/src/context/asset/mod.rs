/// External API and Tauri commands.
mod api;
/// Core business entities and repository traits.
mod domain;
/// Flat BC error enum (error-model.md).
pub mod error;
/// Data persistence implementations.
mod repository;
/// Coordination layer for business operations.
mod service;

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
