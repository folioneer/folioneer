/// External API and Tauri command handlers (boundary, BC root per B39).
pub mod api;
/// Application layer (the `CurrencyService` orchestrator).
pub mod application;
/// Core business entities and repository traits.
pub mod domain;
/// Flat BC error enum (`CurrencyError`).
pub mod error;
/// Data persistence implementations.
pub mod infrastructure;

// Glob re-export mirrors the asset/account BCs: `collect_commands!` in
// specta_builder.rs resolves each command via `currency::<cmd>`, which needs the
// `#[specta::specta]`-generated companion items re-exported alongside the fns.
// The boundary helper `rate_f64_to_micros` is private, so only the six commands
// surface here.
pub use api::*;
pub use application::{CurrencyService, ResolvedRate};
pub use domain::{
    CurrencyPair, CurrencyPairRepository, CurrencyPairSummary, CurrencyRate,
    CurrencyRateRepository, CurrencyRateSource, EurSnapshot, RateHistoryProvider, RateProvider,
};
pub use error::CurrencyError;
pub use infrastructure::{
    ChainedRateProvider, ReqwestEcbClient, ReqwestFrankfurterClient, SqliteCurrencyPairRepository,
    SqliteCurrencyRateRepository,
};

#[cfg(test)]
pub use domain::{MockCurrencyPairRepository, MockCurrencyRateRepository};
