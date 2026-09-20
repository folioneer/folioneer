/// Asset aggregate and repository trait.
pub mod asset;
/// AssetPrice aggregate, repository trait, AssetPriceSource, and PriceProvider trait.
pub mod asset_price;
/// AssetCategory aggregate and repository trait.
pub mod category;
/// Canonical trading venue value object (Exchange) and curated set (AST-021).
pub mod exchange;
/// ISIN format validator and error type (WEB-016).
pub mod isin;
/// OpenFIGI inbound exchange mapper — `micCode` / `exchCode` → `Exchange` (WEB-049).
pub mod openfigi_exchange_mapper;
/// Yahoo outbound exchange mapper — `Exchange` → Yahoo suffix (MKT-110).
pub mod yahoo_exchange_mapper;
/// Yahoo provider symbol derivation from asset reference (MKT-110, ADR-017).
pub mod yahoo_symbol;

pub use asset::*;
pub use asset_price::{
    AssetPrice, AssetPriceRepository, AssetPriceSource, DatedClose, PriceHistoryBackfillOutcome,
    PriceProvider, Quote,
};
pub use category::*;
pub use exchange::Exchange;
pub use yahoo_symbol::{derive_yahoo_symbol, derive_yahoo_symbol_with_exchange};

#[cfg(test)]
pub use asset::MockAssetRepository;
#[cfg(test)]
pub use asset_price::{MockAssetPriceRepository, MockPriceProvider};
#[cfg(test)]
pub use category::MockAssetCategoryRepository;
