/// Asset persistence logic.
mod asset;
/// Asset price persistence logic.
mod asset_price;
/// Asset category persistence logic.
mod category;
/// Yahoo Finance HTTP price provider (ADR-017).
mod yahoo_client;

pub use asset::SqliteAssetRepository;
pub use asset_price::SqliteAssetPriceRepository;
pub use category::SqliteAssetCategoryRepository;
pub use yahoo_client::ReqwestYahooClient;
