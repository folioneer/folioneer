/// SQLite implementation of CurrencyPairRepository.
pub mod currency_pair;
/// SQLite implementation of CurrencyRateRepository.
pub mod currency_rate;
/// ECB XML provider (fallback, FXR-070).
pub mod ecb_client;
/// Frankfurter JSON provider (primary, FXR-070).
pub mod frankfurter_client;
/// Ordered provider chain (ADR-009, FXR-070).
pub mod rate_provider_chain;

pub use currency_pair::SqliteCurrencyPairRepository;
pub use currency_rate::SqliteCurrencyRateRepository;
pub use ecb_client::ReqwestEcbClient;
pub use frankfurter_client::ReqwestFrankfurterClient;
pub use rate_provider_chain::ChainedRateProvider;
