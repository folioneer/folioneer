//! Launch rate refresh (FXR-075): at every launch, in every build, ensures the pairs
//! of active foreign holdings and records the current rate of every persisted pair.
//! Independent of the External provider and of the price fetch tasks.

/// Tauri command handler (`refresh_currency_rates`).
pub mod api;
/// Flat wire-facing error enum (`RateRefreshError`).
pub mod error;
/// Orchestrator deriving the holding pairs and delegating to the currency service.
pub mod orchestrator;

pub use api::*;
pub use error::RateRefreshError;
pub use orchestrator::RateRefreshUseCase;
