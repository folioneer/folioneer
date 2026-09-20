//! Price freshness (MKT-200–203): how recent the holdings' prices are, and when this
//! installation last fetched prices — the two figures behind the header's price item.

/// Tauri command handler (`get_price_freshness`).
pub mod api;
/// Flat wire-facing error enum (`PriceFreshnessError`).
pub mod error;
/// Orchestrator reading the newest held price date and the last fetch on this device.
pub mod orchestrator;

pub use api::*;
pub use error::PriceFreshnessError;
pub use orchestrator::{PriceFreshness, PriceFreshnessUseCase};
