/// Asset persistence logic.
mod asset;
/// Asset price persistence logic.
mod asset_price;
/// Asset category persistence logic.
mod category;
/// The External provider of an E2E run: no data for any symbol (ADR-020). Debug builds only:
/// a release build does not contain it.
#[cfg(debug_assertions)]
mod no_data_provider;

pub use asset::SqliteAssetRepository;
pub use asset_price::SqliteAssetPriceRepository;
pub use category::SqliteAssetCategoryRepository;
#[cfg(debug_assertions)]
pub use no_data_provider::NoDataProvider;
