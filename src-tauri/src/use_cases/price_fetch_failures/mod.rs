//! Why this installation's last fetch could not price an asset (MKT-221): the reasons a
//! holding row shows where its price does.

/// Tauri command handler (`get_price_fetch_failures`).
#[cfg(feature = "app")]
pub mod api;
/// Flat wire-facing error enum (`PriceFetchFailuresError`).
pub mod error;
/// Orchestrator reading the recorded failures, leaving out the prices typed by hand.
pub mod orchestrator;

#[cfg(feature = "app")]
pub use api::*;
pub use error::PriceFetchFailuresError;
pub use orchestrator::PriceFetchFailuresUseCase;
