//! Historical exchange-rate backfill (FXR-110–114): a user-triggered download
//! of the dated daily rate series for every persisted pair, anchored at the
//! earliest transaction date, so historical valuations (yearly performance,
//! as-of views) can resolve rates instead of valuing foreign holdings at 0.

/// Tauri command handler (`backfill_currency_rate_history`).
pub mod api;
/// Flat wire-facing error enum (`RateHistoryBackfillError`).
pub mod error;
/// Orchestrator resolving the range anchor and delegating to the currency service.
pub mod orchestrator;

pub use api::*;
pub use error::RateHistoryBackfillError;
pub use orchestrator::RateHistoryBackfillUseCase;
