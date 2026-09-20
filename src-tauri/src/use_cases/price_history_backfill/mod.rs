//! Price history backfill of one holding (MKT-190–199): a user-triggered download
//! of the daily closes over the period the account held the asset, recorded only
//! on the dates that carry no price, so past valuations stop showing gaps.

/// Tauri command handler (`backfill_holding_price_history`).
pub mod api;
/// Flat wire-facing error enum (`PriceHistoryBackfillError`).
pub mod error;
/// Orchestrator resolving the held period and fetching the closes in windows.
pub mod orchestrator;

pub use api::*;
pub use error::{PriceHistoryBackfillError, PriceHistoryBackfillTask};
pub use orchestrator::PriceHistoryBackfillUseCase;
